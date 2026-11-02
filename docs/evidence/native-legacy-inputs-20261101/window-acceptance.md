# Ordinary native window checkpoint - 2026-11-02

Task 6.1 remains open. Windows was unlocked by the user and Computer Use
activation/capture/input worked for the following real window checks. This
supersedes the earlier statement that no new ordinary-window checks were possible.

The normal frontend evaluation build was launched and selected through returned
Computer Use app/window objects. The input Browse dialog exposed exactly
`*.mp4;*.mpg;*.mpeg;*.wmv`; MPEG-1 was selected through that dialog. WMV3 was
selected through the ordinary source path field and inspected. Output remained
MP4/AAC/SDR, Original FPS, neutral adjustments.

| Source | Window-selected route | Frames | Output | Result |
| --- | --- | --- | --- | --- |
| CC0 MPEG-1/MP2 | Direct FFmpeg CPU | 50 -> 50 | 384x288 | Complete |
| CC0 MPEG-1/MP2 | Direct FFmpeg NVIDIA | 50 -> 50 | 384x288 | Complete; software decode / h264_nvenc |
| Local WMV3 Main/WMA Pro clip | Direct FFmpeg CPU | 48 -> 48 | 788x576 | Complete |
| Local WMV3 Main/WMA Pro clip | Shared GPU NVIDIA | 48 -> 48 | 788x576 | Complete; software decode / h264_nvenc |
| Same WMV3, RTX 2x | Shared GPU NVIDIA | 48 -> 48 | 1576x1152 | Export complete; preview failed as below |

The source preview displayed the MPEG test pattern. WMV3 inspection showed
720x576 coded / 788x576 display dimensions and `wmapro (Windows decoder)`.
WMV3 playback displayed real decoded title frames and advanced to EOF. This
visual check does not independently establish audible synchronization.

The remembered RTX configuration and previously accepted SDK terms were reused;
opening AI setup automatically tested the configured runtime. No JSON import or
new terms acceptance was needed. RTX 2x became available; 4x correctly reported
the unchanged 640x360 input limit. Selecting 2x resolved output to1576x1152 and
disabled ordinary output-size controls.

## Preview bug found and fixed

RTX preview reported `AI preview selection does not match model output geometry`.
The UI used coded dimensions when creating its AI preview policy, whereas the
native processor correctly used display-normalized dimensions. The UI now uses
`display_width` / `display_height` for that policy too. NVIDIA route hints also
now describe NVDEC or software decoding accurately. These are scoped task6.1
fixes. The export itself completed despite the old preview mismatch.

The normal native application was rebuilt successfully and launched. Its new
route hints were visually observed. Input then failed twice with
`foreground window did not report a process id`: once initially and once after
refreshing the returned window, rehydrating and activating it. Input stopped
under the Computer Use recovery rule. The fix's ordinary-window preview retest
is pending; successful compilation is not claimed as visual proof.

## Independent output checks

Both MPEG outputs passed the existing independent complete decoded timeline,
audio endpoint and three sampled image checks. Video PTS/end error <=0.912ms;
audio end error0.434ms; image RGB RMSE <=5.521. Reports are in
`window-mpeg1-nvidia-independent.json` and `window-mpeg1-software-independent.json`.

All three WMV outputs independently decoded complete audio/video without errors,
had48 frames, expected dimensions, square SAR, AAC audio, strictly increasing
video PTS, and the expected libx264/h264_nvenc encoder tag. Details are in
`window-wmv-output-checks.json`. This window checkpoint did not rerun the separate
WMA waveform oracle; the prior120-case matrix remains that evidence.

Post-fix checks: formatting, normal native build, native application tests
(20passed/6explicit runtime tests ignored), and matching-target native/application
Clippy with ai-upscale-pilot and warnings denied pass. The normal build/test
emitted an existing GNU linker `.drectve` warning from the WMA build script;
Clippy passed without warnings. No dependency installation occurred.

Remaining task6.1 window acceptance: retest the fixed non-square-SAR RTX preview;
finish H.264, HEVC, MPEG-2, WMV1, WMV2 and VC-1 selection/preview/conversion through
software/NVIDIA choices. The120-case backend matrix does not replace these UI
checks. No task checkbox is changed on partial evidence. ai-upscale remains24/32.
