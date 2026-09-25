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

- FFmpeg 8.0.1 emits one `Error parsing Opus packet header` diagnostic when opening each Mediabunny-produced Opus WebM, but then decodes the complete audio track and reports the expected duration and non-silent signal. Edge and Mediabunny decode/seek checks pass. This remains a known cross-implementation compatibility limitation; it is not being presented as warning-free FFmpeg interoperability.
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
- The wgpu execution context owns a device-generation-tagged, four-slot input texture ring. Each slot holds an explicit lease plus its decoded frame and optional compatibility bitmap through submitted-work completion. Reuse first retires the slot's previous completion callback; job completion and cancellation drain every remaining slot before cleanup. Telemetry reports allocation/reuse, leases, ingress copies, canvas captures, CPU bridge/submission time, cumulative overlapping completion latency, actual slot-reuse wait, and final-drain wait. Completion latency is explicitly not pure GPU execution time.
- Worker initialization time is measured once and reported separately from conversion. The existing application frame/sample ownership counters and cancellation/restart checks remain in place.

## Initial single-slot short-job baseline

The initial single-slot worker checks used Edge 153.0.4234.32 on Windows x64 with `intel / gen-12lp (BrowserWebGpu)` and Firefox 156.0 with browser-redacted adapter identity. The deterministic 60-frame H.264/AAC fixture was converted, re-decoded in the browser, loaded and midpoint-seeked in an HTML media element, cancelled and restarted for every enabled profile/preference combination. Five repeated M1 image/timestamp checks also passed in each worker run. The later four-slot comparison supersedes these numbers for current performance while retaining them as the measured baseline.

| Browser | Enabled profiles | Requested preference | Selected preference | Result |
|---|---|---|---|---|
| Edge 153 | WebM/VP8/Opus, WebM/VP8/video-only, MP4/H.264/AAC | `no-preference` | `no-preference` | all passed; 407.0, 422.2, and 700.1 ms including verification |
| Edge 153 | same three profiles | `prefer-hardware` | `no-preference` | all passed after visible fallback; exact VP8/H.264 encoder probes rejected the requested preference; 387.2, 376.6, and 738.5 ms |
| Firefox 156 | both WebM profiles | `no-preference` | `no-preference` | both passed; 6,213 and 6,187 ms including verification |
| Firefox 156 | both WebM profiles | `prefer-hardware` | `no-preference` | both passed after visible fallback; exact VP8 encoder probe rejected the requested preference; 6,151 and 6,173 ms |

The hardware-request rows are **baseline fallback runs**, not measurements of hardware-preferred encoding. Timing differences are warm-cache/run variation and do not justify an automatic preference. Firefox continued to disable MP4 because its exact AAC encoder probe fails, independent of the video acceleration setting.

Across these baseline short worker conversions, each stage ended with zero live application-owned items. GPU leases ended at `0/1` live/peak, there were 60 ingress copies and 60 canvas captures, and one input texture slot was reused for all 60 frames after size configuration. Edge used direct VideoFrame ingress with zero additional bitmap conversions. Firefox used its documented VideoFrame→ImageBitmap compatibility conversion 60 times. Edge worker initialization measured 353.0 ms and Firefox 673.0 ms in these runs; that context was reused by every conversion command.

Before the final worker smoke, the same two acceleration requests × three profiles also passed the explicit main-thread and injected automatic-fallback paths in Edge. Together with the worker path, `tests/inspect-chromium-outputs.mjs` independently inspected all 18 short outputs using FFprobe/FFmpeg 8.0.1: every file fully decoded with 60 video frames; audio-preserving files retained the expected AAC or Opus audio, video-only files had none, and MP4 retained fast-start layout. The existing Mediabunny Opus packet-header warning remains even though full decoding succeeds.

## Initial single-slot long run

`tests/chromium-long-run.mjs` used the local 70,439,352-byte `tmp/user-test/Input.mp4` in the Edge dedicated worker with the `no-preference` WebM/VP8/Opus profile:

- Input/output: 3,530 H.264 frames at 1920×1080 plus 6,343 decoded audio samples → 3,530 VP8 frames at 960×540 plus 7,365 Opus packets.
- Duration/output: 147.300 seconds, 32,138,436 bytes; browser re-decode verification passed. The test intentionally retained the output Blob in the browser rather than writing a 32 MiB repository artifact.
- End-to-end time: 47,058.3 ms including verification. A 50 ms window heartbeat ticked 946 times with a 62.9 ms maximum observed gap; this is a responsiveness observation, not a real-time guarantee.
- Stage peaks: decoded video 1, GPU 1, video encoder callbacks 3, video packets 1, decoded audio 1, audio encoder callbacks 1, and audio packets 1. Every stage ended at zero live items. Application frame/sample ownership ended at 0/0 with peaks 2/3.
- GPU pool: 2 lifetime allocations across initial/configured sizes, 3,529 job reuses, leases `0/1`, 3,530 ingress copies, 3,530 canvas captures, and zero bitmap fallbacks. Reported CPU bridge/submission time was 1,966.7 ms; submitted-work waits were 41,723.3 ms and are not interpreted as pure GPU time.

This baseline long run established stable observable high-water marks and cleanup for the original slot path; it was not a browser-process heap or driver-memory profile. The four-slot implementation was separately rerun below.

## Checks

- `cargo fmt --all -- --check`, wasm workspace check, and wasm all-target Clippy with `-D warnings`: passed.
- `cargo test -p media-core -p media-gpu`: 7 core tests passed; the existing GNU linker `corrupt .drectve` warning for the empty GPU test binary remains.
- JavaScript syntax checks and `node tests/worker-transport.test.mjs`: passed (5 tests).
- `dx build --platform web`: passed with Dioxus CLI 0.7.10.
- Final `node tests/chromium-worker-interop.mjs` and `node tests/firefox-worker-interop.mjs worker`: passed. Earlier M3.4 Edge main/fallback matrices and independent inspection also passed. Evidence and generated media remain under ignored `tmp/m33-chromium-*`, `tmp/m34-firefox-worker`, and `tmp/m34-long`.

## Four-slot synchronization follow-up

The single-slot measurements isolated the Firefox bottleneck: about 1.1–1.3 seconds was spent in VideoFrame→ImageBitmap ingress/submission and about 4.6–4.8 seconds in 60 serial submitted-work waits. The processor now retains at most four submissions and retires a slot only before its next reuse. This changes synchronization and resource ownership only; codecs, quality settings, shader, copy path, capture ordering, and output profiles are unchanged.

Final four-slot worker results for the same deterministic input:

| Browser | Profile/request | Single-slot baseline | Four-slot result | Observed change |
|---|---|---:|---:|---:|
| Firefox 156 | WebM/VP8/Opus, baseline | 6,213 ms | 1,410 ms | 77.3% lower elapsed time |
| Firefox 156 | WebM/VP8/video-only, baseline | 6,187 ms | 1,298 ms | 79.0% lower elapsed time |
| Firefox 156 | WebM/VP8/Opus, hardware request falling back to baseline | 6,151 ms | 1,172 ms | 80.9% lower elapsed time |
| Firefox 156 | WebM/VP8/video-only, hardware request falling back to baseline | 6,173 ms | 1,757 ms | 71.5% lower elapsed time |
| Edge 153 | WebM/VP8/Opus, baseline | 407.0 ms | 325.8 ms | 20.0% lower elapsed time |
| Edge 153 | WebM/VP8/video-only, baseline | 422.2 ms | 356.1 ms | 15.7% lower elapsed time |
| Edge 153 | MP4/H.264/AAC, baseline | 700.1 ms | 298.3 ms | 57.4% lower elapsed time |

These are individual acceptance runs rather than a statistical benchmark. The hardware-request rows still selected `no-preference`, so their variation is not a hardware-acceleration result. Firefox's four-slot worker reported lease peaks of 4, only 0–2 ms total slot-reuse wait, 0–18 ms final-drain wait, and zero live leases after each complete job. Its explicit main-thread path also passed both profiles × both requests in 1,260–1,581 ms, including cancellation/restart, playback/seek, and five M1 rounds. Edge reported a lease peak of 4 and at most 1.2 ms slot-reuse wait in the final worker matrix.

Browser re-decode, playback/seek, cancellation/restart, and five repeated deterministic M1 image/timestamp runs passed in both browsers. FFprobe/FFmpeg independently decoded all six newly generated Edge worker outputs with exactly 60 video frames and the expected AAC, Opus, or absent audio; the known Opus warning remains. This verifies that returning captured frames before each individual completion callback did not reorder or corrupt the tested output.

The 147.3-second Edge test was repeated with the ring: 3,530/3,530 frames, 6,343 decoded audio samples, 7,365 Opus packets, 32,138,436 bytes, and full browser verification passed in 46,406.5 ms versus the 47,058.3 ms single-slot run. Leases ended at `0/4`; slot-reuse wait totaled 82.0 ms, final drain 0.1 ms, and the 50 ms heartbeat's maximum gap was 57.4 ms. This only modestly improved the long Edge run, showing that its long-input bottleneck lies elsewhere; the large Firefox short-job gain specifically confirms that per-frame synchronization dominated the earlier Firefox result.

## Explicitly untested or unavailable

- No tested output encoder accepted `prefer-hardware`, so actual hardware-preferred throughput, startup, CPU load, power, and codec-engine selection remain unmeasured. WebCodecs configuration success and the displayed wgpu adapter would not prove hardware codec execution anyway.
- The browser exposes no reliable, codec-attributed CPU-load or power metric used by this app. GPU timestamp queries were not requested/supported in this path; submitted-work waits are not a substitute.
- wgpu 30.0.1's released browser backend does not implement its plane-based external-texture API for this use and exposes no released direct VideoFrame texture import. Therefore no direct-import comparison was possible. The measured baseline remains one external-image copy per frame, plus Firefox's explicit compatibility bitmap conversion.
- Safari, non-Windows systems, other GPUs/drivers, device loss, browser-process/driver heap stability, arbitrary codec hangs, HDR conversion, wide-gamut output, and streaming output remained untested at M3.4. M3.5 subsequently added the tested SDR/geometry policy below; M3.6 is the next planned milestone.

## Bounded bitmap preparation and verification timing, 2026-09-20

Firefox's compatibility ingress now probes the first frame through the real direct-copy path, then—only after that copy establishes that ImageBitmap fallback is required—prepares subsequent bitmaps in bounded groups of four. Decoded frames/samples remain owned while preparation is pending, completed bitmaps are consumed strictly in timestamp order, Rust takes ownership before GPU submission, and cancellation/error cleanup closes every bitmap that was not transferred. Chromium continues to use direct VideoFrame ingress and created zero preparation tasks; there is no browser-name switch.

Firefox 156 worker results for the 60-frame deterministic fixture with full diagnostic verification:

| Profile/request | Converted/finalized | Full verification | Total job | Bitmap preparation |
|---|---:|---:|---:|---|
| WebM/VP8/Opus baseline | 1,248 ms | 65 ms | 1,313 ms | 59 tasks; peak 2; 1,007 ms cumulative; 1 ms ordered wait |
| WebM/VP8/video-only baseline | 1,239 ms | 37 ms | 1,276 ms | 59 tasks; peak 2; 1,011 ms cumulative; 2 ms ordered wait |
| WebM/VP8/Opus hardware request → baseline fallback | 1,204 ms | 49 ms | 1,253 ms | 59 tasks; peak 2; 980 ms cumulative; 6 ms ordered wait |
| WebM/VP8/video-only hardware request → baseline fallback | 1,168 ms | 35 ms | 1,203 ms | 59 tasks; peak 2; 935 ms cumulative; 0 ms ordered wait |

All four jobs passed full browser re-decode, audio checks where applicable, playback/seek, cancellation/restart, zero-live-resource cleanup, and five repeated M1 orientation/color/timestamp rounds. Decoded-video preparation peaked at four retained items; application frame/sample peaks were at most 5/5, GPU leases returned to `0/4`, and bitmap preparation returned to `0/2`. Cancellation summaries also returned bitmap preparation to zero. The explicit Firefox main-thread path passed the same matrix and five M1 rounds, with 1,195–1,527 ms conversion/finalization times. The prior four-slot runs were about 1,172–1,757 ms including verification, so the new wall-time result is directionally better but noisy and modest—not a fourfold bitmap speedup. Firefox appears to expose at most two concurrent preparation tasks here, and cumulative ImageBitmap work remains roughly one second.

With diagnostic verification omitted—the normal UI behavior—two baseline Firefox runs converted/finalized in 1,160–1,208 ms (Opus) and 1,112–1,146 ms (video-only). Full verification itself cost 35–65 ms in the measured worker runs. When verification is skipped, displayed duration is explicitly an expected timeline derived from processed timestamps rather than a re-decoded container duration. Automated interoperability URLs use `?verify=full`; `VERIFY_OUTPUT=skip` exercises the production path.

Edge 153's final worker matrix also passed all three profiles × both acceleration requests, cancellation/restart, playback/seek and five M1 rounds. Direct ingress remained selected: bitmap preparation total/peak were `0/0`. Baseline converted/finalized times were 261.5 ms for WebM/VP8/Opus, 330.2 ms for video-only WebM, and 266.8 ms for MP4/H.264/AAC; their full verification steps cost 15.9, 12.3 and 14.2 ms respectively. FFprobe/FFmpeg independently decoded the six newly generated worker outputs with 60/60 video frames and expected audio/no-audio tracks; the existing Opus warning remains.

Conclusion: both requested changes are validated, but bounded bitmap preparation does not close Firefox's remaining gap. The next justified experiment is the explicitly counted CPU upload compatibility path described in the architecture discussion; increasing the GPU slot count is not supported by these measurements.

# M3.5 geometry, timing, and color interoperability report

Date: 2026-09-21
Status: **acceptance passed for every profile enabled in the tested Firefox/Chromium environments; M3.6 subsequently completed as recorded below**

## Implemented policy

The browser backend now inspects coded dimensions, decoded visible rectangle, square-pixel dimensions, pixel aspect ratio, clockwise rotation, post-rotation horizontal flip, color metadata, HDR state, and transparency before configuring the GPU. `media-core` owns the platform-neutral geometry types and validation. The browser external-image boundary normalizes the decoded visible image, pixel aspect ratio, and accepted SDR color into the square-pixel sRGB processing texture; shared WGSL applies the inverse sampling transform for rotation/flip while performing the half-size resize. The encoder therefore receives pixels with orientation baked in and emits square-pixel, rotation=0, flip=false output.

Accepted color is opaque BT.709/sRGB SDR: primaries may be unspecified or BT.709, transfer may be unspecified, BT.709, or IEC 61966-2-1, and matrix may be unspecified, BT.709, or RGB. Browser color conversion into the `Rgba8Unorm` canvas is relied on and reported; this is not a claim of wide-gamut or HDR fidelity. Transparency, BT.2020/PQ and other HDR/wide-color combinations are rejected before conversion. A change to coded/visible/square-pixel dimensions, orientation, or color metadata after the first decoded sample fails the job and drains owned resources; adaptive resolution reconfiguration is deferred.

Every processed input timestamp and duration remains an integer microsecond value. Full diagnostic verification reopens the finalized container, enumerates all encoded video packets, sorts them into presentation order, and compares every timestamp/duration with the processed input timeline. WebM permits at most 1,000 µs error because its millisecond timecode independently rounds packet endpoints; MP4 permits 5 µs in the tested muxer timescale. When the final packet omits its duration, its start is still checked and the independently checked track coverage supplies the final endpoint. Midpoint video seek and decoded audio near the beginning, middle, and end retain the earlier M3 checks.

## Deterministic fixtures

`tools/generate-m35-fixtures.ps1` generated four CC0-1.0 inputs with FFmpeg 8.0.1:

| Fixture | Bytes | SHA-256 | Purpose |
|---|---:|---|---|
| `m35-geometry-color.mp4` | 56,356 | `03dbdd9a99de71b411f1c22c9feb69fe75ecc38f67bea3a83051072835b8bec9` | H.264 clean-aperture crop, non-square pixels, 90° clockwise rotation, horizontal flip, BT.709 markers, AAC tone |
| `m35-vfr-offset.mp4` | 88,866 | `c16c968ee505aa6e2a0ee3d4fb0ac8ea58aad0378e97062a7872925cb0983724` | 36 VFR frames with 33,333/66,667 µs durations, video start 1.250 s, AAC start 1.228 s |
| `m35-hdr-tagged.mp4` | 9,032 | `f4ee492ca6a2474b65a63fa986e2013edeca19ff33fa3b3cb22a976974919b48` | Deterministic BT.2020/PQ negative input |
| `m35-resolution-change.mp4` | 58,651 | `51bcc46d0498dab9bde031cd3d1e7706846c8a03ce2dbf0b9f6a6f703efe5e45` | 320×180 H.264 followed by 352×198 H.264 |

The geometry fixture begins as a 320×192 synthetic source with explicit H.264 cropping. In both tested browsers the decoder normalized that aperture into a 318×180 coded allocation with a full 318×180 `visibleRect`; the track's sample-aspect/display metadata produced 421×180 square-pixel input and a 180×421 oriented display, resized to 90×210. Thus compressed crop normalization was exercised, while a decoder-exposed non-zero `visibleRect` was not available in this H.264/browser matrix. The implementation validates and uses such a rectangle if supplied, but that subcase remains explicitly unverified.

## Real-browser results

The app was served by Dioxus CLI 0.7.10 in full diagnostic mode and ran in a dedicated worker on Windows x64. The checked-in harnesses were `node tests/chromium-m35-interop.mjs 9230 8084` and `node tests/firefox-m35-interop.mjs 8084`. Evidence is retained locally under ignored `tmp/m35-chromium` and `tmp/m35-firefox`.

| Browser/GPU | Enabled profiles | Geometry/color result | VFR/timing result |
|---|---|---|---|
| Edge 153.0.4234.48; `intel / gen-12lp (BrowserWebGpu)` | WebM/VP8/Opus, WebM/VP8/video-only, MP4/H.264/AAC | 24/24 frames per profile; decoded 90×210; sampled corners red `(253,25,11)`, blue `(3,12,239)`, green `(9,108,0)`, yellow `(255,240,0)` for VP8 and equivalent H.264 values; seek/audio checks passed | 36/36 frames per profile; decoded 160×90; maximum timeline error 667 µs WebM and 4 µs MP4; audio profiles retained non-silent audio and A/V offsets |
| Firefox 156.0; adapter identity redacted (`BrowserWebGpu`) | WebM/VP8/Opus, WebM/VP8/video-only | 24/24 frames per profile; decoded 90×210; sampled corners red `(252,26,1)`, blue `(0,17,250)`, green `(0,109,2)`, yellow `(253,242,4)`; seek/audio checks passed | 36/36 frames per profile; decoded 160×90; maximum timeline error 667 µs; Opus profile retained non-silent audio and A/V offsets |

Every enabled profile reported baked square-pixel output with no rotation/flip metadata, passed packet-count and complete per-frame timeline checks, and returned application-held frames/samples and GPU leases to zero. Chromium exposed all three profiles; Firefox still did not expose the MP4/H.264/AAC encoder profile, so it was not presented as available and was not counted as a Firefox failure.

Both browsers rejected the HDR fixture during inspection with `HDR input is not supported by the M3.5 SDR pipeline (bt2020/pq/bt2020-ncl)`. Both began the resolution-change fixture at 320×180, detected the first 352×198 decoded frame, failed with `Mid-stream geometry change is unsupported`, and cleaned application-held frames/samples to zero. No altered output was offered for either negative case.

## Checks and limitations

Verified in this milestone: Rust host tests/checks/lints, complete wasm workspace checks/lints, JavaScript syntax/unit tests, Dioxus web build, deterministic regeneration metadata/hashes, and the two real-browser harnesses above. Exact commands are listed in the repository README.

Still untested: Safari, other operating systems/GPUs, decoder-exposed non-zero visible-rectangle offsets, vertical flip independent of rotation-matrix decomposition, HDR conversion/tone mapping, wide-gamut outputs, transparency, adaptive mid-stream reconfiguration, and browser-internal color-conversion precision beyond representative decoded marker samples. Successful SDR marker checks do not prove colorimetric conformance, and the wgpu adapter still does not identify the codec execution engine.

# M3.6 configurable-output and bounded-streaming interoperability report

Date: 2026-09-24 through 2026-09-25
Status: **M3.6 acceptance passed in the available Firefox/Chromium environments; unavailable-platform and spontaneous-failure limitations remain explicit**

## Implemented policy

`media-core::ResizeSpec` owns original-size, percentage, and exact-size policy without browser or UI dependencies. The UI exposes original, 75%, 50%, 25%, and exact width/height. Exact sizing preserves display aspect ratio by default; disabling the lock explicitly requests stretching. The size is resolved from the post-crop, square-pixel, oriented display geometry. Requests are checked for zero, invalid percentage, overflow, implicit upscaling, and outputs smaller than the minimum 2×2 codec geometry. Odd resolved dimensions are floored to even values and the adjusted result is displayed before conversion.

File selection and every resize change run the real decoder/encoder capability probes for the resolved dimensions. The same resolved size is carried through worker or main-thread transport, wgpu configuration, canvas capture, status, encoding, playback, and full output verification. The existing deterministic 640×360 H.264/AAC fixture was reused; no new media fixture was necessary.

Input is lazily read through Mediabunny's `BlobSource` with an 8 MiB maximum cache; the former unconditional 256 MiB input rejection was removed. Output now uses Mediabunny 1.58.1's `StreamTarget` over origin-private file storage with WritableStream backpressure, 4 MiB target chunks, and measured write counts/maximum write size. MP4 uses non-fast-start layout on this random-access target so encoded media is not held for in-memory fast-start finalization. After close, the disk-backed `File` supports the existing object URL, playback, and full verification. The latest temporary file remains valid for that URL and is removed when the next conversion supersedes it; abnormal/cancel paths remove their partial file.

When origin-private writable storage is unavailable, the same job uses a clearly reported `BufferTarget` compatibility fallback and retains a 256 MiB input cap. `?output=memory` exists only to exercise that path deterministically. There is no mid-job retry from a failed stream into memory, avoiding duplicate encode work or a sudden unbounded allocation.

## Real-browser results

The app was served by Dioxus CLI 0.7.10 with `?verify=full` and ran in its dedicated worker on Windows x64. The checked-in harnesses were `node tests/chromium-resize-interop.mjs 9231 8084` and `node tests/firefox-resize-interop.mjs 8084`. Evidence is retained locally under ignored `tmp/m36-resize-chromium` and `tmp/m36-resize-firefox`.

| Browser/GPU | Enabled profiles | Verified resize cases |
|---|---|---|
| Edge 153.0.4234.48; `intel / gen-12lp (BrowserWebGpu)` | WebM/VP8/Opus, WebM/VP8/video-only, MP4/H.264/AAC | original 640×360; 75% 480×270; 50% 320×180; 25% 160×90; exact 500×500 locked → 500×280; exact 501×301 unlocked → 500×300 (18 complete conversions) |
| Firefox 156.0.1; adapter identity redacted (`BrowserWebGpu`) | WebM/VP8/Opus, WebM/VP8/video-only | the same six resolved sizes (12 complete conversions) |

Every conversion matched the displayed output geometry, completed full encoded-output re-decode verification, loaded for playback at the expected dimensions, and returned application-held frames/samples and GPU leases to zero. The Firefox matrix continued to expose only the two WebM profiles; unavailable MP4 was not counted as a failure.

All 30 matrix conversions used bounded origin-private output. Their summaries reported the configured 4 MiB chunk boundary and measured writes; the small deterministic outputs required one write each, with maximum writes far below the boundary. Chromium and Firefox additionally converted a valid logical 269,763,667-byte MP4 consisting of the M2 fixture plus a reproducible sparse 257 MiB `free` box. Both processed 60/60 frames at 160×90 through WebM/VP8/video-only, fully re-decoded the disk-backed output, and reported one bounded output write (17,340 bytes Chromium; 19,084 bytes Firefox). This crosses the former input policy boundary and verifies bounded application I/O.

Chromium's explicitly forced memory mode converted the normal fixture and passed full verification while visibly reporting `memory fallback (maximum input 256 MiB)`. After bounded input inspection and capability probing, the same mode rejected the 257.3 MiB logical input before starting the conversion decode/process/encode pumps, with the exact fallback reason. A separate deterministic compatibility case removed `StorageManager.getDirectory` from the main-thread execution context without setting `?output=memory`; the normal automatic branch selected the same memory fallback, named `origin-private file storage is unavailable`, converted 60/60 frames, and fully re-decoded the output. This validates API-unavailable selection, not a naturally shipped browser without the API or a mid-write storage failure.

The bounded output entry remains present only while its returned `File`/object URL is usable. Replacement removes the previous entry. Both harnesses recorded the origin's pre-test `diaxus-*.partial` baseline, observed exactly one additional current-page entry after finalization, dispatched pagehide, and returned to the baseline. They repeated the check with a real same-origin navigation and again returned to the baseline. Existing entries from unrelated/forcibly terminated tabs are deliberately not deleted because ownership cannot be proved.

Both browsers cancelled an active conversion after changing the size and reported zero application-held frame/sample resources. Both rejected an aspect-locked 800×500 request before conversion because it would resolve to 800×450 and upscale the 640×360 source. The error reported the requested and oriented source dimensions; no output was offered.

## Checks and M3.6 completion

Verified for M3.6: focused `media-core` resize/input-policy tests, host Rust tests/checks/lints, complete wasm workspace checks/lints, JavaScript syntax and worker-transport tests, Dioxus web build, the M3.5 real-browser regression harnesses, 30 resize/profile browser conversions, bounded large-input checks, forced and automatic storage fallback, synthetic and real-navigation OPFS cleanup, codec/device-loss recovery in both execution contexts, actual GPU/encoder boundary probes, and the long-run/large-output Chromium stress case. Mediabunny was updated from 1.58.0 to 1.58.1 and the npm lockfile was regenerated.

M3.6 is complete for the available test environments. The matrix now includes 50%; controlled recovery and restart pass; page-exit cleanup returns current-page storage to its baseline; API-unavailable fallback passes; actual GPU/encoder boundaries are recorded below; and the long-duration/large-output run passes. Safari, non-Windows systems, other GPUs/drivers, spontaneous driver/browser-process loss, indefinitely hung codec callbacks, naturally absent/denied storage APIs, and HEVC output on a browser that supports it remain untested. These are explicit compatibility limitations, not claims of universal support and not locally actionable blockers to M4.

## Named resolution presets and source metadata, 2026-09-24

The resize selector now also exposes aspect-preserving maximum bounds for HD/720p (1280×720), Full HD/1080p (1920×1080), 2K width (2048 pixels with source aspect preserved), QHD/1440p (2560×1440), and 4K UHD/2160p (3840×2160). `media-core` owns these concrete `ResizeSpec` constants. A focused UHD-input test verifies every resolved dimension, including 2048×1152 for the 2K-width choice on a 16:9 source. Named modes use the same post-orientation resolution, even-dimension adjustment, profile re-probe, and no-upscale policy as exact sizing.

After selection, the UI now reports filename/file size, display and coded resolution, H.264 codec parameter string, duration, average packet/frame rate, video frame count, and audio codec/channel/sample-rate metadata returned by the browser inspection boundary. Edge 153 and Firefox 156 both displayed `640×360`, an `avc1.*` H.264 string, 60 frames, 2.000 seconds, 30 fps, and mono 48 kHz AAC for the deterministic M2 fixture. Both exposed all five named choices and clearly rejected HD/720p for that 640×360 source as implicit upscaling; the existing original/percentage/exact matrices then passed unchanged. Actual high-resolution encoding at each named bound remains dependent on the exact browser/profile capability probe and was not claimed from this low-resolution fixture.

## H.265/HEVC input and output profile, 2026-09-24

`media-core` now defines `Mp4H265Aac` as a concrete MP4/HEVC/AAC profile. The browser boundary accepts AVC or HEVC MP4 video after exact decoder probing, reports the actual `avc1`/`avc3`/`hvc1`/`hev1` parameter string, and uses the same bounded decode, wgpu processing, audio, cancellation, OPFS output, and full verification path. HEVC output has its own exact video/audio encoder probe and disabled reason; it does not silently fall back to H.264 or become the preferred profile.

The new 52,426-byte CC0 fixture `fixtures/m36-h265-aac.mp4` contains 30 synthetic 320×180 HEVC Main-profile frames at 30 fps and one second of mono 48 kHz AAC. Its checked-in manifest and generator record provenance, expected streams, and SHA-256.

Verified on Windows x64:

- Chromium/Edge 153 decoded the HEVC fixture, displayed `H.265/HEVC (hev1.1.6.L60.90)`, processed all 30 frames through wgpu to 160×90 VP8, finalized bounded OPFS output, and fully re-decoded all 30 output frames. Cleanup returned application-held frames, samples, and GPU leases to zero.
- That Chromium environment rejected the exact HEVC encoder probe at 320×180/30 fps with `hardwareAcceleration=no-preference`; the H.265 output option remained disabled with that reason. No HEVC output file was produced or claimed.
- Firefox 156 rejected the exact HEVC input decoder configuration. Its HEVC output profile also remained unavailable. The existing five-size × two-WebM-profile Firefox regression matrix continued to pass.

The checked-in focused Chromium harness records input and output capabilities independently. Actual H.265 output muxing/re-decode still requires validation on a browser/OS/GPU combination whose `VideoEncoder` probe accepts HEVC. HEVC Main10/HDR, alpha, other containers, non-Windows platforms, and licensing suitability for distribution remain untested or out of scope for this slice.

## Injected codec and WebGPU device-loss recovery, 2026-09-25

The recovery boundary now has two deterministic, test-only failure modes. `?failure=codec-once` throws after the fifth frame has passed through wgpu and before it enters the video encoder. `?failure=device-loss-once` calls `Device::destroy()` in the fifth processing callback, after four completed frames, and fails that frame explicitly. Both modes are one-shot within an execution context so that the same page can retry without reloading.

The failure path stops further input, settles submitted GPU work, closes or discards decoded frames and prepared bitmaps, drains codec callbacks, releases audio samples, aborts the muxer, and removes the partial origin-private output. Device loss additionally invalidates the cached GPU session. The retained canvas/OffscreenCanvas handle is then used to create a fresh wgpu session and generation for the next explicit job; resources are never reused across generations.

The checked-in harnesses were run as `node tests/chromium-recovery-interop.mjs 9233 8086` and `node tests/firefox-recovery-interop.mjs 8086`. They used the deterministic H.264/AAC fixture and the WebM/VP8/Opus output profile with full output verification. Evidence is retained locally under ignored `tmp/m36-recovery-chromium` and `tmp/m36-recovery-firefox`.

| Browser | Contexts | Injected codec failure | Application-triggered device loss |
|---|---|---|---|
| Edge 153.0.4234.48 on Windows x64 | dedicated worker and explicit main-thread fallback | failed after five GPU-processed frames; no partial download; cleanup 0 frames/0 samples; same-page retry decoded 60/60 frames at 320×180 with non-silent Opus audio; device generation remained 1 | failed after four completed frames; no partial download; cleanup 0/0; retry decoded 60/60 frames and recreated device generation 2 |
| Firefox 156.0.1 on Windows x64 | dedicated worker and explicit main-thread fallback | same cleanup and 60/60-frame retry result; Firefox's bounded ImageBitmap compatibility resources also returned to zero; generation remained 1 | same cleanup and retry result; the worker OffscreenCanvas and main-thread canvas paths both recreated generation 2 |

All eight failure/retry scenarios loaded the retry output for playback at 320×180 and about 2.04 seconds. Full diagnostic re-decode verified the complete frame timeline and non-silent audio. The harnesses also required that no download link be exposed after the failed attempt. Application-visible frame/sample counts and GPU leases returned to zero; these counters do not measure browser/driver-internal allocations.

This is evidence for deterministic cleanup and a clean restart after an application-observed codec exception or application-triggered wgpu device destruction. It is not evidence for recovery from an unprompted GPU driver reset, browser/GPU-process crash, device-lost notification arriving at arbitrary pipeline points, an indefinitely missing codec callback, or automatic checkpoint/resume. Those cases remain explicit limitations outside the verified compatibility matrix.

## Final limits, cleanup, fallback, and stress acceptance, 2026-09-25

The final resize runs added the 50% mode to every enabled profile. Chromium completed 18 size/profile combinations and Firefox completed 12; every one passed full output re-decode, playback geometry, timing, audio where applicable, and zero application-owned cleanup. M3.5 geometry/VFR/HDR/resolution-change regressions and the eight controlled recovery/restart scenarios were rerun after the storage and verification changes and remained green.

The selected Chromium WebGPU adapter reported `maxTextureDimension2D = 16384`. `GpuSession::configure` now rejects input or output dimensions exceeding the actual requested device's 2D texture limit before canvas/texture allocation. Exact `VideoEncoder.isConfigSupported` VP8 probes in the same browser accepted 8192×8192 and rejected 16384×16384, 32768×32768, and 65536×65536. Normal profile probing still gates every resolved output size; no oversized allocation was attempted. The low-resolution deterministic conversion fixture cannot validly request those outputs because the independent no-upscale rule rejects them first.

The existing local 70,439,352-byte input provided the long-run/large-compressed-output stress case. It contains 3,530 H.264 frames, 6,343 decoded audio samples, 1920×1080 display geometry, and 147.3 seconds of media. Chromium converted it to 960×540 WebM/VP8/Opus in 21,465.3 ms. The 32,138,436-byte result was emitted through 17 serialized OPFS writes with a maximum write of exactly 4,194,304 bytes, not accumulated in an application ArrayBuffer, and then fully re-decoded: all 3,530 video frames, complete timestamp/duration timeline (maximum quantization error 708 µs), 7,365 non-silent Opus packets, and midpoint seek passed. Stage counters ended at zero, GPU leases were bounded at four, and the page heartbeat recorded 436 ticks with a maximum 62.3 ms gap during the worker job. This input is user-provided and ignored by Git, so the result is reproducible only where `tmp/user-test/Input.mp4` is present; the checked-in deterministic sparse fixture separately preserves the >256 MiB bounded-input test.

That stress run also exposed valid WebM packets with omitted duration fields before the final packet. Verification now derives an omitted non-final duration from the next packet timestamp, as WebM timing permits, while applying the same 1 ms endpoint tolerance; final coverage remains independently checked against track duration. The M3.5 VFR/non-zero-origin fixtures passed after this correction, preventing the compatibility rule from weakening timestamp validation.

With these results, the M3.6 acceptance gate is passed for the available Firefox/Chromium environments and core M3 browser robustness is complete. M3.7 remains an optional FFmpeg WASM compatibility spike rather than required follow-up work.

## Profile selector before input selection, 2026-09-25

The output selector and Convert button now stay disabled until an input-specific profile probe succeeds. Selecting another file or changing the output size invalidates the previous profile result until the new check completes. This is necessary because the exact check includes the input decoder, audio presence/channel count, frame rate, and resolved output dimensions; a browser-wide encoder probe alone cannot establish whether that file can use an MP4 profile.

The Chromium and Firefox resize harnesses checked the initial disabled state and then loaded the deterministic H.264/AAC fixture. After its probe, both controls became active. H.264 MP4 became selectable in Chromium and remained disabled with its capability reason in Firefox. The existing 18 Chromium and 12 Firefox full re-decode cases then passed unchanged.

The UI now selects Original size at startup. Both browser resize harnesses assert that selection and the resolved 640×360 geometry for the 640×360 fixture before exercising the six-size matrix. The earlier half-size worker, recovery, geometry, and stress harnesses now select 50% explicitly so their historical 320×180 and 960×540 expectations remain meaningful.

## M3.7 — conditional FFmpeg WASM backend spike, 2026-09-25

The measured gap is Firefox 156.0.1 on Windows x64: for the deterministic `fixtures/m2-h264-aac.mp4` input, its exact WebCodecs MP4/H.264/AAC output probe is unavailable, while Chromium/Edge 153.0.4234.48 accepts the same profile. An explicit `?backend=ffmpeg-wasm` choice now enables a constrained software output adapter for that profile. Without that query parameter, WebCodecs remains the default. The common browser job still demuxes and decodes through Mediabunny/WebCodecs, checks the same metadata/timeline, invokes the same Rust/wgpu resize callback, drains the same bounded video/audio pumps, and verifies output. FFmpeg receives the processed frames as RGBA via `VideoFrame.copyTo`; it copies the original MP4 into its virtual filesystem for audio decoding and encodes H.264 with libx264 and AAC. It does **not** inherit the WebCodecs path's no-explicit-readback property. No codec/container or resize choice is silently changed.

The spike is pinned to `@ffmpeg/ffmpeg` 0.12.15, single-thread `@ffmpeg/core` 0.12.10, and build-time esbuild 0.28.2. The locally packaged core is loaded only in explicit FFmpeg mode. The core WASM is 32,232,419 bytes (10,287,551 gzip bytes); core JS is 111,804 bytes (29,696 gzip); the bundled worker is 5,654 bytes (1,773 gzip). The normal `m2.js` bundle is shared by both modes (461,002 decoded bytes in the measured Chromium build); the large core is not fetched during the WebCodecs run. These are file/gzip sizes, not guaranteed network transfer sizes under every server/cache policy. The [ffmpeg.wasm architecture](https://ffmpegwasm.netlify.app/docs/overview/) documents its worker and virtual-filesystem model. The installed `@ffmpeg/core` package declares `GPL-2.0-or-later`; [FFmpeg's licensing documentation](https://www.ffmpeg.org/general.html) also notes that libx264 enables GPL obligations. Redistribution review is required before shipping this core in a production artifact.

| Browser and 320×180 MP4/H.264/AAC route | Conversion (60 frames) | Startup of FFmpeg core | Output/verification | Explicit copies and sampled memory |
|---|---:|---:|---|---|
| Edge 153, WebCodecs default | 287.4 ms (~209 frames/s) | not loaded | 101,661 bytes; 60 H.264 + 95 AAC packets; full re-decode pass, max timeline error 4 µs | 0 explicit pixel readbacks; browser/codec internal copies and peak process memory unknown |
| Edge 153, FFmpeg WASM | 1,088.2 ms (~55 frames/s) | 96.4 ms (locally cached core) | 87,943 bytes; 60 H.264 + 95 AAC packets; full re-decode pass, max timeline error 1 µs | 60 `VideoFrame.copyTo` RGBA readbacks; 13,824,000 raw bytes, 27,648,000-byte estimated maximum overlap of explicit frame buffers during assembly; sampled WASM linear-memory allocation peak 33,554,432 bytes |
| Firefox 156.0.1, WebCodecs default | not run: exact MP4 output profile disabled | not loaded | no MP4 output claimed | — |
| Firefox 156.0.1, FFmpeg WASM | 1,806.0 ms (~33 frames/s) | 320.0 ms (locally cached core) | 88,641 bytes; 60 H.264 + 95 AAC packets; full re-decode pass, max timeline error 1 µs | 60 explicit RGBA readbacks; same 13,824,000 raw bytes and 27,648,000-byte explicit-buffer overlap estimate; sampled WASM linear-memory allocation peak 33,554,432 bytes |

The figures are single local runs, not stable benchmarks. Conversion time includes core startup and the shared GPU path, but excludes the separately reported full verification pass (10.6 ms, 16.4 ms, and 79.0 ms respectively). Chromium's page-level JS heap snapshot after conversion was 28,784,170 bytes for WebCodecs and 47,514,455 bytes for FFmpeg mode; it is **not** a peak and excludes worker/WASM/process allocations. The worker samples allocated WASM linear memory every 25 ms, which is not physical resident-memory use. Total browser-process peak memory and internal browser/GPU copies remain unmeasured, so production resource acceptance is provisional. The selected Chromium WebGPU adapter reported `intel / gen-12lp`; Firefox redacted its adapter identity. CPU pixel uploads into wgpu were not added; the explicit readback, raw-buffer assembly, transfer into the FFmpeg worker, and virtual-filesystem writes are counted separately. The source MP4 copy was 279,635 bytes in this fixture.

Both checked-in real-browser harnesses, `tests/chromium-m37-spike.mjs` and `tests/firefox-m37-spike.mjs`, used a dedicated worker, selected 50% output explicitly, required full browser re-decode, and tested cancellation after conversion began followed by a same-page successful retry. After cancellation, both reported zero application-held frames and samples; the FFmpeg worker is terminated, and the shared GPU leases drain. Independent native `ffprobe` on the three saved MP4s confirmed H.264 320×180/60 frames, mono AAC 48 kHz/95 packets, and durations 2.026667, 2.021333, and 2.021333 seconds. The ignored local evidence/output files are under `tmp/m37-chromium` and `tmp/m37-firefox`. The fixture's provenance and timestamp manifest are in `fixtures/m2-h264-aac.json`.

Build checks passed: `cargo fmt --all --check`, host `cargo test -p media-core -p media-gpu -p media-web` (13 core tests), host and wasm32 `cargo clippy` with `-D warnings`, wasm32 `cargo check` for the web crates, JavaScript syntax checks, and `dx build --platform web`. The Edge and Firefox harnesses were rerun after those code changes, followed by native `ffprobe` on their final output files. This does not validate a release-mode bundle, Firefox main-thread fallback, or other hardware.

The adapter intentionally refuses sources over 64 MiB, raw frames over 128 MiB, non-integer reported frame rates, actual per-frame VFR/timestamp deviations, and inputs without exactly one audio track. It is an in-memory compatibility fallback, not a replacement for M3.6's bounded streaming path. The FFmpeg audio encoder is software-only, and audio is decoded a second time from the source MP4 inside FFmpeg after the shared job's audio samples were drained; that work is included in the measured runtime. Main-thread fallback, large files, all named resize modes, unusual A/V offsets, multi-track inputs, HDR, mid-stream geometry changes, and other browsers/OS/GPU combinations were not validated for this backend. The functional M3.7 profile/cancellation gate passed in the available browsers; unrestricted resource/distribution suitability is **not** claimed. The default WebCodecs path remains preferred and M4 is not blocked by these spike limits.

Follow-up selector correction (2026-09-25): on an FFmpeg-mode capability rejection, the page now retains the selected input metadata and resolved size, and prints the exact reason beside the disabled MP4 option as well as in status. The earlier early-return kept the selector disabled but hid that reason, misleadingly continuing to say “Select an input.” The 64 MiB input bound is an application safety policy, not an FFmpeg/browser limit. The ignored local `tmp/user-test/Input.mp4` is 70,439,352 bytes (about 67.2 MiB), so it exceeds that cap. Its 3,530 frames would also need roughly 7.32 GB of raw RGBA at 960×540 (50%) or 1.83 GB at 480×270 (25%), far beyond the separate 128 MiB cap. Simply raising the source limit would not enable this file safely. Both isolated browser harnesses rechecked that the small deterministic fixture still enables and converts MP4, then selected a deterministic 257.3 MiB sparse-MP4 fixture and verified that the selector remained disabled **with** the 64 MiB reason and source metadata visible. Long-file FFmpeg support requires a different streaming frame/virtual-filesystem bridge and is not claimed by this spike.
