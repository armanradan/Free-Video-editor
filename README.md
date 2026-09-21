# Diaxus GPU video converter — M3.5

Dioxus Web controls an MP4/H.264 converter with shared Rust/wgpu half-size resizing. Output profiles are WebM/VP8/Opus, explicit video-only WebM, and capability-gated MP4/H.264/AAC. Browser codecs, container handling, and GPU processing run together in a dedicated worker when supported; otherwise the UI reports the main-thread compatibility fallback and its reason.

M3.5 intentionally retains the validated half-size preset. M3.6 begins by replacing it with user-selectable original, percentage, and exact-size output, with aspect-ratio preservation, visible codec-safe dimension adjustment, exact profile re-probing, and real-browser coverage before the milestone proceeds to streaming and recovery work.

`media-core` owns platform-neutral policy, `media-gpu` owns the shared processor/WGSL, `media-web` owns browser resources and worker transport, and `ui` contains reusable controls. The M1 deterministic regression remains available. M3.5 adds crop/PAR/orientation handling, an explicit BT.709/sRGB SDR policy, VFR/non-zero-origin preservation checks, and clear HDR/resolution-change rejection.

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

The toolchain file selects Rust; no `RUSTUP_TOOLCHAIN` environment override is needed. Select an MP4, choose an available profile and codec-acceleration preference, and convert. `Compatibility baseline` is the default. `Prefer hardware` is used only if the exact decoder and encoder configuration both pass; otherwise the result shows the reason for falling back to the unchanged codec/profile. The execution label reports worker/fallback mode. For a deliberate fallback comparison open the same URL with `?execution=main`; remove it and reload to use automatic worker selection.

Worker code uses the same wasm bundle produced by `dx`; there is no manual worker build. Keep the complete generated `public` directory, including wasm snippets, when deploying. Input is capped at 256 MiB and compressed output remains in memory. This is not a streaming converter, and successful GPU processing does not prove hardware codec execution.

Normal conversions stop timing after encoder/muxer finalization and do not re-decode the completed file. Add `?verify=full` to the app URL for the diagnostic interoperability path, which separately reports its re-decode/seek/audio-check time. The automated browser harnesses enable full verification by default; set `VERIFY_OUTPUT=skip` to exercise the normal UI path.

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
```

The independent output inspector requires FFmpeg/FFprobe on PATH. The Chromium harness creates and closes only its own test tab; `fallback` injects a test-only worker-constructor failure to exercise automatic fallback. It tests both acceleration requests for all three profiles, including MP4. The long-run harness uses `tmp/user-test/Input.mp4`, keeps the produced Blob in the browser, and records evidence under ignored `tmp/m34-long`; it requires that local test input to exist. Other generated outputs/evidence remain under ignored `tmp/m33-chromium-*`. Close the isolated debugging browser when finished; do not use your everyday profile for these tests.

The M3.5 harnesses run the geometry/color and VFR/non-zero-origin fixtures through every enabled profile, sample decoded output corner colors, verify every video packet timestamp/duration within the container timebase, and exercise HDR and mid-stream resolution-change failures:

```powershell
node tests/chromium-m35-interop.mjs 9227 8084
node tests/firefox-m35-interop.mjs 8084
```

See [fixture provenance](fixtures/README.md), [architecture and remaining milestones](docs/architecture.md), and [verified results and remaining limitations](docs/interop-report.md). M3.5 acceptance is complete for the tested Firefox/Chromium profile matrix; this does not claim universal browser/HDR support, codec hardware execution, real-time throughput, or stable memory outside application-visible ownership counters.
