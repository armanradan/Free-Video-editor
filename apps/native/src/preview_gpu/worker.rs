//! One latest-command worker per renderer; no codec/GPU work in paint callbacks.
use super::HistoryPass;
use media_core::{ColorAdjustments, Size, equalization::Equalization};
use media_native::CancellationToken;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};
use wgpu_blitz as w;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Request {
    pub path: PathBuf,
    pub position_us: u64,
    pub selected: Size,
    pub settings: Equalization,
    pub color: ColorAdjustments,
}
impl Request {
    fn same_source(&self, other: &Self) -> bool {
        self.path == other.path
            && self.position_us == other.position_us
            && self.selected == other.selected
    }
}

pub(crate) struct Completed {
    pub texture: w::Texture,
    pub uploads: u64,
}
#[derive(Default)]
struct State {
    latest: Option<(u64, Request)>,
    wanted: Option<Request>,
    revision: u64,
    result: Option<Result<Completed, String>>,
    active: Option<CancellationToken>,
    stopping: bool,
    clearing: bool,
}

pub(crate) struct Worker {
    state: Arc<(Mutex<State>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}
pub(crate) struct Control(std::sync::Weak<(Mutex<State>, Condvar)>);
impl Control {
    pub fn live(&self) -> bool {
        self.0.strong_count() > 0
    }
    pub fn invalidate(&self) {
        if let Some(shared) = self.0.upgrade() {
            let mut state = shared.0.lock().unwrap();
            state.revision = state.revision.wrapping_add(1);
            state.wanted = None;
            state.latest = None;
            state.result = None;
            state.clearing = true;
            if let Some(cancel) = &state.active {
                cancel.cancel();
            }
            shared.1.notify_one();
        }
    }
}
impl Worker {
    pub fn control(&self) -> Control {
        Control(Arc::downgrade(&self.state))
    }
    #[cfg(test)]
    pub fn new(
        device: w::Device,
        queue: w::Queue,
        generation: u64,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        Self::new_checked(
            device,
            queue,
            generation,
            wake,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
    }

    pub fn new_checked(
        device: w::Device,
        queue: w::Queue,
        generation: u64,
        wake: impl Fn() + Send + 'static,
        lost: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let state = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let shared = state.clone();
        let thread = std::thread::spawn(move || {
            let mut cache: Option<(Request, (u64, Option<std::time::SystemTime>), HistoryPass)> =
                None;
            loop {
                let (revision, request, cancel) = {
                    let (lock, changed) = &*shared;
                    let mut state = lock.lock().unwrap();
                    while !state.stopping && state.latest.is_none() && !state.clearing {
                        state = changed.wait(state).unwrap();
                    }
                    if state.stopping {
                        break;
                    }
                    if state.clearing {
                        cache = None;
                        state.clearing = false;
                        if state.latest.is_none() {
                            continue;
                        }
                    }
                    let (revision, request) = state.latest.take().unwrap();
                    let cancel = CancellationToken::default();
                    state.active = Some(cancel.clone());
                    (revision, request, cancel)
                };
                let result = (|| -> Result<Completed, String> {
                    if lost.load(std::sync::atomic::Ordering::Acquire) {
                        return Err("renderer device lost; restart renderer".into());
                    }
                    let metadata = std::fs::metadata(&request.path).map_err(|e| e.to_string())?;
                    let identity = (metadata.len(), metadata.modified().ok());
                    if cache.as_ref().is_none_or(|(key, old_identity, _)| {
                        !key.same_source(&request) || *old_identity != identity
                    }) {
                        cache = None;
                        let (pass, _) = HistoryPass::prepare_checked(
                            &device,
                            &queue,
                            &request.path,
                            request.position_us,
                            request.selected,
                            generation,
                            (&cancel, lost.clone()),
                        )
                        .map_err(|e| e.to_string())?;
                        cache = Some((request.clone(), identity, pass));
                        #[cfg(not(test))]
                        {
                            static INJECTED: std::sync::atomic::AtomicBool =
                                std::sync::atomic::AtomicBool::new(false);
                            if std::env::args().any(|arg| arg == "--clahe-device-loss-check")
                                && !INJECTED.swap(true, std::sync::atomic::Ordering::AcqRel)
                            {
                                device.destroy();
                                let _ = device.poll(w::PollType::Poll);
                                return Err(
                                    "injected renderer device loss; F5 restarts the renderer"
                                        .into(),
                                );
                            }
                        }
                    }
                    if cancel.is_cancelled() {
                        return Err("native history preview cancelled".into());
                    }
                    if lost.load(std::sync::atomic::Ordering::Acquire) {
                        return Err("renderer device lost; restart renderer".into());
                    }
                    let pass = &mut cache.as_mut().ok_or("missing history cache")?.2;
                    // Display enlargement is independent of selected-output
                    // statistics; fit into the player's existing 640x360 box.
                    let display = 640.0 / request.selected.width as f64;
                    let display = display.min(360.0 / request.selected.height as f64);
                    let width = ((request.selected.width as f64 * display).floor() as u32).max(1);
                    let height = ((request.selected.height as f64 * display).floor() as u32).max(1);
                    let texture = device.create_texture(&w::TextureDescriptor {
                        label: Some("completed native history preview handoff"),
                        size: w::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: w::TextureDimension::D2,
                        format: w::TextureFormat::Rgba8Unorm,
                        usage: w::TextureUsages::RENDER_ATTACHMENT | w::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    });
                    pass.render(&texture, request.settings, request.color, generation)
                        .map_err(|e| e.to_string())?;
                    let after = std::fs::metadata(&request.path).map_err(|e| e.to_string())?;
                    if identity != (after.len(), after.modified().ok()) {
                        return Err("preview source changed while preparing".into());
                    }
                    Ok(Completed {
                        texture,
                        uploads: pass.uploaded_frames(),
                    })
                })();
                let publish = {
                    let mut state = shared.0.lock().unwrap();
                    state.active = None;
                    if result.is_err() || cancel.is_cancelled() {
                        cache = None;
                    }
                    if !state.stopping && state.revision == revision && !cancel.is_cancelled() {
                        state.result = Some(result);
                        true
                    } else {
                        false
                    }
                };
                if publish {
                    wake();
                }
            }
        });
        Self {
            state,
            thread: Some(thread),
        }
    }

    #[cfg(test)]
    pub fn request(&self, request: Request) {
        self.request_sample(request, false);
    }

    pub fn request_sample(&self, request: Request, playing: bool) {
        let mut state = self.state.0.lock().unwrap();
        if state.wanted.as_ref() == Some(&request) {
            return;
        }
        // Playback may replace one pending position without starving an active
        // sample. Explicit seek/source changes invalidate via Control first.
        let next_sample = playing
            && state.wanted.as_ref().is_some_and(|old| {
                old.path == request.path
                    && old.selected == request.selected
                    && old.settings == request.settings
                    && old.color == request.color
            });
        if !next_sample
            && state
                .wanted
                .as_ref()
                .is_none_or(|old| !old.same_source(&request))
            && let Some(cancel) = &state.active
        {
            cancel.cancel();
        }
        if !next_sample {
            state.revision = state.revision.wrapping_add(1);
            state.result = None;
        }
        state.wanted = Some(request.clone());
        state.latest = Some((state.revision, request));
        self.state.1.notify_one();
    }

    pub fn take_result(&self) -> Option<Result<Completed, String>> {
        self.state.0.lock().unwrap().result.take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        {
            let mut state = self.state.0.lock().unwrap();
            state.stopping = true;
            state.latest = None;
            state.result = None;
            if let Some(cancel) = &state.active {
                cancel.cancel();
            }
        }
        self.state.1.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait(worker: &Worker) -> Completed {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(result) = worker.take_result() {
                return result.unwrap();
            }
            assert!(
                std::time::Instant::now() < deadline,
                "history worker timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[test]
    #[ignore = "actual renderer-device worker, decode/GPU cancellation and cache"]
    fn latest_request_cache_invalidation_and_shutdown() {
        let instance = w::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        println!("History worker adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m35-vfr-offset.mp4");
        let mut request = Request {
            path: fixture.clone(),
            position_us: 500_000,
            selected: Size::new(160, 90).unwrap(),
            settings: Equalization::new(true, 50).unwrap(),
            color: ColorAdjustments::new(10, 80, 60).unwrap(),
        };
        let wake_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let count = wake_count.clone();
        let worker = Worker::new(device.clone(), queue.clone(), 3, move || {
            count.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        });
        worker.request(request.clone());
        let first = wait(&worker);
        let pixels = super::super::history::tests::readback(&device, &queue, &first.texture);
        assert_eq!(first.uploads, 3);
        // Fast edits replace one pending command, never accumulate a queue.
        for strength in 1..=30 {
            request.settings = Equalization::new(true, strength).unwrap();
            worker.request(request.clone());
        }
        request.settings = Equalization::new(true, 50).unwrap();
        worker.request(request.clone());
        let final_output = wait(&worker);
        assert_eq!(final_output.uploads, first.uploads);
        assert_eq!(
            pixels,
            super::super::history::tests::readback(&device, &queue, &final_output.texture)
        );
        request.settings = Equalization::default();
        worker.request(request.clone());
        let before = wait(&worker);
        assert_eq!(before.uploads, 3);
        request.settings = Equalization::new(true, 50).unwrap();
        worker.request(request.clone());
        assert_eq!(
            pixels,
            super::super::history::tests::readback(&device, &queue, &wait(&worker).texture)
        );
        // Supersede actual decode/source commands, then close/invalidate before
        // completion: no abandoned ticket can publish a texture.
        request.position_us = 1_900_000;
        worker.request(request.clone());
        worker.control().invalidate();
        assert!(worker.take_result().is_none());
        request.position_us = 500_000;
        worker.request(request.clone());
        assert_eq!(
            pixels,
            super::super::history::tests::readback(&device, &queue, &wait(&worker).texture)
        );
        request.path = fixture.with_file_name("m35-hdr-tagged.mp4");
        worker.request(request.clone());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(result) = worker.take_result() {
                assert!(result.is_err());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        request.path = fixture;
        worker.request(request.clone());
        assert_eq!(
            pixels,
            super::super::history::tests::readback(&device, &queue, &wait(&worker).texture)
        );
        request.selected = Size::new(160, 80).unwrap();
        worker.request(request.clone());
        let output = wait(&worker);
        assert_eq!(
            (output.texture.width(), output.texture.height()),
            (640, 320)
        );
        let target =
            super::super::history::tests::make_target(&device, Size::new(640, 360).unwrap());
        super::super::copy_history_display(&device, &queue, &output.texture, &target);
        let displayed = super::super::history::tests::readback(&device, &queue, &target);
        assert!(
            displayed[..640 * 20 * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [0, 0, 0, 255])
        );
        assert!(
            displayed[640 * 340 * 4..]
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [0, 0, 0, 255])
        );
        assert_eq!(
            &displayed[640 * 20 * 4..640 * 340 * 4],
            super::super::history::tests::readback(&device, &queue, &output.texture)
        );
        // Continuous clock samples replace pending metadata without cancelling
        // every active decode. An explicit seek still invalidates all tickets.
        request.selected = Size::new(160, 90).unwrap();
        for position in 0..50 {
            request.position_us = 300_000 + position * 10_000;
            worker.request_sample(request.clone(), true);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(wait(&worker).uploads > 0);
        worker.control().invalidate();
        request.position_us = 500_000;
        worker.request(request.clone());
        assert_eq!(
            pixels,
            super::super::history::tests::readback(&device, &queue, &wait(&worker).texture)
        );
        request.position_us = 1_900_000;
        worker.request(request);
        drop(worker); // cancellation, worker drain/join, no orphaned thread.
        assert!(wake_count.load(std::sync::atomic::Ordering::Acquire) >= 6);
    }
}
