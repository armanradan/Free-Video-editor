# Tasks

Continuation 2026-10-30 (native RTX quality/performance): same-NVIDIA release
RTX/DirectML/shared-wgpu bilinear comparison passes72 full-matrix and24 isolated
Original-FPS files, independent timing/endpoints/audio/full decode. Both scales,
three existing64-frame licensed excerpts and CC0 cut are covered; all chronological
contact sheets plus selected4x details were inspected with limitations recorded.
CPU wall intervals and the RTX helper's final DXGI sampled peak now distinguish
startup from inference/IO and downstream work. RTX total5.18–6.19s includes about
3s startup for5.33s tiny clips; no end-to-end speedup, real-time preview or larger
source claim. Repeated SDK startup per seek is identified, not fixed. Reference
bilinear is diagnostic-only; ordinary Off still rejects enlargement. Actual
bilinear pixels and both configured handoff regressions pass. Task8.2 remains
open for its joined ordinary-preview/release scope; progress24/32. The user chose
desktop tools in a fresh session for remaining native window checks. See newest
interop record and its concrete handoff; ordinary controls remain unavailable.

Continuation 2026-10-30 (ordinary native configuration/handoff): the ordinary
panel imports explicit bounded strict local ONNX/RTX configuration without a
capability grant. Native preview and immutable conversion snapshots now use the
same config; exact backend/scale/session GPU and actual inspected geometry are
checked without fallback. Actual RTX+ONNX2x/4x service/renderer handoff, cancelled
output, successful export and identical paused restoration pass through that
entry point with test-only acceptance metadata. Four exports independently pass
timing/endpoints/audio and full decode. Native/window consent/setup/release
acceptance remains pending;6.2/6.4/7.1–7.3 stay partial, progress24/32. Ordinary
controls remain unavailable. See newest interop evidence; no diagnostic metadata
is imported as product acceptance.

Continuation 2026-10-29 (RTX preview): explicit backend-specific service and
wgpu30-to-wgpu26 renderer staging pass55 independent comparisons at<=1LSB,
cached controls/Before/After,100 latest-only samples, replacement, missing-helper
retry, retirement and controlled renderer loss/fresh generation. SDR validation
now precedes normalization. Diagnostic window configuration accepts RTX, but
no RTX window inspection or ordinary-launch acceptance is claimed. Fresh seeks
restart the helper; interactive performance remains unaccepted. Tasks6.2/6.4
remain partial and unchecked; progress24/32. See newest interop record.

Continuation 2026-10-29 (RTX effects/lifecycle): task5.3 passes selected-size
CLAHE/manual reference gates, actual effects video timing/audio, active pipe
write/read cancellation and timeout, helper crash, controlled processing-device
loss, budget rejection, partial cleanup and seven healthy explicit retries.
Normal helpers exclude diagnostic fault controls. Progress24/32. Native preview,
ordinary UI/exposure and full release quality/performance gates remain open.

Continuation 2026-10-29 (RTX video): task5.2 passes the explicit native RTX
export pilot, two native scales x Original/15/60 FPS, independent complete A/V
decode, preserved source PTS/audio, exact repeats and counted staging. Three
six-file matrices pass; final Rust client/neutral preencoder matches SDK sample
pixels. Progress23/32. Task5.3 effects/lifecycle is next; native preview/UI and
full quality/performance gates remain open. No ordinary RTX control is enabled.

Continuation 2026-10-29 (helper): task5.1 passes actual versioned bounded pipe
exchange and exact unmodified SDK sample equivalence:12 comparisons,17 process
runs, explicit RTX3050/DXGI1 identity, color/geometry, cleanup and fresh retry.
Progress22/32. Task5.2 native source-timestamp/audio/export integration is next;
native RTX controls remain unavailable. See the newest interop record.

Continuation 2026-10-29: after user-installed C++ tools, actual RTX SDK1.1 D3D11
feasibility passes on explicitly selected RTX3050/DXGI1. Two known opaque SDR
inputs at2x/4x, exact upload/size/alpha checks, SDK High/Bicubic differences,
repeatability, actual SDK error/cleanup and fresh-process retry are verified.
Initial link/Bicubic repeat failures remain preserved. Task1.5 is complete,
progress21/32. Task5.1 bounded helper and SDK sample equivalence are next; no
ordinary native/RTX controls are enabled by synthetic feasibility. Earlier
prerequisite-pending statements describe prior observations.

Priority 2026-10-29: user requests native-first and RTX soon; task 1.5 is active
ahead of further browser work. Local SDK terms/evaluation are explicitly
authorized. Standalone D3D11 probe source is prepared but uncompiled: actual
LLVM build stops at missing MSVC excpt.h. Installing the catalogued v143 C++
component requires the pending user choice. No SDK initialization/output or
product capability is claimed; progress remains 20/32.

Continuation 2026-10-26: evaluated Windows Edge 156 NVIDIA Ampere ONNX browser
route gains numeric/tile, real UI/export/lifecycle and performance evidence.
Dedicated-profile GPU launch reduces the three 64-frame clips from Intel
37.53–46.92 seconds to NVIDIA 3.26–3.68 seconds. H.264 is accepted; NVIDIA WebM
remains unavailable after independent endpoint metadata could not be verified.
The selection/recovery regression is fixed and recorded. Native/RTX and full
cross-platform tasks stay open: 20/32. See the newest interop record.

Continuation 2026-10-25: normal browser builds now expose validated ONNX 2x/4x
on the scoped Windows Edge 156 / Intel gen-12lp route. Ordinary main/worker, both
themes, eight independently verified UI exports, missing-assets/retry/Off/limits,
default-compiler numeric/tile gates, 192-frame reference and 24 isolated
AI/bilinear exports have evidence in the newest interop section. Native ordinary
AI, RTX and full cross-platform acceptance remain open; progress stays 20/32.
Earlier continuation statements describe earlier builds.

Implementation started with isolated task-group-1 probes on 2026-10-22; see docs/interop-report.md and tools/ai-upscale/README.md. Core policy, provisioning, bounded browser/native inference and diagnostic video composition/export are implemented on the tested routes; native AI NVDEC/NVENC is unavailable after a timing failure. Browser paused preview and active failure/retry gates now pass their tested diagnostic scope. The native staged preview adapter has actual GPU pixel/cache/replacement/loss-retry evidence; ordinary-window/Before/playback, full quality acceptance and enabled product exposure remain open. Partial 7.1 now adds guarded setup UI in both apps: exact model import/check/removal, explicit browser runtime acquisition/cancel/removal, native read-only runtime inspection and unavailable scale reasons. On selection and preview/conversion wiring remain open. Follow design.md decisions and the ai-upscaling spec. Complete numeric/device spikes before product exposure. Failed candidate assessments are evidence of unavailability, not successful backend implementations. No runtime/model/driver installation, download or license acceptance is authorized merely by these planning files. The user separately authorized isolated open-source evaluation downloads during implementation; SDK terms, driver and C++ toolchain installation remain separate.

## 1. Exact-model and platform feasibility gates

- [x] 1.1 Create an allowlisted compact general Real-ESRGAN evaluation manifest and reproducible ONNX export recipe, recording upstream weights/export hashes, license, graph/opset, normalization, 2x/4x internal-versus-output scale and trusted FP32 outputs; verify output dimensions and reference tensors on the tiny licensed fixture, including odd/edge dimensions.
- [x] 1.2 Evaluate that export on ONNX Runtime Web in worker and main execution with the existing processing GPU device and GPU-buffer IO; verify reference tolerances from design.md, no added production CPU pixel readback, exact device identity and compatible operator behavior, or deliver a specific failing-boundary report without enabling the candidate.
- [x] 1.3 Evaluate the same export through Burn import/generated weights on browser WebGPU; verify Resize/pixel-shuffle semantics, wgpu dependency compatibility, same-device inputs/outputs and the same reference limits, or record the exact import/runtime blocker and unavailable result.
- [ ] 1.4 Evaluate native ONNX Runtime and native Burn on the same model/tensors with an explicitly selected GPU; verify reference tolerances, actual provider/device selection, application-visible transfers and session cleanup, recording each native cell separately in interop evidence.
- [x] 1.5 Inspect the installed RTX 1.1 package's API/driver/toolchain/runtime/license requirements and run a minimal permitted SDK evaluation after prerequisites are available; verify 8-bit SDR initialization, two known inputs producing nonblack correctly sized outputs, explicit cleanup and precise unsupported-device/runtime errors. Do not copy SDK binaries into Git or silently install prerequisites.
- [x] 1.6 Add the numeric/reference runner tests for tiling and compare 128/256 interiors with model-derived overlap and deterministic padding; verify tiled/untiled FP32 max error <= 0.01 and MAE <= 0.001, edge/seam distributions and tile-order invariance, and record measured memory/time. Keep failed tile policies unavailable rather than relaxing limits.
- [x] 1.7 Publish a five-cell compatibility assessment in docs/interop-report.md and record chosen pilot backend(s), exact dependency pins and measured memory/geometry envelopes in docs/architecture.md; verify every cell is explicitly passing-pilot, failed or untested, and stop/review scope if no portable backend passes for a requested platform. No product support claim follows from an import-only pass.

Current gate evidence: 1.1 establishes x4 references; later browser diagnostic video validates x4-then-shared-downscale 2x only on its tested route. 1.2 initially delivered a negative 1.23.0 assessment; unchanged-model Web 1.30.0 passes numeric/GPU-buffer/device/no-CPU/release checks in both contexts. 1.3 is checked for its specified **runtime-blocker/unavailable report**: browser Burn requests 1024 workgroup invocations on the shared device's 256 limit. 1.6 is the independent CPU numeric/tile reference gate. 1.4 stays open because native Burn fails its first GPU kernel, although native DirectML passes. 1.5 stays open after prerequisite/license inspection; the user leaves SDK execution pending. 1.7 publishes all five assessments and 577x579 measured pilot envelopes. Tasks 4.1–4.2 now add bounded concrete browser/native adapters with actual GPU/reference/ownership evidence; native uses explicit counted FP32 staging. Tasks 4.3–4.4 now pass tested browser WebCodecs/FFmpeg WASM and native shared-wgpu/software-codec video composition/export. The AI NVDEC/NVENC timing failure stays unavailable. Task 4.5 passes active cancellation/runtime-error/injected OOM/actual device destruction and ownership/retry checks. Tasks 6.1/6.3 pass paused browser preview and finite-history chronological parity. Task 6.2 has a tested native staged service/renderer adapter but stays open for its ordinary-launch requirement; full lifecycle/release remain open. Memory counters/samples exclude or may miss opaque runtime allocations. Ordinary conversion entry points reject On; neither platform's AI feature is release accepted.

## 2. Core policy, geometry and command compatibility

- [x] 2.1 Add pure Off-default AI settings/backend/model identity and capability results in media-core; verify legacy deserialization, invalid backend/model/scale rejection, immutable copy/snapshot behavior and no platform/runtime dependencies in core.
- [x] 2.2 Add separate checked AI geometry resolution for normalized source 2x/4x while retaining ResizeSpec's existing behavior; verify aspect ratio, even dimensions, rotated geometry, overflow, configured limits, ordinary no-upscale rejection and Off restoration in core unit tests.
- [x] 2.3 Carry AI identity through browser worker/main command transport and native NativeJob snapshots with backward-compatible defaults; extend transport tests to verify old commands remain Off, malformed settings fail and stale generations cannot publish.
- [x] 2.4 Reuse enlarged dimensions for exact profile probing and existing bitrate/FPS/output-size estimation; verify source-aware recommendations update without changing timing, unsupported exact codec/precision/route combinations reject before output creation, and document the operation/codec separation in architecture.md.

## 3. Explicit provisioning and safe model lifecycle

The user chose explicit import of the exact local ONNX export. Tasks 3.1–3.2
originally implemented cache APIs and the native cache CLI. Partial task 7.1 now
adds explicit setup UI; inference remains unavailable in ordinary app commands. Browser runtime acquisition remains a separate explicit action;
verified cached blobs/WASM pass main/worker tensor inference. Actual offline and
origin-quota errors plus native blocked-path recovery are recorded. Active native
disk-write cancellation and broad ACL/disk-full variants are not claimed. SDK/runtime
installation is still never automatic. See the latest interop continuation.

- [x] 3.1 Implement the allowlisted asset manifest/cache on browser and native for passing portable candidates, keeping optional runtime assets lazy; verify startup/AI Off performs no model download, explicit acquisition exposes size/license/source, hashes are checked before publication and valid cache works offline.
- [x] 3.2 Add provisioning failure/cancellation tests for corrupt/truncated assets, incompatible manifests, browser quota/network failure and native unwritable cache paths; verify actionable recovery, no untrusted execution, cleanup of incomplete downloads and no video uploads, and document cache acquisition/removal behavior in README.md.
- [x] 3.3 Add optional native SDK/runtime path discovery and capability reporting with no hard-coded local library path or automatic license acceptance; verify missing/wrong-architecture/version components are reported and proprietary SDK assets are absent from distributable source changes.

## 4. Bounded portable inference and conversion

- [x] 4.1 Integrate the first passing browser inference adapter into media-web's existing serialized execution context with shared GPU packing/unpacking; verify tile/frame credits, uniform/tensor lifetimes, identity and budgets with tiny actual GPU runs and separate test-only readbacks. Document any unavailable second candidate explicitly.
- [x] 4.2 Integrate passing native portable candidate(s) behind a concrete platform dispatcher with exact GPU/provider identity and explicit staging where necessary; verify child/session ownership, bounded frames and transfer accounting without passing resources between devices or wgpu ABIs. Record failed candidate gates as unsupported, not implemented.
- [x] 4.3 Compose AI before the existing CLAHE/manual transform and before FPS resampling on each supporting route; verify source frames advance history once, duplicates reuse processed results, neutral/Off matches existing processing, and unsupported effect or 10-bit/HDR combinations reject rather than being approximated.
- [x] 4.4 Add real export tests for each passing portable platform/backend using Original VFR and 15/60 fixed FPS, supported audio profiles, 2x/4x sizes and full independent re-decode; verify exact output grid/coverage and clean ownership. Include browser WebCodecs and supported fixed-FPS FFmpeg WASM paths without repeating inference inside the encoder.
- [x] 4.5 Exercise active inference cancel, OOM/budget rejection, runtime failure, actual controlled device destruction and explicit fresh-generation retry; verify partial output is unpublished and application-owned frames/tensors/leases return to zero, then record exact outcomes/copy counts and remaining limits in interop-report.md.

4.3–4.4 evidence: browser main/worker WebCodecs H.264/AAC, VP8/Opus and VP8 video-only run 2x/4x × Original/15/60; FFmpeg WASM runs both scales at supported 15/60. Native DirectML plus shared-wgpu/software codecs runs both scales × Original/15/60, CLAHE/manual controls and independent complete A/V decode. Native NVDEC/NVENC AI fails the actual Original-VFR first PTS gate (22000 us -> 0 us) and now rejects before inspection/output; it is not a passing route or automatic fallback. Tiny native pixel tests pass shared x4/2x post-scale/manual math at <=1 LSB. Task 4.5 additionally passes browser active-dispatch cancellation, real runtime validation error, injected OOM scope responses, actual device destruction, native helper crash/budget rejection, application ownership cleanup and fresh retry. Physical memory exhaustion and spontaneous driver loss remain unverified. These pilot gates do not enable UI or satisfy preview/release acceptance.

## 5. Gated Windows RTX native path

- [x] 5.1 For a passing RTX pilot, build a source-only optional native helper with a versioned bounded frame protocol and SDK-specific status mapping, preserving the Rust app's unsafe-code boundaries; verify reference sample equivalence with a pinned tolerance, exact chosen NVIDIA device, output geometry/color tags and clean helper startup/shutdown. A failed pilot stays unavailable with a tested reason.
- [x] 5.2 Connect the staged RTX adapter to existing native source-timestamp/audio/encoder policy and independent output verification; verify actual enhanced file output, 2x/4x capability checks, repeatability and counted host/device transfers without claiming NVDEC-to-RTX surface sharing.
- [x] 5.3 Gate RTX plus manual controls/CLAHE on a proven explicit bridge and keep Main 10/HDR disabled; verify supported operation order, early incompatible-combination rejection, cancel during helper pipe IO, helper crash/timeout, partial cleanup and healthy explicit retry. Record API/driver/license/deployment evidence and reproducible commands in interop-report.md/architecture.md.

## 6. Matching preview and bounded interactive lifecycle

Active-preview continuation 2026-10-25: explicit browser source/seek/AI-identity
invalidation retires only preview work; probes/conversions remain current.
Automatic playback and downstream edits retain active progress. Actual sampled
AI preview passes 12 active cancellation/runtime/loss cases and 40 successful
outputs across both scales/contexts, differing source replacement and exact
restoration. Eight independent canvas comparisons preserve displayed pixels on
cancel/runtime failure; lost-device contents are not asserted. Ordinary Off
UI seek bursts pass, as do 16 clock/history/transport tests and the eight-export
AI handoff regression. Task 6.4 remains open for ordinary AI On/coalescing/full
lifecycle acceptance; sampled command flags are not clock/UI or performance proof.

Continuation 2026-10-25: browser producer/audio-attempt identities now reject
retired completions; preview retirement failures propagate and retire the GPU
context before conversion can start. Ordinary Off main/worker UI passes rapid
Pause/Play, late/current errors, explicit retry, replacement and handoff. Actual
AI 2x/4x implicit preview/conversion/pixel restoration and injected release-response
failure/fresh-device retry pass; eight exports are independently verified. The
existing active failure suite passes ten retry exports after a retained-lost-
context regression was reproduced and fixed. Evidence preserves both initial
failures. Task 6.4 stays open for ordinary AI On/full lifecycle acceptance;
diagnostic response injection is not a real broken-runtime cleanup guarantee.

Continuation 2026-10-24: the ordinary native conversion handler now reserves
preview admission and waits off the UI thread for playback and active AI/history
cache retirement. Stale sample requests reject while reserved. Actual diagnostic
2x/4x handoff, pre-cancelled output, fresh verified AI conversion and paused pixel
restoration pass; ordinary AI Off window success/error return also pass. A newly
exposed AI cut verification failure was corrected by comparing packet endpoints
on output as well as input; 12 release cut/VFR exports and independent endpoint
checks pass. The unchanged AI Off cut/header limitation is explicitly recorded.
Task 6.4 remains open for full browser/ordinary AI On lifecycle acceptance; 6.2's
ordinary-launch AI requirement is not claimed from these tests.

Continuation 2026-10-23: native warm Before/After preserves the enhanced cache
and finite history; cold Before works without a helper. Final display fitting
and map retirement are corrected. Actual service/renderer reference checks pass
55 comparisons at <=1 LSB. The explicit `--ai-preview-check` window exercises
both scales, Before/After, cached edits/Reset color, cut seek and Play/Pause.
Actual worker tests cover 100 coalesced playback samples, invalidation/retirement,
missing-helper retry and renderer loss/fresh generation. This is a diagnostic
launch, so 6.2's ordinary-launch requirement and 6.4's full cross-platform/UI
handoff matrix remain open; no checkbox or product capability is granted.

- [x] 6.1 Extend browser preview to use the selected passing AI backend/model at selected output geometry before display scaling; verify paused preview/pre-encoder export pixel parity, Before restoration, cached downstream slider/Strength edits and preview availability when FFmpeg export is rejected.
- [ ] 6.2 Add a native preview adapter using validated compatible renderer inference or explicitly counted bounded display-image staging from the native service; verify ordinary-launch preview/export parity without registering foreign-device textures, and document the selected bridge and wgpu-26/30 ownership boundary.
- [x] 6.3 Extend exact CLAHE preview preroll through AI on supported combinations; verify source PTS, finite history and selected-size statistics against chronological conversion, including seek backward, scene cuts, scale/model changes and Before/After restoration.
- [ ] 6.4 Add real preview lifecycle regressions for play/pause, error-state Pause, seek/source replacement, latest-only coalescing, conversion handoff, device loss and explicit retry; verify bounded pending work, no stale pixels, no cancellation starvation and released ownership, recording limitations instead of claiming real-time audio sync.

## 7. Compact native and browser controls

Continuation recorded 2026-10-24: shared selection callbacks and both app command
handlers now check exact capabilities; snapshots, estimates/profile checks,
retained resize and separate Before/Reset scopes carry AI metadata. Actual shared
handler tests use test-only validated metadata; actual ordinary browser/native UI
runs remain Off with unavailable 2x/4x. Browser AI Off preview/Play-Pause continues
when export is rejected. No product capability is granted, so 7.1/7.2/7.3 remain
open pending actual ordinary On and the full acceptance matrix.

- [ ] 7.1 Add collapsed AI Upscaling controls with Off, validated backend/model/scale choices, explicit acquisition and size/status reasons; verify UI tests enforce availability in both DOM and native command handlers and browser RTX is never offered.
- [ ] 7.2 Wire conversion snapshots, retained ordinary resize settings, AI Off and independent Reset color/Before scopes; verify actual 2x selection updates dimensions/bitrate/profile checks, Off restores ordinary resize, Reset color leaves AI enabled and unsupported route settings never silently fall back.
- [ ] 7.3 Inspect normal native and browser windows in both supported themes using available UI tools; verify preview/playback, missing-assets/error states, default native no-page-scroll layout and disabled export with working preview. Update README.md with only actually supported setup, model/license acquisition, controls and limitations.

## 8. Cross-path acceptance and handoff

Continuation 2026-10-24: task 8.2 gains a three-excerpt licensed real-content
release matrix: 18 native plus 36 main/worker AI exports, both scales and all
three FPS policies, independently decoded; another 18 browser Off exports pass.
All 24 source frames pass CPU PyTorch/DirectML FP32 parity with DML-only node
placement. The initial native verification error (AAC priming packet versus
presentation origin) is preserved and independently resolved. Tiny excerpts,
visible small-text/detail limitations, diagnostic readbacks, different adapters,
incomplete component timing and full preview/lifecycle/temporal coverage keep
8.2 open. No product AI capability is granted.

- [x] 8.1 Run locked relevant core/GPU/native checks/tests/Clippy, wasm32 web checks/lints, formatting, JS syntax and ordinary non-AI regression exports; verify old settings and AI Off have unchanged behavior and no extra inference allocations/downloads. Record exact commands/results, including unavailable environments, in interop-report.md.
- [ ] 8.2 Run the supported matrix in release mode on the same licensed motion/text/face/texture/cut clips and inspect temporal artifacts; verify numeric/tile gates and preview/export/timing/audio/lifecycle results together, compare load/inference/transfers/encode/verification and total time against bilinear and matching-model runtimes, and publish measured memory/copy/quality limitations. RTX is compared as a different model, not a matching-weight parity result.
- [ ] 8.3 Audit every exposed backend against the five-cell evidence and asset/dependency license inventory; verify unsupported/untested cells remain unavailable, no unchecked tasks are represented as completed implementations, README/architecture/report agree and no browser helper/cloud or unreviewed SDK binaries were added. Leave a requested platform unaccepted if it has no passing implementation.

## Workflow follow-up

- Review planning artifacts before applying. Implementation starts only on a separate user request.
- Archive after implementation, documented evidence and review are complete; do not archive merely because the planning artifacts validate.
