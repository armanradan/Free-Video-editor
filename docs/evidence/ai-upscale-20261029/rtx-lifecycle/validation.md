# Actual task5.3 validation, 2026-10-29

- Separate normal/RTX_DIAGNOSTIC MSVC helpers compile. Existing two SDK header
  C4100 warnings remain; no SDK files or binaries are vendored.
- Normal helper excludes diagnostic strings and ignores fault environment in an
  actual two-frame process run. See normal-fault-env.json and stderr audit.
- Rebuilt normal helper passes full17-process/12-image exact SDK sample gate.
  See helper-regression/report.json; independent SDK reference stays unmodified.
- Actual selected-size RTX/CLAHE/manual test passes both scales x strength0/50/100
  x four source frames. Scalar manual reference max1LSB (pinned2); zero bypass
  exact and strength deltas increase. See effects/report.json/effect-order-final.log.
- Six actual RTX effect exports (2x/4x x Original/15/60FPS) pass native checks and
  independent full FFmpeg/FFprobe A/V decode, rational grid, geometry/endpoints.
  See video/native.json and video/independent-video.json. Tiny CC0 input only.
- Strengthened active suite passes seven failures with effects enabled: blocked
  write/read cancel, write/read timeout, helper crash, controlled downstream
  device destruction and client budget rejection. All partials removed; seven
  explicit healthy retries pass. Five helper cases confirm actual IO stage,
  retired ownership and independent Windows PID absence. See active/report.json.
- Initial five-case suite is retained under active-initial, not substituted for
  the stronger final suite. Diagnostic stalls occur after actual SDK startup and
  one completed frame; response-side stall/crash follows second SDK readback.
- Tests shorten frame deadlines to3s; production remains60s. No claim of active
  vendor-kernel interruption, physical OOM or spontaneous driver loss.
- Actual source10-bit/HDR CLI rejections publish no output. Native unit tests
  reject direct effect routes and Main10 before source IO.
- Actual DirectML reference/cancel/crash/reap/retry regression passes after the
  receive cancellation recheck. See onnx-regression.log.
- cargo test -p media-native --features ai-upscale-pilot --lib:
  49passed/0failed/12ignored; actual RTX/ONNX tests invoked separately above.
- cargo check -p converter-native: passes.
- cargo check -p media-core -p media-gpu: passes.
- cargo check -p media-web --target wasm32-unknown-unknown: passes.
- cargo clippy -p media-native --features ai-upscale-pilot -- -D warnings: passes.
- Matching native Clippy with --tests also passes. The test-only literal-const
  thread-local initializer has a local annotation for the pinned Clippy's macro
  diagnostic; no production warning suppression or numeric-gate relaxation.
- cargo fmt --all -- --check, Python/PowerShell parsing, OpenSpec strict
  validation, source hashes and git diff whitespace: pass.
- Refreshed apply confirms task5.3 done,24/32 complete,8remaining. Native preview,
  ordinary UI, release quality/performance and shipping review remain open.

SDK/API/driver/license/deployment inventory and derived CC0 fixture recipe/hashes
are separate compact JSON artifacts. Consent covers local evaluation only. Media,
helpers/objects/cache remain ignored; no raw frame sequence is saved.
