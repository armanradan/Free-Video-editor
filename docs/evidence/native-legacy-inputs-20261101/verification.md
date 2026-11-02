# Native legacy inputs verification checkpoint (2026-11-02)

OpenSpec progress is24/25. Only task6.1 ordinary-window acceptance remains;
its implementation is built, while desktop verification is blocked as below.

Completed:104 Rust tests (media-native84, converter-native20);25+6explicit
opt-ins remain ignored in the default run. Separately executed actual Windows
audio preparation/cancel/retry, active inspection cancellation, hybrid export
failure/cancel/retry and RTX geometry/parity checks are documented in their
evidence folders. The complete120-case codec/route/FPS/RTX matrix independently
passes, as do the original AAC and original MPG exports described in evidence.

Checks pass:

- `cargo fmt --all -- --check`
- `cargo test -p media-native -p converter-native`
- `cargo clippy -p media-native -p converter-native --all-targets --features media-native/ai-upscale-pilot -- -D warnings`
- `cargo check -p media-core -p media-gpu` on the host and wasm32-unknown-unknown
- Native CLI build with ai-upscale-pilot, plus the normal native application build
- Strict `openspec validate native-legacy-inputs --strict` and `git diff --check`

Rust1.98.1 x86_64-pc-windows-gnu; MSVC14.51.36231/Windows SDK10.0.26100.0 for the
embedded WMA helper; FFmpeg8.0.1 essentials. The native application was built to
a separate ignored evaluation executable because the old app remained running
and desktop input was unavailable. Removing only its debug sections reduced
that evaluation executable from870.7MB to202.2MB without new tool downloads.
No replacement of the still-running older window is claimed.

The normal-window check remains blocked: black capture, activation failure and
recovery `GetCursorPos failed: Access is denied (0x80070005)`. Desktop actions
stopped; no UI-based source/route acceptance is marked complete on that basis.
The current build contains the expanded input picker and Windows audio/actual
decoder status, but ordinary-window selection/preview/conversion verification
must resume after desktop access is restored. Output remains MP4.

The ai-upscale task file was not changed by this work; OpenSpec reports24/32
there. Its separate remaining acceptance is not completed by this change. No
new browser runtime claims, model/runtime downloads, drivers or SDK installation.
Private media and generated evaluation outputs remain ignored; only tiny CC0
fixtures and redacted evidence are tracked. Changes are uncommitted.

## 2026-11-02 ordinary-window continuation

Desktop unlock allowed real Browse/inspect/preview/playback/export checks for
MPEG-1 and WMV3/WMA Pro. Five window-driven MP4 exports passed complete decoded
output checks, including shared NVIDIA RTX2x WMV3. A non-square-SAR AI preview
policy bug was found and fixed in the UI; the normal app was rebuilt, application
tests and matching-target Clippy passed. Its window retest and the other six
families remain pending after Computer Use input recovery failed twice with
`foreground window did not report a process id`. See `window-acceptance.md` for
the exact scope, observations, output evidence and remaining acceptance. Progress
remains24/25; this partially successful window check does not complete task6.1.

## 2026-11-02 successful desktop retry - final acceptance

The later user-requested Computer Use retry succeeded. Corrected non-square-SAR
RTX preview now displays a visible WMV3 frame after seeking and restores it after
export. H.264, HEVC, MPEG-2, WMV1, WMV2 and VC-1 each passed ordinary source
selection/inspection/visible RTX2x preview and Shared GPU software/NVIDIA exports.
Together with the preceding MPEG-1/WMV3 window checks, all eight families have
both software/NVIDIA window coverage. Output Browse remains MP4-only.

All13continuation exports passed independent full AV decode, every video PTS/end,
PCM waveform/end, count/geometry/SAR and encoder checks: <=0.908ms video error,
<=16.259ms audio endpoint error and <=0.003301 waveform RMSE. The unchanged
current-device gate correctly rejected stale LUID0:78387; independent helper
handshake read0:78380 and ordinary UI configuration/test restored validation
under the existing SDK evaluation approval. This does not implement automatic
identity refresh. Details/remaining profile and setup limits are in
`window-acceptance.md`; redacted machine-readable checks are in
`window-continuation-independent.json`.

Task6.1 is now complete:25/25. Prior formatting/native build/application tests
and Clippy checks still apply; this continuation changes verification tools and
evidence only. Strict OpenSpec validation and diff checks are rerun. The separate
ai-upscale task file remains unchanged at24/32. Changes remain uncommitted.
