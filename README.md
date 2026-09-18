# Diaxus GPU video converter — M2 browser converter

The repository implements the smallest usable M2 browser path while retaining M1 as a regression probe:

```text
MP4/H.264 file → Mediabunny demux → WebCodecs decode → VideoFrame
→ wgpu copy_external_image_to_texture → shared WGSL half-size resize
→ WebGPU HTML canvas → VideoFrame → WebCodecs VP8 encode
→ Mediabunny WebM mux → downloadable, seekable video-only file
```

The UI is Dioxus Web 0.7.10. `media-core` contains platform-neutral timing, dimensions, and the M2 size policy; `media-gpu` contains the shared wgpu 30.0.1 render pipeline/WGSL; `media-web` owns browser resources and the codec/container bridge; and `ui` contains reusable Dioxus controls/status components. M3 features are intentionally absent.

## Prerequisites

- Rust 1.98.1 with `wasm32-unknown-unknown`
- a C/C++ linker usable by Cargo build scripts
- Dioxus CLI 0.7.10 (`dx`)
- Node.js/npm (to install the pinned Mediabunny 1.58.0 package)
- a secure browser context with WebGPU and WebCodecs; `http://127.0.0.1` and `http://localhost` qualify

On this Windows host the Visual Studio C++ workload was absent, so checks used the Rust GNU host toolchain and installed MinGW:

```powershell
rustup toolchain install 1.98.1-x86_64-pc-windows-gnu --profile minimal --component rustfmt,clippy
rustup target add wasm32-unknown-unknown --toolchain 1.98.1-x86_64-pc-windows-gnu
```

## Build and run

```powershell
$env:RUSTUP_TOOLCHAIN = '1.98.1-x86_64-pc-windows-gnu' # only needed when the default host linker is unavailable
Set-Location apps/web
npm ci
dx serve --web
```

Open the local URL shown by Dioxus, select an MP4 containing H.264 video, and press **Convert**. M2 accepts files up to 256 MiB, resizes to half width/height, omits audio with an explicit warning, and produces a downloadable VP8 WebM. The original M1 probe remains under the regression disclosure.

Required checks:

```powershell
cargo +1.98.1-x86_64-pc-windows-gnu fmt --all -- --check
cargo +1.98.1-x86_64-pc-windows-gnu check -p media-core -p media-gpu
cargo +1.98.1-x86_64-pc-windows-gnu test -p media-core -p media-gpu
cargo +1.98.1-x86_64-pc-windows-gnu check --workspace --target wasm32-unknown-unknown
cargo +1.98.1-x86_64-pc-windows-gnu clippy --workspace --target wasm32-unknown-unknown -- -D warnings
$env:RUSTUP_TOOLCHAIN = '1.98.1-x86_64-pc-windows-gnu'
Set-Location apps/web
dx build --web --locked
```

The deterministic fixtures are documented in [fixtures/README.md](fixtures/README.md). Measured M1 and M2 interoperability evidence is in [docs/interop-report.md](docs/interop-report.md). Browser-internal copies and actual codec hardware use remain unknown.
