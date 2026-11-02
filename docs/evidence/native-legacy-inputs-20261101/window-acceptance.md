# Ordinary native window checkpoint - 2026-11-02

Initial checkpoint: task 6.1 remained open. Windows was unlocked by the user and Computer Use
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

## Successful desktop continuation - task 6.1 complete

The user requested a fresh desktop retry. Computer Use discovery, activation,
clicks, text entry and screenshots worked again. The current normal native build
at `CARGO_TARGET_DIR/debug/converter-native.exe` was launched through Computer
Use. No UI test hook or CLI conversion replaced these window actions.

The fixed WMV3 RTX preview displayed an actual frame after seeking to the middle
of the clip. Export completed with48/48frames at1576x1152, then preview restored
the selected image without the previous geometry error. The remaining six
families were selected in the ordinary source field, inspected, visibly previewed
with RTX2x, and converted through both Shared GPU software codecs and Shared GPU
NVIDIA. All used Original FPS, neutral adjustments and MP4 H.264/AAC output.

| Source | Frames per export | RTX2x output | Software / NVIDIA results |
| --- | --- | --- | --- |
| H.264/AAC | 60 | 1280x720 | Both complete; software/libx264 and NVDEC/h264_nvenc |
| HEVC/AAC | 30 | 640x360 | Both complete; software/libx264 and NVDEC/h264_nvenc |
| MPEG-2/MP2 | 30 | 384x288 | Both complete; software/libx264 and software/h264_nvenc |
| WMV1/WMA1 | 30 | 384x288 | Both complete; software/libx264 and software/h264_nvenc |
| WMV2/WMA2 | 30 | 384x288 | Both complete; software/libx264 and software/h264_nvenc |
| VC-1 Advanced/WMA2 | 30 | 588x480 | Both complete; software/libx264 and software/h264_nvenc |

Together with the earlier MPEG-1 and WMV3 window checks above, all eight required
video families now have ordinary-window source/preview/conversion coverage with
software and NVIDIA codec choices. This continuation created13exports; the two
window sessions created18total. The Source Browse filter was verified earlier;
the real Save As dialog was also inspected and retained `MP4 video (*.mp4)`.
WMV1/2/VC-1 metadata visibly reported the bounded final-duration estimate and
Windows audio decoder. VC-1 remains the deliberately limited repeated-frame
fixture; this does not claim broader temporal/profile coverage.

### Current-device RTX validation

After restart the saved LUID0:78387 no longer matched the same named DXGI1
RTX3050LaptopGPU. The existing runtime gate rejected it before enabling AI.
An initial visual transcription was wrong (78880); a separate bounded handshake
with the unchanged evaluated helper read the exact current identity0:78380,
vendor4318/device9634, SDK1.1.0, High3. Only that LUID in an ignored evaluation
configuration was refreshed and imported using the normal UI. The same supplied
SDK terms were acknowledged under the user's existing local-evaluation approval,
then the ordinary runtime test passed all four2x/4x checks. Existing SDK/helper
paths, Vulkan adapter key and admission gates were retained. No gate was bypassed
and no new automatic identity refresh feature is implemented here. Remembered
LUID handling across restarts remains a separate setup usability limitation.

The evaluated RTX helper SHA256 remains
`e9188624f8a4605f4429f83f8c74463a22db9bfd374a36f590a23f37cf5cac95`.
No new SDK, driver or runtime was installed. The source-built Windows WMA helper
used for independent reference has SHA256
`28086ccb7fd84972b1f8fba78a93bdcfb175d9f19c15cc1f5cf12b8fcc3c799f`;
its protocol isDXWMA001. The older exploratory clock executable does not supply
that protocol and was not used as the successful oracle.

### Independent continuation verification

`window-continuation-independent.json` records all13successful output checks.
Every output passed complete AV decoding, every source video PTS and endpoint
comparison, exact frame count/selected geometry, square SAR and expected encoder
tag. NVIDIA RTX outputs also had no decoded Bframes. Independent PCM waveform
and endpoint checks used Windows Media Foundation for WMA and FFmpeg for AAC/MP2.
Maximum video PTS/end error0.9071ms, audio endpoint error16.259ms and waveform
RMSE0.003301; existing1ms/25ms tolerances were unchanged. Outputs total3,626,503
bytes. This continuation adds no perceptual image-quality claim; the separate
120-case matrix retains its independent image checks.

Reproduction: create a fresh ignored JSON manifest with explicit `label`,
`source`, `output`, `frames`, `size` and `encoder` for already-created window
exports, then run `tools/native-legacy-inputs/verify_window_exports.py --manifest
<manifest> --helper <source-built-diaxus-wma.exe> --report <new-report>`. The checker
never converts files, refuses to replace evidence and omits private paths/PCM.

Task6.1 is complete. Native-legacy-inputs is25/25; ai-upscale remains24/32 and its
task file is unchanged. Browser, HDR/interlaced/encrypted media, other codec
profiles, expanded RTX envelopes and zero-copy/hardware-audio claims remain
outside this acceptance. No automatic archive/sync/commit/push is performed.
