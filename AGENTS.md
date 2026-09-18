# Repository instructions

## Purpose and current phase

Build a Rust GPU-accelerated video converter with Dioxus Web initially, Dioxus Native/Blitz eventually, shared media-core and wgpu processing, browser WebCodecs, and a later native codec backend.

Read `docs/architecture.md` before implementation. The current authorized design milestone is M1: a tiny browser decode → wgpu resize → encode round trip. Implement only the milestone requested by the user. Future roadmap phases are not instructions to build everything now.

## Architecture boundaries

- `media-core` owns platform-neutral media types and job policy. No Dioxus, wgpu, web-sys, native codec FFI, DOM, filesystem, or mandatory async runtime dependencies.
- `media-gpu` owns shared wgpu processing, WGSL, device identity, and GPU resource lifetimes. No browser codec or UI types.
- `media-web` owns WebCodecs and browser frame ingress/egress. Keep necessary JavaScript glue small and local to this boundary.
- `media-native`, when added, owns native codecs and platform surface interoperability.
- Dioxus components send commands and display metadata/status. Do not put raw frames, GPU resources, or per-frame processing in reactive UI state.
- Share processing code and transform math. Do not duplicate the processor in JavaScript or native-only code.
- Use Dioxus Native/Blitz for the eventual native renderer requirement; `dioxus-desktop` is a webview renderer.
- Prefer concrete implementations until a real second implementation or test motivates an abstraction. Do not create deferred crates as empty scaffolding.

## GPU and frame rules

- Use one processing device/queue per execution context. Never assume resources from different devices are interchangeable.
- Baseline browser ingress uses the pinned wgpu API for copying a VideoFrame into a texture. Direct external import is an optimization only when the selected version and runtime support it.
- Never infer released API support from development documentation or confuse plane-based external-texture creation with browser VideoFrame import.
- Minimize explicit GPU↔CPU frame transfers. Do not silently add `VideoFrame.copyTo`, `getImageData`, mapped GPU readback, or CPU pixel upload loops to the fast path.
- Diagnostic readbacks belong to a clearly separate verification path and must be excluded from conversion throughput metrics.
- Describe evidence accurately: no explicit CPU pixel readback does not prove zero internal copies or hardware codec execution.
- Own and explicitly close every VideoFrame. Provide a Drop safety net. Retain frame/import guards for their required GPU lifetime and close resources on all error/cancel paths.
- Do not reuse pooled textures before GPU and downstream consumers release them. Carry device generation through resource ownership.
- Keep canvas render/submit/capture ordering explicit. No unrelated asynchronous yield before capturing the current output image.

## Scheduling and metadata

- Browser futures and handles may be non-Send. Do not add unsafe Send/Sync implementations or force Tokio/threading into shared code.
- Drain codec output concurrently with input. Do not assume one packet produces exactly one frame immediately.
- Bound decoder submissions, callback queues, encoder submissions, and retained frames. Codec queue counters alone are insufficient.
- Preserve all frames during conversion; preview is independently rate-limited.
- Preserve integer timestamps, durations, geometry, orientation, and color metadata. Use checked time-base conversion.
- End-of-stream drains decoder, processing, and encoder in order while consuming callbacks, then finalizes output.
- Cancellation stops input, ignores stale job callbacks, cleans resources, and handles already-submitted GPU work safely.

## Dependencies and verification

- Pin a compatible toolchain and dependency set; commit Cargo.lock once the application is scaffolded. Keep platform dependencies target-specific.
- Verify library APIs against the selected version. Record any experimental flags or patches and why they are necessary.
- For Rust changes, run formatting and relevant checks/tests; check shared core/GPU crates on the host and browser crates on wasm32. Use matching-target lint commands once scaffolded.
- Add meaningful tests for timestamp conversion, lifecycle, bounded scheduling, and image correctness as implemented. Do not add tests that merely restate constants or mirror implementation.
- Browser interoperability requires a real browser test using decoded frames and re-decoding the encoder output. Mock tests and compilation alone do not prove it.
- Record browser/OS/GPU where available, versions, codec config, frame counts, queue peaks, known copies, and actual results in `docs/interop-report.md`.
- If a browser or GPU is unavailable, complete buildable work and clearly mark runtime validation pending. Do not report M1 as proven.
- Use tiny reproducible fixtures with provenance/license and a timestamp/config manifest. Never add large media files or downloaded private assets.

## Completion reports

State what changed, what was actually tested, and remaining limitations. If interop fails, record the failing boundary and evidence before broadening scope. Keep this file and architecture decisions aligned with validated findings.
