# Proposal

## Why

Diaxus currently resizes video but explicitly rejects enlargement; users cannot improve low-resolution material with learned super resolution. Add an explicit, opt-in AI upscale operation with truthful backend availability, consistent preview/export behavior and bounded processing, without changing ordinary resize jobs.

## What Changes

- Introduce AI upscale settings separate from codec/encoder selection: Off by default, an explicitly selected backend, model identity where applicable, and validated enlargement/output geometry. Begin with general-content, opaque SDR and 2x/4x enlargement; preserve existing timing/audio policy.
- Evaluate five platform/backend combinations: ONNX Runtime and Burn in browser and native, plus RTX Video SDK on Windows/NVIDIA. These are candidates, not five promised working implementations. Enable a combination only after its exact model/runtime/device/route passes the release gates; report unsupported candidates rather than substituting another engine.
- Use Real-ESRGAN as the initial portable model family. ONNX Runtime loads a validated ONNX export; Burn requires an independently validated import/generated model and weights. RTX Video uses NVIDIA's models and its own native adapter, not Real-ESRGAN weights.
- Add compact native/browser controls, explicit asset acquisition, model/runtime status, exact output-size/profile checks and settings-aware bitrate estimates. Preserve source preview when only an encoder is unavailable.
- Integrate inference into bounded preview and conversion scheduling, with cancellation, cache/device-generation ownership and transfer/memory telemetry. Freeze the same upscale policy into each conversion job. Preserve independent CLAHE/manual sliders where the selected combination is validated.
- Add reproducible pixel, tile-boundary, temporal-quality, timing/audio, lifecycle and performance comparisons before choosing a shipping default. No candidate is advertised as faster or higher quality without measured evidence.

Out of scope: training, arbitrary user-supplied model graphs, temporal/diffusion video models, face restoration, interpolation, HDR/tone mapping, new codecs, browser RTX/local-helper/cloud services and mandatory zero-copy codec-surface integration. Initial AI processing is 8-bit SDR; RTX SDK 10-bit capability does not authorize silently reducing a 10-bit source or claiming Main 10 support.

## Capabilities

### New Capabilities

- `ai-upscaling`: Explicit AI enlargement, backend/model availability and provisioning, bounded inference, matching preview/export settings, lifecycle handling and evidence-based release gates.

### Modified Capabilities

None. The main OpenSpec capability inventory is currently empty. Existing implementation behavior remains the baseline, especially no implicit enlargement with AI Off.

## Impact

- `media-core`: platform-neutral settings, geometry/capability policy and serialization defaults.
- `media-gpu`: shared geometry, tensor packing/unpacking and downstream processing where real device-compatible integration is proven.
- `media-web`: model/runtime assets, worker/main transport, inference ownership and browser preview/conversion adapters.
- `media-native`: inference/SDK adapters, codec bridging, exact GPU identity and job cleanup; native presenter remains a separate renderer device/ABI.
- `ui`, `apps/web`, `apps/native`: compact selection/status controls and settings snapshots; no frame data in reactive state.
- New runtime/model dependencies remain optional and version-pinned after evaluation. NVIDIA components and model weights require separate license/distribution review. The local SDK at `D:/Libraries/RTX_Video_SDK_v1.1.0` is evaluation input, not a repository asset or portable default path.
- Tests and user/architecture/interop documentation gain supported-combination evidence during implementation. This proposal changes planning artifacts only.
