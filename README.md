# Diaxus GPU video converter — M4 native harness in progress

Dioxus Web accepts MP4 with H.264/AVC or H.265/HEVC video and applies shared Rust/wgpu configurable resizing. Output profiles are WebM/VP8/Opus, explicit video-only WebM, capability-gated MP4/H.264/AAC, and capability-gated MP4/H.265/AAC. Browser codecs, container handling, and GPU processing run together in a dedicated worker when supported; otherwise the UI reports the main-thread compatibility fallback and its reason.

M3.1 through M3.6 pass their acceptance gates in the available Firefox/Chromium environments. The app defaults to original-size output and provides user-selectable 75%, 50%, 25%, HD 720p, Full HD 1080p, 2K-width, QHD 1440p, 4K UHD, and exact-size output plus bounded browser input/output. Named modes are aspect-preserving maximum bounds; exact sizing preserves aspect ratio by default and exposes an explicit stretch option. Every mode rejects implicit upscaling, displays codec-safe even dimensions before conversion, checks the actual WebGPU device limit, and re-probes every output profile for that exact size. Selecting a file immediately displays its display/coded resolution, codec string, duration, average frame rate, frame count, audio details, and file size. Deterministic codec-failure and WebGPU device-loss tests verify cleanup and same-page restart.

`media-core` owns platform-neutral resize and media policy, `media-gpu` owns the shared processor/WGSL, `media-web` owns browser resources and worker transport, and `ui` contains reusable controls. The M1 deterministic regression remains available. M3.5 added crop/PAR/orientation handling, an explicit BT.709/sRGB SDR policy, VFR/non-zero-origin preservation checks, and clear HDR/resolution-change rejection.

## Prerequisites

- Pinned Rust 1.98.1 with `wasm32-unknown-unknown`, rustfmt, and Clippy (edition 2024).
- A host linker. The repository currently pins Windows GNU and requires MinGW; MSVC instead requires Visual Studio C++ Build Tools and a matching toolchain override.
- Dioxus CLI 0.7.10 (`dx`), Node.js/npm, and installed package dependencies.
- A secure browser context with WebGPU and WebCodecs (`localhost` qualifies).

## Build and run

```powershell
Set-Location apps/web
npm ci
dx serve --web --locked
```

The toolchain file selects Rust; no `RUSTUP_TOOLCHAIN` environment override is needed. Select an MP4, choose a resize mode, an available profile, and a codec-acceleration preference, then convert. The profile selector and Convert button become active after the selected file's codec, audio, frame rate, and resolved output size have been checked; MP4 availability cannot be established accurately before that inspection. The resolved codec-safe output size is shown before conversion. `Compatibility baseline` is the default. `Prefer hardware` is used only if the exact decoder and encoder configuration both pass; otherwise the result shows the reason for falling back to the unchanged codec/profile. The execution label reports worker/fallback mode. For a deliberate fallback comparison open the same URL with `?execution=main`; remove it and reload to use automatic worker selection.

The optional `?backend=ffmpeg-wasm` MP4/H.264/AAC route uses M3.8's bounded live raw-frame ring. Start its development server with `dx serve --web --locked --cross-origin-policy`; production hosting likewise needs COOP `same-origin` and COEP `require-corp` (or a tested equivalent) for shared memory. Without cross-origin isolation, the UI disables FFmpeg and explains why; the default WebCodecs path does not require it. FFmpeg still performs an explicit RGBA GPU readback per frame, needs OPFS for the compressed output, and uses a large GPL-licensed WASM core. See the architecture and interop report before distributing it.

Worker code uses the same wasm bundle produced by `dx`; there is no manual worker build. Keep the complete generated `public` directory, including wasm snippets, when deploying. Input uses an 8 MiB cache and compressed output streams with backpressure in 4 MiB chunks to origin-private file storage before its disk-backed `File` is exposed through the download link. The current temporary output is retained only while its download URL is usable, removed when replaced, and explicitly cleaned on page exit. If the browser cannot provide origin-private writable storage, the status clearly reports a memory fallback whose input remains capped at 256 MiB. `?output=memory` deliberately exercises that fallback. Successful GPU processing still does not prove hardware codec execution.

Normal conversions stop timing after encoder/muxer finalization and do not re-decode the completed file. Add `?verify=full` to the app URL for the diagnostic interoperability path, which separately reports its re-decode/seek/audio-check time. The automated browser harnesses enable full verification by default; set `VERIFY_OUTPUT=skip` to exercise the normal UI path.

## Native M4 headless harness (initial slice)

The Windows native correctness harness uses installed FFmpeg/FFprobe 8.0.1 on `PATH`. It offers a direct-FFmpeg software route and a comparison route that decodes to RGBA with FFmpeg, applies the **same `media-gpu` WGSL resize**, then reads back RGBA to FFmpeg's software encoder. Both use `media-core` resize/profile policy and produce MP4/H.264/AAC. Direct FFmpeg is the intended native default for resize-only and FFmpeg-expressible effects; the current CLI requires an explicit route while M4 is in progress. Keep wgpu for custom effects shared with the web app or a GPU effect chain that proves worthwhile in complete conversion benchmarks. The current wgpu route explicitly counts CPU→GPU and GPU→CPU frame bytes; it is not zero-copy hardware codec interoperability. It currently accepts only zero-origin, square-pixel, unrotated, 8-bit CFR sources with one video and at most one audio track. Ctrl-C requests cooperative cancellation and reaps FFmpeg children; `--cancel-after-ms N` is a deterministic cancellation-test option. Native Dioxus/Blitz UI, VFR/rotation/HDR, and direct codec-surface sharing are not implemented yet.

```powershell
cargo run --locked -p media-native --bin native-convert -- list-gpus
cargo run --locked -p media-native --bin native-convert -- convert --input fixtures/m2-h264-aac.mp4 --output tmp/m4/direct-output.mp4 --route direct --resize 50
cargo run --locked -p media-native --bin native-convert -- save-gpu --adapter-key 'Dx12:10de:25a2:NVIDIA GeForce RTX 3050 Laptop GPU' --preference tmp/m4/gpu-preference.json
cargo run --locked -p media-native --bin native-convert -- convert --input fixtures/m2-h264-aac.mp4 --output tmp/m4/wgpu-output.mp4 --route wgpu --resize 50 --preference tmp/m4/gpu-preference.json
node tests/native-m4-interop.mjs
cargo test -p media-native --lib cancel_active_jobs_and_retry_in_same_process -- --ignored --nocapture
```

`list-gpus` shows the adapter keys available on your machine; use one of those keys in place of the example. A saved preference is re-resolved on each run, with an explicit reported fallback if missing. The harness refuses to overwrite an output. Details and measured limitations are in the interop report.

## Checks

From the repository root:

```powershell
cargo fmt --all -- --check
cargo check -p media-core -p media-gpu
cargo test -p media-core -p media-gpu
cargo clippy -p media-core -p media-gpu -- -D warnings
cargo check --workspace --target wasm32-unknown-unknown
cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings
node --test tests/worker-transport.test.mjs
Set-Location apps/web
dx build --web --locked
```

The real-Firefox harness requires an isolated Firefox instance exposing WebDriver BiDi on port 9226 and `dx serve --web --locked --port 8084`. It does not launch a browser or alter browser preferences. Run from the repository root:

```powershell
node tests/firefox-worker-interop.mjs worker
node tests/firefox-worker-interop.mjs main
node tests/firefox-resize-interop.mjs 8084
```

It tests every enabled profile, cancellation/restart, output loading/seeking, window timer responsiveness, M1 cancellation and five repeated M1 marker checks. Generated outputs/evidence go under ignored `tmp/m33-*`. The transport unit tests simulate unsupported startup, cancellation, stale messages, and worker crashes; they are not browser interoperability evidence.

For Chromium validation, start an isolated browser debugging session yourself (the harness never launches a browser). For example, from the repository root on Windows with Edge installed:

```powershell
& 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe' --remote-debugging-port=9227 --user-data-dir="$PWD\tmp\edge-m33-validation" --no-first-run about:blank
```

With the app served on port 8084, run:

```powershell
node tests/chromium-worker-interop.mjs worker
node tests/chromium-worker-interop.mjs main
node tests/chromium-worker-interop.mjs fallback
node tests/inspect-chromium-outputs.mjs
node tests/chromium-long-run.mjs
node tests/chromium-m35-interop.mjs
node tests/chromium-resize-interop.mjs 9227 8084
node tests/chromium-hevc-interop.mjs 9227 8084
node tests/chromium-recovery-interop.mjs 9227 8084
```

The independent output inspector requires FFmpeg/FFprobe on PATH. The Chromium harness creates and closes only its own test tab; `fallback` injects a test-only worker-constructor failure to exercise automatic fallback. It tests both acceleration requests for every enabled profile. The focused HEVC harness validates HEVC input decoding and conversion and either fully verifies HEVC output or records the exact failed capability probe. The long-run harness uses `tmp/user-test/Input.mp4`, keeps the produced Blob in the browser, and records evidence under ignored `tmp/m34-long`; it requires that local test input to exist. Other generated outputs/evidence remain under ignored `tmp/m33-chromium-*`. Close the isolated debugging browser when finished; do not use your everyday profile for these tests.

The M3.5 harnesses run the geometry/color and VFR/non-zero-origin fixtures through every enabled profile, sample decoded output corner colors, verify every video packet timestamp/duration within the container timebase, and exercise HDR and mid-stream resolution-change failures:

```powershell
node tests/chromium-m35-interop.mjs 9227 8084
node tests/firefox-m35-interop.mjs 8084
node tests/chromium-resize-interop.mjs 9227 8084
node tests/firefox-resize-interop.mjs 8084
node tests/firefox-recovery-interop.mjs 8084
```

The M3.6 harnesses run the deterministic 640×360 fixture through original, 75%, 50%, 25%, exact aspect-locked, and exact stretched modes in every enabled profile. They verify source metadata display, availability of all named presets, the named-preset no-upscale boundary, displayed and decoded output dimensions, disk-backed full re-decode, cancellation after changing size, zero-resource cleanup, page-exit temporary-file cleanup, and clear no-upscale rejection. They also exercise the deterministic HEVC/AAC input, generate an ignored sparse MP4 above 256 MiB, convert it through the bounded path in both browsers, and verify forced and API-unavailable memory fallback behavior in Chromium. The recovery harnesses exercise worker and explicit main-thread paths with test-only `?failure=codec-once` and `?failure=device-loss-once` modes. They require the failed job to expose no partial download, return application-owned resources to zero, and complete a full re-decode after retry; the device-loss case also requires a new device generation.

See [fixture provenance](fixtures/README.md), [architecture and remaining milestones](docs/architecture.md), and [verified results and remaining limitations](docs/interop-report.md). M3.6 is complete for the tested Firefox/Chromium profile matrix; M3.7 and M3.8 add an optional FFmpeg WASM compatibility route. This does not claim universal browser/HDR support, codec hardware execution, real-time throughput, spontaneous driver/process-loss recovery, or stable memory outside application-visible ownership counters. M4's initial headless native slice is in progress; its full acceptance gate has not passed.
