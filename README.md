# Diaxus GPU video converter — M1 experiment

This repository currently implements milestone M1 only: a fixed browser experiment that validates this path:

```text
30-packet VP8 fixture → WebCodecs VideoDecoder → VideoFrame
→ wgpu copy_external_image_to_texture → shared WGSL resize (320×180 → 160×90)
→ WebGPU HTML canvas → VideoFrame → WebCodecs VideoEncoder
→ verification VideoDecoder → test-only RGBA checks
```

The UI is Dioxus Web 0.7.10. `media-core` contains platform-neutral timing and dimensions, `media-gpu` contains the shared wgpu 30.0.1 render pipeline/WGSL, and `media-web` owns browser resources and the codec bridge. M2 features are intentionally absent.

## Prerequisites

- Rust 1.98.1 with `wasm32-unknown-unknown`
- a C/C++ linker usable by Cargo build scripts
- Dioxus CLI 0.7.10 (`dx`)
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
dx serve --web
```

Open the local URL shown by Trunk and press **Run M1 probe**. The page reports exact capability failures and lifecycle/copy counters. Press **Cancel** to test cleanup and restart.

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

The deterministic fixture is documented in [fixtures/README.md](fixtures/README.md). The measured interoperability evidence is in [docs/interop-report.md](docs/interop-report.md). M1 passed five consecutive full runs plus cancellation/restart in the tested Edge environment; browser-internal copies and actual codec hardware use remain unknown.
