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

- Input other than MP4/H.264 and output other than WebM/VP8 are rejected or not offered. A capability-driven output-profile selector, including additional MP4/WebM profiles, is now an explicit M3 deliverable.
- Audio is not copied or transcoded; it is detected, reported, and omitted.
- Files larger than 256 MiB are rejected because compressed output is still buffered in memory. Streaming I/O is not implemented.
- Rotation and horizontal-flip metadata are rejected. HDR, crop, non-square pixels, resolution changes, and defined wide-color processing remain untested.
- Firefox, Safari, non-Windows Chromium, worker WebGPU, long-run memory behavior, and device-loss recovery remain untested.
- Browser-internal copies and actual codec hardware acceleration remain unknown.
- The native backend and exact native GPU selection remain M4/M5 work. At the time this M2 result was recorded, no M3 work had started; the following section supersedes that milestone-status statement.

---

# M3.1 output-profile and audio interoperability report

Date: 2026-09-18
Status: **M3.1 functional acceptance passed in Microsoft Edge; M3.2 was not started**

## Verified behavior

The preferred profile converted the deterministic `fixtures/m2-h264-aac.mp4` into a 97,168-byte WebM containing all 60 resized VP8 frames and an Opus track. The browser pipeline decoded 95 AAC audio samples, encoded 102 Opus packets, and finalized 2.040 seconds of output. Its independent reopen check verified 320×180 VP8, mono Opus at 48 kHz, midpoint video seeking, audio decoding near the beginning/middle/end, and non-silent decoded audio with peak amplitude 0.0424. One shared integer-microsecond origin preserved the input tracks' relative start offsets; audio and video were pumped concurrently and finalized together.

Edge's HTML media element loaded the result, reported 320×180 and 2.040 seconds, and completed a seek to 1.020 seconds. FFprobe 8.0.1 independently reported one VP8 stream and one mono 48 kHz Opus stream. FFmpeg decoded 97,608 audio samples and measured mean volume -21.1 dB and peak volume -17.6 dB. A seeked decode at 1.5 seconds succeeded for both streams.

The explicit video-only profile still produced 60 VP8 frames, no audio stream, 320×180 geometry, and 2.000 seconds of seekable output. Cancellation during each profile returned `CANCELLED: conversion stopped by user`; immediate restart passed. The M1 deterministic probe passed 30/30 after both profile runs with zero application-owned live frames after cleanup.

The ignored user test file `tmp/user-test/Input.mp4` also passed without modification. Its 3,530 H.264 frames and 6,343 decoded 44.1 kHz stereo AAC samples became a 32,138,440-byte, 960×540 VP8/48 kHz stereo Opus WebM in 29.066 seconds. The output contained 7,365 Opus packets, lasted 147.300 seconds, sought to 73.650 seconds in Edge, and produced non-silent decoded audio at all three verification points (peak 0.2075). FFprobe reported both streams starting at 0.000 seconds; FFmpeg decoded 14,140,176 stereo samples with mean volume -9.2 dB and peak 0.0 dB.

## Contracts, policy, and lifecycle

- `media-core` now defines reusable output-profile IDs, explicit audio policies, the Opus codec choice, and capability results that retain exact unsupported reasons. The preferred profile preserves audio; the video-only profile is always an explicit user choice rather than a silent fallback.
- `media-web` invokes one exercised, non-`Send` local `BrowserConversionBackend` implementation. At the JavaScript boundary, `WebCodecsMediabunnyBackend` composes a concrete MP4 input adapter, WebM output adapter, and the job orchestrator. No codec frames or GPU resources enter Dioxus state.
- The Opus profile probes both the exact VP8 output geometry/frame rate and the exact Opus channel count/48 kHz encoder configuration. Missing audio, undecodable input audio, and unsupported VP8 or Opus configurations fail with stage-specific reasons; audio failures point to the video-only profile.
- Audio resampling to 48 kHz occurs inside the concrete Mediabunny/WebCodecs adapter. Each audio sample is timestamped from the same origin as video, awaited through encoder/mux backpressure, and closed in `finally`. The application-owned audio submission high-water mark is one; Mediabunny's internal decoder prefetch depth is not exposed and remains unknown.
- Video and audio pumps stop as siblings on cancellation or failure. End-of-stream waits for both pumps, then finalizes the muxer. A failed/cancelled muxer is cancelled, and the input is disposed on every path.
- Conversion still performs zero explicit CPU pixel readbacks. The three-point audio amplitude read is a post-finalization verification operation and is excluded from conversion timing/copy claims.

## Tested environment and versions

| Item | Observed value |
|---|---|
| Browser | Microsoft Edge 153.0.4234.32, headless Chromium (`Edg/153.0.0.0`) |
| Browser control | Edge local debugging protocol; the in-app browser native bridge was unavailable |
| OS | Windows Chromium platform `Win32` |
| WebGPU adapter shown by app | `intel / gen-12lp (BrowserWebGpu)` |
| Application origin | `http://127.0.0.1:8080` |
| Rust / edition | Rust 1.98.1 / edition 2024 |
| Dioxus / wgpu / Mediabunny | 0.7.10 / 30.0.1 / 1.58.0 |
| Codec acceleration preference | `no-preference`; the M3.4 comparison has not started |

## Build verification

Verified with the Rust 1.98.1 GNU Windows host toolchain:

- `cargo test -p media-core -p media-gpu`: passed; six `media-core` tests, including preferred-profile/audio policy and exact capability-reason coverage.
- `cargo check --workspace --target wasm32-unknown-unknown`: passed.
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `dx build --web --locked`: passed with Dioxus CLI 0.7.10.

## Known findings and explicitly untested behavior

- FFmpeg 8.0.1 emits one `Error parsing Opus packet header` diagnostic when opening each Mediabunny-produced Opus WebM, but then decodes the complete audio track and reports the expected duration and non-silent signal. Edge and Mediabunny decode/seek checks pass. This cross-implementation warning remains an open compatibility finding for M3.2/M3.6 investigation; it is not being presented as clean FFmpeg interoperability.
- Automated checks establish the presence of a decodable, non-silent signal and HTML media-element loading/seeking. Human acoustic listening and speaker output were not exercised in the headless browser.
- The application bounds its own audio submissions at one. Mediabunny's internal callback/prefetch high-water marks are not observable, so full audio queue telemetry remains M3.4.
- Only primary audio is preserved. Multiple audio tracks, language/disposition metadata, audio edits, channel layouts beyond the tested mono/stereo inputs, and passthrough were not tested.
- Firefox, Safari, non-Windows Chromium, worker execution, hardware-codec proof, long-run memory profiling, device loss, HDR, rotation/flip, and streaming output remain untested or scheduled for later M3 slices.
- The VS Code embedded browser is an Electron webview rather than a standalone browser. It has been reported to reject an otherwise working preserve-audio codec configuration; the app now identifies this environment and reports the failing pipeline stage with instructions to open the local URL in a standalone browser with WebCodecs and WebGPU support.
- At the time M3.1 was recorded, MP4 output had not been added. The following M3.2 section supersedes that milestone-status statement; arbitrary codec/container mixing remains intentionally unsupported.

---

# M3.2 output-container interoperability report

Date: 2026-09-18
Status: **M3.2 passed for capability-gated MP4/H.264/AAC in the tested browser; M3.3 was not started**

## Verified behavior

Selecting the deterministic `fixtures/m2-h264-aac.mp4` enabled the MP4 profile only after exact H.264 and AAC encoder probes passed for the computed 320×180 output, measured frame rate, 48 kHz sample rate, and source channel count. Conversion produced a 157,659-byte fast-start MP4 containing all 60 H.264 frames and 95 AAC packets. The application reopened the result, verified H.264/AAC codecs, geometry, packet counts, timing, midpoint video seek, and non-silent audio near the beginning, midpoint, and end. Edge's HTML media element reported 320×180, 2.026667 seconds, and completed a seek to 1.013333 seconds.

FFprobe 8.0.1 independently reported H.264 High profile (`avc1`), 320×180 `yuv420p`, exactly 60 frames, AAC-LC mono at 48 kHz, exactly 95 packets, and 2.026667 seconds of container duration. FFmpeg decoded 97,280 audio samples with mean volume -21.1 dB and peak -17.6 dB. Independent beginning/mid/end seeked decodes completed without errors.

The ignored `tmp/user-test/Input.mp4` exercised the longer stereo/resampling path. Its 3,530 H.264 frames and 6,343 decoded 44.1 kHz stereo AAC samples became a 75,590,653-byte, 960×540 MP4 in 26.645 seconds. FFprobe reported all 3,530 H.264 High-profile frames, 6,904 AAC-LC packets at 48 kHz stereo, both streams starting at 0.000 seconds, video duration 147.230417 seconds, audio/container duration 147.285333 seconds, and no dropped video frames. FFmpeg decoded 14,139,392 non-silent stereo samples.

Both existing WebM profiles were rerun through the generalized adapter. WebM/VP8/Opus produced all 60 frames plus 102 Opus packets and 2.040 seconds; video-only WebM produced all 60 frames, no audio, and 2.000 seconds. Each profile passed cancellation/restart and the M1 30/30 regression with zero application-owned live frames after cleanup.

## Capability and UI behavior

- `media-core` binds each profile to a container, video codec, and audio policy: WebM/VP8/Opus, WebM/VP8/video-only, or MP4/H.264/AAC. The UI does not expose arbitrary invalid codec/container combinations.
- File selection invokes the concrete browser backend's exact capability probes. MP4 remains disabled while unprobed or unsupported, and displays the returned reason. A video-only MP4 input was tested: MP4 stayed disabled with `MP4/H.264/AAC requires an input audio track.`
- Conversion repeats the exact probe before allocating the output, so stale UI capability state cannot start an unsupported profile. There is no silent substitution to WebM or another codec.
- MP4 uses Mediabunny's in-memory fast-start mode. The existing 256 MiB input limit and complete in-memory compressed output remain explicit until M3.6.

## Tested environment and measurements

| Item | Observed value |
|---|---|
| Browser | Microsoft Edge 153.0.4234.32, headless Chromium (`Edg/153.0.0.0`) |
| Browser control | Local Chromium debugging protocol; in-app bridge unavailable |
| OS / WebGPU adapter | Windows `Win32`; `intel / gen-12lp (BrowserWebGpu)` |
| Local origin | `http://127.0.0.1:8083` |
| Deterministic MP4 elapsed | 1,323.8 ms including final verification |
| Long stereo MP4 elapsed | 26,644.7 ms including final verification |
| Codec acceleration preference | `no-preference`; actual hardware codec execution remains unknown |
| Versions | Rust 1.98.1, edition 2024, Dioxus 0.7.10, wgpu 30.0.1, Mediabunny 1.58.0 |

## Build verification

- `cargo test -p media-core -p media-gpu`: passed; six focused `media-core` tests, now including the MP4/H.264/AAC profile binding.
- `cargo check --workspace --target wasm32-unknown-unknown`: passed.
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `dx build --web --locked`: passed with Dioxus CLI 0.7.10.

## Explicitly untested or deferred

- H.264/AAC encode support is configuration- and browser-dependent. Only profiles whose exact probes pass are enabled; this report does not claim universal browser support.
- Automated signal checks establish decodable non-silent audio, but human acoustic listening was not performed in the headless run.
- Multiple audio tracks, language/disposition metadata, input codecs other than H.264/AAC, HDR, orientation/flip, and streaming output remain unsupported or untested.
- Worker execution, main-thread responsiveness measurements, compatibility fallback behavior, and worker WebGPU/OffscreenCanvas probes are M3.3. No M3.3 implementation was started.

## Firefox ingress correction — 2026-09-19

Verified with installed Firefox 156.0 (build 20260909172920), Windows, a separate automation profile and WebDriver BiDi. Headless Firefox exposed navigator.gpu but returned null for the WebGPU canvas context; the successful run used normal rendering mode with no WebGPU preference overrides.

Firefox rejected VideoFrame as the source of copyExternalImageToTexture. The old wgpu wrapper could panic on that JavaScript exception, leaving the processing future and UI waiting indefinitely, including after Cancel. The frame ingress now uses the released wgpu 30.0.1 queue/texture `as_webgpu` handles to catch the exception at the browser boundary on the same device. A TypeError triggers createImageBitmap(frame) followed by the same texture copy. Other errors propagate. The bitmap is retained with the decoded frame until GPU completion and closed by a Rust Drop guard. Shader processing and immediate render/present/capture ordering remain unchanged. There is no application pixel readback; each fallback frame adds one browser-managed bitmap conversion, explicitly reported in job results. Its internal copies and hardware execution are unknown.

The deterministic H.264/AAC input completed with 60/60 VP8 frames, 95 decoded audio samples, 102 Opus packets, 320×180 geometry, 2.034 seconds, and 102,243 bytes. The recorded restart took 6,183 ms including verification; it used 60 bitmap compatibility conversions. Cancellation returned CANCELLED, and immediate restart succeeded. The HTML video element loaded the output and sought to 1.0169995 seconds. M1 passed all 30 frame timestamp, orientation, and RGB-marker checks, with zero application-owned live frames at cleanup. FFprobe independently decoded 60 VP8 frames and 102 audio frames at 48 kHz mono; the previously documented Opus packet-header warning remains.

MP4 stayed disabled with the measured reason: AAC encoding unsupported at 48000 Hz with one channel. No codec/container substitution or browser-vendor block was added.

Rust formatting, host core/GPU tests, wasm check/Clippy and locked Dioxus web build passed. A new Chromium regression browser launch was blocked by the execution policy, so the modified direct ingress path has not been revalidated there in this correction. Longer Firefox inputs, other Firefox versions/platforms, and arbitrary driver/device-loss stalls remain untested.

---

# M3.3 dedicated worker interoperability report

Date: 2026-09-19
Initial status at the time of this M3.3 run: **Firefox worker and main-thread fallback passed every profile enabled there; Chromium/MP4 validation was pending.** The completion section below closes that gap; the later M3.4 section records subsequent work.

## Implementation

Demux, WebCodecs, the existing Rust/wgpu processor and output verification now execute in one dedicated module worker by default. The worker initializes the same wasm bundle emitted by `dx`; it never launches Dioxus. Its transferred OffscreenCanvas is also the preview surface. Window/worker commands carry File inputs and metadata; the only media returned is the finalized compressed Blob. No raw frames or GPU resources enter reactive UI state. The main window owns/revokes download URLs.

Startup probes secure context, worker video/audio codec APIs, WebGPU, OffscreenCanvas, the real wgpu device/surface, and canvas VideoFrame capture. Exact output-profile probes run in the selected context. An unsupported/failed startup or 30-second startup timeout selects the original main-thread pipeline with a visible reason. `?execution=main` explicitly requests that compatibility path. There is no silent mid-job fallback or codec substitution. Tagged, serialized commands support cancellation, stale-progress rejection, failed-worker teardown and retry. Rust now also waits for submitted GPU work before releasing ingress guards when canvas capture fails.

## Real-browser environment and configuration

| Item | Observed/requested value |
|---|---|
| Browser | Firefox 156.0, build 20260909172920, normal rendering mode, isolated automation profile |
| OS | Windows, browser platform version 10.0 |
| Automation | Local WebDriver BiDi; in-app browser setup failed (`failed to write kernel assets`) |
| GPU | `identity redacted by browser (BrowserWebGpu)` in both contexts; no exact adapter identification claimed |
| Origin | `http://127.0.0.1:8084/`; fallback repeated with `?execution=main` |
| Versions | Rust 1.98.1 GNU, edition 2024, Dioxus/CLI 0.7.10, wgpu 30.0.1, Mediabunny 1.58.0 |
| Input | Deterministic `fixtures/m2-h264-aac.mp4`, 640×360 H.264, 60 frames, AAC mono; provenance in fixture README |
| Conversion video settings | VP8, 320×180, Mediabunny `Quality("high")`, 2-second keyframe interval, `latencyMode: "quality"`, `hardwareAcceleration: "no-preference"` |
| Conversion audio settings | Opus, 48,000 Hz, one channel, `Quality("high")`; video-only profile has no audio track |
| M1 settings | VP8 decode 320×180, optimizeForLatency; encode 160×90, 500,000 bit/s, 30 fps, realtime, no-preference |
| MP4 capability | Disabled in both contexts: `AAC encoding is unsupported at 48000 Hz with 1 channel(s).` |

The conversion quality settings above are requested adapter settings, not independently measured encoder bitrates or proof of hardware execution.

## Verified conversion and fallback results

| Execution | Profile | Video frames / Opus packets | Bytes / duration | Elapsed including verification | Window 50 ms timer: ticks / max gap |
|---|---|---|---|---|---|
| Worker | VP8 + Opus | 60 / 102 | 102,243 / 2.034 s | 6,124 ms | 123 / 54 ms |
| Worker | VP8 video-only | 60 / none | 62,666 / 2.000 s | 6,154 ms | 123 / 66 ms |
| Main-thread fallback | VP8 + Opus | 60 / 102 | 102,243 / 2.034 s | 6,153 ms | 122 / 71 ms |
| Main-thread fallback | VP8 video-only | 60 / none | 62,666 / 2.000 s | 6,241 ms | 123 / 86 ms |

Both audio-preserving runs consumed all 95 decoded source audio samples. Final verification decoded non-silent audio near beginning/middle/end (peak 0.0574), checked track timing and dimensions, and reopened the completed container. Firefox's independent HTML video element loaded both profiles at 320×180 and sought successfully to 1.0169995 s (audio-preserving) or 1.000 s (video-only). Each profile in each context passed cancellation during conversion followed by immediate restart. Starting without a file produced an error and permitted subsequent normal conversion.

Application-held conversion frame references/samples returned to **0/0** after success and cancellation; observed peaks were **2/2**. These counters conservatively include closed frame references retained until their cleanup block; they do not measure library-internal codec resources or GPU memory. Each successful conversion reported 60 additional VideoFrame→ImageBitmap compatibility conversions, 60 ingress image copies by the processing path, and no explicit CPU pixel readback in conversion. Hidden copies remain unknown.

FFprobe 8.0.1 independently decoded/count-checked all four files: 60 VP8 frames each, exactly 102 Opus audio frames at 48 kHz mono in each audio-preserving file, and no audio stream in either video-only file. FFmpeg full audio/video decodes of both audio-preserving files completed with exit code zero. **The previously recorded `Error parsing Opus packet header` diagnostic still appears** during independent open/probing; this is not clean, warning-free FFmpeg interoperability.

The window timer and working Cancel controls demonstrate event-loop responsiveness on this small fixture, not a frame-rate guarantee or statistically established speedup. End-to-end conversion remains roughly six seconds in both modes; worker migration has not removed per-frame synchronization or the Firefox bitmap compatibility path.

## M1 regression and lifecycle checks

- Five consecutive complete worker runs and five consecutive complete main-thread runs passed 30/30 decode → resize → encode → re-decode frames, 160×90 output, timestamps within ±1 µs, orientation and RGB markers, and zero application-owned live frames after cleanup.
- Across these repeats: peak live frames 8, decoder submissions 4, decoded callback queue 6, encoder queue 0, verifier submissions 4, verifier callback queue up to 6. These are observed high-water marks, not newly introduced queue limits.
- Worker M1 elapsed range: 3,088–3,119 ms; main-thread range: 3,046–3,119 ms. These include diagnostic verification and are not conversion-only throughput measurements.
- Every full M1 run counted 30 ingress copies, 30 additional bitmap conversions, zero conversion readbacks and 30 separate test-only pixel readbacks.
- M1 cancellation in each context reported cleanup live frames=0, followed by the five passing restarts above.

## Build and automated checks

- `cargo fmt --all -- --check`: passed.
- `cargo check -p media-core -p media-gpu`: passed.
- `cargo test -p media-core -p media-gpu`: passed, six core tests; GNU linker retained its existing `corrupt .drectve at end of def file` warning for the GPU test binary.
- `cargo clippy -p media-core -p media-gpu -- -D warnings`: passed.
- `cargo check --workspace --target wasm32-unknown-unknown`: passed.
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed.
- `dx serve --web --locked --addr 127.0.0.1 --port 8084` and `dx build --web --locked`: passed; no separate worker build required.
- `node --test tests/worker-transport.test.mjs`: five tests passed. These simulate command serialization/stale events, cancellation and restart, exact-reason startup fallback, runtime-crash teardown/retry without silent fallback, and explicit main-thread mode. These tests are **not** real-browser crash/device-loss evidence.
- `node tests/firefox-worker-interop.mjs worker` and `node tests/firefox-worker-interop.mjs main`: passed. Local detailed logs and media are under ignored `tmp/m33-worker` and `tmp/m33-main`; the harness and aggregate evidence here are tracked.

## Explicitly pending and limitations

- A normal isolated Chromium launch was rejected by the execution environment. Chromium's direct VideoFrame ingress, worker OffscreenCanvas, and MP4/H.264/AAC through the new transport have **not** been revalidated; older M3.2 MP4 results do not prove these changes. Safari and other platforms are also untested.
- Real browser fallback comparison used the explicit mode switch. Startup-capability failures, stale messages, and worker crashes were simulated in transport tests, not injected into a real GPU driver. Worker startup timeout behavior and deployment under restrictive CSP/non-root base paths were not runtime-tested.
- Browser identity was redacted, so neither an exact GPU nor the codec acceleration implementation is established. Hardware-preference comparisons remain M3.4.
- M1's verification and final container/audio verification are diagnostic paths, not fast-path pixel transfers. Conversion timings above include verification; no pure throughput gain is claimed.
- Long-duration worker memory/queue behavior, arbitrary codec hangs, device loss, pooled texture reuse, higher throughput, streaming, geometry/color extensions, and native/FFmpeg work remain outside M3.3. The same wasm bundle is instantiated in both contexts; this duplicates wasm instance memory and is not a minimal worker download.
- Human listening was not performed. The existing silent-source audio verifier limitation, multiple audio tracks, and the Opus header diagnostic are unchanged by worker migration.

## M3.3 completion — Chromium/MP4 validation, 2026-09-19

**Acceptance gate passed for the tested browser/profile matrix.** This section supersedes the earlier Chromium/MP4 pending status. No runtime implementation changes were necessary in this completion pass; reusable Chromium and independent output-inspection harnesses were added. The later M3.4 section records subsequent work.

The user started an isolated Edge session with local debugging port 9227, resolving the browser-launch restriction. The in-app browser bridge remained unavailable (`privileged native pipe bridge is not available; browser-client is not trusted`). Tests connected to the user-provided session, created/closed their own tabs, and did not inspect or change the everyday browser profile.

Environment: Microsoft Edge 153.0.4234.32, normal rendering mode, Windows x64, WebGPU adapter `intel / gen-12lp (BrowserWebGpu)`, app served at `http://127.0.0.1:8084/`. Dependency/toolchain versions and deterministic fixtures are unchanged. All three exact profile probes passed in the worker and main-thread contexts. The acceleration preference remains `no-preference`; adapter identity does not identify the codec engine.

### Real-browser results

Each of three modes tested **all three profiles**, cancellation during frame processing followed by immediate restart, zero application-held frame references/samples after cleanup, HTML media-element loading and midpoint seeking, M1 cancellation and **five consecutive complete M1 rounds**:

1. Dedicated worker: OffscreenCanvas and direct VideoFrame ingress.
2. Explicit compatibility path: `?execution=main`.
3. Automatic compatibility path: a test-only replacement Worker constructor throws `Injected worker startup failure for interoperability test`; the app displays that exact reason and runs the unchanged main-thread implementation. This is a real-browser transport/startup-failure test, not a GPU-driver failure or a claim that the browser lacks workers.

Final foreground-tab measurements (`document.visibilityState` recorded as `visible` for every conversion):

| Mode | Profile | Elapsed including verification | 50 ms timer ticks / max gap |
|---|---|---|---|
| Worker | WebM/VP8/Opus | 405.2 ms | 8 / 53.0 ms |
| Worker | WebM/VP8/video-only | 384.5 ms | 7 / 53.3 ms |
| Worker | MP4/H.264/AAC | 830.4 ms | 17 / 61.0 ms |
| Explicit main | WebM/VP8/Opus | 417.5 ms | 8 / 59.6 ms |
| Explicit main | WebM/VP8/video-only | 408.0 ms | 8 / 56.4 ms |
| Explicit main | MP4/H.264/AAC | 431.9 ms | 9 / 51.4 ms |
| Automatic fallback | WebM/VP8/Opus | 419.9 ms | 8 / 63.3 ms |
| Automatic fallback | WebM/VP8/video-only | 370.3 ms | 7 / 57.1 ms |
| Automatic fallback | MP4/H.264/AAC | 382.1 ms | 7 / 52.1 ms |

An earlier run without explicit tab foregrounding observed a 1,025.5 ms timer gap during the first automatic-fallback conversion; visibility was not recorded in that run, so its cause is not established. The harness now brings its own tab forward and records visibility. These small, variable measurements establish continued window event handling and working cancellation, not a statistically established speed advantage or background-tab responsiveness guarantee.

All nine final outputs retained **60/60 video frames** at 320×180. WebM/Opus used all 95 decoded source audio samples and emitted 102 Opus packets (97,164 bytes, 2.040 s); video-only WebM had no audio (62,512 bytes, 2.000 s). MP4 used all 95 decoded source audio samples and emitted 95 AAC packets (101,653 bytes, 2.026667 s). Browser audio verification near beginning/middle/end measured peaks 0.0424 for Opus and 0.1478 for AAC. HTML midpoint seeks reached 1.02 s, 1.00 s, and 1.013333 s respectively.

Application-held frame-reference/sample peaks were 2/2 in each final conversion; cleanup was 0/0 after completion and cancellation. There were **zero additional bitmap compatibility conversions**, confirming the direct VideoFrame ingress branch was exercised in Chromium. Each processed frame still uses the baseline external-image texture copy; zero explicit conversion pixel readbacks is not proof of zero internal copies.

All 15 final M1 rounds passed 30/30 decode → resize → encode → re-decode frames, 160×90 geometry, timestamps within ±1 µs, orientation and RGB markers. Each reported zero live frames after cleanup, 30 ingress copies, zero bitmap conversions, and 30 separate verification readbacks. Peak metrics were live frames 8, decoder submissions 4, decoded callbacks 6, encoder submissions 1, verifier submissions 4, verifier callbacks 5. M1 elapsed ranges including verification were worker 212.0–471.3 ms, explicit main 228.6–470.6 ms, automatic fallback 213.1–235.9 ms. M1 cancellation in each mode returned live frames=0 and restart succeeded.

### Independent inspection

`tests/inspect-chromium-outputs.mjs` used FFprobe/FFmpeg 8.0.1 to verify all nine files. Each decoded completely with exit code zero and exactly 60 video frames. The six audio-preserving outputs also decoded 50 ms audio segments at 0.1, 1.0 and 1.8 seconds with a finite, non-silent peak. Video-only files contained no audio stream.

MP4 results in all three modes: H.264 High profile (`avc1`, 38-byte codec initialization data), AAC-LC (`mp4a`, 2-byte codec initialization data), mono 48 kHz, 95 decoded audio frames. The `moov` box precedes `mdat` (offsets 28 and 1,947), verifying fast-start layout. Video starts at 0.021337 s and lasts 2.000 s; AAC starts at 0.000 s and lasts 2.026667 s. The measured 21.337 ms track-start offset is within the existing 50 ms verification tolerance; this is not a sample-exact synchronization claim. Independent AAC seek peaks were -17.8, -17.8 and -17.9 dB. MP4 probing and full decoding emitted no warnings.

WebM outputs retained VP8 and the expected Opus/no-audio tracks; each audio-preserving output independently decoded 102 Opus frames at 48 kHz mono. The pre-existing **Opus packet-header warning remains** during FFmpeg open/probing despite complete successful decode. It remains a compatibility finding for later investigation, not a worker migration failure or a warning-free interoperability claim.

### Reproduction, checks and remaining scope

- `node tests/chromium-worker-interop.mjs worker`, `main`, and `fallback`: passed; both an initial run and a final visibility-instrumented run completed in each mode. Final detailed results/media are under ignored `tmp/m33-chromium-*`.
- `node tests/inspect-chromium-outputs.mjs`: all nine outputs passed independent checks; detailed metadata/warnings are saved beside browser evidence.
- Five transport unit tests, Rust formatting, wasm workspace check/Clippy, six core host tests and the locked Dioxus web build passed. The pre-existing GNU GPU-test linker warning is unchanged.
- The earlier Firefox worker/main results remain valid for its two enabled WebM profiles; Firefox still cannot enable this AAC profile. The unsupported reason is expected capability behavior, not a remaining M3.3 implementation task.
- No human listening, arbitrary worker runtime crash/device-loss injection, startup-timeout test, Safari/other-OS test, or restrictive-CSP/subpath deployment validation was added. Transport runtime-crash handling remains unit-tested; driver recovery and broader compatibility are later milestones.
- Long-run memory/queue profiling, resource pools, hardware-preference comparisons, geometry/color work and streaming remain M3.4–M3.6. Completion here is scoped to the M3.3 acceptance gate, not completion of M3 overall.

# M3.4 bounded-throughput and resource-reuse report

Date: 2026-09-19
Status: **acceptance passed for the tested Firefox/Chromium environments; M3.5 was not started**

## Implemented behavior

- The UI and platform-neutral job policy expose `no-preference` (default compatibility baseline) and `prefer-hardware`. The chosen value crosses the worker boundary and is applied to the exact input decoder and selected output encoder probes. A requested hardware preference is used only if the complete video profile passes. Otherwise the app visibly reports the failed exact configuration and uses `no-preference`; it never changes codec or container.
- Conversion now reports live/peak/total/discarded counts for decoded video, GPU work, video encoder callbacks, video packet callbacks, decoded audio, audio encoder callbacks, and audio packet callbacks. Completion and cancellation settle application-owned callback counts back to zero.
- The pinned Mediabunny implementation bounds its combined decoder packet/callback queue at 40 before decoded output and 8 while producing decoded samples, and waits when the WebCodecs encoder queue reaches 4. Application decoded-frame, GPU, and source-add stages are serial, audio submissions are serial, and mux writes are promise-serialized. These are separate bounds; codec/library-internal memory is not claimed as observable.
- The wgpu execution context owns a device-generation-tagged, single-slot input texture pool. Each use holds an explicit lease through submitted-work completion and output capture. A concurrent lease or generation mismatch fails rather than reusing the texture. Telemetry reports allocation/reuse, leases, ingress copies, canvas captures, CPU bridge/submission time, and submitted-work completion waits. The latter includes synchronization and is explicitly not pure GPU execution time.
- Worker initialization time is measured once and reported separately from conversion. The existing application frame/sample ownership counters and cancellation/restart checks remain in place.

## Verified short-job matrix

The final-code worker checks used Edge 153.0.4234.32 on Windows x64 with `intel / gen-12lp (BrowserWebGpu)` and Firefox 156.0 with browser-redacted adapter identity. The deterministic 60-frame H.264/AAC fixture was converted, re-decoded in the browser, loaded and midpoint-seeked in an HTML media element, cancelled and restarted for every enabled profile/preference combination. Five repeated M1 image/timestamp checks also passed in each final worker run.

| Browser | Enabled profiles | Requested preference | Selected preference | Result |
|---|---|---|---|---|
| Edge 153 | WebM/VP8/Opus, WebM/VP8/video-only, MP4/H.264/AAC | `no-preference` | `no-preference` | all passed; 407.0, 422.2, and 700.1 ms including verification |
| Edge 153 | same three profiles | `prefer-hardware` | `no-preference` | all passed after visible fallback; exact VP8/H.264 encoder probes rejected the requested preference; 387.2, 376.6, and 738.5 ms |
| Firefox 156 | both WebM profiles | `no-preference` | `no-preference` | both passed; 6,213 and 6,187 ms including verification |
| Firefox 156 | both WebM profiles | `prefer-hardware` | `no-preference` | both passed after visible fallback; exact VP8 encoder probe rejected the requested preference; 6,151 and 6,173 ms |

The hardware-request rows are **baseline fallback runs**, not measurements of hardware-preferred encoding. Timing differences are warm-cache/run variation and do not justify an automatic preference. Firefox continued to disable MP4 because its exact AAC encoder probe fails, independent of the video acceleration setting.

Across final short worker conversions, each stage ended with zero live application-owned items. GPU leases ended at `0/1` live/peak, there were 60 ingress copies and 60 canvas captures, and one input texture slot was reused for all 60 frames after size configuration. Edge used direct VideoFrame ingress with zero additional bitmap conversions. Firefox used its documented VideoFrame→ImageBitmap compatibility conversion 60 times. Edge worker initialization measured 353.0 ms and Firefox 673.0 ms in these runs; that context was reused by every conversion command.

Before the final worker smoke, the same two acceleration requests × three profiles also passed the explicit main-thread and injected automatic-fallback paths in Edge. Together with the worker path, `tests/inspect-chromium-outputs.mjs` independently inspected all 18 short outputs using FFprobe/FFmpeg 8.0.1: every file fully decoded with 60 video frames; audio-preserving files retained the expected AAC or Opus audio, video-only files had none, and MP4 retained fast-start layout. The existing Mediabunny Opus packet-header warning remains even though full decoding succeeds.

## Verified long run

`tests/chromium-long-run.mjs` used the local 70,439,352-byte `tmp/user-test/Input.mp4` in the Edge dedicated worker with the `no-preference` WebM/VP8/Opus profile:

- Input/output: 3,530 H.264 frames at 1920×1080 plus 6,343 decoded audio samples → 3,530 VP8 frames at 960×540 plus 7,365 Opus packets.
- Duration/output: 147.300 seconds, 32,138,436 bytes; browser re-decode verification passed. The test intentionally retained the output Blob in the browser rather than writing a 32 MiB repository artifact.
- End-to-end time: 47,058.3 ms including verification. A 50 ms window heartbeat ticked 946 times with a 62.9 ms maximum observed gap; this is a responsiveness observation, not a real-time guarantee.
- Stage peaks: decoded video 1, GPU 1, video encoder callbacks 3, video packets 1, decoded audio 1, audio encoder callbacks 1, and audio packets 1. Every stage ended at zero live items. Application frame/sample ownership ended at 0/0 with peaks 2/3.
- GPU pool: 2 lifetime allocations across initial/configured sizes, 3,529 job reuses, leases `0/1`, 3,530 ingress copies, 3,530 canvas captures, and zero bitmap fallbacks. Reported CPU bridge/submission time was 1,966.7 ms; submitted-work waits were 41,723.3 ms and are not interpreted as pure GPU time.

The final strict lease/generation guard was subsequently exercised by the complete short Edge and Firefox matrices. The long run establishes stable observable high-water marks and cleanup for the same pool path; it is not a browser-process heap or driver-memory profile.

## Checks

- `cargo fmt --all -- --check`, wasm workspace check, and wasm all-target Clippy with `-D warnings`: passed.
- `cargo test -p media-core -p media-gpu`: 7 core tests passed; the existing GNU linker `corrupt .drectve` warning for the empty GPU test binary remains.
- JavaScript syntax checks and `node tests/worker-transport.test.mjs`: passed (5 tests).
- `dx build --platform web`: passed with Dioxus CLI 0.7.10.
- Final `node tests/chromium-worker-interop.mjs` and `node tests/firefox-worker-interop.mjs worker`: passed. Earlier M3.4 Edge main/fallback matrices and independent inspection also passed. Evidence and generated media remain under ignored `tmp/m33-chromium-*`, `tmp/m34-firefox-worker`, and `tmp/m34-long`.

## Explicitly untested or unavailable

- No tested output encoder accepted `prefer-hardware`, so actual hardware-preferred throughput, startup, CPU load, power, and codec-engine selection remain unmeasured. WebCodecs configuration success and the displayed wgpu adapter would not prove hardware codec execution anyway.
- The browser exposes no reliable, codec-attributed CPU-load or power metric used by this app. GPU timestamp queries were not requested/supported in this path; submitted-work waits are not a substitute.
- wgpu 30.0.1's released browser backend does not implement its plane-based external-texture API for this use and exposes no released direct VideoFrame texture import. Therefore no direct-import comparison was possible. The measured baseline remains one external-image copy per frame, plus Firefox's explicit compatibility bitmap conversion.
- Safari, non-Windows systems, other GPUs/drivers, device loss, browser-process/driver heap stability, arbitrary codec hangs, HDR/geometry/color extensions, and streaming output remain untested. These are not folded into the M3.4 acceptance claim. M3.5 is the next planned milestone.
