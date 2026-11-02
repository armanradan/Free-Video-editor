# Tasks

## 1. Prove the narrow AAC normalization path

- [x] 1.1 Add a tiny reproducible synthetic AAC leading-PTS anomaly and valid priming/offset/gap controls with provenance and exact packet manifests; verify the anomaly reproduces the original failure without committing private media.
- [x] 1.2 Evaluate audio-only stream-copy preparation changing packet zero PTS only; verify every payload hash, DTS, duration and later PTS against the original, record exact FFmpeg arguments/version and confirm no original bytes changed. Record a blocker if mux semantics fail rather than widening the correction.
- [x] 1.3 Independently decode prepared audio and compare sample content/alignment with a known correct synthetic reference; verify valid negative priming, offsets, edit-list timing and gaps survive the proposed path and record results in a compatibility evidence report.

## 2. Implement automatic source-bound audio preparation

- [x] 2.1 Replace prefix-only AAC rejection with a bounded complete packet classifier and checked immutable timing decision; test the supported anomaly, valid controls, missing/overflowing timestamps and ambiguous/repeated/midstream faults.
- [x] 2.2 Add owned audio-only preparation with exact packet verification, disk/size admission, cancellation and unique scratch ownership; test exhaustion, subprocess failure, cleanup and source replacement without modifying user files.
- [x] 2.3 Bind the normalized metadata/prepared owner to InspectionCache source identity and revalidation; test cache reuse, changed-file invalidation and delayed-consumer retirement, and document policy/limits in architecture and the compatibility evidence report.
- [x] 2.4 Use the shared native audio plan in direct CPU/NVIDIA and shared software/NVIDIA exports; run synthetic anomaly and valid-control exports through supported routes and independently verify sample alignment, complete video timeline and A/V endpoints under unchanged tolerances.
- [x] 2.5 Use the same source-bound audio plan in preview/playback with correct seek origins; test initial playback, forward/backward seeks, terminal coverage and cancellation against the known reference and document any measured preview limits.
- [x] 2.6 Open the original private AAC file directly in the normal native UI and complete an RTX export without a manual derivative; independently verify full decoded output/timing and original-file identity, record redacted results and observed preparation cost.

## 3. Accept legacy container metadata and codec families

- [x] 3.1 Generate a small progressive MPEG-PS/MPEG-1/MP2 fixture with absent frame-count metadata and nonzero stream offsets; verify its decoded packet/frame manifest and record license/provenance.
- [x] 3.2 Extend the existing decoded timestamp scan to derive exact frame count and endpoints when metadata is absent while retaining declared-count mismatch checks; test rational PTS, VFR, malformed EOF, cancellation and bounded metadata ownership.
- [x] 3.3 Verify original-frame preservation and audio coverage through direct software conversion of the synthetic MPEG-PS input; independently decode output and document the accepted metadata/color/timing envelope and exact unsupported boundaries.
- [x] 3.4 Build or obtain tiny licensed fixtures covering MPEG-2/MP2 and ASF WMV1/2/3/VC-1 with WMA1/2/Pro, alongside existing MP4 H.264/HEVC controls; record installed decoder capabilities, exact codec/profile/color/timing manifests and any blocked fixture or decoder instead of assuming support.
- [x] 3.5 Implement ASF/WMV inspection, count/endpoint derivation and legacy audio/color handling using decoded evidence, including the approved embedded Windows Media Foundation WMA1/2/Pro path with bounded verified source-bound PCM preparation shared by playback/export, and the approved reported final-duration fallback only when the declared rate matches the full timestamp grid; test VFR, offsets, absent metadata, negative priming where present and explicit encrypted/interlaced/profile rejection, and document the accepted envelope.

## 4. Normalize legacy pixel aspect for shared GPU and RTX

- [x] 4.1 Implement checked rational display-geometry resolution with nearest-even width and explicit tie/rounding policy; test SAR178:163 at352x288 resolves384x288, overflow/invalid SAR rejection and unchanged square-pixel behavior, and document the rounding rule.
- [x] 4.2 Apply the same pre-AI normalization in conversion and preview/history ingress while preserving timing and color policy; verify structured-aspect fixture pixels and selected-resolution preview/export parity, with explicit staging/copy accounting.
- [x] 4.3 Admit actual normalized geometry through unchanged RTX scale/memory gates independently of decoder choice; test allowed/rejected geometry and software-decoded MPEG/WMV ingress without granting unverified hardware decode support.
- [x] 4.4 Export the synthetic and original MPG files through the accepted shared software-codec RTX route; independently verify all decoded frames, audio alignment/endpoints, display aspect and image structure, and record device/runtime/route limits in the interop report.

## 5. Separate decoder choice from NVIDIA encoding

- [x] 5.1 Introduce independent native decode/encode planning and selected-device decoder capability checks before partial creation; test software/software, NVDEC/NVENC and software/NVENC plans for all target input families, retaining explicit errors for absent NVENC and unexpected decoder failures.
- [x] 5.2 Implement software-decode/NVENC in direct and shared NVIDIA routes, replacing codec-name-only admission after runtime verification; verify actual encoder identity, timing/B-frame policy, GPU selection and transfer counters, plus failure/cancel cleanup without software-encoder substitution.
- [x] 5.3 Run independent complete decode/timeline/audio/image checks for MP4 H.264/HEVC, MPEG-1/2 and WMV1/2/3/VC-1/WMA fixtures through software and NVIDIA route families, including shared RTX at accepted dimensions and Original/fixed FPS. Record decoder/encoder/device/copies and unresolved profiles; do not mark the required family matrix complete while blocked.
- [x] 5.4 Export the original MPG through NVIDIA encoding with software decoding and accepted RTX, and exercise hybrid failure/fresh retry; independently verify image aspect, all frames/audio endpoints and actual NVENC identity, and update architecture/interop documentation with the new route semantics.

## 6. Native UI and integrated acceptance

- [ ] 6.1 Extend native input Browse to MP4/MPG/MPEG/WMV and show concise timing/geometry normalization, actual decoder/encoder mode and precise capability errors; verify ordinary-window selection/preview/conversion for all target families through both route choices while output remains MP4 and AI/device consent policy remains unchanged.
- [x] 6.2 Exercise source replacement, cancellation during inspection/preparation/export, failure cleanup and healthy retry across preview-to-conversion handoff; verify no stale prepared audio, orphan child process or leaked owned scratch survives and record evidence.
- [x] 6.3 Run formatting, relevant native tests and matching-target lint plus shared/build checks affected by final changes; validate this change strictly, document actual original-file results/remaining limitations and confirm ai-upscale progress was not altered by this change.

## Workflow follow-up

- Review completed evidence and synchronize/archive this change separately when requested; keep ai-upscale open until its own remaining acceptance work is complete.
