//! Bounded browser integration pilot. Ordinary command handlers still reject On
//! until composition, actual video and lifecycle acceptance gates pass.
use js_sys::{Function, Promise};
use media_core::{
    MediaError, Size,
    ai_upscale::{AiMemoryPlan, pilot_tiles},
};
use media_gpu::ai_tile::AiTilePipeline;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/src/ai-inference.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = createAiSession)]
    fn create_session(
        ort: &JsValue,
        model: &js_sys::Uint8Array,
        device: &JsValue,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch, js_name = runAiTile)]
    fn run_tile(
        owner: &JsValue,
        input: &JsValue,
        output: &JsValue,
        width: u32,
        height: u32,
    ) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch, js_name = finishAiWork)]
    fn finish_work(owner: &JsValue) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch, js_name = releaseAiSession)]
    fn release_session(owner: &JsValue) -> Result<Promise, JsValue>;
}

fn error(value: JsValue) -> MediaError {
    MediaError::AiUnavailable(format!("AI GPU runtime: {value:?}"))
}

pub struct BrowserAiProcessor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: AiTilePipeline,
    owner: Option<JsValue>,
    size: Size,
    generation: u64,
    poisoned: bool,
    plans: Vec<wgpu::Buffer>,
    memory_plan: AiMemoryPlan,
}

struct PendingTarget(Option<wgpu::Texture>);
impl Drop for PendingTarget {
    fn drop(&mut self) {
        if let Some(texture) = self.0.take() {
            texture.destroy();
        }
    }
}

impl BrowserAiProcessor {
    /// Explicit setup reads hash-checked cache only. Caller supplies the existing
    /// processing device/queue; no requestDevice, new ABI or implicit acquisition.
    pub async fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        size: Size,
        generation: u64,
        downstream_bytes: u64,
        retained_bytes: u64,
    ) -> Result<Self, MediaError> {
        pilot_tiles(size, 128)?;
        // Pinned conservative pilot allowances, qualified by measured requested
        // resources. This is admission control, not a strict driver-memory cap.
        let memory_plan = AiMemoryPlan::new(
            size,
            8 << 20,
            128 << 20,
            downstream_bytes,
            retained_bytes,
            384 << 20,
        )?;
        let model = crate::read_ai_model().await?;
        let ort = crate::load_ai_runtime().await?;
        let raw = device.as_webgpu().ok_or_else(|| {
            MediaError::AiUnavailable("AI requires the processing WebGPU device".into())
        })?;
        let oom_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = AiTilePipeline::new(&device, size, generation)?;
        let created = match create_session(&ort, &model, raw.as_ref()) {
            Ok(promise) => JsFuture::from(promise).await.map_err(error),
            Err(value) => Err(error(value)),
        };
        let validation = validation_scope.pop().await;
        let oom = oom_scope.pop().await;
        if let Some(failure) = oom.or(validation) {
            if let Ok(owner) = created
                && let Ok(promise) = release_session(&owner)
            {
                let _ = JsFuture::from(promise).await;
            }
            pipeline.input().destroy();
            pipeline.output().destroy();
            return Err(MediaError::AiUnavailable(format!(
                "AI setup GPU error: {failure}"
            )));
        }
        let owner = match created {
            Ok(owner) => owner,
            Err(failure) => {
                pipeline.input().destroy();
                pipeline.output().destroy();
                return Err(failure);
            }
        };
        Ok(Self {
            device,
            queue,
            pipeline,
            owner: Some(owner),
            size,
            generation,
            poisoned: false,
            plans: Vec::new(),
            memory_plan,
        })
    }

    /// `&mut self` supplies one frame credit. Each tile drains before reusing the
    /// one input/output pair. Source is already oriented/square-pixel RGBA8 SDR.
    /// The returned x4 texture stays on this device for downstream processing.
    pub async fn process(
        &mut self,
        source: &wgpu::Texture,
        generation: u64,
        cancelled: &Function,
    ) -> Result<wgpu::Texture, MediaError> {
        if generation != self.generation || self.poisoned || self.owner.is_none() {
            return Err(MediaError::AiUnavailable(
                "AI session generation is retired; explicit fresh setup required".into(),
            ));
        }
        let oom_scope = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let result = self.process_inner(source, cancelled).await;
        // Drain failed/cancelled work too before destroying immutable uniforms.
        let drained = if let Some(owner) = self.owner.as_ref() {
            match finish_work(owner) {
                Ok(promise) => JsFuture::from(promise).await.map_err(error),
                Err(e) => Err(error(e)),
            }
        } else {
            Ok(JsValue::UNDEFINED)
        };
        for plan in self.plans.drain(..) {
            plan.destroy();
        }
        let validation = scope.pop().await;
        let oom = oom_scope.pop().await;
        if let Some(failure) = oom.or(validation) {
            self.poisoned = true;
            if let Ok(texture) = result {
                texture.destroy();
            }
            return Err(MediaError::AiUnavailable(format!(
                "AI GPU error: {failure}"
            )));
        }
        if let Err(e) = drained {
            self.poisoned = true;
            if let Ok(texture) = result {
                texture.destroy();
            }
            return Err(e);
        }
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    async fn process_inner(
        &mut self,
        source: &wgpu::Texture,
        cancelled: &Function,
    ) -> Result<wgpu::Texture, MediaError> {
        let owner = self
            .owner
            .as_ref()
            .ok_or_else(|| MediaError::AiUnavailable("AI session closed".into()))?;
        let input = self
            .pipeline
            .input()
            .as_webgpu()
            .ok_or_else(|| MediaError::AiUnavailable("AI input has no WebGPU handle".into()))?;
        let output =
            self.pipeline.output().as_webgpu().ok_or_else(|| {
                MediaError::AiUnavailable("AI output has no WebGPU handle".into())
            })?;
        let is_cancelled = || -> Result<(), MediaError> {
            if cancelled.call0(&JsValue::NULL).map_err(error)?.as_bool() != Some(false) {
                return Err(MediaError::AiUnavailable("AI operation cancelled".into()));
            }
            Ok(())
        };
        is_cancelled()?;
        let mut target =
            PendingTarget(Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("AI one-frame x4 result"),
                size: wgpu::Extent3d {
                    width: self.size.width * 4,
                    height: self.size.height * 4,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })));
        for tile in pilot_tiles(self.size, 128)? {
            is_cancelled()?;
            let mut encoder = self.device.create_command_encoder(&Default::default());
            self.plans.push(self.pipeline.record_pack(
                &self.device,
                &mut encoder,
                source,
                tile,
                self.generation,
            )?);
            self.queue.submit([encoder.finish()]);
            JsFuture::from(
                run_tile(
                    owner,
                    input.as_ref(),
                    output.as_ref(),
                    tile.x1 - tile.x0,
                    tile.y1 - tile.y0,
                )
                .map_err(error)?,
            )
            .await
            .map_err(error)?;
            is_cancelled()?;
            let mut encoder = self.device.create_command_encoder(&Default::default());
            self.plans.push(self.pipeline.record_unpack(
                &self.device,
                &mut encoder,
                target.0.as_ref().expect("pending target"),
                tile,
                self.generation,
            )?);
            self.queue.submit([encoder.finish()]);
            JsFuture::from(finish_work(owner).map_err(error)?)
                .await
                .map_err(error)?;
        }
        is_cancelled()?;
        Ok(target.0.take().expect("completed target"))
    }

    /// Wait for submitted work before explicitly releasing runtime and buffers.
    pub async fn close(&mut self) -> Result<(), MediaError> {
        self.poisoned = true;
        if let Some(owner) = self.owner.take() {
            let result = match release_session(&owner) {
                Ok(promise) => JsFuture::from(promise).await.map_err(error),
                Err(value) => Err(error(value)),
            };
            self.pipeline.input().destroy();
            self.pipeline.output().destroy();
            result?;
        }
        Ok(())
    }

    /// Exact application-owned allocations; ORT/driver workspace is additional
    /// and must be measured separately, not advertised as a hard VRAM cap.
    pub fn owned_bytes(&self) -> u64 {
        AiTilePipeline::INPUT_BYTES
            + AiTilePipeline::OUTPUT_BYTES
            + u64::from(self.size.width) * u64::from(self.size.height) * 16 * 4
    }

    pub fn memory_plan(&self) -> AiMemoryPlan {
        self.memory_plan
    }
}

impl Drop for BrowserAiProcessor {
    fn drop(&mut self) {
        // Explicit close is the observable completion boundary. Drop is a safety
        // net for error unwinding; never destroy the shared processing device.
        if let Some(owner) = self.owner.take() {
            let input = self.pipeline.input().clone();
            let output = self.pipeline.output().clone();
            let plans = std::mem::take(&mut self.plans);
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(promise) = release_session(&owner) {
                    let _ = JsFuture::from(promise).await;
                }
                input.destroy();
                output.destroy();
                for plan in plans {
                    plan.destroy();
                }
            });
        }
    }
}
