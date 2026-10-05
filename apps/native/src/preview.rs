//! Blitz-specific presenter. Conversion textures never enter this module.
use dioxus::native::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use media_native::{
    CancellationToken,
    playback::{self, PlaybackControl},
    preview::{self, PreviewFrame},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

#[derive(Default)]
pub struct FrameSlot {
    frame: Option<PreviewFrame>,
    revision: u64,
    color: media_core::ColorAdjustments,
    color_revision: u64,
}

struct Job {
    cancel: CancellationToken,
    control: Arc<PlaybackControl>,
    thread: JoinHandle<()>,
}

#[derive(Clone, Default, PartialEq)]
pub struct PlayerState {
    pub ready: bool,
    pub paused: bool,
    pub ended: bool,
    pub position_us: u64,
    pub duration_us: u64,
    pub audio_active: bool,
    pub muted: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct PreviewService {
    pub frames: Arc<Mutex<FrameSlot>>,
    job: Mutex<Option<Job>>,
    closed: AtomicBool,
}

impl PreviewService {
    pub fn set_color(&self, color: media_core::ColorAdjustments) {
        if let Ok(mut slot) = self.frames.lock()
            && slot.color != color
        {
            slot.color = color;
            slot.color_revision = slot.color_revision.wrapping_add(1);
        }
    }
    pub fn start(
        &self,
        path: PathBuf,
        position_us: u64,
        playing: bool,
        muted: bool,
    ) -> Result<(), String> {
        let mut job = self.job.lock().map_err(|_| "preview job lock poisoned")?;
        if self.closed.load(Ordering::Acquire) {
            return Err("Preview service is shut down".into());
        }
        let old = job.take();
        if let Some(old) = &old {
            old.cancel.cancel();
        }
        self.clear();
        let cancel = CancellationToken::default();
        let token = cancel.clone();
        let frames = self.frames.clone();
        let control = Arc::new(PlaybackControl::default());
        control.paused.store(!playing, Ordering::Release);
        control.muted.store(muted, Ordering::Release);
        control.position_us.store(position_us, Ordering::Release);
        let worker_control = control.clone();
        let thread = std::thread::spawn(move || {
            // Joining happens off the UI thread. A seek never overlaps codec trees.
            if let Some(old) = old {
                let _ = old.thread.join();
            }
            let result = playback::play(&path, position_us, &worker_control, &token, |frame| {
                let mut slot = frames.lock().map_err(|_| "preview frame lock poisoned")?;
                if token.is_cancelled() {
                    return Err("Preview cancelled".into());
                }
                slot.revision = slot.revision.wrapping_add(1);
                slot.frame = Some(frame);
                Ok(())
            });
            if !token.is_cancelled() {
                if let Err(error) = result
                    && let Ok(mut slot) = worker_control.error.lock()
                {
                    *slot = Some(error.to_string());
                }
                worker_control.ended.store(true, Ordering::Release);
                worker_control.paused.store(true, Ordering::Release);
                worker_control.ready.store(true, Ordering::Release);
            }
        });
        *job = Some(Job {
            cancel,
            control,
            thread,
        });
        Ok(())
    }

    pub fn pause(&self, paused: bool) {
        if let Ok(job) = self.job.lock()
            && let Some(job) = job.as_ref()
        {
            job.control.paused.store(paused, Ordering::Release);
        }
    }

    pub fn state(&self) -> PlayerState {
        let Ok(job) = self.job.lock() else {
            return PlayerState::default();
        };
        let Some(job) = job.as_ref() else {
            return PlayerState::default();
        };
        let c = &job.control;
        PlayerState {
            ready: c.ready.load(Ordering::Acquire),
            paused: c.paused.load(Ordering::Acquire),
            ended: c.ended.load(Ordering::Acquire),
            position_us: c.position_us.load(Ordering::Acquire),
            duration_us: c.duration_us.load(Ordering::Acquire),
            audio_active: c.audio_active.load(Ordering::Acquire),
            muted: c.muted.load(Ordering::Acquire),
            error: c.error.lock().ok().and_then(|v| v.clone()),
        }
    }

    fn clear(&self) {
        if let Ok(mut slot) = self.frames.lock() {
            slot.frame = None;
            slot.revision = slot.revision.wrapping_add(1);
        }
    }

    pub fn cancel(&self) {
        if let Ok(job) = self.job.lock()
            && let Some(job) = job.as_ref()
        {
            job.cancel.cancel();
        }
        self.clear();
    }

    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        self.cancel();
        if let Ok(mut job) = self.job.lock()
            && let Some(job) = job.take()
        {
            let _ = job.thread.join();
        }
    }
}

impl Drop for PreviewService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct Presenter {
    color_pass: Option<crate::preview_gpu::ColorPass>,
    color_revision: Option<u64>,
    frames: Arc<Mutex<FrameSlot>>,
    device: Option<DeviceHandle>,
    texture: Option<wgpu_blitz::Texture>,
    handle: Option<TextureHandle>,
    revision: Option<u64>,
}

impl Presenter {
    pub fn new(frames: Arc<Mutex<FrameSlot>>) -> Self {
        Self {
            color_pass: None,
            color_revision: None,
            frames,
            device: None,
            texture: None,
            handle: None,
            revision: None,
        }
    }
}

impl CustomPaintSource for Presenter {
    fn resume(&mut self, device: &DeviceHandle) {
        self.suspend();
        self.device = Some(device.clone());
    }
    fn suspend(&mut self) {
        // Renderer suspension drops its texture registry. Never carry a handle
        // into the resumed renderer/device generation.
        self.handle = None;
        self.texture = None;
        self.device = None;
        self.revision = None;
        self.color_pass = None;
        self.color_revision = None;
    }
    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        _width: u32,
        _height: u32,
        _scale: f64,
    ) -> Option<TextureHandle> {
        let slot = self.frames.lock().ok()?;
        let frame = slot.frame.as_ref()?;
        let device = self.device.as_ref()?;
        if self.texture.is_none() {
            self.color_pass = Some(crate::preview_gpu::ColorPass::new(&device.device));
            let texture = device
                .device
                .create_texture(&wgpu_blitz::TextureDescriptor {
                    label: Some("independent SDR frame preview"),
                    size: wgpu_blitz::Extent3d {
                        width: preview::WIDTH,
                        height: preview::HEIGHT,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu_blitz::TextureDimension::D2,
                    format: wgpu_blitz::TextureFormat::Rgba8Unorm,
                    usage: wgpu_blitz::TextureUsages::TEXTURE_BINDING
                    | wgpu_blitz::TextureUsages::RENDER_ATTACHMENT
                    // Vello copies registered images into its GPU atlas.
                    | wgpu_blitz::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
            self.handle = Some(ctx.register_texture(texture.clone()));
            self.texture = Some(texture);
        }
        if self.revision != Some(slot.revision) {
            device.queue.write_texture(
                wgpu_blitz::TexelCopyTextureInfo {
                    texture: &self.color_pass.as_ref()?.source,
                    mip_level: 0,
                    origin: wgpu_blitz::Origin3d::ZERO,
                    aspect: wgpu_blitz::TextureAspect::All,
                },
                &frame.rgba,
                wgpu_blitz::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(preview::WIDTH * 4),
                    rows_per_image: Some(preview::HEIGHT),
                },
                wgpu_blitz::Extent3d {
                    width: preview::WIDTH,
                    height: preview::HEIGHT,
                    depth_or_array_layers: 1,
                },
            );
        }
        if self.revision != Some(slot.revision) || self.color_revision != Some(slot.color_revision)
        {
            self.color_pass.as_ref()?.render(
                &device.device,
                &device.queue,
                self.texture.as_ref()?,
                slot.color,
            );
            self.revision = Some(slot.revision);
            self.color_revision = Some(slot.color_revision);
        }
        self.handle.clone()
    }
}

pub fn seek_from_fraction(fraction: f64, duration_us: u64) -> u64 {
    if !fraction.is_finite() {
        return 0;
    }
    ((fraction.clamp(0.0, 1.0) * duration_us as f64) as u64).min(duration_us.saturating_sub(1))
}

pub fn format_time(us: u64) -> String {
    let seconds = us / 1_000_000;
    if seconds >= 3_600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3_600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeline_clamps_and_formats() {
        assert_eq!(seek_from_fraction(0.5, 2_000_000), 1_000_000);
        assert_eq!(seek_from_fraction(-1.0, 2_000_000), 0);
        assert_eq!(seek_from_fraction(2.0, 2_000_000), 1_999_999);
        assert_eq!(seek_from_fraction(f64::NAN, 2_000_000), 0);
        assert_eq!(seek_from_fraction(1.0, 0), 0);
        assert_eq!(format_time(3_661_000_000), "1:01:01");
        assert_eq!(format_time(59_999_999), "0:59");
    }

    #[test]
    fn preview_service_retains_one_frame_clears_and_drains_shutdown() {
        let service = PreviewService::default();
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4");
        service.start(fixture.clone(), 0, false, true).unwrap();
        let started = std::time::Instant::now();
        loop {
            let state = service.state();
            assert!(state.error.is_none(), "{:?}", state.error);
            if state.ready {
                break;
            }
            assert!(started.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            service
                .frames
                .lock()
                .unwrap()
                .frame
                .as_ref()
                .unwrap()
                .rgba
                .len(),
            preview::FRAME_BYTES
        );
        let (frame_revision, frame_pointer) = {
            let slot = service.frames.lock().unwrap();
            (slot.revision, slot.frame.as_ref().unwrap().rgba.as_ptr())
        };
        for value in 0..60 {
            service.set_color(media_core::ColorAdjustments::new(value, 150, 0).unwrap());
            let slot = service.frames.lock().unwrap();
            assert_eq!(slot.revision, frame_revision);
            assert_eq!(slot.frame.as_ref().unwrap().rgba.as_ptr(), frame_pointer);
        }
        service.set_color(media_core::ColorAdjustments::default());
        service.cancel();
        assert!(service.frames.lock().unwrap().frame.is_none());
        // Rapid replacements are serialized and cannot publish stale frames.
        service
            .start(fixture.clone(), 500_000, false, true)
            .unwrap();
        service
            .start(fixture.clone(), 1_000_000, false, true)
            .unwrap();
        service.shutdown();
        assert!(service.frames.lock().unwrap().frame.is_none());
        assert!(service.job.lock().unwrap().is_none());
        assert!(service.start(fixture, 0, false, true).is_err());
    }
}
