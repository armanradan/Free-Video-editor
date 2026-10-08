# Diaxus GPU video converter — M4 native headless harness

M5 also provides a Dioxus Native/Blitz app with dark/light themes, GPU selection and a compact inline source/output player. Selecting a source automatically loads its preview paused after 600 ms without path changes; Play/Pause, Mute and a seek timeline sit below the image. Expand preview opens a separate maximized, bordered preview window while keeping the main controls available. Return inline, closing that window or Escape preserves the shared playback session; closing the main window exits the app. Expanded source preview also offers Before/After. Click the inline player to enable Space and arrow shortcuts; clicking a path field or pressing Tab relinquishes them. Preview is independently CPU-decoded SDR display, not hardware codec-surface sharing. Each window owns its own presenter textures, sharing only the cached CPU frame and playback service. Enlarging the current 640×360 preview does not add source detail or guarantee selected-output geometry/10-bit display fidelity. See the interop report for tested behavior and remaining M5 work.

Dioxus Web accepts MP4 with H.264/AVC or H.265/HEVC video and applies shared Rust/wgpu configurable resizing. Output profiles are WebM/VP8/Opus, explicit video-only WebM, capability-gated MP4/H.264/AAC, and capability-gated MP4/H.265/AAC. Browser codecs, container handling, and GPU processing run together in a dedicated worker when supported; otherwise the UI reports the main-thread compatibility fallback and its reason.

Browser and native controls now offer source/resolution/FPS-aware Smaller / Recommended / Higher bitrate, custom decimal Mbps and estimated output size. Output FPS defaults to Original; fixed rates duplicate/drop frames without changing playback speed or audio timing. Source inspection labels FPS as CFR or VFR-average. VBR targets and file-size estimates are approximate, not strict caps. Browser settings apply to WebCodecs and the optional FFmpeg WASM backend; Original VFR still requires WebCodecs, while fixed-FPS VFR can use the FFmpeg raw bridge.

Settings types, custom-Mbps/FPS parsing and track-coverage estimation are shared in `media-core`; FPS/bitrate preset catalogs are shared in `ui`. Codec execution and platform layouts remain separate. Native CLI `--bitrate` also accepts decimal Mbps, such as `--bitrate 3.5`.

Browser and native now include brightness, contrast and saturation, Reset color and source-preview Before/After. Slider updates reuse one original frame; Before changes only the preview, not saved export settings. Browser WebCodecs/FFmpeg WASM and native shared-GPU routes apply the shared shader before output-FPS resampling. Native CPU FFmpeg uses the corresponding RGB16 expression with explicit BT.709↔sRGB conversion, including Main 10 output. Native **Color adjustments** expands a collapsed inline box with compact mouse/keyboard tracks and paused source preview; previewing a converted output bypasses adjustments. Direct NVIDIA rejects non-neutral settings: choose CPU or Shared GPU explicitly, with no automatic fallback. Native default-size UI smoke checks passed; broader accessibility/DPI, exact cross-route preview parity and CLAHE remain pending. See `docs/color-adjustments-plan.md` and `docs/interop-report.md`.

Native CLI example: `native-convert convert --input INPUT.mp4 --output NEW.mp4 --route direct --brightness 10 --contrast 150 --saturation 0`. Ranges/defaults match the shared controls. Run `node tests/native-color.mjs` for the explicit CPU/shared-GPU/NVIDIA-staged and Main 10 export gate; it requires FFmpeg/FFprobe and the NVIDIA test hardware. Native preview needs FFmpeg zscale. Renderer-ABI diagnostic: `cargo test --locked -p converter-native preview_gpu -- --include-ignored --nocapture`. These tests do not substitute for live native UI/layout checks.

Run `node tests/browser-color-preview.mjs chromium 8084` and `node tests/browser-color-preview.mjs firefox 8084` against the isolated debug sessions described below to check paused-preview pixels/cache reuse and adjusted exports. The same isolated server and FFmpeg tools as the bitrate/FPS gate are required. Evidence remains under ignored `tmp/color-preview`. The test-only native GPU reference gate is `cargo test --locked -p media-native color_tests -- --include-ignored --nocapture`.

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

## Native M4 headless harness

M5 additionally provides `--route wgpu-nvidia`: NVDEC → CPU-staged RGBA → shared WGSL resize → CPU-staged NV12 → NVENC. The three original routes described below remain available; this fourth route is 8-bit SDR only and explicitly counts codec transfers in addition to the wgpu uploads/readbacks.

The Windows native correctness harness uses installed FFmpeg/FFprobe 8.0.1 on `PATH`. Three explicit routes share `media-core` resize/profile policy: `direct` (software FFmpeg, CLI default), `nvidia` (CUDA decode/resize → NVENC), and `wgpu` (software decode → **shared `media-gpu` WGSL resize** → readback → software encode). All preserve tested VFR and non-negative A/V offsets, verifying every decoded output timestamp before publishing. The wgpu bridge streams timestamped RGBA through Matroska in memory, with no raw-frame disk spool, but still counts one CPU→GPU upload and GPU→CPU readback per frame. NVIDIA keeps video surfaces inside FFmpeg/CUDA without an application-managed pixel readback; audio and source/output inspection still use CPU. NVIDIA GPU selection maps a uniquely matching wgpu vendor/device/name to an NVIDIA UUID, never an enumeration index; ambiguous identity and hardware failures are errors, not silent CPU fallback. NVIDIA and wgpu require square pixels and no display transform; software FFmpeg also supports the tested crop/SAR/orientation fixture. H.264/AAC is available on all routes, HEVC Main 10/AAC on software/NVIDIA only. HDR, wide color, full range, negative origins and multiple audio tracks remain unsupported. Tiny HEVC inputs can be below NVDEC's minimum dimensions; choose software explicitly if needed. Ctrl-C and `--cancel-after-ms N` request cooperative cancellation and clean partial outputs. The M5 app now has independent SDR playback; hardware codec-surface sharing with wgpu remains unimplemented.

The native GPU route also detects wgpu device loss as a failed job and discards its unpublished output; an application-triggered `Device::destroy()` test passes cleanup and a fresh same-process retry. This does not establish spontaneous driver-loss or hung-driver recovery.

M5 has started with a separate Dioxus Native/Blitz window. Run `cargo run --locked -p converter-native` with FFmpeg and FFprobe on `PATH`. Choose input/output MP4 paths, inspect the input, then select Direct FFmpeg · NVIDIA, Direct FFmpeg · CPU, Shared GPU · NVIDIA, or Shared GPU · software codecs. Shared NVIDIA combines NVDEC, shared WGSL resize and NVENC, but uses explicit CPU-staged pixel transfers; it is not zero-copy and is limited to 8-bit SDR. The UI initially selects direct NVIDIA when discovery finds an NVIDIA device (discovery is not proof that every codec/geometry works). Successful input inspection is cached for the unchanged file within this session; first inspection still fully decodes on CPU, and output verification always fully decodes. The UI displays Inspecting/Preparing/Converting/Verifying/Publishing and reports total, preflight, conversion and verification times separately. The window starts in dark mode and offers a light/dark toggle, grouped settings, resize/profile buttons, cancellation, and GPU choices for NVIDIA/shared routes; software FFmpeg ignores that preference. GPU preference is saved in `%LOCALAPPDATA%/Diaxus/gpu-preference.json` on Windows (override with `DIAXUS_GPU_PREFERENCE`); choosing Automatic clears that file. An unavailable saved adapter uses Automatic with a visible warning. Theme and inspection cache remain session-local. Closing the last window cancels and waits for active job cleanup before process exit; forced process termination and hung GPU drivers are not covered.

Source Preview and Preview last output open an independent SDR video player inside Blitz. Use Play/Pause, drag the timeline, or focus it and use Left/Right (5 seconds), Home/End, and Space. Elapsed/total time and Mute are included. Continuous FFmpeg preview decoding samples 15 fps into a bounded 640×360 RGBA8 slot and Blitz GPU texture; audio goes through a bounded 250 ms PCM queue to the native output device. Preview may drop late frames without changing conversion. Seeking/muting restarts only the preview, not conversion. Missing audio-device errors are explicit; choose Mute to retry video-only playback. HDR is rejected and 10-bit SDR is reduced to 8-bit for display only. This player does not establish exact frame stepping, physical A/V synchronization or GPU codec-surface sharing.

Native bitrate defaults to Recommended. Smaller / Recommended / Higher display adaptive Mbps targets based on the inspected source video bitrate, output dimensions, output FPS and codec; Custom accepts 0.25–120 Mbps, including decimals. Recommendations are starting heuristics, not quality guarantees. Inspect shows source FPS (CFR or VFR average) and an approximate output size, updated when settings change. The compact FPS chooser defaults to Original (preserve source timestamps, including VFR); selecting a fixed rate duplicates/drops frames without changing playback speed or audio timing. Common fractional rates use exact 24000/1001, 30000/1001 and 60000/1001 rationals. Audio and a small muxing allowance are included in the size estimate; VBR can differ substantially. CLI: `--bitrate smaller|recommended|higher` or integer Mbps (e.g. `--bitrate 4`), plus `--fps original|NUM[/DEN]` (e.g. `--fps 30000/1001`). Omitting bitrate retains legacy CRF/CQ20; omitting FPS preserves source timing. Browser controls are unchanged. Run `node tests/native-bitrate.mjs` and `node tests/native-frame-rate.mjs` for explicit four-route NVIDIA gates (FFmpeg/CUDA/NVENC required).

```powershell
cargo run --locked -p media-native --bin native-convert -- list-gpus
cargo run --locked -p media-native --bin native-convert -- convert --input fixtures/m2-h264-aac.mp4 --output tmp/m4/direct-output.mp4 --resize 50
cargo run --locked -p media-native --bin native-convert -- convert --input fixtures/m4-10bit-sdr.mp4 --output tmp/m4/main10-output.mp4 --profile mp4-h265-main10-aac --resize 50
cargo run --locked -p media-native --bin native-convert -- save-gpu --adapter-key 'Dx12:10de:25a2:NVIDIA GeForce RTX 3050 Laptop GPU' --preference tmp/m4/gpu-preference.json
cargo run --locked -p media-native --bin native-convert -- convert --input fixtures/m2-h264-aac.mp4 --output tmp/m4/wgpu-output.mp4 --route wgpu --resize 50 --preference tmp/m4/gpu-preference.json
node tests/native-m4-interop.mjs
node tests/native-m5-hardware.mjs
cargo test -p media-native --lib cancel_active_jobs_and_retry_in_same_process -- --ignored --nocapture
cargo test -p media-native --lib injected_device_loss_cleans_job_and_allows_fresh_gpu_job -- --ignored --nocapture
node tests/native-m4-release-bench.mjs
node --test tests/native-m5-release-bench.test.mjs
node tests/native-m5-release-bench.mjs
```

`list-gpus` shows the adapter keys available on your machine; use one of those keys in place of the example. A saved preference is re-resolved on each run, with an explicit reported fallback if missing. The harness refuses to overwrite an output. The ignored lifecycle test also exercises the headless `NativeSession` adapter switch when both a discrete and an integrated GPU are present: it cancels an active job, waits for cleanup, then runs on the second GPU with a new generation. The release benchmark generates seeded 1080p/4K synthetic A/V inputs and checks both routes; its outputs and evidence stay under ignored `tmp/m4/release-bench`. Neither facility adds a native UI. Details and measured limitations are in the interop report.

The M5 release benchmark compares `direct`, `nvidia`, `wgpu`, and `wgpu-nvidia` serially on generated 1080p/180-frame and 4K/90-frame sources. It requires a compatible NVIDIA device and hardware-enabled FFmpeg; unavailable hardware is a failure, not a passing skip. One warm-up per route/workload is excluded, followed by three timed rounds in rotated order. Every output passes independent video/audio/timestamp/color checks; reports include stage-time ranges/medians, transfer counts, output bytes and first-timed-output PSNR/SSIM against an uncompressed bilinear reference. CPU CRF20 and NVIDIA CQ20 are not matched-rate or matched-quality settings, so these are current-route baselines, not encoder-efficiency or universal-speedup claims. Use `--runs=4` for four timed rounds or `--adapter-key="EXACT KEY"` to select another uniquely resolvable NVIDIA descriptor. Fixtures, hashes, commands and evidence stay under ignored `tmp/m5/release-bench`; failed runs retain their artifacts for diagnosis. CPU/power/VRAM and opaque codec/driver memory remain unmeasured. Close any playing previews before timing comparisons.

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

See [fixture provenance](fixtures/README.md), [architecture and remaining milestones](docs/architecture.md), and [verified results and remaining limitations](docs/interop-report.md). M3.6 is complete for the tested Firefox/Chromium profile matrix; M3.7 and M3.8 add an optional FFmpeg WASM compatibility route. This does not claim universal browser/HDR support, codec hardware execution, real-time throughput, spontaneous driver/process-loss recovery, or stable memory outside application-visible ownership counters. M4 is complete for the tested Windows headless direct-FFmpeg subset; the shared-wgpu route has explicit limits. Native Dioxus/Blitz UI and hardware codec-surface interoperability are M5 work.

The browser bitrate/FPS gate requires the isolated debug browsers above, FFmpeg/FFprobe, and an app served on port 8084 with `--cross-origin-policy` for FFmpeg WASM:

```powershell
node tests/browser-output-settings.mjs chromium 8084
node tests/browser-output-settings.mjs firefox 8084
```

It verifies enabled-profile FPS grids, adaptive labels, custom bitrate/size changes, VFR, cancellation/retry, FFmpeg WASM, short/no-audio EOF and main-thread fallback. Media/evidence stay under ignored `tmp/browser-output-settings`. Firefox needs a normal isolated window for WebGPU on the tested machine; headless canvas support was unavailable.
