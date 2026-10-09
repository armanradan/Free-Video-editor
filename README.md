# Diaxus Video Converter

Resize, compress and adjust videos using a native Windows app or a browser app. Both versions offer a source preview, configurable resolution and frame rate, adaptive bitrate recommendations, brightness/contrast/saturation controls, and optional CLAHE local contrast enhancement.

The native app uses installed FFmpeg and can use NVIDIA hardware codecs. The browser app uses WebCodecs by default, with an optional FFmpeg WASM encoder. Codec availability depends on your machine, browser and selected video.

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

Open `http://127.0.0.1:8084` in your browser. Localhost is suitable for browser GPU access; deployed sites need HTTPS. Serve the app over HTTP/HTTPS, not by opening its HTML file directly. Stop the development server with Ctrl+C.

The cross-origin policy flag enables the optional FFmpeg WASM backend. It is not required for the default WebCodecs backend.

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
