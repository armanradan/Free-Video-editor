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
    pub before: bool,
    pub ai: Option<media_native::ai_preview::PreviewPolicy>,
    pub path: PathBuf,
    pub position_us: u64,
    pub selected: Size,
    pub settings: Equalization,
    pub color: ColorAdjustments,
}
impl Request {
    fn same_source(&self, other: &Self) -> bool {
        self.ai == other.ai
            && self.path == other.path
            && self.position_us == other.position_us
            && self.selected == other.selected
    }
}

pub(crate) struct Completed {
    #[cfg(test)]
    pub request: Request,
    pub texture: w::Texture,
    pub uploads: u64,
    pub ai_transfers: Option<media_native::ai_preview::Transfers>,
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
    suspended: bool,
}

pub(crate) struct Worker {
    state: Arc<(Mutex<State>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}
#[derive(Clone)]
pub(crate) struct Control(std::sync::Weak<(Mutex<State>, Condvar)>);
impl Control {
    pub fn live(&self) -> bool {
        self.0.strong_count() > 0
    }
    pub fn invalidate(&self) {
        self.retire(false);
    }
    pub fn suspend(&self) {
        self.retire(true);
    }
    fn retire(&self, suspend: bool) {
        if let Some(shared) = self.0.upgrade() {
            let mut state = shared.0.lock().unwrap();
            state.suspended |= suspend;
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
    pub fn resume(&self) {
        if let Some(shared) = self.0.upgrade() {
            shared.0.lock().unwrap().suspended = false;
        }
    }
    /// Called on the conversion worker, never in a paint callback or UI event.
    pub fn wait_retired(
        &self,
        cancel: &CancellationToken,
        deadline: std::time::Instant,
    ) -> Result<(), String> {
        let Some(shared) = self.0.upgrade() else {
            return Ok(());
        };
        let mut state = shared
            .0
            .lock()
            .map_err(|_| "preview worker lock poisoned")?;
        while state.clearing || state.active.is_some() {
            if cancel.is_cancelled() {
                return Err("Preview handoff cancelled; conversion was not started".into());
            }
            if std::time::Instant::now() >= deadline {
                return Err(
                    "Timed out retiring preview GPU work; conversion was not started".into(),
                );
            }
            state = shared
                .1
                .wait_timeout(state, std::time::Duration::from_millis(10))
                .map_err(|_| "preview retirement lock poisoned")?
                .0;
        }
        Ok(())
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
            let mut ai_cache: Option<(
                Request,
                (u64, Option<std::time::SystemTime>),
                media_native::ai_preview::AiPreview,
            )> = None;
            loop {
                let (revision, request, cancel) = {
                    let (lock, changed) = &*shared;
                    let mut state = lock.lock().unwrap();
                    while !state.stopping && state.latest.is_none() && !state.clearing {
                        state = changed.wait(state).unwrap();
                    }
                    if state.stopping {
                        // A handoff may already hold an upgraded Control while
                        // the renderer closes. Acknowledge only after cached
                        // processing resources retire, including on this exit.
                        drop(state);
                        drop(cache.take());
                        drop(ai_cache.take());
                        let mut state = shared.0.lock().unwrap();
                        state.active = None;
                        state.clearing = false;
                        shared.1.notify_all();
                        break;
                    }
                    if state.clearing {
                        cache = None;
                        ai_cache = None;
                        state.clearing = false;
                        shared.1.notify_all();
                        if state.latest.is_none() {
                            continue;
                        }
                    }
                    let (revision, request) = state.latest.take().unwrap();
                    let cancel = CancellationToken::default().with_external_stop(lost.clone());
                    state.active = Some(cancel.clone());
                    (revision, request, cancel)
                };
                let result = (|| -> Result<Completed, String> {
                    if lost.load(std::sync::atomic::Ordering::Acquire) {
                        return Err("renderer device lost; restart renderer".into());
                    }
                    let metadata = std::fs::metadata(&request.path).map_err(|e| e.to_string())?;
                    let identity = (metadata.len(), metadata.modified().ok());
                    if let Some(policy) = &request.ai {
                        cache = None;
                        let cold = ai_cache.as_ref().is_none_or(|(key, old_identity, _)| {
                            !key.same_source(&request) || *old_identity != identity
                        });
                        let mut image = if request.before && cold {
                            ai_cache = None;
                            media_native::ai_preview::AiPreview::original_display(
                                &request.path,
                                request.position_us,
                                policy,
                                generation,
                                &cancel,
                            )
                        } else {
                            if cold {
                                ai_cache = None;
                                let (pass, _) = media_native::ai_preview::AiPreview::prepare(
                                    &request.path,
                                    request.position_us,
                                    policy,
                                    generation,
                                    &cancel,
                                )
                                .map_err(|e| e.to_string())?;
                                if pass.selected_size() != request.selected {
                                    return Err(
                                        "AI preview selection does not match model output geometry"
                                            .into(),
                                    );
                                }
                                ai_cache = Some((request.clone(), identity, pass));
                            }
                            let pass = &mut ai_cache.as_mut().ok_or("missing AI preview cache")?.2;
                            if request.before {
                                pass.render_before(generation, &cancel)
                            } else {
                                pass.render(request.settings, request.color, generation, &cancel)
                            }
                        }
                        .map_err(|e| e.to_string())?;
                        if lost.load(std::sync::atomic::Ordering::Acquire) || cancel.is_cancelled()
                        {
                            return Err("AI preview renderer lost or request cancelled".into());
                        }
                        let after = std::fs::metadata(&request.path).map_err(|e| e.to_string())?;
                        if identity != (after.len(), after.modified().ok()) {
                            return Err("AI preview source changed while preparing".into());
                        }
                        // Only this bounded CPU display raster crosses the ABI.
                        let texture = device.create_texture(&w::TextureDescriptor {
                            label: Some("native AI renderer-owned display image"),
                            size: w::Extent3d {
                                width: image.size.width,
                                height: image.size.height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: w::TextureDimension::D2,
                            format: w::TextureFormat::Rgba8Unorm,
                            usage: w::TextureUsages::COPY_DST | w::TextureUsages::COPY_SRC,
                            view_formats: &[],
                        });
                        queue.write_texture(
                            w::TexelCopyTextureInfo {
                                texture: &texture,
                                mip_level: 0,
                                origin: w::Origin3d::ZERO,
                                aspect: w::TextureAspect::All,
                            },
                            &image.rgba,
                            w::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(image.size.width * 4),
                                rows_per_image: Some(image.size.height),
                            },
                            texture.size(),
                        );
                        image.transfers.renderer_upload_bytes = image.rgba.len() as u64;
                        return Ok(Completed {
                            #[cfg(test)]
                            request: request.clone(),
                            texture,
                            uploads: 1,
                            ai_transfers: Some(image.transfers),
                        });
                    }
                    ai_cache = None;
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
                        #[cfg(test)]
                        request: request.clone(),
                        texture,
                        uploads: pass.uploaded_frames(),
                        ai_transfers: None,
                    })
                })()
                .map_err(|error| {
                    if lost.load(std::sync::atomic::Ordering::Acquire) {
                        format!("renderer device lost; preview retired: {error}")
                    } else {
                        error
                    }
                });
                let publish = {
                    let mut state = shared.0.lock().unwrap();
                    state.active = None;
                    shared.1.notify_all();
                    if result.is_err() || cancel.is_cancelled() {
                        cache = None;
                        ai_cache = None;
                    }
                    if !state.stopping
                        && state.revision == revision
                        && (!cancel.is_cancelled()
                            || lost.load(std::sync::atomic::Ordering::Acquire))
                    {
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
        if state.suspended || state.stopping {
            return;
        }
        if state.wanted.as_ref() == Some(&request) {
            return;
        }
        // Playback may replace one pending position without starving an active
        // sample. Explicit seek/source changes invalidate via Control first.
        let next_sample = playing
            && state.wanted.as_ref().is_some_and(|old| {
                old.ai == request.ai
                    && old.before == request.before
                    && old.path == request.path
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
    #[cfg(test)]
    pub fn preparing(&self) -> bool {
        self.state.0.lock().unwrap().active.is_some()
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
    #[test]
    #[ignore = "actual DirectML and wgpu 30 to renderer wgpu 26 staged AI preview"]
    fn actual_ai_renderer_staging_cache_and_invalidation() {
        use media_core::ai_upscale::{AiModel, AiScale, AiSettings};
        use media_native::{ai_inference::OnnxHelperConfig, ai_preview::PreviewPolicy};
        let rtx = std::env::var("DIAXUS_TEST_PREVIEW_BACKEND").as_deref() == Ok("rtx");
        let helper: media_native::ai_preview::PreviewHelper = if rtx {
            media_native::rtx_inference::RtxHelperConfig {
                executable: std::env::var_os("DIAXUS_RTX_HELPER").unwrap().into(),
                sdk_root: std::env::var_os("DIAXUS_RTX_SDK").unwrap().into(),
                cache_parent: std::env::var_os("DIAXUS_RTX_CACHE").unwrap().into(),
                dxgi_index: 1,
                exact_adapter_name: "NVIDIA GeForce RTX 3050 Laptop GPU".into(),
                expected_luid_high: std::env::var("DIAXUS_RTX_LUID_HIGH")
                    .unwrap()
                    .parse()
                    .unwrap(),
                expected_luid_low: std::env::var("DIAXUS_RTX_LUID_LOW")
                    .unwrap()
                    .parse()
                    .unwrap(),
            }
            .into()
        } else {
            OnnxHelperConfig {
                executable: std::env::var_os("DIAXUS_AI_HELPER").unwrap().into(),
                runtime_directory: std::env::var_os("DIAXUS_ORT_DIRECTORY").unwrap().into(),
                model_cache: std::env::var_os("DIAXUS_AI_MODEL_CACHE").unwrap().into(),
                dxgi_index: 1,
                exact_adapter_name: "NVIDIA GeForce RTX 3050 Laptop GPU".into(),
            }
            .into()
        };
        let helper_name = match &helper {
            media_native::ai_preview::PreviewHelper::Onnx(c) => &c.exact_adapter_name,
            media_native::ai_preview::PreviewHelper::Rtx(c) => &c.exact_adapter_name,
        };
        let processing = media_native::enumerate_adapters()
            .into_iter()
            .find(|a| a.name == *helper_name && a.backend == "Vulkan")
            .unwrap();
        let instance = w::Instance::default();
        let renderer = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(renderer.request_device(&Default::default())).unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
        let directory = root.join(format!(
            "tmp/ai-upscale/native-{}-renderer-{}",
            if rtx { "rtx" } else { "onnx" },
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let worker = Worker::new(device.clone(), queue.clone(), 41, || {});
        let mut cases = vec![];
        for (fixture, position, source, frames, scales) in [
            (
                "ai-upscale-cut",
                100000,
                Size::new(64, 36).unwrap(),
                3,
                vec![AiScale::Two, AiScale::Four],
            ),
            (
                "ai-upscale-preview",
                0,
                Size::new(192, 96).unwrap(),
                1,
                vec![AiScale::Four],
            ),
        ] {
            for scale in scales {
                let ai = if rtx {
                    AiSettings::RtxVideo {
                        configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
                        scale,
                    }
                } else {
                    AiSettings::OnnxRuntime {
                        model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                        scale,
                    }
                };
                let selected = ai
                    .requested_output_size(source, media_core::ResizeSpec::Original)
                    .unwrap();
                for strength in [0, 50, 80] {
                    let mut request = Request {
                        before: false,
                        ai: Some(PreviewPolicy {
                            ai,
                            helper: helper.clone(),
                            processing_adapter: processing.key.clone(),
                        }),
                        path: root.join(format!("fixtures/{fixture}.mp4")),
                        position_us: position,
                        selected,
                        settings: Equalization::new(strength != 0, strength).unwrap(),
                        color: if strength == 0 {
                            ColorAdjustments::default()
                        } else {
                            ColorAdjustments::new(10, 115, 80).unwrap()
                        },
                    };
                    worker.request(request.clone());
                    let output = wait(&worker);
                    let transfers = output.ai_transfers.unwrap();
                    assert_eq!(transfers.source_frames, frames);
                    assert_eq!(output.uploads, 1);
                    assert_eq!(
                        transfers.renderer_upload_bytes,
                        u64::from(output.texture.width()) * u64::from(output.texture.height()) * 4
                    );
                    let pixels =
                        super::super::history::tests::readback(&device, &queue, &output.texture);
                    let index = cases.len();
                    std::fs::write(directory.join(format!("{index}-renderer.rgba")), pixels)
                        .unwrap();
                    request.before = true;
                    worker.request(request.clone());
                    let before = wait(&worker);
                    let before_transfers = before.ai_transfers.unwrap();
                    assert_eq!(before_transfers.source_frames, transfers.source_frames);
                    assert_eq!(
                        before_transfers.inference_upload_bytes,
                        transfers.inference_upload_bytes
                    );
                    assert_eq!(
                        before_transfers.processing_upload_bytes,
                        transfers.processing_upload_bytes
                    );
                    std::fs::write(
                        directory.join(format!("{index}-before.rgba")),
                        super::super::history::tests::readback(&device, &queue, &before.texture),
                    )
                    .unwrap();
                    request.before = false;
                    worker.request(request);
                    let after = wait(&worker);
                    assert_eq!(
                        super::super::history::tests::readback(&device, &queue, &after.texture),
                        std::fs::read(directory.join(format!("{index}-renderer.rgba"))).unwrap()
                    );
                    cases.push(serde_json::json!({"index":index,"fixture":fixture,"scale":format!("{scale:?}"),
                        "strength":strength,"selected_us":position,"transfers":transfers}));
                }
            }
        }
        // A pending large preparation is invalidated; only the replacement can publish.
        let ai = if rtx {
            AiSettings::RtxVideo {
                configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
                scale: AiScale::Four,
            }
        } else {
            AiSettings::OnnxRuntime {
                model: AiModel::RealEsrGeneralX4v3StrongFp32V1,
                scale: AiScale::Four,
            }
        };
        let mut request = Request {
            before: false,
            ai: Some(PreviewPolicy {
                ai,
                helper: helper.clone(),
                processing_adapter: processing.key.clone(),
            }),
            path: root.join("fixtures/ai-upscale-preview.mp4"),
            position_us: 100000,
            selected: Size::new(768, 384).unwrap(),
            settings: Equalization::new(true, 50).unwrap(),
            color: ColorAdjustments::default(),
        };
        worker.request(request.clone());
        std::thread::sleep(std::time::Duration::from_millis(30));
        worker.control().invalidate();
        request.path = root.join("fixtures/ai-upscale-cut.mp4");
        request.position_us = 100000;
        request.selected = Size::new(256, 144).unwrap();
        worker.request(request.clone());
        let replacement = wait(&worker);
        assert_eq!(
            (replacement.texture.width(), replacement.texture.height()),
            (640, 360)
        );
        assert_eq!(replacement.request, request);
        assert_eq!(replacement.ai_transfers.unwrap().source_frames, 3);
        // Real AI preparation must make progress while playback replaces only
        // the latest pending position. Observe the actual active request token,
        // rather than inferring activity from a fixed sleep.
        request.position_us = 0;
        worker.request(request.clone());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let active = loop {
            if let Some(token) = worker.state.0.lock().unwrap().active.clone() {
                break token;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "no active AI preparation"
            );
            std::thread::yield_now();
        };
        for position in 1..=100 {
            request.position_us = position * 2000;
            worker.request_sample(request.clone(), true);
            assert!(
                !active.is_cancelled(),
                "playback starved an active AI request"
            );
        }
        let final_request = request.clone();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut published_positions = vec![];
        loop {
            let output = wait(&worker);
            published_positions.push(output.request.position_us);
            if output.request == final_request {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "latest AI sample never completed"
            );
        }
        // Once paused, a downstream edit may reuse the last AI result. Explicit
        // seek cancels/retires prior work and cannot publish its previous ticket.
        request.before = true;
        worker.request_sample(request.clone(), false);
        assert_eq!(wait(&worker).request, request);
        worker.control().invalidate();
        request.position_us = 0;
        request.before = false;
        worker.request(request.clone());
        assert_eq!(wait(&worker).request, request);
        // An actual missing helper error has no automatic fallback. A different
        // valid explicit policy is a healthy retry, without an old cache.
        *request.ai.as_mut().unwrap().helper.executable_mut() =
            directory.join("missing-helper.exe");
        request.before = true;
        worker.request(request.clone());
        let cold_before = wait(&worker);
        assert_eq!(cold_before.ai_transfers.as_ref().unwrap().source_frames, 0);
        assert_eq!(
            cold_before
                .ai_transfers
                .as_ref()
                .unwrap()
                .inference_upload_bytes,
            0
        );
        std::fs::write(
            directory.join("cold-before.rgba"),
            super::super::history::tests::readback(&device, &queue, &cold_before.texture),
        )
        .unwrap();
        request.before = false;
        worker.request(request.clone());
        let error_deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let helper_error = loop {
            if let Some(result) = worker.take_result() {
                break result.err().expect("missing helper published pixels");
            }
            assert!(std::time::Instant::now() < error_deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert!(
            helper_error.contains("configured native AI helper"),
            "{helper_error}"
        );
        let service = crate::preview::PreviewService::default();
        service.start(request.path.clone(), 0, true, true).unwrap();
        service.history_unavailable(&helper_error);
        assert!(
            service.state().paused,
            "AI error left the preview clock running"
        );
        let paused_position = service.state().position_us;
        service.pause(true); // Pause after an error must not restart/seek.
        assert_eq!(service.state().position_us, paused_position);
        service.shutdown();
        request.ai.as_mut().unwrap().helper = helper.clone();
        worker.request(request.clone());
        assert_eq!(wait(&worker).request, request);
        // Invalidate for conversion handoff, wait for the worker's retirement
        // acknowledgement, and check that no old completed pixels survive.
        worker.control().invalidate();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            let state = worker.state.0.lock().unwrap();
            if !state.clearing && state.active.is_none() {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            drop(state);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(worker.take_result().is_none());
        worker.request(request.clone());
        assert_eq!(wait(&worker).request, request);
        drop(worker);
        let lost = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback = lost.clone();
        device.set_device_lost_callback(move |_, _| {
            callback.store(true, std::sync::atomic::Ordering::Release);
        });
        let loss_worker =
            Worker::new_checked(device.clone(), queue.clone(), 42, || {}, lost.clone());
        request.path = root.join("fixtures/ai-upscale-preview.mp4");
        request.selected = Size::new(768, 384).unwrap();
        loss_worker.request(request.clone());
        std::thread::sleep(std::time::Duration::from_millis(30));
        device.destroy();
        let _ = device.poll(w::PollType::Poll);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let loss_error = loop {
            if let Some(result) = loss_worker.take_result() {
                break result.err().expect("lost renderer published pixels");
            }
            assert!(
                std::time::Instant::now() < deadline,
                "renderer-loss deadline"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert!(lost.load(std::sync::atomic::Ordering::Acquire));
        assert!(loss_error.contains("lost"), "{loss_error}");
        drop(loss_worker);
        let (fresh_device, fresh_queue) =
            pollster::block_on(renderer.request_device(&Default::default())).unwrap();
        let retry = Worker::new(fresh_device, fresh_queue, 43, || {});
        request.path = root.join("fixtures/ai-upscale-cut.mp4");
        request.position_us = 100000;
        request.selected = Size::new(256, 144).unwrap();
        retry.request(request);
        assert_eq!(wait(&retry).ai_transfers.unwrap().source_frames, 3);
        drop(retry);
        std::fs::write(
            directory.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
            "status":"renderer GPU runs completed; independent pixels required",
            "renderer":format!("{:?}",renderer.get_info()),"processing":processing,
            "invalidation":"replacement published at correct geometry; worker joined",
            "device_loss":loss_error,"fresh_generation":43,
            "playback_published_positions_us":published_positions,
            "playback_samples_submitted":100,
            "playback_active_request_cancelled":false,
            "helper_error":helper_error,
            "cold_before":{"fixture":"ai-upscale-cut","source_index":0,"input":{"width":64,"height":36},"display":{"width":640,"height":360},"transfers":cold_before.ai_transfers},
            "error_pause":"actual worker error pauses service clock; Pause does not restart",
            "handoff":"worker invalidation acknowledged and result empty; fresh request passes; UI conversion handoff remains pending",
            "cases":cases}))
            .unwrap(),
        )
        .unwrap();
        println!("Native renderer AI evidence: {}", directory.display());
    }
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
            before: false,
            ai: None,
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
