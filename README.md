# Diaxus Video Converter

Resize, compress and adjust videos using a native Windows app or a browser app. Both versions offer a source preview, configurable resolution and frame rate, adaptive bitrate recommendations, brightness/contrast/saturation controls, and optional CLAHE local contrast enhancement.

The native app uses installed FFmpeg and can use NVIDIA hardware codecs. The browser app uses WebCodecs by default, with an optional FFmpeg WASM encoder. Codec availability depends on your machine, browser and selected video.

## Experimental AI model setup

Both apps have a collapsed **AI Upscaling** section and start with AI **Off**.
The normal browser build enables ONNX Runtime **2x/4x** on the tested Windows
Edge 156 / Intel gen-12lp or NVIDIA Ampere WebGPU routes, using a secure, cross-origin-isolated
origin. Other browser/GPU combinations show an unvalidated reason. Native
2x/4x remain disabled pending ordinary-window preview/export acceptance.

For browser AI, inspect an opaque 8-bit SDR source no larger than **577×579**,
import the exact model below, explicitly download the runtime, then select
**2x** or **4x**. AI determines output size; **Off** restores your saved ordinary
resize. **Reset color** keeps AI selected, and **Before** affects preview only.
H.264/AAC exports have evidence on both GPUs. NVIDIA AI WebM remains unavailable
pending independent endpoint verification; the earlier Intel VP8 evidence is
unchanged. AI HEVC is unavailable. Codec probes
still apply, and FFmpeg WASM retains its fixed-FPS/temporary-storage limits.
2x uses the model's 4x result followed by shared GPU downscaling.

This image model can smooth fine detail, reconstruct text/texture and flicker.
On the evaluated Intel GPU, 64-frame, 192×80, 5.33-second clips took roughly
38–47 seconds with AI in the isolated diagnostic comparison. Preview is sampled
and may be slow. No real-time, fidelity improvement or zero-copy claim is made.
See [the acceptance record](docs/interop-report.md#browser-ai-controls-enabled--2026-10-25).

For faster AI on this evaluated machine, start the local server, then run:

```powershell
& ./tools/ai-upscale/launch-nvidia-browser.ps1
```

This opens a separate Edge profile with an explicit high-performance GPU launch
option; it changes no Windows graphics settings or default browser profile.
Load a source and confirm **Selected GPU: nvidia / ampere**, then explicitly
choose **MP4/H.264/AAC** and 2x/4x. The launcher downloads nothing. Model/runtime
setup belongs to this separate profile; existing verified cache is reused.
The evaluated NVIDIA route converted the same 64-frame 192×80 clips in
**3.26–3.68 seconds**, compared with **37.53–46.92 seconds** on Intel (about
10–14× faster in those tests). Cold model setup adds roughly 0.6 seconds.
This result is specific to these clips and launch configuration, not a speed or
real-time playback guarantee for larger inputs. Edge's normal high-performance
adapter request still selected Intel; the launcher uses a separate process so
its GPU argument takes effect.

In the browser, choose the validated ONNX file and click **Import model**.
**Check stored model** rechecks its hash. **Download runtime** explicitly
acquires the pinned ONNX Runtime Web assets; it supports cancellation and retry.
Separate removal buttons clear the stored model/runtime. Startup and AI Off
perform no automatic acquisition; videos are never uploaded by these actions.

In native, open AI Upscaling, choose a local cache folder, then import/check/remove
the model. Setup uses a dedicated view; **Done** returns to the retained source
and settings. Preview pauses while setup is open. **Inspect runtime folder** reads
architecture/version metadata from an existing installation and loads no DLL.
Native runtime/helper configuration and AI controls remain gated. No SDK, runtime,
driver or toolchain installation or license acceptance is automatic.

Browser/native inference, video and preview adapters have separate diagnostic
evidence. Native preview uses a counted bounded display image between independent
processing and renderer devices. Native runtime paths can also be inspected from
the command line without loading a library:

```powershell
cargo run --locked -p media-native --bin native-convert -- ai-runtime-status --ort-directory PATH_TO_ORT_DIRECTORY --rtx-sdk PATH_TO_SDK
```

With the evaluated local assets, developers can explicitly launch
`converter-native --ai-preview-check` to test real 2x/4x preview in the native
window on the tiny CC0 fixture. Before/After, color edits and the player work in
that diagnostic; export is disabled. Required asset/device/scale variables and
test commands are documented in [the diagnostic runner guide](tools/ai-upscale/README.md#native-window-preview-diagnostic).
The diagnostic also accepts an explicitly configured local RTX SDK helper via
`DIAXUS_AI_PREVIEW_BACKEND=rtx`. Actual staged RTX service/renderer pixel and
lifecycle tests pass on the evaluated machine; an RTX window inspection and
ordinary-launch preview/export acceptance remain pending. Fresh seeks restart
the SDK helper, so interactive performance is not yet accepted.
Ordinary native launch still starts with AI Off and grants no AI capability.
Its AI panel now imports an explicit local configuration for either backend and
shows the corresponding model/runtime description. Import does not execute a
runtime or enable controls. Native preview-to-export handoff tests pass for RTX
and ONNX at both scales through the shared configuration entry point; ordinary
window and release acceptance remain pending. See the runner guide above for
the configuration example and independently verified handoff tests.
The native release comparison now has96 independently verified RTX/ONNX/bilinear
exports. On the three5.33-second192×80 clips, total RTX time is5.18–6.19seconds,
including roughly3seconds of helper startup. Fresh seeks currently repeat that
startup; it remains a preview bottleneck. These measurements do not establish
real-time preview or support above the192×108 RTX input cap. See
[the measured comparison](docs/interop-report.md#native-rtx-qualityperformance-comparison--2026-10-30).

Both paths are optional. `DIAXUS_ORT_DIRECTORY` and `DIAXUS_RTX_SDK` are explicit
environment alternatives; command arguments take precedence. Discovery checks x64
PE architecture/version metadata and the assessed SDK layout, reports missing or
incompatible components, and keeps inference unavailable. It installs nothing and
does not accept SDK terms. File-version metadata is not proof of API/provider or
GPU support. The standalone RTX SDK D3D11 feasibility probe now passes 2x/4x
on the evaluated RTX 3050 after explicit local-evaluation consent. The optional
bounded native helper also passes exact NVIDIA sample equivalence at2x/4x.
The explicit native export pilot now passes both scales with Original/15/60 FPS,
VFR/audio verification and byte-identical repeats. Input is currently capped at
192x108. The staged RTX CLAHE/manual bridge and active pipe cancellation,
timeout/crash cleanup and retry gates also pass. Native preview/UI acceptance,
full release quality/performance and redistribution review remain pending;
ordinary RTX controls are unavailable. See [the helper guide](tools/ai-upscale/native-rtx-helper/README.md).

The validated model setup uses **explicit local file import**, chosen during the
feasibility work. Import the exact 4,866,413-byte `model.onnx` generated by the
[pinned reference recipe](tools/ai-upscale/README.md); arbitrary models reject.
The shared manifest records the source, BSD-3-Clause license and SHA-256.

Developer-only native cache commands (these do not enable inference):

```powershell
cargo run --locked -p media-native --bin native-convert -- import-ai-model --model tmp/ai-upscale/reference/model.onnx --cache tmp/ai-upscale/model-cache
cargo run --locked -p media-native --bin native-convert -- ai-model-status --cache tmp/ai-upscale/model-cache
cargo run --locked -p media-native --bin native-convert -- remove-ai-model --cache tmp/ai-upscale/model-cache
```

Native cache reads recheck integrity, valid entries work offline, and corrupt cache
entries require explicit removal/reimport. The caller chooses the cache directory;
no inference runtime, SDK, toolchain or driver is installed automatically.

Browser provisioning APIs likewise import the local model into CacheStorage.
Optional ONNX Runtime Web 1.30.0 assets total **26,901,320 bytes**, under MIT, and
require a separate explicit acquisition action showing their URLs/license/size.
Startup and AI Off do not acquire/load these assets. Model/runtime cache reads
verify hashes; valid caches work offline. Quota/network/corruption errors require
explicit recovery. Model and runtime caches can be removed separately; removal
does not unload active compiled code. Source video is never uploaded by these APIs.
Browser setup, ordinary controls and the limited route above are accepted;
ordinary native AI preview/export acceptance remains pending.
The developer-only native `ai-upscale-pilot` feature adds `ai-video-pilot` for
DirectML inference plus shared-wgpu/software-codec conversion, with explicit
helper/runtime/cache/GPU configuration. Its NVDEC/NVENC AI route is unavailable:
the actual Original-VFR test failed timestamp preservation. Build/protocol and
reproduction are in [the helper guide](tools/ai-upscale/native-onnx-helper/README.md).
No AI controls or normal conversion capability are enabled by that feature. See
[verified results and limitations](docs/interop-report.md).

## Get the source

If you do not already have a checkout:

```powershell
git clone https://github.com/armanradan/Free-Video-editor.git
Set-Location Free-Video-editor
```

## Build prerequisites

The instructions below target Windows PowerShell, the currently tested development environment. Run commands from the repository root unless a different directory is shown.

- Install Rust through rustup. The repository selects **Rust 1.98.1, Windows GNU, edition 2024** through `rust-toolchain.toml`; no `RUSTUP_TOOLCHAIN` environment variable is needed.
- Have a compatible Windows GNU/MinGW linker available. If you choose an MSVC toolchain instead, install Visual Studio C++ Build Tools and explicitly select the matching Rust toolchain; VS Code alone does not supply the linker. These instructions use the repository's GNU toolchain.
- A graphics driver supporting the app's GPU backend is needed for preview and shared-GPU processing. NVIDIA routes additionally require a compatible NVIDIA GPU and driver.
- Native only: install **FFmpeg and FFprobe** on `PATH`. The tested version is **8.0.1**. Use a build containing `zscale`, `libx264` and `libx265`; NVIDIA routes also need CUDA/NVDEC/NVENC support.
- Browser only: install Node.js/npm and **Dioxus CLI 0.7.10**, matching the project's Dioxus version. Use a browser with WebGPU and WebCodecs support.

Install the pinned Rust tools:

```powershell
rustup toolchain install 1.98.1-x86_64-pc-windows-gnu --profile minimal --component rustfmt --component clippy
rustup target add wasm32-unknown-unknown --toolchain 1.98.1-x86_64-pc-windows-gnu
```

For the browser build, install the matching CLI:

```powershell
cargo install dioxus-cli --version 0.7.10 --locked
dx --version
```

The native app does not need Node.js, npm or the Dioxus CLI. Other operating systems are not currently covered by these build/run instructions.

## Native app

### Run locally

Check that both FFmpeg tools are available:

```powershell
ffmpeg -version
ffprobe -version
```

From the repository root:

```powershell
cargo run --locked -p converter-native
```

### Build an optimized executable

```powershell
cargo build --release --locked -p converter-native
cargo run --release --locked -p converter-native
```

The executable is `converter-native.exe` in Cargo's `release` output directory (normally `target/release`; `CARGO_TARGET_DIR` can change it). FFmpeg and FFprobe must still be installed and available at runtime. Close a running converter window before rebuilding: Windows can lock its executable.

### Convert a video

1. Browse for an input MP4. Its source preview opens automatically, paused.
2. Click **Inspect** to display video/audio metadata and enable source-aware bitrate estimates and CLAHE. The first inspection can take time; it checks the full video.
3. Choose a new output MP4 path. Existing files are not overwritten.
4. Choose output size, H.264 8-bit or H.265 10-bit SDR, frame rate and bitrate.
5. Choose a processing route from the table below. Open **Color adjustments** if needed.
6. Click **Convert video**. You can cancel an active job. Completion includes output verification, so it can take longer than encoding alone.
7. Use **Preview last output** to review a successfully converted file.

| Processing route | When to use it | Important restriction |
|---|---|---|
| Direct FFmpeg · NVIDIA | NVIDIA hardware decoding, resizing and encoding | Non-neutral color controls and CLAHE are unsupported |
| Direct FFmpeg · CPU | Software conversion, including the three color sliders and 10-bit output | CLAHE is unsupported |
| Shared GPU · NVIDIA | GPU image adjustments/CLAHE with NVIDIA decoding and encoding | 8-bit output only; transfers through CPU memory |
| Shared GPU · software codecs | GPU image adjustments/CLAHE with software codecs | 8-bit output only; transfers through CPU memory |

Hardware routes can fail for unsupported input codecs or geometry; the app does not silently switch to CPU. Choose a different route explicitly if necessary. Shared GPU is not guaranteed to be faster than direct FFmpeg.

**Change GPU…** selects the preferred processing adapter. The preference is saved between sessions; it does not select the independent preview renderer's GPU. Direct CPU conversion does not use this preference. NVIDIA routes require a uniquely matching NVIDIA device.

The window starts in dark mode and includes a light-mode toggle.

## Browser app

### Run locally

From the repository root:

```powershell
Set-Location apps/web
npm ci
npm run build:ffmpeg-worker
dx serve --web --locked --addr 127.0.0.1 --port 8084 --cross-origin-policy
```

For a stable test of an existing build, stop `dx serve`, build once with
`dx build --web --locked --package converter-web` from the repository root,
then run `node tools/serve-web-build.mjs <built-public-directory> 8084`.
It serves only that build with isolation and no-store headers. Avoid running a
separate `dx build` against the same output while `dx serve` is building it.

Open `http://127.0.0.1:8084` in your browser. Localhost is suitable for browser GPU access; deployed sites need HTTPS. Serve the app over HTTP/HTTPS, not by opening its HTML file directly. Stop the development server with Ctrl+C.

The cross-origin policy flag is required for AI and the optional FFmpeg WASM backend. Ordinary AI Off WebCodecs does not require it.

### Build for hosting

From `apps/web`, after installing npm dependencies and generating the worker:

```powershell
dx build --web --release --locked
```

Deploy the **entire generated `public` directory** reported by Dioxus, including its assets and WebAssembly files. Configure your host to serve WebAssembly correctly. To enable FFmpeg WASM, also send these response headers:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Production hosting configurations need their own compatibility checks.

### Convert a video

1. Select an MP4 containing H.264 or supported H.265/HEVC video. Source metadata and paused preview load automatically.
2. Choose output size, frame rate and bitrate.
3. Select an available output profile. The app checks compatibility for the selected source and settings; unavailable profiles display a reason.
4. Optionally open **Color adjustments** and compare the preview.
5. Click **Convert**, then use the download link to save the finished video. **Cancel** stops an active conversion.

WebCodecs offers WebM/VP8 with Opus audio, an explicit video-only WebM profile, and capability-gated MP4/H.264/AAC or MP4/H.265/AAC. Browser support varies: seeing a GPU name does not prove the browser uses hardware video codecs.

Use the **FFmpeg WASM** navigation link to select the alternative encoder; **WebCodecs** returns to the default. FFmpeg WASM currently offers MP4/H.264/AAC only, requires one audio track and browser temporary-file storage, and needs cross-origin isolation. For VFR input, choose a fixed output FPS such as 30; Original VFR export is unsupported on this backend. Source preview remains available even when the selected export settings are unavailable.

FFmpeg WASM has a large initial download and software-encoding overhead. Its preview still uses WebCodecs/WebGPU, so switching encoders does not remove those preview requirements. CLAHE preview is verified, but FFmpeg WASM CLAHE export acceptance remains unverified.

Download completed output before leaving the page or replacing it. Temporary browser storage can run out; when writable temporary storage is unavailable, the WebCodecs memory fallback limits input to 256 MiB.

## Resolution, compression and color

- **Original size** is the default. Percentage presets, HD/720p, FHD/1080p, 2K width, QHD/1440p and 4K/2160p fit within the selected bounds while retaining aspect ratio. They do not upscale small videos. Browser exact-size mode also provides an aspect-ratio option. Check the displayed actual output dimensions.
- **Original FPS** preserves source timing. Fixed FPS duplicates or drops frames without changing playback speed; it does not create motion-interpolated frames.
- **Recommended bitrate** adapts to the source, output resolution, frame rate and codec. Smaller/Higher adjust that recommendation; Custom accepts 0.25–120 Mbps. Bitrate is a variable-rate target and estimated size is approximate, not a guaranteed cap.
- **Brightness, contrast and saturation** remain independently editable. **CLAHE** adds local contrast enhancement with its own Strength; it can amplify noise and make preview playback slower.
- **Before/After** affects preview only. **Reset color** restores neutral sliders, disables CLAHE and restores its default Strength. It does not reset resolution, FPS or bitrate.
- Native CLAHE requires **Inspect** first and a Shared GPU route for conversion. Direct FFmpeg and native 10-bit CLAHE output are not supported.

## Preview controls

Use Play/Pause, click the video to toggle playback, drag the timeline to seek, and use Mute for preview audio. Muting preview does not remove audio from the export.

In the native app, **Expand preview** opens a separate maximized, bordered window. Close it or press Escape to return to the inline preview. With player shortcuts active, Space toggles playback, Left/Right seek five seconds and Home/End jump to the start/end. Clicking a text field or pressing Tab releases player shortcuts. In the browser, Space/Enter toggle playback when the video is focused.

Preview is intended for reviewing settings, not exact frame stepping or color-critical grading. Native display is an 8-bit SDR approximation; enlarging the preview does not add detail. Preview may skip or lag frames without changing conversion's frame policy.

## Troubleshooting and limitations

- **Black native preview with an inspection warning:** inspect the selected source before enabling CLAHE. Unresolved geometry pauses playback; turning CLAHE Off restores ordinary preview.
- **Unavailable browser format:** read the compatibility reason and try another profile. FFmpeg WASM VFR export needs fixed FPS; preview availability is independent of export availability.
- **No browser GPU preview:** check WebGPU/WebCodecs availability, HTTPS/localhost and graphics drivers. Switching to FFmpeg WASM does not supply a software-only preview.
- **Missing linker:** install the linker for the active Rust toolchain. `link.exe` errors indicate MSVC; the repository's pinned GNU toolchain uses a different linker.
- **FFmpeg not found or missing filter/encoder:** check `PATH` and your FFmpeg build's features. Preview/color controls require `zscale`; NVIDIA conversion requires compatible hardware support.
- **Native renderer-device loss:** the window reports the failure. F5 explicitly restarts its renderer and resets controls; it does not automatically resume conversion.

Support is currently focused on SDR MP4 input. HDR/wide-color input, changing video geometry, and some route-specific geometry/audio combinations are rejected. Windows and selected Chromium/Firefox configurations have been tested, not every browser, GPU or OS. Recent preview checks did not include Firefox. Hardware codec-surface sharing, universal performance gains and transparent recovery from driver failures are not promised.

## Further information

- [Verified compatibility and known limitations](docs/interop-report.md)
- [Architecture and roadmap](docs/architecture.md)
- [Color adjustment behavior and plan](docs/color-adjustments-plan.md)
- [Reproducible test fixtures](fixtures/README.md)

## License

Project source is covered by the [MIT license](LICENSE), copyright Arman Radan. Dependencies have their own licenses. In particular, the bundled FFmpeg WASM core is GPL-licensed; review its redistribution requirements before distributing a browser build. Native FFmpeg licensing depends on the build you distribute.
