//! Counted display-image staging between native processing (wgpu 30) and Blitz
//! (wgpu 26). No GPU resource or device handle leaves this service.
use crate::{
    CancellationToken, GpuProcessor, NativeEqualization, NativeResult,
    ai_inference::{NativeAiProcessor, OnnxHelperConfig},
    preview::{HistorySummary, stream_history},
};
use media_core::{
    ColorAdjustments, OutputProfileId, ResizeSpec, Rotation, Size,
    ai_upscale::{AiMemoryPlan, AiSettings},
    equalization::Equalization,
};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(
    tag = "backend",
    content = "configuration",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PreviewHelper {
    Onnx(OnnxHelperConfig),
    Rtx(crate::rtx_inference::RtxHelperConfig),
}
impl From<OnnxHelperConfig> for PreviewHelper {
    fn from(config: OnnxHelperConfig) -> Self {
        Self::Onnx(config)
    }
}
impl From<crate::rtx_inference::RtxHelperConfig> for PreviewHelper {
    fn from(config: crate::rtx_inference::RtxHelperConfig) -> Self {
        Self::Rtx(config)
    }
}
impl PreviewHelper {
    pub fn processor(
        &self,
        ai: AiSettings,
        source: Size,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<NativeAiProcessor> {
        match (self, ai) {
            (Self::Onnx(config), AiSettings::OnnxRuntime { .. }) => {
                NativeAiProcessor::new(ai, Some(config), source, generation, cancel)
            }
            (Self::Rtx(config), AiSettings::RtxVideo { .. }) => {
                NativeAiProcessor::new_rtx(ai, config, source, generation, cancel)
            }
            _ => Err("Native preview backend/configuration mismatch; no fallback".into()),
        }
    }
    pub fn executable_mut(&mut self) -> &mut std::path::PathBuf {
        match self {
            Self::Onnx(config) => &mut config.executable,
            Self::Rtx(config) => &mut config.executable,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreviewPolicy {
    pub ai: AiSettings,
    pub helper: PreviewHelper,
    pub processing_adapter: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_core::ai_upscale::{AiModel, AiScale};
    use serde_json::json;

    #[test]
    fn preview_rejects_incompatible_color_before_normalization() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let ai = AiSettings::RtxVideo {
            configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
            scale: AiScale::Two,
        };
        let cancel = CancellationToken::default();
        assert!(source_size(&root.join("ai-upscale-cut.mp4"), ai, &cancel).is_ok());
        for (fixture, reason) in [("m35-hdr-tagged.mp4", "SDR"), ("m4-10bit-sdr.mp4", "8-bit")] {
            let error = source_size(&root.join(fixture), ai, &cancel)
                .unwrap_err()
                .to_string();
            assert!(error.contains(reason), "{fixture}: {error}");
        }
    }

    #[test]
    fn preview_backend_mismatch_never_starts_a_helper() {
        let config = PreviewHelper::Onnx(OnnxHelperConfig {
            executable: "must-not-be-launched.exe".into(),
            runtime_directory: "missing".into(),
            model_cache: "missing".into(),
            dxgi_index: 0,
            exact_adapter_name: "unselected".into(),
        });
        let ai = AiSettings::RtxVideo {
            configuration: media_core::ai_upscale::RtxConfiguration::Sdk11SdrV1,
            scale: AiScale::Four,
        };
        let result = config.processor(
            ai,
            Size::new(64, 36).unwrap(),
            1,
            &CancellationToken::default(),
        );
        let error = result
            .err()
            .expect("incompatible backend accepted")
            .to_string();
        assert!(error.contains("backend/configuration mismatch"), "{error}");
        assert!(validate_geometry(ai, Size::new(192, 108).unwrap()).is_ok());
        assert!(validate_geometry(ai, Size::new(193, 108).unwrap()).is_err());
    }

    #[test]
    #[ignore = "actual configured DirectML, native wgpu processing and decoded-video preview/export pixels"]
    fn actual_ai_preview_chronological_pixels_and_cache() {
        let rtx = std::env::var("DIAXUS_TEST_PREVIEW_BACKEND").as_deref() == Ok("rtx");
        let helper: PreviewHelper = if rtx {
            crate::rtx_inference::RtxHelperConfig {
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
            crate::ai_inference::tests::actual_config().into()
        };
        let helper_name = match &helper {
            PreviewHelper::Onnx(c) => &c.exact_adapter_name,
            PreviewHelper::Rtx(c) => &c.exact_adapter_name,
        };
        let adapter = crate::enumerate_adapters()
            .into_iter()
            .find(|a| a.name == *helper_name && a.backend == "Vulkan")
            .unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
        let output = root.join(format!(
            "tmp/ai-upscale/native-{}-preview-{}",
            if rtx { "rtx" } else { "onnx" },
            std::process::id()
        ));
        std::fs::create_dir(&output).unwrap();
        let token = CancellationToken::default();
        let mut results = vec![];
        for (fixture, position, index, predecessors) in [
            ("ai-upscale-cut", 100000, 2, 2),
            ("ai-upscale-cut", 233333, 3, 0),
            ("ai-upscale-preview", 0, 0, 0),
        ] {
            let path = root.join(format!("fixtures/{fixture}.mp4"));
            let source = crate::probe_source(&path).unwrap();
            let input = Size::new(source.width, source.height).unwrap();
            let decoded = std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&path)
                .args([
                    "-an",
                    "-vf",
                    &format!("{},format=gbrp,format=rgba", crate::color::TO_SRGB),
                    "-fps_mode",
                    "passthrough",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ])
                .output()
                .unwrap();
            assert!(decoded.status.success());
            let frame_bytes = (input.width * input.height * 4) as usize;
            assert_eq!(decoded.stdout.len(), frame_bytes * 4);
            std::fs::write(
                output.join(format!("{fixture}-original-0.rgba")),
                &decoded.stdout[..frame_bytes],
            )
            .unwrap();
            for scale in [AiScale::Two, AiScale::Four] {
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
                let policy = PreviewPolicy {
                    ai,
                    helper: helper.clone(),
                    processing_adapter: adapter.key.clone(),
                };
                let (mut preview, summary) =
                    AiPreview::prepare(&path, position, &policy, 31, &token).unwrap();
                assert_eq!(summary.predecessors, predecessors);
                assert_eq!(preview.transfers.source_frames, (predecessors + 1) as u64);
                assert!(
                    preview
                        .render(
                            Equalization::default(),
                            ColorAdjustments::default(),
                            30,
                            &token
                        )
                        .unwrap_err()
                        .to_string()
                        .contains("generation")
                );
                let selected = ai
                    .requested_output_size(input, ResizeSpec::Original)
                    .unwrap();
                let x4 = if rtx {
                    selected
                } else {
                    Size::new(input.width * 4, input.height * 4).unwrap()
                };
                let original = &decoded.stdout[index * frame_bytes..(index + 1) * frame_bytes];
                std::fs::write(
                    output.join(format!("{fixture}-original-{index}.rgba")),
                    original,
                )
                .unwrap();
                for (strength, color) in [
                    (0, ColorAdjustments::default()),
                    (50, ColorAdjustments::new(10, 115, 80).unwrap()),
                    (80, ColorAdjustments::new(10, 115, 80).unwrap()),
                ] {
                    let settings = Equalization::new(strength != 0, strength).unwrap();
                    // Same pre-encoder conversion processor, fed full chronological
                    // decode independently of the seek/preroll implementation.
                    let mut oracle = GpuProcessor::new(x4, selected, Some(&adapter.key)).unwrap();
                    oracle.prepare_ai_postscale(true);
                    if settings.active() {
                        oracle.equalization = Some(
                            NativeEqualization::new(
                                &oracle.device,
                                &oracle.queue,
                                selected,
                                settings,
                                31,
                            )
                            .unwrap(),
                        );
                    }
                    oracle.transform = oracle.pipeline.create_adjustment_buffer(
                        &oracle.device,
                        &oracle.queue,
                        Rotation::Deg0,
                        false,
                        color,
                    );
                    let mut inference = helper.processor(ai, input, 31, &token).unwrap();
                    let mut expected = vec![];
                    for (ordinal, frame) in decoded
                        .stdout
                        .chunks_exact(frame_bytes)
                        .take(index + 1)
                        .enumerate()
                    {
                        let enhanced = inference.process_frame(frame.to_vec(), 31, &token).unwrap();
                        expected.clear();
                        oracle
                            .process(
                                &enhanced.rgba,
                                [0, 66667, 100000, 233333][ordinal],
                                &mut expected,
                                &token,
                            )
                            .unwrap();
                    }
                    inference.close().unwrap();
                    let image = preview.render(settings, color, 31, &token).unwrap();
                    assert_eq!(image.selected_us, [0, 66667, 100000, 233333][index]);
                    assert_eq!(image.transfers.source_frames, (predecessors + 1) as u64);
                    let id = results.len();
                    std::fs::write(output.join(format!("{id}-preview.rgba")), &image.rgba).unwrap();
                    std::fs::write(output.join(format!("{id}-export.rgba")), expected).unwrap();
                    let before = preview.render_before(31, &token).unwrap();
                    if strength == 0 {
                        let mut cold_policy = policy.clone();
                        *cold_policy.helper.executable_mut() = output.join("missing-helper.exe");
                        let cold =
                            AiPreview::original_display(&path, position, &cold_policy, 31, &token)
                                .unwrap();
                        assert_eq!(cold.rgba, before.rgba);
                        assert_eq!(cold.transfers.source_frames, 0);
                        assert_eq!(cold.transfers.inference_upload_bytes, 0);
                        assert_eq!(cold.transfers.inference_readback_bytes, 0);
                    }
                    assert_eq!(
                        before.transfers.source_frames,
                        image.transfers.source_frames
                    );
                    assert_eq!(
                        before.transfers.inference_upload_bytes,
                        image.transfers.inference_upload_bytes
                    );
                    assert_eq!(
                        before.transfers.inference_readback_bytes,
                        image.transfers.inference_readback_bytes
                    );
                    assert_eq!(
                        before.transfers.processing_upload_bytes,
                        image.transfers.processing_upload_bytes
                    );
                    assert_eq!(before.transfers.original_upload_bytes, frame_bytes as u64);
                    std::fs::write(output.join(format!("{id}-before.rgba")), &before.rgba).unwrap();
                    let restored = preview.render(settings, color, 31, &token).unwrap();
                    assert_eq!(
                        restored.rgba, image.rgba,
                        "Before mutated selected-size statistics"
                    );
                    results.push(json!({"fixture":fixture,"scale":format!("{scale:?}"),"strength":strength,
                        "source_index":index,"predecessors":predecessors,"selected_us":image.selected_us,
                        "identity":preview.identity(),"input":input,"selected":selected,"display":image.size,"transfers":image.transfers,"index":id}));
                }
                let cancelled = CancellationToken::default();
                cancelled.cancel();
                assert!(
                    preview
                        .render(
                            Equalization::default(),
                            ColorAdjustments::default(),
                            31,
                            &cancelled
                        )
                        .is_err()
                );
            }
        }
        std::fs::write(
            output.join("report.json"),
            serde_json::to_vec_pretty(&json!({
            "status":"GPU runs completed; independent display pixel comparison required",
            "adapter":adapter,"cases":results}))
            .unwrap(),
        )
        .unwrap();
        println!("Native preview evidence: {}", output.display());
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Transfers {
    pub source_frames: u64,
    pub inference_upload_bytes: u64,
    pub inference_readback_bytes: u64,
    pub processing_upload_bytes: u64,
    pub original_upload_bytes: u64,
    pub display_readback_bytes: u64,
    /// Per published image; the service itself does not perform this upload.
    pub renderer_upload_bytes: u64,
    pub admission_bytes: u64,
}

/// One bounded display raster. The renderer uploads it on its own device.
#[derive(Debug)]
pub struct DisplayImage {
    pub rgba: Vec<u8>,
    pub size: Size,
    pub selected_us: i64,
    pub generation: u64,
    pub transfers: Transfers,
}

/// A single selected AI result plus finite selected-size CLAHE statistics.
/// Construct/use/drop on the existing latest-command preview worker.
pub struct AiPreview {
    identity: crate::ai_inference::HelperIdentity,
    gpu: GpuProcessor,
    selected_enhanced: Vec<u8>,
    original: wgpu::Texture,
    selected_us: i64,
    display: Size,
    generation: u64,
    transfers: Transfers,
}

impl AiPreview {
    /// Cold Before needs neither model nor helper. It decodes the same selected
    /// source and resizes directly to display, without an intermediate upscale.
    pub fn original_display(
        path: &Path,
        position_us: u64,
        policy: &PreviewPolicy,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<DisplayImage> {
        let input = source_size(path, policy.ai, cancel)?;
        validate_geometry(policy.ai, input)?;
        let selected = policy
            .ai
            .requested_output_size(input, ResizeSpec::Original)?;
        let factor = (640.0 / f64::from(selected.width)).min(360.0 / f64::from(selected.height));
        let size = Size::new(
            (f64::from(selected.width) * factor).floor().max(1.0) as u32,
            (f64::from(selected.height) * factor).floor().max(1.0) as u32,
        )?;
        let mut gpu = GpuProcessor::new(input, size, Some(&policy.processing_adapter))?;
        if gpu.fallback.is_some() {
            return Err("Original comparison adapter unavailable; no fallback".into());
        }
        let mut rgba = Vec::with_capacity((size.width * size.height * 4) as usize);
        let summary = stream_history(path, position_us, cancel, |frame| {
            if frame.size != input {
                return Err("Original comparison source geometry changed".into());
            }
            if frame.selected {
                gpu.process(frame.rgba, frame.relative_us, &mut rgba, cancel)?;
            }
            Ok(())
        })?;
        // Checked source envelope <=577x579 and display <=640x360 bound this
        // simpler path; it allocates no model, tensors, x4 or CLAHE resources.
        let admission_bytes = u64::from(input.width) * u64::from(input.height) * 8
            + u64::from(size.width) * u64::from(size.height) * 12
            + u64::from(gpu.padded_bytes_per_row) * u64::from(size.height);
        Ok(DisplayImage {
            rgba,
            size,
            selected_us: summary.selected_us,
            generation,
            transfers: Transfers {
                original_upload_bytes: gpu.uploaded,
                display_readback_bytes: gpu.downloaded,
                admission_bytes,
                ..Default::default()
            },
        })
    }

    pub fn identity(&self) -> &crate::ai_inference::HelperIdentity {
        &self.identity
    }

    pub fn selected_size(&self) -> Size {
        self.gpu.output_size
    }

    pub fn prepare(
        path: &Path,
        position_us: u64,
        policy: &PreviewPolicy,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<(Self, HistorySummary)> {
        cancel.check()?;
        if !matches!(
            policy.ai,
            AiSettings::OnnxRuntime { .. } | AiSettings::RtxVideo { .. }
        ) {
            return Err("Native AI preview requires a passing explicit ONNX or RTX helper".into());
        }
        // Reuse the cancellable, 64-KiB metadata probe. Preview must not scan
        // a complete conversion timeline before its bounded source preroll.
        let input = source_size(path, policy.ai, cancel)?;
        validate_geometry(policy.ai, input)?;
        let selected = policy
            .ai
            .requested_output_size(input, ResizeSpec::Original)?;
        let internal_factor = match policy.ai {
            AiSettings::RtxVideo { scale, .. } => scale.factor(),
            _ => 4,
        };
        let enhanced = Size::new(
            input
                .width
                .checked_mul(internal_factor)
                .ok_or("AI preview width overflow")?,
            input
                .height
                .checked_mul(internal_factor)
                .ok_or("AI preview height overflow")?,
        )?;
        // Match the player's fixed display framing. Enlargement here is only
        // presentation after selected-size processing, never model input/output.
        let factor = (640.0 / f64::from(selected.width)).min(360.0 / f64::from(selected.height));
        let display = Size::new(
            (f64::from(selected.width) * factor).floor().max(1.0) as u32,
            (f64::from(selected.height) * factor).floor().max(1.0) as u32,
        )?;
        let bytes = |s: Size| u64::from(s.width) * u64::from(s.height) * 4;
        let padded = (u64::from(display.width) * 4 + 255) & !255;
        let downstream = ((u64::from(selected.width) * 4 + 255) & !255)
            * u64::from(selected.height)
            + bytes(selected) * 2
            + bytes(display) * 2
            + padded * u64::from(display.height)
            + media_gpu::equalization::EqualizationStage::estimated_scratch_bytes(selected)?;
        let admission = AiMemoryPlan::new(
            input,
            8 << 20,
            128 << 20,
            downstream,
            bytes(enhanced) + bytes(input),
            384 << 20,
        )?
        .total;
        let mut gpu = GpuProcessor::new(enhanced, selected, Some(&policy.processing_adapter))?;
        if gpu.fallback.is_some() {
            return Err("Native AI preview processing adapter is unavailable; no fallback".into());
        }
        gpu.prepare_ai_postscale(true);
        gpu.prepare_preview_display(display)?;
        let original = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("native AI comparison original"),
            size: wgpu::Extent3d {
                width: input.width,
                height: input.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        gpu.equalization = Some(NativeEqualization::new(
            &gpu.device,
            &gpu.queue,
            selected,
            Equalization::new(true, 50)?,
            generation,
        )?);
        let mut inference = policy
            .helper
            .processor(policy.ai, input, generation, cancel)?;
        let identity = inference
            .identity()
            .cloned()
            .ok_or("Missing AI preview provider identity")?;
        if identity.adapter_name != gpu.adapter.name
            || identity.vendor_id != gpu.adapter.vendor
            || identity.device_id != gpu.adapter.device
        {
            return Err(
                "Native AI preview inference/processing adapter mismatch; no fallback".into(),
            );
        }
        let mut transfers = Transfers {
            admission_bytes: admission,
            ..Default::default()
        };
        let mut selected_enhanced = None;
        let summary = stream_history(path, position_us, cancel, |frame| {
            if frame.size != input {
                return Err("Native AI preview source geometry changed".into());
            }
            let result = inference.process_frame(frame.rgba.to_vec(), generation, cancel)?;
            if result.size != enhanced || result.device_generation != generation {
                return Err("Native preview inference geometry/generation mismatch".into());
            }
            transfers.source_frames += 1;
            transfers.inference_upload_bytes += result.host_to_gpu_bytes;
            transfers.inference_readback_bytes += result.gpu_to_host_bytes;
            // Explicit bounded display readback also drains each predecessor.
            // The unused predecessor image is never retained or published.
            gpu.process(
                &result.rgba,
                frame.relative_us,
                &mut std::io::sink(),
                cancel,
            )?;
            if frame.selected {
                gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &original,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    frame.rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(input.width * 4),
                        rows_per_image: Some(input.height),
                    },
                    original.size(),
                );
                transfers.original_upload_bytes = bytes(input);
                selected_enhanced = Some(result.rgba);
            }
            Ok(())
        })?;
        inference.close()?;
        cancel.check()?;
        Ok((
            Self {
                identity,
                gpu,
                selected_enhanced: selected_enhanced.ok_or("No selected AI preview source")?,
                original,
                selected_us: summary.selected_us,
                display,
                generation,
                transfers,
            },
            summary,
        ))
    }

    pub fn render(
        &mut self,
        settings: Equalization,
        color: ColorAdjustments,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<DisplayImage> {
        if generation != self.generation {
            return Err("Native AI preview generation mismatch".into());
        }
        self.gpu
            .equalization
            .as_mut()
            .ok_or("Missing AI preview statistics")?
            .settings = settings;
        self.gpu.transform = self.gpu.pipeline.create_adjustment_buffer(
            &self.gpu.device,
            &self.gpu.queue,
            Rotation::Deg0,
            false,
            color,
        );
        let mut rgba = Vec::with_capacity((self.display.width * self.display.height * 4) as usize);
        self.gpu
            .process(&self.selected_enhanced, self.selected_us, &mut rgba, cancel)?;
        self.transfers.processing_upload_bytes = self.gpu.uploaded;
        self.transfers.display_readback_bytes = self.gpu.downloaded;
        Ok(DisplayImage {
            rgba,
            size: self.display,
            selected_us: self.selected_us,
            generation,
            transfers: self.transfers.clone(),
        })
    }

    /// Comparison bypasses AI and color directly at the display framing. It
    /// preserves the enhanced result/statistics and never starts a helper.
    pub fn render_before(
        &mut self,
        generation: u64,
        cancel: &CancellationToken,
    ) -> NativeResult<DisplayImage> {
        cancel.check()?;
        if generation != self.generation {
            return Err("Native AI preview generation mismatch".into());
        }
        self.gpu.check_device_lost()?;
        let (display, neutral, size) = self
            .gpu
            .preview_display
            .as_ref()
            .ok_or("Missing AI preview display")?;
        let display = display.clone();
        let size = *size;
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.gpu.pipeline.record_resize(
            &self.gpu.device,
            &mut encoder,
            &self.original.create_view(&Default::default()),
            neutral,
            &display.create_view(&Default::default()),
            size,
        );
        let mut rgba = Vec::with_capacity((size.width * size.height * 4) as usize);
        self.gpu
            .capture_preview_or_output(encoder, &display, size, &mut rgba, cancel)?;
        self.transfers.processing_upload_bytes = self.gpu.uploaded;
        self.transfers.display_readback_bytes = self.gpu.downloaded;
        Ok(DisplayImage {
            rgba,
            size,
            selected_us: self.selected_us,
            generation,
            transfers: self.transfers.clone(),
        })
    }
}

fn validate_geometry(ai: AiSettings, source: Size) -> NativeResult<()> {
    if matches!(ai, AiSettings::RtxVideo { .. }) {
        crate::rtx_inference::validate_source(source)
    } else {
        media_core::ai_upscale::pilot_tiles(source, 128)
            .map(|_| ())
            .map_err(Into::into)
    }
}

fn source_size(path: &Path, ai: AiSettings, cancel: &CancellationToken) -> NativeResult<Size> {
    let metadata = crate::preview::probe_metadata(path, cancel)?;
    let source = metadata["streams"]
        .as_array()
        .and_then(|streams| {
            streams
                .iter()
                .find(|stream| stream["codec_type"] == "video")
        })
        .ok_or("Missing AI preview video")?;
    let color_metadata: crate::ProbeStream = serde_json::from_value(source.clone())?;
    crate::validate_native_sdr(&color_metadata, false)?;
    if source["sample_aspect_ratio"].as_str() != Some("1:1")
        || source["side_data_list"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
    {
        return Err("Native AI preview requires square pixels without display transforms".into());
    }
    ai.validate_format(
        if source["pix_fmt"].as_str() == Some("yuv420p") {
            8
        } else {
            0
        },
        true,
        OutputProfileId::Mp4H264Aac,
    )?;
    Ok(Size::new(
        u32::try_from(source["width"].as_u64().ok_or("Missing AI preview width")?)?,
        u32::try_from(
            source["height"]
                .as_u64()
                .ok_or("Missing AI preview height")?,
        )?,
    )?)
}
