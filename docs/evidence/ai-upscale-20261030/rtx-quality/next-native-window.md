# Remaining native verification


Latest larger-envelope update:2x<=1280x720,4x<=640x360 on the evaluated helper,
with actual normal-window preview and720p2x export. See `../rtx-larger/` and the
latest interop entry. Missing nominal-rate metadata in the FFmpeg bridge caused
initial gray images despite timing success; fixed exports and regressions are
recorded separately. Remaining work includes long-run maximum-size stress,
further GPU/device coverage, persistent preview sessions/performance, NVDEC/NVENC
timing, complete cross-platform lifecycle and licence/distribution audit. Older
limit statements below describe prior builds; do not repeat superseded setup steps.

Update2026-10-30: desktop tools now work; ordinary RTX2x/4x UI/export evidence
and app-owned setup/restart validation are recorded in `../native-setup/` and the
latest interop entry. Local deployment loads `assets/native-rtx.json` beside the
debug executable; saved setup takes precedence. Users need no JSON import.
RTX remains limited to192x108 source, evaluated RTX3050Laptop/Vulkan/SDK1.1/helper,
explicit software-codec export. LUID0:82840 is current observed identity. Do not
silently substitute after Windows-session changes. No SDK binaries redistributed.

Remaining window checks include4x/CLAHE/long-loop EOF after the endpoint fix,
late disabled Cancel after completion, and full lifecycle/release audit. Native
2x motion EOF now passes; broader real-time/audio synchronization is unvalidated.
Do not repeat the96-file quality comparison without a relevant new failure.

The following is the earlier handoff, retained as historical context:

# Resume with native desktop tools

The user chose to make desktop tools available in a fresh session. None were
exposed in the measurement session. Load the current Computer Use skill and
verify real app enumeration; follow AGENTS.md if its remembered Windows runtime
failure recurs. Do not reuse an obsolete plugin path or perform automatic repair.

The existing SDK local-evaluation authorization remains valid. Do not ask for
the same permission again. No SDK redistribution or automatic installation was
authorized. Runtime/SDK consent and provisioning UX for the product remains
unfinished. No accepted capability can be imported from a local JSON file.

Current ordinary executable:
`D:/Libraries/Cargo/CARGO_TARGET_DIR/debug/converter-native.exe`.
Existing local configuration, deliberately ignored:
`tmp/ai-upscale/native-ui-20261030/rtx-configuration.json`.
On the subsequent setup run the saved LUID0:78108 was correctly rejected;
the runtime observed0:82840 on the same RTX3050/DXGI1. A separate ignored
`tmp/ai-upscale/native-ui-20261030/rtx-configuration-current-session.json`
was verified with that identity. Prefer that file while this session is current;
re-inspect after a Windows session change. See `../native-setup/rtx-preflight.json`.
It points to the normal source-built helper and assessed SDK/cache. Recorded
identity is RTX3050Laptop, DXGI1, LUID0:78108. Re-verify current identity if the
system session has changed. Select the matching processing Vulkan GPU and
Shared GPU software-codec route explicitly; the AI NVDEC/NVENC route remains
unavailable. The192x108 source cap must not be enlarged from these results.

The ordinary app still supplies pending capabilities. Read-only configuration
inspection and an explicit synthetic runtime test/SDK acknowledgement are now
implemented and API-tested for RTX/ONNX. Their window controls are uninspected.
Finish source/route/capability and conversion-consent integration, then verify actual ordinary UI
selection/preview/export before declaring that configuration accepted. Merely
viewing disabled controls or injecting test metadata is not ordinary AI On
acceptance. Diagnostic `--ai-preview-check` results are a separate category.

Use the existing192x80 licensed clips in
`tmp/ai-upscale/quality-temporal-20261025/` and CC0 cut/VFR fixtures. Check:

- Both themes and normal compact layout; AI setup and missing-asset/error states.
- Explicit2x/4x dimensions, bitrate/profile checks, retained ordinary resize,
  Off restoration, independent Reset color/Before scopes and exact paused pixels.
- Play/Pause, Pause after error, seek/source replacement, latest-only progress,
  explicit retry and conversion handoff through the actual controls.
- Unsupported export with working source preview. Main10/HDR, other GPUs and
  unsupported routes must not fall back or become selectable.
- Immutable H.264/AAC export settings, cancellation/cleanup and paused preview
  restoration. Verify ordinary exported files independently before acceptance.
- Setup busy state must block conversion; asset import/removal retires AI before
  mutation. Configuration import itself executes no runtime or helper.

Avoid repeating the96-file comparison without a new failure or relevant change.
It already passes independent timing/audio/decode; bilinear pixels and configured
RTX/ONNX handoff regressions also pass. Pure GPU/codec kernel times remain
unavailable; CPU wall interval scopes are documented in the latest interop report.
Fresh SDK startup per seek remains a measured performance problem, not a fixed
issue or a real-time-preview claim. Record actual window evidence and keep tasks
unchecked until their full specified behavior has passed.
