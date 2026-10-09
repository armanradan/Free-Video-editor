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
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub const COLOR_INTERVAL: Duration = Duration::from_millis(34);

/// One attempt per settled selection, so Close/output preview is not undone by
/// the timer. UI typing and busy jobs defer opening; no pixels are retained here.
pub struct AutoPreviewSelection {
    observed: String,
    attempted: bool,
    changed: Instant,
}
impl AutoPreviewSelection {
    pub fn new(now: Instant) -> Self {
        Self {
            observed: String::new(),
            attempted: false,
            changed: now,
        }
    }
    pub fn poll(&mut self, input: &str, busy: bool, now: Instant) -> Option<String> {
        let selected = input.trim();
        if selected != self.observed {
            self.observed = selected.to_string();
            self.attempted = false;
            self.changed = now;
        }
        if busy
            || self.attempted
            || selected.is_empty()
            || now.saturating_duration_since(self.changed) < Duration::from_millis(600)
        {
            return None;
        }
        self.attempted = true;
        Some(self.observed.clone())
    }
}

/// Latest-value policy only. Pixels and submitted GPU uniforms remain elsewhere.
#[derive(Default)]
struct ColorUpdates {
    requested: media_core::ColorAdjustments,
    last_published: Option<Instant>,
}
impl ColorUpdates {
    fn publish(
        &mut self,
        current: media_core::ColorAdjustments,
        now: Instant,
    ) -> Option<media_core::ColorAdjustments> {
        if current == self.requested
            || self
                .last_published
                .is_some_and(|last| now.saturating_duration_since(last) < COLOR_INTERVAL)
        {
            return None;
        }
        self.last_published = Some(now);
        Some(self.requested)
    }
}

#[derive(Default)]
pub struct FrameSlot {
    frame: Option<PreviewFrame>,
    revision: u64,
    color: media_core::ColorAdjustments,
    color_revision: u64,
    color_updates: ColorUpdates,
    history_policy: Option<(media_core::Size, media_core::equalization::Equalization)>,
    history_request: Option<crate::preview_gpu::worker::Request>,
    history_error: Option<String>,
    history_playing: bool,
    completed_revision: u64,
    published_completion: u64,
    history_controls: Vec<crate::preview_gpu::worker::Control>,
}

struct Job {
    path: PathBuf,
    cancel: CancellationToken,
    control: Arc<PlaybackControl>,
    mailbox: Arc<(Mutex<Requests>, Condvar)>,
    thread: JoinHandle<()>,
}

struct Request {
    path: PathBuf,
    position_us: u64,
    cancel: CancellationToken,
    control: Arc<PlaybackControl>,
}
#[derive(Default)]
struct Requests {
    latest: Option<Request>,
    stopping: bool,
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
    #[cfg(test)]
    publication_gate: Arc<Mutex<()>>,
}

impl PreviewService {
    pub fn presentation_revision(&self) -> (u64, u64) {
        let slot = self.frames.lock().unwrap();
        (
            slot.revision,
            slot.color_revision.wrapping_add(slot.completed_revision),
        )
    }

    pub fn set_color(&self, color: media_core::ColorAdjustments) {
        if let Ok(mut slot) = self.frames.lock() {
            slot.color_updates.requested = color;
        }
    }

    /// Exact finite-preroll preview policy. None restores the original player.
    pub fn set_history_preview(
        &self,
        policy: Option<(media_core::Size, media_core::equalization::Equalization)>,
    ) -> Result<(), String> {
        let job = self.job.lock().map_err(|_| "preview job lock poisoned")?;
        let mut slot = self
            .frames
            .lock()
            .map_err(|_| "preview frame lock poisoned")?;
        if slot.history_policy == policy {
            if policy.is_none() {
                slot.history_error = None;
            }
            return Ok(());
        }
        if let Some((selected, settings)) = policy {
            let job = job.as_ref().ok_or("open a paused source preview first")?;
            if selected.width == 0 || selected.height == 0 {
                return Err("invalid selected preview geometry".into());
            }
            slot.history_request = Some(crate::preview_gpu::worker::Request {
                path: job.path.clone(),
                position_us: job.control.position_us.load(Ordering::Acquire),
                selected,
                settings,
                color: slot.color,
            });
        } else {
            slot.history_request = None;
            for control in &slot.history_controls {
                control.invalidate();
            }
        }
        slot.history_policy = policy;
        slot.history_error = None;
        slot.color_revision = slot.color_revision.wrapping_add(1);
        Ok(())
    }

    pub fn history_unavailable(&self, reason: &str) {
        if let Ok(mut slot) = self.frames.lock() {
            slot.history_error = Some(reason.to_owned());
        }
    }

    /// Called by one UI timer, not once per slider event. True requests a redraw.
    pub fn publish_color(&self) -> bool {
        let Ok(mut slot) = self.frames.lock() else {
            return false;
        };
        let current = slot.color;
        let completed = slot.completed_revision != slot.published_completion;
        slot.published_completion = slot.completed_revision;
        if let Some(color) = slot.color_updates.publish(current, Instant::now()) {
            slot.color = color;
            slot.color_revision = slot.color_revision.wrapping_add(1);
            true
        } else {
            completed
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
        let duration_us = job
            .as_ref()
            .filter(|old| old.path == path && !old.cancel.is_cancelled())
            .map(|old| old.control.duration_us.load(Ordering::Acquire))
            .unwrap_or(0);
        if let Some(old) = job.as_ref() {
            old.cancel.cancel();
        }
        // A seek keeps the current image until the requested frame arrives.
        // Only a different/new source hides pixels and resets timeline metadata.
        if duration_us == 0 {
            self.clear();
        }
        let cancel = CancellationToken::default();
        let control = Arc::new(PlaybackControl::default());
        control.paused.store(!playing, Ordering::Release);
        control.muted.store(muted, Ordering::Release);
        control.position_us.store(position_us, Ordering::Release);
        control.duration_us.store(duration_us, Ordering::Release);
        let request = Request {
            path: path.clone(),
            position_us,
            cancel: cancel.clone(),
            control: control.clone(),
        };
        {
            let mut slot = self
                .frames
                .lock()
                .map_err(|_| "preview frame lock poisoned")?;
            if let Some((selected, settings)) = slot.history_policy {
                slot.history_request = Some(crate::preview_gpu::worker::Request {
                    path: path.clone(),
                    position_us,
                    selected,
                    settings,
                    color: slot.color,
                });
            }
            slot.history_error = None;
            slot.history_playing = playing;
            for control in &slot.history_controls {
                control.invalidate();
            }
        }
        if let Some(job) = job.as_mut() {
            let mut requests = job
                .mailbox
                .0
                .lock()
                .map_err(|_| "preview mailbox poisoned")?;
            requests.latest = Some(request);
            job.path = path;
            job.cancel = cancel;
            job.control = control;
            job.mailbox.1.notify_one();
        } else {
            let mailbox = Arc::new((
                Mutex::new(Requests {
                    latest: Some(request),
                    stopping: false,
                }),
                Condvar::new(),
            ));
            let worker_mailbox = mailbox.clone();
            let frames = self.frames.clone();
            #[cfg(test)]
            let publication_gate = self.publication_gate.clone();
            let thread = std::thread::spawn(move || {
                loop {
                    let request = {
                        let (lock, wake) = &*worker_mailbox;
                        let mut requests = lock.lock().unwrap();
                        while !requests.stopping && requests.latest.is_none() {
                            requests = wake.wait(requests).unwrap();
                        }
                        if requests.stopping {
                            break;
                        }
                        requests.latest.take().unwrap()
                    };
                    let result = playback::play(
                        &request.path,
                        request.position_us,
                        &request.control,
                        &request.cancel,
                        |frame| {
                            #[cfg(test)]
                            let _publication = publication_gate.lock().unwrap();
                            let mut slot =
                                frames.lock().map_err(|_| "preview frame lock poisoned")?;
                            if request.cancel.is_cancelled() {
                                return Err("Preview cancelled".into());
                            }
                            slot.revision = slot.revision.wrapping_add(1);
                            slot.history_playing = !request.control.paused.load(Ordering::Acquire);
                            if let Some(history) = slot.history_request.as_mut() {
                                history.position_us = frame.requested_us;
                            }
                            slot.frame = Some(frame);
                            Ok(())
                        },
                    );
                    if !request.cancel.is_cancelled() {
                        if let Err(error) = result
                            && let Ok(mut slot) = request.control.error.lock()
                        {
                            *slot = Some(error.to_string());
                        }
                        request.control.ended.store(true, Ordering::Release);
                        request.control.paused.store(true, Ordering::Release);
                        request.control.ready.store(true, Ordering::Release);
                    }
                }
            });
            *job = Some(Job {
                path,
                cancel,
                control,
                mailbox,
                thread,
            });
        }
        Ok(())
    }

    pub fn pause(&self, paused: bool) {
        if let Ok(job) = self.job.lock()
            && let Some(job) = job.as_ref()
        {
            job.control.paused.store(paused, Ordering::Release);
            if let Ok(mut slot) = self.frames.lock() {
                slot.history_playing = !paused;
            }
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
            error: self
                .frames
                .lock()
                .ok()
                .and_then(|slot| slot.history_error.clone())
                .or_else(|| c.error.lock().ok().and_then(|v| v.clone())),
        }
    }

    fn clear(&self) {
        if let Ok(mut slot) = self.frames.lock() {
            slot.frame = None;
            slot.history_request = None;
            slot.history_error = None;
            for control in &slot.history_controls {
                control.invalidate();
            }
            slot.revision = slot.revision.wrapping_add(1);
        }
    }

    pub fn cancel(&self) {
        if let Ok(job) = self.job.lock()
            && let Some(job) = job.as_ref()
        {
            job.cancel.cancel();
            if let Ok(mut requests) = job.mailbox.0.lock() {
                requests.latest = None;
            }
        }
        self.clear();
    }

    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        self.cancel();
        if let Ok(mut job) = self.job.lock()
            && let Some(job) = job.take()
        {
            if let Ok(mut requests) = job.mailbox.0.lock() {
                requests.stopping = true;
                requests.latest = None;
            }
            job.mailbox.1.notify_one();
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
    history_worker: Option<crate::preview_gpu::worker::Worker>,
    generation: u64,
    history_source: Option<(PathBuf, media_core::Size)>,
    detached: bool,
    lost: Arc<AtomicBool>,
    loss_reporting: Arc<AtomicBool>,
}

impl Presenter {
    fn render_history(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        request: crate::preview_gpu::worker::Request,
        playing: bool,
    ) -> Option<TextureHandle> {
        let device = self.device.as_ref()?;
        if self.history_worker.is_none() {
            let frames = self.frames.clone();
            self.history_worker = Some(crate::preview_gpu::worker::Worker::new_checked(
                device.device.clone(),
                device.queue.clone(),
                self.generation,
                move || {
                    if let Ok(mut slot) = frames.lock() {
                        slot.completed_revision = slot.completed_revision.wrapping_add(1);
                    }
                },
                self.lost.clone(),
            ));
            if let Ok(mut slot) = self.frames.lock() {
                slot.history_controls.retain(|control| control.live());
                slot.history_controls
                    .push(self.history_worker.as_ref()?.control());
            }
            // Never display an old differently processed image as CLAHE.
            if let Some(handle) = self.handle.take() {
                ctx.unregister_texture(handle);
            }
            self.texture = None;
        }
        let worker = self.history_worker.as_ref()?;
        let source = (request.path.clone(), request.selected);
        if self.history_source.as_ref() != Some(&source) {
            if let Some(handle) = self.handle.take() {
                ctx.unregister_texture(handle);
            }
            self.texture = None;
        }
        self.history_source = Some(source);
        worker.request_sample(request, playing);
        if let Some(result) = worker.take_result() {
            match result {
                Ok(output) => {
                    let _uploads = output.uploads;
                    if self.texture.as_ref().is_none() {
                        if let Some(handle) = self.handle.take() {
                            ctx.unregister_texture(handle);
                        }
                        let texture =
                            device
                                .device
                                .create_texture(&wgpu_blitz::TextureDescriptor {
                                    label: Some("registered native history preview"),
                                    size: wgpu_blitz::Extent3d {
                                        width: 640,
                                        height: 360,
                                        depth_or_array_layers: 1,
                                    },
                                    mip_level_count: 1,
                                    sample_count: 1,
                                    dimension: wgpu_blitz::TextureDimension::D2,
                                    format: wgpu_blitz::TextureFormat::Rgba8Unorm,
                                    usage: wgpu_blitz::TextureUsages::COPY_DST
                                        | wgpu_blitz::TextureUsages::COPY_SRC
                                        | wgpu_blitz::TextureUsages::TEXTURE_BINDING
                                        | wgpu_blitz::TextureUsages::RENDER_ATTACHMENT,
                                    view_formats: &[],
                                });
                        self.handle = Some(ctx.register_texture(texture.clone()));
                        self.texture = Some(texture);
                    }
                    crate::preview_gpu::copy_history_display(
                        &device.device,
                        &device.queue,
                        &output.texture,
                        self.texture.as_ref()?,
                    );
                    if let Ok(mut slot) = self.frames.lock() {
                        slot.history_error = None;
                    }
                }
                Err(error) => {
                    if let Some(handle) = self.handle.take() {
                        ctx.unregister_texture(handle);
                    }
                    self.texture = None;
                    if let Ok(mut slot) = self.frames.lock() {
                        slot.history_error = Some(error);
                    }
                }
            }
        }
        self.handle.clone()
    }

    pub fn new(frames: Arc<Mutex<FrameSlot>>) -> Self {
        Self {
            color_pass: None,
            color_revision: None,
            frames,
            device: None,
            texture: None,
            handle: None,
            revision: None,
            history_worker: None,
            generation: 0,
            history_source: None,
            detached: false,
            lost: Arc::new(AtomicBool::new(false)),
            loss_reporting: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn detached(frames: Arc<Mutex<FrameSlot>>) -> Self {
        let mut presenter = Self::new(frames);
        presenter.detached = true;
        presenter
    }
}

impl CustomPaintSource for Presenter {
    fn resume(&mut self, device: &DeviceHandle) {
        self.suspend();
        self.device = Some(device.clone());
        self.generation = self.generation.wrapping_add(1);
        self.lost = Arc::new(AtomicBool::new(false));
        let lost = self.lost.clone();
        self.loss_reporting = Arc::new(AtomicBool::new(true));
        let reporting = self.loss_reporting.clone();
        let detached = self.detached;
        // This app installs exactly one presenter on each window's independent
        // Vello renderer/context. The callback belongs to that context, not to
        // individual cache workers; replacing a worker never overwrites it.
        device
            .device
            .set_device_lost_callback(move |reason, message| {
                lost.store(true, Ordering::Release);
                #[cfg(not(test))]
                if reporting.load(Ordering::Acquire) {
                    crate::window::report_renderer_loss(detached);
                }
                eprintln!("Renderer device lost: {reason:?}: {message}");
                #[cfg(test)]
                let _ = (detached, &reporting);
            });
    }
    fn suspend(&mut self) {
        self.loss_reporting.store(false, Ordering::Release);
        // Renderer suspension drops its texture registry. Never carry a handle
        // into the resumed renderer/device generation.
        self.handle = None;
        self.texture = None;
        self.device = None;
        self.revision = None;
        self.color_pass = None;
        self.color_revision = None;
        self.history_worker = None;
        self.history_source = None;
    }
    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        _width: u32,
        _height: u32,
        _scale: f64,
    ) -> Option<TextureHandle> {
        if self.lost.load(Ordering::Acquire) {
            return None;
        }
        let slot = self.frames.lock().ok()?;
        if slot.history_error.is_some() && slot.history_request.is_none() {
            return None;
        }
        if let Some(mut request) = slot.history_request.clone() {
            request.color = slot.color;
            let playing = slot.history_playing;
            drop(slot);
            return self.render_history(ctx, request, playing);
        }
        if self.history_worker.is_some() {
            drop(slot);
            self.history_worker = None;
            if let Some(handle) = self.handle.take() {
                ctx.unregister_texture(handle);
            }
            self.texture = None;
            self.revision = None;
        } else {
            drop(slot);
        }
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
    #[ignore = "real renderer ABI device, presenter suspension and fresh worker"]
    fn presenter_suspend_resume_retires_history_worker() {
        let instance = wgpu_blitz::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        println!("Presenter lifecycle adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut device = DeviceHandle {
            instance,
            adapter,
            device,
            queue,
        };
        let frames = Arc::new(Mutex::new(FrameSlot::default()));
        let mut presenter = Presenter::new(frames);
        let request = crate::preview_gpu::worker::Request {
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/m35-vfr-offset.mp4"),
            position_us: 500_000,
            selected: media_core::Size::new(160, 90).unwrap(),
            settings: media_core::equalization::Equalization::new(true, 50).unwrap(),
            color: media_core::ColorAdjustments::default(),
        };
        let mut reference = None;
        for generation in 1..=2 {
            presenter.resume(&device);
            assert_eq!(presenter.generation, generation);
            assert!(presenter.texture.is_none() && presenter.handle.is_none());
            // Same worker construction as render_history; CustomPaintCtx's
            // registry is private, so registration is covered by live UI checks.
            presenter.history_worker = Some(crate::preview_gpu::worker::Worker::new(
                device.device.clone(),
                device.queue.clone(),
                generation,
                || {},
            ));
            let worker = presenter.history_worker.as_ref().unwrap();
            let control = worker.control();
            worker.request(request.clone());
            let deadline = Instant::now() + Duration::from_secs(15);
            let output = loop {
                if let Some(result) = worker.take_result() {
                    break result.unwrap();
                }
                assert!(Instant::now() < deadline, "presenter worker timed out");
                std::thread::sleep(Duration::from_millis(2));
            };
            assert_eq!(output.uploads, 3); // Fresh history, not a previous generation's cache.
            let pixels = crate::preview_gpu::diagnostic_readback(
                &device.device,
                &device.queue,
                &output.texture,
            );
            if let Some(expected) = &reference {
                assert_eq!(&pixels, expected);
            } else {
                reference = Some(pixels);
            }
            worker.request(crate::preview_gpu::worker::Request {
                position_us: 1_900_000,
                ..request.clone()
            });
            presenter.suspend(); // Cancels and joins even with pending preparation.
            assert!(!control.live());
            assert!(presenter.device.is_none() && presenter.history_worker.is_none());
            assert!(presenter.texture.is_none() && presenter.handle.is_none());
            if generation == 1 {
                // Destroy only this test-owned, already retired renderer device,
                // not the live application's UI device or an active queue.
                let lost = Arc::new(AtomicBool::new(false));
                let observed = lost.clone();
                device
                    .device
                    .set_device_lost_callback(move |reason, message| {
                        println!("Retired renderer device loss: {reason:?}: {message}");
                        observed.store(true, Ordering::Release);
                    });
                device.device.destroy();
                let deadline = Instant::now() + Duration::from_secs(2);
                while !lost.load(Ordering::Acquire) {
                    let _ = device.device.poll(wgpu_blitz::PollType::Poll);
                    assert!(Instant::now() < deadline, "missing device-loss callback");
                    std::thread::sleep(Duration::from_millis(2));
                }
                let (replacement, queue) =
                    pollster::block_on(device.adapter.request_device(&Default::default())).unwrap();
                device.device = replacement;
                device.queue = queue;
            }
        }
    }
    #[test]
    fn history_policy_tracks_seek_close_and_playback_samples() {
        let service = PreviewService::default();
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4");
        let policy = (
            media_core::Size::new(160, 90).unwrap(),
            media_core::equalization::Equalization::new(true, 50).unwrap(),
        );
        assert!(service.set_history_preview(Some(policy)).is_err());
        service.start(fixture.clone(), 0, false, true).unwrap();
        service.set_history_preview(Some(policy)).unwrap();
        let revision = service.presentation_revision();
        service.set_history_preview(Some(policy)).unwrap();
        assert_eq!(revision, service.presentation_revision());
        service.start(fixture.clone(), 0, true, true).unwrap();
        service.pause(false);
        assert!(!service.state().paused);
        assert!(service.state().error.is_none());
        service
            .start(fixture.clone(), 500_000, false, true)
            .unwrap();
        {
            let slot = service.frames.lock().unwrap();
            let request = slot.history_request.as_ref().unwrap();
            assert_eq!(request.path, fixture);
            assert_eq!(request.position_us, 500_000);
            assert!(slot.history_error.is_none());
        }
        service.cancel();
        assert!(service.frames.lock().unwrap().history_request.is_none());
        service.set_history_preview(None).unwrap();
        service.start(fixture, 0, true, true).unwrap();
        service.shutdown();
    }
    #[test]
    fn secondary_presenter_shares_frame_without_restarting_or_canceling_playback() {
        let service = PreviewService::default();
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m2-h264-aac.mp4");
        service.start(fixture, 1_000_000, false, true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !service.state().ready {
            assert!(service.state().error.is_none());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let control = service
            .job
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .control
            .clone();
        let initial = service.state();
        let revision = service.presentation_revision();
        let inline = Presenter::new(service.frames.clone());
        for _ in 0..5 {
            let extra = Presenter::new(service.frames.clone());
            assert!(Arc::ptr_eq(&inline.frames, &extra.frames));
            drop(extra);
            assert!(Arc::ptr_eq(
                &control,
                &service.job.lock().unwrap().as_ref().unwrap().control
            ));
            assert!(service.state() == initial);
            assert_eq!(service.presentation_revision(), revision);
        }
        // A paused color update is visible through both presenters without
        // replacing decoded pixels or touching the playback control.
        service.set_color(media_core::ColorAdjustments::new(10, 150, 50).unwrap());
        assert!(service.publish_color());
        assert_eq!(service.presentation_revision().0, revision.0);
        assert_ne!(service.presentation_revision().1, revision.1);
        let extra = Presenter::new(service.frames.clone());
        let extra_color = extra.frames.lock().unwrap().color;
        let inline_color = inline.frames.lock().unwrap().color;
        assert_eq!(extra_color, inline_color);
        drop(extra);
        drop(inline);
        assert!(service.state() == initial);
        service.shutdown();
        assert!(service.frames.lock().unwrap().frame.is_none());
        assert!(service.job.lock().unwrap().is_none());
    }
    #[test]
    fn automatic_preview_waits_for_latest_stable_selection_and_does_not_reopen() {
        let now = Instant::now();
        let mut selection = AutoPreviewSelection::new(now);
        assert_eq!(selection.poll("part", false, now), None);
        assert_eq!(
            selection.poll("  fixture.mp4  ", false, now + Duration::from_millis(100)),
            None
        );
        assert_eq!(
            selection.poll("fixture.mp4", false, now + Duration::from_millis(699)),
            None
        );
        assert_eq!(
            selection.poll("fixture.mp4", true, now + Duration::from_millis(900)),
            None
        );
        assert_eq!(
            selection.poll("fixture.mp4", false, now + Duration::from_millis(901)),
            Some("fixture.mp4".into())
        );
        // No repeated decoder opens, including after a user closes this preview.
        assert_eq!(
            selection.poll("fixture.mp4", false, now + Duration::from_secs(5)),
            None
        );
        assert_eq!(
            selection.poll("other.mp4", false, now + Duration::from_secs(6)),
            None
        );
        assert_eq!(
            selection.poll("", false, now + Duration::from_secs(7)),
            None
        );
        assert_eq!(
            selection.poll("", false, now + Duration::from_secs(8)),
            None
        );
        assert_eq!(
            selection.poll("fixture.mp4", false, now + Duration::from_secs(9)),
            None
        );
        assert_eq!(
            selection.poll("fixture.mp4", false, now + Duration::from_secs(10)),
            Some("fixture.mp4".into())
        );
    }
    #[test]
    fn color_updates_coalesce_and_publish_last_value_without_new_input() {
        let start = Instant::now();
        let neutral = media_core::ColorAdjustments::default();
        let mut updates = ColorUpdates::default();
        let first = media_core::ColorAdjustments::new(1, 100, 100).unwrap();
        updates.requested = first;
        assert_eq!(updates.publish(neutral, start), Some(first));
        for brightness in 2..=60 {
            updates.requested = media_core::ColorAdjustments::new(brightness, 150, 0).unwrap();
            assert_eq!(
                updates.publish(first, start + Duration::from_millis(10)),
                None
            );
        }
        let latest = updates.requested;
        assert_eq!(updates.publish(first, start + COLOR_INTERVAL), Some(latest));
        assert_eq!(updates.publish(latest, start + COLOR_INTERVAL * 2), None);
        // Reset is also latest-value, and must not require another slider event.
        updates.requested = neutral;
        assert_eq!(
            updates.publish(latest, start + COLOR_INTERVAL * 2),
            Some(neutral)
        );
    }
    #[test]
    fn same_source_seek_keeps_timeline_and_image_until_target_is_ready() {
        let service = PreviewService::default();
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m2-h264-aac.mp4");
        service
            .start(fixture.clone(), 500_000, false, true)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !service.state().ready {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let duration = service.state().duration_us;
        assert!(duration > 1_000_000);
        // Block publication, making the loading-state assertion deterministic.
        {
            let _publication = service.publication_gate.lock().unwrap();
            let (revision, pointer) = {
                let slot = service.frames.lock().unwrap();
                (slot.revision, slot.frame.as_ref().unwrap().rgba.as_ptr())
            };
            service
                .start(fixture.clone(), 1_000_000, false, true)
                .unwrap();
            let pending = service.state();
            let slot = service.frames.lock().unwrap();
            assert_eq!(pending.duration_us, duration);
            assert_eq!(pending.position_us, 1_000_000);
            assert_eq!(slot.revision, revision);
            assert_eq!(slot.frame.as_ref().unwrap().rgba.as_ptr(), pointer);
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while !service.state().ready {
            let state = service.state();
            assert_eq!(state.duration_us, duration);
            assert_eq!(state.position_us, 1_000_000);
            assert!(state.error.is_none(), "{:?}", state.error);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let expected =
            preview::decode_frame(&fixture, 1_000_000, &CancellationToken::default()).unwrap();
        assert_eq!(
            service.frames.lock().unwrap().frame.as_ref().unwrap().rgba,
            expected.rgba
        );
        service.cancel();
        // Reopening even the same path after Close must not reuse its old image.
        service.start(fixture, 0, false, true).unwrap();
        assert_eq!(service.state().position_us, 0);
        service.shutdown();
    }

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

    #[test]
    fn rapid_color_seek_and_source_replacement_keeps_one_worker_and_latest_frame() {
        let service = PreviewService::default();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        service
            .start(root.join("m2-h264-aac.mp4"), 0, false, true)
            .unwrap();
        let worker = service
            .job
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .thread
            .thread()
            .id();
        for index in 0..40 {
            let file = if index % 2 == 0 {
                "m35-vfr-offset.mp4"
            } else {
                "m2-h264-aac.mp4"
            };
            service.set_color(media_core::ColorAdjustments::new(index, 150, 50).unwrap());
            service
                .start(root.join(file), (index as u64 % 3) * 200_000, false, true)
                .unwrap();
            assert_eq!(
                service
                    .job
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .thread
                    .thread()
                    .id(),
                worker
            );
        }
        service
            .start(root.join("m2-h264-aac.mp4"), 500_000, false, true)
            .unwrap();
        let start = Instant::now();
        while !service.state().ready {
            assert!(
                start.elapsed() < Duration::from_secs(15),
                "latest request stalled"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(service.state().error.is_none());
        let expected = preview::decode_frame(
            &root.join("m2-h264-aac.mp4"),
            500_000,
            &CancellationToken::default(),
        )
        .unwrap();
        {
            let slot = service.frames.lock().unwrap();
            let frame = slot.frame.as_ref().unwrap();
            assert_eq!(frame.requested_us, 500_000);
            assert_eq!(frame.rgba, expected.rgba, "stale source/position published");
        }
        service.cancel();
        std::thread::sleep(Duration::from_millis(100));
        assert!(service.frames.lock().unwrap().frame.is_none());
        service.shutdown();
        assert!(service.job.lock().unwrap().is_none());
        println!(
            "40 adjusted seek/source replacements: one worker, latest pixels, cancellation/shutdown PASS in {:?}",
            start.elapsed()
        );
    }
}
