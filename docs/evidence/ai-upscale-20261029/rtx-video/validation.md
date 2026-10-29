# Actual task5.2 validation, 2026-10-29

- Native debug CLI built with ai-upscale-pilot; optional SDK helper unchanged from
  the source-hashed task5.1 executable. No install/download or redistribution.
- Three six-file RTX matrices pass native source-timeline/audio verification:
  2x/4x x Original/15/60FPS, four source inference frames per export.
- Separate FFprobe rational grids/presentation endpoints and full FFmpeg A/V
  decode pass all18 exports. Source is the checked-in tiny CC0 VFR/audio fixture.
- First-to-repeat and repeat-to-final MP4s are byte-identical for all six cases;
  independently decoded RGBA/frame hashes also repeat exactly.
- Actual Rust RTX client: SDK sample output hashes match at both native scales,
  repeated frames match, wrong generation rejects, upload/readback counts match.
  Four native neutral preencoder comparisons have max0LSB (pinned limit1LSB).
  Final log: repeat/rust-rtx-final.log.
- Five actual rejecting CLI cases: wrong LUID, wrong downstream GPU, unvalidated
  effects, failed NVDEC/NVENC route, real640x360 source above192x108 cap.
  No output or partial is published. See negative/report.json.
- Actual ONNX shared-owner regression passes pixels/repeats, active cancel,
  child crash/reap and fresh retry. See repeat/rust-onnx-regression.log.
- Actual ONNX CLI export with CLAHE/manual controls and complete A/V decode
  passes after the common CLI/child changes. See onnx-export-regression/.
- cargo test -p media-native --features ai-upscale-pilot --lib:
  49passed/0failed/10ignored. Actual RTX/ONNX tests invoked separately above;
  unrelated ignored hardware tests are not passing runtime coverage.
- cargo check -p converter-native: passes.
- cargo check -p media-core -p media-gpu: passes.
- cargo check -p media-web --target wasm32-unknown-unknown: passes.
- cargo test -p ui --lib:2passed/0failed.
- cargo clippy -p media-native --features ai-upscale-pilot -- -D warnings: passes.
- cargo clippy -p ui --target wasm32-unknown-unknown -- -D warnings: passes.
- cargo fmt --all -- --check: passes.
- Python matrix/comparison AST parsing: passes.
- openspec validate ai-upscale --strict: passes; refreshed apply confirms
  task5.2 complete, task5.3 open,23/32 complete and9remaining.
- git -c core.safecrlf=false diff --check and final source-hash checks: pass.

RTX effects/active IO cancellation/crash/timeout, native preview/UI, larger
envelopes and release quality/performance are still open. Native RTX controls
remain disabled with an accurate remaining-gate reason. Peak memory0 in Rust
reports is explicitly flagged unavailable, not zero allocation. No browser
runtime/UI acceptance or hardware codec surface bridge is claimed.
