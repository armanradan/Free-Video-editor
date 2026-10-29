# Design

## Context

See proposal.md for motivation and specs/ai-upscaling/spec.md for behavior. Current implementation facts:

- `media-core::ResizeSpec::output_size` rejects percentages above 100 and exact enlargement. `VideoSettings` carries bitrate, FPS, color and equalization; old serialized fields default safely. AI requires a separate operation, not removal of those guards.
- Browser `media-web` serializes commands and owns WebCodecs/wgpu processing in worker or main execution. `apps/web` now separates preview readiness from export compatibility; FFmpeg WASM remains an encoder/storage alternative, not a separate GPU processor.
- Native conversion is composed in `media-native`; direct FFmpeg subprocesses do not expose their CUDA surfaces. Shared software/NVIDIA routes currently stage RGBA through CPU memory. A GPU descriptor is not a portable inference device handle.
- Processing uses wgpu 30.0.1. Blitz preview uses wgpu 26.0.1 via `media-gpu-blitz`, compiling shared processor sources for that ABI. Preview and conversion devices are independent; no textures can cross them by Rust type conversion or assumption.
- CLAHE uses source-PTS-aware, finite 100 ms preroll; its statistics depend on selected output geometry before manual sliders. Existing preview workers retain a current image/latest request rather than full raw histories.
- At proposal time no AI backend was implemented or measured. Subsequent task-group-1 evidence in docs/interop-report.md establishes numerical pilots for Web ONNX Runtime 1.30.0 and native DirectML 1.23.0, plus version-specific ONNX/Burn failures; bounded concrete inference adapters and diagnostic browser/native software-codec video composition now have pilot evidence. Native AI NVDEC/NVENC fails its Original-VFR timing gate and remains unavailable; preview, full lifecycle/quality and release acceptance remain open. The extracted RTX 1.1 SDK has native DX11/DX12/CUDA/Vulkan APIs and Windows runtime DLLs. Its sample README requires R550.50+ drivers, R570.20+ for CUDA, and CUDA 12.8+ for the relevant samples; package requirements were inspected, but runtime evaluation remains pending prerequisites.

## Goals / Non-Goals

**Goals:** separate model identity from execution backend and codec route; prove one exact portable model consistently across candidate engines; reuse bounded scheduling and downstream processing; allow measured platform-specific implementations without changing output policy.

**Non-Goals:** a generic arbitrary-model graph engine, simultaneous multi-backend inference, an installed browser helper, automatic engine fallback, a new async runtime, or an assumption that every candidate must ship. Initial general-content 8-bit SDR and 2x/4x are proposed product defaults, not measured quality/performance claims. Supporting other content/models or 10-bit requires separate validation and scope review.

## Decisions

### 1. Separate AI operation, inference engine and encoder

Extend core settings with an Off-default upscale policy and concrete backend IDs (ONNX Runtime, Burn, RTX Video). A model reference includes an immutable manifest/version/content hash for portable models; RTX records its SDK/runtime/effect configuration instead of pretending to load Real-ESRGAN weights. Job/cache keys carry scale, normalized source identity, model/configuration, precision and device generation.

When AI is On, its 2x/4x selection replaces ordinary resize sizing for that job: multiply normalized source dimensions with checked arithmetic, apply the existing even codec-size rules, then check actual inference/GPU/encoder limits. Ordinary resize controls are inactive with an explanation, but their previous values are retained. Off restores them without weakening their no-upscale checks. No arbitrary AI target size/stretch in this slice. A model's internal scale must be distinguished from user scale: if a validated x4 model supplies x2 via a final shared downscale, name and test that operation explicitly; prefer native-scale artifacts where available. Never bilinearly enlarge input before feeding the model.

AI settings are not color settings. Reset color leaves AI/backend/scale alone; AI has its own Off action. Before temporarily bypasses AI and all color effects, showing the original normalized source at the same display framing. That comparison can use a neutral presentation resize; it is not permission to export an ordinary no-AI upscale.

Alternative rejected: treating ONNX Runtime, Burn and RTX as interchangeable model-file loaders. Burn's documented import generates Rust code/weight artifacts; RTX uses NVIDIA's own models. Runtime-specific representation remains behind platform adapters.

### 2. Evaluate all five candidates, ship only proven combinations

| Platform | ONNX Runtime | Burn | RTX Video |
|---|---|---|---|
| Browser | Exact exported model on WebGPU, worker/main checked separately | Exact imported model on a compatible WebGPU backend | Not offered |
| Windows native | Supported GPU provider, exact selected device and transfer path | Compatible GPU backend and exact model/device | Native SDK 1.1, driver/device/runtime capability probe |

Use a compact general Real-ESRGAN model as first candidate, with official weights/export and a reproducible export recipe. Record exact graph/opset and model-local normalization, padding and upsampling semantics. Compare ONNX Runtime and Burn on the same weights and reference tensors; do not infer parity from an operator-name list. Inspect Resize semantics and pixel shuffle/unshuffle exports explicitly. A failed import/operator/device-sharing spike yields an unavailable reason and evidence, not an automatic replacement engine. Pin runtime versions after this compatibility evaluation, not whichever development documentation looks newest.

Each matrix cell must receive an assessment. Development proceeds with the first passing portable cell without requiring two inference engines to be maintained from day one. Successful evaluation does not make a cell user-selectable until its full preview/export/lifecycle gates pass. A platform with no passing cell retains AI unavailable; do not call that platform's AI feature complete. Replacing a failed candidate or materially relaxing gates requires a follow-up decision, not silently checking its implementation tasks complete.

### 3. Keep boundaries concrete and device-aware

Core owns only pure settings, dimension/budget arithmetic and structured capability results. Shared GPU code owns normalization, texture/tensor packing, overlap/cropping and postprocessing math where it can execute on the existing device. Browser runtime glue stays in `media-web`; native inference/SDK/codec bridges stay in `media-native`. No Dioxus frames/tensors or vendor APIs enter core. Add an enum-backed platform dispatcher first; introduce a common operational trait only when actual second implementations demonstrate a useful common interface. No empty backend crates.

For browser inference, prove the selected runtime can use the existing execution-context device and GPU-buffer inputs/outputs. Convert texture tiles to model layout and back on that same device; validate ordering and tensor lifetimes before canvas capture. Do not silently allocate a second GPU device or add `VideoFrame.copyTo`/CPU tensor readback to make a candidate work. A candidate failing this gate stays unavailable pending an explicit architecture revision. Exact APIs and version compatibility are spike outputs, not assumptions.

Native provider APIs may use CUDA, Vulkan or D3D resources different from wgpu; same physical GPU does not make resources interchangeable. Prefer a measured, explicitly counted staging baseline over unsupported foreign-handle casts. Any new native bridge carries its own ownership/transfer telemetry. Coordinate device-loss callbacks with their owning service; do not overwrite Blitz callbacks from a worker. Renderer ABI upgrades are not an implicit dependency of this change.

### 4. RTX is optional native processing, not a browser or FFmpeg flag

Use the extracted SDK as a local evaluation input via an explicit configurable SDK path, never hard-code Arman's library directory into distributed binaries. Keep proprietary headers/libraries/runtime assets out of Git. Inspect the shipped license, required notices and redistributable list before packaging; no license is accepted automatically.

The first RTX spike exercises the supplied native API on tiny 8-bit SDR inputs and checks initialization/evaluation/cleanup on the RTX 3050. A narrow native helper with a versioned local protocol is the proposed baseline for isolating C++/SDK tooling from the GNU Rust app: one bounded input/output frame slot over pipes or explicitly owned local shared memory, no per-frame files. It is a native app implementation detail, never a browser service. Record SDK/provider errors and kill/reap the owned helper on cancellation. Do not reuse a possibly corrupted helper after failure.

The CUDA surface chain `NVDEC -> RTX -> NVENC` is a later optimization gate within native evaluation, not required to claim a counted staged baseline works. Existing FFmpeg CLI processes cannot hand us their surfaces; avoiding transfers requires a real codec integration proof. Do not use desktop/browser playback enhancement as export evidence. RTX plus CLAHE/manual sliders needs a validated same-device or explicit staging bridge; unavailable combinations reject early. SDK 10-bit/HDR features remain disabled in the initial 8-bit product path.

### 5. Order processing once per source frame

```text
decode -> crop/PAR/orientation + required color normalization
       -> AI upscale -> CLAHE -> saturation/contrast/brightness
       -> output-FPS duplicate/drop -> encode/mux
```

Off retains the old fused resize path and incurs no AI session/model cost. Model normalization follows its manifest (for the initial candidate, encoded sRGB RGB float tensors); clipping and quantization points must be explicit and identical in preview and conversion. Keep existing input color/geometry restrictions where platform ingress cannot normalize accurately. Re-probe exact codecs and recalculate bitrate/size estimates using actual enlarged dimensions. FFmpeg WASM receives already processed frames; do not repeat inference in its encoder or weaken its CFR/audio/storage constraints.

### 6. Fixed-shape tiled portable inference, bounded whole-frame RTX

Start with one inference frame and one tile in flight. Benchmark fixed tile interiors (128 and 256 source pixels) with model-specific overlap and deterministic edge padding. Derive halo/cropping from the exported model's receptive field and measured reference equivalence; no guessed universal overlap. Tile outputs contribute only their validated interior. Alternate tile size/order must reproduce the same result within the recorded tolerance. RTX can remain whole-frame when that is its supported interface; reject over-budget sizes rather than assuming portable-model tiling is valid for it.

Use a per-job memory ledger including weights, input/output tensors, activations/workspace estimates, full output texture, downstream CLAHE scratch and retained frame/import guards. Existing CLAHE's 192 MiB scratch limit is not a total AI budget. Before UI exposure, calibrate/pin concrete safe envelopes on the target device and record them in backend manifests/evidence. Unknown runtime/driver allocations are separately measured/qualified, not asserted strictly bounded by application counters. Runtime OOM terminates the job cleanly. Timings separate model loading/compilation, decode, inference, transfers, downstream processing, encoding and verification; no real-time preview promise.

### 7. Preview is the same operation, not a cheap stand-in

Paused preview evaluates the selected normalized source at selected AI output geometry, then downsizes only the final adjusted image for display. Cache one AI output per current source PTS/model/scale/device identity; downstream Strength/manual changes can reuse it. Source/model/scale/seek/device changes invalidate it. With CLAHE, stream exact finite source preroll through AI before advancing output-resolution statistics; do not equalize a separately resized low-resolution preview. Large/slow combinations can be unavailable rather than falsely labeled equivalent.

Browser preview remains in its existing execution context. Native preview requires an explicit adapter because Blitz has a different device and wgpu ABI: reuse a compatible renderer inference context only after proving it, otherwise stage one bounded processed display image from a separate native preview service and count the transfers. Never register a processing texture on the renderer's device. Compare preview and export pre-encoder pixels before lossy encoding/display scaling; accept display-only rounding separately. Before/After does not mutate job settings or force re-download/re-inference when caches remain valid.

Playback independently samples positions and coalesces into one latest pending command; preserve active progress rather than starving every inference through cancellation. Explicit source/seek invalidation prevents stale publication. Pause always pauses its clock even after an AI error. Conversion uses immutable settings and an independent preserve-all-frames source timeline.

### 8. Gate assets, product controls and acceptance

Provisioning decision confirmed by the user during implementation (2026-10-22):
the exact validated ONNX graph is imported from a local file, not fetched from an
invented/unverified model URL. Official upstream PyTorch weights plus the pinned
export recipe remain provenance, not browser-executable model assets. Verify exact
byte count/hash before model cache publication on either platform. Browser runtime
assets have their own explicit acquisition action with pinned 1.30.0 byte hashes;
cached code is verified before execution through module blobs and a verified WASM
binary. Off/startup invokes neither acquisition nor runtime loading. Native runtime
installation remains separate and never automatic. Cache removal is explicit;
removing persistent assets does not claim to unload active compiled runtime code.

A small allowlisted manifest records official source URL, model/export hashes, native/internal scale, normalization, precision, tile policy, attribution/license, runtime compatibility and byte sizes. Acquire only after an explicit action; verify before cache publication, cancel downloads safely, handle quota/offline/corruption, and reuse valid cached assets offline. Burn generated artifacts belong to the build, weights can remain separately provisioned. Optional runtime code/assets should not inflate Off startup unnecessarily; do not download proprietary SDK components automatically.

Put a collapsed AI Upscaling section beside size settings in both apps. Separate AI backend/model from codec backend/processing route; show supported scale, resulting size, acquisition state and unavailable reasons. Offer choices only after their gates pass. Keep window compact and inspect actual native/browser layout. AI errors do not disable Pause or hide actionable messages. Incompatible native route/effect settings remain visible but cannot start conversion. Do not switch routes on the user's behalf.

Acceptance uses a checked-in tiny CC0 source/manifest plus licensed representative motion, text, faces, texture and cut cases under ignored test storage. Numeric portable baseline: FP32 pre-encoder output compared with trusted reference at identical padding/geometry, maximum absolute normalized-channel error <= 0.01 and mean absolute error <= 0.001; also inspect edge/interior distributions and model output before clamping. Compare tiled versus untiled on small inputs within the same limits. Calibrate artifact/seam checks before enabling tiling; do not relax tolerances to pass an incorrect Resize import. Different RTX models need their own reference/repeatability/seam and human-reviewed temporal checks, not artificial equality to Real-ESRGAN. Any lower-precision optimization needs new explicit tolerance evidence.

Test full decode/encode/re-decode frame grids, A/V coverage, exact dimensions, main/worker and native preview/export, repeated jobs, active cancel/seek/replacement, device destruction and fresh retry. Include AI Off regression, unsupported combinations, source-preview independence and settings snapshots. Compare complete release-build conversion against ordinary bilinear resize and between runtimes; report quality/performance rather than promising superiority. Evidence identifies runtime/model/driver/browser/GPU, budgets, copies, queue peaks and test-only readbacks. Firefox remains untested unless the user requests new checks; unavailable platforms are not passing skips.

## Risks / Trade-offs

- Image-model temporal flicker, invented texture and text damage -> evaluate motion/cuts/faces/text; name the selected model, no fidelity guarantee or hidden face enhancement.
- Operator import or Resize differences -> exact graph/reference checks first; leave incompatible candidates unavailable.
- Multiple device/API versions and callback ownership -> generation-tagged adapters and explicit bridges; never cast foreign handles or install competing loss callbacks.
- GPU memory pressure on the laptop GPU -> conservative measured limits, tiled portable inference, bounded preview, explicit OOM failure; do not promise 4K at every scale.
- Proprietary SDK/toolchain/redistribution constraints -> isolated optional native helper, package-specific license review, source-only adapter in repository.
- Slower preview with exact CLAHE preroll -> cache successful AI outputs, latest-only scheduling and truthful loading/error state; preserve responsive controls without claiming synchronized real-time playback.

## Migration Plan

1. Finish model/device/runtime spikes and record all five matrix outcomes before selecting a supported pilot backend per available platform.
2. Add Off-default policy/transport/geometry and bounded adapters behind disabled experimental capability flags. Keep existing manifests/command payloads compatible.
3. Integrate supported preview/conversion paths and provisioning, then run the acceptance matrix. Unsupported candidates remain unavailable; failed gates cannot be replaced by unchecked fallbacks.
4. Expose only passing combinations and update README, architecture and interop evidence. All currently supported non-AI jobs remain unchanged.
5. Roll back by disabling AI capabilities/defaulting new jobs Off, rejecting unsupported saved On settings with a clear message, and retiring AI-owned resources safely. Do not silently export an AI-requested job without AI.

## Open Questions

- Which exact candidate artifact/runtime versions and calibrated tile/memory envelopes pass on each available device? These are outputs of the specified spikes, not assumptions of interoperability.
- Which passing portable runtime wins the measured quality/performance/maintenance comparison? Both remain candidates until evidence exists; the public capability gate and task order do not depend on that result.

## References

Reviewed during exploration on 2026-10-22; re-check against the versions selected for implementation:

- [Real-ESRGAN official model catalog](https://github.com/xinntao/Real-ESRGAN/blob/master/docs/model_zoo.md) and [ONNX exporter](https://github.com/xinntao/Real-ESRGAN/blob/master/scripts/pytorch2onnx.py).
- [ONNX Runtime WebGPU GPU-buffer IO](https://onnxruntime.ai/docs/tutorials/web/ep-webgpu.html) and [operator coverage](https://github.com/microsoft/onnxruntime/blob/main/js/web/docs/webgpu-operators.md).
- [Burn ONNX import](https://burn.dev/books/burn/onnx-import.html) and [Resize compatibility issue](https://github.com/tracel-ai/burn-onnx/issues/546). Support lists do not prove a given exported model correct.
- [RTX Video SDK requirements](https://developer.nvidia.com/rtx-video-sdk/getting-started); the extracted SDK's `samples/ReadMe.md`, `samples/RTX_Video_API/rtx_video_api.h` and license are the package-specific evaluation inputs. No binaries have been executed or redistributed by this planning change.
