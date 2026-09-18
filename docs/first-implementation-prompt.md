# First implementation prompt

Implement milestone M1 from `docs/architecture.md`. Read the repository's `AGENTS.md` first.

Build the smallest real browser experiment that proves:

```text
compressed fixture → WebCodecs VideoDecoder → VideoFrame
→ wgpu texture copy → shared WGSL resize → WebGPU canvas
→ VideoFrame(canvas) → WebCodecs VideoEncoder
→ compressed output → verification decoder
```

## Scope

- Create a Rust workspace containing only `apps/web` (`converter-web`), `crates/media-core`, `crates/media-gpu`, and `crates/media-web`.
- Use Dioxus Web for a small page with Run, Cancel, a canvas, and status/results. Use one processing device and queue.
- Pin a compatible released Rust/Dioxus/wgpu/wasm-bindgen/web-sys dependency set and commit Cargo.lock. Verify the exact APIs/features against that selection.
- Target one available Chromium-based browser, one supported codec (start with VP8), opaque SDR, 320×180 input resized to 160×90, and 30 timestamped frames.
- Use a tiny checked-in compressed fixture plus packet/configuration manifest and documented generation/provenance. Include changing frame identifiers, color patches, and orientation markers. Do not replace the decoder with synthetic raw frames in the actual test.
- No container demux/mux, arbitrary file selection, audio, native backend, native UI, worker migration, render graph, or broad settings interface in this milestone.

## Implementation requirements

1. Probe secure context, WebGPU, the selected WebCodecs decoder/encoder configurations, and required canvas/frame capabilities. Show precise unsupported reasons.
2. Start with the released wgpu `copy_external_image_to_texture` VideoFrame path. Do not assume `create_external_texture` imports browser frames. Do not use development-only APIs without explicitly recording and justifying a dependency decision.
3. Keep resize pipeline and WGSL in `media-gpu`, independent of DOM/WebCodecs/Dioxus. Render directly into the supplied canvas target for M1.
4. Prove a single frame through encoding early, then expand to 30 frames. Do not stop after previewing decoded video.
5. Capture the processed canvas while its submitted image is current, before an unrelated asynchronous yield or surface invalidation. Validate and document the render/submit/capture/present sequence for the pinned wgpu API.
6. Allow one processing frame in flight initially; bound codec input and callback queues separately. Pump inputs and drain outputs concurrently so codec buffering cannot deadlock the pipeline. Flush at end-of-stream while continuing to drain callbacks.
7. Explicitly own/close all VideoFrames and retain GPU/import resources until safe. Close the application's output frame after successful encode submission. Add cancellation and restart cleanup, including stale callback protection.
8. Do not use CPU pixel readback/upload in the conversion path. Compressed packet copies and uniform uploads are allowed. If a required bridge cannot work without readback, document the blocker instead of silently adding a fallback and calling the fast path successful.
9. Re-decode encoded output with the returned codec configuration. Validate decoded frame count, 160×90 dimensions, timestamps/durations, changing markers, orientation, and representative colors with documented lossy-codec tolerances. Test-only pixel readback is allowed here, separately counted and excluded from conversion timing.
10. Prefer concrete M1 adapters. The architecture's future trait sketches are guidance, not a requirement to build a general media framework.

## Acceptance and deliverables

- All 30 frames complete a real browser decode/resize/encode/re-decode run with correct timing and markers, no stale/black output, and no WebGPU validation errors.
- Five repeated runs plus cancel/restart finish with zero application-owned live frames and bounded queues after cleanup.
- Report explicit copy counters, peak retained-frame/queue counts, and measured timing. Label browser-internal copies and actual codec hardware use as unknown unless evidence supports a narrower statement.
- Check `media-core` and `media-gpu` on the host; check the web application for `wasm32-unknown-unknown`; run formatting and relevant tests/lints. Add focused tests only where they verify meaningful behavior.
- Provide a short README with exact build/run commands and prerequisites.
- Write `docs/interop-report.md` containing exact dependency versions/features, browser/OS/GPU where available, codec configuration, frame path, lifecycle/capture ordering, verification results, and unresolved limitations.
- If direct external import is supported by the selected release, it is an optional bounded comparison only after the baseline succeeds.
- If runtime browser/GPU testing is unavailable, finish the scaffold/checks and runnable probe, then clearly report runtime validation pending. Compilation is not proof that M1 passed.

Finish with a concise summary of implemented behavior, actual validation, and the decision supported by the experiment. Do not proceed to M2.
