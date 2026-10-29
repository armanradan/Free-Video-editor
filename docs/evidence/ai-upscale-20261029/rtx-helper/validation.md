# Actual validation, 2026-10-29

- MSVC19.51 source-only helper and separate unmodified installed SDK sample build
  succeed. Existing two SDK header C4100 warnings remain; no bypassed build errors.
- Final helper suite:17 process runs;12 exact sample image comparisons at pinned
  RGBA8 max0/MAE0; malformed/stale protocol, clean EOF/shutdown, missing runtime,
  wrong adapter, cleanup and fresh repeat pass. See report.json and per-run logs.
- Shared SDK utility extraction regression: original probe's six-process suite
  passes16 independent output checks, strict repeats, actual SDK platform error,
  startup failures, cleanup and fresh retry. See probe-regression/report.json.
- cargo fmt --all -- --check: passes.
- cargo check -p media-native --features ai-upscale-pilot: passes.
- cargo check -p ui --target wasm32-unknown-unknown: passes.
- cargo check -p media-core -p media-gpu: passes.
- cargo test -p ui --lib:2 passed,0 failed.
- Python evaluator AST parsing and PowerShell build-script AST parsing: pass.
- openspec validate ai-upscale --strict: passes.
- Refreshed OpenSpec apply confirms task5.1 done and22/32 complete,10 remaining.
- git -c core.safecrlf=false diff --check: passes.

Only the native availability reason changed in Rust/UI. No real native video,
active RTX cancellation/timeout/crash, effects bridge, preview or ordinary RTX
controls are claimed by this gate. Browser runtime/UI was not exercised.
