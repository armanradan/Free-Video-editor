# Diaxus GPU video converter — M3.3

Dioxus Web controls an MP4/H.264 converter with shared Rust/wgpu half-size resizing. Output profiles are WebM/VP8/Opus, explicit video-only WebM, and capability-gated MP4/H.264/AAC. Browser codecs, container handling, and GPU processing run together in a dedicated worker when supported; otherwise the UI reports the main-thread compatibility fallback and its reason.

`media-core` owns platform-neutral policy, `media-gpu` owns the shared processor/WGSL, `media-web` owns browser resources and worker transport, and `ui` contains reusable controls. The M1 deterministic regression remains available. M3.4 performance work has not started.

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

The toolchain file selects Rust; no `RUSTUP_TOOLCHAIN` environment override is needed. Select an MP4, choose an available profile, and convert. The execution label reports worker/fallback mode. For a deliberate fallback comparison open the same URL with `?execution=main`; remove it and reload to use automatic worker selection.

Worker code uses the same wasm bundle produced by `dx`; there is no manual worker build. Keep the complete generated `public` directory, including wasm snippets, when deploying. Input is capped at 256 MiB and compressed output remains in memory. This is not a streaming converter, and successful GPU processing does not prove hardware codec execution.

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
```

It tests every enabled profile, cancellation/restart, output loading/seeking, window timer responsiveness, M1 cancellation and five repeated M1 marker checks. Generated outputs/evidence go under ignored `tmp/m33-*`. The transport unit tests simulate unsupported startup, cancellation, stale messages, and worker crashes; they are not browser interoperability evidence.

See [fixture provenance](fixtures/README.md), [architecture and remaining milestones](docs/architecture.md), and [verified results and untested behavior](docs/interop-report.md). Firefox worker/fallback WebM is verified; Chromium/MP4 worker validation remains pending.
