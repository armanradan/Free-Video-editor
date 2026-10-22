# Tasks

Implementation is not started. Follow design.md decisions and the ai-upscaling spec. Complete numeric/device spikes before product exposure. Failed candidate assessments are evidence of unavailability, not successful backend implementations. No runtime/model/driver installation, download or license acceptance is authorized merely by these planning files.

## 1. Exact-model and platform feasibility gates

- [ ] 1.1 Create an allowlisted compact general Real-ESRGAN evaluation manifest and reproducible ONNX export recipe, recording upstream weights/export hashes, license, graph/opset, normalization, 2x/4x internal-versus-output scale and trusted FP32 outputs; verify output dimensions and reference tensors on the tiny licensed fixture, including odd/edge dimensions.
- [ ] 1.2 Evaluate that export on ONNX Runtime Web in worker and main execution with the existing processing GPU device and GPU-buffer IO; verify reference tolerances from design.md, no added production CPU pixel readback, exact device identity and compatible operator behavior, or deliver a specific failing-boundary report without enabling the candidate.
- [ ] 1.3 Evaluate the same export through Burn import/generated weights on browser WebGPU; verify Resize/pixel-shuffle semantics, wgpu dependency compatibility, same-device inputs/outputs and the same reference limits, or record the exact import/runtime blocker and unavailable result.
- [ ] 1.4 Evaluate native ONNX Runtime and native Burn on the same model/tensors with an explicitly selected GPU; verify reference tolerances, actual provider/device selection, application-visible transfers and session cleanup, recording each native cell separately in interop evidence.
- [ ] 1.5 Inspect the installed RTX 1.1 package's API/driver/toolchain/runtime/license requirements and run a minimal permitted SDK evaluation after prerequisites are available; verify 8-bit SDR initialization, two known inputs producing nonblack correctly sized outputs, explicit cleanup and precise unsupported-device/runtime errors. Do not copy SDK binaries into Git or silently install prerequisites.
- [ ] 1.6 Add the numeric/reference runner tests for tiling and compare 128/256 interiors with model-derived overlap and deterministic padding; verify tiled/untiled FP32 max error <= 0.01 and MAE <= 0.001, edge/seam distributions and tile-order invariance, and record measured memory/time. Keep failed tile policies unavailable rather than relaxing limits.
- [ ] 1.7 Publish a five-cell compatibility assessment in docs/interop-report.md and record chosen pilot backend(s), exact dependency pins and measured memory/geometry envelopes in docs/architecture.md; verify every cell is explicitly passing-pilot, failed or untested, and stop/review scope if no portable backend passes for a requested platform. No product support claim follows from an import-only pass.

## 2. Core policy, geometry and command compatibility

- [ ] 2.1 Add pure Off-default AI settings/backend/model identity and capability results in media-core; verify legacy deserialization, invalid backend/model/scale rejection, immutable copy/snapshot behavior and no platform/runtime dependencies in core.
- [ ] 2.2 Add separate checked AI geometry resolution for normalized source 2x/4x while retaining ResizeSpec's existing behavior; verify aspect ratio, even dimensions, rotated geometry, overflow, configured limits, ordinary no-upscale rejection and Off restoration in core unit tests.
- [ ] 2.3 Carry AI identity through browser worker/main command transport and native NativeJob snapshots with backward-compatible defaults; extend transport tests to verify old commands remain Off, malformed settings fail and stale generations cannot publish.
- [ ] 2.4 Reuse enlarged dimensions for exact profile probing and existing bitrate/FPS/output-size estimation; verify source-aware recommendations update without changing timing, unsupported exact codec/precision/route combinations reject before output creation, and document the operation/codec separation in architecture.md.

## 3. Explicit provisioning and safe model lifecycle

- [ ] 3.1 Implement the allowlisted asset manifest/cache on browser and native for passing portable candidates, keeping optional runtime assets lazy; verify startup/AI Off performs no model download, explicit acquisition exposes size/license/source, hashes are checked before publication and valid cache works offline.
- [ ] 3.2 Add provisioning failure/cancellation tests for corrupt/truncated assets, incompatible manifests, browser quota/network failure and native unwritable cache paths; verify actionable recovery, no untrusted execution, cleanup of incomplete downloads and no video uploads, and document cache acquisition/removal behavior in README.md.
- [ ] 3.3 Add optional native SDK/runtime path discovery and capability reporting with no hard-coded local library path or automatic license acceptance; verify missing/wrong-architecture/version components are reported and proprietary SDK assets are absent from distributable source changes.

## 4. Bounded portable inference and conversion

- [ ] 4.1 Integrate the first passing browser inference adapter into media-web's existing serialized execution context with shared GPU packing/unpacking; verify tile/frame credits, uniform/tensor lifetimes, identity and budgets with tiny actual GPU runs and separate test-only readbacks. Document any unavailable second candidate explicitly.
- [ ] 4.2 Integrate passing native portable candidate(s) behind a concrete platform dispatcher with exact GPU/provider identity and explicit staging where necessary; verify child/session ownership, bounded frames and transfer accounting without passing resources between devices or wgpu ABIs. Record failed candidate gates as unsupported, not implemented.
- [ ] 4.3 Compose AI before the existing CLAHE/manual transform and before FPS resampling on each supporting route; verify source frames advance history once, duplicates reuse processed results, neutral/Off matches existing processing, and unsupported effect or 10-bit/HDR combinations reject rather than being approximated.
- [ ] 4.4 Add real export tests for each passing portable platform/backend using Original VFR and 15/60 fixed FPS, supported audio profiles, 2x/4x sizes and full independent re-decode; verify exact output grid/coverage and clean ownership. Include browser WebCodecs and supported fixed-FPS FFmpeg WASM paths without repeating inference inside the encoder.
- [ ] 4.5 Exercise active inference cancel, OOM/budget rejection, runtime failure, actual controlled device destruction and explicit fresh-generation retry; verify partial output is unpublished and application-owned frames/tensors/leases return to zero, then record exact outcomes/copy counts and remaining limits in interop-report.md.

## 5. Gated Windows RTX native path

- [ ] 5.1 For a passing RTX pilot, build a source-only optional native helper with a versioned bounded frame protocol and SDK-specific status mapping, preserving the Rust app's unsafe-code boundaries; verify reference sample equivalence with a pinned tolerance, exact chosen NVIDIA device, output geometry/color tags and clean helper startup/shutdown. A failed pilot stays unavailable with a tested reason.
- [ ] 5.2 Connect the staged RTX adapter to existing native source-timestamp/audio/encoder policy and independent output verification; verify actual enhanced file output, 2x/4x capability checks, repeatability and counted host/device transfers without claiming NVDEC-to-RTX surface sharing.
- [ ] 5.3 Gate RTX plus manual controls/CLAHE on a proven explicit bridge and keep Main 10/HDR disabled; verify supported operation order, early incompatible-combination rejection, cancel during helper pipe IO, helper crash/timeout, partial cleanup and healthy explicit retry. Record API/driver/license/deployment evidence and reproducible commands in interop-report.md/architecture.md.

## 6. Matching preview and bounded interactive lifecycle

- [ ] 6.1 Extend browser preview to use the selected passing AI backend/model at selected output geometry before display scaling; verify paused preview/pre-encoder export pixel parity, Before restoration, cached downstream slider/Strength edits and preview availability when FFmpeg export is rejected.
- [ ] 6.2 Add a native preview adapter using validated compatible renderer inference or explicitly counted bounded display-image staging from the native service; verify ordinary-launch preview/export parity without registering foreign-device textures, and document the selected bridge and wgpu-26/30 ownership boundary.
- [ ] 6.3 Extend exact CLAHE preview preroll through AI on supported combinations; verify source PTS, finite history and selected-size statistics against chronological conversion, including seek backward, scene cuts, scale/model changes and Before/After restoration.
- [ ] 6.4 Add real preview lifecycle regressions for play/pause, error-state Pause, seek/source replacement, latest-only coalescing, conversion handoff, device loss and explicit retry; verify bounded pending work, no stale pixels, no cancellation starvation and released ownership, recording limitations instead of claiming real-time audio sync.

## 7. Compact native and browser controls

- [ ] 7.1 Add collapsed AI Upscaling controls with Off, validated backend/model/scale choices, explicit acquisition and size/status reasons; verify UI tests enforce availability in both DOM and native command handlers and browser RTX is never offered.
- [ ] 7.2 Wire conversion snapshots, retained ordinary resize settings, AI Off and independent Reset color/Before scopes; verify actual 2x selection updates dimensions/bitrate/profile checks, Off restores ordinary resize, Reset color leaves AI enabled and unsupported route settings never silently fall back.
- [ ] 7.3 Inspect normal native and browser windows in both supported themes using available UI tools; verify preview/playback, missing-assets/error states, default native no-page-scroll layout and disabled export with working preview. Update README.md with only actually supported setup, model/license acquisition, controls and limitations.

## 8. Cross-path acceptance and handoff

- [ ] 8.1 Run locked relevant core/GPU/native checks/tests/Clippy, wasm32 web checks/lints, formatting, JS syntax and ordinary non-AI regression exports; verify old settings and AI Off have unchanged behavior and no extra inference allocations/downloads. Record exact commands/results, including unavailable environments, in interop-report.md.
- [ ] 8.2 Run the supported matrix in release mode on the same licensed motion/text/face/texture/cut clips and inspect temporal artifacts; verify numeric/tile gates and preview/export/timing/audio/lifecycle results together, compare load/inference/transfers/encode/verification and total time against bilinear and matching-model runtimes, and publish measured memory/copy/quality limitations. RTX is compared as a different model, not a matching-weight parity result.
- [ ] 8.3 Audit every exposed backend against the five-cell evidence and asset/dependency license inventory; verify unsupported/untested cells remain unavailable, no unchecked tasks are represented as completed implementations, README/architecture/report agree and no browser helper/cloud or unreviewed SDK binaries were added. Leave a requested platform unaccepted if it has no passing implementation.

## Workflow follow-up

- Review planning artifacts before applying. Implementation starts only on a separate user request.
- Archive after implementation, documented evidence and review are complete; do not archive merely because the planning artifacts validate.
