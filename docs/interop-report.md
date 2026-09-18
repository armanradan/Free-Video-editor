# M1 browser interoperability report

Date: 2026-09-17  
Status: **M1 passed in the tested Microsoft Edge environment**

## Result

The M1 application completed the required compressed VP8 → WebCodecs decode → wgpu resize → WebCodecs encode → independent WebCodecs decode round trip. Five consecutive runs processed and verified all 30 frames. A cancellation run returned application-owned live frames to zero, and its immediate restart passed all 30 frames.

The conversion path performs no explicit CPU pixel readback or pixel upload. The verifier performs one `VideoFrame.copyTo` readback per output frame; those 30 diagnostic readbacks are counted separately and are not evidence about browser-internal copies or codec hardware execution.

No M2 container, audio, file-picker, native-backend, or native-UI work was started.

## Pinned stack

| Item | Exact version / setting |
|---|---|
| Rust | 1.98.1 |
| Rust edition | 2024 |
| Dioxus | 0.7.10 (`web`, `minimal`) |
| wgpu | 30.0.1 |
| wasm-bindgen | 0.2.128 |
| wasm-bindgen-futures | 0.4.78 |
| web-sys / js-sys | 0.3.105 |
| serde | 1.0.229 |
| futures-channel | 0.3.34 |
| wasm target flag | `--cfg=web_sys_unstable_apis` for WebCodecs bindings |

`Cargo.lock` records the complete resolution. wgpu 30's released `ExternalImageSource::VideoFrame` and `Queue::copy_external_image_to_texture` APIs are used; no development-only external-texture import API is used.

## Tested environment

| Item | Observed value |
|---|---|
| Browser | Microsoft Edge 153.0.4234.32, headless Chromium (`Edg/153.0.0.0`) |
| Browser flags | `--headless=new --enable-unsafe-webgpu --ignore-gpu-blocklist --use-angle=d3d11 --disable-gpu-sandbox --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows` |
| OS | Windows API reported “Windows 10 Home”, version 2009, build 26200; browser platform `Win32` |
| WebGPU adapter info | vendor `nvidia`, architecture `ampere`; device/description not disclosed |
| Secure context | `http://127.0.0.1:8081` |
| Capability probes | `navigator.gpu`, `VideoDecoder`, `VideoEncoder`, `VideoFrame`, VP8 decoder config, VP8 encoder config, and `VideoFrame(HTMLCanvasElement)` all available |
| Hardware codec use | unknown; WebCodecs acceleration is a preference, not proof |
| Browser-internal copies | unknown |

The in-app browser-control bridge was unavailable before page attachment, so validation used the installed Edge executable and Chromium's local debugging protocol against the same built bundle. Chrome was not installed and was not required. Background-throttling flags keep the automated headless page active; they are not application prerequisites for a foreground Edge tab.

## Fixture and codec configuration

The checked-in `fixtures/m1-vp8.ivf` is 12,858 bytes with SHA-256 `dcb10a78baa7b70d270a9c6364d58195639c1776681b5d5d2de7ece04ae96b9f`. `fixtures/m1-vp8.json` contains 30 packet offsets, sizes, timestamps, durations, keyframe flags, generation details, and the hash. It was deterministically generated with FFmpeg 8.0.1/libvpx from owned filter output with changing decimal frame identifiers and fixed red/green/blue/yellow orientation patches.

- Decoder: `vp8`, coded 320×180, latency optimization enabled.
- Encoder: `vp8`, 160×90, 500,000 bit/s, 30 fps, realtime latency, hardware acceleration `no-preference`.
- Timing: 30 frames, timestamp `index × 33,333 µs`, duration 33,333 µs.
- Color prototype: opaque SDR, sRGB canvas boundary, bilinear filtering of encoded channel values. Linear-light and wide-color processing remain outside M1.

## Frame path and ownership

One cached wgpu instance/device/queue is used for the browser execution context and reused across job restarts. For each frame:

1. WebCodecs emits an application-owned `VideoFrame`.
2. Rust passes that frame to `Queue::copy_external_image_to_texture`, targeting a reusable 320×180 `Rgba8Unorm` texture with `COPY_DST | TEXTURE_BINDING | RENDER_ATTACHMENT` usage.
3. `media-gpu` records a fullscreen-triangle WGSL bilinear resize directly into the current 160×90 canvas surface texture.
4. The queue submits, then immediately presents/releases the acquired surface texture.
5. A timestamped `VideoFrame` is constructed from the persistent canvas, then both the decoded source and canvas snapshot are retained through the submission-specific completion callback.
6. The output frame is submitted to `VideoEncoder` and explicitly closed. The source frame is closed by an RAII `Drop` guard on success and every Rust error path.

Decoder submission, decoded callback, encoder submission, verifier submission, and verifier callback queues are separately bounded. Codec input pumping and callback draining run concurrently. Decoder, processor, encoder, and verifier drains are ordered; generation-based cancellation rejects stale work and resets codecs.

## Canvas ordering experiments

Four no-readback sequences were evaluated:

1. `submit → VideoFrame(canvas) → present → completion`: encoded black frames.
2. `submit → completion → VideoFrame(canvas) → present`: Edge reported an uninitialized 160×90 shared image and produced uniform output.
3. `submit → completion → present → VideoFrame(canvas)`: one full run passed, but a later run stalled at frame 10 while the acquired surface texture was held through the completion wait.
4. `submit → present → VideoFrame(canvas) → completion`: five consecutive runs plus cancel/restart passed. This is the implemented order.

The current sequence gives Chromium a consumer for the committed canvas image without an unrelated asynchronous yield, while retaining both relevant frame handles until submitted GPU work completes. A wgpu validation error scope covers every frame submission. No validation error occurred in the passing runs.

The original reported marker failure (`TL=0,77,0 TR=0,77,0 BL=0,77,0`) also exposed two correctness requirements now implemented: the external-copy destination texture includes `RENDER_ATTACHMENT`, and verifier samples honor the `offset` and `stride` returned by `VideoFrame.copyTo` instead of assuming tightly packed rows.

## Measured runtime evidence

Five consecutive runs in one page/execution context:

| Run | Result | Elapsed including verification | Live-frame cleanup |
|---:|---|---:|---:|
| 1 | 30/30 passed | 1,083.8 ms | 0 |
| 2 | 30/30 passed | 561.4 ms | 0 |
| 3 | 30/30 passed | 755.9 ms | 0 |
| 4 | 30/30 passed | 210.1 ms | 0 |
| 5 | 30/30 passed | 369.2 ms | 0 |

Across those runs:

- output geometry: 160×90
- encoded and independently decoded frames: 30/30 per run
- timestamp tolerance: passed at ±1 µs; durations passed when supplied by the decoder
- orientation and representative red/green/blue marker checks: passed for every frame
- explicit external-image API copies: 30 per run
- explicit CPU pixel readbacks/uploads in conversion: 0
- test-only verifier readbacks: 30 per run
- peak application-owned live frames: 8
- peak decoder submission queue: 4
- peak decoded callback queue: 6
- peak encoder submission queue: 1 (hard bound 4)
- peak verifier submission queue: 4
- peak verifier callback queue: 6
- WebGPU validation errors: 0 observed

Elapsed time is a correctness baseline, not a throughput claim. It includes verification readback and independent decoding and is sensitive to browser warm-up and headless scheduling.

## Lifecycle validation

- Cancellation was requested after one frame had been processed. The result reported `processed=1`, `encoded=0`, bounded peaks (`live=6`, decoder=4, callbacks=5, encoder=1), and `cleanup live frames=0`.
- The immediate restart passed 30/30 frames in 367.5 ms with `cleanup live frames=0`.
- The five normal runs and cancel/restart shared one cached processing device/queue. No queue growth was observed between runs.
- Cancellation is cooperative at the bounded scheduling and callback-drain points. Already-submitted GPU work is retained safely through its completion callback rather than forcibly interrupted.

## Build verification

Verified on Rust 1.98.1 using the GNU Windows host toolchain because the installed Visual Studio instance lacks the C++ linker/Windows SDK library workload:

- `cargo check -p media-core -p media-gpu`: passed
- `cargo test -p media-core -p media-gpu`: passed (2 focused timestamp tests)
- `cargo check --workspace --target wasm32-unknown-unknown`: passed
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed
- `cargo fmt --all -- --check`: passed
- `dx build --web --locked` with Dioxus CLI 0.7.10: passed from `apps/web`

The native MSVC-host check was attempted but could not link because `link.exe` and `msvcrt.lib` were absent from the installed Visual Studio workloads. The GNU-host checks provide the requested host portability evidence without changing application behavior.

## Decision and remaining limitations

The experiment answers M1's architecture question **yes** for the pinned stack and tested Edge environment: compressed frames can be decoded, copied through the released wgpu VideoFrame API, resized by shared WGSL, captured from the WebGPU canvas, encoded, and independently verified without application-managed CPU pixel transfer.

This result does not prove zero internal copies, hardware codec execution, support in non-Chromium browsers, wide-color correctness, worker operation, long-run memory stability, or native interoperability. Those remained explicitly untested and outside M1 when this M1 section was recorded.

---

# M2 browser converter interoperability report

Date: 2026-09-18
Status: **M2 passed for the tested MP4/H.264 → video-only WebM/VP8 path in Microsoft Edge**

## Verified behavior

The Dioxus application accepted the checked-in `fixtures/m2-h264-aac.mp4`, demuxed its MP4 container, decoded all 60 H.264 frames in presentation order, resized every frame from 640×360 to 320×180 through the shared wgpu shader, encoded VP8, finalized a downloadable WebM, and explicitly omitted its AAC track. The finalized file was 62,512 bytes.

The application reopened the finalized WebM, verified VP8 codec, 320×180 geometry, 60 packets for 60 decoded input frames, two-second duration within one-frame precision, and a successful midpoint decode. Edge's independent HTML video element then loaded the object URL, reported 320×180 and 2.000 seconds, and completed a seek to 1.000 second. Finally, FFmpeg 8.0.1/ffprobe independently reported one VP8 video stream, no audio stream, 60 decoded frames, 30 fps, 320×180, and 2.000 seconds; `ffmpeg -ss 1` decoded a frame successfully.

Cancellation during conversion reported `CANCELLED: conversion stopped by user`. An immediate restart processed all 60 frames and produced the verified output. The M1 deterministic probe was then run in the same page and on the same cached wgpu device after dynamic reconfiguration; it again passed 30/30 frames with zero application-owned live frames after cleanup.

An additional ignored test clip derived from the user-provided recording also passed: 60 H.264 frames at 640×360 with one AAC track became a 20,488-byte, 320×180 video-only WebM with 60 frames and a 2.000-second player/ffprobe duration. The original 376,836,980-byte recording is intentionally rejected by the 256 MiB M2 limit and was not modified.

## Selected path and versions

| Item | Exact value |
|---|---|
| Input | ISO MP4, primary H.264/AVC video track, unrotated/unflipped |
| Output | WebM, VP8 video only, two-second keyframe interval |
| Resize | 50% width and height, rounded down to even codec dimensions |
| Container implementation | Mediabunny 1.58.0, pinned by `package-lock.json` |
| Browser input cache | Mediabunny `BlobSource`, 8 MiB maximum cache |
| Application input limit | 256 MiB |
| Output target | `BufferTarget`; complete compressed output retained in memory until download |
| Rust/Dioxus/wgpu | Rust 1.98.1, edition 2024, Dioxus 0.7.10, wgpu 30.0.1 |

Mediabunny supplies the proven MP4 demux/WebM mux integration, exact decoder initialization metadata, decode-order handling for H.264 B-frames, keyframe scheduling, and output finalization. Its integer microsecond sample accessors are used at the WebCodecs boundary. The first input presentation timestamp is normalized to zero; positive sample durations are preserved, and only missing/zero durations use the measured average packet interval. WebM metadata duration and decoded sample coverage must agree within one frame.

The frame path is sequentially backpressured: one decoded `VideoSample` is converted to a `VideoFrame`, consumed and closed by Rust/wgpu, wrapped as one output sample, and awaited by the encoder/mux source before the next sample is accepted. Mediabunny may predecode a small internal window; its exact internal high-water mark was not exposed. No raw frame or GPU resource enters Dioxus reactive state.

## Fixture provenance

`fixtures/m2-h264-aac.mp4` is 279,635 bytes with SHA-256 `5a7802b764ad928056d2777e2d25a86ec7fdb2c69696d70258b3ae5702fcb54b`. FFmpeg 8.0.1 generated its 60-frame H.264 video from `testsrc2` and its AAC mono audio from an 880 Hz `sine` source. It contains no third-party media and is CC0-1.0. `fixtures/m2-h264-aac.json` records the complete command and stream configuration.

## Tested environment and measured run

| Item | Observed value |
|---|---|
| Browser | Microsoft Edge 153.0.4234.32, headless Chromium (`Edg/153.0.0.0`) |
| OS | Windows browser platform `Win32` / Windows 10-compatible user agent |
| WebGPU adapter shown by app | `intel / gen-12lp (BrowserWebGpu)` |
| Local origin | `http://127.0.0.1:8080` |
| Fixture conversion time | 390.2 ms for the recorded fixture run; correctness baseline only |
| Output | 62,512 bytes, 60 frames, 320×180, 2.000 seconds |
| Output streams | one VP8 video stream; zero audio streams |
| Cancellation/restart | passed |
| Midpoint HTML video seek | passed at 1.000 second |
| Independent ffprobe/frame decode | passed |
| M1 regression after M2 | passed 30/30; cleanup live frames=0 |

The conversion path adds no `VideoFrame.copyTo`, canvas `getImageData`, mapped GPU readback, or CPU pixel upload. There is one explicit WebGPU external-image copy per converted frame. This does not establish zero browser-internal copies or hardware codec execution.

## Build verification

Verified with the Rust 1.98.1 GNU Windows host toolchain:

- `cargo test -p media-core -p media-gpu`: passed; four focused `media-core` tests, including even half-size geometry and the browser input-size policy.
- `cargo check --workspace --target wasm32-unknown-unknown`: passed.
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `dx build --web --locked`: passed with Dioxus CLI 0.7.10 and the bundled Mediabunny asset.

## Explicitly untested or unsupported

- Input other than MP4/H.264 and output other than WebM/VP8 are rejected or not offered.
- Audio is not copied or transcoded; it is detected, reported, and omitted.
- Files larger than 256 MiB are rejected because compressed output is still buffered in memory. Streaming I/O is not implemented.
- Rotation and horizontal-flip metadata are rejected. HDR, crop, non-square pixels, resolution changes, and defined wide-color processing remain untested.
- Firefox, Safari, non-Windows Chromium, worker WebGPU, long-run memory behavior, and device-loss recovery remain untested.
- Browser-internal copies and actual codec hardware acceleration remain unknown.
- The native backend and exact native GPU selection remain M4/M5 work. No M3 work was started.
