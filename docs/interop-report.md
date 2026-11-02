# M1 browser interoperability report


## Native legacy complete codec matrix - 2026-11-02

All120independent codec/route/FPS cases pass:80without AI and40RTX2x shared
software/NVIDIA cases, Original and15fps. H.264/HEVC, MPEG-1/2, WMV1/2/3 and
VC-1 inputs, with AAC/MP2/WMA1/2/Pro, pass complete A/V decode, every original
PTS or full rational fixed-FPS grid, endpoints and middle-frame image checks.
Maximum video PTS error0.9071ms, packet endpoint1ms, audio endpoint18.5035ms,
waveform RMSE0.005845 and RGB image RMSE11.2382, under unchanged timing gates.
All120outputs separately pass dimensions/encoder/packet/copy-contract checks;
RTX NVIDIA output has zero reorder depth and PTS=DTS. Private source identities
are unchanged. See evidence/native-legacy-inputs-20261101/matrix.

Active inspection, Windows audio preparation and hybrid export cancellation,
helper/encoder failure, source replacement and preview/conversion owner handoff
pass with fresh retries and no remaining owned resources. Native/app tests84+20,
host/wasm shared checks and strict lint pass. Browser runtime scope is unchanged.
The new native input picker and status compile; ordinary-window acceptance is
blocked by desktop-access failure, not claimed from these CLI checks. See
[evidence/native-legacy-inputs-20261101/verification.md](evidence/native-legacy-inputs-20261101/verification.md).


## Native legacy hybrid NVIDIA route - 2026-11-02

NVIDIA encoding now uses independently planned NVDEC or software decoding after
actual selected-device preflights, preserving NVENC and exact GPU identity.
MPEG/WMV/VC-1 software-decode/NVENC routes no longer reject solely for absent
NVDEC or non-square SAR. Actual MPEG-1 RTX2x/4x and WMA Pro RTX2x exports pass
independent A/V/timing/image checks;12WMA hybrid and20AAC regression exports pass.
The actual original MPG completes6941frames at768x576 through RTX2x/NVENC;
full independent decode/timing/audio/aspect passes and original SHA/size/mtime
are unchanged. Video PTS/end error<=0.778ms, audio origin0/end0.245ms, sampled
RGB RMSE<=3.923. Total163245ms included other concurrent validation; no controlled
performance claim. Software video decode, zero NVDEC download and4,605,714,432
NVENC upload bytes are reported. Real preflight failure/active cancel/fresh retry
checks pass both hybrid routes. Full fixed-FPS/image matrix and new native-window
acceptance remain pending. See evidence/native-legacy-inputs-20261101/hybrid.


## Native Windows WMA audio path - 2026-11-02

Media Foundation now supplies WMA1/2/Pro PCM and100ns sample timing through an
embedded project-source-built helper. Exact PCM/PTS/duration verification and
bounded owned audio-only preparation precede preview/export. FFmpeg retains
video and MP4 encoding. Earlier FFmpeg-relative WMA clock claims are superseded.

Twelve direct/shared software exports across WMV1/2/3, WMA1/2/Pro, VC-1 Advanced
and offset/gap controls pass complete independent A/V checks; maximum video PTS
error0.014ms, audio endpoint18.503ms under unchanged25ms gate, waveform RMSE
<=0.005845. One48-frame WMA Pro excerpt also passes actual RTX2x at1576x1152
(libx264 software codecs), total12023ms on the evaluated RTX3050Laptop/Vulkan/
SDK1.1 High3/driver621.01 setup. Full private WMA Pro audio-only preparation
passes:8,007,825stereo24bit samples,48,274,724scratch bytes,2510ms measured,
unchanged source stamp and released owned scratch. Full original video export
is not claimed. Native/app tests82+20pass; new ordinary-window WMA interaction,
hybrid NVIDIA and full fixed-FPS/lifecycle matrix remain pending. No browser
or additional RTX envelope acceptance is implied. Detailed evidence and limits:
[evidence/native-legacy-inputs-20261101/codecs/windows-wma.md](evidence/native-legacy-inputs-20261101/codecs/windows-wma.md).


## Native RTX with NVDEC/NVENC accepted - 2026-10-30

The normal native app now accepts explicit Shared GPU NVIDIA plus RTX2x/4x
on the evaluated RTX3050Laptop/Vulkan, driver621.01, SDK1.1 High3 setup. H.264
and HEVC8-bit SDR input, square pixels/no display transform; maximums remain
2x1280x720 and4x640x360, output<=2560x1440. Hardware output>=192x144 is a
conservative tested envelope; the actual minimum corner encodes,128x72 fails
NVENC initialization and is now rejected before helper/output creation.
Preview and software codecs remain independently available for smaller output.

The original22ms->0ms VFR failure reproduced. Diagnostic traces show NVENC
input/output PTS1980 at1/90000 but default B-frame DTS-19020; MP4 demux loses
the offset. With-bf0, PTS=DTS and22ms is preserved. Only RTX hardware encoding
uses this policy; ordinary non-AI NVIDIA encoding and native ONNX gates remain.
No offset filter, timing tolerance relaxation, frame/audio trimming or fallback
was used. Source timing is verified after encoding as before. Primary timestamp
code consulted: https://github.com/FFmpeg/FFmpeg/blob/n8.0/libavcodec/nvenc.c
(`nvenc_set_timestamp`). Observed route-specific interaction, not a universal
FFmpeg/driver defect claim. B-frame removal may reduce compression efficiency.

Evidence: evidence/ai-upscale-20261030/rtx-hardware/. Thirty independently
verified matrix exports:6 VFR2x/4x x Original/15/60,9 useful-size synthetic,
9 licensed motion/face/text,6 HEVC-input2x/4x x Original/15/60. Every file passes
full decode/audio, every PTS/grid/endpoint and known-fixture structure checks.
The30 independent output probes confirm H.264 without B-frames; reports retain
exact codec GPU, processing adapter, inference identity and separate transfers.
Seven source decode comparisons (VFR, two larger synthetic, three licensed,
HEVC) match software-decoded normalized RGBA exactly; one frame pair is retained,
no raw sequence. This test readback is excluded from production timing.

An actual opt-in regression covers22ms at both scales and small-output rejection
before a deliberately unavailable helper. Initial fixture recipes accidentally
rounded the nonzero origin to38ms and used a too-small UI fixture; those assertion
failures are preserved. Explicit encoder time base and a valid larger fixture
fix the tests; no production tolerance was changed. Seven actual640x360-payload
hardware lifecycle cases pass active pipe cancel/timeouts, crash, controlled
processing-device destruction, budget failure, no published/partial outputs and
fresh retries. The original report's192x108 provenance label is preserved and
corrected using its actual SourceInfo; the test now records actual geometry/route.

A complete3202-frame720p->1440p CLI export independently passes timing/audio/full
decode, total307947ms including setup/verification. The ordinary native window
also completes3202 frames540p->1080p using Shared GPU NVIDIA and reports
h264_nvenc; its output independently passes the same checks. UI inference counters
are unavailable and remain null. These are correctness observations, not a fair
codec-quality/speed comparison; other checks ran concurrently. CPU staging,
normalization/manual color conversion, audio processing and verification remain.
No zero-copy, real-time or quality superiority claim is made.

During actual setup, Windows PID reuse selected an already-existing old cache
directory. Atomic parent reservations with a time tag/collision retries now ensure
the SDK's ngx directory is new; no existing cache is removed or reused. Unit
collision preservation and actual automatic setup recheck/full UI export pass.
SDK/helper binary and approval hashes remain unchanged. Native tests62 passed,
19 ignored; app20 passed,6 ignored; native all-target Clippy-Dwarnings, formatting,
Python syntax and strict OpenSpec validation pass. Other devices/drivers, Main10/
HDR, native ONNX hardware codecs, larger geometry and surface sharing remain
unaccepted. No broad AI task checkbox advances;24/32 remain.

## Native RTX long-video failure and repair - 2026-10-30

The user's real 1280x720, 25fps, 212.12-second H.264/AAC input reproduced failure
after 71 SDK evaluations: the helper's sampled DXGI local-memory guard rejected.
The UI hid that cause behind a pipe EOF and success-log prefix. Earlier 24-frame
gates did not cover this failure. No source filename, media or screenshots are
committed. Compact redacted evidence: [rtx-streaming](evidence/ai-upscale-20261030/rtx-streaming/).

The helper now reuses one D3D input/output/UAV/readback resource set rather than
creating new resource identities every frame. Synchronous map/readback establishes
completion before reuse. Existing 1536 MiB helper ceiling, 256 MiB reserve,
384 MiB application ledger and geometry gates are unchanged. Driver/SDK internal
allocation behavior remains opaque. New evaluated helper SHA256 is
e9188624f8a4605f4429f83f8c74463a22db9bfd374a36f590a23f37cf5cac95.
All 20 exact sample comparisons and 17 owned-process protocol/failure runs pass;
512 additional consecutive 720p 2x evaluations retain identical output with a
401,436,672-byte sampled helper peak (the matrix also visits larger outputs).

Full release CLI retry through the same native conversion service/settings passes:
5,303 source/inferred/output frames, 2560x1440 H.264/AAC, 160,842,878 bytes.
Independent full A/V decode, every presentation timestamp, endpoint and non-silent
audio pass. Sampled helper peak is 239,550,464 bytes, not whole-app/strict total
VRAM. Total reported time is 528,719 ms (8m49s), including inspection/verification;
other checks ran concurrently, so this is not a comparative performance benchmark.
CPU codecs and counted host staging remain; real-time performance is not claimed.

The same input omits pixel-aspect metadata. Inspection/history treated it as
square pixels while AI preview incorrectly required an explicit 1:1 string.
Preview now accepts omission/N/A consistently, retaining non-square/transform
rejections. Actual ordinary-window RTX 2x start and seek to 31 seconds render;
managed setup rechecks automatically without another import/acknowledgement/test
click. The full export retry used the release CLI; a second full ordinary-window
export was not repeated. The rebuilt normal app is open on the source with RTX 2x.

Long-helper stderr capture retains a bounded tail; terminal structured failures
show their SDK/resource cause before the pipe error. Native tests:60 passed,
18 ignored; app:19 passed,6 ignored; Clippy with -D warnings, formatting, script
syntax and strict OpenSpec validation pass. An initial test-fixture recipe retained
1:1 metadata and was corrected to generated testsrc2/setsar=0 before the final
passing run. No duration/GPU/hardware-codec/precision envelope is broadened.

## Native RTX useful-size inputs and gray-frame fix — 2026-10-30

The local normal native build now supports the evaluated RTX3050Laptop/Vulkan,
driver621.01, SDK1.1 High3 path at **2x input<=1280x720** and **4x input<=640x360**,
output<=2560x1440. Source-aware choices are independent:720p offers2x,4x is disabled
with the360p/resource reason. Unsupported requests do not silently change scale.
Opaque8-bit SDR, square pixels/no display transform, explicit GPU and software-
codec export remain required. Native ONNX/Burn, AI NVDEC/NVENC, Main10/HDR, other
GPUs/drivers and larger/portrait envelopes remain unaccepted. Full long-duration
maximum-size stress and cross-platform acceptance are not claimed.

Evidence: [rtx-larger](evidence/ai-upscale-20261030/rtx-larger/). No SDK, runtime,
model, executable or video was added there. Short evaluation media remain ignored.
The20 exact RGBA8 helper/unmodified-SDK-sample comparisons include the original
12 tiny/odd images plus two patterns at each larger size/scale. All max error/MAE
are0;17 owned-process runs pass invalid protocol/geometry/color/identity/runtime,
cleanup and fresh retry. The original tiny output hashes are unchanged. The
new normal helper SHA256 is3dc1dd796acf36135d98ce64a01114b102faf9d752fc51dc89ed0457379ef12a.

Three initial module-audit failures are preserved. They did not fail numerical
comparisons: NGX was loading its pre-existing VSR cache `.bin`, not a module named
nvngx_vsr.dll. An OS module inventory found the loaded path. Its SHA256
c3d88eea5ff7a548edefa66414cf6e77464d0947277c904f324dd23abf58a1ed exactly matches the
supplied SDK DLL. The audit now records the actual cache module and requires that
same byte hash; it does not accept arbitrary cache/runtime updates. Case-insensitive
names and a post-first-evaluation inventory improve observability. Initial Windows
PowerShell script invocation was blocked by signature policy; builds succeeded
using the existing PowerShell host and imported installed compiler environment,
without changing policy/installing anything. SDK header warnings are external;
the release GNU linker warning about .drectve is preserved in build logs.

Larger geometry exposed an incorrect shared assumption: RTX admission reused
Real-ESRGAN's577x579 tile envelope. It now has a checked whole-frame ledger for
helper CPU/D3D surfaces, shared upload, downstream/retained images and estimated
opaque workspace.384MiB staged-resource admission,192MiB CLAHE scratch,1536MiB
helper sampled-local limit and256MiB remaining-budget reserve are unchanged.
1280x720 at4x produces5120x2880: isolated SDK equality passes, but full effects/
preview/export admission rejects before output creation. No resource gate was
relaxed. Fixed synthetic admission is182.84/309.40/317.31MiB for360p2x/360p4x/720p2x.
Actual helper DXGI sampled peaks191.34–659.72MiB exceed opaque workspace estimates;
these are not total application VRAM, allocator peaks or a strict total bound.

Visual inspection caught a more serious existing encoder bridge failure. The
first larger matrices passed timing/audio/full decode but began with gray frames.
Source decode, direct helper output and adjusted staged preview first frames had
structure. A GPU-independent reproduction traced the problem to omitted Matroska
DefaultDuration: FFmpeg inferred1,000,000fps from microsecond ticks and x264
average-bitrate control spent almost no bits on early images. The same pixels
and exact timestamps with an inspected12FPS nominal hint retained structure.
The bridge now writes that hint; explicit cluster PTS/BlockDuration still govern
VFR timing and the existing FPS filter governs selected CFR. It does not replace
the source timeline with a guessed constant grid. A real encoder regression checks
structured first-image content and unequal source intervals; a separate larger
raw RGBA Matroska roundtrip is byte-exact. Both initial gray contact sheets and
corrected complete chronological sheets are preserved. Original timing-only
reports remain labelled with their limited scope, not image acceptance.

After the fix, independent FFprobe grids/packet endpoints, full FFmpeg A/V decode,
audio peak and dimensions pass for24 CLI files:9 synthetic(360p2x/4x,720p2x times
Original/15/60FPS),9 licensed real-content Original-FPS files(three motion/face/text
clips,640x2682x/4x and1280x5342x), and6 tiny nonzero-origin VFR exports(both scales,
all three FPS modes). Licensed excerpts are24frames/2seconds, acquired by explicit
HTTP-range FFmpeg evaluation from the same CC-BY3.0 Tears of Steel source; original
soundtrack excluded, synthetic CC0 sine used. The full movie is not retained.
Acquisition commands, hashes and attribution are in the copied manifest. Nine
complete24-frame contact sheets and three unscaled center details were inspected:
no initial gray frames remain; enlargement retains softened/limited texture and
out-of-focus detail. No fidelity improvement, intrinsic flicker or model superiority
claim follows from these short, unpaired clips and applied CLAHE/manual settings.

Fixed synthetic wall time is6.48–11.87seconds for2seconds of source; licensed
files5.53–8.42seconds, including startup/downstream encode/full verification.
These functional observations are not a throughput benchmark; normal validation/
build work also occurred in the session. New metadata changed rate control and
output bytes, so old gray-run timings cannot represent accepted image performance.
No real-time playback/audio sync claim. Fresh helper startup per seek remains.

Six larger preview/chronological-processing comparisons at the three supported
source/scale cells pass0LSB in neutral and CLAHE50/manual10/115/80 settings.
Before/After restoration retains cached source-frame counts; endpoint requests
clamp to inspected final video PTS at all three cells. A licensed first-frame
preview diagnostic also retains spatial structure before/after effects. The
640x360 active-payload suite passes cancelled pipe write/read, timeout write/read,
helper crash, controlled processing-device destruction, budget rejection and seven
fresh transaction retries, with no published/partial output and owned helper reaping.
That suite ran before the later rate-hint fix; it proves lifecycle/transaction
handling rather than accepted encoded-image quality for those earlier retry files.
Its initial budget-test failure was an outdated error-label expectation after
splitting RTX admission, preserved then corrected without changing the guard.

Normal native window checks verify360p4x preview,720p4x refusal then explicit2x,
resolved2560x1440 dimensions and source-aware bitrate, actual720p2x preview and an
ordinary H.264/AAC UI export:24 input/output frames,4,041,478bytes, displayed total
7693ms. It independently passes timing/audio/decode. The19 final larger files
(18 CLI plus this UI export) also pass a separate all-frame spatial-structure
gate on their known non-flat fixtures; the initial gray matrix correctly fails.
This diagnostic is not a rule for rejecting users’ legitimate flat frames.
Both normal window themes fit without page scrolling. UI inference telemetry is
unavailable and recorded null. Local app-owned setup and saved component location
were updated to the evaluated helper; unchanged SDK acknowledgement was retained,
and an actual setup test saved fresh approval. Users need no JSON import. The
initial encrypted-file backup copy failed; prior preference fields are recoverable
from the earlier saved-setup evidence, and a reconstructed local snapshot is marked
as such. Original helper and SDK files remain unchanged. UI screenshots are scoped
to the converter and include the positive and unavailable scale states.

Locked validation: native59passed/18ignored(serial), application19passed/6ignored,
all-target native/app Clippy warnings denied, wasm32 shared-UI check, formatting,
OpenSpec strict validation. The initial parallel native run had a blocked-pipe
wall-clock deadline failure under concurrent work; unchanged thresholds pass on
the serial full run. Other initial test/Clippy failures remain in logs. Larger
input exposure does not complete the still-open joined lifecycle/audit/quality
matrix; tracked progress remains24/32. No SDK redistribution is authorized.


## Native RTX ordinary controls and remembered app setup — 2026-10-30

This continuation supersedes earlier setup/window-pending statements below.
Computer Use26.930.21537 was found in the bundled plugin and verified with actual
app enumeration. The normal `converter-native.exe` window was inspected through
`@oai/sky`, without diagnostic launch flags. RTX3050Laptop/driver621.01, Windows,
Vulkan processing, SDK1.1 High3 and the evaluated source-built helper are the
scope. Controls expose2x/4x only for inspected opaque8-bit SDR input<=192x108.
Native ONNX/Burn remain unavailable; larger inputs, Main10/HDR and AI hardware
codec export remain blocked. GPU/route choices remain explicit, no fallback.

Ordinary window evidence under `evidence/ai-upscale-20261030/native-setup/` covers:
2x cut export, 4x motion export, retained resize/Off, independent Before/Reset,
actual CLAHE50/manual10/115/80, source replacement, both supported themes and
compact color setup, unsupported export route/Main10 with working preview, and
missing-helper error-state Pause followed by restored-helper retry. Five-second
Pause attempts raced EOF; the successful error-state Pause observation used an
owned32-second loop and a clock held0:10 over20seconds. Its provenance is recorded.
The helper fixture was a byte-identical owned copy, restored after missing-file
checks; original helper/SDK files were unchanged. Initial color overflow was
observed, then fixed with a dedicated compact color view retaining the player.

Three actual ordinary UI exports independently pass complete timing grids,
packet endpoints, dimensions and full FFmpeg A/V decode (`independent-ui-stage3.json`):
cut2x Original4frames128x72; motion4x Original64frames768x320; motion4x60FPS320frames.
UI inference counters are unavailable and recorded as null, not reconstructed.
The4x Original file matches its earlier CLI matrix file byte-for-byte
(`ui-reference-equivalence.json`). The short60FPS output completed before Cancel;
its filename says cancelled but it is not cancellation proof. A longer loop job
was actually cancelled during Verifying: target absent, no matching partial or
helper process at audit, paused source preview restored. The disabled late Cancel
event changed completed status in the old build; its handler now checks running
before changing status. That exact final-handler window regression is pending.

The user rejected repeated import/terms/test steps. App-owned deployment metadata
now loads automatically from `assets/native-rtx.json` beside the executable;
`deploy-local-rtx.ps1` installs only that local metadata, refusing overwrite. No
machine paths are compiled into the app or committed as a deployment asset. This
is a local developer deployment, not a distributable SDK installation or general
LUID discovery. The current local asset uses observed LUID0:82840; stale0:78108
was rejected by the earlier real test. A Windows-session change may require
refreshing deployment identity. Existing evaluated helper/SDK stay external.

First panel opening inspects automatically. SDK acknowledgement and successful
explicit Test runtime & remember setup persist configuration/terms/helper hashes
in `%LOCALAPPDATA%/Diaxus/ai-setup.json`. On a real app restart, opening AI setup
started and completed the actual runtime test without Import, Inspect, another
acknowledgement or a Test click. The UI then offered both scales after source
inspection, and2x preview actually prepared. Evidence: `ui-app-owned-setup-saved.png`,
`ui-auto-resume-after-restart.png`, `ui-remembered-setup-scales-ready.png` and
`saved-setup-restart.json`. No configured helper existed at the collapsed-startup
audit instant. Saved JSON contains approval metadata, never a runtime result or
accepted capability; release/source/device checks still run. Advanced setup holds
developer import/model controls. Revocation removes remembered runtime approval.
The first persistence attempt failed with Windows OS17 during same-directory
rename in the encrypted application-data folder; its screenshot is preserved.
Synced direct writes fixed the actual save and relaunch. Truncated/corrupt files
fail strict bounded parsing; no consent or result can be recovered from them.

Playback had exposed a request beyond final decoded video PTS. Inspection now
retains exact relative final PTS and a metadata file stamp; AI preview clamps
before cache keys and rejects mutated files before decode. A normal2x motion
playback now reaches Ended0:05/0:05 with the last enhanced image and no EOF error.
This is a functional endpoint observation, not real-time/audio-sync evidence;
4x/CLAHE and long-loop endpoint window regressions remain pending. The initial
EOF error screenshot remains preserved. Source file mutation/reinspection test
verifies stale endpoint rejection and refreshed bounds. No new preview frame
buffers or extra timeline scan was introduced.

Validation: native library56passed/16ignored; native application19passed/6ignored;
locked all-target native/library Clippy with warnings denied; formatting; normal
native debug build; actual local RTX first-test and automatic-restart validation.
Preference tests verify missing/corrupt/oversized data, changed config/helper/terms,
revocation and overwrite. Detailed logs and screenshots are in the evidence folder.
No96-file release matrix repeat was needed for setup metadata and preview endpoint
changes. Full cross-platform lifecycle/acceptance/audit tasks remain open:24/32.

## Native explicit runtime setup and stale-device rejection — 2026-10-30

**Partial native setup implementation; progress24/32.** New ordinary setup
controls inspect an imported configuration without execution, then offer a
separate explicit runtime test. For RTX, the view displays the local supplied
SDK license path/hash and requires acknowledgement of those terms for local
execution in the current session. No configuration file imports consent or
release capabilities; startup/Off performs no runtime loading or acquisition.
The acknowledged hash is compared again with the bounded local PDF and assessed
SDK1.1 hash before executing the configured helper. Configuration/GPU changes
reset inspection/acknowledgement and cancel setup-test work. The shared busy gate
blocks conversion/asset changes, Cancel signals the existing helper ownership,
and results are compared with the current configuration/GPU after awaiting the
worker. Model import keeps its own cancellation scope.

### Actual runtime results

Evidence: [native-setup](evidence/ai-upscale-20261030/native-setup/).
`ai_preflight::tests::actual_rtx_setup_test_checks_consent_cancel_identity_output_and_cleanup`
was run explicitly with the existing locally authorized source-built RTX helper
and supplied SDK. The first attempt rejected the saved LUID. A diagnostic rerun
reported expected0:78108, actual0:82840, same NVIDIA RTX3050Laptop/DXGI1/vendor10de
and SDK1.1 High3. Both failures are preserved (`rtx-test.log`, `rtx-identity.log`);
there was no identity fallback or frame publication. Only after observing the
current identity was a separate ignored current-session configuration written.
The unchanged device gate then passed (`rtx-current-session.log`).

The successful RTX run processed two distinct original opaque16x12 RGB patterns
at2x and4x:32x24 and64x48, nonblack, alpha255, exact generation and distinct hashes.
Per frame, counted uploads were768bytes; downloads3072/12288bytes. Exact helper
runtime/configuration/DXGI/vendor/name/LUID checks and successful close passed.
No/stale acknowledgement, mismatched selected GPU, pre-cancellation and a missing
SDK root rejected; a passing result still failed the pending release-capability
check. `rtx-preflight.json` preserves identities, hashes, transfers, supplied terms
path/hash and `release_accepted:false`.

The analogous explicit DirectML/ONNX test passed with the existing hash-verified
Real-ESRGAN export and runtime1.23.0 on that same GPU/current LUID, CPU fallback
false. Both requested scales compute the model's internal64x48 result; this setup
test does not test the downstream2x display/export downscale. Its two helpers
closed successfully. See `onnx-test.log`/`onnx-preflight.json`. An initial PID-only
audit found PID4596 present, but process inspection identified its current occupant
as Windows `conhost.exe`, not the helper. The confirmed audit records each current
occupant/executable and finds zero processes matching either configured helper
executable. PID presence alone is insufficient cleanup evidence. Successful close
and this process audit are not claims about all driver allocations.

### Checks and explicit limits

- `cargo test --locked -p media-native --lib`:56 passed,16 ignored. The new normal
  test checks changed/missing/oversized terms, missing helper, mismatched GPU and
  pre-cancelled work without execution. Actual tests above were separate explicit
  invocations, not passing skips.
- `cargo test --locked -p converter-native --bin converter-native`:17 passed,
  6 ignored. Native Clippy/build, formatting and strict OpenSpec validation are
  recorded alongside the evidence. Initial Clippy rejected constant-size chunk
  iteration; it was changed to `as_chunks`. A later UI cancellation check attempted
  to call a private library method; the UI now uses public `is_cancelled` checks.
  Both failures are preserved, and the confirmed Clippy check passed.
- No desktop/Computer Use tools or current bundled Computer Use skill are exposed
  in this session. No repair, installation or automated desktop action was
  attempted. The setup view has compile evidence only: actual native checkbox,
  button/error/cancel behavior, both themes/layout, ordinary AI On preview/export
  and source/route/release acceptance remain pending. No selectable native backend
  follows from these synthetic outputs. Task7.1 remains partial; no checkbox is
  changed. Existing192x108 RTX limit, source precision restrictions and fresh-seek
  startup bottleneck are unchanged. The96-file release matrix was not repeated.

## Native RTX quality/performance comparison — 2026-10-30

**Measured native comparison; progress24/32.** Task8.2 gains a same-GPU release
comparison of RTX SDK1.1 High3, ONNX Runtime DirectML1.23.0/Real-ESRGAN and shared
wgpu bilinear enlargement. The ordinary native window/capability/consent path is
still unaccepted; tasks6.2/6.4/7.1–7.3/8.2 remain unchecked. The user chose to
make desktop tools available in a fresh session for the remaining window work;
none are exposed here. Ordinary2x/4x controls remain disabled, now with a reason
that names the remaining ordinary-window acceptance rather than missing service
integration. No new SDK authorization, installation or download was requested.

### Exact matrix and independent verification

All engines use the same existing licensed64-frame192x80,12FPS Tears of Steel
text/texture, face and motion excerpts, plus the four-frame64x36 CC0 cut/VFR/audio
fixture. Source hashes match their existing manifest. Source attribution and
soundtrack replacement remain explicit: CC BY3.0 movie excerpts, synthetic CC0
sine audio. Each case applies selected-resolution CLAHE50 followed by manual
brightness10/contrast115/saturation80, software libx264/AAC, identical source-aware
recommended bitrate and existing output-FPS policy. RTX is a **different model**,
not a matching-weight Real-ESRGAN parity result. RTX runs native2x/4x; ONNX uses
its validated internal4x and shared downscale for2x.

The first full matrix has **72 files**:3 engines x4 sources x2 scales x
Original/15/60FPS. It overlapped some compilation/tests, so its timings are not
used as the isolated performance result. A fresh serial **24-file Original-FPS
comparison** ran after those jobs stopped, with no compiler/test/verification
work launched concurrently. Verification ran afterward. These are single runs
per source/scale/engine, not repeated-trial distributions or guaranteed cold
driver caches. Normal OS/background graphics activity is not excluded.

**All96 files pass** independent FFprobe rational grids, source-frame counts,
packet presentation endpoints, selected dimensions, audible audio and complete
FFmpeg A/V decode. Inference runs once per source frame before FPS resampling;
the bilinear comparator runs no helper/model. Both initial and isolated results
are preserved under [rtx-quality evidence](evidence/ai-upscale-20261030/rtx-quality/).

### Measured time and memory

Windows/RTX3050Laptop/driver621.01, exact NVIDIA Vulkan processing adapter and
DXGI1/LUID0:78108 are used throughout. Release CLI external wall time below
includes process startup, adapter discovery, inspection, inference/processing,
codec work, verification and shutdown. Ranges span the three different64-frame
clips (each5.33 seconds), **not** the much shorter cut fixture:

| Engine | Scale | CLI wall seconds | Helper startup seconds |64-frame inference+helper IO seconds |
|---|---|---|---|---|
| Shared GPU bilinear |2x|1.81–2.06|0|0|
| Shared GPU bilinear |4x|1.90–2.07|0|0|
| RTX High3 |2x|5.18–5.87|2.99–3.56|0.164–0.179|
| RTX High3 |4x|5.88–6.19|3.27–3.50|0.318–0.335|
| DirectML Real-ESRGAN |2x|5.10–5.61|0.38–0.50|2.83–3.04|
| DirectML Real-ESRGAN |4x|5.14–5.66|0.37–0.41|2.83–3.11|

RTX inference/IO is much shorter in these cases, but **total export is not a
demonstrated RTX speedup** over DirectML because SDK startup dominates. The
current preview starts that helper again for a new seek/preparation. Keeping a
healthy compatible session alive is therefore a concrete optimization target;
it has **not** been implemented or validated by this measurement. These tiny
inputs do not establish interactive or real-time performance on larger video.

New `gpu_wall_times` report CPU intervals for processing-device setup, helper
startup, decoder pipe waits, inference roundtrip, postprocessing plus encoder
pipe writes, codec drain and helper shutdown. Existing total/preflight/conversion/
verification timers remain separate. Postprocessing includes downstream uploads,
CLAHE/manual processing, GPU completion/readback and pipe backpressure. Inference
includes helper pipe IO and SDK/provider transfers. Decode/encode processes run
concurrently; none of these intervals is pure GPU or codec execution, and they
do not partition every setup/header/mux operation. Pure kernel/transfer/encoder
durations remain unavailable, not zero. Detailed intervals and transfer counts
are retained in each report and `isolated-ranges.json`.

After successful RTX shutdown, the Rust owner now reads the existing bounded
helper log's final per-process DXGI sampled local peak. No SDK/helper kernel or
binary protocol changed. This yields approximately **60.88–60.90MiB at2x** and
**104.88–104.90MiB at4x** for these clips. DirectML reports about63.95MiB for its
helper. These are helper-process samples, not total application VRAM, strict
transient peaks or GPU-global usage. The shared processor/renderer and opaque
runtime allocations are separate. Conservative application admission is about
153.9MB/160.7MB for RTX2x/4x; this is not measured physical usage. The192x108 input
cap and existing budget guards remain unchanged; no HD/4K support follows.

### Image/temporal inspection and comparator policy

All eight chronological contact sheets (three licensed excerpts plus cut, both
scales) and three4x frame32 detail triptychs were inspected using the local image
viewer. Fine wires/outlines, text and skin/detail appearance differ: ONNX shows
stronger reconstruction/smoothing in the selected examples; RTX changes are more
modest relative to bilinear. This is not a fidelity ranking. Scene transitions
remain at matching positions. Small glyphs/textures are not reliably established
as faithful reconstruction from192x80. All consecutive encoded RGB differences
are also recorded without motion compensation. Genuine motion, CLAHE and lossy
compression affect them; no intrinsic-flicker threshold or ground-truth quality
claim is asserted. Static sheets are not real-time playback inspection. See
[the exact inspection record](evidence/ai-upscale-20261030/rtx-quality/visual-inspection.md).

The new `bilinear-video-pilot` is compiled only with `ai-upscale-pilot` (or tests),
requires AI Off, Original resize and explicit Shared GPU software codecs, and
uses the same normalization/effects/timestamp/codec path at selected2x/4x size.
Its192x108 evaluation envelope and checked even-factor sizing are separate from
ordinary Off policy. No ordinary UI/transport flag can select the reference
override. An actual7x5 edge/color pattern at2x/4x matches an independent scalar
bilinear sampler within **1LSB**, under the pinned2LSB gate. Ordinary Off still
rejects implicit enlargement with the expected error; unsupported reference
routes and AI settings reject before execution.

### Reproduction and remaining work

Use `native_quality.py` with the existing explicit CLI/helper/SDK/cache/runtime/
model paths and current verified LUID. Default runs all72 cases; `--fps original`
runs the24-file subset in a fresh directory. Set `AI_UPSCALE_NATIVE_VIDEO=1` and
run `verify_video.mjs` on each engine directory after conversion. Then run
`native_quality_inspect.py` for bounded contact sheets and frame differences.
[The runner guide](../tools/ai-upscale/README.md#native-release-quality-and-performance-comparison)
documents the commands. No runtime assets were fetched. Raw outputs/inspection
PNGs remain ignored; this evaluation folder is about79MiB, not a stored raw-frame
sequence. Provenance, measurements and compact verification evidence are tracked.

Native tests pass55/14 ignored, with the actual bilinear pixel test separately
passed. Both RTX and ONNX configured2x/4x preview-to-export handoff tests were
rerun after session-option changes and pass exact paused restoration and cleanup.
Native/app test Clippy, shared host and browser-app wasm checks, native build,
formatting, JS syntax, strict OpenSpec and diff whitespace checks pass. Initial
private-size-helper/Clippy issues and the rejected overlapping executable rebuild
are preserved in logs; a serial release rebuild succeeds. GNU linking still emits
the recorded `corrupt .drectve at end of def file` warning; the actual release
outputs nevertheless pass the independent decode/timing checks.

This comparison provides identified native quality/performance evidence. It does
not complete the joined ordinary UI/preview/release acceptance or asset/exposure
audit. Do not infer product support from these test-only capability records or
diagnostic reference commands. [Fresh-session handoff](evidence/ai-upscale-20261030/rtx-quality/next-native-window.md)
lists the ordinary native checks without requiring another duplicate matrix.

## Ordinary native AI configuration and actual handoff — 2026-10-30

**Progress24/32; tasks6.2/6.4/7.1–7.3 remain partial.** The ordinary native app
now has an explicit local configuration import/clear action and a common
preview/conversion configuration path for ONNX and RTX. Its production capability
metadata remains pending; this entry is not ordinary-window AI acceptance.
The current native executable was rebuilt successfully but has not been visually
inspected. No desktop Computer Use tool/skill is exposed in this session.

### Implementation and exact gates

`NativeAiConfiguration` reads at most64KiB, rejects unknown fields/backends and
relative asset paths, and matches the explicit Vulkan key/name against actual
adapters. Import loads no inference runtime, starts no helper and cannot provide
acceptance metadata. An actual local RTX configuration import passes on the
evaluated GPU while pending AI selection still rejects. The source-only example
uses placeholder asset paths/LUID; the working local configuration stays ignored.
Model/runtime/configuration changes set AI Off and retire its preview cache;
checking a model hash preserves configuration. No assets are installed/downloaded.
Setup busy state also blocks conversion in both the button and command handler;
an import/removal attempt retires selected AI before modifying model assets.
The native file picker supplies cancellation for configuration selection; the
setup cancel button is shown only when an operation owns a cancellation token.

The native panel now describes the configured ONNX or RTX backend and selects
the exact model/configuration/scale from separate capability records. Shared
native/browser event tests cover both backend modes, unavailable/mismatched
metadata, forged disabled/busy clicks, Off and frozen settings. Supplying RTX
mode to the browser panel still produces ONNX choices; browser RTX is not offered.
This is VirtualDom event testing, not native layout inspection.

The ordinary handler freezes helper paths, processing GPU, accepted scale/limits
and settings before preview retirement. `convert_configured_job` validates exact
backend/scale and the session adapter after serialized admission; a mismatch
releases admission before touching input/output. Accepted source/output limits
are applied to the actual inspected source before creating a partial file.
The app requires an explicitly selected Shared GPU software-codec route for AI
export. Failed NVDEC/NVENC, direct routes and Main10/HDR do not become supported.
Off takes the old conversion entry point and ignores configured AI assets.

### Actual configured entry point and independent exports

The existing opt-in preview-to-conversion handoff test now runs both ONNX and
RTX through the configured ordinary entry point, using **test-only injected
acceptance metadata**, never a product capability or a configuration-file flag.
On Windows/driver621.01, NVIDIA RTX3050Laptop/DXGI1/LUID0:78108 owns inference
and wgpu30 processing; independently owned wgpu26 IntelUHD renders the staged
display images. Both2x/4x cases use the tiny CC0 cut/VFR/audio source with CLAHE50.

All four cases pass active preparation retirement, stale sample rejection,
pre-cancelled conversion with no output, healthy verified four-frame export,
fresh paused preview with **identical display pixels**, and worker closure
concurrent with retirement. RTX's full test takes28.54s, ONNX10.73s, including
repeated initialization/verification/preview work; these are not throughput
benchmarks. Each backend produces a2x and4x H.264/AAC file. The separate FFprobe/
FFmpeg verifier checks complete rational source grids, packet presentation
endpoints, selected dimensions, audible audio and full A/V decode: **4/4 pass**.

Reproduce with the explicit existing asset/device variables in
[the runner guide](../tools/ai-upscale/README.md#ordinary-native-local-configuration-and-job-handoff):

```powershell
cargo test -p converter-native actual_ai_conversion_handoff_retires_and_retries -- --ignored --nocapture
tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/verify_native_handoff.py '<new-handoff-evidence-directory>'
```

Set `DIAXUS_TEST_PREVIEW_BACKEND=rtx` or `onnx` explicitly and run binaries
serially. `DIAXUS_TEST_AI_CONFIGURATION` plus the ignored
`actual_local_configuration_import_keeps_acceptance_separate` test exercises the
same configuration reader without executing a runtime. Earlier user consent for
local SDK evaluation remains in force; the app's consent/provisioning UX for
other launches/users and redistribution review are separate unfinished work.

### Verification and limits

Media-native passes54 tests/13 ignored (the explicitly run configuration import
and actual handoff tests are separately recorded). New ordinary tests reject
oversize/unknown/relative configuration, pending acceptance, wrong backend/scale
and unselected session GPU without output/admission leaks, reject geometry above
accepted limits, and complete Off conversion despite invalid AI asset paths.
Native app passes17 tests/6 ignored; UI passes2 tests. Native/app/UI test Clippy,
shared host checks, browser app wasm32 check/Clippy, native build, formatting,
strict OpenSpec and diff whitespace checks pass. An initial native test-Clippy
failure for test-module placement was fixed by moving tests after production
items; initial and passing logs are retained.

[Compact evidence](evidence/ai-upscale-20261030/native-ui/) contains handoff
reports, independent video checks and validation logs. Raw media and the working
local configuration remain ignored. No proprietary SDK files or new dependencies
are added. Ordinary window interactions/themes/layout, runtime/SDK provisioning
and consent UX, representative RTX/native quality/performance and final exposure
audit remain open. Native2x/4x controls are still unavailable. The existing fresh
helper per seek remains a performance limitation; no real-time playback or audio
synchronization claim follows from successful handoff tests.

## Native RTX preview service and renderer pass — 2026-10-29

**Tasks6.2/6.4 are partial; progress remains24/32.** RTX now works through the
explicit native preview policy and opt-in `--ai-preview-check` configuration.
This record covers actual service and renderer worker tests, not an inspected
RTX window or ordinary-launch UI acceptance. Ordinary native controls stay
unavailable. Browser work remains paused by user direction.

The real SDK1.1 High3 helper runs on Windows/NVIDIA driver621.01,
RTX3050Laptop/DXGI1, vendor10de/device25a2, verified LUID0:78108. Shared wgpu30
processing uses that NVIDIA Vulkan adapter. The separately owned wgpu26 renderer
selected IntelUHD Vulkan (8086:9a68). A bounded host display image explicitly
crosses the device/ABI boundary; no foreign texture is registered. Per-case
upload/readback counts and conservative admission bytes are preserved.

The service tests both native scales on the64x36 cut/VFR fixture at100000us and
233333us, and the192x96 fixture at0us, with CLAHE strengths0/50/80 and manual
brightness10/contrast115/saturation80 for nonzero strengths. Selected-size
processing precedes display fitting. Independent chronological preencoder
rasters and NumPy display-resize checks pass all **55 service/renderer/Before
comparisons at maximum1LSB**, below the pinned2LSB gate. The renderer subset
covers both scales on the cut fixture and4x on the larger fixture. Before/After
restores exact pixels; cached edits do not re-run inference. Cold Before works
with a nonexistent helper and zero inference transfers.

Actual worker regressions pass source replacement/seek invalidation,100 latest
pending playback samples without cancelling active inference (published0 and
200000us), missing-helper failure without fallback, error-state Pause, valid
explicit retry, retirement acknowledgement with empty stale results, controlled
renderer destruction and healthy generation43. This tests worker handoff;
ordinary UI preview-to-conversion handoff remains pending. The service takes
112.22s for its full test and the renderer82.14s; these totals include diagnostics,
independent processing and repeated startups, and are not conversion benchmarks.
Fresh preparation starts a new SDK helper/session. Responsive seeking/playback
has not passed performance acceptance.

Preview metadata now includes codec identity and validates SDR color tags before
normalization. Actual HDR-tagged8-bit and SDR Main10 fixtures reject, while the
ordinary SDR fixture passes. A backend/config mismatch rejects before helper
startup; the RTX192x108 input bound remains in force. No scale substitutes ONNX.

Reproduction uses the existing explicitly authorized normal RTX helper and
cache, with `DIAXUS_TEST_PREVIEW_BACKEND=rtx` and all documented RTX asset/device
variables. Run separately:

```powershell
cargo test -p media-native --features ai-upscale-pilot --lib actual_ai_preview_chronological_pixels_and_cache -- --ignored --nocapture
cargo test -p converter-native actual_ai_renderer_staging_cache_and_invalidation -- --ignored --nocapture
```

Use `native_preview_pixels.py` on both new evidence directories as documented
in [the runner guide](../tools/ai-upscale/README.md#native-window-preview-diagnostic).
Do not rebuild a running native test executable on Windows: the initial ordinary
app-test link overlapped the running renderer and failed permission denied;
the serial retry passes17 tests/6 ignored. Media-native passes51 tests/12 ignored,
including the new format/backend rejection tests. Both crates' test Clippy,
shared host checks, wasm media-web check, formatting, strict OpenSpec validation
and diff whitespace checks pass. The same service/renderer tests rerun with
ONNX/DirectML also pass55 independent comparisons at maximum1LSB; their reports
are preserved alongside RTX. No SDK installation/download or license scope
change occurred. Normal helper SHA remains
`e3424ad7c59f45b55b7f529898ed1dfb7f139b1a4cb4b5a75d27ee97bbdc7895`.

Compact evidence is in [rtx-preview](evidence/ai-upscale-20261029/rtx-preview/).
Raw diagnostic rasters remain ignored under `tmp`. Actual RTX window inspection
is pending: no desktop Computer Use tool/skill is exposed in this session.
Normal UI setup, ordinary-launch parity/export handoff, representative release
quality/performance and packaging/license review remain open. No screenshot,
real-time audio synchronization or spontaneous driver-loss result is claimed.

## Native RTX effects and active lifecycle pass — 2026-10-29

**Task5.3 passes; progress24/32.** The explicit native shared-wgpu RTX pilot
supports the tested staged `RTX -> selected-size CLAHE -> manual color -> FPS`
order. Ordinary native RTX preview/UI and full release quality/performance stay
open; this does not enable native controls or enlarge the192x108 input cap.
Earlier effects/lifecycle-pending notes below describe prior observations.

### Actual effects bridge and video

An actual RTX/native-GPU test feeds four normalized CC0 VFR source frames through
native2x/4x. Two independent selected-resolution GPU processors apply identical
chronological CLAHE histories at strengths0/50/100, one neutral and one with
brightness10/contrast115/saturation80. A scalar CPU manual transform of the
neutral CLAHE output predicts the combined output within **maximum1LSB** in all
six cases, below the pinned2LSB limit. This checks manual color after CLAHE on
RTX-enhanced pixels. Strength0 matches SDK pixels exactly; source/CLAHE maximum
RGB differences are45/91LSB at2x and46/92LSB at4x for strength50/100. Thus CLAHE
is neither bypassed nor insensitive to strength. Selected geometry and causal
source timestamps are explicit; no low-resolution substitute is equalized.

The ordinary diagnostic CLI then exports **2x/4x x Original/15/60FPS** with the
normal helper, CLAHE50 and the same three manual controls. All six files pass
native source/audio policy and separate FFprobe full rational-grid/endpoint,
geometry and complete FFmpeg A/V decode checks. Four source frames invoke RTX
four times and record16 CLAHE passes per file;4/5/17 output frames follow the
existing FPS policy. Duplicates reuse already processed images. Source is the
tiny checked-in64x36 CC0 VFR/audio fixture, not representative quality evidence.
The readback/upload bridge remains explicit and counted, not surface sharing.

### Actual active failures and retries

A separately compiled **RTX_DIAGNOSTIC** C++ helper initializes the real SDK on
the chosen RTX3050 and completes one frame, then deliberately stalls before
reading the second input payload, stalls before returning the second response,
or exits73 after real SDK evaluation/readback. The192x108 CC0 derivative used
for this test has82,944-byte input payloads, exceeding pipe buffering. Rust
records its actual IO stage; cancellation/timeout is not merely scheduled before
runtime startup. Test hooks and environment controls exist only in diagnostic
C++ / `cfg(test)` Rust. Production startup/frame deadlines remain60s; tests
shorten only the post-startup frame deadline to3s. No process-global environment
mutation or new unsafe Rust is used.

The strengthened final suite passes **seven cases**, all with CLAHE/manual
controls enabled and no published/partial output:

| Case | Observed boundary / failure | Explicit healthy retry |
|---|---|---|
| Cancel while writing | Rust blocked in payload write; real helper at before-payload barrier | Fresh normal helper, generation8, complete four-frame output |
| Cancel while reading | Rust blocked reading response header after SDK readback | Fresh normal helper, generation8, complete output |
| Timeout while writing | Same real write stall, shortened3s frame deadline expires | Fresh normal helper, generation8, complete output |
| Timeout while reading | Same real read stall, shortened3s deadline expires | Fresh normal helper, generation8, complete output |
| Helper crash | Deliberate exit73 after SDK evaluation/readback | Fresh normal helper, generation8, complete output |
| Processing-device loss | Controlled `wgpu::Device::destroy` after first processed frame | Fresh processing context/helper, generation10, complete output |
| Client memory budget | Preallocation ledger deliberately limited to1byte | Healthy budget/context/helper, generation10, complete output |

All five helper-fault cases verify cleared ownership **and independently query
Windows tasklist to confirm the owned PID no longer exists**. Their whole failure
intervals4.24–7.65s include inspection, real SDK startup, one completed frame,
pipe failure/3s wait and cleanup; they are not cancellation response latency.
Kills/crashes do not run orderly SDK destructors: OS/process retirement and
successful fresh retries are the evidence. Normal successful close separately
checks SDK cleanup, process exit and protocol acknowledgement. The receive path
also rechecks cancellation after receiving a reply to prevent a queued result
from escaping an already-cancelled request.

The device-loss test destroys the downstream wgpu context, not NVIDIA's D3D11
context or the physical GPU. The stalls bracket pipe IO, not interruption inside
an active proprietary GPU kernel. Budget rejection is not physical VRAM
exhaustion. Spontaneous driver/device loss and physical OOM remain unverified.

### Production separation, incompatible inputs and deployment

The rebuilt normal helper has **no diagnostic environment/control strings** and
successfully processes two frames even when fault environment values request a
stall. Its unchanged SDK image behavior also passes the complete17-process,
12-comparison, exact unmodified sample regression plus malformed/startup/cleanup
and fresh-retry cases. Normal helper SHA-256:
`e3424ad7c59f45b55b7f529898ed1dfb7f139b1a4cb4b5a75d27ee97bbdc7895`.
Diagnostic helper SHA-256:
`de604f17dcd171ff21dd74dc669e5cc3e194250c4f36dd99864dcef862e5da09`.
The separate diagnostic executable is not a production prerequisite.

Native tests reject direct effect routes and Main10 before source IO. Actual
CLI tests with the known10-bit SDR and HDR-tagged fixtures also reject without
publishing output; no precision reduction/tone mapping or automatic route/engine
fallback occurs. The failed AI NVDEC/NVENC route remains unavailable. The actual
DirectML regression still passes repeated reference pixels, active cancellation,
helper crash/reap and fresh retry after the shared receive change.

Environment remains the explicitly selected RTX3050Laptop, DXGI1,
0x10de:0x25a2/LUID0:78108, driver621.01, installed SDK1.1.0, MSVC19.51/tools14.51
and native Rust GNU/wgpu30. D3D11 public NGX VSR High3 create/evaluate/release/
destroy/shutdown is the SDK path; TrueHDR is disabled. API/runtime/SDK library,
header and license hashes are in the inventory. Supplied license PDF SHA-256:
`5addfcf7c78891a12f1dcd6b5f86c2512351ab71b3872f446ba10e1b6da54464`.
The user explicitly accepted these supplied terms for local evaluation. No new
acceptance, driver/toolchain/runtime download/install, proprietary Git content or
redistribution occurred. Deployment remains an optional source-only helper built
against a separately installed SDK with explicit paths; shipping license and
packaging review remain open. Local evaluation does not grant redistribution.

Compact [final lifecycle evidence](evidence/ai-upscale-20261029/rtx-lifecycle/active/report.json),
[effects references](evidence/ai-upscale-20261029/rtx-lifecycle/effects/report.json),
[independent effects-video checks](evidence/ai-upscale-20261029/rtx-lifecycle/video/independent-video.json)
and the pinned inventory/logs/source hashes are checked in. The initial narrower
five-case lifecycle run is retained separately. Helpers, objects, caches and
generated MP4s stay ignored; no raw frame sequence is persisted. The derivative
has a CC0 provenance/recipe/hash manifest. Reproduction is in
[the helper guide](../tools/ai-upscale/native-rtx-helper/README.md).


## Native RTX staged video export passes — 2026-10-29

**Task5.2 passes; implementation progress23/32.** The explicitly configured,
feature-gated native RTX export pilot now reuses the existing source-PTS/audio,
output-FPS, H.264 encoder/mux and independent verification policy. Ordinary
native RTX controls remain unavailable; effects/lifecycle, preview/UI and full
quality/performance acceptance remain open. Browser work stays paused.

`media-native::rtx_inference` implements the existing helper protocol without
Rust SDK FFI/unsafe code. ONNX and RTX share bounded child ownership, one frame
request in flight,64KiB retained stderr,60s startup/frame deadline,2s shutdown
wait and kill/reap retirement. Normal close checks both successful child exit
and protocol-worker completion; RTX verifies the shutdown acknowledgement.
Actual active RTX cancel/crash/timeout acceptance remains task5.3. The dispatcher
now names generic frame processing explicitly; portable inference retains its
internalx4 path while RTX generates the selected **native2x/native4x** directly.
Startup checks owned PID, SDK1.1/quality3/color/credit policy, explicit DXGI1/name/
NVIDIA/LUID0:78108 and downstream NVIDIA Vulkan name/vendor/device. No resource
handle crosses SDK/wgpu contexts; LUID is not treated as a shared-device handle.

The evaluated environment remains RTX3050Laptop, driver621.01, SDK1.1.0 and the
previously built/source-hashed C++ helper. Rust1.98.1 GNU and wgpu30 use the native
**debug** build; codec execution is explicitly **software FFmpeg/libx264 + AAC**.
The final CLI SHA-256 is
`bcc558757c25386794af9214c769d5ec1b204b2a557de211e6ca880abf668417`.
No NVDEC/RTX/NVENC surface sharing, hardware-codec fallback, package redistribution
or release-throughput claim is made.

### Actual video, pixels and accounting

Three independent six-file matrices pass the checked-in CC0 synthetic
`ai-upscale-vfr.mp4`:64x36, four source frames, B-frame/VFR presentation coverage
and nonzero video/audio origins. Each matrix tests **2x128x72 /4x256x144 ×
Original /15FPS /60FPS**. Every source frame runs RTX once (4 inference calls per
file); output frame counts are4/5/17 respectively. Fixed-FPS duplication reuses
processed images instead of repeating inference. All18 outputs pass native full
inspection/verification and the separate FFprobe rational-grid/presentation-end
and complete FFmpeg A/V decode checks. Audible AAC signal is nonzero; existing
native audio start/duration tolerance is25ms and video timing tolerance1ms,
not bit-exact audio. Final independent maximum PTS error is0.667ms (CFR codec
time-base rounding), maximum presentation-end error0.000334ms.

Both first-to-repeat and repeat-to-final comparisons pass **byte-identical MP4
files** and decoded RGBA frame hashes for all six configurations, despite fresh
helpers/jobs and the final stricter shutdown checks. Reproducible comparison uses
`tools/ai-upscale/compare_native_video.py`; no raw frame sequence is persisted.

A separate actual Rust GPU test sends both original192x108 patterns at both
selected scales through the production client. All four first-frame outputs
match the pinned unmodified SDK sample SHA-256s exactly, and all four immediate
repeats match. Passing those pixels through the actual neutral native wgpu
preencoder stage gives **maximum0LSB**, inside a pinned1LSB limit. This verifies
that SDK-enhanced pixels reach native preencoder processing, not merely that a
helper started. It does not establish fidelity/temporal quality on real content.

For each four-source-frame file, helper host upload is **36,864bytes**;
helper output readback is **147,456bytes at2x /589,824bytes at4x**. Downstream
wgpu upload and readback each add the same selected-resolution byte totals and
remain separate report fields. All codec GPU-transfer counters are0 because
codecs are software. The shared job ledger admits150,756,608bytes at2x and
151,198,976 at4x using conservative allowances; this is not measured total SDK
VRAM. `helper_memory_sampling_available=false` explicitly makes its numeric peak0
**unavailable telemetry**, not zero memory use. The C++ helper's own sampled DXGI
admission guard stays active; opaque allocations remain qualified. Final whole
job totals3.845–5.503s include startup/inspection/encode/verification and four tiny
source frames; no pure-inference throughput or native-vs-ONNX ranking is inferred.

### Rejecting paths, regression and remaining gates

Five actual CLI negatives reject wrong LUID, Intel downstream processing GPU,
unvalidated manual effects, failed AI NVDEC/NVENC timing route and a known
640x360 source above the **192x108 pilot input cap**. No output/partial is
published. Separate native tests reject RTX manual/CLAHE and Main10 before source
IO; HDR/10-bit source rejection remains the existing native format contract.
Geometry is rejected instead of resized or tiled. No helper/runtime is installed
or downloaded and no alternate engine/GPU/codec route is selected on failure.

Actual DirectML regression passes shared-owner frame pixels/counts, active cancel,
crash/reap and fresh retry; an ONNX CLI export with CLAHE/manual controls and
complete A/V decode also passes. Native tests49pass/0fail/10ignored; the actual
RTX/ONNX tests were separately invoked and pass. Host native app/shared crates,
wasm browser crate, native Clippy with warnings denied, formatting and UI tests
pass. Ignored unrelated hardware tests are not claimed as runtime coverage.

Task5.3 remains open: validate the RTX effects bridge and its operation order,
active pipe cancellation, timeout/crash/device failure, partial cleanup and
healthy retry. Native RTX preview/ordinary UI, larger input envelopes, shipping
license/deployment and full release content/performance gates are also open.
The UI reason now describes the remaining preview/release integration work.

Compact reports, command arguments, CLI/source hashes, rejecting cases and Rust
GPU/regression logs are in
[evidence/rtx-video](evidence/ai-upscale-20261029/rtx-video/final/native.json).
Generated MP4s stay under ignored `tmp/ai-upscale/rtx-video-{first,repeat,final}-20261029`;
no SDK/runtime binary, new downloaded asset or raw frame sequence enters Git.
[The helper guide](../tools/ai-upscale/native-rtx-helper/README.md) contains explicit
native CLI/matrix/repeat commands and session-specific DXGI/LUID cautions.


## Native RTX bounded helper and SDK sample equivalence — 2026-10-29

**Task 5.1 passes; implementation progress is 22/32.** The optional source-only
C++ helper now exchanges bounded RGBA frames over versioned binary pipes. The
Rust application has no new unsafe/FFI boundary. Native video/preview integration
and ordinary RTX selection remain pending tasks 5.2/5.3/6/7; this does not enable
native controls. Earlier task1.5-only records below describe prior observations.

The final actual GPU suite contains **17 process runs** and **12 exact sample
comparisons**: two original opaque synthetic patterns at 192x108, odd 191x107,
and 64x36, each at2x/4x, SDK High quality3, TrueHDR disabled. Reference tolerance
was pinned before execution at **RGBA8 maximum error0 / MAE0**; every comparison
passes and a fresh process reproduces the 192x108-to768x432 hash exactly. The
reference executable links the external **unmodified NVIDIA SDK sample**
`rtx_video_api_dx11_impl.cpp` (SHA-256
`e85050f46c76010107dd2f26f5b91deb1b47a06434e8ca6a19b6ee2483461f58`).
Only texture ingress, defined black/opaque destination initialization and output
staging are shared with the harness; feature create/evaluate/shutdown use the
sample's separate wrapper. This is SDK sample equivalence, not Real-ESRGAN parity
or a claim of improved visual fidelity/colorimetric accuracy.

Both executables compile with the previously verified MSVC19.51/tools14.51 and
SDK1.1 static library, with only the two existing SDK unused-parameter warnings.
Exact DXGI1 RTX3050Laptop **0x10de:0x25a2, LUID0:78108**, PID and name are checked.
The helper loads VSR from the supplied SDK root. The reference's default feature
discovery uses local hard links to the separately installed DLLs beside its
ignored executable; actual module inventory and byte hashes prove the same VSR
runtime. No proprietary files enter Git or the distributable application. The
runtime/driver/license environment is unchanged from task1.5.

Protocol1 has a 40-byte explicit little-endian header, a bounded JSON startup
identity, strictly increasing request IDs, one fixed nonzero generation and one
frame in flight. It accepts declared full-range encoded RGB, BT.709 primaries,
sRGB transfer, RGBA8 alpha255 only; output echoes the color identity without
inferring or converting source tags. HDR/10-bit/alpha and unknown color tags are
rejected. Input is conservatively capped at **192x108**, with only tested shapes
claimed as runtime evidence; no arbitrary 1080p/4K support is inferred. One input
upload and one output readback per frame are explicit. No raw-frame sequence is
persisted. At maximum geometry, application pixel buffers total1,410,048 bytes;
explicit D3D11 input/output/staging pixels total2,737,152 bytes, excluding opaque
runtime/driver allocations and alignment. Sampled DXGI admission requires256MiB
remaining budget and process-local usage<=1536MiB; it is not a strict VRAM cap.

Each 12-frame helper/reference run accounts for **695,632 upload bytes** and
**6,956,320 readback bytes**. Sampled local peaks are49,197,056 bytes (helper) and
49,180,672 (sample); sampling can miss peaks. Whole process intervals5.885s and
3.178s overlap across the two processes and include startup, pipe exchanges,
comparison and waiting; these are **not inference throughput or a speed ranking**.
There is no NVDEC/RTX/wgpu/renderer surface-sharing claim.

Actual rejecting cases cover wrong version, oversized header, invalid scale,
color/length, zero/changed generation, repeated request ID, nonopaque input,
truncated header/payload, wrong adapter and absent SDK runtime. Malformed-frame
rejections perform no additional SDK evaluation; owned feature/parameter/device
cleanup succeeds. Clean shutdown and clean-boundary EOF pass. Wrong-adapter and
missing-runtime failures happen before NGX initialization and publish no binary
success response. Fresh retry passes after failures. Raw NGX status plus enum
meaning are preserved (platform error is not relabeled unsupported GPU).
The refactored standalone probe also passes its full six-process regression
(16 independently inspected outputs plus fresh retry). Its actual missing-resource
SDK rejection is `0xbad00002` with meaning `platform-error`; the exception now
retains the same raw code, and all cleanup statuses succeed.
Application-owned timeout/cancel, active pipe failure/crash, spontaneous device
loss/OOM, real source PTS/audio, effects bridge and native preview/UI remain open.
The Python evaluation owner bounds IO/deadlines and kills/reaps on failures;
production Rust lifecycle integration is still required.

Reproduction and wire specification are in
[the helper README](../tools/ai-upscale/native-rtx-helper/README.md). Compact
[report, logs and source hashes](evidence/ai-upscale-20261029/rtx-helper/report.json)
are checked in; executables, objects, SDK links and caches stay under ignored
`tmp/ai-upscale/rtx-{helper,sample}-accepted-20261029`. Final suite:
`tmp/ai-upscale/rtx-helper-accepted-suite-20261029`. Earlier intermediate suites
also passed their narrower checks and remain ignored; final evidence supersedes
their earlier handshake/color labels. No browser work was performed.


## Native RTX SDK feasibility passes — 2026-10-29

**Task 1.5 is complete; progress is now 21/32.** After the user accepted local SDK
evaluation terms and installed the C++ tools, the actual D3D11 RTX SDK probe
initializes, reports VSR available, evaluates two known inputs at both 2x and 4x,
and explicitly cleans up on the selected RTX 3050 Laptop GPU. This is a passing
**feasibility pilot**, not native UI/export support or SDK-demo equivalence.
Older prerequisite/execution-pending notes below are historical.

### Exact environment and build

The user installed MSVC tools **14.51.36231**, compiler **19.51.36247** x64,
Windows SDK **10.0.26100.0**, in the existing VS Enterprise 2026 installation.
This differs from the initially suggested v143, but the standalone actual build,
link and execution pass; no extra toolset was required or installed. Rust/GNU,
wgpu, native codecs and application dependencies were unchanged. No CUDA/Vulkan
SDK, MFC/ATL, driver installation or proprietary redistribution occurred.

Installed RTX Video SDK **1.1.0**, supplied static `nvsdk_ngx_s.lib`, D3D11 and
public NGX APIs are used from their external paths. The first real link failed
on missing `GetWindowThreadProcessId`/`MessageBoxA` imports; adding **user32.lib**
fixes it. This was a probe link omission, not an unsupported SDK/GPU. SDK helper
headers emit two unused-parameter warnings; there is no ABI bypass or ignored
link error. Source-only probe/build/evaluator are under
`tools/ai-upscale/native-rtx-probe`; no SDK code or binaries are vendored in Git.

Explicit **DXGI index1**, exact name **NVIDIA GeForce RTX 3050 Laptop GPU**,
vendor/device **0x10de:0x25a2**, LUID **0:78108**, driver **621.01** are recorded.
Index0 was Intel UHD Graphics and is used only for a rejecting negative test.
The probe does not switch GPU, load Real-ESRGAN or use CPU inference on failure.

Runtime module inventory proves `nvngx_vsr.dll` comes from the explicit SDK's
`bin/Windows/x64/rel` directory. Its SHA-256 is
`c3d88eea5ff7a548edefa66414cf6e77464d0947277c904f324dd23abf58a1ed`.
The SDK's `nvngx_truehdr.dll` is also loaded for capability discovery; no TrueHDR
feature is created or evaluated. The driver's `_nvngx.dll` path and all three
hashes are recorded in the report. SDK initialization and VSR availability do
not independently establish permissions for redistribution or universal support.

### Actual outputs, repeatability and failures

Two original synthetic **opaque RGBA8 SDR 192x108** patterns exercise **2x
384x216** and **4x 768x432**, requesting **High quality3**, with SDK **Bicubic0**
as a separate diagnostic baseline. All eight raw outputs per successful process
have exact byte counts/dimensions, nonblack RGB and alpha255. Source texture
readback independently matches every uploaded byte. High output differs in RGB
from SDK Bicubic, and the two enhanced input responses differ. This verifies
that the requested operation returns real different images, not improved
fidelity or artificial equality to Real-ESRGAN's different model.

Two complete independent evaluation suites pass. Each suite has three successful
fresh-process runs (first, repeat, fresh after failure), checking exact SHA-256
repeatability of all eight images, plus three negative paths. The driver/SDK
reports `0x00000001` for initialize, capability query, feature creation, all
evaluations and release/destroy/shutdown. Needs-driver-update is0, and scratch
request is0 on this tested configuration. Process wall time of a successful
complete four-case probe is approximately3.2–3.5 seconds including startup,
all eight evaluations, diagnostic copies and disk writes; **this is not video
throughput or pure GPU inference timing**.

The first strict all-output repeat assertion failed on SDK Bicubic at 2x for
pattern1: maximum29 LSB across56 pixels/136 RGB channels, bounding box x349–369,
y5–7. All High images were exact repeats. The destination texture previously
had undefined initial contents; it is now explicitly GPU-cleared to opaque
black before **each** evaluation. Both complete strict-repeat suites then pass
without relaxing any threshold. The old failure, old probe source/binary and
measurements are preserved. This observation does not prove a driver root cause
or universal determinism. A separate exploratory SDK version-zero initialization
was accepted, so it was not used as an error-path proof; that mode was removed
and its original log remains preserved.

Negative checks are actual runs, not mocked SDK results:

- Explicit selection of index0 reports the Intel identity and rejects before
  SDK initialization, with no NVIDIA/CPU/route fallback.
- An empty SDK directory reports the exact missing runtime file before init.
- Missing input/output resources on an initialized VSR feature produce raw NGX
  **0xbad00002 (PlatformError)**. The probe preserves its failing boundary and
  code, rather than calling it an unsupported GPU. Feature release, app-owned
  capability-map destruction and per-device shutdown still return success. No
  image is published; a fresh successful process afterward matches all outputs.

The two complete suites each verify these same paths. Physical unsupported GPUs,
old drivers, spontaneous loss, OOM, helper cancellation/timeouts and SDK recovery
inside a still-live application are not inferred from them.

### Copies, scope and next work

Each successful four-case process has four explicit source uploads (**331,776
bytes**), four diagnostic input readbacks and eight diagnostic output readbacks
(**6,967,296 bytes total readback**), plus eight GPU output clears in total. No zero-copy or codec-surface sharing is claimed.
SDK/driver workspace/physical VRAM, internal copies and total memory envelope
remain unmeasured. Bounded synthetic rasters and raw outputs stay ignored; only
compact reports, logs and one diagnostic contact image are in evidence. Contact
image SDK Bicubic is left, High is right; it was visually inspected as a limited
synthetic operation check, not a natural-content quality gate.

Evidence: [feasibility summary](evidence/ai-upscale-20261029/rtx-feasibility/summary.json),
[final independent suite](evidence/ai-upscale-20261029/rtx-feasibility/evaluation-final.json),
[first passing suite](evidence/ai-upscale-20261029/rtx-feasibility/evaluation-first.json),
[initial repeat failure](evidence/ai-upscale-20261029/rtx-feasibility/initial-repeat-failure.json),
[toolchain](evidence/ai-upscale-20261029/rtx-feasibility/compiler-version.txt),
[hash manifest](evidence/ai-upscale-20261029/rtx-feasibility/sha256.json).
Build-script PowerShell parsing, Python syntax, strict OpenSpec and whitespace
checks pass; actual release-optimized C++ build and GPU runs are the relevant
implementation checks. Product Rust/UI code is unchanged in this slice.

Five-cell update: native RTX moves from **untested** to **passing-pilot
(D3D11 synthetic feasibility only)**; native ONNX remains diagnostic-only,
Burn remains unavailable and browser exposure is unchanged. Task5.1 is next:
bounded helper protocol, SDK reference/sample equivalence and exact native
processing-device identity. Then real video timing/audio, shared color effects,
preview, lifecycle and ordinary native UI gates must pass. **RTX/native controls
remain unavailable** until those checks; no unchecked implementation is claimed.

## Native-first RTX feasibility preparation — 2026-10-29

The user reprioritized native and RTX over further browser work. RTX task 1.5 is
now the first active gate. The user explicitly accepted the installed NVIDIA
terms for **local SDK compilation/evaluation**; the earlier execution-pending
statements below are historical. Redistribution/release licensing and automatic
prerequisite installation are not authorized by that answer. No SDK execution
has occurred in this continuation.

Fresh inventory confirms RTX 3050 Laptop GPU, NVIDIA driver 621.01, the supplied
SDK 1.1.0, existing Windows SDK 10.0.26100.0 and LLVM clang-cl 18.1.2. Visual Studio
Enterprise 2026 18.7.11903.348 is registered, but its C++ component and VC/Tools/MSVC
are absent. An actual LLVM compile with explicit installed Windows/RTX include
paths fails at **missing `excpt.h`** included by Windows.h, before SDK code can
compile. LLVM presence alone does not provide the Microsoft CRT/STL headers or
libraries needed for this SDK build. No RTX capability or hardware failure is
inferred from this prerequisite failure.

A source-only standalone D3D11 probe is prepared in
`tools/ai-upscale/native-rtx-probe`. It uses external installed SDK headers/static
library, exact selected NVIDIA/DXGI identity, bounded synthetic opaque SDR RGBA8
inputs, 2x/4x High versus SDK Bicubic output checks, explicit upload/readback and
NGX status/cleanup reporting. It is **an uncompiled draft**, not a passing
implementation, runtime capability check, sample-equivalence or video/preview
acceptance. No SDK headers/libraries/DLLs/models were copied into Git. D3D11 avoids
introducing CUDA/Vulkan/MFC/ATL dependencies into this standalone probe; actual
compilation must still prove the prerequisites/linkage and runtime behavior.

The installed VS catalog identifies
`Microsoft.VisualStudio.Component.VC.14.44.17.14.x86.x64` as v143/v14.44 x64/x86
build tools, matching the shipped samples' toolset generation. Installation was
explicitly requested from the user and is **pending**, not performed. Existing
Windows SDK can be reused; no full workload, CUDA/Vulkan SDK, MFC/ATL, driver
upgrade, OS graphics change or reboot was performed. Observed free disk was
C:21.02 GiB / D:14.70 GiB; component download/installed size has not been established
by an actual installer plan and is not claimed.

Evidence: [status](evidence/ai-upscale-20261029/status.json),
[prerequisites](evidence/ai-upscale-20261029/prerequisites.json),
[compile prerequisite failure](evidence/ai-upscale-20261029/llvm-build-prerequisite.log).
Build-script PowerShell parsing and whitespace validation pass. Product Rust/UI
code was not changed. **20/32 tasks remain complete**, with task 1.5 and native
ordinary exposure still open. No browser follow-up work was undertaken.

## Browser NVIDIA performance route — 2026-10-26

After the user reported unusably slow preview **and** export on Intel, actual
adapter experiments found that Edge 156.0.4314.42 returns `intel / gen-12lp` for
both low-power and high-performance adapter requests. Launching a separate Edge
process with **`--force-high-performance-gpu --use-angle=d3d11`** instead returns
**`nvidia / ampere`**. Vulkan/ANGLE in the tested launch returned no adapter and
was not enabled. These are observed results, not assumptions about launch flags.
The machine's known discrete GPU is RTX 3050 Laptop GPU; browser device/description
and driver fields are not exposed, so the capability is scoped to observed
vendor/family on this Windows/Edge configuration, not universal Ampere support.

Normal builds now also accept this ONNX GPU family within the unchanged secure
isolated origin, model/runtime, FP32 preprocessing, source limit 577×579 and
opaque 8-bit SDR bounds. This uses **the same Real-ESRGAN/ORT Web model and shared
Rust processing device**, not RTX Video SDK. No SDK license or binary is involved.
Native ordinary AI, both Burn candidates, unknown browser/GPU contexts and AI
HEVC remain unavailable. No source geometry is reduced to make AI faster, and
2x continues to run x4 inference followed by the tested shared downscale.

### Reference, video, UI and lifecycle evidence

- Fresh default-compiler main/worker caller-owned FP32 probes pass all five
  shapes: max error **6.318092346191406e-6**, within unchanged max/MAE gates.
  Exact device/buffer identity, one device, session release and post-release
  usability pass. All six shared tiled shapes through 577×579 pass in each
  context, max **1 RGBA8 LSB**, null GPU validation errors and bounded ownership.
- Three identical licensed 64-frame 192×80/12-FPS text/texture, face and motion
  excerpts, 2x/4x × bilinear/AI × main/worker: **24 benchmark exports** pass
  independent full FFmpeg A/V decode, geometry, complete grids and endpoints.
  Same weights/normalization/CLAHE/manual controls/profile/source FPS are used.
- Ordinary app UI main/worker × light/dark exports **eight** 64-frame H.264/AAC
  files, all independently verified. Before/After, Reset retaining AI, selected
  geometry/bitrate, Play/Pause, seek, conversion handoff and Off restoring saved
  ordinary resize pass. The initial visible worker/dark run also passes
  missing-model removal/reimport/retry, stopped clock and source-size rejection,
  with **no unsafe WebGPU launch flag**.
- Actual lifecycle diagnostics pass both scales/contexts: **eight** handoff/retry
  exports independently verified, preview restoration, release-response failure
  and fresh contexts. Active preview tests pass **12** cancellation/runtime-error/
  device-destruction cases, **40** successful previews, distinct source replacement
  and exact retry/restore pixels, zero application-held ownership. Canvas pixels
  are unchanged on cancel/runtime failure; lost-device display contents are not
  asserted. Readbacks are diagnostic and excluded from benchmark timing.
- H.264 Original/15/60 FPS, both scales/contexts: **12** independently verified
  tiny VFR exports. FFmpeg WASM H.264/AAC supported 15/60 FPS, both scales/contexts:
  **eight** independently verified exports, retaining its explicit output readback
  and existing OPFS/audio restrictions. No hardware codec execution claim.

All use existing local evaluation assets; no new dependencies/models/downloads.
The same CC BY 3.0 **Tears of Steel (2012), Blender Institute; directed by Ian
Hubert, produced by Ton Roosendaal** fixture provenance and synthetic CC0 audio
manifest from Oct 25 apply. All-64-frame 4x overviews for the three NVIDIA clips
were visually inspected against same-size bilinear. The same sharpening,
smoothing and reconstructed fine texture/text limits remain. No gross extra
flashes were apparent in those overviews; this is not a ground-truth fidelity
or universal flicker/stability guarantee. Six-case offline temporal differences
and contact sheets are under ignored evaluation storage, not source assets.

### Measured improvement and limits

| 64-frame AI H.264 conversion | Main | Worker |
| --- | ---: | ---: |
| Earlier Intel seconds | 37.533–46.923 | 38.162–44.352 |
| NVIDIA seconds | **3.256–3.678** | **3.282–3.654** |
| NVIDIA AI processing seconds | 3.173–3.584 | 3.201–3.562 |
| NVIDIA setup ms (cold first / later warm) | 59.8–585.3 | 67.6–623.8 |
| NVIDIA verification ms | 7.3–10.3 | 6.7–15.0 |
| NVIDIA diagnostic wall seconds | 3.340–4.293 | 3.380–4.315 |

The paired observed conversion improvement is about **10–14×**, for these three
5.333-second clips at these sizes. The Intel run explicitly disabled DXC; the
NVIDIA run uses its default compiler plus the GPU launch option, so this is an
observed GPU/configuration comparison, not an isolated hardware-only effect.
Both benchmarks ran sequentially without concurrent evaluation GPU work. UI and
failure tests were separate. Cold setup is outside conversion time; wall time
includes setup, validation and artifact collection. Timers overlap and include
CPU scheduling/queue waits; they are not pure hardware durations. Larger sources
and interactive audio-synchronized playback speeds are not inferred.

NVIDIA benchmark requested-resource window peak was **78,448,736 bytes**, with no
unknown texture requests. WebCodecs runs report zero explicit conversion pixel
readbacks, zero application-held frames/samples/leases at completion and expected
ingress copies/canvas captures. Opaque runtime/compiler/driver/surface allocations,
GC and browser-internal copies remain unknown. The unchanged 384-MiB admission
policy is not a physical VRAM guarantee.

### Additional profile and build failures, preserved accurately

The wider WebM matrix passes browser-side checks, but independent FFprobe output
omits VP8 packet durations. The verifier rejects `BigInt(undefined)` at the exact
endpoint calculation; no duration is guessed or tolerance relaxed. The first
attempt also used the nonexistent `webm-vp8-video` profile (harness mistake),
corrected to `webm-vp8-video-only`. Original failed reports/artifacts remain
ignored. The accepted H.264 subset preserves original request/artifact lineage
and independently passes. **NVIDIA AI WebM remains unavailable**, in both backend
conversion and selected-profile UI reporting; Intel's earlier exposure is not
expanded by this failed check. Preview remains available, and H.264 must be
chosen explicitly. Full cross-platform tasks remain open.

Testing that profile rejection exposed an actual recovery defect: the selector
was disabled when the selected profile was unavailable. It now remains usable
if another exact probe passed, while Convert stays disabled. AI profile changes
re-probe current selection, and backend checks remain independent of UI state.
Initial failures and the final clean-build visible worker/dark recovery check
are retained separately; the latter passes both scales, unavailable WebM with
working preview, explicit H.264 recovery, missing assets/retry/Off and limits.

Another late test failure showed two `// Actually perform the load` bootstrap
blocks and duplicate source-file controls in generated debug JavaScript after
concurrent dev-server/separate builds. The initial source event could update a
different mounted app. The owned dev server was stopped; generated bindings were
preserved by a checked move within their build-output directory, then rebuilt.
The clean generated file contains **one** bootstrap block. No source workaround
or user browser/default profile data was changed. Avoid simultaneous `dx serve`
and independent `dx build` writing the same output. The existing clean build is served by `tools/serve-web-build.mjs`, a localhost-
only static server with isolation/no-store headers and no rebuild watcher. A
visible persistent-profile smoke test confirms one source control, NVIDIA
preview and zero external requests with the cached model/runtime. A fresh
browser reload uses the clean output. Failed reports remain evidence, not passing skips.

### User-facing launch and verification

`tools/ai-upscale/launch-nvidia-browser.ps1` checks installed Edge 156 and local
server isolation headers, then opens a separate profile under ignored
`tmp/ai-upscale/edge-nvidia-profile`. It installs/downloads nothing and changes no
Windows graphics/default-profile settings. This separate process lets the GPU
argument take effect even while ordinary Edge is running. A visible persistent-
profile test against the actual local server confirmed NVIDIA, secure isolation,
H.264-only accepted status and the hash-verified model/runtime cache. Existing
local assets were imported/acquired through explicit test UI actions; network
requests were fulfilled from the pinned evaluation files. This does not prove
fresh CDN availability. The user still loads a source and selects H.264 and AI.

Evidence is under [ai-upscale-20261026](evidence/ai-upscale-20261026/): adapter
experiments, default-compiler numeric/tiles, benchmark/performance comparison,
ordinary UI/independent verification, lifecycle/active cases, H.264 FPS/FFmpeg
checks, visible-window reports/screenshots, original verifier/profile/bootstrap
failures and final checks. Large videos/raw parity frames/contact sheets remain
ignored. No new model, runtime, proprietary SDK binary or dependency version was
added. Model BSD-3-Clause and ORT Web MIT inventory/hashes are unchanged.

Rust formatting, wasm media-web/converter-web Clippy, native app host check,
UI tests (2), JS transport/clock/history tests (16), JS/launcher syntax and strict
OpenSpec/whitespace checks cover the changed source. Overall **20/32** remains;
this performance fix does not complete native/RTX/full cross-platform acceptance.

## Browser AI controls enabled — 2026-10-25

The normal `converter-web` build now enables its optional `media-web/ai-upscale`
engine. **ONNX 2x/4x is selectable on the validated Windows Edge 156, Intel
`gen-12lp` BrowserWebGpu route, on a secure cross-origin-isolated origin.** The
actual browser was Edge **156.0.4314.42** on Windows; the GPU driver version was
not exposed. OS/browser-major/GPU-family matching bounds availability; it is not
an assertion that every driver/device in that family was tested. Unknown
contexts remain unavailable with a reason. Native ordinary controls remain
unavailable. Earlier sections below are chronological evidence and their
"no product capability" statements describe those earlier builds.

The accepted model is the existing 4,866,413-byte FP32 strong-denoise
`realesr-general-x4v3` ONNX export, SHA-256
`1793a6e7fdf15a53eed213ba269ea768b5373858ae433d56ee8e1b0424377cc5`.
Runtime is ONNX Runtime Web **1.30.0**, GPU-buffer IO on the same Rust processing
device, CPU fallback disabled. 2x uses shared downscaling of x4 inference.
Sources must be opaque 8-bit SDR, normalized dimensions at most **577×579**;
limits are enforced again against the actual backend GPU before inference.
H.264/AAC and VP8 retain exact codec probes. AI HEVC is explicitly unvalidated
and rejected; FFmpeg WASM retains its existing fixed-FPS/storage/audio limits.
Burn and RTX cannot be selected. Model import and runtime acquisition remain
explicit, hash checked and lazy; AI Off does not load inference assets.

### Ordinary UI and recovery

Actual ordinary commands passed main/worker × light/dark, both scales, using a
64-frame 192×80 H.264/AAC source. The eight ordinary H.264 exports each processed
64 source frames and independently passed full FFmpeg A/V decode and FFprobe
geometry/grid/coverage checks. UI checks cover selected dimensions and bitrate,
CLAHE/manual edits, Before/After, Reset retaining AI, sampled Play/Pause, seek,
conversion, and Off restoring the retained 50% ordinary resize. Additional runs
cover missing-model errors, stopped playback clock, explicit reimport and retry
without automatic Play, and 640×360 sources leaving both scales unavailable
while Off preview works. All four contexts passed with no page errors.

This testing exposed a real `AlreadyBorrowed` Dioxus panic: signal read guards
survived an awaited preview while a color edit wrote the same signal. Preview
commands now freeze values before awaiting. The original panic is preserved;
subsequent ordinary UI runs pass. Earlier selector/option and final assertion
mistakes were harness failures, not evidence of a failed inference backend.
Neutral sampled AI playback also now reports its playback status correctly.

Finally, `dx build --web --locked --package converter-web` without extra feature
arguments passed. That normal build independently passed 2x/4x preview controls,
Off restoration, missing-model recovery and oversized-source checks in main/light.
The four-context/eight-export matrix used the same engine explicitly enabled
before promoting it into the application's normal dependency features. Test
servers only observe transport/session events; they do not replace capabilities
or ordinary backend commands. Explicit runtime requests were served the already
cached, hash-pinned files, so this is not a fresh CDN/network-availability claim.

### Default compiler, model and temporal checks

Fresh Edge 156 probes with **no experimental Dawn compiler argument** pass all
five caller-owned FP32 tensor shapes in both contexts, maximum absolute error
**6.16908073425293e-6**, below the unchanged 0.01 / 0.001 max/MAE gate. Exact
input/output device identity, one device, session release and post-release device
usability pass. The actual shared Rust tiled path additionally passes six shapes
through **577×579** in both contexts, maximum **1 RGBA8 LSB**, against the trusted
reference. Session retirement, stale generation, one-frame admission, cancellation,
fresh retry and outside-envelope rejection pass; GPU validation errors are null.
Readbacks in these numeric diagnostics are excluded from conversion timing.
Earlier Edge 155 compiler findings remain historical, not requirements for this
validated Edge 156 route.

Three licensed Tears of Steel excerpts now contain **64 consecutive frames each**
(192×80, 12 FPS, 5.333 seconds), covering faces, motion, texture, text, scene cuts
and a fade. Total compressed fixture bytes are 466,275; original soundtrack is
excluded and replaced with synthetic CC0 sine audio. Source is CC BY 3.0:
**Tears of Steel (2012), Blender Institute; directed by Ian Hubert, produced by
Ton Roosendaal**. The evidence manifest retains source/license/credit links,
changes, hashes and exact generation commands. Clips remain in ignored evaluation
storage; no full movie or frame sequence was added to source control.

All **192 frames** independently pass trusted CPU PyTorch versus native DirectML
FP32: maximum absolute error **1.5139579772949219e-5**, maximum per-frame MAE
**1.1423330192883441e-7**. All **13,440** profiled node events use DML, with CPU
fallback disabled. This is a matching-weight reference check, not the browser's
GPU-placement proof. DML setup was 0.533 seconds; per-clip DML 4.74–5.17 seconds,
PyTorch 11.96–15.87 seconds. These include explicit host staging on NVIDIA RTX
3050 Laptop GPU; they do not establish a fair browser/NVIDIA speed ranking.

All 64-frame overviews for both scales and detailed consecutive samples were
visually inspected against the same-size bilinear exports. AI sharpens edges
but smooths/reconstructs faces, hair, texture and small text. No gross new flashes
were apparent in those inspected images; source scene cuts/fades appear in both
paths. This is a limited visual assessment, not ground-truth fidelity improvement
or a flicker-free guarantee. Offline per-frame absolute differences are also
recorded; they are not motion compensated and have no perceptual acceptance
threshold. Contact sheets and encoded outputs remain under ignored
`tmp/ai-upscale/quality-temporal-inspection-20261025` and benchmark directories.

### Isolated performance and memory

Sequential main and worker runs compare the same three clips at 2x/4x, Original
FPS, H.264/AAC, CLAHE and 10/115/80 manual controls: **24 exports**, all independently
verified. The private diagnostic bilinear operation uses the same enlarged
output geometry and downstream effects/codecs without loading inference; it is
not exposed as an ordinary AI command. No other evaluation GPU workload ran
concurrently. This comparison explicitly used `--disable-dawn-features=use_dxc`;
default-compiler numeric/tile and ordinary UI checks above are separate. The first
benchmark overlapped UI work and was excluded from timing acceptance; its worker
was deliberately interrupted and its incomplete run remains recorded.

| Observed range | Main bilinear | Main AI | Worker bilinear | Worker AI |
| --- | ---: | ---: | ---: | ---: |
| Conversion seconds | 0.323–0.705 | 37.533–46.923 | 0.336–0.490 | 38.162–44.352 |
| AI setup ms | 0 | 69.9–551.2 | 0 | 69.3–579.8 |
| AI processing seconds | 0 | 37.434–46.819 | 0 | 38.068–44.254 |
| Encoder submission wait ms | 119.3–236.1 | 8.9–11.6 | 132.2–249.6 | 9.5–13.2 |
| Independent browser verification ms | 7.5–14.7 | 8.8–18.6 | 7.5–12.1 | 8.3–16.2 |
| Diagnostic wall seconds | 0.346–0.749 | 37.626–47.034 | 0.357–0.516 | 38.273–44.964 |

Setup includes cache/hash/runtime/session preparation, with a cold first and warm
later cases. AI processing includes packing/inference/unpacking/queue completion
and 2x downscale. Decoder waits were 105.6–159.0 ms bilinear, 4.3–14.0 ms AI;
CPU GPU submission/bridge telemetry was 25.4–44.8 ms. These counters overlap,
include scheduling and are **not additive or pure GPU/codec hardware time**.
Diagnostic wall time additionally includes export artifact collection and checks.
Inference dominates and is substantially slower than bilinear on this GPU;
real-time playback or faster conversion is not claimed.

Maximum application-observed requested resource window peak was **78,448,736
bytes** for AI (bilinear 37,116,656), with no unknown texture requests. The larger
tile diagnostic reaches 106,569,740 bytes including explicit diagnostic IO.
Admission remains bounded by 384 MiB. Counts exclude opaque runtime/compiler/
driver/surface memory and do not observe garbage collection. WebCodecs runs
report **zero explicit conversion pixel readbacks**, 64 ingress copies and 64
canvas captures per clip, with zero application-held frame/sample references and
zero live leases at cleanup. This does not prove zero browser-internal copies,
hardware codec execution or total physical memory usage. FFmpeg WASM's separate
explicit output readback path remains documented below.

### Exposed backend audit and remaining scope

| Candidate | Current exposure |
| --- | --- |
| Browser ONNX Runtime Web 1.30.0 | Ordinary 2x/4x enabled only for the context/source limits above; exact model/runtime hashes and existing 73-WebGPU-node feasibility evidence retained. |
| Browser Burn | Unavailable after actual kernel/workgroup validation failure. |
| Native ONNX Runtime DirectML 1.23.0 | Diagnostic numeric/video/preview passing scope; ordinary-window setup/preview/export acceptance pending, controls disabled. Native AI NVDEC/NVENC timing failure stays unavailable. |
| Native Burn | Unavailable after failed reference/import/runtime assessment recorded below. |
| Native RTX Video SDK | Unverified execution/toolchain/license gates; SDK presence is not a pass, controls unavailable. |

The exposed model retains its BSD-3-Clause manifest/source attribution, and
optional ORT Web runtime assets retain MIT, byte counts, URLs and hashes. Existing
native runtime and candidate license inventories below remain applicable; no
new dependency version, proprietary SDK binary, model/runtime binary, browser
helper service or cloud processing was added. The normal web app only opts into
the implemented engine; native capabilities are not inferred from web evidence.

Evidence is in [browser-enablement](evidence/ai-upscale-20261025/browser-enablement/):
ordinary context reports plus independent eight-export verification, recovery and
normal-build reports, default FP32/tile reports, 192-frame reference/fixture
manifest, temporal differences, isolated 24-case reports and performance summary,
selected light/dark 2x/4x screenshots and the preserved actual panic. Encoded
videos, model/runtime assets, large contact sheets and profiles remain ignored.

Verification: normal wasm Clippy for media-web/converter-web, diagnostic wasm
Clippy, host core/GPU checks, UI tests (2), JS clock/history/transport tests (16),
formatting, changed JS syntax checks, strict OpenSpec validation and whitespace
checks. No new native implementation is claimed from this browser slice.
**OpenSpec remains 20/32**, because the outstanding tasks include native/RTX and
full cross-platform acceptance. Browser enablement does not mark those complete.

## Active browser preview invalidation — 2026-10-25

Task 6.4 gains explicit source/seek invalidation and actual sampled AI preview
failure/replacement checks. **Progress remains 20/32:** ordinary AI On UI/playback
coalescing and the full release acceptance matrix are still open.

The browser transport now carries a separate preview revision. Explicit source,
seek and changed AI identity retire active/queued obsolete preview commands;
profile probes and conversions remain current. Worker invalidation addresses
only its active preview job; main mode requests cancellation from its owning
Rust instance. Automatic playback samples and downstream edits do not invalidate
active inference. The UI keeps its clock running through a seek, ignores an
obsolete revision's error, and submits the latest desired position. Processing
also checks generation immediately before surface presentation/capture, after
asynchronous ingress/inference. Feature-only ownership metadata now reports
successful canvas captures for diagnostic assertions. Ordinary Off still loads
no inference runtime or model.

Edge main/worker diagnostics pass both scales with actual decoded CC0 cut and
VFR sources, selected-output CLAHE/manual processing and sampled-playback command
keys. Each scale exercises cancellation during actual ORT compute dispatch,
the real runtime's missing-input error, and device.destroy during inference
submission, followed by explicit retry and a different-source replacement/restore.
Across contexts there are **12 failed requests and 40 successful preview outputs**.
Failed selected requests return no output and do not increment successful canvas
captures. Eight separate before/after canvas RGBA8 comparisons prove cancellation
and runtime failure preserve the preceding displayed pixels. Destroyed-device
display contents are deliberately not asserted. Fresh-generation retry restores
the preceding selected pixels exactly; replacement source pixels differ and
switching back restores the original exactly. Each context creates three
successive devices over its two destruction cases; healthy cancellation/runtime
failure retains its processing generation. Fresh scopes have no validation error.
Failures retire caller tensors, tracked AI resources, sessions, leases and pending
submissions. These are application ownership claims, not zero driver memory.

The built ordinary AI Off app passes seven lifecycle cases per context, adding
a four-seek burst while playing to the prior Pause/Play/error/replacement/handoff
matrix. Playback remains active and reaches the latest requested source position
without publishing a cancellation error. Its two 60-frame exports pass independent
FFmpeg A/V decode/FFprobe checks; optional asset caches remain empty. Three new
transport tests separately prove active/queued preview invalidation, preserved
queued profile work, fresh preview admission and isolation from active conversion
in both execution modes. The combined clock/history/transport suite passes 16
tests. The previous actual AI implicit-handoff/failure/retry suite also passes
after these changes, including eight independently verified full A/V exports.

Environment and unchanged runtime/model pins match the preceding Oct 25 evidence:
Windows, Edge 155.0.4283.62, Intel gen-12lp WebGPU (driver unavailable), ORT Web
1.30.0 and Rust 1.98.1 GNU. Release diagnostic WASM, ordinary app build, relevant
wasm Clippy with/without diagnostic feature, host core/GPU checks, formatting,
JS syntax, strict OpenSpec and whitespace checks pass. No downloads/installations
were needed. Raw snapshots are separate CPU readbacks excluded from throughput.
The sampled-playback command flag is not ordinary AI On clock/UI acceptance,
synchronized playback or a performance result. Reproduce with the probe guide;
results and scope are indexed in
[the invalidation check inventory](evidence/ai-upscale-20261025/browser-preview-invalidation-checks.json).

## Browser preview ownership and conversion retirement — 2026-10-25

Task 6.4 gains browser command-boundary and ordinary AI Off UI evidence.
**Progress remains 20/32.** Ordinary AI On playback/UI, the full lifecycle
matrix and quality/performance release acceptance remain open. No candidate
or product scale is enabled by these diagnostics.

An old playback producer could continue after Pause/Play, publish a late error
or pause the resumed clock. The app now checks the playback token before/after
each awaited request and only the current producer may stop playback. Conversion
and the M1 probe invalidate that token and save the paused clock position. The
producer records the settings revision it actually submitted, so edits made
during processing remain pending. Audio play attempts also have identities:
an interrupted promise rejection cannot pause or set an error on a newer
same-source resume. The clock regression covers replacement, resume and Pause.

The built ordinary app passes six cases in each of Edge main/worker execution:
rapid Pause/Play with an outstanding successful response; a retired response
error after resume; a current response error that stops the clock; explicit
Play recovery; source replacement with a late old error; and conversion handoff
with a late old error followed by paused preview restoration. The test server
holds a response **after actual successful decode/GPU processing**, optionally
rejecting delivery. This tests producer scheduling and error ownership, not a
real codec failure or proof that an already rendered old frame was prevented.
Both 640x360, 60-frame VP8/Opus outputs pass full browser verification and
independent FFmpeg A/V decode/FFprobe frame counts. Optional asset caches remain
empty; page errors are zero. The existing ordinary Off FFmpeg-rejected-export,
CLAHE, Before/After and Play/Pause regression also passes both contexts.

Preview retirement previously ignored GPU-drain/session-close errors. It now
attempts both cleanup steps, reports failures before conversion startup and
destroys/removes the failed processing context. Session-close failure also
retires its owning context on other paths. Preview failure keeps the original
error and any cleanup error. A real AI command-boundary probe runs both scales
with finite CLAHE history/manual effects, directly from preview into conversion
without an explicit clear command. After successful conversion and after a
failed cleanup/fresh-context retry, selected preview RGBA8 equals its preceding
reference exactly. Two successful exports per scale/context (**eight total**)
pass independent complete A/V decode, frame grid, geometry, audio and packet
endpoint checks. The cleanup fault is deliberately injected **after real ORT
release**; it proves failure propagation/retirement without claiming a genuine
broken ORT session releases all internal resources. Failed conversion returns
no artifact and invokes no conversion progress callbacks. Caller tensors,
tracked AI resources, leases and pending submissions are zero after retirement;
all instrumented sessions have released. Each retry creates exactly one fresh
device generation (three devices total per context over the two fault cases),
with no concurrent inference-device fallback. These are diagnostic pixel
readbacks, excluded from performance evidence; opaque driver/runtime memory is
not asserted zero.

The existing active cancellation/runtime-error/injected setup/run OOM/actual
device.destroy suite initially exposed an extra-retry regression: the failed
conversion had retained its lost context, so its first retry failed at the new
retirement check. Both initial failure reports are retained. Conversion now
retires an actually lost context before returning its failure. All five cases
per context pass with one explicit retry; their **ten retry exports** pass the
independent verifier. Physical OOM and spontaneous driver loss remain untested.

Environment: Windows, Edge 155.0.4283.62, browser-reported Intel gen-12lp WebGPU
(driver unavailable), Rust 1.98.1 GNU, ORT Web 1.30.0 and the unchanged hash-pinned
compact model. No downloads/installations or native changes were required.
Relevant wasm Clippy with/without the diagnostic feature, host core/GPU checks,
13 clock/history/transport tests, formatting, JS syntax, strict OpenSpec and
whitespace checks pass. Reproduction commands and exact scope are in
[the probe guide](../tools/ai-upscale/README.md); accepted results, preserved
initial failures and check inventory are under
[2026-10-25 evidence](evidence/ai-upscale-20261025/browser-lifecycle-checks.json).
Encoded files/raw diagnostic rasters remain in ignored evaluation storage.

## Native preview/conversion retirement and presentation coverage — 2026-10-24

Task 6.4 gains a real native handoff and failure/retry regression. **Progress
remains 20/32:** ordinary AI On, browser handoff/lifecycle coverage and the
remaining release gates are still open. This section supersedes the earlier
worker-only handoff limitation for the native service/ordinary AI Off handler,
not the ordinary AI On acceptance requirement.

The ordinary native conversion handler now reserves preview admission before
spawning conversion. It pauses/cancels playback, removes pending seeks and
visible source pixels, suspends every registered renderer worker, invalidates
its tickets and retires AI/history caches. The conversion thread waits for
playback and active preparation/cache-retirement acknowledgement with a checked
30-second deadline and cancellation; no retirement wait runs in a paint callback
or UI event. Suspended workers reject stale paint/playback requests even after
invalidation, closing a requeue race. Conversion starts only after successful
retirement. A scoped reservation releases admission on success/error/cancellation;
the app reopens a still-visible preview paused at its saved position. Closing
the preview prevents reopening. No frame/resource enters reactive state, and no
processing texture crosses the wgpu-30/renderer-wgpu-26 boundary. Final captions
say Paused / Preview paused during conversion instead of implying a new decode.
Renderer registry/driver pools are not claimed as zero total GPU memory.

An actual configured DirectML/renderer test passes both 2x and 4x on the CC0 cut
fixture. It first prepares a real enhanced preview, observes an active subsequent
preparation token, reserves the service and verifies retirement and rejection of
stale requests/new source/policy commands. This observes active preparation, not
proof of an executing GPU dispatch. A pre-cancelled conversion publishes no file;
a fresh reservation then performs a four-frame AI/CLAHE conversion with complete
native verification. After release, fresh paused-preview display pixels equal
the preceding reference exactly. Concurrent renderer close during a reservation
also joins and acknowledges retirement after dropping caches. A dev-only media-native feature exposes this
existing diagnostic conversion entry point to the test; ordinary app commands
and capabilities remain gated. The existing renderer AI cache/coalescing/helper-
error/device-destruction/fresh-generation test also passes after this change.

The new test initially failed conversion verification: expected 266667 us,
reported 200000 us. A separate CLI reproduction confirmed the failure and that
neither final output nor partial file survived. An explicitly unverified snapshot
captured under ignored storage showed output packet PTS+duration endpoints cover
266667 us, while its MP4 decode-duration header describes only 200000 us. Input
AI planning already used exact packet coverage, but output verification still
used the header. Both sides now use the same bounded packet scan and checked
time-base arithmetic for AI jobs. No timestamp tolerance, frame equality or A/V
check is relaxed. Error text includes expected/actual duration. The initial
failure record is retained; the snapshot is not accepted export evidence.

The fixed **release** native CLI passes cut and VFR fixtures × 2x/4x ×
Original/15/60 (**12 exports**). The independent verifier now additionally checks
the final packet endpoint, preventing an actually shortened presentation tail
from passing merely because frame PTS match. All twelve pass full independent
FFmpeg A/V decode, grid/geometry/audio and endpoint checks. The prior real-face
browser main/worker matrix also passes the stronger endpoint verifier (12 cases).
These are explicit diagnostic runs; native AI NVDEC/NVENC remains unavailable.

An owned ordinary 960x760 window, with no diagnostic flag and AI Off, successfully
converts m2-h264-aac.mp4 through Shared GPU/software codecs on the UI-selected
NVIDIA DX12 adapter: **60 frames, 640x360, 254068 bytes**, independently re-decoded.
During handoff its old player image is hidden and input/Play/seek admission is
blocked; it reopens paused after success. A later attempt while playback visibly
advances correctly refuses to overwrite the verified output and reopens paused
at a nonzero position. The output hash is unchanged and a subsequent player-region
comparison changes zero pixels. Stderr is empty, the owned window closes normally
and no converter/helper/FFmpeg processes remain. The actual window renderer was
not newly identified. Screenshots precede the final caption-only improvement;
that text change is build/test checked, without claiming another window run.

A separate ordinary AI Off attempt on ai-upscale-cut.mp4 rejects `invalid frame
duration`: its unchanged legacy header-based source policy ends before the final
source PTS. This is recorded as an Off/shared-GPU fixture limitation, not a
successful conversion or a new AI failure. The source hash remains intact and
no output is published. The passing ordinary-window case uses the supported CFR
fixture; AI cut conversion passes with the exact-coverage policy above. Broader
Off duration-policy changes were not silently included in this AI slice.

Checks: core 38 pass; native library 46 pass/9 ignored; shared UI 2 pass; native
app **17 pass/6 ignored**, including the new real decoded-playback reservation
regression. The two named AI handoff/renderer tests were additionally run with
`--ignored --nocapture`; other ignored tests are not passes. Locked native
all-target Clippy (-D warnings), host core/GPU checks, wasm web/app all-target
Clippy, formatting, JS syntax and normal native build pass. The existing native
release link `.drectve` warning remains recorded; actual release execution passes.
No dependency version, model, SDK/toolchain installation or download changed.

Evidence: [AI service/conversion handoff](evidence/ai-upscale-20261024/native-conversion-handoff.json),
[initial failure](evidence/ai-upscale-20261024/native-cut-duration-initial-failure.json),
[release cut matrix](evidence/ai-upscale-20261024/native-presentation-cut-matrix.json),
[independent cut endpoint checks](evidence/ai-upscale-20261024/native-presentation-cut-independent.json),
[VFR endpoint checks](evidence/ai-upscale-20261024/native-presentation-vfr-independent.json),
[renderer lifecycle regression](evidence/ai-upscale-20261024/native-handoff-renderer-regression.json),
[ordinary window scope/results](evidence/ai-upscale-20261024/native-handoff-window.json),
[handoff window](evidence/ai-upscale-20261024/native-handoff-window-converting.png),
[successful paused return](evidence/ai-upscale-20261024/native-handoff-window-complete.png),
[error paused return](evidence/ai-upscale-20261024/native-handoff-window-error-paused.png),
[Off cut rejection](evidence/ai-upscale-20261024/native-off-cut-duration-rejection.png).

## Required regression checks — 2026-10-24

Task **8.1 is complete; progress is 20/32**. This is regression/build acceptance,
not permission to enable ordinary AI On. Commands ran from the repository with
`CARGO_HOME=D:/Libraries/Cargo/CARGO_HOME` and
`CARGO_TARGET_DIR=D:/Libraries/Cargo/CARGO_TARGET_DIR`, Rust 1.98.1 GNU:

```text
cargo test --locked -p media-core -p media-native -p converter-native -p ui
cargo check --locked -p media-core -p media-gpu
cargo clippy --locked -p media-core -p media-gpu -p media-native -p converter-native -p ui --all-targets -- -D warnings
cargo clippy --locked -p converter-web -p media-web --target wasm32-unknown-unknown --all-targets -- -D warnings
cargo test --locked -p media-gpu -- --ignored --nocapture
cargo test --locked -p media-native --lib injected_device_loss_cleans_job_and_allows_fresh_gpu_job -- --ignored --nocapture
cargo fmt --all --check
node --test tests/worker-transport.test.mjs
git -c core.safecrlf=false diff --check
openspec validate ai-upscale --strict
```

Results: core **38 pass**; native library **46 pass/9 ignored**; native app **16
pass/5 ignored**; shared UI **2 pass**; worker transport **9 pass**. Native tests
include an ordinary direct-route export with stage/inspection/timing checks.
The explicit native AI Off device-loss/fresh shared-GPU export test additionally
passes full output verification. All **three actual GPU diagnostics pass**:
AI pack/crop/quantize/generation on NVIDIA Vulkan, and CLAHE/reference/history/
manual composition on NVIDIA Vulkan and Intel DX12. NVIDIA reports driver
621.01; Intel DX12 reports 31.0.101.5333. Explicit test readbacks are diagnostic.
Other native ignored tests are not counted as passed in this run; earlier AI
lifecycle/preview evidence remains separately recorded.

Host checks, both all-target Clippy commands, formatting, strict OpenSpec and
diff checks pass. `node --check` passes all six top-level tools/ai-upscale MJS
runners plus apps/web/assets/m2.js, both media-web AI glue modules and the four
AI setup/asset/preview UI runners (13 files). `python -m py_compile` passes the
three changed quality/native matrix scripts. Locked release native and browser
WASM builds pass as recorded in the next section. Native test/release linking
emits the recorded `.drectve` warning; the checks are not described as warning-free.

The next section records **18 independently decoded browser AI Off release
exports**, with neutral settings, Original/15/60 and no AI provisioning/session.
Legacy deserialization/geometry/snapshot tests and actual Off startup/setup/UI
preview tests pass their recorded scope. No new Off inference allocation,
automatic model/runtime acquisition or media upload is introduced. Ordinary
native/browser AI On, broader temporal quality and missing RTX prerequisites
remain separate open gates. No Safari, other OS or spontaneous driver-loss
coverage is claimed. D: has about 17.6 GiB free after combined checks; the small
new media inputs total 61 KB, while build artifacts remain in the external Cargo
cache for subsequent work.

## Licensed short-clip release matrix — 2026-10-24

Partial task 8.2 now has real-content release evidence. **This is not product
acceptance:** ordinary AI On remains unavailable, and full preview/lifecycle,
component performance, longer temporal coverage and other open tasks remain.
No Burn or RTX result changes.

`quality_fixtures.py` explicitly reads HTTP ranges of the official Tears of Steel
720p movie and retains only three 192x80, 12-FPS, eight-frame excerpts starting
at 45/60/120 seconds: text/architecture, face/organic texture, and cloth/rope
motion. The inputs total **61,422 bytes**, under ignored tmp storage. The complete
movie is not stored. Attribution: **Tears of Steel (2012), Blender Institute;
directed by Ian Hubert, produced by Ton Roosendaal**, licensed CC BY 3.0 as stated
on [the project page](https://mango.blender.org/about/). The manifest records
source/license/credit links, excerpt times, hashes, FFmpeg recipe/version and
changes. Original audio is excluded; an original CC0 sine supplies A/V tests.
The separately released soundtrack has
[its own restriction](https://mango.blender.org/production/soundtrack-files-released/)
and is not reused. No film excerpt or film screenshot is added to Git.

Locked release builds of native-convert (`ai-upscale-pilot`) and the real shared
Rust browser WASM probe run the same acquired clips. Each clip tests 2x/4x ×
Original/15/60, CLAHE 50 and brightness/contrast/saturation 10/115/80. **18 native
and 36 browser main/worker AI exports pass independent complete FFmpeg decode,
exact rational frame grids, dimensions and audible AAC checks.** Original/15/60
produce 8/10/40 frames; inference still advances exactly eight source frames.
Native uses DirectML 1.23.0 on explicitly checked NVIDIA RTX 3050 Laptop DXGI 1,
with matching NVIDIA Vulkan wgpu processing and software codecs. Browser Edge
155.0.4283.62 reports Intel gen-12lp WebGPU, one processing device per context,
no validation error, zero final held frame references/samples/leases, and no
additional production pixel readback. Explicit diagnostic ingress/pre-encoder
readbacks are enabled in these AI runs. Browser hardware codec execution is
unknown. These different adapters cannot establish a same-GPU speed comparison.

A separate **18 browser AI Off release exports** run all three clips at
Original/15/60, neutral color/CLAHE, without model/runtime provisioning or an AI
session. Independent full decode/timeline/audio checks pass. Existing actual
ordinary UI tests establish empty optional caches on Off startup/preview; the
standalone matrix does not replace those UI checks.

The first independent native oracle incorrectly used an AAC priming packet at
-1024/48000 s as the audible origin and failed all three first cases. Those
failure JSONs are retained. FFprobe side data says skip 1024 samples; independent
FFmpeg decoding starts its first audio frame at zero, matching native stream
presentation policy. The oracle now uses native stream presentation origin and
retains the independently measured browser packet-origin policy. All 18 native
cases and the older six-case VFR native matrix pass the corrected oracle. This
does not relax timestamp tolerance or change native output. Browser preserves
1024 priming samples: its video starts at 21.337 ms and strong decoded sine onset
is sample 1025; native/source video starts at zero and sine onset is sample 1.
That paired shift preserves the measured initial A/V relationship. Browser
track durations include its priming/padding and are not reported identical to
native track durations. The diagnostic onset samples are retained.

`quality_reference.py` checks the exact hashed model and source clips, enumerates
DXGI and verifies the explicitly named NVIDIA adapter at ordinal 1. All **24
real-content frames** pass independent CPU PyTorch versus untiled DirectML FP32:
maximum absolute error **0.0000179783**, maximum frame MAE **0.0000000876183**,
below unchanged 0.01/0.001 limits. Profiling records **1,680 DML node events, zero
CPU node events**; CPU provider registration alone is not execution. This adds
real-content matching-weight evidence, not browser CLAHE pixel equivalence.
Previously recorded tiled/edge and preview/export reference gates remain separate.

Neutral first-frame x4 outputs were visually compared with an independently
computed encoded-RGB bilinear baseline under ignored storage. Four chronological
native export frames per excerpt were also inspected as contact sheets. The
model produces sharper contours but visibly deforms small lettering and smooths
facial/fine texture details; it does not recover reliably readable text. These
are observations on deliberately low-resolution excerpts, not a perceptual score
or general improvement claim. Eight-frame samples/contact sheets do not prove
absence of flicker, hallucination or longer-run temporal problems. Uncompensated
frame-difference numbers include actual motion and are not labeled flicker scores.

Measured native total job time is **2.791–3.504 s** per eight-frame clip, including
load, processing, encode and independent native verification. Browser diagnostic
wall time is **4.466–5.508 s**, including explicit test readbacks and therefore
excluded from throughput comparisons. Native admitted bytes are 154,662,224 at
2x / 160,740,688 at 4x; browser 158,533,264 / 162,891,408. Requested browser resource
peaks are 39,610,512–57,440,448 bytes across these windows, including diagnostic
resources/runtime pools and excluding opaque driver/GC memory. Each native job
reports 1,996,800 input / 31,948,800 output tensor staging bytes, 7,864,320 x4
processing upload bytes and 1,966,080 (2x) / 7,864,320 (4x) output readback
bytes. Sampled helper local memory is not a total-VRAM ceiling.

The offline reference additionally records model load and CPU/DirectML/bilinear
host timings, explicitly separate from product GPU throughput. Native per-stage
load/inference/transfer/encode timing, shared-GPU bilinear comparison and
end-to-end preview/lifecycle coverage remain unfinished. Release native linking
warned `corrupt .drectve at end of def file`; build and all actual executions
succeeded. The warning also appears on the native test link and is not hidden.
No new dependency, SDK install, model download or license acceptance occurred;
only the tiny licensed excerpts were acquired. D: retained about 19.85 GiB free
after the matrix, before the final combined checks.

Evidence: [release summary/build hashes](evidence/ai-upscale-20261024/quality-summary.json),
[fixture provenance](evidence/ai-upscale-20261024/quality-fixtures.json),
[matching-model reference](evidence/ai-upscale-20261024/quality-reference.json),
[native face independent decode](evidence/ai-upscale-20261024/quality-tos-face-native-independent.json),
[browser face independent decode](evidence/ai-upscale-20261024/quality-tos-face-browser-independent.json),
[AI Off independent decode](evidence/ai-upscale-20261024/quality-off-independent.json),
[audio onset](evidence/ai-upscale-20261024/quality-audio-onset.json).
All other per-clip main/worker/native records and initial oracle failures are in
the same evidence folder. See the runner guide for reproducible commands.

## Gated selection snapshots and AI Off UI regression — recorded 2026-10-24

Tasks 7.1/7.2 have additional settings wiring; **progress remains 19/32**.
Ordinary 2x/4x remain disabled in both apps. Shared and parent handlers require
an exact validated capability, with none granted by default or by acquisition.
The shared selector now has real Off/2x/4x callbacks. A Dioxus VirtualDOM event
regression dispatches actual click handlers for browser/native panel variants:
unavailable choices, mismatched scale capability and busy changes reject; exact
**test-only** validated metadata permits selection, Off restores it, and a copied
job snapshot retains its earlier choice. This is handler evidence, not backend
or release acceptance.

Both apps freeze the selection into conversion requests, use enlarged geometry
for estimates/profile checks, and retain ordinary resize fields. Inactive resize
text is not parsed as an AI target; Off retains the existing no-upscale guards.
Before remains separate preview metadata, preserving saved AI and color state.
Reset color does not touch AI. Browser Pause executes before readiness guards;
an unsupported AI profile cannot silently switch to a replacement. Ordinary
native preview still explicitly rejects unaccepted On, avoiding a neutral
substitute. No inference resources were added to reactive state.

Verification ran on 2026-10-23, with records collected on 2026-10-24. Locked UI
unit tests pass (2), native app tests pass (16; 5 ignored), worker transport tests
pass (9); native all-target and wasm web Clippy with -D warnings, formatting,
web/native builds and diff checks pass. Ignored GPU tests were not rerun for this
slice. The UI test adds only the pinned, already cached Dioxus HTML serialization
dev dependency; the lockfile adds its UI dependency edge, without package upgrades.

Actual built Dioxus/Edge 155.0.4283.62 tests pass main/default-worker contexts:
both disabled scales resist a forged DOM click; Off retains 75% ordinary resize;
explicit import/runtime acquisition/cancel/corrupt recovery/remove still pass.
Runtime CDN responses are served from exact pinned local bytes, so these tests
do not establish CDN reachability. A second actual decoded-preview test passes
CLAHE, Before/After and Play/Pause while FFmpeg Original-VFR export is rejected,
then enables fixed-FPS export and returns to Original without losing preview.
AI Off optional caches stay empty and no page errors occur. Flags are
`--enable-unsafe-webgpu --use-angle=d3d11`; playback additionally uses
`--autoplay-policy=no-user-gesture-required`. The worker nonblack-frame check
uses a test-only VideoFrame readback, excluded from production/throughput claims.
These are AI Off regressions, not AI playback acceptance.

An owned ordinary native 960x760 window retains 75% after opening setup,
clicking unavailable scales and returning through Off/Done. Dark setup and light
collapsed layouts were inspected. Stderr is empty; no helper was started, and
the owned window closed normally. This does not prove ordinary AI inference.

Evidence: [browser selection/setup](evidence/ai-upscale-20261024/selection-browser.json),
[AI Off preview](evidence/ai-upscale-20261024/selection-preview-off.json),
[native scope](evidence/ai-upscale-20261024/selection-native.json),
[dark setup](evidence/ai-upscale-20261024/selection-native-dark.png),
[retained resize](evidence/ai-upscale-20261024/selection-native-retained.png),
[light layout](evidence/ai-upscale-20261024/selection-native-light.png).

## Native AI comparison, display framing and window diagnostics — 2026-10-23

Partial tasks 6.2/6.4 now include native Before/After and explicit real-window
evidence. **Progress remains 19/32.** Product 2x/4x controls and ordinary On
commands remain unavailable. `converter-native --ai-preview-check` is an explicit
diagnostic launch with only the checked-in CC0 cut fixture and required local
helper/runtime/cache/device/scale configuration. Both its export button and
command handler reject conversion. It is not ordinary-launch AI acceptance.

The unchanged hash-pinned compact Real-ESRGAN ONNX model executes through the
existing DirectML 1.23.0 helper on explicitly selected NVIDIA DXGI 1. Native
wgpu-30 processing uses the matching RTX 3050 Laptop GPU/Vulkan. The wgpu-26
worker probe uses Intel UHD/Vulkan; that probe identity is not a new measurement
of the actual window's renderer adapter. Only a bounded RGBA8 display image
crosses the service/renderer boundary, with no foreign GPU handles or ABI casts.

The selected original now has one retained texture on the processing device.
Warm Before renders it directly at the same display framing, bypassing AI and
color while preserving the enhanced result and finite CLAHE statistics. Cold
Before needs no model/helper: even a missing helper can display the original,
with zero inference transfers. Original upload, processing upload, display
readback and renderer upload are counted separately. Native readback now unmaps
on interrupted/error exits too. These are explicit native staging costs; raw
test rasters/readbacks remain separate diagnostics, not throughput evidence.

Actual chronological service and renderer tests pass **55 independent NumPy
comparisons, maximum 1 LSB**, against the unchanged 2-LSB gate. They cover both
scales, neutral/manual settings, CLAHE Off/50/80, finite history/cut/gap, bounded
display scaling, warm Before/After and cold Before with a nonexistent helper.
Returning After reproduces the preceding image exactly; inference counters stay
unchanged on comparison and downstream edits. Source generation/geometry checks,
opaque alpha and cancellation remain enforced.

The worker test observes a real active preparation token while submitting 100
playback samples. It does not cancel that active preparation; positions 0 and
200000 us complete, demonstrating progress and latest-request completion.
Explicit seek/source invalidation, cache-retirement acknowledgement with no old
result, actual missing-helper failure, explicit valid-policy retry, renderer
device destruction and healthy generation-43 retry pass. Feeding the real worker
error to the preview service pauses its clock; Pause does not restart it. This
does not establish sustained ordinary-window error-state Pause or real-time A/V
sync. Ordinary conversion handoff and the full browser/native matrix remain open.

Owned 960x760 native windows visibly display both scales in the diagnostic.
Before changes 57,279 of the observed 320x180 pixels at 2x; After restoration
changes **zero pixels at both scales**. Strength and Reset color leave inference
counters unchanged. A 4x seek across the 100000-us cut completes with three
prepared frames. Play then Pause advances/stops the clock, and the observed
player region remains identical after another 500 ms. Missing-helper errors are
visible, while Before still displays the original without inference. Dark/light
inspection passes the collapsed layout; expanded color settings can extend the
page, so no stronger no-scroll claim is made. All owned windows close normally;
no converter/helper processes remain after this run.

An additional final ordinary launch, with all optional local asset/device
environment variables populated but no diagnostic flag, stays Off with empty
source, collapsed setup, zero helper processes and empty stderr. Its 960x760
layout fits without page scroll; the window closes normally.

Initial window inspection caught small AI results centered at native pixel size
inside the fixed player. Final display fitting now fills the same framing after
selected-size processing; it never enlarges model input. The initial screenshot
is preserved. A new lifecycle test initially retained a three-frame retry
assertion after moving its seek to zero; setting the explicit retry back to
100000 us corrected that test bookkeeping. This was not a model/runtime failure.
The initial missing-helper OS error also lacked component context; helper startup
now identifies the ONNX helper and instructs checking its executable path.

Tests: core 38 pass; native library 46 pass/9 ignored; native app 16 pass/5
ignored. The service/renderer ignored AI tests were run explicitly above; ignored
tests are not counted as passes. The native AI Off device-loss test also passes
fresh shared-GPU conversion and independent verification; the regular library
suite includes a 36-frame direct-route export. Host core/GPU checks, native
all-target Clippy, wasm web Clippy, formatting and build pass. Host GPU's three
other diagnostic tests remain ignored in this run. Exact diagnostic commands
are in [the runner documentation](../tools/ai-upscale/README.md).

Evidence: [pixel results](evidence/ai-upscale-20261023/native-before-pixels.json),
[service metadata](evidence/ai-upscale-20261023/native-before-service.json),
[worker lifecycle](evidence/ai-upscale-20261023/native-before-renderer.json),
[window scope/results](evidence/ai-upscale-20261023/native-preview-window.json),
[2x After](evidence/ai-upscale-20261023/native-preview-2x-after.png),
[2x Before](evidence/ai-upscale-20261023/native-preview-2x-before.png),
[4x cut seek](evidence/ai-upscale-20261023/native-preview-4x-seek.png),
[4x light/paused](evidence/ai-upscale-20261023/native-preview-4x-light-paused.png),
[cold Before](evidence/ai-upscale-20261023/native-preview-missing-helper-before.png).
No model/runtime/raw sequence was added to Git. Representative temporal quality,
release performance, normal On controls and complete lifecycle acceptance remain
pending; native Burn and RTX assessments have not changed.

## Guarded AI setup controls — 2026-10-23

Partial task 7.1 now exposes a collapsed **AI Upscaling** setup section in the
ordinary browser and native apps. Off remains selected; 2x/4x are disabled with
the pending preview/playback/quality reason. There is no On UI callback or
capability grant. Burn stays unavailable and RTX appears only as an unavailable
native assessment. Ordinary processing commands still reject On. Neither 7.1,
7.2 nor 7.3 is checked from this setup-only slice; progress remains 19/32.

The browser UI imports the exact local model, rechecks/removes its cache and
explicitly acquires/removes pinned ONNX Runtime Web 1.30.0 assets. The shared
view identifies model bytes, BSD-3-Clause, source/license links and runtime
bytes/license/source before actions. Cache IO stays behind media-web; its new
DOM helper reads the model file input only on Import. No startup acquisition or
video upload was added. Actual built Dioxus/Edge 155.0.4283.62 UI tests pass in
main/default-worker page modes: collapsed Off startup with empty caches/no
optional-runtime requests; missing/truncated/full-size corrupt model rejection;
exact import/check; preservation of a good cache after bad input; cancellation
of an active delayed runtime fetch leaving no partial assets; successful explicit
retry; and model/runtime removal. CDN responses are mirrors of the exact existing
pinned bytes, not new external downloads or CDN reachability evidence. All
observed requests are GET. These UI setup actions execute on the DOM context;
this does not add inference/worker execution evidence. UTF-8 text and 375-pixel
layout pass; no horizontal overflow. Removing a disabled attribute and clicking
cannot enable AI. Browser never offers RTX.

Native ordinary-window testing uses the normal binary, Windows common file
pickers and owned-window snapshots. At 960x760, collapsed layout and source
preview fit without page scroll. Setup fits both supported themes. It uses a
dedicated retained-state view because actual Blitz nested fixed-overlay layout
failed clipping/stacking. Opening pauses preview/relinquishes player keys; Done
returns to the existing source/settings. The UI-selected cache receives only the
exact 4,866,413-byte graph; an independent SHA-256 matches the pinned export.
Stored-model check succeeds, truncated import rejects while that cache remains
valid, and explicit Remove deletes the test copy. Read-only runtime-folder
inspection reports assessed PE components as present-unvalidated and loads no
runtime; the tested process module inventory contains no onnxruntime/DirectML/
nvngx module. No native runtime/helper/SDK installation or automatic terms
acceptance occurs. Native live-import cancellation, ACL/disk-full UI variants and
actual AI preview/playback/conversion controls remain pending.

An ordinary paused CC0 source preview survives entering/leaving setup with
**zero changed pixels across the observed 320x180 display region** and remains
paused. This checks the existing AI Off preview. Native model/runtime/cache paths
are setup metadata, not Dioxus frame state; IO uses one explicit action worker
with a cancellation token. Initial screenshot review also caught text corrupted
by a Windows-default encoding edit; affected files were repaired as UTF-8 and a
real rendered-text assertion now guards the browser view.

Evidence: [browser UI cases/requests](evidence/ai-upscale-20261023/ai-setup-ui-browser.json),
[native UI cases/checks](evidence/ai-upscale-20261023/ai-setup-ui-native.json),
[browser screenshot](evidence/ai-upscale-20261023/ai-setup-browser.png),
[native dark](evidence/ai-upscale-20261023/ai-setup-native-dark.png),
[native light](evidence/ai-upscale-20261023/ai-setup-native-light.png),
[restored ordinary preview](evidence/ai-upscale-20261023/ai-setup-native-source-restored.png).
Initial failed overlay and intermediate import/error/inspection snapshots are
retained alongside them. Tests: native app 16 pass/5 ignored; shared UI test
passes; worker transport 9 pass; locked wasm web/native all-target Clippy,
formatting, JS syntax, build and strict OpenSpec checks pass. No ignored GPU test
is counted as newly passed. Reproduce the browser setup UI test with
`node tests/ai-setup-ui.mjs <evaluation-root> <built-public-directory> <fresh-output>`;
native steps use the ordinary UI and existing local assets. Full AI acceptance
and enabling 2x/4x remain separate unfinished work.

## Native AI staged preview adapter — 2026-10-23

Task 6.2 now has a concrete native service and renderer-worker adapter, but
**remains unchecked** because ordinary-window/UI acceptance is still pending.
`media-native::ai_preview` uses the unchanged compact ONNX model and explicit
DirectML 1.23.0 helper on NVIDIA DXGI 1. Shared wgpu 30 Vulkan processing uses
the same NVIDIA RTX 3050 Laptop GPU (driver 621.01); the actual wgpu 26 renderer
probe selects Intel UHD Graphics/Vulkan. That difference is intentional and
proven by counted CPU display-image staging. No texture/device handle crosses
the ABI or physical-device boundary, and no Blitz callback is replaced by the
worker or the native service.

The service reads cancellable <=64-KiB metadata, not a whole-file conversion
scan, then streams the existing exact 100-ms preroll through inference and
selected-size CLAHE. It closes/reaps the helper after preparation. One selected
x4 result and finite statistics are retained for downstream slider/Strength
edits; no further inference occurs during those edits. Both 2x and 4x keep the
conversion order and RGBA8 quantization point. The service applies display
scaling only after selected-size effects, reading back <=640x360 RGBA8. The
renderer worker uploads that image into a renderer-owned texture; Vello's atlas
copy remains an additional previously documented renderer operation.

Actual service tests cover 2x/4x, CLAHE Off/50/80, neutral/manual color, the cut
at 100 ms with two predecessors, a zero-predecessor target after the gap, and
192x96 -> 768x384 -> 640x320 display limiting. Full chronological FFmpeg decoding
and the actual native pre-encoder processor provide reference pixels, independent
of preview seeking/preroll. Independent NumPy display interpolation checks
**18 service + 9 renderer comparisons, all <=1 LSB** against the unchanged 2-LSB
gate. Source PTS/relative time, predecessor count, opaque alpha, generation
mismatch rejection and selected-result inference reuse are checked.

Transfer metadata separates inference FP32 staging, repeated cached-x4 uploads
to the processing device, all bounded display readbacks, and the renderer upload
per published image. Preroll currently also reads back and discards one display
image per predecessor: explicit overhead, not a zero-copy or throughput claim.
The small 2x cut case's first published image records three inferred source
frames, 82,944 inference-upload / 1,327,104 inference-readback bytes, 589,824
processing-upload / 147,456 display-readback bytes, and a 36,864-byte renderer
upload. Tested admission totals are 151,207,184–165,254,480 bytes under 384 MiB;
they include workspace estimates, not a strict bound on opaque runtime/driver
allocations. Preview geometries beyond these fixtures are not newly accepted.

The actual worker also passes source replacement during preparation, correct
replacement geometry, worker shutdown, real renderer `device.destroy()`, an
actionable loss error and fresh generation 43 retry. A linked external stop flag
retires preparation when the renderer is lost, without allowing a seek cancel
to mark the renderer unhealthy. Explicit Off clears stale requests/counters and
restores ordinary player policy even with missing assets. Initial tests caught
retained inactive CLAHE still being executed, and an Off request surviving a
policy-cache early return; both failures are preserved and fixed. Neither the
cancellation test nor the timer alone proves destruction during an active model
dispatch. Spontaneous driver loss and physical OOM remain unverified.

Evidence in `evidence/ai-upscale-20261023/`:
[native service/model/provider metadata](evidence/ai-upscale-20261023/native-ai-preview-service.json),
[renderer, replacement and controlled loss/retry](evidence/ai-upscale-20261023/native-ai-preview-renderer.json),
[independent pixels](evidence/ai-upscale-20261023/native-ai-preview-pixels.json),
and corresponding service/renderer test transcripts plus initial failure logs.
Raw rasters stay ignored. Reproduction is in `tools/ai-upscale/README.md`.
Final checks for this native slice: locked `media-native` library tests pass
46 with 9 ignored; ordinary `converter-native` binary tests pass 16 with 5
ignored. The two actual GPU preview tests were run explicitly, as was the
four-test AI video regression (including controlled cancellation/device loss,
helper crash, budget rejection, pixel reference and fresh retry). Host Clippy
for both native packages with `--all-targets -- -D warnings`, formatting,
Python syntax, diff whitespace and strict OpenSpec validation pass. These
counts do not turn remaining ignored tests into passing skips.

This is actual decoded-video/GPU headless evidence, not ordinary launch,
Before/After, Play/Pause/audio sync, UI exposure or native release acceptance.
Tasks 6.2/6.4 and 7 remain open; RTX remains pending and both Burn candidates
remain unavailable.

## AI continuation checks — 2026-10-23

After the browser preview/cache changes, `cargo fmt --all -- --check`, locked
wasm32 Clippy for `media-web` plus `converter-web`, and the separate
`media-web --features gpu-setup-diagnostic` Clippy all pass with warnings denied.
`node --test tests/worker-transport.test.mjs` passes all nine tests. Node syntax
checks pass the browser, failure, paused-preview and history harnesses; Python
AST checks pass the evaluation scripts, and the two new CC0 fixture SHA-256
manifests match their small generated media. `git diff --check` and
`openspec validate ai-upscale --strict` pass. The isolated release-WASM probe
runs actual GPU tests; ordinary app UI/runtime release acceptance is still open.

## Browser AI finite-history preview — 2026-10-23

Task 6.3 now passes on the supporting browser diagnostic route in Edge
155.0.4283.62, Intel gen-12lp, ONNX Web 1.30.0, main and worker. The existing
exact preroll now processes each predecessor through the same concrete AI stage
before shared CLAHE statistics at selected output resolution. It retains no
raw-frame sequence: predecessors are consumed/drained one at a time, with the
existing 64-frame/100-ms bound. Source/seek/AI scale/device identity invalidates
the selected result and statistics; downstream edits reuse both.

The tiny CC0 `ai-upscale-cut.mp4` adds an abrupt synthetic inversion at 100 ms
without downloading media. Explicit PTS assertions check selection at 1350000,
1316666 and 1483333 us. The first target includes two predecessors (including
the exact 100-ms boundary), backward seek includes one, and the final target
has zero after the 133333-us gap. Both 2x and 4x statistics use selected sizes
128x72 / 256x144. Strength edits and Before/After do not increase the AI source
counter; scale changes rebuild three required frames. Only one model is
allowlisted: an unvalidated model change rejects explicitly, and restoration of
the validated model succeeds; no second model is claimed.

**20 comparisons pass at maximum 1 LSB** against matching source-PTS frames
from chronological pre-encoder exports (plus independent original pixels for
Before). Eight four-frame H.264/AAC reference exports independently pass complete
FFprobe and FFmpeg A/V decoding. The initial gap request was just before the
last source frame and selected the preceding frame; that run is not gap-expiry
evidence. The final probe asserts exact PTS and predecessor count, requests a
position inside the final frame, and preserves the original tolerance.

Evidence: browser-ai-preview-history-{main,worker,pixels,independent}.json in
`evidence/ai-upscale-20261023/`; source recipe/hash in the fixture manifest.
Native AI preview was outside this browser gate; the newer staged native
evidence is above, and task 6.2 remains open. Playback/coalescing and
UI/release acceptance are separate. The post-preview regression reruns all ten
active conversion failure/retry cases successfully, including ownership cleanup,
with ten independently decoded retries; see
browser-ai-failures-preview-regression-{main,worker,independent}.json.

## Browser AI paused preview — 2026-10-23

Task 6.1 now has actual Edge 155.0.4283.62 main/worker evidence on the
assessed Intel device and ONNX Web 1.30.0. The diagnostic-only
`preview-ai-pilot` / `preview-ai-before-pilot` commands reuse the selected
model and shared processing device. They normalize orientation, infer at the
selected 2x/4x output geometry, apply CLAHE/manual controls there, then scale
for the existing 640x360 display bound. Ordinary UI commands still reject On.

One source-PTS result is cached with source identity, AI settings and device
generation. Manual and Strength edits reuse it; the test records exactly one
processed AI source frame throughout the paused edit sequence. Before shows
the normalized original with neutral color while retaining the selected AI
settings/cache; After restores the cached result. Off releases the AI stage and
restores ordinary resize. Preview remains available after the explicit FFmpeg
Original-VFR profile probe rejects export. Clearing preview now awaits pending
GPU work and AI retirement, including conversion handoff. No CPU frame readback
was added to production preview; raw comparison readbacks are diagnostic-only.

Four matrices cover small 4x, small 2x, display-limited 4x (192x96 -> 768x384
-> 640x320), and rotated/flipped 4x. Each runs main and worker with neutral,
manual, CLAHE 50/80, Before/After, rejected-export preview and Off cases:
**64 pixel comparisons pass at maximum 1 LSB**, inside the unchanged 2-LSB gate.
The 32 matching four-frame H.264/AAC exports independently pass complete
FFprobe timestamp/grid checks and FFmpeg video/audio decoding. Non-Before
pixels compare with the matching pre-encoder export plus independent bilinear
display scaling; Before/Off compare with independently normalized original
pixels. Alpha and source PTS are checked. An initial large Before result failed
at 13 LSB because retained CLAHE resources caused a second resize; that failed
result is retained and the bypass now uses direct original-to-display scaling.

Admission counts retained preview targets, including the internal x4 surface
for 2x: 24 bytes per selected output pixel for 2x, 8 for 4x, in addition to
existing inference/downstream reservations. The shared selected-size color
target avoids a duplicate allocation when CLAHE is active. These tests cover
paused first-frame edits with empty temporal preroll. They do **not** establish
playback, latest-only coalescing, native preview, UI exposure or real-time
performance. Finite-history seeking/cut evidence is in the newer section above;
tasks 6.2/6.4 and release gates remain open.

Evidence under `evidence/ai-upscale-20261023/`: browser-ai-preview-{4x,2x,display,rotated}-{main,worker}.json,
corresponding `-pixels.json` and `-independent.json`, and
[both Before failure contexts](evidence/ai-upscale-20261023/browser-ai-preview-before-initial-failed.json).
Raw rasters and encoded outputs remain in ignored diagnostic directories;
committed evidence contains metadata/results, not model/runtime binaries.
Reproduction: `tools/ai-upscale/README.md`.

## Active AI failures and ownership — 2026-10-23

Task 4.5 now has actual decoded-video failure/retry evidence in Edge
155.0.4283.62 main and worker on the existing Intel gen-12lp device, ONNX Web
1.30.0 and the unchanged model. Diagnostic-only instrumentation served by the
isolated harness exercises five cases: cancellation after a real model dispatch;
the real ORT missing-input error; injected WebGPU out-of-memory scope results at
setup and after a tile run; and actual device destruction after queue submission
while the AI tile is active. OOM responses are **injected**, not physical GPU
exhaustion. The new failure instrumentation stays in the diagnostic server/native test configuration; the production AI boundary adds no failure hook or pixel readback.

The tests exposed two gaps. Setup/tile processing now consume explicit OOM as
well as validation scopes, and setup errors release a created session plus the
caller-owned buffers before returning. A synchronous release-call error also
cannot skip buffer destruction. Actual device destruction initially allowed
conversion to return success: queue completion alone is insufficient loss proof.
The concrete JS boundary now observes `device.lost` once per device, independently
of session lifetime and without replacing wgpu's callback. It checks loss before
and after inference/drain. Rust checks its existing loss flag before frame
submission/capture and before accepting a successful settled result. Decoded
frames enter their Drop guard before early configuration/loss errors.

All ten main/worker failure cases now reject without returning an output artifact.
After each failure and each explicit retry, runtime instrumentation records zero
active runs/caller GPU tensors and zero tracked AI resources, including tensor
buffers, tile uniforms, source/results and the video stage's two uniforms. Every
created session is released, inactive and poisoned with no retained session.
Rust's feature-only diagnostic independently reports **zero live leases, zero
pending submissions, no AI stage and no held AI credit**. This is application
ownership evidence, not driver/ORT pool/GC memory proof. Device loss is observed
on generation 1; same-canvas retry creates healthy generation 2. Other failures
retry with a fresh session on the still-healthy generation 1. Both contexts request
exactly two devices across the complete test. Ten retry outputs independently
pass complete FFprobe grids and FFmpeg video/audio decoding. WebCodecs conversion
reports zero explicit pixel readbacks; diagnostic ownership counters add none.

Native DirectML/shared-wgpu video tests additionally pass real helper process
crash after one processed frame and checked preallocation-budget rejection.
Cancellation and actual processing-device destruction after one processed frame
continue to remove unpublished partials; fresh generation 14 passes the four-frame
retry. A test-only 1-byte budget exercises the real 152,067,344-byte reservation
check before GPU/helper setup; it does not exhaust memory. The separately verified
native helper active-request cancellation and kill/reap remain applicable. Native
staging remains explicit and separately counted as in the preceding conversion
section; no foreign texture or codec-surface sharing was added.

Evidence: [main failures/ownership](evidence/ai-upscale-20261023/browser-ai-failures-ownership-main.json),
[worker failures/ownership](evidence/ai-upscale-20261023/browser-ai-failures-ownership-worker.json),
[independent retries](evidence/ai-upscale-20261023/browser-ai-failures-independent.json),
[native actual failures](evidence/ai-upscale-20261023/native-ai-failures.txt),
[initial actual-loss failure](evidence/ai-upscale-20261023/browser-ai-device-loss-initial-failed-main.json),
[first harness result](evidence/ai-upscale-20261023/browser-ai-device-loss-first-harness-miss-main.json),
[tile regression main](evidence/ai-upscale-20261023/browser-ai-tile-regression-main.json),
[tile regression worker](evidence/ai-upscale-20261023/browser-ai-tile-regression-worker.json).
The first harness did not establish a rejected loss case; the stronger tile-active
queue hook confirmed actual destruction and the bad success path before its fix.
The six-shape numerical/tile/lifecycle/budget regression still passes main/worker.
Native test suite for this slice: four tests including the actual GPU test pass;
wasm/native pilot Clippy passes with warnings denied. Spontaneous driver reset,
physical allocation pressure, OS process termination and deadline expiry remain
unverified. No preview/UI or real-time performance claim follows.

## Portable conversion and route gates — 2026-10-23

The native feature `ai-upscale-pilot` now provides an explicit `ai-video-pilot`
CLI/session entry point. Normal app `convert_job` still rejects On. On the
assessed NVIDIA DirectML helper plus NVIDIA Vulkan shared-wgpu processing,
**six** actual software-codec H.264/AAC exports pass 2x/4x at Original/15/60 FPS
with CLAHE 50 and brightness/contrast/saturation 10/115/80. Each of four source
frames runs inference once; recorded CLAHE pass count is 16. Exports contain
four/five/17 frames. Independent FFprobe full-grid checks and complete FFmpeg
video/audio decoding pass all six. Native reports retain separate inference
tensor staging and downstream wgpu upload/readback counters; no codec-surface
sharing is implied. These app builds were debug, not release performance tests.

Native order is explicit FFmpeg decode/color normalization -> DirectML x4
clamp/quantization -> upload to the selected processing device -> optional shared
RGBA8 2x post-downscale -> CLAHE/manual controls -> readback -> timestamped
Matroska pipe -> existing FPS/encode/audio policy. Source SAR/display transforms
remain rejected by the native shared-route policy. Downstream surfaces, padded
readback, CPU full frames and CLAHE scratch are admitted alongside core runtime
allowances before allocation. Exact hardware/provider/session identity and all
explicit staging remain visible, with no new raw-frame files in conversion.

The first tiny native attempt failed `invalid frame duration`: the container's
declared stream duration omitted its B-frame presentation tail. AI jobs now scan
integer packet PTS+duration with bounded scalar state/count, check the source
origin and use exact presentation coverage for the last frame and CFR endpoint.
The pre-existing AI Off duration policy is unchanged. The actual fixture tests
3072 declared ticks versus **4096 presentation ticks**; no tolerance was widened.

Actual native pre-encoder synthetic pixel tests combine the same helper with
shared post-scale/manual controls and the checked-in independent CPU FP32
reference: **max 1 LSB** for x4 and x4->2x. Actual video cancellation and processing
device destruction after one completed AI frame remove unpublished partials;
fresh generation 14 passes a new four-frame export. The separate helper test
cancels an active request and tests a real child crash. This does not prove
driver OOM recovery, device loss inside an inference kernel or timeout expiry.

**Native NVDEC/NVENC AI is unavailable.** The first 128x72 attempt fails NVENC
minimum output dimensions. A separately generated original CC0 96x80 source
passes dimensions at 2x but fails the unchanged Original-VFR check: expected
first video PTS **22000 us**, got **0 us**. Decoder and encoder exited successfully;
verification rejected publication and the partial was removed. This is a codec
timing failure, not an ONNX numerical failure or an RTX Video SDK result. The AI
route now returns that specific unavailable reason before source inspection or
output creation. No implicit software-codec fallback is added, and ordinary
non-AI NVIDIA behavior remains unchanged. Further NVENC timestamp investigation
is pending; no passing AI native NVIDIA-codec claim is made.

Browser Edge main/worker additionally pass **24** WebCodecs WebM cases: 2x/4x ×
Original/15/60 × VP8/Opus or VP8 video-only, with the same CLAHE/manual settings.
The existing four VFR source frames run AI once each; independent complete grid,
geometry, metadata, audio and decode checks pass. Video-only uses video origin,
so its 15/60 grids have four/16 frames rather than the audio-inclusive five/17.
An initial verifier incorrectly included the omitted audio origin; that failed
result is preserved, and the corrected verifier matches declared track policy.
The exports themselves required no change. No explicit WebCodecs pixel readback
was added.

The existing fixed-FPS **FFmpeg WASM** route also passes **eight** main/worker
cases: 2x/4x × 15/60 with H.264/AAC, CLAHE/manual settings, complete independent
video/audio decode and exact grid checks. AI runs four times; FPS duplicates
reuse it. This route explicitly performs five/17 `VideoFrame.copyTo` encoder
readbacks (184,320/626,688 bytes at 2x, 737,280/2,506,752 at 4x), one raw frame
plus a 4 MiB live ring and OPFS compressed output. No raw spool or whole-source
application copy is introduced. Sampled WASM linear allocation is 33,554,432
bytes, not a strict bound on all browser/codec/runtime memory. Original VFR is
still outside the existing FFmpeg WASM contract; no inference is run in its encoder.

Evidence: [native matrix](evidence/ai-upscale-20261022/native-video-software-native.json),
[native independent decode](evidence/ai-upscale-20261022/native-video-software-independent.json),
[native pixel/lifecycle test](evidence/ai-upscale-20261022/native-video-lifecycle.txt),
[native helper lifecycle](evidence/ai-upscale-20261022/native-helper-v2-lifecycle.txt),
[initial duration failure](evidence/ai-upscale-20261022/native-video-initial-duration-error.txt),
[NVENC dimensions](evidence/ai-upscale-20261022/native-video-nvenc-dimension-error.txt),
[NVENC timing](evidence/ai-upscale-20261022/native-video-nvenc-timing-error.txt),
[WebM main](evidence/ai-upscale-20261022/browser-video-webm-matrix-main.json),
[WebM independent](evidence/ai-upscale-20261022/browser-video-webm-matrix-independent.json),
[initial video-only oracle failure](evidence/ai-upscale-20261022/video-initial-video-only-origin-error.json),
[FFmpeg WASM main](evidence/ai-upscale-20261022/browser-video-ffmpeg-matrix-main.json),
[FFmpeg WASM independent](evidence/ai-upscale-20261022/browser-video-ffmpeg-matrix-independent.json).
Matching worker evidence is retained alongside main. Preview, UI, broader active
failure/OOM tests and release temporal/quality acceptance remain open. All earlier
notes below describe their dated slices; this section supersedes their native
video, x2 and browser route-pending statements only for the tested pilots.

Verification for this continuation: `cargo test --locked -p media-core -p
media-native -p media-web --lib` passes **38 core / 46 native / 3 web** tests
(eight native GPU/environment-dependent tests remain ignored there; the new
helper and AI video runtime tests were explicitly executed separately above).
`cargo test --locked -p converter-native --bin converter-native` passes **15**
tests with four existing renderer-dependent tests ignored. Worker transport
passes **9**. Locked all-target Clippy with warnings denied passes host core,
both shared GPU ABIs, media-native/native app; native `ai-upscale-pilot` also
passes separately. Wasm32 media-web/web app Clippy passes, plus diagnostic-feature
media-web Clippy. Formatting, JS/Python syntax, whitespace, all evidence JSON,
tiny fixture hashes/licenses and strict OpenSpec validation pass. The first app
test invocation requested a nonexistent library target; the actual binary test
target above was then run successfully. Known GNU `.drectve` linker warnings
remain nonfatal. No new non-ignored file exceeds 1 MiB; compiled runtimes/models,
large raw diagnostics and build products remain outside source changes. This is
not full task-8 release/quality acceptance or GUI evidence.

## Native portable frame adapter — 2026-10-23

`media-native::ai_inference` now has a concrete Off/ONNX dispatcher. Failed Burn
and pending RTX candidates return their assessed unavailable reasons. The optional
source-only Windows C++ helper loads existing ONNX Runtime DirectML **1.23.0**
through C API 23, verifies the allowlisted graph again, disables CPU fallback and
requires exact DXGI ordinal/name. The Rust parent checks local model integrity,
runtime x64/product metadata, handshake version/model/device/PID and device
generation. No external runtime was added to Cargo or installed automatically.
MinGW-w64 builds against hash-pinned official headers with warnings denied; its
scoped SAL compatibility block is documented in the helper README.

Actual Windows execution selects **NVIDIA GeForce RTX 3050 Laptop GPU**, DXGI 1,
vendor 4318/device 9634, LUID 0:80112. The helper's numeric mode passes all five
unclamped FP32 reference shapes, worst max **3.45707e-6**, MAE **3.96991e-7**.
Its actual profile has **350 DirectML node events, zero CPU provider events**.
The v2 production-shaped pipe accepts one <=196x196 FP32 tile; shared Rust core
owns 128-interior/34-halo planning and Rust owns explicit CPU packing/cropping.
No resource crosses the wgpu-30/26 ABI or DirectML device boundary by handle.

The actual Rust CLI completes **577x579 -> 2308x2316**, 25 tiles, comparing all
final RGB pixels with the independent unclipped PyTorch reference after identical
clamp/quantization: max **1 LSB**, MAE **0.0001002745 LSB**, alpha opaque. Counted
tensor payload staging is **8,669,988 bytes host-to-GPU / 138,719,808 GPU-to-host**,
including overlap; pipe copies and provider-internal copies are additional.
Sampled process-local peak is **126,369,792 bytes**, wall **7.487 seconds** including
process/model initialization, transfers, inference and close. Numeric-mode peak
is 167,919,616 bytes. DXGI samples/headroom estimates do not strictly bound runtime
allocations, and do not imply that a released session returns global VRAM to zero.

Actual tests pass repeated frames, reference pixels, transfer totals, stale
generation rejection, active-request cancellation with kill/reap, retired-owner
rejection, explicit fresh-child retry and child crash cleanup. Cancellation is
in an active request/pipe; it does not establish exactly which GPU kernel was
active. Independent malformed protocol/version, oversized tile and truncated
payload tests fail precisely and reap each child. Mutable frame ownership plus
zero-capacity request/one-capacity reply channels prevent queued frame sequences;
startup/frame timeout is 60 s, shutdown allowance 2 s. Timeout expiry itself has
not been stress-tested. Off/failed candidates create no child or asset IO.

Evidence: [actual numeric/provider/envelope/protocol results](evidence/ai-upscale-20261022/native-helper-v2.json).
Reproduction and protocol: [helper README](../tools/ai-upscale/native-onnx-helper/README.md).
This completes the native bounded inference-adapter slice, **not native AI video,
preview or UI**. Ordinary conversion still rejects On. Native Burn remains failed
and task 1.4 remains open; SDK execution/terms/toolchain remain pending.

## Browser AI video composition pilot — 2026-10-23

The isolated `gpu-setup-diagnostic` build now offers `convert-ai-pilot`; ordinary
app builds still reject AI On. Existing decoded-frame ingress normalizes
crop/PAR/orientation on the existing device, then runs cached ONNX Web **1.30.0**,
clamps/quantizes x4 RGB to RGBA8, optionally uses the shared final downscale for
2x, then applies CLAHE/manual controls before existing FPS resampling and muxing.
One AI frame/tile credit is retained; source frames advance AI/CLAHE once and
output duplicates reuse processing. Admission includes downstream CLAHE and
retained frame/surface reservations. Errors, including synchronous glue errors,
follow drain/close before returning. Precision checks require known opaque 8-bit
SDR source/profile; unknown/high-precision AI formats reject.

Actual **Edge 155.0.4283.62**, Windows, Intel gen-12lp processing device, main and
worker: 16 tiny cases pass (Off baseline, 2x/4x each at Original/15/60 FPS with
CLAHE/manual controls, plus rotated/flipped 4x). The checked-in original CC0
64x36 fixtures have four VFR source frames and offset audio. Original exports
four frames; complete rational 15/60 grids contain five/17 frames. Independent
FFprobe plus complete FFmpeg video/audio decode checks pass all cases. WebCodecs
H.264/AAC is the tested route; no hardware codec execution claim follows.

Independent first-frame pre-encoder CPU pixel checks pass six main/worker cases:
2x with manual controls max **2 LSB**, MAE **0.16855 LSB**; 4x manual max **2 LSB**,
MAE **0.00165 LSB**; 90-degree/flip 4x neutral max **1 LSB**, MAE **0.00110 LSB**.
This diagnostic reads ingress/output pixels and is excluded from fast-path
throughput. CLAHE is covered by frame/history counts and its existing engine
tests, **not an independent AI+CLAHE full-video CPU pixel oracle**.

The original 320x180, 36-frame CC0 offset/VFR clip also passes x4 H.264/AAC full
browser re-decode and independent FFmpeg checks in both contexts: 1280x720,
36 frames, 95 audio packets, maximum browser timestamp quantization 4 microseconds.
Wall time is **175.84/182.14 seconds** for roughly two seconds of media, so no
real-time claim is justified. All normal video runs report zero explicit pixel
readbacks, zero ImageBitmap ingress conversions and zero application-held
frames/samples/GPU leases at cleanup. Requested allocation counters can retain
destroy-unobserved pooled/CLAHE allocations; they are not actual live VRAM/GC
measurements. Opaque driver/runtime memory is not bounded by these counters.

Actual controlled device destruction after one completed AI source frame fails
without publishing an artifact and releases held frames/samples; an explicit
same-canvas retry creates generation 2 and passes full four-frame video/audio
decode. Both main/worker request exactly two devices across failure/retry. This
tests destruction between AI frames, not loss inside an active inference kernel.
Raw Edge FP32 caller-buffer tests also pass the five shapes, max **6.16908e-6**,
MAE **5.6124e-7**, all 73 optimized nodes on the exact processing GPU device.
Edge uses enable-unsafe-WebGPU/ANGLE D3D11 flags; the older Chromium FXC workaround
is not used for this Edge evidence. Firefox was not tested.

An initial video run rejected the old ordinary no-enlargement check before
processing; the diagnostic AI path now checks exact explicit scale geometry.
An initial independent timing oracle used MP4 stream duration (0.2 s), which
omitted the B-frame presentation tail. The corrected independent verifier uses
maximum integer packet PTS+duration (0.266667 s presentation coverage) and the
existing rational-grid oracle; it does not relax tolerance or discard an ending
frame. The failed result is preserved beside the passing evidence.

Evidence: [tiny main](evidence/ai-upscale-20261022/browser-video-tiny-matrix-main.json),
[tiny independent decode](evidence/ai-upscale-20261022/video-independent-tiny.json),
[original main](evidence/ai-upscale-20261022/browser-video-original-main.json),
[original independent decode](evidence/ai-upscale-20261022/video-independent-original.json),
[CPU pixels](evidence/ai-upscale-20261022/video-cpu-pixel-reference.json),
[initial duration-oracle failure](evidence/ai-upscale-20261022/video-initial-duration-oracle-failed.json),
[device loss main](evidence/ai-upscale-20261022/browser-video-device-loss-main.json),
[device loss worker](evidence/ai-upscale-20261022/browser-video-device-loss-worker.json),
[retry independent decode](evidence/ai-upscale-20261022/video-independent-device-loss.json).
Matching worker/raw/parity evidence is retained in the same directory.

Tasks 4.3–4.5 remain open: native composition/export, browser WebM/audio and
supported fixed-FPS FFmpeg WASM routes, full active-inference video cancellation,
runtime/OOM stress and preview remain unaccepted. No UI or release capability is
enabled by these diagnostics. Locked host/core/GPU/native and diagnostic wasm
Clippy pass with warnings denied; the known GNU `.drectve` linker warning remains.

## Native runtime discovery — 2026-10-23

The optional `ai-runtime-status` CLI reads explicit directories or
DIAXUS_ORT_DIRECTORY / DIAXUS_RTX_SDK. It never loads their libraries, searches
system DLL locations, downloads/installs dependencies or accepts SDK terms.
Safe-Rust bounded PE/resource parsing reports missing files, non-x64 architecture,
malformed/versionless components and metadata outside the assessed versions.
No proprietary binaries were added to source changes.

Actual explicit-path inspection finds ONNX Runtime DLL product version
**1.23.25.924**, DirectML **1.15.4.0**, and the supplied SDK 1.1.0 package's VSR DLL
**1.6.0.0**, all x64. The ONNX product resource is build metadata, not its C API's
semantic 1.23.0 version. Likewise VSR DLL 1.6.0.0 does not mean SDK package 1.6.
Headers/license presence is reported separately; every component remains
present-unvalidated and inference availability is false. SDK execution is still
pending as requested, with no license/toolchain installation or acceptance.

Three native tests pass absent/default paths without file creation, malformed
headers, x86 architecture rejection, out-of-bounds resources, incompatible version,
explicit corrected-file recovery and missing-file recovery. Actual CLI inspection
also passes. Native all-target Clippy passes with warnings denied. See
[discovery evidence](evidence/ai-upscale-20261022/native-runtime-discovery.json).

## Enlarged codec probing and estimates — 2026-10-23

The browser's real `execute_job` probe path now resolves AI intent separately from
ordinary resize and inference availability. With the existing CC0 640x360
H.264/AAC fixture, retained 50% resize produces 320x180 for Off; 2x/4x requests
probe 1280x720/2560x1440. Source-aware rates and file-size estimates use the same
geometry, selected FPS and unchanged source/A/V coverage. Original and 60 FPS
queries are checked against actual delegated VideoEncoder.isConfigSupported calls.
For example, 4x/60 queries 2560x1440, 60 FPS and 8,049,568 bps, with an estimated
2,124,017-byte AVC/AAC file. These are estimates/codec checks, not produced exports.

Bundled Playwright Chromium 141 **fails input H.264 decoding** in both contexts;
that failing boundary is retained. Installed Microsoft Edge 155.0.4283.62 passes
all six combinations in main and worker without requesting a processing GPU
device or loading the model/runtime. AVC is supported for these exact queries;
HEVC is unsupported. An actual 999998-square exact codec query rejects both codecs.
The rejection message now correctly reports selected 60 FPS instead of source
30 FPS. Successful AVC probing still reports AI unavailable, and both conversion
and preview On requests reject at their entry gate before processing/output.
The 640x360 probe fixture exceeds the inference pilot's width envelope, further
illustrating that codec support grants no inference capability.

Native tests inspect the real fixture via FFprobe and pass AI-aware 2x/4x
source-aware rate/estimate checks for Original/60 FPS, legacy Off equality and
unchanged source metadata/timing. All four native routes and both AVC/Main-10
profiles reject unavailable On snapshots before opening a nonexistent input or
creating output. Core tests reject unknown/non-8-bit, non-SDR/nonopaque and Main 10
AI formats. No encoder fallback or precision truncation is added.

Evidence: [Edge main](evidence/ai-upscale-20261022/browser-ai-codec-main.json),
[Edge worker](evidence/ai-upscale-20261022/browser-ai-codec-worker.json),
[Chromium decoding failure](evidence/ai-upscale-20261022/browser-ai-codec-chromium-failed.json).
This does not establish encoded output correctness or Edge WebGPU inference;
those remain independent integration/acceptance work.

## Shared GPU AI tile adapter continuation — 2026-10-22

The browser integration pilot now uses shared Rust/wgpu packing and unpacking:
normalized RGBA8 unorm texture -> FP32 RGB NCHW -> ONNX Web 1.30.0 -> cropped,
clamped/quantized RGBA8 x4 texture. Both tensor buffers are allocated by Rust on
the existing processing device. ORT writes into the caller's output buffer; no
foreign buffer is imported into wgpu, and no production pixel readback/upload is
added. The earlier caller-output probe passed all five FP32 references with exact
output-buffer identity and all 73 nodes on WebGPU. Its initial diagnostic failed
because caller-owned tensors have no default getData downloader; adding a separate
test-only download callback fixed the harness without changing inference.

`BrowserAiProcessor` owns one 196x196-capacity input/output pair (7,836,864 bytes),
one inference frame at a time, immutable tile uniforms and explicit completion
waits before reuse. The opt-in `ProcessingAiPilot` belongs to the actual GpuSession
and rejects a second session until cleanup completes. Wrong generations reject;
cancel/runtime failures retire the processor and cannot return a partial image.
Model/runtime creation reads only the verified caches. Explicit close drains GPU
work, releases ORT and destroys owned buffers; Drop is an additional safety net.

Actual release-WASM Chromium 141.0.7390.37 tests on Intel gen-12lp pass in main and
worker with one processing-device request and an empty GPU validation scope.
All six shapes (1x1 through 577x579) match independently clamped/quantized PyTorch
references within **one RGBA8 byte level**, including the 25-tile envelope. This
final-image check supplements the existing unclamped FP32 tolerance checks; it
does not replace or relax them. Cancellation after the first actual GPU tile,
stale-generation rejection, second-session rejection, idempotent close, two-frame
buffer reuse and explicit fresh-session retry pass. Requested resources return
to the original processing baseline (32 buffer bytes + 921,600 texture bytes)
after every fixture and the lifecycle sequence.

The largest requested-resource peak is **106,569,740 bytes**, including test-only
upload and 21,937,152-byte padded RGBA8 readback. The admission ledger reserves
source/output textures, both tensor buffers, tile uniforms, 8 MiB for weights,
128 MiB estimated runtime workspace, and caller-supplied downstream/retained-frame
bytes against 384 MiB. The 577x579 reservation is 173,162,444 bytes before downstream
or retained-frame costs. Oversized geometry and combined downstream reservations
reject before model/runtime setup. The workspace is a conservative pilot estimate;
opaque driver/compiler/internal-surface allocations are unknown, and the diagnostic
allocation counter does not observe GC. No total-VRAM hard-cap claim follows.

An independent native GPU boundary test also passes on NVIDIA RTX 3050 Laptop,
Vulkan, driver 621.01: nine reverse-order tiles at 257x259, exact RGB FP32 packing,
halo crop and <=1-byte quantization error, plus malformed-plan/generation rejection.
The same module compiles against both processing wgpu 30 and renderer wgpu 26;
this is not evidence of native inference integration or cross-device sharing.
Burn remains unavailable at its recorded browser/native kernel failures.

Evidence: [main adapter](evidence/ai-upscale-20261022/browser-tile-context-main.json),
[worker adapter](evidence/ai-upscale-20261022/browser-tile-context-worker.json),
[caller-owned output](evidence/ai-upscale-20261022/browser-caller-output-main.json),
[initial downloader harness error](evidence/ai-upscale-20261022/browser-caller-output-initial-harness-error.json).
The standalone source-only harness has a locked dependency set and uses no new
downloaded runtime or model. Actual decoded-video composition, CLAHE/FPS/codec,
x2 post-downscale, device destruction/OOM and preview/export acceptance remain
open. Ordinary app builds do not enable the pilot owner API; both conversion
entry points continue to reject AI On, and no AI UI is exposed.

Validation during this slice: 38 core tests, three command-codec tests and nine
worker-transport tests pass. Host shared GPU crates and wasm media-web Clippy pass
with warnings denied; formatting and JS syntax checks pass. Full non-AI video
regression exports have not been rerun, so task 8.1 remains open.

## Explicit AI asset provisioning continuation — 2026-10-22

The user selected local import of the exact validated ONNX graph. The shared
`media-core/src/ai-upscale-model.json` records its 4,866,413 bytes, SHA-256, upstream
weights/commit, export recipe and BSD-3-Clause license. It also pins ONNX Runtime
Web 1.30.0's three assets (26,901,320 bytes), their hashes, MIT license and explicit
acquisition URLs. CDN HEAD checks return 200 and `Access-Control-Allow-Origin: *`;
runtime functional tests use local mirrors of the pinned bytes, not a claim of an
end-to-end CDN download test. The upstream MIT text is retained in tools/ai-upscale.

Browser APIs perform no cache/network work when their module is imported. Model
acquisition accepts a local File only, verifies size/hash before atomic CacheStorage
publication, rechecks every cache read, preserves valid entries when bad input is
submitted and provides explicit removal/reimport. Runtime acquisition is a separate
explicit action: bounded streamed response bodies, 60-second asset deadlines,
cancellation polling/AbortController, per-asset hash checks and rollback of entries
newly published by a failed/cancelled attempt. Runtime loading performs no network
fallback; it verifies cached code before module-blob execution, supplies verified
WASM bytes directly and serializes concurrent loads into one runtime instance.
Compiled code is retained for its execution context; persistent-cache deletion is
not claimed to unload an active runtime. These APIs are not connected to AI UI yet.

Actual Chromium 141 CacheStorage tests pass valid import, independent offline read,
existing-cache reuse, corrupt/truncated input and corrupt-cache rejection,
incompatible-manifest rejection, cancellation before/after hashing, cancellation
after one runtime entry was published with rollback, explicit recovery and offline
runtime-module loading. A separate actual offline runtime acquisition fails with
an actionable error and leaves no newly downloaded entries. A real origin quota
override of **one byte** produces a cache-publication quota error; restoring the
quota permits explicit import and verified read. An additional mocked quota path
is identified separately. Observed fixture/runtime requests are GET only; no
source media is uploaded. The cache probe creates no GPU device.

The same verified model/runtime caches also pass **all five existing numerical
reference cases in both main and worker** on the actual processing GPUDevice,
using GPU-buffer IO, no CPU EP fallback and one device request. Original numeric
errors, all-WebGPU placement, empty validation scope and post-release device reuse
are preserved with the module-blob/WASM-binary loading path. This remains a tensor
pilot using diagnostic uploads/readbacks, not a video conversion acceptance test.

Native `ModelCache` creates no files at construction or cancelled/missing reads.
Explicit import verifies the model in bounded host memory, publishes a synced
temporary file by rename, and re-verifies every subsequent read. Actual CLI tests
pass import/separate-process read, reuse without rewriting, corrupt/truncated input
rejection before cache directory creation, corrupt-cache rejection, explicit
removal/recovery, and failure for a cache path whose parent is a regular file.
That tests an unwritable path, not every Windows ACL/disk-full failure. No partial
files remain in those cases. Pre-cancelled imports perform no filesystem work;
cancellation during an actual native disk write remains unmeasured. SHA-256 uses
pinned sha2 0.10.9; Cargo.lock now includes that dependency. It was already cached,
so no installation was needed. No native inference runtime is provisioned here.

Evidence: [browser cache/failures](evidence/ai-upscale-20261022/provisioning-browser.json),
[native cache/failures](evidence/ai-upscale-20261022/provisioning-native.json),
[main verified runtime](evidence/ai-upscale-20261022/browser-cached-runtime-main.json),
[worker verified runtime](evidence/ai-upscale-20261022/browser-cached-runtime-worker.json).
Core now has 34 passing tests; the three command-codec tests, two native cache
tests, native early-rejection test and nine worker-transport tests pass. Host
core/GPU/web/native/native-app and wasm32 web/app Clippy pass with warnings denied.
GNU linker `.drectve` warnings occur in some test builds and were not suppressed.
AI On still rejects at both execution entry points. Profile/codec integration,
actual frame packing, previews, video/audio/lifecycle acceptance and UI remain open.

## AI tile/memory and core-policy continuation — 2026-10-22

The current five-cell assessment remains Web ONNX 1.30.0 and native DirectML
1.23.0 passing numerical pilots, both Burn cells failed, and RTX untested/pending.
Neither pilot is a release-accepted backend. The model/weights, original numerical
limits and browser diagnostic FXC flag are unchanged.

A new independent CPU reference at **577x579 -> 2308x2316** exercises full 196x196
and 324x324 halo-expanded tiles for interiors 128/256. Both interiors, edges,
seven-pixel seam bands, forward/reverse order and cross-interior comparisons pass
on main/worker Chromium 141 Intel gen-12lp and native NVIDIA DirectML. Model output
remains unclamped FP32. Maximum browser error is 5.663e-6, MAE 5.661e-7; native
max is 3.785e-6, MAE <=3.994e-7. Reversing order changes no pixels. Browser interiors
are identical here; native cross-interior max is 1.908e-6. This is spatial evidence
on a synthetic fixture, not a temporal/natural-image quality test.

| Pilot diagnostic | 128 interior | After also exercising 256 interior |
|---|---:|---:|
| Browser visible requested-resource peak, including bounded diagnostic readback | 169,927,488 bytes | 367,059,776 bytes |
| Native DXGI sampled process-local peak | 126,369,792 bytes | 244,858,880 bytes |
| Native DXGI sampled process-nonlocal peak | 43,761,664 bytes | 72,237,056 bytes |

Browser instrumentation starts before the actual Rust processing device is created
and counts every requested buffer and supported texture allocation until explicit
destroy. It does not retain handles or treat GC as observed deallocation. Its
384 MiB admission cap rejects before calling WebGPU; a deliberate over-cap request
is rejected and the same device remains usable. Unknown texture layouts reject
instead of guessing memory. The probe includes GPU-only cropped-row assembly into
one 64,143,936-byte NCHW result and a 21,381,312-byte RGBA8 texture **reservation**,
not a rendered postprocessing stage. A single 8 MiB staging buffer reads that
result for verification; no per-tile readback is added. Runtime pools are included.
Both contexts return to the measured pre-model baseline of one 32-byte buffer and
four textures totaling 921,600 bytes after session release. Internal surfaces,
compiler/driver allocations and physical residency are unobserved; these counters
are not total VRAM. The earlier full-frame diagnostic readback hit the same cap
after successful 256 inference and remains recorded as a failed run; only test
readback was changed to chunks, with no larger budget or tolerance.

Native monitoring uses the selected DXGI ordinal's `IDXGIAdapter3` from the
inference process. Microsoft's [QueryVideoMemoryInfo contract](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-queryvideomemoryinfo)
defines process usage and budget. Local/nonlocal usage is sampled every nominal
25 ms; short peaks may be missed. Each tile checks the latest local sample against
both the OS budget and a 1536 MiB admission threshold. This is not a strict
allocation guard inside DirectML. Profile audit finds 4,760 DirectML events and no
CPU node events. Explicit host input/output staging is counted per tile. After
session release, usage falls to 40,824,832 local / 34,324,480 nonlocal bytes after
100 ms, **not zero**; remaining runtime/driver caches are unaccounted beyond process
exit. Whole-process peak host working set is 859,185,152 bytes.

The next integration pilot uses **128 interior / 34 clipped halo**, one tile and
one source frame in flight, restricted to the measured source rectangle up to
577x579 (x4 output up to 2308x2316), on the recorded devices/versions only. This
conservative selection is for integration, not a proven production-safe envelope
or a performance winner. CLAHE scratch/history, real frame/import guards, codecs,
preview retention, actual tensor-to-RGBA rendering and device-loss/OOM acceptance
remain separate integration gates. No larger geometries or x2 post-downscale are
validated. Logged cold timings include shader compilation and exploratory overlap;
they are not comparable conversion benchmarks.

Evidence: [reference](evidence/ai-upscale-20261022/reference-envelope.json),
[browser main](evidence/ai-upscale-20261022/browser-memory-main.json),
[browser worker](evidence/ai-upscale-20261022/browser-memory-worker.json),
[native](evidence/ai-upscale-20261022/native-memory.json). Large tensors stay in
ignored evaluation storage; no new downloads were needed.

Core now has Off-default, allowlisted immutable AI identities and distinct
unavailable/pilot-only/validated capability results. Separate AI geometry resolves
checked 2x/4x from normalized/oriented source; Off delegates unchanged to ResizeSpec.
Legacy settings, malformed backend/model/scale/extra fields, snapshot independence,
rotation/aspect ratio, overflow and configured dimension/pixel limits are tested.
The synthetic validated capabilities in unit tests do not certify a runtime.
All 33 core tests and three host command-codec tests pass. Nine worker transport
tests cover unchanged AI identity in worker/main execution and cancelled output
suppression; the native snapshot test verifies On rejects before source inspection,
job stages or output creation. Existing command variants through bitrate/FPS/color/
equalization default absent AI to Off; the checked encoder/parser now carries the
explicit identity, and malformed or excess fields reject. Native job constructors
default Off and copy the selected identity. Both real platform entry points reject
On until an inference adapter passes its gates, preventing silent ordinary exports.
Transport mocks do not prove inference or browser media interoperability.

Host core/GPU/web/native/native-app and wasm32 web/app all-target Clippy pass with
warnings denied. The focused native test build passes with a GNU linker warning
about a corrupt `.drectve` entry; it was not silenced or treated as a runtime
failure. Inference integration and real non-AI export regression remain separate
work; no AI controls have been exposed. No new dependency downloads were needed;
D: still has approximately 4.6 GB free after fixture generation and incremental
checks. The existing evaluation environment and browser are retained.

## AI upscale browser feasibility continuation — 2026-10-22

This continuation supersedes the earlier no-browser-pilot assessment, while retaining
the original failures below. The exact model, weights, FP32 references, GPU ownership
rules and max <=0.01 / MAE <=0.001 limits are unchanged. No product AI controls or
processing route is enabled by these diagnostic results.

| Cell | Current assessment | Verified result |
|---|---|---|
| Browser ONNX Runtime Web **1.30.0** | **passing-pilot (numeric/device only)** | Main and worker pass all five exact x4 reference shapes; GPU-buffer IO, one actual processing device, CPU fallback disabled, all 73 optimized nodes on WebGPU. |
| Browser Burn 0.22.1 / CubeCL 0.11.2 | **failed** | Real main/worker runs share the Rust processing handles and load hash-verified generated weights; first copy kernel requests 1024 invocations on a device permitting 256, causing validation/panic. |
| Native ONNX Runtime DirectML 1.23.0 | **passing-pilot (numeric only)** | Previously recorded five-shape NVIDIA reference/provider test remains valid; no native code path was changed in this continuation. |
| Native Burn 0.22.1 | **failed** | Previously recorded first-Conv2d CubeCL IR type failure remains; the browser has a separately measured failure. |
| RTX Video SDK 1.1.0 | **untested** | C++ prerequisites remain unresolved; no SDK installation, license acceptance or execution occurred. |

### ONNX diagnosis and passing version

Pinned Web 1.23.0's numeric exception was decoded in an opt-in diagnostic HTTP
response transformation of its Emscripten JS module. Only the exception object's
first 24 bytes and bounded ASCII string fields were inspected. The result identifies
`BufferManager::Download` in `core/providers/webgpu/buffer_manager.cc`, not an image
dimension error. Original verbose placement logs identify **33 CPU PRelu nodes**,
with MemcpyToHost/MemcpyFromHost transitions between convolution layers. Consequently
that version does not meet the GPU-only browser inference contract even if its
download exception were bypassed. No such fallback was added to production.

Setting `session.disable_cpu_ep_fallback=1` makes the unchanged full graph fail session
creation before any input is submitted in both contexts. Isolating original Conv,
PRelu, CRD DepthToSpace and nearest Resize nodes further confirms that PRelu cannot
create a GPU-only session in 1.23.0. Conv/DepthToSpace/Resize at **both** 1x1 and 17x19
reach execution but fail `WebGpuContext::Wait` with status 3. These are diagnostic
subgraphs from the allowlisted graph, not replacement product models. Their NumPy/
ONNX ReferenceEvaluator fixtures and source graph hashes are reproducible through
`tools/ai-upscale/isolate_onnx.py`. See
[1.23 diagnosis](evidence/ai-upscale-20261022/browser-ort-1.23-diagnosis.json).
Upstream version-specific source confirms the
[download path](https://github.com/microsoft/onnxruntime/blob/v1.23.0/onnxruntime/core/providers/webgpu/buffer_manager.cc)
and [wait path](https://github.com/microsoft/onnxruntime/blob/v1.23.0/onnxruntime/core/providers/webgpu/webgpu_context.cc);
the numeric thrown address is not interpreted as a stable error code.

An isolated comparison with released **1.30.0** resolves these failures for the
tested graph/device. No graph rewrite, denoise-weight blend, reduced precision,
CPU fallback or different GPU is used. The ONNX SHA-256 remains
`1793a6e7fdf15a53eed213ba269ea768b5373858ae433d56ee8e1b0424377cc5`.
Current evaluation package/lockfiles pin 1.30.0 plus Playwright 1.56.1; the original
1.23.0 package/lockfiles are preserved separately for reproduction. The checked-in
[runtime asset inventory](evidence/ai-upscale-20261022/ort-web-1.30-assets.json)
records actual JS/WASM bytes and hashes without shipping those assets in Git.

On Chromium 141.0.7390.37 / Intel gen-12lp, the same explicit FXC diagnostic flag is
still required to avoid the separately recorded DXC-loading issue. Across both
main and worker runs, observed FP32 errors are:

| Source | Max absolute error | MAE |
|---|---:|---:|
| 1x1 | 6.15e-8 | 2.94e-8 |
| 3x5 | 3.87e-7 | 9.00e-8 |
| 17x19 | 4.29e-6 | 4.99e-7 |
| 33x31 | 3.40e-6 | 5.28e-7 |
| 257x259 | 6.32e-6 | 5.62e-7 |

Each case retains GPU-buffer output until its separately counted reference readback;
the largest host input payload is 798,756 bytes, aligned input allocation 798,768
bytes, and diagnostic output payload 12,780,096 bytes. These are tensor payloads,
**not total GPU memory or a calibrated safe frame envelope**. All 73 optimized nodes
are assigned to WebGPU, and the test verifies the runtime's GPUDevice is the exact
Rust processing device with only one device request. Session release succeeds;
a subsequent 4-byte GPU copy/readback returns the expected marker on that device,
and the validation scope is empty. That additional readback is explicitly diagnostic.
It is not proof of every allocator release or full cancel/device-loss lifecycle.
See [main](evidence/ai-upscale-20261022/browser-ort-1.30-main.json) and
[worker](evidence/ai-upscale-20261022/browser-ort-1.30-worker.json).

### Independent browser Burn result

`tools/ai-upscale/burn-browser` is a standalone locked wasm32 diagnostic crate,
compiling the same generated model/weights used by the native import. A new
**off-default** `media-web/gpu-setup-diagnostic` feature retains the existing Rust
Instance/Adapter and clones that session's Device/Queue to the harness. Ordinary
applications do not enable it and gain no Burn/runtime dependency or inference
allocation. No second device or foreign-device handle conversion is used.

Actual main and worker execution selects the original Intel/gen-12lp processing
GPU, records generation 1 and one device request, imports the wgpu 30.0.1 setup,
configures F32 and loads the hash-verified BPK. First 1x1 forward fails at
`copy_kernel_t_f32_n_1_ced71a9c`: 1024 invocations exceed
`maxComputeInvocationsPerWorkgroup=256`. The adapter reports that a higher limit
could be requested at device creation, but the existing device contract was not
changed to make Burn pass. No native IR failure is assumed to apply to the browser.
An initial diagnostic continued far enough to produce invalid pixels, which its
reference check rejected (max ~4.1425, MAE ~0.3390); those values are downstream of
GPU validation failure and are not a meaningful numeric quality comparison.

The final harness captures the first uncaught panic, preserves progress/validation
evidence and retires the failed page/worker with a bounded watchdog rather than
dropping worker evidence. No other model sizes, Resize/pixel-shuffle numerical
behavior or successful Burn input/output lifetime are claimed. No tolerance,
device limit, dependency or shader patch is silently substituted. See
[main](evidence/ai-upscale-20261022/browser-burn-main.json) and
[worker](evidence/ai-upscale-20261022/browser-burn-worker.json).

This establishes an ONNX browser feasibility pilot and a concrete unavailable reason
for browser Burn. Total GPU memory, production geometry/tile envelopes, real video,
preview/export, A/V timing and lifecycle gates remain before product exposure.
RTX remains pending as requested. The earlier snapshot and failures below are
historical evidence, not the current selected browser runtime.

Continuation verification passed: host Clippy for media-core/media-gpu/media-web;
wasm32 Clippy for media-web/converter-web and separately media-web with
`gpu-setup-diagnostic`; and the standalone browser Burn locked wasm32 release
build, check and all-target Clippy. All Clippy runs deny warnings. Workspace/probe
formatting, JavaScript syntax, Python compilation, evidence JSON/fixture integrity,
strict OpenSpec validation and diff whitespace checks also pass. The application
Cargo.lock is unchanged; downloaded model/runtime/SDK binaries are absent from
tracked and non-ignored new files. Ordinary video export regressions were not run
in this continuation, and these checks do not complete task group 8.

## AI upscale feasibility gates — 2026-10-22

This is task-group-1 evidence for `ai-upscale`, not product acceptance. All application
AI controls/backends remain unimplemented. Numeric pilot means only the explicitly
tested model/device/tensor gate; preview, video output, audio/timing, quality, GPU
budget/OOM and cancellation/device-loss gates remain open. No driver/C++ toolchain
was installed and no NVIDIA license was accepted. The user explicitly authorized
isolated open-source evaluation downloads; initial PyPI timeouts cleared after the
user repaired the connection and dependencies installed successfully.

| Cell | Assessment | Verified boundary / remaining work |
|---|---|---|
| Browser ONNX Runtime Web 1.23.0 | **failed** | Main and worker import the exact export on the real Rust processing GPUDevice with identity preserved, then fail the first 1x1 GPU-buffer `session.run`. No reference output or added production pixel path. |
| Browser Burn | **untested** | Burn ONNX 0.22.0 imports the exact graph; resolved processing wgpu ABI matches 30.0.1. Browser generated-model execution, Resize/pixel-shuffle numerical behavior and same-device tensor IO have not been proved. |
| Native ONNX Runtime / DirectML 1.23.0 | **passing-pilot (numeric only)** | Five x4 tensor geometries pass on explicitly selected NVIDIA DXGI ordinal 1, with CPU fallback disabled; profile contains 350 DirectML events and no CPU node events. Host staging is explicit. No video/preview or production memory envelope. |
| Native Burn 0.22.1 | **failed** | Explicit-FP32 runner selects NVIDIA/Vulkan and loads generated weights, then CubeCL IR panics compiling the first 1x1 Conv2d contiguous-copy kernel; no reference output. |
| Native RTX Video SDK 1.1.0 | **untested** | Package/API/license/prerequisites inspected. No registered VS C++ component/compiler found; no SDK initialization/evaluation or two-input output test attempted. |

Reproduce with [tools/ai-upscale/README.md](../tools/ai-upscale/README.md). Persisted
evidence is under [evidence/ai-upscale-20261022](evidence/ai-upscale-20261022/reference.json);
larger diagnostics and models stay under ignored `tmp/ai-upscale`. Browser trials
used the existing 2026-10-09 debug Dioxus public build, WASM SHA-256
`202ab5ab80a010663c41ed5f58c6649bcfb2738a747c436a2416b21b53b53cbf`.
The probe calls the application's actual `initialize_worker` / GpuSession constructor
using an OffscreenCanvas in main and dedicated-worker contexts, not the ordinary
HTML-canvas UI, decoded-video ingress or export path. Its GPU-buffer input uploads
and output reference readbacks are diagnostics, never production throughput evidence.

Environment: Windows 11 Home 10.0.26300 x64; NVIDIA GeForce RTX 3050 Laptop GPU,
driver 621.01, nvidia-smi 4096 MiB, PCI `00000000:01:00.0`, UUID
`GPU-17655cfc-aba6-5572-bdec-274f531aefe1`. DXGI adapter 0 is Intel UHD; adapter 1 is
the NVIDIA device (vendor 0x10de/device 0x25a2; dedicated DXGI bytes 4,154,458,112).
Browser-selected processing device reports Intel/gen-12lp; it is not switched to
NVIDIA to obtain a different result. Other GPUs/browsers, Firefox and other operating
systems were not evaluated.

### Exact model and spatial reference

Official `realesr-general-x4v3.pth`: 4,885,111 bytes, SHA-256
`8dc7edb9ac80ccdc30c3a5dca6616509367f05fbc184ad95b731f05bece96292`.
The [allowlist](../tools/ai-upscale/candidate.json) records pinned upstream commit,
source/license URLs and hashes. Strength is 1 without WDN blending; the upstream
CLI defaults to 0.5 and is a different operation. BSD-3-Clause notice is retained.
CPU PyTorch 2.8.0+cpu, NumPy 2.2.6 and ONNX 1.19.0 generate FP32 unclamped references
using the original hash-verified network with only its unrelated BasicSR registration
removed. No runtime output is used as its own oracle.

Legacy TorchScript export (`dynamo=False`, deliberately pinned despite deprecation)
produces a 4,866,413-byte dynamic-spatial opset-17 ONNX graph with SHA-256
`1793a6e7fdf15a53eed213ba269ea768b5373858ae433d56ee8e1b0424377cc5`.
A second export reproduced that hash. Graph: 34 Conv, 33 PRelu, one CRD
DepthToSpace(blocksize 4), one asymmetric/floor nearest Resize, Add and one Constant.
Inputs are encoded-sRGB RGB /255, NCHW batch 1. No mean/std, global prepad, face
enhancement or input enlargement. Native x4 is checked; x2 post-downscale is still
unavailable. Raw outputs include negative values and values >1; comparisons occur
before clipping/quantization. This is not the upstream default CLI postprocessing.

The [CC0 source fixture manifest](../fixtures/ai-upscale.json) identifies the original
synthetic edge/ramp/texture pattern. Evaluated inputs 1x1, 3x5, 17x19, 33x31 and
257x259 produce exactly 4x dimensions, including odd source sizes. Tiny golden
tensors and a 17x19 PPM are checked in; larger tensors/weights are not. Full reference
hashes/extrema/geometry and measured CPU memory/time are in
[reference.json](evidence/ai-upscale-20261022/reference.json).

Tiling uses a derived 34-source-pixel halo (34 padded 3x3 convolutions, receptive
field 69). Halos clip at actual image edges; each model layer retains zero padding.
On 257x259 -> 1028x1036, 128 interiors use 9 tiles and 256 interiors use 4:

| Interior | Max absolute error | MAE | Forward / reverse time (CPU, seconds) |
|---|---:|---:|---:|
| 128 | 7.75e-7 | 1.77e-9 | 4.647 / 2.785 |
| 256 | 2.57e-7 | 2.88e-12 | 1.528 / 3.652 |

Both meet max <=0.01 / MAE <=0.001. Reverse-order difference is zero; edge/seam/interior
distributions and direct cross-tile-size error are recorded separately. These are
single diagnostic runs under concurrent build load, not a benchmark. Whole CPU
reference/export process peak working set is 432,922,624 bytes, not GPU VRAM or per-tile
activation/workspace usage. No GPU tile size, large-video envelope, temporal-quality
or performance advantage follows from this CPU reference gate.

### Runtime boundaries

Native DirectML: `session.disable_cpu_ep_fallback=1`, sequential execution, memory
pattern disabled and exact DXGI device_id 1. ORT lists CPU as an available/registered
provider, but profile auditing finds **only** DirectML node execution. Across the
five sizes, maximum absolute error is 3.4571e-6 and largest MAE 3.9699e-7. The 257x259
case explicitly stages 798,756 input bytes and 12,780,096 output bytes; these are host
tensor payloads, not measured PCIe traffic or opaque allocator totals. Session owners
are released and the diagnostic process exits; driver reclamation is unmeasured.
See [native-ort.json](evidence/ai-upscale-20261022/native-ort.json). Real export,
preview, tiled GPU parity, cancellation and total-memory calibration remain pending.

Browser: Playwright 1.56.1 downloads Chromium 141.0.7390.37 into ignored test storage.
Default headless shell finds no suitable adapter. Full Chromium with
`--enable-unsafe-webgpu --use-angle=d3d11` finds an adapter but fails processing-device
creation in Dawn `EnsureDXCLibraries`, `DynamicLib.Open: dxil.dll Windows Error: 87`.
The additional diagnostic `--disable-dawn-features=use_dxc` reaches the existing
processing device without changing the app's settings or installing DXC. Its limits
include 128 MiB storage-buffer binding, 256 MiB buffer size and 8 storage buffers/stage.
Current generic documentation's `env.webgpu.device` setting alone did not preserve
identity in this exact 1.23 build; inspecting its installed source found custom device
support in EP options. With `{name:'webgpu', device}`, identity passes and the exact
allowlisted graph imports. First GPU-buffer 1x1 execution then throws a numeric WASM
exception (e.g. 19351728 in the verbose run) in both contexts, with no output.
Repeating with 16-byte-aligned input allocation and an explicit GPU validation error
scope produces the same failed boundary; the scope reports no GPU validation error.
The numeric value is recorded as returned, not interpreted as a stable error code
or a proven operator defect. Session release executes. No CPU-input/model/GPU fallback
or tolerance relaxation was added to obtain a pass.

Burn: importer build and actual graph generation pass using locked Burn ONNX 0.22.0;
model.rs SHA-256 `a66c8cafde296ff71fc8592e8b74e273ee815f6eaa28c171b2700d76ec021b83`,
model.bpk (4,861,120 bytes) SHA-256
`0e5f20302257c42a8082a4b6bc067e514d38073e22659d6dbd6fd6f347a97784`.
The generated nearest Resize and CRD pixel-shuffle need independent runtime reference
checks; source-code generation is not acceptance. Its locked CubeCL 0.11.2 / wgpu
30.0.1 is ABI-compatible with processing, not Blitz 26.0.1. Do not infer that an
existing-device initialization API proves tensor IO or lifecycle on the app's device.

Actual native Burn/Vulkan runner builds and selects NVIDIA RTX 3050 (vendor 0x10de,
device 0x25a2, driver 621.01). FloatDType::F32 is configured explicitly before model
or tensor creation. Weights load in approximately 0.009 seconds; the first 1x1 model
forward panics before output with `Expected type cube.f32 to implement dyn
MaybeVectorizedType`. The backtrace locates CubeCL IR type vector-size lookup through
`copy_kernel`, `into_contiguous_aligned`, and `conv_direct` into the first Conv2d.
This is not a demonstrated Resize/pixel-shuffle defect, numeric mismatch or OOM.
A second run with backtrace reproduces it. No other reference sizes execute; the
probe exits 1 after unwinding. Driver/runtime-cache reclamation and resource counters
remain unmeasured. See [native-burn.json](evidence/ai-upscale-20261022/native-burn.json)
and [backtrace](evidence/ai-upscale-20261022/native-burn-backtrace.txt). No precision,
dependency, shader or CPU fallback was substituted to force a pass. Browser Burn
execution is still untested, not marked failed solely from this native result.

RTX: the supplied SDK README requires Windows 10+, driver R550.50+ (R570.20+ for
CUDA), VS2019+/C++ MFC and ATL; CUDA/Vulkan samples additionally call for CUDA 12.8+
and Vulkan SDK. The shipped DX11/DX12 projects actually select v143. Registered
VS Enterprise 2026 has no `VC.Tools.x86.x64` component according to vswhere, and its
VC/Tools/MSVC directory is absent. `nvcc` is absent from PATH and the default CUDA
v13.0 root contains only integration extras; a directory/version label alone does
not establish a usable toolkit. `VULKAN_SDK` is unset, not proof of every possible
installation's absence. No package executable is supplied, so no SDK run occurs.

The package API has create/evaluate/shutdown for DX11, DX12, CUDA and Vulkan. DX11
accepts a caller-selected D3D device and ARGB/ABGR 8-bit input for VSR; CUDA also
offers host-pointer and device-pointer staging; Vulkan requires explicit resources.
The simplified wrapper returns a boolean, losing precise NGX errors; a future helper
must retain the underlying failure boundary/status, not label every failure an
unsupported GPU. Driver 621.01 clears the README's numeric minimum but proves no
SDK feature initialization, 2x/4x capability or enhanced output. Zero known-input
SDK outputs have been produced and cleanup has not been exercised.

The installed February-2024 RTX license and supplement were inspected: use constitutes
acceptance; distribution has separate object-code/application/notice conditions;
sample-source derivatives require attribution and developer tools are for internal
use. Commercial-release notification clauses for NGX/DLSS require separate deployment
review. No automatic acceptance or redistribution review completion is claimed.
File hashes and tool inventory are in
[prerequisites.json](evidence/ai-upscale-20261022/prerequisites.json); proprietary
headers, libraries, DLLs and the license PDF were not copied into Git.

No portable browser pilot has passed. Task groups 2–8 do not proceed by assuming
availability, silently changing runtime/model/precision, or treating these diagnostics
as implemented features. The remaining group-1 gates and requested-platform scope
need review before product integration. No ordinary video conversion was run by
these probes. Group 1 tasks 1.1, 1.2 (negative assessment) and 1.6 are complete;
29/32 tasks remain open, including browser Burn execution, native Burn's failing
reference gate, RTX runtime evaluation and calibrated production memory envelopes.

### Verification of this feasibility snapshot

- `preflight.ps1 -SdkPath <explicit path>` ran successfully; its interpreter module
  inventory predates the separately authorized isolated installs. No SDK DLL was loaded.
- `reference.py --assets tmp/ai-upscale --output tmp/ai-upscale/reference-final --tiles`
  passed all dimensions, FP32 tile/seam/edge/order/cross-size thresholds and ONNX
  checker/hash gates. Re-export reproduced the exact graph identity. Deliberately
  corrupting one weights byte was rejected before runtime imports/output creation.
- `native_ort.py --reference tmp/ai-upscale/reference-final --output tmp/ai-upscale/native-ort-final --adapter-name <exact NVIDIA name>`
  passed five independent-reference comparisons and the no-CPU-node profile audit.
- `browser.mjs` ran actual main/worker probes, both without and with the explicit FXC
  diagnostic flag. Results **fail** as detailed above; asset import and device identity
  are partial successes, not a numerical pass. Full local logs remain ignored.
- Locked release `burn-import` and `burn-native` builds passed; actual ONNX import
  passed. The native Vulkan execution and its backtrace repeat **failed** at the first
  Conv2d kernel and exited 1. Import build emitted the pre-existing GNU linker warning
  `corrupt .drectve at end of def file`; it did not prevent generation.
- `cargo check --locked -p media-core -p media-gpu` and
  `cargo check --locked --target wasm32-unknown-unknown -p media-web` passed.
- Matching host/wasm Clippy for those packages with `-D warnings` passed. Both isolated
  probe crates passed `cargo clippy --locked --manifest-path <probe>/Cargo.toml --all-targets -- -D warnings -A linker_messages`.
  The explicit allowance covers the known GNU linker diagnostic, not Rust lints.
  Native-probe Clippy first caught `chunks_exact_to_as_chunks`; the parser now uses
  typed chunks and rejects an incomplete FP32 element, and final Clippy passes.
- Workspace/probe formatting, Python compilation, JS syntax, JSON/committed-fixture
  hash validation, `git diff --check`, and `openspec validate ai-upscale --strict --json`
  passed. No app Rust/JS processing source or root dependency lockfile changed.

## Browser FFmpeg WASM source-preview readiness correction — 2026-10-09

Reproduced in the in-app Chromium browser on Intel gen-12lp: selecting the CC0 VFR fixture with `?backend=ffmpeg-wasm` / Original FPS returned valid source metadata and geometry but rejected the timestamp-less FFmpeg export profile. The UI incorrectly gated preview on that export readiness, leaving the panel hidden, its status at Loading and Play disabled.

Source preview now has its own readiness signal, invalidated on source/probe changes and enabled only after successful inspection/selected-geometry resolution. Paused/playback commands and Play use that signal; Convert/profile controls retain the independent export gate. Preview continues to use the existing WebCodecs/shared-wgpu service regardless of output backend. This does not add FFmpeg decoding to preview, bypass export restrictions or claim that FFmpeg can preserve Original VFR.

Verified through `tests/browser-ffmpeg-preview-ui.mjs` on the actual UI in both worker and explicit main-thread execution: rejected Original VFR export still permits visible paused preview, CLAHE preview, playback and pause; selecting 30 FPS enables export; returning to Original rejects export without removing preview. Worker capture additionally verified nonblack decoded fixture pixels using one test-only output readback. Evidence: `tmp/ffmpeg-preview-ui-worker/evidence.json` (six cases) and `tmp/ffmpeg-preview-ui-main/evidence.json` (five cases). The first runner attempt waited for a Playing label that ordinary preview does not emit and timed out; the corrected assertion verifies actual transport state and advancing position instead. All final cases passed. Wasm check/Clippy with `-D warnings`, formatting, JS syntax and diff checks passed. No FFmpeg conversion was run in this UI regression; FFmpeg CLAHE export acceptance, Firefox (skipped), other input/decoder failures and broader browser coverage are not implied.

## Native CLAHE missing-inspection / Pause correction — 2026-10-09

Computer Use reproduced the reported failure in an ordinary native launch: CLAHE was On with no inspected source metadata, the presenter deliberately hid the unresolved image, and audio/playback continued. Clicking Pause sought to zero/restarted because both mouse and keyboard toggles treated every preview error as a restart request. Inspecting the user's 1920×1080 H.264/AAC, 8,582-frame source restored the CLAHE image. This was missing selected geometry, not evidence of a black equalization shader result.

The enable command now enforces inspection/busy availability independently of the DOM disabled attribute. A real click on the disabled-looking native button still dispatched its handler; the corrected handler retained Off and the ordinary preview, and displayed the inspection requirement. Missing geometry now pauses the playback/audio clock. Both toggle paths only restart ended/failed playback when already paused, never when the requested action is Pause. A regression verifies error-state action selection, preservation of the requested position, automatic pause and error clearing when equalization is removed. Native ordinary tests passed (15; four GPU diagnostics not rerun), build, all-targets Clippy with `-D warnings`, formatting and diff checks passed. The pre-existing GNU linker warning remains. Browser code and the equalization shader were not changed.

Rebuilt normal-launch Computer Use validation used the same user-selected source, Original 1920×1080 geometry and Strength 50. Inspection enabled CLAHE; an adjusted image displayed at approximately 1:29, playback advanced with no preview error, and Pause retained the image at approximately 1:45 instead of restarting at zero. This is visual/control evidence, not a numerical image comparison, performance or physical audio-sync claim. The rebuilt application was left open paused for user testing; no export of the user's video was started.

## CLAHE controls, sampled playback and active renderer recovery — 2026-10-09

This continuation supersedes the earlier controls-hidden/paused-only statements for the supported, scoped paths. Both applications now expose Off/On and Strength in their collapsed Color section. Reset disables CLAHE/restores Strength 50 and neutral sliders; Before bypasses both operations without changing conversion settings. Sliders remain independently editable. Native requires source inspection to resolve selected geometry before ordinary CLAHE preview; unresolved geometry hides the adjusted image with an explanation instead of substituting an unadjusted preview. Native conversion snapshots the saved policy and explicitly rejects direct CPU/NVIDIA FFmpeg and Main 10/shared restrictions; no route fallback is added.

Playback implementation: each displayed CLAHE sample rebuilds the exact bounded preceding 100 ms history on the existing renderer/processing device. It includes source frames skipped by the preview clock, then displays the selected frame with the saved sliders. Only one current original, finite GPU maps, one replaceable pending request and one completed result are retained; no raw pixel-history window is added. Native advancing playback positions do not cancel every active sample (which would starve output); explicit seek/source/geometry/settings changes still invalidate publication. Paused edits reuse successful statistics. This correctness-first sampling adds decode/upload work and can lag the separate audio clock; it is not a throughput or physical A/V-sync claim. Off/zero keeps the existing ordinary preview path.

Verified:

- Native Computer Use observed active renderer-device destruction after successful CLAHE preroll. The first attempt exposed a pinned Vello/wgpu validation panic in `Texture::create_view`. The corrected event-loop boundary contains an unwind **only with confirmed loss on the affected renderer**, suspends its resources and stops drawing/resizing that device; other panics propagate. The window survived with `GPU renderer lost · F5 restarts (resets controls) · Close exits`. F5 explicitly created a fresh Dioxus document, renderer and private device pool; the fixture preview rendered again. No failed conversion is automatically resubmitted. Main-window restart cancels active work and resets UI selections; this is not transparent checkpoint recovery. Lost contexts have one callback owned by their one presenter, shared by its workers; suspension disables stale-generation loss reporting. No dependency/vendor patch or cross-device texture import was added.
- Native CLAHE playback reached EOF displaying the fixture, without a preview error. The final four-slider expanded panel, actions and footer fit the normal 962×792 captured window without scrolling. Broad DPI and smaller-window coverage is not implied.
- Actual native UI conversion first rejected Direct FFmpeg with CLAHE enabled, then succeeded after explicitly selecting Shared GPU / software codecs: NVIDIA RTX 3050 / Dx12 processing, Strength 50, Original 320×180, 36 input/output frames, H.264/AAC, 84,125 bytes and 2,491 ms reported total. Independent `ffprobe -count_frames` found 36 video frames and 95 AAC frames; FFmpeg fully decoded the output without errors. Output: `tmp/clahe-ui-20261009.mp4`. The diagnostic preview selected 160×90; this UI run does not claim preview/export pixel parity at different geometries or hardware codec execution.
- All 18 native application tests, including the four explicitly enabled real-GPU diagnostics, passed on Intel UHD / Vulkan. The worker regression now includes 50 advancing sampled positions, progress without cancellation starvation, explicit invalidation and exact paused retry. Renderer/conversion pre-encoder parity still gives maximum error 0 for 30 samples/settings/geometries; separate display checks remain ≤1.
- Native direct-route rejection and controlled conversion cancel/device-loss/clean retry tests passed (`cargo test --locked -p media-native clahe -- --include-ignored --nocapture --test-threads=1`). Core/UI tests passed (27 + 1). Native and wasm application Clippy with `-D warnings`, formatting and diff checks passed. Native builds succeed with the pre-existing GNU linker warning `corrupt .drectve at end of def file`; it is not a test failure.
- Supported Browser regression `runClahePreview` passed 13 cases each in main and dedicated-worker execution, including exact sampled-playback/paused pixel identity, cache/bypass/seek/invalidation, actual preview device destruction, generation-2 retry and a full 36-frame H.264/AAC export/re-decode. Evidence: `tmp/clahe-final-main/evidence.json` and `tmp/clahe-final-worker/evidence.json`. Test-only pixel readbacks are separate from conversion.
- Actual browser controls enabled CLAHE, changed Strength 50→100, played to EOF, reset to Off and exported Original-size WebM/VP8/Opus through the UI. Full output verification passed for 36 frames with audio, 36 source-frame CLAHE recordings/144 nominal compute passes, 1,265,680 scratch bytes, zero conversion readbacks, zero live frames/samples and zero live texture leases. Main and worker playback reached the last decoded source position without a playback error. This uses Chromium/Intel gen-12lp; Firefox remains skipped as requested.

Remaining coverage, not completion claims: spontaneous driver loss/permanent queue stalls, OS-driven suspend/resume, secondary-window active loss, long/high-resolution playback stress, physical A/V sync/visible latency, broad GPUs/platforms/DPI and an FFmpeg WASM CLAHE end-to-end acceptance run. Native direct FFmpeg CLAHE and Main 10 CLAHE remain unsupported, not silently approximated. Preview is an 8-bit SDR display. The wider CLAHE flicker/scene-cut stress and M5 codec-surface-sharing roadmap are separate from this integration slice.

## Native CLAHE live-window and presenter lifecycle follow-up — 2026-10-09

Verified on Windows with Rust 1.98.1, Dioxus 0.7.10 and renderer wgpu 26.0.1 (conversion remains wgpu 30.0.1):

- `cargo build --locked -p converter-native` passed. The opt-in `--clahe-preview-check` starts the checked-in CC0 `m35-vfr-offset.mp4` paused with 160×90 selected statistics and Strength 50. It is a diagnostic, not ordinary output-size/CLAHE controls or conversion-settings wiring; normal launch is unchanged.
- Final checks passed: native ordinary tests (14 passed, 4 GPU diagnostics skipped by default), the explicitly invoked lifecycle diagnostic above, native all-targets Clippy with `-D warnings`, workspace formatting and `git diff --check`. The other GPU diagnostics' earlier recorded results are not new runs in this follow-up.
- Computer Use observed actual Blitz rendering in the 962×792 decorated main window. Before/After labels toggled; brightness 0→30 visibly updated the paused image; seeking to about 0.867 seconds replaced the timestamped fixture image while retaining the prior image during preparation. No preview error was displayed. Screenshots are visual presentation evidence, not an independent pixel comparison or latency measurement.
- The separate decorated 1920×1080 preview showed the same selected position and brightness. Escape closed only that window; the main preview retained its position and adjusted image. Closing the main window exited normally. This checks actual texture registration/presentation and a secondary-window open/close, not OS suspend/resume or prolonged multi-window/DPI stress.
- The processing preference displayed NVIDIA GeForce RTX 3050 Laptop GPU / Dx12. The live renderer adapter was not independently identified; do not equate this preference with renderer-device identity.
- `cargo test --locked -p converter-native presenter_suspend_resume_retires_history_worker -- --include-ignored --nocapture --test-threads=1` passed on Intel UHD Graphics / Vulkan. It invokes the real Presenter suspend/resume hooks, cancels/joins a worker with pending preparation, verifies the old weak control is dead, and checks texture/device/worker ownership was cleared. Destruction of the **retired test-owned** device delivered `DeviceLostReason::Destroyed`; a newly requested device/generation rebuilt three source uploads and produced byte-identical pixels. Test-only readback is excluded from conversion. This does not destroy the live app's renderer device or inject loss during active rendering.

Still untested: spontaneous/active renderer-device loss and automatic Blitz device replacement, OS-driven suspend/resume, stalled queues, larger live selected geometry, playback history and broad DPI/GPU coverage. In pinned wgpu 26, `poll` errors alone are not a device-loss notification. Do not install competing callbacks on a shared renderer or claim framework recovery from the retired-device test. CLAHE Enable/Strength controls and immutable conversion-settings UI wiring remain hidden/pending these gates; the existing three sliders are unchanged.

Date: 2026-09-17  
Status: **M1 passed in the tested Microsoft Edge environment**

## Result

The M1 application completed the required compressed VP8 → WebCodecs decode → wgpu resize → WebCodecs encode → independent WebCodecs decode round trip. Five consecutive runs processed and verified all 30 frames. A cancellation run returned application-owned live frames to zero, and its immediate restart passed all 30 frames.

The conversion path performs no explicit CPU pixel readback or pixel upload. The verifier performs one `VideoFrame.copyTo` readback per output frame; those 30 diagnostic readbacks are counted separately and are not evidence about browser-internal copies or codec hardware execution.

No M2 container, audio, file-picker, native-backend, or native-UI work was started.

## Pinned stack

| Item | Exact version / setting |
|---|---|
| Rust | 1.98.1 |
| Rust edition | 2024 |
| Dioxus | 0.7.10 (`web`, `minimal`) |
| wgpu | 30.0.1 |
| wasm-bindgen | 0.2.128 |
| wasm-bindgen-futures | 0.4.78 |
| web-sys / js-sys | 0.3.105 |
| serde | 1.0.229 |
| futures-channel | 0.3.34 |
| wasm target flag | `--cfg=web_sys_unstable_apis` for WebCodecs bindings |

`Cargo.lock` records the complete resolution. wgpu 30's released `ExternalImageSource::VideoFrame` and `Queue::copy_external_image_to_texture` APIs are used; no development-only external-texture import API is used.

## Tested environment

| Item | Observed value |
|---|---|
| Browser | Microsoft Edge 153.0.4234.32, headless Chromium (`Edg/153.0.0.0`) |
| Browser flags | `--headless=new --enable-unsafe-webgpu --ignore-gpu-blocklist --use-angle=d3d11 --disable-gpu-sandbox --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows` |
| OS | Windows API reported “Windows 10 Home”, version 2009, build 26200; browser platform `Win32` |
| WebGPU adapter info | vendor `nvidia`, architecture `ampere`; device/description not disclosed |
| Secure context | `http://127.0.0.1:8081` |
| Capability probes | `navigator.gpu`, `VideoDecoder`, `VideoEncoder`, `VideoFrame`, VP8 decoder config, VP8 encoder config, and `VideoFrame(HTMLCanvasElement)` all available |
| Hardware codec use | unknown; WebCodecs acceleration is a preference, not proof |
| Browser-internal copies | unknown |

The in-app browser-control bridge was unavailable before page attachment, so validation used the installed Edge executable and Chromium's local debugging protocol against the same built bundle. Chrome was not installed and was not required. Background-throttling flags keep the automated headless page active; they are not application prerequisites for a foreground Edge tab.

## Fixture and codec configuration

The checked-in `fixtures/m1-vp8.ivf` is 12,858 bytes with SHA-256 `dcb10a78baa7b70d270a9c6364d58195639c1776681b5d5d2de7ece04ae96b9f`. `fixtures/m1-vp8.json` contains 30 packet offsets, sizes, timestamps, durations, keyframe flags, generation details, and the hash. It was deterministically generated with FFmpeg 8.0.1/libvpx from owned filter output with changing decimal frame identifiers and fixed red/green/blue/yellow orientation patches.

- Decoder: `vp8`, coded 320×180, latency optimization enabled.
- Encoder: `vp8`, 160×90, 500,000 bit/s, 30 fps, realtime latency, hardware acceleration `no-preference`.
- Timing: 30 frames, timestamp `index × 33,333 µs`, duration 33,333 µs.
- Color prototype: opaque SDR, sRGB canvas boundary, bilinear filtering of encoded channel values. Linear-light and wide-color processing remain outside M1.

## Frame path and ownership

One cached wgpu instance/device/queue is used for the browser execution context and reused across job restarts. For each frame:

1. WebCodecs emits an application-owned `VideoFrame`.
2. Rust passes that frame to `Queue::copy_external_image_to_texture`, targeting a reusable 320×180 `Rgba8Unorm` texture with `COPY_DST | TEXTURE_BINDING | RENDER_ATTACHMENT` usage.
3. `media-gpu` records a fullscreen-triangle WGSL bilinear resize directly into the current 160×90 canvas surface texture.
4. The queue submits, then immediately presents/releases the acquired surface texture.
5. A timestamped `VideoFrame` is constructed from the persistent canvas, then both the decoded source and canvas snapshot are retained through the submission-specific completion callback.
6. The output frame is submitted to `VideoEncoder` and explicitly closed. The source frame is closed by an RAII `Drop` guard on success and every Rust error path.

Decoder submission, decoded callback, encoder submission, verifier submission, and verifier callback queues are separately bounded. Codec input pumping and callback draining run concurrently. Decoder, processor, encoder, and verifier drains are ordered; generation-based cancellation rejects stale work and resets codecs.

## Canvas ordering experiments

Four no-readback sequences were evaluated:

1. `submit → VideoFrame(canvas) → present → completion`: encoded black frames.
2. `submit → completion → VideoFrame(canvas) → present`: Edge reported an uninitialized 160×90 shared image and produced uniform output.
3. `submit → completion → present → VideoFrame(canvas)`: one full run passed, but a later run stalled at frame 10 while the acquired surface texture was held through the completion wait.
4. `submit → present → VideoFrame(canvas) → completion`: five consecutive runs plus cancel/restart passed. This is the implemented order.

The current sequence gives Chromium a consumer for the committed canvas image without an unrelated asynchronous yield, while retaining both relevant frame handles until submitted GPU work completes. A wgpu validation error scope covers every frame submission. No validation error occurred in the passing runs.

The original reported marker failure (`TL=0,77,0 TR=0,77,0 BL=0,77,0`) also exposed two correctness requirements now implemented: the external-copy destination texture includes `RENDER_ATTACHMENT`, and verifier samples honor the `offset` and `stride` returned by `VideoFrame.copyTo` instead of assuming tightly packed rows.

## Measured runtime evidence

Five consecutive runs in one page/execution context:

| Run | Result | Elapsed including verification | Live-frame cleanup |
|---:|---|---:|---:|
| 1 | 30/30 passed | 1,083.8 ms | 0 |
| 2 | 30/30 passed | 561.4 ms | 0 |
| 3 | 30/30 passed | 755.9 ms | 0 |
| 4 | 30/30 passed | 210.1 ms | 0 |
| 5 | 30/30 passed | 369.2 ms | 0 |

Across those runs:

- output geometry: 160×90
- encoded and independently decoded frames: 30/30 per run
- timestamp tolerance: passed at ±1 µs; durations passed when supplied by the decoder
- orientation and representative red/green/blue marker checks: passed for every frame
- explicit external-image API copies: 30 per run
- explicit CPU pixel readbacks/uploads in conversion: 0
- test-only verifier readbacks: 30 per run
- peak application-owned live frames: 8
- peak decoder submission queue: 4
- peak decoded callback queue: 6
- peak encoder submission queue: 1 (hard bound 4)
- peak verifier submission queue: 4
- peak verifier callback queue: 6
- WebGPU validation errors: 0 observed

Elapsed time is a correctness baseline, not a throughput claim. It includes verification readback and independent decoding and is sensitive to browser warm-up and headless scheduling.

## Lifecycle validation

- Cancellation was requested after one frame had been processed. The result reported `processed=1`, `encoded=0`, bounded peaks (`live=6`, decoder=4, callbacks=5, encoder=1), and `cleanup live frames=0`.
- The immediate restart passed 30/30 frames in 367.5 ms with `cleanup live frames=0`.
- The five normal runs and cancel/restart shared one cached processing device/queue. No queue growth was observed between runs.
- Cancellation is cooperative at the bounded scheduling and callback-drain points. Already-submitted GPU work is retained safely through its completion callback rather than forcibly interrupted.

## Build verification

Verified on Rust 1.98.1 using the GNU Windows host toolchain because the installed Visual Studio instance lacks the C++ linker/Windows SDK library workload:

- `cargo check -p media-core -p media-gpu`: passed
- `cargo test -p media-core -p media-gpu`: passed (2 focused timestamp tests)
- `cargo check --workspace --target wasm32-unknown-unknown`: passed
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed
- `cargo fmt --all -- --check`: passed
- `dx build --web --locked` with Dioxus CLI 0.7.10: passed from `apps/web`

The native MSVC-host check was attempted but could not link because `link.exe` and `msvcrt.lib` were absent from the installed Visual Studio workloads. The GNU-host checks provide the requested host portability evidence without changing application behavior.

## Decision and remaining limitations

The experiment answers M1's architecture question **yes** for the pinned stack and tested Edge environment: compressed frames can be decoded, copied through the released wgpu VideoFrame API, resized by shared WGSL, captured from the WebGPU canvas, encoded, and independently verified without application-managed CPU pixel transfer.

This result does not prove zero internal copies, hardware codec execution, support in non-Chromium browsers, wide-color correctness, worker operation, long-run memory stability, or native interoperability. Those remained explicitly untested and outside M1 when this M1 section was recorded.

---

# M2 browser converter interoperability report

Date: 2026-09-18
Status: **M2 passed for the tested MP4/H.264 → video-only WebM/VP8 path in Microsoft Edge**

## Verified behavior

The Dioxus application accepted the checked-in `fixtures/m2-h264-aac.mp4`, demuxed its MP4 container, decoded all 60 H.264 frames in presentation order, resized every frame from 640×360 to 320×180 through the shared wgpu shader, encoded VP8, finalized a downloadable WebM, and explicitly omitted its AAC track. The finalized file was 62,512 bytes.

The application reopened the finalized WebM, verified VP8 codec, 320×180 geometry, 60 packets for 60 decoded input frames, two-second duration within one-frame precision, and a successful midpoint decode. Edge's independent HTML video element then loaded the object URL, reported 320×180 and 2.000 seconds, and completed a seek to 1.000 second. Finally, FFmpeg 8.0.1/ffprobe independently reported one VP8 video stream, no audio stream, 60 decoded frames, 30 fps, 320×180, and 2.000 seconds; `ffmpeg -ss 1` decoded a frame successfully.

Cancellation during conversion reported `CANCELLED: conversion stopped by user`. An immediate restart processed all 60 frames and produced the verified output. The M1 deterministic probe was then run in the same page and on the same cached wgpu device after dynamic reconfiguration; it again passed 30/30 frames with zero application-owned live frames after cleanup.

An additional ignored test clip derived from the user-provided recording also passed: 60 H.264 frames at 640×360 with one AAC track became a 20,488-byte, 320×180 video-only WebM with 60 frames and a 2.000-second player/ffprobe duration. The original 376,836,980-byte recording is intentionally rejected by the 256 MiB M2 limit and was not modified.

## Selected path and versions

| Item | Exact value |
|---|---|
| Input | ISO MP4, primary H.264/AVC video track, unrotated/unflipped |
| Output | WebM, VP8 video only, two-second keyframe interval |
| Resize | 50% width and height, rounded down to even codec dimensions |
| Container implementation | Mediabunny 1.58.0, pinned by `package-lock.json` |
| Browser input cache | Mediabunny `BlobSource`, 8 MiB maximum cache |
| Application input limit | 256 MiB |
| Output target | `BufferTarget`; complete compressed output retained in memory until download |
| Rust/Dioxus/wgpu | Rust 1.98.1, edition 2024, Dioxus 0.7.10, wgpu 30.0.1 |

Mediabunny supplies the proven MP4 demux/WebM mux integration, exact decoder initialization metadata, decode-order handling for H.264 B-frames, keyframe scheduling, and output finalization. Its integer microsecond sample accessors are used at the WebCodecs boundary. The first input presentation timestamp is normalized to zero; positive sample durations are preserved, and only missing/zero durations use the measured average packet interval. WebM metadata duration and decoded sample coverage must agree within one frame.

The frame path is sequentially backpressured: one decoded `VideoSample` is converted to a `VideoFrame`, consumed and closed by Rust/wgpu, wrapped as one output sample, and awaited by the encoder/mux source before the next sample is accepted. Mediabunny may predecode a small internal window; its exact internal high-water mark was not exposed. No raw frame or GPU resource enters Dioxus reactive state.

## Fixture provenance

`fixtures/m2-h264-aac.mp4` is 279,635 bytes with SHA-256 `5a7802b764ad928056d2777e2d25a86ec7fdb2c69696d70258b3ae5702fcb54b`. FFmpeg 8.0.1 generated its 60-frame H.264 video from `testsrc2` and its AAC mono audio from an 880 Hz `sine` source. It contains no third-party media and is CC0-1.0. `fixtures/m2-h264-aac.json` records the complete command and stream configuration.

## Tested environment and measured run

| Item | Observed value |
|---|---|
| Browser | Microsoft Edge 153.0.4234.32, headless Chromium (`Edg/153.0.0.0`) |
| OS | Windows browser platform `Win32` / Windows 10-compatible user agent |
| WebGPU adapter shown by app | `intel / gen-12lp (BrowserWebGpu)` |
| Local origin | `http://127.0.0.1:8080` |
| Fixture conversion time | 390.2 ms for the recorded fixture run; correctness baseline only |
| Output | 62,512 bytes, 60 frames, 320×180, 2.000 seconds |
| Output streams | one VP8 video stream; zero audio streams |
| Cancellation/restart | passed |
| Midpoint HTML video seek | passed at 1.000 second |
| Independent ffprobe/frame decode | passed |
| M1 regression after M2 | passed 30/30; cleanup live frames=0 |

The conversion path adds no `VideoFrame.copyTo`, canvas `getImageData`, mapped GPU readback, or CPU pixel upload. There is one explicit WebGPU external-image copy per converted frame. This does not establish zero browser-internal copies or hardware codec execution.

## Build verification

Verified with the Rust 1.98.1 GNU Windows host toolchain:

- `cargo test -p media-core -p media-gpu`: passed; four focused `media-core` tests, including even half-size geometry and the browser input-size policy.
- `cargo check --workspace --target wasm32-unknown-unknown`: passed.
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `dx build --web --locked`: passed with Dioxus CLI 0.7.10 and the bundled Mediabunny asset.

## Explicitly untested or unsupported

- Input other than MP4/H.264 and output other than WebM/VP8 are rejected or not offered. A capability-driven output-profile selector, including additional MP4/WebM profiles, is now an explicit M3 deliverable.
- Audio is not copied or transcoded; it is detected, reported, and omitted.
- Files larger than 256 MiB are rejected because compressed output is still buffered in memory. Streaming I/O is not implemented.
- Rotation and horizontal-flip metadata are rejected. HDR, crop, non-square pixels, resolution changes, and defined wide-color processing remain untested.
- Firefox, Safari, non-Windows Chromium, worker WebGPU, long-run memory behavior, and device-loss recovery remain untested.
- Browser-internal copies and actual codec hardware acceleration remain unknown.
- The native backend and exact native GPU selection remain M4/M5 work. At the time this M2 result was recorded, no M3 work had started; the following section supersedes that milestone-status statement.

---

# M3.1 output-profile and audio interoperability report

Date: 2026-09-18
Status: **M3.1 functional acceptance passed in Microsoft Edge; M3.2 was not started**

## Verified behavior

The preferred profile converted the deterministic `fixtures/m2-h264-aac.mp4` into a 97,168-byte WebM containing all 60 resized VP8 frames and an Opus track. The browser pipeline decoded 95 AAC audio samples, encoded 102 Opus packets, and finalized 2.040 seconds of output. Its independent reopen check verified 320×180 VP8, mono Opus at 48 kHz, midpoint video seeking, audio decoding near the beginning/middle/end, and non-silent decoded audio with peak amplitude 0.0424. One shared integer-microsecond origin preserved the input tracks' relative start offsets; audio and video were pumped concurrently and finalized together.

Edge's HTML media element loaded the result, reported 320×180 and 2.040 seconds, and completed a seek to 1.020 seconds. FFprobe 8.0.1 independently reported one VP8 stream and one mono 48 kHz Opus stream. FFmpeg decoded 97,608 audio samples and measured mean volume -21.1 dB and peak volume -17.6 dB. A seeked decode at 1.5 seconds succeeded for both streams.

The explicit video-only profile still produced 60 VP8 frames, no audio stream, 320×180 geometry, and 2.000 seconds of seekable output. Cancellation during each profile returned `CANCELLED: conversion stopped by user`; immediate restart passed. The M1 deterministic probe passed 30/30 after both profile runs with zero application-owned live frames after cleanup.

The ignored user test file `tmp/user-test/Input.mp4` also passed without modification. Its 3,530 H.264 frames and 6,343 decoded 44.1 kHz stereo AAC samples became a 32,138,440-byte, 960×540 VP8/48 kHz stereo Opus WebM in 29.066 seconds. The output contained 7,365 Opus packets, lasted 147.300 seconds, sought to 73.650 seconds in Edge, and produced non-silent decoded audio at all three verification points (peak 0.2075). FFprobe reported both streams starting at 0.000 seconds; FFmpeg decoded 14,140,176 stereo samples with mean volume -9.2 dB and peak 0.0 dB.

## Contracts, policy, and lifecycle

- `media-core` now defines reusable output-profile IDs, explicit audio policies, the Opus codec choice, and capability results that retain exact unsupported reasons. The preferred profile preserves audio; the video-only profile is always an explicit user choice rather than a silent fallback.
- `media-web` invokes one exercised, non-`Send` local `BrowserConversionBackend` implementation. At the JavaScript boundary, `WebCodecsMediabunnyBackend` composes a concrete MP4 input adapter, WebM output adapter, and the job orchestrator. No codec frames or GPU resources enter Dioxus state.
- The Opus profile probes both the exact VP8 output geometry/frame rate and the exact Opus channel count/48 kHz encoder configuration. Missing audio, undecodable input audio, and unsupported VP8 or Opus configurations fail with stage-specific reasons; audio failures point to the video-only profile.
- Audio resampling to 48 kHz occurs inside the concrete Mediabunny/WebCodecs adapter. Each audio sample is timestamped from the same origin as video, awaited through encoder/mux backpressure, and closed in `finally`. The application-owned audio submission high-water mark is one; Mediabunny's internal decoder prefetch depth is not exposed and remains unknown.
- Video and audio pumps stop as siblings on cancellation or failure. End-of-stream waits for both pumps, then finalizes the muxer. A failed/cancelled muxer is cancelled, and the input is disposed on every path.
- Conversion still performs zero explicit CPU pixel readbacks. The three-point audio amplitude read is a post-finalization verification operation and is excluded from conversion timing/copy claims.

## Tested environment and versions

| Item | Observed value |
|---|---|
| Browser | Microsoft Edge 153.0.4234.32, headless Chromium (`Edg/153.0.0.0`) |
| Browser control | Edge local debugging protocol; the in-app browser native bridge was unavailable |
| OS | Windows Chromium platform `Win32` |
| WebGPU adapter shown by app | `intel / gen-12lp (BrowserWebGpu)` |
| Application origin | `http://127.0.0.1:8080` |
| Rust / edition | Rust 1.98.1 / edition 2024 |
| Dioxus / wgpu / Mediabunny | 0.7.10 / 30.0.1 / 1.58.0 |
| Codec acceleration preference | `no-preference`; the M3.4 comparison has not started |

## Build verification

Verified with the Rust 1.98.1 GNU Windows host toolchain:

- `cargo test -p media-core -p media-gpu`: passed; six `media-core` tests, including preferred-profile/audio policy and exact capability-reason coverage.
- `cargo check --workspace --target wasm32-unknown-unknown`: passed.
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `dx build --web --locked`: passed with Dioxus CLI 0.7.10.

## Known findings and explicitly untested behavior

- FFmpeg 8.0.1 emits one `Error parsing Opus packet header` diagnostic when opening each Mediabunny-produced Opus WebM, but then decodes the complete audio track and reports the expected duration and non-silent signal. Edge and Mediabunny decode/seek checks pass. This remains a known cross-implementation compatibility limitation; it is not being presented as warning-free FFmpeg interoperability.
- Automated checks establish the presence of a decodable, non-silent signal and HTML media-element loading/seeking. Human acoustic listening and speaker output were not exercised in the headless browser.
- The application bounds its own audio submissions at one. Mediabunny's internal callback/prefetch high-water marks are not observable, so full audio queue telemetry remains M3.4.
- Only primary audio is preserved. Multiple audio tracks, language/disposition metadata, audio edits, channel layouts beyond the tested mono/stereo inputs, and passthrough were not tested.
- Firefox, Safari, non-Windows Chromium, worker execution, hardware-codec proof, long-run memory profiling, device loss, HDR, rotation/flip, and streaming output remain untested or scheduled for later M3 slices.
- The VS Code embedded browser is an Electron webview rather than a standalone browser. It has been reported to reject an otherwise working preserve-audio codec configuration; the app now identifies this environment and reports the failing pipeline stage with instructions to open the local URL in a standalone browser with WebCodecs and WebGPU support.
- At the time M3.1 was recorded, MP4 output had not been added. The following M3.2 section supersedes that milestone-status statement; arbitrary codec/container mixing remains intentionally unsupported.

---

# M3.2 output-container interoperability report

Date: 2026-09-18
Status: **M3.2 passed for capability-gated MP4/H.264/AAC in the tested browser; M3.3 was not started**

## Verified behavior

Selecting the deterministic `fixtures/m2-h264-aac.mp4` enabled the MP4 profile only after exact H.264 and AAC encoder probes passed for the computed 320×180 output, measured frame rate, 48 kHz sample rate, and source channel count. Conversion produced a 157,659-byte fast-start MP4 containing all 60 H.264 frames and 95 AAC packets. The application reopened the result, verified H.264/AAC codecs, geometry, packet counts, timing, midpoint video seek, and non-silent audio near the beginning, midpoint, and end. Edge's HTML media element reported 320×180, 2.026667 seconds, and completed a seek to 1.013333 seconds.

FFprobe 8.0.1 independently reported H.264 High profile (`avc1`), 320×180 `yuv420p`, exactly 60 frames, AAC-LC mono at 48 kHz, exactly 95 packets, and 2.026667 seconds of container duration. FFmpeg decoded 97,280 audio samples with mean volume -21.1 dB and peak -17.6 dB. Independent beginning/mid/end seeked decodes completed without errors.

The ignored `tmp/user-test/Input.mp4` exercised the longer stereo/resampling path. Its 3,530 H.264 frames and 6,343 decoded 44.1 kHz stereo AAC samples became a 75,590,653-byte, 960×540 MP4 in 26.645 seconds. FFprobe reported all 3,530 H.264 High-profile frames, 6,904 AAC-LC packets at 48 kHz stereo, both streams starting at 0.000 seconds, video duration 147.230417 seconds, audio/container duration 147.285333 seconds, and no dropped video frames. FFmpeg decoded 14,139,392 non-silent stereo samples.

Both existing WebM profiles were rerun through the generalized adapter. WebM/VP8/Opus produced all 60 frames plus 102 Opus packets and 2.040 seconds; video-only WebM produced all 60 frames, no audio, and 2.000 seconds. Each profile passed cancellation/restart and the M1 30/30 regression with zero application-owned live frames after cleanup.

## Capability and UI behavior

- `media-core` binds each profile to a container, video codec, and audio policy: WebM/VP8/Opus, WebM/VP8/video-only, or MP4/H.264/AAC. The UI does not expose arbitrary invalid codec/container combinations.
- File selection invokes the concrete browser backend's exact capability probes. MP4 remains disabled while unprobed or unsupported, and displays the returned reason. A video-only MP4 input was tested: MP4 stayed disabled with `MP4/H.264/AAC requires an input audio track.`
- Conversion repeats the exact probe before allocating the output, so stale UI capability state cannot start an unsupported profile. There is no silent substitution to WebM or another codec.
- MP4 uses Mediabunny's in-memory fast-start mode. The existing 256 MiB input limit and complete in-memory compressed output remain explicit until M3.6.

## Tested environment and measurements

| Item | Observed value |
|---|---|
| Browser | Microsoft Edge 153.0.4234.32, headless Chromium (`Edg/153.0.0.0`) |
| Browser control | Local Chromium debugging protocol; in-app bridge unavailable |
| OS / WebGPU adapter | Windows `Win32`; `intel / gen-12lp (BrowserWebGpu)` |
| Local origin | `http://127.0.0.1:8083` |
| Deterministic MP4 elapsed | 1,323.8 ms including final verification |
| Long stereo MP4 elapsed | 26,644.7 ms including final verification |
| Codec acceleration preference | `no-preference`; actual hardware codec execution remains unknown |
| Versions | Rust 1.98.1, edition 2024, Dioxus 0.7.10, wgpu 30.0.1, Mediabunny 1.58.0 |

## Build verification

- `cargo test -p media-core -p media-gpu`: passed; six focused `media-core` tests, now including the MP4/H.264/AAC profile binding.
- `cargo check --workspace --target wasm32-unknown-unknown`: passed.
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `dx build --web --locked`: passed with Dioxus CLI 0.7.10.

## Explicitly untested or deferred

- H.264/AAC encode support is configuration- and browser-dependent. Only profiles whose exact probes pass are enabled; this report does not claim universal browser support.
- Automated signal checks establish decodable non-silent audio, but human acoustic listening was not performed in the headless run.
- Multiple audio tracks, language/disposition metadata, input codecs other than H.264/AAC, HDR, orientation/flip, and streaming output remain unsupported or untested.
- Worker execution, main-thread responsiveness measurements, compatibility fallback behavior, and worker WebGPU/OffscreenCanvas probes are M3.3. No M3.3 implementation was started.

## Firefox ingress correction — 2026-09-19

Verified with installed Firefox 156.0 (build 20260909172920), Windows, a separate automation profile and WebDriver BiDi. Headless Firefox exposed navigator.gpu but returned null for the WebGPU canvas context; the successful run used normal rendering mode with no WebGPU preference overrides.

Firefox rejected VideoFrame as the source of copyExternalImageToTexture. The old wgpu wrapper could panic on that JavaScript exception, leaving the processing future and UI waiting indefinitely, including after Cancel. The frame ingress now uses the released wgpu 30.0.1 queue/texture `as_webgpu` handles to catch the exception at the browser boundary on the same device. A TypeError triggers createImageBitmap(frame) followed by the same texture copy. Other errors propagate. The bitmap is retained with the decoded frame until GPU completion and closed by a Rust Drop guard. Shader processing and immediate render/present/capture ordering remain unchanged. There is no application pixel readback; each fallback frame adds one browser-managed bitmap conversion, explicitly reported in job results. Its internal copies and hardware execution are unknown.

The deterministic H.264/AAC input completed with 60/60 VP8 frames, 95 decoded audio samples, 102 Opus packets, 320×180 geometry, 2.034 seconds, and 102,243 bytes. The recorded restart took 6,183 ms including verification; it used 60 bitmap compatibility conversions. Cancellation returned CANCELLED, and immediate restart succeeded. The HTML video element loaded the output and sought to 1.0169995 seconds. M1 passed all 30 frame timestamp, orientation, and RGB-marker checks, with zero application-owned live frames at cleanup. FFprobe independently decoded 60 VP8 frames and 102 audio frames at 48 kHz mono; the previously documented Opus packet-header warning remains.

MP4 stayed disabled with the measured reason: AAC encoding unsupported at 48000 Hz with one channel. No codec/container substitution or browser-vendor block was added.

Rust formatting, host core/GPU tests, wasm check/Clippy and locked Dioxus web build passed. A new Chromium regression browser launch was blocked by the execution policy, so the modified direct ingress path has not been revalidated there in this correction. Longer Firefox inputs, other Firefox versions/platforms, and arbitrary driver/device-loss stalls remain untested.

---

# M3.3 dedicated worker interoperability report

Date: 2026-09-19
Initial status at the time of this M3.3 run: **Firefox worker and main-thread fallback passed every profile enabled there; Chromium/MP4 validation was pending.** The completion section below closes that gap; the later M3.4 section records subsequent work.

## Implementation

Demux, WebCodecs, the existing Rust/wgpu processor and output verification now execute in one dedicated module worker by default. The worker initializes the same wasm bundle emitted by `dx`; it never launches Dioxus. Its transferred OffscreenCanvas is also the preview surface. Window/worker commands carry File inputs and metadata; the only media returned is the finalized compressed Blob. No raw frames or GPU resources enter reactive UI state. The main window owns/revokes download URLs.

Startup probes secure context, worker video/audio codec APIs, WebGPU, OffscreenCanvas, the real wgpu device/surface, and canvas VideoFrame capture. Exact output-profile probes run in the selected context. An unsupported/failed startup or 30-second startup timeout selects the original main-thread pipeline with a visible reason. `?execution=main` explicitly requests that compatibility path. There is no silent mid-job fallback or codec substitution. Tagged, serialized commands support cancellation, stale-progress rejection, failed-worker teardown and retry. Rust now also waits for submitted GPU work before releasing ingress guards when canvas capture fails.

## Real-browser environment and configuration

| Item | Observed/requested value |
|---|---|
| Browser | Firefox 156.0, build 20260909172920, normal rendering mode, isolated automation profile |
| OS | Windows, browser platform version 10.0 |
| Automation | Local WebDriver BiDi; in-app browser setup failed (`failed to write kernel assets`) |
| GPU | `identity redacted by browser (BrowserWebGpu)` in both contexts; no exact adapter identification claimed |
| Origin | `http://127.0.0.1:8084/`; fallback repeated with `?execution=main` |
| Versions | Rust 1.98.1 GNU, edition 2024, Dioxus/CLI 0.7.10, wgpu 30.0.1, Mediabunny 1.58.0 |
| Input | Deterministic `fixtures/m2-h264-aac.mp4`, 640×360 H.264, 60 frames, AAC mono; provenance in fixture README |
| Conversion video settings | VP8, 320×180, Mediabunny `Quality("high")`, 2-second keyframe interval, `latencyMode: "quality"`, `hardwareAcceleration: "no-preference"` |
| Conversion audio settings | Opus, 48,000 Hz, one channel, `Quality("high")`; video-only profile has no audio track |
| M1 settings | VP8 decode 320×180, optimizeForLatency; encode 160×90, 500,000 bit/s, 30 fps, realtime, no-preference |
| MP4 capability | Disabled in both contexts: `AAC encoding is unsupported at 48000 Hz with 1 channel(s).` |

The conversion quality settings above are requested adapter settings, not independently measured encoder bitrates or proof of hardware execution.

## Verified conversion and fallback results

| Execution | Profile | Video frames / Opus packets | Bytes / duration | Elapsed including verification | Window 50 ms timer: ticks / max gap |
|---|---|---|---|---|---|
| Worker | VP8 + Opus | 60 / 102 | 102,243 / 2.034 s | 6,124 ms | 123 / 54 ms |
| Worker | VP8 video-only | 60 / none | 62,666 / 2.000 s | 6,154 ms | 123 / 66 ms |
| Main-thread fallback | VP8 + Opus | 60 / 102 | 102,243 / 2.034 s | 6,153 ms | 122 / 71 ms |
| Main-thread fallback | VP8 video-only | 60 / none | 62,666 / 2.000 s | 6,241 ms | 123 / 86 ms |

Both audio-preserving runs consumed all 95 decoded source audio samples. Final verification decoded non-silent audio near beginning/middle/end (peak 0.0574), checked track timing and dimensions, and reopened the completed container. Firefox's independent HTML video element loaded both profiles at 320×180 and sought successfully to 1.0169995 s (audio-preserving) or 1.000 s (video-only). Each profile in each context passed cancellation during conversion followed by immediate restart. Starting without a file produced an error and permitted subsequent normal conversion.

Application-held conversion frame references/samples returned to **0/0** after success and cancellation; observed peaks were **2/2**. These counters conservatively include closed frame references retained until their cleanup block; they do not measure library-internal codec resources or GPU memory. Each successful conversion reported 60 additional VideoFrame→ImageBitmap compatibility conversions, 60 ingress image copies by the processing path, and no explicit CPU pixel readback in conversion. Hidden copies remain unknown.

FFprobe 8.0.1 independently decoded/count-checked all four files: 60 VP8 frames each, exactly 102 Opus audio frames at 48 kHz mono in each audio-preserving file, and no audio stream in either video-only file. FFmpeg full audio/video decodes of both audio-preserving files completed with exit code zero. **The previously recorded `Error parsing Opus packet header` diagnostic still appears** during independent open/probing; this is not clean, warning-free FFmpeg interoperability.

The window timer and working Cancel controls demonstrate event-loop responsiveness on this small fixture, not a frame-rate guarantee or statistically established speedup. End-to-end conversion remains roughly six seconds in both modes; worker migration has not removed per-frame synchronization or the Firefox bitmap compatibility path.

## M1 regression and lifecycle checks

- Five consecutive complete worker runs and five consecutive complete main-thread runs passed 30/30 decode → resize → encode → re-decode frames, 160×90 output, timestamps within ±1 µs, orientation and RGB markers, and zero application-owned live frames after cleanup.
- Across these repeats: peak live frames 8, decoder submissions 4, decoded callback queue 6, encoder queue 0, verifier submissions 4, verifier callback queue up to 6. These are observed high-water marks, not newly introduced queue limits.
- Worker M1 elapsed range: 3,088–3,119 ms; main-thread range: 3,046–3,119 ms. These include diagnostic verification and are not conversion-only throughput measurements.
- Every full M1 run counted 30 ingress copies, 30 additional bitmap conversions, zero conversion readbacks and 30 separate test-only pixel readbacks.
- M1 cancellation in each context reported cleanup live frames=0, followed by the five passing restarts above.

## Build and automated checks

- `cargo fmt --all -- --check`: passed.
- `cargo check -p media-core -p media-gpu`: passed.
- `cargo test -p media-core -p media-gpu`: passed, six core tests; GNU linker retained its existing `corrupt .drectve at end of def file` warning for the GPU test binary.
- `cargo clippy -p media-core -p media-gpu -- -D warnings`: passed.
- `cargo check --workspace --target wasm32-unknown-unknown`: passed.
- `cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings`: passed.
- `dx serve --web --locked --addr 127.0.0.1 --port 8084` and `dx build --web --locked`: passed; no separate worker build required.
- `node --test tests/worker-transport.test.mjs`: five tests passed. These simulate command serialization/stale events, cancellation and restart, exact-reason startup fallback, runtime-crash teardown/retry without silent fallback, and explicit main-thread mode. These tests are **not** real-browser crash/device-loss evidence.
- `node tests/firefox-worker-interop.mjs worker` and `node tests/firefox-worker-interop.mjs main`: passed. Local detailed logs and media are under ignored `tmp/m33-worker` and `tmp/m33-main`; the harness and aggregate evidence here are tracked.

## Explicitly pending and limitations

- A normal isolated Chromium launch was rejected by the execution environment. Chromium's direct VideoFrame ingress, worker OffscreenCanvas, and MP4/H.264/AAC through the new transport have **not** been revalidated; older M3.2 MP4 results do not prove these changes. Safari and other platforms are also untested.
- Real browser fallback comparison used the explicit mode switch. Startup-capability failures, stale messages, and worker crashes were simulated in transport tests, not injected into a real GPU driver. Worker startup timeout behavior and deployment under restrictive CSP/non-root base paths were not runtime-tested.
- Browser identity was redacted, so neither an exact GPU nor the codec acceleration implementation is established. Hardware-preference comparisons remain M3.4.
- M1's verification and final container/audio verification are diagnostic paths, not fast-path pixel transfers. Conversion timings above include verification; no pure throughput gain is claimed.
- Long-duration worker memory/queue behavior, arbitrary codec hangs, device loss, pooled texture reuse, higher throughput, streaming, geometry/color extensions, and native/FFmpeg work remain outside M3.3. The same wasm bundle is instantiated in both contexts; this duplicates wasm instance memory and is not a minimal worker download.
- Human listening was not performed. The existing silent-source audio verifier limitation, multiple audio tracks, and the Opus header diagnostic are unchanged by worker migration.

## M3.3 completion — Chromium/MP4 validation, 2026-09-19

**Acceptance gate passed for the tested browser/profile matrix.** This section supersedes the earlier Chromium/MP4 pending status. No runtime implementation changes were necessary in this completion pass; reusable Chromium and independent output-inspection harnesses were added. The later M3.4 section records subsequent work.

The user started an isolated Edge session with local debugging port 9227, resolving the browser-launch restriction. The in-app browser bridge remained unavailable (`privileged native pipe bridge is not available; browser-client is not trusted`). Tests connected to the user-provided session, created/closed their own tabs, and did not inspect or change the everyday browser profile.

Environment: Microsoft Edge 153.0.4234.32, normal rendering mode, Windows x64, WebGPU adapter `intel / gen-12lp (BrowserWebGpu)`, app served at `http://127.0.0.1:8084/`. Dependency/toolchain versions and deterministic fixtures are unchanged. All three exact profile probes passed in the worker and main-thread contexts. The acceleration preference remains `no-preference`; adapter identity does not identify the codec engine.

### Real-browser results

Each of three modes tested **all three profiles**, cancellation during frame processing followed by immediate restart, zero application-held frame references/samples after cleanup, HTML media-element loading and midpoint seeking, M1 cancellation and **five consecutive complete M1 rounds**:

1. Dedicated worker: OffscreenCanvas and direct VideoFrame ingress.
2. Explicit compatibility path: `?execution=main`.
3. Automatic compatibility path: a test-only replacement Worker constructor throws `Injected worker startup failure for interoperability test`; the app displays that exact reason and runs the unchanged main-thread implementation. This is a real-browser transport/startup-failure test, not a GPU-driver failure or a claim that the browser lacks workers.

Final foreground-tab measurements (`document.visibilityState` recorded as `visible` for every conversion):

| Mode | Profile | Elapsed including verification | 50 ms timer ticks / max gap |
|---|---|---|---|
| Worker | WebM/VP8/Opus | 405.2 ms | 8 / 53.0 ms |
| Worker | WebM/VP8/video-only | 384.5 ms | 7 / 53.3 ms |
| Worker | MP4/H.264/AAC | 830.4 ms | 17 / 61.0 ms |
| Explicit main | WebM/VP8/Opus | 417.5 ms | 8 / 59.6 ms |
| Explicit main | WebM/VP8/video-only | 408.0 ms | 8 / 56.4 ms |
| Explicit main | MP4/H.264/AAC | 431.9 ms | 9 / 51.4 ms |
| Automatic fallback | WebM/VP8/Opus | 419.9 ms | 8 / 63.3 ms |
| Automatic fallback | WebM/VP8/video-only | 370.3 ms | 7 / 57.1 ms |
| Automatic fallback | MP4/H.264/AAC | 382.1 ms | 7 / 52.1 ms |

An earlier run without explicit tab foregrounding observed a 1,025.5 ms timer gap during the first automatic-fallback conversion; visibility was not recorded in that run, so its cause is not established. The harness now brings its own tab forward and records visibility. These small, variable measurements establish continued window event handling and working cancellation, not a statistically established speed advantage or background-tab responsiveness guarantee.

All nine final outputs retained **60/60 video frames** at 320×180. WebM/Opus used all 95 decoded source audio samples and emitted 102 Opus packets (97,164 bytes, 2.040 s); video-only WebM had no audio (62,512 bytes, 2.000 s). MP4 used all 95 decoded source audio samples and emitted 95 AAC packets (101,653 bytes, 2.026667 s). Browser audio verification near beginning/middle/end measured peaks 0.0424 for Opus and 0.1478 for AAC. HTML midpoint seeks reached 1.02 s, 1.00 s, and 1.013333 s respectively.

Application-held frame-reference/sample peaks were 2/2 in each final conversion; cleanup was 0/0 after completion and cancellation. There were **zero additional bitmap compatibility conversions**, confirming the direct VideoFrame ingress branch was exercised in Chromium. Each processed frame still uses the baseline external-image texture copy; zero explicit conversion pixel readbacks is not proof of zero internal copies.

All 15 final M1 rounds passed 30/30 decode → resize → encode → re-decode frames, 160×90 geometry, timestamps within ±1 µs, orientation and RGB markers. Each reported zero live frames after cleanup, 30 ingress copies, zero bitmap conversions, and 30 separate verification readbacks. Peak metrics were live frames 8, decoder submissions 4, decoded callbacks 6, encoder submissions 1, verifier submissions 4, verifier callbacks 5. M1 elapsed ranges including verification were worker 212.0–471.3 ms, explicit main 228.6–470.6 ms, automatic fallback 213.1–235.9 ms. M1 cancellation in each mode returned live frames=0 and restart succeeded.

### Independent inspection

`tests/inspect-chromium-outputs.mjs` used FFprobe/FFmpeg 8.0.1 to verify all nine files. Each decoded completely with exit code zero and exactly 60 video frames. The six audio-preserving outputs also decoded 50 ms audio segments at 0.1, 1.0 and 1.8 seconds with a finite, non-silent peak. Video-only files contained no audio stream.

MP4 results in all three modes: H.264 High profile (`avc1`, 38-byte codec initialization data), AAC-LC (`mp4a`, 2-byte codec initialization data), mono 48 kHz, 95 decoded audio frames. The `moov` box precedes `mdat` (offsets 28 and 1,947), verifying fast-start layout. Video starts at 0.021337 s and lasts 2.000 s; AAC starts at 0.000 s and lasts 2.026667 s. The measured 21.337 ms track-start offset is within the existing 50 ms verification tolerance; this is not a sample-exact synchronization claim. Independent AAC seek peaks were -17.8, -17.8 and -17.9 dB. MP4 probing and full decoding emitted no warnings.

WebM outputs retained VP8 and the expected Opus/no-audio tracks; each audio-preserving output independently decoded 102 Opus frames at 48 kHz mono. The pre-existing **Opus packet-header warning remains** during FFmpeg open/probing despite complete successful decode. It remains a compatibility finding for later investigation, not a worker migration failure or a warning-free interoperability claim.

### Reproduction, checks and remaining scope

- `node tests/chromium-worker-interop.mjs worker`, `main`, and `fallback`: passed; both an initial run and a final visibility-instrumented run completed in each mode. Final detailed results/media are under ignored `tmp/m33-chromium-*`.
- `node tests/inspect-chromium-outputs.mjs`: all nine outputs passed independent checks; detailed metadata/warnings are saved beside browser evidence.
- Five transport unit tests, Rust formatting, wasm workspace check/Clippy, six core host tests and the locked Dioxus web build passed. The pre-existing GNU GPU-test linker warning is unchanged.
- The earlier Firefox worker/main results remain valid for its two enabled WebM profiles; Firefox still cannot enable this AAC profile. The unsupported reason is expected capability behavior, not a remaining M3.3 implementation task.
- No human listening, arbitrary worker runtime crash/device-loss injection, startup-timeout test, Safari/other-OS test, or restrictive-CSP/subpath deployment validation was added. Transport runtime-crash handling remains unit-tested; driver recovery and broader compatibility are later milestones.
- Long-run memory/queue profiling, resource pools, hardware-preference comparisons, geometry/color work and streaming remain M3.4–M3.6. Completion here is scoped to the M3.3 acceptance gate, not completion of M3 overall.

# M3.4 bounded-throughput and resource-reuse report

Date: 2026-09-19
Status: **acceptance passed for the tested Firefox/Chromium environments; M3.5 was not started**

## Implemented behavior

- The UI and platform-neutral job policy expose `no-preference` (default compatibility baseline) and `prefer-hardware`. The chosen value crosses the worker boundary and is applied to the exact input decoder and selected output encoder probes. A requested hardware preference is used only if the complete video profile passes. Otherwise the app visibly reports the failed exact configuration and uses `no-preference`; it never changes codec or container.
- Conversion now reports live/peak/total/discarded counts for decoded video, GPU work, video encoder callbacks, video packet callbacks, decoded audio, audio encoder callbacks, and audio packet callbacks. Completion and cancellation settle application-owned callback counts back to zero.
- The pinned Mediabunny implementation bounds its combined decoder packet/callback queue at 40 before decoded output and 8 while producing decoded samples, and waits when the WebCodecs encoder queue reaches 4. Application decoded-frame, GPU, and source-add stages are serial, audio submissions are serial, and mux writes are promise-serialized. These are separate bounds; codec/library-internal memory is not claimed as observable.
- The wgpu execution context owns a device-generation-tagged, four-slot input texture ring. Each slot holds an explicit lease plus its decoded frame and optional compatibility bitmap through submitted-work completion. Reuse first retires the slot's previous completion callback; job completion and cancellation drain every remaining slot before cleanup. Telemetry reports allocation/reuse, leases, ingress copies, canvas captures, CPU bridge/submission time, cumulative overlapping completion latency, actual slot-reuse wait, and final-drain wait. Completion latency is explicitly not pure GPU execution time.
- Worker initialization time is measured once and reported separately from conversion. The existing application frame/sample ownership counters and cancellation/restart checks remain in place.

## Initial single-slot short-job baseline

The initial single-slot worker checks used Edge 153.0.4234.32 on Windows x64 with `intel / gen-12lp (BrowserWebGpu)` and Firefox 156.0 with browser-redacted adapter identity. The deterministic 60-frame H.264/AAC fixture was converted, re-decoded in the browser, loaded and midpoint-seeked in an HTML media element, cancelled and restarted for every enabled profile/preference combination. Five repeated M1 image/timestamp checks also passed in each worker run. The later four-slot comparison supersedes these numbers for current performance while retaining them as the measured baseline.

| Browser | Enabled profiles | Requested preference | Selected preference | Result |
|---|---|---|---|---|
| Edge 153 | WebM/VP8/Opus, WebM/VP8/video-only, MP4/H.264/AAC | `no-preference` | `no-preference` | all passed; 407.0, 422.2, and 700.1 ms including verification |
| Edge 153 | same three profiles | `prefer-hardware` | `no-preference` | all passed after visible fallback; exact VP8/H.264 encoder probes rejected the requested preference; 387.2, 376.6, and 738.5 ms |
| Firefox 156 | both WebM profiles | `no-preference` | `no-preference` | both passed; 6,213 and 6,187 ms including verification |
| Firefox 156 | both WebM profiles | `prefer-hardware` | `no-preference` | both passed after visible fallback; exact VP8 encoder probe rejected the requested preference; 6,151 and 6,173 ms |

The hardware-request rows are **baseline fallback runs**, not measurements of hardware-preferred encoding. Timing differences are warm-cache/run variation and do not justify an automatic preference. Firefox continued to disable MP4 because its exact AAC encoder probe fails, independent of the video acceleration setting.

Across these baseline short worker conversions, each stage ended with zero live application-owned items. GPU leases ended at `0/1` live/peak, there were 60 ingress copies and 60 canvas captures, and one input texture slot was reused for all 60 frames after size configuration. Edge used direct VideoFrame ingress with zero additional bitmap conversions. Firefox used its documented VideoFrame→ImageBitmap compatibility conversion 60 times. Edge worker initialization measured 353.0 ms and Firefox 673.0 ms in these runs; that context was reused by every conversion command.

Before the final worker smoke, the same two acceleration requests × three profiles also passed the explicit main-thread and injected automatic-fallback paths in Edge. Together with the worker path, `tests/inspect-chromium-outputs.mjs` independently inspected all 18 short outputs using FFprobe/FFmpeg 8.0.1: every file fully decoded with 60 video frames; audio-preserving files retained the expected AAC or Opus audio, video-only files had none, and MP4 retained fast-start layout. The existing Mediabunny Opus packet-header warning remains even though full decoding succeeds.

## Initial single-slot long run

`tests/chromium-long-run.mjs` used the local 70,439,352-byte `tmp/user-test/Input.mp4` in the Edge dedicated worker with the `no-preference` WebM/VP8/Opus profile:

- Input/output: 3,530 H.264 frames at 1920×1080 plus 6,343 decoded audio samples → 3,530 VP8 frames at 960×540 plus 7,365 Opus packets.
- Duration/output: 147.300 seconds, 32,138,436 bytes; browser re-decode verification passed. The test intentionally retained the output Blob in the browser rather than writing a 32 MiB repository artifact.
- End-to-end time: 47,058.3 ms including verification. A 50 ms window heartbeat ticked 946 times with a 62.9 ms maximum observed gap; this is a responsiveness observation, not a real-time guarantee.
- Stage peaks: decoded video 1, GPU 1, video encoder callbacks 3, video packets 1, decoded audio 1, audio encoder callbacks 1, and audio packets 1. Every stage ended at zero live items. Application frame/sample ownership ended at 0/0 with peaks 2/3.
- GPU pool: 2 lifetime allocations across initial/configured sizes, 3,529 job reuses, leases `0/1`, 3,530 ingress copies, 3,530 canvas captures, and zero bitmap fallbacks. Reported CPU bridge/submission time was 1,966.7 ms; submitted-work waits were 41,723.3 ms and are not interpreted as pure GPU time.

This baseline long run established stable observable high-water marks and cleanup for the original slot path; it was not a browser-process heap or driver-memory profile. The four-slot implementation was separately rerun below.

## Checks

- `cargo fmt --all -- --check`, wasm workspace check, and wasm all-target Clippy with `-D warnings`: passed.
- `cargo test -p media-core -p media-gpu`: 7 core tests passed; the existing GNU linker `corrupt .drectve` warning for the empty GPU test binary remains.
- JavaScript syntax checks and `node tests/worker-transport.test.mjs`: passed (5 tests).
- `dx build --platform web`: passed with Dioxus CLI 0.7.10.
- Final `node tests/chromium-worker-interop.mjs` and `node tests/firefox-worker-interop.mjs worker`: passed. Earlier M3.4 Edge main/fallback matrices and independent inspection also passed. Evidence and generated media remain under ignored `tmp/m33-chromium-*`, `tmp/m34-firefox-worker`, and `tmp/m34-long`.

## Four-slot synchronization follow-up

The single-slot measurements isolated the Firefox bottleneck: about 1.1–1.3 seconds was spent in VideoFrame→ImageBitmap ingress/submission and about 4.6–4.8 seconds in 60 serial submitted-work waits. The processor now retains at most four submissions and retires a slot only before its next reuse. This changes synchronization and resource ownership only; codecs, quality settings, shader, copy path, capture ordering, and output profiles are unchanged.

Final four-slot worker results for the same deterministic input:

| Browser | Profile/request | Single-slot baseline | Four-slot result | Observed change |
|---|---|---:|---:|---:|
| Firefox 156 | WebM/VP8/Opus, baseline | 6,213 ms | 1,410 ms | 77.3% lower elapsed time |
| Firefox 156 | WebM/VP8/video-only, baseline | 6,187 ms | 1,298 ms | 79.0% lower elapsed time |
| Firefox 156 | WebM/VP8/Opus, hardware request falling back to baseline | 6,151 ms | 1,172 ms | 80.9% lower elapsed time |
| Firefox 156 | WebM/VP8/video-only, hardware request falling back to baseline | 6,173 ms | 1,757 ms | 71.5% lower elapsed time |
| Edge 153 | WebM/VP8/Opus, baseline | 407.0 ms | 325.8 ms | 20.0% lower elapsed time |
| Edge 153 | WebM/VP8/video-only, baseline | 422.2 ms | 356.1 ms | 15.7% lower elapsed time |
| Edge 153 | MP4/H.264/AAC, baseline | 700.1 ms | 298.3 ms | 57.4% lower elapsed time |

These are individual acceptance runs rather than a statistical benchmark. The hardware-request rows still selected `no-preference`, so their variation is not a hardware-acceleration result. Firefox's four-slot worker reported lease peaks of 4, only 0–2 ms total slot-reuse wait, 0–18 ms final-drain wait, and zero live leases after each complete job. Its explicit main-thread path also passed both profiles × both requests in 1,260–1,581 ms, including cancellation/restart, playback/seek, and five M1 rounds. Edge reported a lease peak of 4 and at most 1.2 ms slot-reuse wait in the final worker matrix.

Browser re-decode, playback/seek, cancellation/restart, and five repeated deterministic M1 image/timestamp runs passed in both browsers. FFprobe/FFmpeg independently decoded all six newly generated Edge worker outputs with exactly 60 video frames and the expected AAC, Opus, or absent audio; the known Opus warning remains. This verifies that returning captured frames before each individual completion callback did not reorder or corrupt the tested output.

The 147.3-second Edge test was repeated with the ring: 3,530/3,530 frames, 6,343 decoded audio samples, 7,365 Opus packets, 32,138,436 bytes, and full browser verification passed in 46,406.5 ms versus the 47,058.3 ms single-slot run. Leases ended at `0/4`; slot-reuse wait totaled 82.0 ms, final drain 0.1 ms, and the 50 ms heartbeat's maximum gap was 57.4 ms. This only modestly improved the long Edge run, showing that its long-input bottleneck lies elsewhere; the large Firefox short-job gain specifically confirms that per-frame synchronization dominated the earlier Firefox result.

## Explicitly untested or unavailable

- No tested output encoder accepted `prefer-hardware`, so actual hardware-preferred throughput, startup, CPU load, power, and codec-engine selection remain unmeasured. WebCodecs configuration success and the displayed wgpu adapter would not prove hardware codec execution anyway.
- The browser exposes no reliable, codec-attributed CPU-load or power metric used by this app. GPU timestamp queries were not requested/supported in this path; submitted-work waits are not a substitute.
- wgpu 30.0.1's released browser backend does not implement its plane-based external-texture API for this use and exposes no released direct VideoFrame texture import. Therefore no direct-import comparison was possible. The measured baseline remains one external-image copy per frame, plus Firefox's explicit compatibility bitmap conversion.
- Safari, non-Windows systems, other GPUs/drivers, device loss, browser-process/driver heap stability, arbitrary codec hangs, HDR conversion, wide-gamut output, and streaming output remained untested at M3.4. M3.5 subsequently added the tested SDR/geometry policy below; M3.6 is the next planned milestone.

## Bounded bitmap preparation and verification timing, 2026-09-20

Firefox's compatibility ingress now probes the first frame through the real direct-copy path, then—only after that copy establishes that ImageBitmap fallback is required—prepares subsequent bitmaps in bounded groups of four. Decoded frames/samples remain owned while preparation is pending, completed bitmaps are consumed strictly in timestamp order, Rust takes ownership before GPU submission, and cancellation/error cleanup closes every bitmap that was not transferred. Chromium continues to use direct VideoFrame ingress and created zero preparation tasks; there is no browser-name switch.

Firefox 156 worker results for the 60-frame deterministic fixture with full diagnostic verification:

| Profile/request | Converted/finalized | Full verification | Total job | Bitmap preparation |
|---|---:|---:|---:|---|
| WebM/VP8/Opus baseline | 1,248 ms | 65 ms | 1,313 ms | 59 tasks; peak 2; 1,007 ms cumulative; 1 ms ordered wait |
| WebM/VP8/video-only baseline | 1,239 ms | 37 ms | 1,276 ms | 59 tasks; peak 2; 1,011 ms cumulative; 2 ms ordered wait |
| WebM/VP8/Opus hardware request → baseline fallback | 1,204 ms | 49 ms | 1,253 ms | 59 tasks; peak 2; 980 ms cumulative; 6 ms ordered wait |
| WebM/VP8/video-only hardware request → baseline fallback | 1,168 ms | 35 ms | 1,203 ms | 59 tasks; peak 2; 935 ms cumulative; 0 ms ordered wait |

All four jobs passed full browser re-decode, audio checks where applicable, playback/seek, cancellation/restart, zero-live-resource cleanup, and five repeated M1 orientation/color/timestamp rounds. Decoded-video preparation peaked at four retained items; application frame/sample peaks were at most 5/5, GPU leases returned to `0/4`, and bitmap preparation returned to `0/2`. Cancellation summaries also returned bitmap preparation to zero. The explicit Firefox main-thread path passed the same matrix and five M1 rounds, with 1,195–1,527 ms conversion/finalization times. The prior four-slot runs were about 1,172–1,757 ms including verification, so the new wall-time result is directionally better but noisy and modest—not a fourfold bitmap speedup. Firefox appears to expose at most two concurrent preparation tasks here, and cumulative ImageBitmap work remains roughly one second.

With diagnostic verification omitted—the normal UI behavior—two baseline Firefox runs converted/finalized in 1,160–1,208 ms (Opus) and 1,112–1,146 ms (video-only). Full verification itself cost 35–65 ms in the measured worker runs. When verification is skipped, displayed duration is explicitly an expected timeline derived from processed timestamps rather than a re-decoded container duration. Automated interoperability URLs use `?verify=full`; `VERIFY_OUTPUT=skip` exercises the production path.

Edge 153's final worker matrix also passed all three profiles × both acceleration requests, cancellation/restart, playback/seek and five M1 rounds. Direct ingress remained selected: bitmap preparation total/peak were `0/0`. Baseline converted/finalized times were 261.5 ms for WebM/VP8/Opus, 330.2 ms for video-only WebM, and 266.8 ms for MP4/H.264/AAC; their full verification steps cost 15.9, 12.3 and 14.2 ms respectively. FFprobe/FFmpeg independently decoded the six newly generated worker outputs with 60/60 video frames and expected audio/no-audio tracks; the existing Opus warning remains.

Conclusion: both requested changes are validated, but bounded bitmap preparation does not close Firefox's remaining gap. The next justified experiment is the explicitly counted CPU upload compatibility path described in the architecture discussion; increasing the GPU slot count is not supported by these measurements.

# M3.5 geometry, timing, and color interoperability report

Date: 2026-09-21
Status: **acceptance passed for every profile enabled in the tested Firefox/Chromium environments; M3.6 subsequently completed as recorded below**

## Implemented policy

The browser backend now inspects coded dimensions, decoded visible rectangle, square-pixel dimensions, pixel aspect ratio, clockwise rotation, post-rotation horizontal flip, color metadata, HDR state, and transparency before configuring the GPU. `media-core` owns the platform-neutral geometry types and validation. The browser external-image boundary normalizes the decoded visible image, pixel aspect ratio, and accepted SDR color into the square-pixel sRGB processing texture; shared WGSL applies the inverse sampling transform for rotation/flip while performing the half-size resize. The encoder therefore receives pixels with orientation baked in and emits square-pixel, rotation=0, flip=false output.

Accepted color is opaque BT.709/sRGB SDR: primaries may be unspecified or BT.709, transfer may be unspecified, BT.709, or IEC 61966-2-1, and matrix may be unspecified, BT.709, or RGB. Browser color conversion into the `Rgba8Unorm` canvas is relied on and reported; this is not a claim of wide-gamut or HDR fidelity. Transparency, BT.2020/PQ and other HDR/wide-color combinations are rejected before conversion. A change to coded/visible/square-pixel dimensions, orientation, or color metadata after the first decoded sample fails the job and drains owned resources; adaptive resolution reconfiguration is deferred.

Every processed input timestamp and duration remains an integer microsecond value. Full diagnostic verification reopens the finalized container, enumerates all encoded video packets, sorts them into presentation order, and compares every timestamp/duration with the processed input timeline. WebM permits at most 1,000 µs error because its millisecond timecode independently rounds packet endpoints; MP4 permits 5 µs in the tested muxer timescale. When the final packet omits its duration, its start is still checked and the independently checked track coverage supplies the final endpoint. Midpoint video seek and decoded audio near the beginning, middle, and end retain the earlier M3 checks.

## Deterministic fixtures

`tools/generate-m35-fixtures.ps1` generated four CC0-1.0 inputs with FFmpeg 8.0.1:

| Fixture | Bytes | SHA-256 | Purpose |
|---|---:|---|---|
| `m35-geometry-color.mp4` | 56,356 | `03dbdd9a99de71b411f1c22c9feb69fe75ecc38f67bea3a83051072835b8bec9` | H.264 clean-aperture crop, non-square pixels, 90° clockwise rotation, horizontal flip, BT.709 markers, AAC tone |
| `m35-vfr-offset.mp4` | 88,866 | `c16c968ee505aa6e2a0ee3d4fb0ac8ea58aad0378e97062a7872925cb0983724` | 36 VFR frames with 33,333/66,667 µs durations, video start 1.250 s, AAC start 1.228 s |
| `m35-hdr-tagged.mp4` | 9,032 | `f4ee492ca6a2474b65a63fa986e2013edeca19ff33fa3b3cb22a976974919b48` | Deterministic BT.2020/PQ negative input |
| `m35-resolution-change.mp4` | 58,651 | `51bcc46d0498dab9bde031cd3d1e7706846c8a03ce2dbf0b9f6a6f703efe5e45` | 320×180 H.264 followed by 352×198 H.264 |

The geometry fixture begins as a 320×192 synthetic source with explicit H.264 cropping. In both tested browsers the decoder normalized that aperture into a 318×180 coded allocation with a full 318×180 `visibleRect`; the track's sample-aspect/display metadata produced 421×180 square-pixel input and a 180×421 oriented display, resized to 90×210. Thus compressed crop normalization was exercised, while a decoder-exposed non-zero `visibleRect` was not available in this H.264/browser matrix. The implementation validates and uses such a rectangle if supplied, but that subcase remains explicitly unverified.

## Real-browser results

The app was served by Dioxus CLI 0.7.10 in full diagnostic mode and ran in a dedicated worker on Windows x64. The checked-in harnesses were `node tests/chromium-m35-interop.mjs 9230 8084` and `node tests/firefox-m35-interop.mjs 8084`. Evidence is retained locally under ignored `tmp/m35-chromium` and `tmp/m35-firefox`.

| Browser/GPU | Enabled profiles | Geometry/color result | VFR/timing result |
|---|---|---|---|
| Edge 153.0.4234.48; `intel / gen-12lp (BrowserWebGpu)` | WebM/VP8/Opus, WebM/VP8/video-only, MP4/H.264/AAC | 24/24 frames per profile; decoded 90×210; sampled corners red `(253,25,11)`, blue `(3,12,239)`, green `(9,108,0)`, yellow `(255,240,0)` for VP8 and equivalent H.264 values; seek/audio checks passed | 36/36 frames per profile; decoded 160×90; maximum timeline error 667 µs WebM and 4 µs MP4; audio profiles retained non-silent audio and A/V offsets |
| Firefox 156.0; adapter identity redacted (`BrowserWebGpu`) | WebM/VP8/Opus, WebM/VP8/video-only | 24/24 frames per profile; decoded 90×210; sampled corners red `(252,26,1)`, blue `(0,17,250)`, green `(0,109,2)`, yellow `(253,242,4)`; seek/audio checks passed | 36/36 frames per profile; decoded 160×90; maximum timeline error 667 µs; Opus profile retained non-silent audio and A/V offsets |

Every enabled profile reported baked square-pixel output with no rotation/flip metadata, passed packet-count and complete per-frame timeline checks, and returned application-held frames/samples and GPU leases to zero. Chromium exposed all three profiles; Firefox still did not expose the MP4/H.264/AAC encoder profile, so it was not presented as available and was not counted as a Firefox failure.

Both browsers rejected the HDR fixture during inspection with `HDR input is not supported by the M3.5 SDR pipeline (bt2020/pq/bt2020-ncl)`. Both began the resolution-change fixture at 320×180, detected the first 352×198 decoded frame, failed with `Mid-stream geometry change is unsupported`, and cleaned application-held frames/samples to zero. No altered output was offered for either negative case.

## Checks and limitations

Verified in this milestone: Rust host tests/checks/lints, complete wasm workspace checks/lints, JavaScript syntax/unit tests, Dioxus web build, deterministic regeneration metadata/hashes, and the two real-browser harnesses above. Exact commands are listed in the repository README.

Still untested: Safari, other operating systems/GPUs, decoder-exposed non-zero visible-rectangle offsets, vertical flip independent of rotation-matrix decomposition, HDR conversion/tone mapping, wide-gamut outputs, transparency, adaptive mid-stream reconfiguration, and browser-internal color-conversion precision beyond representative decoded marker samples. Successful SDR marker checks do not prove colorimetric conformance, and the wgpu adapter still does not identify the codec execution engine.

# M3.6 configurable-output and bounded-streaming interoperability report

Date: 2026-09-24 through 2026-09-25
Status: **M3.6 acceptance passed in the available Firefox/Chromium environments; unavailable-platform and spontaneous-failure limitations remain explicit**

## Implemented policy

`media-core::ResizeSpec` owns original-size, percentage, and exact-size policy without browser or UI dependencies. The UI exposes original, 75%, 50%, 25%, and exact width/height. Exact sizing preserves display aspect ratio by default; disabling the lock explicitly requests stretching. The size is resolved from the post-crop, square-pixel, oriented display geometry. Requests are checked for zero, invalid percentage, overflow, implicit upscaling, and outputs smaller than the minimum 2×2 codec geometry. Odd resolved dimensions are floored to even values and the adjusted result is displayed before conversion.

File selection and every resize change run the real decoder/encoder capability probes for the resolved dimensions. The same resolved size is carried through worker or main-thread transport, wgpu configuration, canvas capture, status, encoding, playback, and full output verification. The existing deterministic 640×360 H.264/AAC fixture was reused; no new media fixture was necessary.

Input is lazily read through Mediabunny's `BlobSource` with an 8 MiB maximum cache; the former unconditional 256 MiB input rejection was removed. Output now uses Mediabunny 1.58.1's `StreamTarget` over origin-private file storage with WritableStream backpressure, 4 MiB target chunks, and measured write counts/maximum write size. MP4 uses non-fast-start layout on this random-access target so encoded media is not held for in-memory fast-start finalization. After close, the disk-backed `File` supports the existing object URL, playback, and full verification. The latest temporary file remains valid for that URL and is removed when the next conversion supersedes it; abnormal/cancel paths remove their partial file.

When origin-private writable storage is unavailable, the same job uses a clearly reported `BufferTarget` compatibility fallback and retains a 256 MiB input cap. `?output=memory` exists only to exercise that path deterministically. There is no mid-job retry from a failed stream into memory, avoiding duplicate encode work or a sudden unbounded allocation.

## Real-browser results

The app was served by Dioxus CLI 0.7.10 with `?verify=full` and ran in its dedicated worker on Windows x64. The checked-in harnesses were `node tests/chromium-resize-interop.mjs 9231 8084` and `node tests/firefox-resize-interop.mjs 8084`. Evidence is retained locally under ignored `tmp/m36-resize-chromium` and `tmp/m36-resize-firefox`.

| Browser/GPU | Enabled profiles | Verified resize cases |
|---|---|---|
| Edge 153.0.4234.48; `intel / gen-12lp (BrowserWebGpu)` | WebM/VP8/Opus, WebM/VP8/video-only, MP4/H.264/AAC | original 640×360; 75% 480×270; 50% 320×180; 25% 160×90; exact 500×500 locked → 500×280; exact 501×301 unlocked → 500×300 (18 complete conversions) |
| Firefox 156.0.1; adapter identity redacted (`BrowserWebGpu`) | WebM/VP8/Opus, WebM/VP8/video-only | the same six resolved sizes (12 complete conversions) |

Every conversion matched the displayed output geometry, completed full encoded-output re-decode verification, loaded for playback at the expected dimensions, and returned application-held frames/samples and GPU leases to zero. The Firefox matrix continued to expose only the two WebM profiles; unavailable MP4 was not counted as a failure.

All 30 matrix conversions used bounded origin-private output. Their summaries reported the configured 4 MiB chunk boundary and measured writes; the small deterministic outputs required one write each, with maximum writes far below the boundary. Chromium and Firefox additionally converted a valid logical 269,763,667-byte MP4 consisting of the M2 fixture plus a reproducible sparse 257 MiB `free` box. Both processed 60/60 frames at 160×90 through WebM/VP8/video-only, fully re-decoded the disk-backed output, and reported one bounded output write (17,340 bytes Chromium; 19,084 bytes Firefox). This crosses the former input policy boundary and verifies bounded application I/O.

Chromium's explicitly forced memory mode converted the normal fixture and passed full verification while visibly reporting `memory fallback (maximum input 256 MiB)`. After bounded input inspection and capability probing, the same mode rejected the 257.3 MiB logical input before starting the conversion decode/process/encode pumps, with the exact fallback reason. A separate deterministic compatibility case removed `StorageManager.getDirectory` from the main-thread execution context without setting `?output=memory`; the normal automatic branch selected the same memory fallback, named `origin-private file storage is unavailable`, converted 60/60 frames, and fully re-decoded the output. This validates API-unavailable selection, not a naturally shipped browser without the API or a mid-write storage failure.

The bounded output entry remains present only while its returned `File`/object URL is usable. Replacement removes the previous entry. Both harnesses recorded the origin's pre-test `diaxus-*.partial` baseline, observed exactly one additional current-page entry after finalization, dispatched pagehide, and returned to the baseline. They repeated the check with a real same-origin navigation and again returned to the baseline. Existing entries from unrelated/forcibly terminated tabs are deliberately not deleted because ownership cannot be proved.

Both browsers cancelled an active conversion after changing the size and reported zero application-held frame/sample resources. Both rejected an aspect-locked 800×500 request before conversion because it would resolve to 800×450 and upscale the 640×360 source. The error reported the requested and oriented source dimensions; no output was offered.

## Checks and M3.6 completion

Verified for M3.6: focused `media-core` resize/input-policy tests, host Rust tests/checks/lints, complete wasm workspace checks/lints, JavaScript syntax and worker-transport tests, Dioxus web build, the M3.5 real-browser regression harnesses, 30 resize/profile browser conversions, bounded large-input checks, forced and automatic storage fallback, synthetic and real-navigation OPFS cleanup, codec/device-loss recovery in both execution contexts, actual GPU/encoder boundary probes, and the long-run/large-output Chromium stress case. Mediabunny was updated from 1.58.0 to 1.58.1 and the npm lockfile was regenerated.

M3.6 is complete for the available test environments. The matrix now includes 50%; controlled recovery and restart pass; page-exit cleanup returns current-page storage to its baseline; API-unavailable fallback passes; actual GPU/encoder boundaries are recorded below; and the long-duration/large-output run passes. Safari, non-Windows systems, other GPUs/drivers, spontaneous driver/browser-process loss, indefinitely hung codec callbacks, naturally absent/denied storage APIs, and HEVC output on a browser that supports it remain untested. These are explicit compatibility limitations, not claims of universal support and not locally actionable blockers to M4.

## Named resolution presets and source metadata, 2026-09-24

The resize selector now also exposes aspect-preserving maximum bounds for HD/720p (1280×720), Full HD/1080p (1920×1080), 2K width (2048 pixels with source aspect preserved), QHD/1440p (2560×1440), and 4K UHD/2160p (3840×2160). `media-core` owns these concrete `ResizeSpec` constants. A focused UHD-input test verifies every resolved dimension, including 2048×1152 for the 2K-width choice on a 16:9 source. Named modes use the same post-orientation resolution, even-dimension adjustment, profile re-probe, and no-upscale policy as exact sizing.

After selection, the UI now reports filename/file size, display and coded resolution, H.264 codec parameter string, duration, average packet/frame rate, video frame count, and audio codec/channel/sample-rate metadata returned by the browser inspection boundary. Edge 153 and Firefox 156 both displayed `640×360`, an `avc1.*` H.264 string, 60 frames, 2.000 seconds, 30 fps, and mono 48 kHz AAC for the deterministic M2 fixture. Both exposed all five named choices and clearly rejected HD/720p for that 640×360 source as implicit upscaling; the existing original/percentage/exact matrices then passed unchanged. Actual high-resolution encoding at each named bound remains dependent on the exact browser/profile capability probe and was not claimed from this low-resolution fixture.

## H.265/HEVC input and output profile, 2026-09-24

`media-core` now defines `Mp4H265Aac` as a concrete MP4/HEVC/AAC profile. The browser boundary accepts AVC or HEVC MP4 video after exact decoder probing, reports the actual `avc1`/`avc3`/`hvc1`/`hev1` parameter string, and uses the same bounded decode, wgpu processing, audio, cancellation, OPFS output, and full verification path. HEVC output has its own exact video/audio encoder probe and disabled reason; it does not silently fall back to H.264 or become the preferred profile.

The new 52,426-byte CC0 fixture `fixtures/m36-h265-aac.mp4` contains 30 synthetic 320×180 HEVC Main-profile frames at 30 fps and one second of mono 48 kHz AAC. Its checked-in manifest and generator record provenance, expected streams, and SHA-256.

Verified on Windows x64:

- Chromium/Edge 153 decoded the HEVC fixture, displayed `H.265/HEVC (hev1.1.6.L60.90)`, processed all 30 frames through wgpu to 160×90 VP8, finalized bounded OPFS output, and fully re-decoded all 30 output frames. Cleanup returned application-held frames, samples, and GPU leases to zero.
- That Chromium environment rejected the exact HEVC encoder probe at 320×180/30 fps with `hardwareAcceleration=no-preference`; the H.265 output option remained disabled with that reason. No HEVC output file was produced or claimed.
- Firefox 156 rejected the exact HEVC input decoder configuration. Its HEVC output profile also remained unavailable. The existing five-size × two-WebM-profile Firefox regression matrix continued to pass.

The checked-in focused Chromium harness records input and output capabilities independently. Actual H.265 output muxing/re-decode still requires validation on a browser/OS/GPU combination whose `VideoEncoder` probe accepts HEVC. HEVC Main10/HDR, alpha, other containers, non-Windows platforms, and licensing suitability for distribution remain untested or out of scope for this slice.

## Injected codec and WebGPU device-loss recovery, 2026-09-25

The recovery boundary now has two deterministic, test-only failure modes. `?failure=codec-once` throws after the fifth frame has passed through wgpu and before it enters the video encoder. `?failure=device-loss-once` calls `Device::destroy()` in the fifth processing callback, after four completed frames, and fails that frame explicitly. Both modes are one-shot within an execution context so that the same page can retry without reloading.

The failure path stops further input, settles submitted GPU work, closes or discards decoded frames and prepared bitmaps, drains codec callbacks, releases audio samples, aborts the muxer, and removes the partial origin-private output. Device loss additionally invalidates the cached GPU session. The retained canvas/OffscreenCanvas handle is then used to create a fresh wgpu session and generation for the next explicit job; resources are never reused across generations.

The checked-in harnesses were run as `node tests/chromium-recovery-interop.mjs 9233 8086` and `node tests/firefox-recovery-interop.mjs 8086`. They used the deterministic H.264/AAC fixture and the WebM/VP8/Opus output profile with full output verification. Evidence is retained locally under ignored `tmp/m36-recovery-chromium` and `tmp/m36-recovery-firefox`.

| Browser | Contexts | Injected codec failure | Application-triggered device loss |
|---|---|---|---|
| Edge 153.0.4234.48 on Windows x64 | dedicated worker and explicit main-thread fallback | failed after five GPU-processed frames; no partial download; cleanup 0 frames/0 samples; same-page retry decoded 60/60 frames at 320×180 with non-silent Opus audio; device generation remained 1 | failed after four completed frames; no partial download; cleanup 0/0; retry decoded 60/60 frames and recreated device generation 2 |
| Firefox 156.0.1 on Windows x64 | dedicated worker and explicit main-thread fallback | same cleanup and 60/60-frame retry result; Firefox's bounded ImageBitmap compatibility resources also returned to zero; generation remained 1 | same cleanup and retry result; the worker OffscreenCanvas and main-thread canvas paths both recreated generation 2 |

All eight failure/retry scenarios loaded the retry output for playback at 320×180 and about 2.04 seconds. Full diagnostic re-decode verified the complete frame timeline and non-silent audio. The harnesses also required that no download link be exposed after the failed attempt. Application-visible frame/sample counts and GPU leases returned to zero; these counters do not measure browser/driver-internal allocations.

This is evidence for deterministic cleanup and a clean restart after an application-observed codec exception or application-triggered wgpu device destruction. It is not evidence for recovery from an unprompted GPU driver reset, browser/GPU-process crash, device-lost notification arriving at arbitrary pipeline points, an indefinitely missing codec callback, or automatic checkpoint/resume. Those cases remain explicit limitations outside the verified compatibility matrix.

## Final limits, cleanup, fallback, and stress acceptance, 2026-09-25

The final resize runs added the 50% mode to every enabled profile. Chromium completed 18 size/profile combinations and Firefox completed 12; every one passed full output re-decode, playback geometry, timing, audio where applicable, and zero application-owned cleanup. M3.5 geometry/VFR/HDR/resolution-change regressions and the eight controlled recovery/restart scenarios were rerun after the storage and verification changes and remained green.

The selected Chromium WebGPU adapter reported `maxTextureDimension2D = 16384`. `GpuSession::configure` now rejects input or output dimensions exceeding the actual requested device's 2D texture limit before canvas/texture allocation. Exact `VideoEncoder.isConfigSupported` VP8 probes in the same browser accepted 8192×8192 and rejected 16384×16384, 32768×32768, and 65536×65536. Normal profile probing still gates every resolved output size; no oversized allocation was attempted. The low-resolution deterministic conversion fixture cannot validly request those outputs because the independent no-upscale rule rejects them first.

The existing local 70,439,352-byte input provided the long-run/large-compressed-output stress case. It contains 3,530 H.264 frames, 6,343 decoded audio samples, 1920×1080 display geometry, and 147.3 seconds of media. Chromium converted it to 960×540 WebM/VP8/Opus in 21,465.3 ms. The 32,138,436-byte result was emitted through 17 serialized OPFS writes with a maximum write of exactly 4,194,304 bytes, not accumulated in an application ArrayBuffer, and then fully re-decoded: all 3,530 video frames, complete timestamp/duration timeline (maximum quantization error 708 µs), 7,365 non-silent Opus packets, and midpoint seek passed. Stage counters ended at zero, GPU leases were bounded at four, and the page heartbeat recorded 436 ticks with a maximum 62.3 ms gap during the worker job. This input is user-provided and ignored by Git, so the result is reproducible only where `tmp/user-test/Input.mp4` is present; the checked-in deterministic sparse fixture separately preserves the >256 MiB bounded-input test.

That stress run also exposed valid WebM packets with omitted duration fields before the final packet. Verification now derives an omitted non-final duration from the next packet timestamp, as WebM timing permits, while applying the same 1 ms endpoint tolerance; final coverage remains independently checked against track duration. The M3.5 VFR/non-zero-origin fixtures passed after this correction, preventing the compatibility rule from weakening timestamp validation.

With these results, the M3.6 acceptance gate is passed for the available Firefox/Chromium environments and core M3 browser robustness is complete. M3.7 remains an optional FFmpeg WASM compatibility spike rather than required follow-up work.

## Profile selector before input selection, 2026-09-25

The output selector and Convert button now stay disabled until an input-specific profile probe succeeds. Selecting another file or changing the output size invalidates the previous profile result until the new check completes. This is necessary because the exact check includes the input decoder, audio presence/channel count, frame rate, and resolved output dimensions; a browser-wide encoder probe alone cannot establish whether that file can use an MP4 profile.

The Chromium and Firefox resize harnesses checked the initial disabled state and then loaded the deterministic H.264/AAC fixture. After its probe, both controls became active. H.264 MP4 became selectable in Chromium and remained disabled with its capability reason in Firefox. The existing 18 Chromium and 12 Firefox full re-decode cases then passed unchanged.

The UI now selects Original size at startup. Both browser resize harnesses assert that selection and the resolved 640×360 geometry for the 640×360 fixture before exercising the six-size matrix. The earlier half-size worker, recovery, geometry, and stress harnesses now select 50% explicitly so their historical 320×180 and 960×540 expectations remain meaningful.

## M3.7 — conditional FFmpeg WASM backend spike, 2026-09-25

The measured gap is Firefox 156.0.1 on Windows x64: for the deterministic `fixtures/m2-h264-aac.mp4` input, its exact WebCodecs MP4/H.264/AAC output probe is unavailable, while Chromium/Edge 153.0.4234.48 accepts the same profile. An explicit `?backend=ffmpeg-wasm` choice now enables a constrained software output adapter for that profile. Without that query parameter, WebCodecs remains the default. The common browser job still demuxes and decodes through Mediabunny/WebCodecs, checks the same metadata/timeline, invokes the same Rust/wgpu resize callback, drains the same bounded video/audio pumps, and verifies output. FFmpeg receives the processed frames as RGBA via `VideoFrame.copyTo`; it copies the original MP4 into its virtual filesystem for audio decoding and encodes H.264 with libx264 and AAC. It does **not** inherit the WebCodecs path's no-explicit-readback property. No codec/container or resize choice is silently changed.

The spike is pinned to `@ffmpeg/ffmpeg` 0.12.15, single-thread `@ffmpeg/core` 0.12.10, and build-time esbuild 0.28.2. The locally packaged core is loaded only in explicit FFmpeg mode. The core WASM is 32,232,419 bytes (10,287,551 gzip bytes); core JS is 111,804 bytes (29,696 gzip); the bundled worker is 5,654 bytes (1,773 gzip). The normal `m2.js` bundle is shared by both modes (461,002 decoded bytes in the measured Chromium build); the large core is not fetched during the WebCodecs run. These are file/gzip sizes, not guaranteed network transfer sizes under every server/cache policy. The [ffmpeg.wasm architecture](https://ffmpegwasm.netlify.app/docs/overview/) documents its worker and virtual-filesystem model. The installed `@ffmpeg/core` package declares `GPL-2.0-or-later`; [FFmpeg's licensing documentation](https://www.ffmpeg.org/general.html) also notes that libx264 enables GPL obligations. Redistribution review is required before shipping this core in a production artifact.

| Browser and 320×180 MP4/H.264/AAC route | Conversion (60 frames) | Startup of FFmpeg core | Output/verification | Explicit copies and sampled memory |
|---|---:|---:|---|---|
| Edge 153, WebCodecs default | 287.4 ms (~209 frames/s) | not loaded | 101,661 bytes; 60 H.264 + 95 AAC packets; full re-decode pass, max timeline error 4 µs | 0 explicit pixel readbacks; browser/codec internal copies and peak process memory unknown |
| Edge 153, FFmpeg WASM | 1,088.2 ms (~55 frames/s) | 96.4 ms (locally cached core) | 87,943 bytes; 60 H.264 + 95 AAC packets; full re-decode pass, max timeline error 1 µs | 60 `VideoFrame.copyTo` RGBA readbacks; 13,824,000 raw bytes, 27,648,000-byte estimated maximum overlap of explicit frame buffers during assembly; sampled WASM linear-memory allocation peak 33,554,432 bytes |
| Firefox 156.0.1, WebCodecs default | not run: exact MP4 output profile disabled | not loaded | no MP4 output claimed | — |
| Firefox 156.0.1, FFmpeg WASM | 1,806.0 ms (~33 frames/s) | 320.0 ms (locally cached core) | 88,641 bytes; 60 H.264 + 95 AAC packets; full re-decode pass, max timeline error 1 µs | 60 explicit RGBA readbacks; same 13,824,000 raw bytes and 27,648,000-byte explicit-buffer overlap estimate; sampled WASM linear-memory allocation peak 33,554,432 bytes |

The figures are single local runs, not stable benchmarks. Conversion time includes core startup and the shared GPU path, but excludes the separately reported full verification pass (10.6 ms, 16.4 ms, and 79.0 ms respectively). Chromium's page-level JS heap snapshot after conversion was 28,784,170 bytes for WebCodecs and 47,514,455 bytes for FFmpeg mode; it is **not** a peak and excludes worker/WASM/process allocations. The worker samples allocated WASM linear memory every 25 ms, which is not physical resident-memory use. Total browser-process peak memory and internal browser/GPU copies remain unmeasured, so production resource acceptance is provisional. The selected Chromium WebGPU adapter reported `intel / gen-12lp`; Firefox redacted its adapter identity. CPU pixel uploads into wgpu were not added; the explicit readback, raw-buffer assembly, transfer into the FFmpeg worker, and virtual-filesystem writes are counted separately. The source MP4 copy was 279,635 bytes in this fixture.

Both checked-in real-browser harnesses, `tests/chromium-m37-spike.mjs` and `tests/firefox-m37-spike.mjs`, used a dedicated worker, selected 50% output explicitly, required full browser re-decode, and tested cancellation after conversion began followed by a same-page successful retry. After cancellation, both reported zero application-held frames and samples; the FFmpeg worker is terminated, and the shared GPU leases drain. Independent native `ffprobe` on the three saved MP4s confirmed H.264 320×180/60 frames, mono AAC 48 kHz/95 packets, and durations 2.026667, 2.021333, and 2.021333 seconds. The ignored local evidence/output files are under `tmp/m37-chromium` and `tmp/m37-firefox`. The fixture's provenance and timestamp manifest are in `fixtures/m2-h264-aac.json`.

Build checks passed: `cargo fmt --all --check`, host `cargo test -p media-core -p media-gpu -p media-web` (13 core tests), host and wasm32 `cargo clippy` with `-D warnings`, wasm32 `cargo check` for the web crates, JavaScript syntax checks, and `dx build --platform web`. The Edge and Firefox harnesses were rerun after those code changes, followed by native `ffprobe` on their final output files. This does not validate a release-mode bundle, Firefox main-thread fallback, or other hardware.

The adapter intentionally refuses sources over 64 MiB, raw frames over 128 MiB, non-integer reported frame rates, actual per-frame VFR/timestamp deviations, and inputs without exactly one audio track. It is an in-memory compatibility fallback, not a replacement for M3.6's bounded streaming path. The FFmpeg audio encoder is software-only, and audio is decoded a second time from the source MP4 inside FFmpeg after the shared job's audio samples were drained; that work is included in the measured runtime. Main-thread fallback, large files, all named resize modes, unusual A/V offsets, multi-track inputs, HDR, mid-stream geometry changes, and other browsers/OS/GPU combinations were not validated for this backend. The functional M3.7 profile/cancellation gate passed in the available browsers; unrestricted resource/distribution suitability is **not** claimed. The default WebCodecs path remains preferred and M4 is not blocked by these spike limits.

Follow-up selector correction (2026-09-25): on an FFmpeg-mode capability rejection, the page now retains the selected input metadata and resolved size, and prints the exact reason beside the disabled MP4 option as well as in status. The earlier early-return kept the selector disabled but hid that reason, misleadingly continuing to say “Select an input.” The 64 MiB input bound is an application safety policy, not an FFmpeg/browser limit. The ignored local `tmp/user-test/Input.mp4` is 70,439,352 bytes (about 67.2 MiB), so it exceeds that cap. Its 3,530 frames would also need roughly 7.32 GB of raw RGBA at 960×540 (50%) or 1.83 GB at 480×270 (25%), far beyond the separate 128 MiB cap. Simply raising the source limit would not enable this file safely. Both isolated browser harnesses rechecked that the small deterministic fixture still enables and converts MP4, then selected a deterministic 257.3 MiB sparse-MP4 fixture and verified that the selector remained disabled **with** the 64 MiB reason and source metadata visible. Long-file FFmpeg support requires a different streaming frame/virtual-filesystem bridge and is not claimed by this spike.

### M3.7 disk-backed streaming follow-up — 2026-09-25

This follow-up supersedes the FFmpeg-specific memory limits and copy claims above; those paragraphs describe the earlier in-memory spike. A shared browser `OutputStorageKind` policy now selects OPFS or memory for both output adapters. WebCodecs still uses its bounded Mediabunny `StreamTarget` and can explicitly fall back to memory. FFmpeg requires OPFS: each processed RGBA frame is copied once from the wgpu output `VideoFrame` into a per-frame JS buffer and written to a temporary OPFS file; the original input `File` is mounted read-only in FFmpeg's worker through Emscripten WORKERFS; the raw spool and final MP4 are mounted as synchronous OPFS file devices. The worker writes a seekable normal MP4 with `+faststart`, preserving the previous timestamp behavior. It no longer assembles all raw frames, calls `file.arrayBuffer()` on the source, copies either entire file into MEMFS, or reads the entire encoded result into a JS buffer. The raw spool is completed before FFmpeg encoding; this is disk-backed streaming I/O, **not** concurrent frame-by-frame encode. Explicit GPU→CPU pixel readbacks remain one per frame. FFmpeg's internal WASM/process allocations remain unbounded by this policy.

The application now rejects a predicted raw spool above 2 GiB, checks available origin-private storage quota before starting, and still validates each frame against supported constant-rate timing. Supported rates are integer 1–120 fps and 24000/1001, 30000/1001, or 60000/1001. The source has no FFmpeg-specific byte cap. This does not prove a 2 GiB recording will complete: quota estimates are advisory and FFmpeg may grow its own memory. At original or 50% size the ignored 3,530-frame user recording is still above the raw-spool bound; at 25% its predicted ~1.83 GB falls under that bound but its full conversion has **not** been run or verified. The 257.3 MiB test source below is logically large but contains only the deterministic 60 frames and therefore does not test long-duration scheduling or a large raw spool.

Verified on isolated Windows browser profiles using `tests/chromium-m37-spike.mjs` and `tests/firefox-m37-spike.mjs` after these changes:

| Browser | 60-frame FFmpeg fixture | 257.3 MiB sparse MP4 source | Cancellation/retry | Raw spool and output |
|---|---|---|---|---|
| Edge/Chromium 153.0.4234.48, Intel gen-12lp WebGPU | PASS, 320×180 H.264/AAC, full re-decode; 60 readbacks, 13,824,000 raw bytes | PASS, same decoded 60-frame timeline and non-silent AAC | PASS; zero application-held frames/samples after cancellation | Source read through WORKERFS; OPFS output device reported 5 writes, largest 84,053 bytes; no new raw entry left by completed run |
| Firefox 156.0.1, adapter identity redacted | PASS, 320×180 H.264/AAC, full re-decode; 60 readbacks, 13,824,000 raw bytes | PASS, same decoded 60-frame timeline and non-silent AAC | PASS; zero application-held frames/samples after cancellation | Source read through WORKERFS; OPFS output device reported 5 writes, largest 84,783 bytes; no new raw entry left by completed run |

For the saved small-fixture outputs, independent FFprobe reported 60 H.264 320×180 frames and 95 mono AAC packets at 48 kHz in both browsers; file durations were 2.021333 s and sizes 87,943 bytes (Chromium) and 88,641 bytes (Firefox). Full in-browser re-decode verified packet timestamps/durations within 1 µs and non-silent audio. The sampled WASM linear-memory allocation was 33,554,432 bytes in these short runs; that is not browser-process peak memory. The WebCodecs/default Chromium route also passed the same fixture in the rerun and used zero explicit pixel readbacks. The isolated Chromium profile contained an orphaned spool from an earlier failed development run; cleanup is now mandatory on successful jobs with short lock-release retries. The harness checks that the current run adds no raw-spool orphan; it does not claim crash recovery or sweep unrelated older entries.

Build verification: `npm run build:ffmpeg-worker`, `node --check` for both browser scripts, `cargo fmt --all --check`, `cargo test --locked -p media-core -p media-gpu` (13 core tests), `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`, and `dx build --web --locked` passed. The browser harnesses above passed after the final code changes. Still untested: the full 3,530-frame user clip, actual near-2 GiB OPFS use, quota exhaustion mid-job, browser crash recovery, Safari/other operating systems, main-thread fallback, other GPUs, long-run WASM/process memory growth, and distribution licensing review.

### M3.8 bounded live FFmpeg frame stream — 2026-09-25

M3.8 supersedes **only the M3.7 raw-RGBA OPFS spool and its 2 GiB raw-spool policy**. The selected MP4 source still uses WORKERFS, and the finished compressed MP4 still streams to OPFS. The shared WebCodecs decode/wgpu processing path and default WebCodecs encoder path are unchanged. FFmpeg's rawvideo input is now a non-seekable synchronous Emscripten device backed by a four-slot, 1 MiB-per-slot `SharedArrayBuffer` ring. FFmpeg `exec` runs concurrently with frame production; the producer asynchronously waits for free slots and keeps one reusable full RGBA frame buffer. The consumer blocks only in the dedicated FFmpeg worker. An EOF/abort state and peer notification handle normal drain and failure. No raw OPFS file is created, and the finalizer checks published, consumed, and expected RGBA byte counts. FFmpeg still requires one explicit `VideoFrame.copyTo` GPU→CPU readback per frame and a copy into WASM memory. The ring bounds explicit bridge storage, **not** FFmpeg's opaque WASM heap, browser-internal copies, decoded/GPU pools, or total process memory.

Runtime requirement: serve the app with cross-origin isolation. `dx serve --web --locked --addr 127.0.0.1 --port 8084 --cross-origin-policy` returned `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`; `crossOriginIsolated` was true in both tested browsers. The same command without `--cross-origin-policy` on port 8085 returned `unsafe-none` for both headers. In that non-isolated Edge session, the FFmpeg profile was disabled with the exact shared-memory/COOP/COEP reason, while the WebCodecs profile still enabled. Production-hosting header configuration and embedded-browser compatibility are not yet verified; there is no silent fallback to the old raw spool.

Verified on Windows 11 Home, isolated Edge/Chromium 153.0.4234.48 (WebGPU adapter `intel / gen-12lp`) and Firefox 156.0.1 (adapter identity redacted). The deterministic fixture is generated by `tests/generate-m38-long-fixture.mjs` from FFmpeg `lavfi` color and sine sources (CC0-1.0, no third-party media): 640×360 H.264/AAC MP4, 2,400 frames at 30 fps, 1,118,263 compressed bytes, 2,211,840,000 logical RGBA bytes. The generated file and browser outputs are ignored under `tmp/m38-long`; the generator checks its own FFprobe manifest.

| Browser | 2,400-frame conversion and bridge | Full browser re-decode / independent FFprobe | Ring and page responsiveness |
|---|---|---|---|
| Edge/Chromium | PASS in 24,220.5 ms; 2,211,840,000 raw bytes published and consumed; 2,400 explicit pixel readbacks; no raw OPFS entry | PASS: all 2,400 H.264 640×360 frames, 3,751 AAC packets, 80.021333 s, non-silent audio, timeline error ≤1 µs; saved seekable MP4 1,221,510 bytes | 4 MiB capacity, peak 4 ready slots, 1,696 producer waits, 13 consumer waits; page heartbeat 488 ticks, maximum interval 62.35 ms |
| Firefox | PASS in 54,906.9 ms; same published/consumed bytes and readback count; no raw OPFS entry | PASS: same frame/audio/timing counts, non-silent audio and seekable MP4 | Peak 3 ready slots, 0 producer waits, 670 consumer waits; heartbeat 1,098 ticks, maximum interval 65.58 ms |

These are single local runs, not comparative hardware benchmarks; verification is excluded from conversion time. The worker's 25 ms samples reported a 33,554,432-byte peak *allocated WASM linear-memory size* in each run, not resident browser/process peak. Chromium and Firefox exercised producer-blocked and consumer-starved ring conditions respectively. The compressed output was fetched only for test inspection after conversion. Both long-case harnesses were rerun after the final code change: Edge cancelled when conversion progress reached at least 10%, Firefox cancelled five seconds after starting a second long conversion; both reported zero application-held frames/samples and successfully converted the 60-frame fixture on the same page. An earlier Firefox test-harness progress matcher failed to trigger (the 2,400-frame conversion itself still passed); the corrected timed-cancellation harness passed. The previous 60-frame fixture, a valid 257.3 MiB sparse-source fixture, cancellation/same-page retry, and full output re-decode passed in both browsers through `tests/chromium-m37-spike.mjs` and `tests/firefox-m37-spike.mjs` after the live-ring change. Codec-failure and injected WebGPU device-loss recovery also passed in both browsers, in dedicated-worker and main-thread execution, with successful same-page retries using `tests/*-recovery-interop.mjs` in FFmpeg mode. Independent `ffmpeg -ss 40` reads of each saved long MP4 succeeded, in addition to FFprobe's stream/packet checks.

M3.8 checks passed: `npm run build:ffmpeg-worker`, `node --check` for the bridge/worker and new browser harnesses, `cargo fmt --all --check`, `cargo test --locked -p media-core -p media-gpu -p media-web` (13 core tests), host and wasm32 `cargo clippy --locked ... -- -D warnings`, `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`, `dx build --web --locked`, and `git diff --check`. The host test run emitted only a linker `.drectve` warning. Browser evidence and FFprobe/FFmpeg output files are under ignored `tmp/m38-*` paths; the reproducible fixture generator and browser harnesses are checked in.

Not yet claimed: full 3,530-frame user recording, high-resolution (>640×360) sustained ring tests, actual browser-process peak resident memory, quota exhaustion on the compressed output, browser crash recovery, Safari/other operating systems and GPUs, production distribution/licensing review, or a no-readback/zero-copy FFmpeg path. Shared-memory isolation may affect third-party assets or embedding and needs deployment-specific review. The separate initial M4 native work is recorded below.

## M4 initial native headless correctness slice — 2026-09-25

At this initial-slice point, M4 had **started but had not passed its acceptance gate**. On Windows 11 Home, `media-native` uses the installed FFmpeg/FFprobe 8.0.1 executable (Gyan essentials build) to compare two MP4/H.264/AAC software-encode routes: direct FFmpeg decode → `scale=...:flags=bilinear` → encode, and FFmpeg raw-RGBA decode → shared Rust `media-gpu::ResizePipeline`/WGSL → RGBA readback → FFmpeg encode. Both use `media-core`'s profile/resize resolution and require a 8-bit, square-pixel, unrotated, zero-origin CFR source with exactly one video and at most one audio track. FFprobe checks decoded frame timestamps against the rational rate before conversion and checks output frame count/timing/codec/geometry/audio before publishing the final file. Output is written to a process-specific partial MP4 and never overwrites an existing output.

The CLI enumerated Intel UHD and NVIDIA GeForce RTX 3050 Laptop GPU on Vulkan/D3D12 (plus Intel GL and Microsoft Basic Render Driver). It records backend, vendor/device IDs, device type, name, and driver; PCI bus identity was not exposed by the selected wgpu API. An explicit adapter key can be saved to an explicit preference JSON file and re-resolved on a later invocation; a stale key produced a visible fallback to a currently available adapter in the test. Updating that preference to Intel/D3D12 and rerunning the 60-frame fixture also passed (1,365 ms conversion, 60 H.264 frames, AAC present), demonstrating exact adapter selection on both available physical GPUs. The GPU route uses one device/queue for its entire job, but live adapter switching, device-generation teardown/recreation, and hardware decoder/encoder surface sharing remain unimplemented.

The checked-in `tests/native-m4-interop.mjs` harness used the CC0 `fixtures/m2-h264-aac.mp4` (640×360, 60 frames, AAC) at 50% size. It selected NVIDIA/Vulkan and passed both routes: each independently inspected output had 60 H.264 320×180 frames, 95 AAC packets, 2.005 s duration, non-silent audio at −17.5 dB maximum, and a successful midpoint seek. The two decoded videos had 36.546647 dB average PSNR; scaling in YUV versus RGBA and lossy re-encoding need not produce identical pixels. The harness also verified saved adapter selection, explicit missing-adapter fallback, VFR/non-zero-origin rejection without an output file, and refusal to overwrite an existing output.

| Fixture and native route | Conversion time (verification excluded) | Explicit frame transfers | Independently checked output |
|---|---:|---:|---|
| 60-frame M2, direct FFmpeg | 205 ms in one harness run; 221 ms in a rerun | none in application | 60 H.264 frames + 95 AAC packets, 320×180, seek/audio pass |
| 60-frame M2, shared wgpu on NVIDIA/Vulkan | 1,000 ms; 835 ms in rerun | 55,296,000 CPU→GPU + 13,824,000 GPU→CPU bytes | same stream counts and seek/audio pass |
| 2,400-frame CC0 synthetic 80 s source, direct FFmpeg | 4,324 ms | none in application | 2,400 H.264 frames + 3,751 AAC packets, 320×180, 80.000 s; seek at 40 s pass |
| Same 2,400-frame source, shared wgpu on NVIDIA/D3D12 | 6,456 ms | 2,211,840,000 CPU→GPU + 552,960,000 GPU→CPU bytes | same frame/audio counts, dimensions, duration, and midpoint seek pass |

The long source is reproducibly generated by `tests/generate-m38-long-fixture.mjs` from FFmpeg `lavfi` color/sine (CC0-1.0) and is only 1,118,263 bytes compressed; it deliberately stresses frame streaming, not high-detail image processing. Independent FFmpeg `volumedetect` reported −17.5 dB maximum for the AAC track in both long outputs. The CC0 `fixtures/m36-h265-aac.mp4` input also passed both routes at original 320×180 size: direct FFmpeg took 134 ms and shared wgpu on NVIDIA/D3D12 took 843 ms; independent FFprobe found 30 H.264 output frames and 48 AAC packets in each. These are debug-build, single-run end-to-end conversion measurements (including device/process startup, excluding preflight and output verification), not a general native CPU-versus-GPU benchmark. The current GPU path synchronously waits for each readback and uses FFmpeg software codecs on both sides, so it is expected to pay for transfers; it does **not** measure a future zero-copy hardware codec path. The direct FFmpeg route was faster in these measured cases only.

Build and tests passed: `cargo fmt --all --check`; `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web` (13 core and 4 native tests); host `cargo clippy --locked -p media-core -p media-gpu -p media-native -p media-web --all-targets -- -D warnings`; browser `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`; `dx build --web --locked`; `node --check tests/native-m4-interop.mjs`; `node tests/native-m4-interop.mjs`; and `git diff --check`. The Windows GNU linker emitted a `.drectve` warning in tests/builds, without failures.

M4 continuation (2026-09-25): Added `CancellationToken`/`convert_with_control`, Ctrl-C handling in `native-convert`, and diagnostic `--cancel-after-ms N`. Both FFmpeg routes now own their child processes through kill-and-reap guards; output remains a partial file until validation and is removed after cancellation. The GPU readback wait checks cancellation between 100 ms polls. An ignored, explicitly invoked integration-style Rust test generates a deterministic 2,400-frame red CFR source with FFmpeg, waits until an FFmpeg partial is opened, cancels both routes during active conversion, asserts no final/partial output, then converts the 60-frame CC0 A/V fixture successfully on both routes **in the same process**. It passed twice on Windows 11/NVIDIA Vulkan. On Windows, immediate partial deletion initially failed with OS error 32 (file in use); a bounded retry of up to 2 seconds made both runs pass. The GPU decoder can print broken-pipe diagnostics on expected cancellation. `node tests/native-m4-interop.mjs` was rerun after the change and passed: direct 221 ms, shared-wgpu 808 ms conversion on the 60-frame fixture, both 60-frame H.264/AAC outputs with non-silent audio, midpoint seek and 36.546647 dB decoded-video PSNR.

Verified cancellation scope at that point: cooperative cancellation between direct-process polls and GPU frames/readback polls, child reaping, partial-output cleanup, and same-process retry under the generated fixture. A later slice below adds blocked-pipe and encoder-open failure tests. Still not tested: physical Ctrl-C in a console, genuine GPU-driver/device failure injection, device generation swapping, and other OS/GPU combinations. The test's generated red source has no audio; its same-process retry uses the CC0 audio/video fixture. Do not interpret these results as general crash- or power-loss cleanup guarantees.

Continuation checks passed: `cargo fmt --all --check`; `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web` (13 core and 4 ordinary native tests); the ignored active-cancellation/same-process-retry test invoked explicitly twice; host `cargo clippy --locked -p media-core -p media-gpu -p media-native -p media-web --all-targets -- -D warnings`; `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`; `dx build --web --locked --package converter-web`; `node tests/native-m4-interop.mjs`; and `git diff --check`. The CLI diagnostic `--cancel-after-ms 200` exited with the cancellation error on the wgpu route and left neither final nor partial output. A plain `dx build --web --locked` from the workspace root failed only because Dioxus CLI requires `--package` when multiple binaries exist; the explicit-package build passed. The pre-existing Windows GNU linker `.drectve` warning remains non-fatal.

At that point, M4 still needed full geometry/color/VFR policies rather than the current fail-fast subset; genuine device-loss recovery; device generation and safe adapter switching; representative high-detail/4K fixtures and release-mode benchmarks; and cross-platform validation. Hardware codec-surface interop and the Dioxus Native/Blitz UI remain M5.

Further M4 failure-recovery slice (2026-09-25): the shared-wgpu route now starts a scoped cancellation watcher for its FFmpeg decoder and encoder. The watcher kills both processes on token cancellation independently of the processing thread, so a blocked decoder pipe read or encoder pipe write can return; process guards then reap both children. A Windows test held a PowerShell child idle with stdout or stdin piped, respectively, cancelled after 200 ms, and observed both blocked operations release within 2 seconds (actual test elapsed 0.44 s for both cases). The active-conversion/same-process-retry test still passed on both FFmpeg routes. It now also injects an encoder output-open failure via a missing output directory on each route and verifies that a subsequent job in the same process succeeds. The normal CC0 A/V native interop harness still passed after the change: 60 frames, AAC, midpoint seek, and 36.546647 dB direct-versus-wgpu decoded-video PSNR; one debug run measured 223 ms direct and 845 ms wgpu/NVIDIA Vulkan. These checks do not simulate an arbitrary child process tree, permanent GPU-driver stall, device loss, or power/process crash. M4 remains incomplete; hardware codec-surface interop and Native/Blitz UI are M5 work.

Headless device-generation slice (2026-09-25): `NativeSession` owns one active cancellation token, preferred adapter key, and monotonically increasing generation. It rejects overlapping jobs; an adapter switch first validates the exact new key, blocks new jobs, cancels and waits for the old conversion to finish cleanup, then advances the generation. Each job constructs and drops its own wgpu device/resources, so cross-generation resource reuse is absent by construction. The ignored lifecycle test passed on this Windows 11 machine with both physical GPUs: it started a long NVIDIA job, rejected an overlapping job, switched to Intel while active, observed the cancelled job leave no final or partial output, and successfully converted the 60-frame CC0 A/V fixture on Intel in generation 2. Switching to the same key leaves generation unchanged. On machines lacking both discrete and integrated adapters, only this cross-GPU section of the test is skipped. This verifies orderly same-process switching under cooperative cancellation, **not** spontaneous driver/device loss or a GPU submission that never completes. The API is headless and does not add a native UI.

Checks for this slice passed: `cargo fmt --all --check`; host `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web` (13 core and 5 ordinary native tests); explicitly invoked ignored lifecycle/switch test; host `cargo clippy --locked -p media-core -p media-gpu -p media-native -p media-web --all-targets -- -D warnings`; `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`; `node tests/native-m4-interop.mjs`; and `dx build --web --locked --package converter-web`. The native harness again verified 60-frame H.264/AAC outputs, non-silent audio, midpoint seek, and 36.546647 dB decoded-video PSNR; its debug conversion times were 221 ms direct and 967 ms NVIDIA/Vulkan wgpu in this run. These timings are observations, not release benchmarks.

At that point, M4 still needed broader geometry/color/VFR handling and fixture coverage; genuine device-loss and stuck-GPU recovery; representative high-detail/4K release-build benchmarks; and validation on other OS/GPU combinations. Hardware codec-surface interop and Dioxus Native/Blitz UI remain M5.

Release-mode high-resolution slice (2026-09-25): `tests/native-m4-release-bench.mjs` builds `native-convert` with `cargo build --release --locked`, then generates H.264/AAC sources using FFmpeg 8.0.1 `testsrc2` plus `noise=all_seed=17:alls=12:allf=t+u` and a 523 Hz sine tone. The script declares these locally generated assets CC0-1.0 and records each exact generator command, SHA-256, stream metadata, OS, Rust 1.98.1, FFmpeg version, and adapter in ignored `tmp/m4/release-bench/run-1790344803290-25784/evidence.json`. No large fixture is checked in. The selected adapter was NVIDIA GeForce RTX 3050 Laptop GPU/Vulkan on Windows 11. The scripts tested 50% output size, fully decoded both MP4 outputs with FFmpeg, checked H.264/yuv420p/AAC 48 kHz, exact frame counts, dimensions, duration within 60 ms, non-silent audio (−12.8 dB maximum), and direct-versus-wgpu decoded-video PSNR.

| Release input → output | Input size | Frames | Direct conversion | Shared-wgpu conversion | Wgpu explicit upload / readback | Decoded-video PSNR |
|---|---:|---:|---:|---:|---:|---:|
| 1920×1080 → 960×540 | 3,781,528 bytes | 60 | 330 ms | 1,436 ms | 497,664,000 / 124,416,000 bytes | 39.71 dB |
| 3840×2160 → 1920×1080 | 8,191,604 bytes | 30 | 587 ms | 2,624 ms | 995,328,000 / 248,832,000 bytes | 40.73 dB |

Those conversion times are the binary's decode→process→encode intervals; they exclude source preflight, output verification, and the harness's independent checks. They are single release-build runs, not statistical benchmarks. The synthetic seeded noise adds high-frequency texture but is not camera footage; the 4K case lasts only one second. Its result confirms that the current wgpu readback route is slower on this machine and workload, not that GPUs or future same-adapter hardware codec paths are generally slower. Real-world high-detail footage, longer 4K runs, actual memory peaks, color fidelity beyond SDR metadata, non-Windows hardware, and driver/device-loss recovery remain untested for M4.

Native timing-policy continuation (2026-09-25): direct FFmpeg now accepts the CC0 `fixtures/m35-vfr-offset.mp4` while the shared-wgpu raw-RGBA route still rejects it before output creation. Input preflight scans every decoded frame's PTS and geometry; direct conversion uses `-copyts -start_at_zero`, `-fps_mode passthrough`, and `-enc_time_base:v 1:90000`; output verification compares every normalized video PTS (1 ms limit), stream durations, and the source/output audio offset before publishing. In the independently checked debug harness run, the 36-frame 320×180 input (video starts 1.250 s, AAC 1.228 s) produced 36 H.264 160×90 frames with AAC, full re-decode and non-silent audio. The maximum per-frame timeline error and A/V offset error were effectively 0 µs at FFprobe's six-decimal precision. The GPU route's rejection remained explicit. The earlier M4 harness result describing VFR rejection applied before this route-specific change. Direct support does not imply GPU VFR support, negative-origin/edit-list support, or multi-track preservation.

The user-provided local `tmp/user-test/Input.mp4` was also tested without copying it into tracked files. FFprobe reports 1920×1080 H.264 at 24000/1001 fps, 3,530 frames, AAC, and 147.284 s duration. The release binary converted the full source at 50%: direct FFmpeg took 11,724 ms and shared-wgpu on NVIDIA/Vulkan took 39,650 ms, excluding preflight/output verification. The GPU route counted 29,279,232,000 CPU→GPU bytes and 7,319,808,000 GPU→CPU bytes. Independent FFprobe found 3,530 H.264 960×540 frames, 6,905 AAC packets, and 147.284 s duration in both outputs. Both fully decoded without FFmpeg errors, preserved non-silent audio (0.0 dB measured maximum), and passed a seek to 70 s. After aligning the two output video streams by frame index and a common time base, decoded-video PSNR was 38.368473 dB. A naive PSNR comparison without time-base alignment gave a misleading lower result and an FFmpeg warning; it is not used as evidence. This is one private user sample on one OS/GPU, not a reproducible public fixture or a throughput distribution; its outputs remain under ignored `tmp/m4`.

The post-change release benchmark also passed on the same seeded public synthetic inputs: 1080p direct/wgpu conversion took 325/1,391 ms and 4K direct/wgpu took 561/2,562 ms, with the previously recorded 39.71/40.73 dB route-comparison PSNR. Its latest ignored evidence is under `tmp/m4/release-bench/run-1790346064932-25004`. Final checks passed: `cargo fmt --all --check`, host `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web` (13 core, 6 ordinary native tests), the explicitly invoked native lifecycle test, host `cargo clippy --locked -p media-core -p media-gpu -p media-native -p media-web --all-targets -- -D warnings`, browser `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`, `dx build --web --locked --package converter-web`, `node --check tests/native-m4-interop.mjs`, and `node tests/native-m4-interop.mjs`. The Windows GNU linker still emits a non-fatal `.drectve` warning.

Native geometry/color continuation (2026-09-25): the direct FFmpeg route converted the checked-in CC0 `fixtures/m35-geometry-color.mp4` through the native CLI and independent debug interop harness. FFprobe reports a 316×180 stream, 4:3 sample aspect ratio, a reflected −90° display matrix, BT.709 limited-range tags, 24 H.264 frames, and AAC; decoded-frame inspection reports a stable 318×180 aperture, so direct preflight now compares decoded dimensions across frames rather than equating them to the stream header. `media-core::FrameGeometry` resolves the stream's 180×421 oriented display size; a 50% resize produced 90×210 H.264 with square pixels, no display matrix, all four BT.709/range tags, 24 frames, and AAC. Full FFmpeg decode passed. First-frame decoded RGB marker samples were TL [255,23,0], TR [0,14,255], BL [0,107,0], BR [255,238,0], matching expected red/blue/green/yellow order. The shared-wgpu route explicitly rejected this fixture before output creation. This verifies the fixture's orientation and representative markers, not universal display matrices, crop variants, or colorimetric accuracy; the flip is applied by FFmpeg autorotation, not reconstructed in Rust.

The post-change `node tests/native-m4-interop.mjs` passed twice on Windows 11 with the NVIDIA Vulkan adapter, including its prior CFR/VFR, audio, decode, PSNR, and rejection checks. `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web`, host matching-target Clippy with `-D warnings`, browser-target `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`, `node --check tests/native-m4-interop.mjs`, and formatting passed. The native geometry change was not separately run in a browser; M3.5's browser fixture results above remain the browser evidence. The existing non-fatal Windows GNU linker `.drectve` warning remains.

Native color-policy continuation (2026-09-25): both routes now preflight for 8-bit opaque YUV 4:2:0 and BT.709 limited-range SDR (or unspecified tags), rejecting other pixel formats, full-range, wide-color, and HDR tags before creating a partial output. For the shared-wgpu raw-RGBA path, FFmpeg output flags alone omitted transfer and primaries in a local probe; adding libx264 VUI parameters retained them. `node tests/native-m4-interop.mjs` converted the checked-in CC0 `fixtures/m36-h265-aac.mp4` through both direct and NVIDIA/Vulkan wgpu routes to 30-frame 160×90 H.264/AAC. FFprobe found `tv` range and `bt709` matrix, transfer, and primaries on both outputs; midpoint decode passed. Decoded-video direct-versus-wgpu PSNR on this tagged fixture was 32.184355 dB. The checked-in CC0 `fixtures/m35-hdr-tagged.mp4` failed preflight on both routes with `unsupported matrix: bt2020nc`, leaving no output. This is verified tag preservation and route comparison, not a measured ΔE/colorimetric comparison or proof that untagged inputs are actually BT.709.

After the color change, host `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web` passed (13 core and 7 ordinary native tests); `cargo clippy --locked -p media-core -p media-gpu -p media-native -p media-web --all-targets -- -D warnings`, `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`, `cargo fmt --all --check`, `node --check tests/native-m4-interop.mjs`, and the expanded native interop harness passed. The ignored, explicitly invoked same-process cancellation/failure/retry test also passed; its FFmpeg broken-pipe/output-open messages are expected from injected failures. No real browser runtime test was needed for this native-only change, and no non-Windows native run or hardware device-loss test was performed.

Native Main 10 continuation (2026-09-25): `mp4-h265-main10-aac` is now a direct-only native profile. `tools/generate-m4-10bit-fixture.ps1` generated a checked-in 147,840-byte CC0 HEVC Main 10/AAC source (`fixtures/m4-10bit-sdr.mp4`, SHA-256 `7e4ab455163e60d140ac727e1483be6d19e9003a2dced5aca199ccbfe9342cfb`) with 24 160×96 `yuv420p10le` frames, deterministic 10-bit Y/U/V gradients, and BT.709 limited-range tags. A 50% direct conversion produced 24 80×48 HEVC Main 10 `hvc1`/`yuv420p10le` frames with AAC and all four source color tags; FFmpeg fully decoded the result, midpoint seek and frame-time checks passed, audio was non-silent (maximum −16.4 dB), and 2,877 of 3,840 first-frame output luma samples were not multiples of four. This establishes a 10-bit encoded output with sub-8-bit-step decoded samples, not lossless preservation after CRF 20 encoding. The native harness confirmed the GPU route rejects the Main 10 profile before output creation and the 8-bit H.264 profile rejects the 10-bit input rather than silently reducing depth. Browser 10-bit output and HDR are unimplemented and untested.

Post-change checks passed: host `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web` (13 core, 8 ordinary native tests), matching-target host and wasm Clippy with `-D warnings`, wasm-target `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`, `dx build --web --locked --package converter-web`, `cargo fmt --all --check`, `node --check tests/native-m4-interop.mjs`, and the expanded `node tests/native-m4-interop.mjs` on Windows 11/NVIDIA Vulkan. The fixture generator produced the same SHA-256 on a second run. An optimized native binary also converted the 10-bit fixture successfully in a single run (24 frames, 80×48, 23,064-byte output, 301 ms conversion interval excluding source preflight and output verification); independent FFprobe confirmed Main 10/hvc1/yuv420p10le, AAC, and BT.709 tags. This was not a sustained-throughput benchmark. Browser runtime behavior was not tested for this native-only profile, and no browser selector exposes it.

Post-M4 limitations outside the supported Windows headless subset: broader geometry/display-matrix and color policy, negative origins and unusual edit lists, VFR and transformed geometry support through the GPU route (which needs richer frame ingress), genuine device-loss/stuck-GPU recovery, longer high-resolution memory/throughput measurements, and other operating systems/GPUs. Native UI and hardware codec-surface sharing remain M5.

Controlled native GPU-loss continuation (2026-09-25): each wgpu job now registers `Device::set_device_lost_callback`; the callback stores its reason and signals the FFmpeg child watcher without making reentrant wgpu calls. The processing loop checks that state before frame upload, during readback polling, and around pipe reads. An explicitly invoked ignored Rust test destroyed the NVIDIA/Vulkan device after one processed frame of the 60-frame CC0 H.264/AAC fixture. The job returned `native GPU device lost (Destroyed)`, left neither final nor partial MP4, and a fresh GPU job in the same process completed all 60 frames and passed output probing. The test passed twice; FFmpeg's broken-pipe diagnostics on the deliberately aborted decoder are expected. This is a controlled API-triggered `Destroyed` loss, **not** a physical-driver failure, `Unknown` loss, or a permanent GPU stall. The test does not prove automatic checkpoint recovery; it proves fail-and-restart with new job resources.

After the final watcher/readback cleanup change, the controlled loss/retry test and `node tests/native-m4-interop.mjs` passed again. Host `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web` (13 core, 8 ordinary native tests), host Clippy with `-D warnings`, `cargo fmt --all --check`, and wasm-target `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown` passed. The injected loss test is ignored in normal `cargo test` because it requires FFmpeg/FFprobe and a native wgpu adapter; it was run explicitly on NVIDIA/Vulkan. No real browser runtime or non-Windows native test was performed for this native-only change.

Decision following these measurements: direct FFmpeg is the native CLI default for resize-only and other effect graphs it can express with acceptable parity. Retain the shared-wgpu route as an explicit alternative for custom effects shared with the web build or GPU-heavy chains, but require representative release-build end-to-end and correctness evidence before promoting it to a default. This is a route default, **not** an effect-aware automatic selector or native UI: the CLI requires `--route wgpu` to select the comparison route, and the present native GPU route still pays one upload and one readback per frame. See the M4 route decision in `docs/architecture.md`.

M4 scope closure (2026-09-25): the requested Windows headless native correctness milestone is complete for the tested direct-FFmpeg subset. The CLI now defaults to direct FFmpeg when `--route` is omitted; the native interop harness exercises this default on the checked-in VFR/non-negative A/V-offset fixture and passed. Direct output also passed the checked-in CFR A/V, crop/SAR/rotation-plus-flip SDR, tagged HEVC/BT.709 SDR, and 10-bit SDR HEVC Main 10/AAC cases, with the independent decode, frame/timing/geometry/color-tag, seek, and audio checks detailed above. Shared wgpu remains an explicit 8-bit zero-origin CFR comparison route; its rejected VFR, transformed-geometry, and Main 10 cases are not silently rerouted. Cancellation, injected device-loss cleanup/retry, and same-process adapter switching passed the focused Windows tests already recorded above. A final single-run release regression with seeded synthetic sources passed: 1080p/60 frames took 299 ms direct versus 1,323 ms wgpu (39.71 dB decoded-video PSNR); 4K/30 frames took 573 ms direct versus 2,659 ms wgpu (40.73 dB PSNR). These conversion intervals exclude source preflight and output verification. Real camera footage at sustained 4K, physical driver loss or permanent stalls, negative-origin/edit-list cases, broader HDR/color/geometry policy, and other OS/GPU combinations remain untested. M5 native UI and codec-surface interop have **not** begun.

Final M4 closeout checks passed: `node tests/native-m4-interop.mjs`, `node tests/native-m4-release-bench.mjs`, host `cargo test --locked -p media-core -p media-gpu -p media-native -p media-web` (13 core and 8 ordinary native tests; two environment-dependent native tests remain ignored in the default run and were explicitly run earlier), host `cargo clippy --locked -p media-core -p media-gpu -p media-native -p media-web --all-targets -- -D warnings`, `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown`, `cargo fmt --all --check`, `node --check tests/native-m4-interop.mjs`, and `git diff --check`. Rust test builds emitted a non-fatal Windows GNU `.drectve` linker warning. This closeout was native-only; no new browser runtime validation was performed.

## M5 native UI first slice — 2026-09-25

Implemented, not yet an M5 acceptance pass: `apps/native` uses Dioxus 0.7.10's `native` feature and Blitz renderer (not the `desktop` webview). It exposes source/output MP4 paths plus native file dialogs, source metadata inspection, the shared web/native resize preset selector, H.264/AAC and direct-only HEVC Main 10/AAC profiles, direct FFmpeg versus explicit shared-wgpu route, GPU adapter descriptors and preference switching, Convert, Cancel, and completion/error status. Conversion runs on a worker thread through the M4 `NativeSession`; reactive state carries only paths, metadata, selections, and status. A direct-route source probe was added so VFR/non-zero-origin and the tested crop/SAR/display-matrix fixture can be inspected in this UI without inheriting the comparison GPU route's CFR/square-pixel restriction. A new Rust test passed on both deterministic fixtures. The native GPU selector has no effect on direct FFmpeg and is not a hardware codec selector; this is stated in the UI/documentation.

Verified on Windows 11 with Rust 1.98.1 GNU: `cargo check -p converter-native`, `cargo build --locked -p converter-native`, `cargo test --locked -p media-core -p media-gpu -p media-native -p ui -p converter-native` (13 core, 9 ordinary native; two environment-dependent M4 tests ignored by the default run), host Clippy for those packages with `--all-targets -- -D warnings`, and `cargo check --locked -p media-web -p converter-web --target wasm32-unknown-unknown` passed. The native link emitted the previously observed non-fatal GNU `.drectve` warning. The Windows UI-control connection failed during setup (`failed to write kernel assets: The system cannot find the path specified`, repeated once), so window rendering, file-dialog interaction, Convert/Cancel clicks, and GPU switching through the new UI are **not yet runtime-verified**. The underlying headless conversion, cancellation, adapter switch, and direct-FFmpeg output checks remain the separate M4 evidence above. No native video preview or decoder/encoder hardware-surface bridge has been implemented; Blitz's wgpu 26 and `media-gpu`'s wgpu 30.0.1 resources are not interchangeable. No M5 throughput or same-adapter codec-surface claim is made.

The shared selector also passed `dx build --web --locked --package converter-web`; `cargo fmt --all --check` and `git diff --check` passed. This is a browser bundle check, not a new browser runtime interoperability run. Native UI GPU preference is session-local in this slice; preference-file persistence, close-window cancellation, native preview, hardware surfaces, and complete M5 conversion benchmarks remain follow-up work.

## M5 native UI correction and themes — 2026-10-02

Scope: repair the native window and add dark mode, not complete the hardware/preview portion of M5. Computer Use now connected successfully and captured the actual Dioxus 0.7.10/Blitz window on Windows 11. The baseline was an unstyled form with HTML `select` options rendered consecutively inline. Native selections now use buttons with clear selected states; the browser retains its dropdown and shares the same preset vocabulary. Native controls omit `disabled` when false because Blitz interprets a present `disabled="false"` as disabled. The embedded stylesheet groups files, output settings, processing and job status; the window has an application title, dark-by-default styling, and a light/dark toggle. GPU options are shown only for the shared-wgpu route, and Main 10 is disabled there.

Verified in the live native window using Computer Use:

- Readable dark and light palettes; theme switching in both directions; initial Original size selection and selecting 50%.
- Source Browse opened the Windows Open dialog and selected the checked-in CC0 `fixtures/m2-h264-aac.mp4`. Inspect displayed 640×360, H.264, yuv420p, 60 frames, AAC. Output Browse opened Save As and selected a fresh output path.
- Inspect was disabled without an input and enabled after selection; Convert was disabled without an output. During conversion, settings became disabled, the primary label changed to Converting, and Cancel became enabled. Completion re-enabled settings and displayed the result.
- Direct-FFmpeg H.264/AAC conversion through the UI completed at 50%: 60 frames, 320×180, 78,724 bytes, reported conversion interval 914 ms (not a throughput benchmark). Output: ignored `tmp/m5-ui-20261002-direct.mp4`. Independent `ffprobe -count_frames` confirmed 60 H.264 video frames and an AAC stream (94 decoded audio frames); `ffmpeg -v error -i ... -f null -` fully decoded video/audio with exit 0 and no errors.
- Selecting Shared GPU displayed enumerated adapter choices and visibly disabled Main 10. Clicking NVIDIA GeForce RTX 3050 Laptop GPU / Vulkan updated its selected state. This proves the selection interaction, not execution on that GPU in this UI validation session.

Build/test evidence: `cargo build --locked -p converter-native`; host `cargo clippy --locked -p media-core -p media-gpu -p media-native -p ui -p converter-native --all-targets -- -D warnings`; `cargo test --locked` for those packages (13 core and 9 ordinary native tests, two environment-dependent native tests ignored); `cargo check --locked -p ui -p converter-web --target wasm32-unknown-unknown`; `cargo fmt --all --check`; and `git diff --check` passed. The previously documented non-fatal GNU `.drectve` linker warning remains. No dependency/toolchain versions changed.

Still untested in this UI session: an actual shared-GPU conversion, clicking Cancel during an active job, Main 10 conversion, exhaustive presets/adapters, narrow-window/HiDPI/keyboard-accessibility behavior, and window-close cleanup. Repeated Computer Use input-interruption guards prevented completing additional output-editing tests; manual path typing is not claimed verified. Earlier M4 headless lifecycle/adapter/codec tests remain separate evidence, not substitutes for these UI tests. No new browser runtime test was performed (WASM compilation only). Theme and adapter preference are session-local; automatic OS-theme selection/persistence, native video preview, hardware codec-surface interoperability, and a full M5 acceptance pass remain unavailable/pending.

## M5 native lifecycle, preference persistence and route limits — 2026-10-02

Implemented: the native launcher calls permanent `NativeSession::shutdown` after the last window closes. Shutdown rejects delayed worker starts, signals cancellation, and waits for active conversion cleanup. A UI-owned cancellation token covers Cancel before conversion worker registration. GPU descriptor preference is saved in the per-user app-data directory, or a `DIAXUS_GPU_PREFERENCE` override, and re-resolved on launch; unavailable/corrupt preferences produce a visible warning and Automatic fallback. Theme remains session-local. These are conversion lifecycle changes; independent source-inspection/file-dialog workers are not yet part of the session shutdown gate.

Two real Windows failures were found and fixed during validation:

- The encrypted application-data directory rejected same-parent preference rename with Windows error 17. Preference saving now falls back to copy/remove for that error only; this fallback is **not atomic** and is not used to publish media. The UI reports save failures without misrepresenting a session-only choice as persistent. Saving the NVIDIA/Vulkan descriptor succeeded after the fix, and a fresh app launch displayed Saved GPU preference restored.
- Cancelling a shared-GPU job using Chocolatey's FFmpeg executable shims initially left a 55,630,450-byte partial MP4. Killing a shim did not reliably stop its real codec descendant. Windows child cleanup now invokes `taskkill /PID <owned live child> /T /F` without a console window, then reaps the owned child. A regression test starts a launcher with a descendant holding stdout open and proves cancellation releases the pipe promptly. The failed test's exact partial file was removed after confirming no codec processes remained; the input was unchanged.

Verified with Computer Use in real Windows 11/Dioxus 0.7.10/Blitz windows:

- Manual input/output path typing worked. Shared GPU with NVIDIA GeForce RTX 3050 Laptop GPU / Vulkan converted the CC0 `m2-h264-aac.mp4` fixture at 50%: 60 frames, 320×180, 94,331 bytes, reported 11,016 ms conversion interval, output `tmp/m5-next-gpu.mp4`. The UI reported the actual NVIDIA/Vulkan processing adapter. Independent FFprobe counted 60 H.264 frames and an AAC stream (94 decoded audio frames); full FFmpeg decode exited 0. This debug-run interval is not a release benchmark or proof of hardware codecs.
- After the process-tree fix, Cancel during a direct job's source preflight using the user-provided 147-second 1080p `tmp/user-test/Input.mp4` returned cancelled, re-enabled controls, and left no FFmpeg/FFprobe processes or final/partial output. Same-window retry started successfully.
- On that retry, direct encoding created `tmp/m5-next-close.<pid>.partial.mp4`; both the FFmpeg shim and its actual descendant were observed. Closing the window during encoding terminated the converter and both codec processes and removed the partial file. No final output was published.
- Direct Main 10 at 50% converted the CC0 `m4-10bit-sdr.mp4` fixture through the UI: 24 frames, 80×48, 23,064 bytes, reported 222 ms conversion interval, output `tmp/m5-next-main10.mp4`. FFprobe confirmed HEVC Main 10/hvc1/yuv420p10le, limited-range BT.709 matrix/transfer/primaries, AAC (47 decoded audio frames); full FFmpeg decode exited 0. Shared GPU was visibly disabled for the Main 10 profile.

Route clarification prompted by a user test: Direct FFmpeg still uses software codecs and CPU scaling. Shared GPU uses wgpu for resize only, with software codecs and one upload/readback per frame; GPU preference is not an NVENC/NVDEC selector. The user's selected recording had video `start_time=0.060033`, audio `start_time=0`, nominal rate `60000/1001`, and average rate `257460000/6874523`. The current shared bridge rejects non-zero-origin/VFR sources by design, irrespective of NVIDIA Vulkan versus Dx12 selection. Do not normalize away timestamps or silently drop frames to claim support. Inspection now reports known GPU-route restrictions and disables its choice for incompatible sources; the zero-origin backend error includes the actual origin and suggests Direct FFmpeg. Full conversion preflight remains authoritative; passing the UI metadata check is not a proof that a job will succeed.

Checks passed: native build; host Clippy for core/GPU/native/UI/app packages with `--all-targets -- -D warnings`; native/app tests (13 ordinary native, one app preference test, two environment-dependent native tests ignored by default); core/GPU/UI tests (13 core tests); WASM check for media-web/converter-web; formatting and diff whitespace checks. The ignored `cancel_active_jobs_and_retry_in_same_process` test was explicitly run and passed, including direct/GPU cancellation, failure/retry and adapter switching. Expected FFmpeg abort/missing-output diagnostics were emitted by that harness. The known non-fatal GNU `.drectve` linker warning persists. No new browser runtime test or dependency upgrade was performed.

Final capability UI validation: the rebuilt app inspected the user's selected recording, displayed its 1920×1080/H.264/yuv420p/8,582-frame/AAC metadata, disabled Shared GPU, and showed the precise 60,033 µs origin restriction and Direct FFmpeg alternative before conversion. The software-codec labels rendered correctly. Source/output paths and 50% selection were restored after rebuilding; no new conversion of that recording was started. This is inspection/gating evidence, not GPU support for that source.

Still pending: comprehensive narrow-window/HiDPI/keyboard-accessibility tests, independent inspection/dialog worker shutdown, spontaneous GPU-driver loss/stalls, hardware codec acceleration/surface sharing, native video preview and full end-to-end hardware benchmarks. The GUI shared-GPU conversion evidence predates the final capability-label change; the underlying conversion pipeline is unchanged. This continuation does not complete all of M5.

## M5 — NVIDIA hardware route and shared-GPU VFR/origin fix — 2026-10-02

This section supersedes the preceding claim that hardware codecs and shared-GPU non-zero/VFR timing are unimplemented. Preview and codec-surface sharing **with wgpu/Blitz** remain unimplemented.

Environment: Windows 11, Rust 1.98.1 GNU / edition 2024, Dioxus Native/Blitz 0.7.10, processing wgpu 30.0.1, FFmpeg/FFprobe 8.0.1, NVIDIA GeForce RTX 3050 Laptop GPU (vendor 10de, device 25a2), driver 616.64. No dependency upgrades or OS/driver changes were needed.

Implemented:

- Explicit `NvidiaFfmpeg` route: CUDA decode with CUDA output surfaces, `scale_cuda` bilinear resizing, H.264 or HEVC Main 10 NVENC, CPU AAC. Software FFmpeg remains a separate selectable route and the CLI default; UI initially selects NVIDIA when a device is discovered. The report names the NVIDIA device and actual NVENC encoder. No silent hardware-to-CPU fallback.
- Selected wgpu NVIDIA vendor/device/name resolves to one NVIDIA UUID, scoped through the child process's `CUDA_VISIBLE_DEVICES`; no use of wgpu enumeration index as a CUDA ordinal. Non-NVIDIA, unavailable and ambiguous device selections fail. NVIDIA Vulkan and Dx12 selector entries on this single-board system resolve to the same physical NVIDIA GPU.
- Shared-wgpu output uses a sequential, private Matroska RGBA stream with microsecond PTS and explicit durations, written directly to encoder stdin. GPU readback rows follow each frame header without another full output pixel allocation or raw disk persistence. The existing preflight holds frame-timestamp metadata, not all pixel frames. Both tracks use one integer origin; all routes re-decode/check every output PTS and A/V start/duration before publishing. VFR is neither resampled nor dropped.
- Codec stderr is drained concurrently with an 8 KiB retained diagnostic tail, so failure details reach the UI without an unbounded log queue or a blocked stderr pipe.

Verified deterministic runtime cases:

| Test | Result |
|---|---|
| Updated `node tests/native-m4-interop.mjs` | Passed existing software/shared correctness, color/geometry/HDR/depth checks and new shared VFR/offset conversion. Shared CFR: 60 frames, 320×180; shared VFR: all 36 frames, 160×90, PTS/A/V offset within 1 ms. |
| `node tests/native-m5-hardware.mjs` | Passed real CUDA/NVENC CFR H.264 (60 frames), VFR/offset H.264 (36), HEVC input→H.264 (30), and Main 10 HEVC (24); complete independent decode and non-silent AAC output on every case. Every independently decoded PTS matched its normalized input to the FFprobe display precision. |
| Hardware image/depth checks | H.264 decoded-video PSNR against software reference: 35.855602 dB. Main 10 output: HEVC Main 10/hvc1/yuv420p10le with over 1,000 first-frame luma samples not restricted to 8-bit steps. This is one synthetic fixture, not universal colorimetric fidelity. |
| Hardware rejection/cleanup | Missing and Intel adapter keys rejected; no final output. This GPU rejects the tiny 160×96 HEVC source below its 144-pixel NVDEC minimum height; diagnostics were returned, no software fallback, no final/partial output. Successful Main 10 used a 320×192 derivative of the CC0 fixture, generated under ignored `tmp` by the harness. |
| Native UI, Computer Use | Inspected the non-zero-origin VFR fixture; Shared GPU remained enabled and completed 36 frames at 160×90 on the saved NVIDIA Dx12 adapter (63,347 bytes, 1,338 ms). Same-window switch to Direct FFmpeg · NVIDIA completed 36 frames (73,334 bytes, 332 ms); status named NVIDIA / CUDA / h264_nvenc. Metadata, selected route/device and busy controls were visible. |

Full user recording regression: `tmp/The Last of Us™ Part II 2025-06-26.mp4`, 1920×1080 H.264/yuv420p, 8,582 VFR frames, video origin 60,033 µs, AAC origin zero. Both CLI routes completed at 50% without the old zero-origin error:

| Route | Output under ignored `tmp` | Frames / size | Video conversion interval | Separate verification |
|---|---|---|---|---|
| CUDA/NVENC | `m5-fixed-user-nvidia.mp4` | 8,582 / 960×540 / 51,863,801 bytes | 10,837 ms | 19,783 ms |
| Shared WGSL, NVIDIA Vulkan | `m5-fixed-user-wgpu.mp4` | 8,582 / 960×540 / 20,446,990 bytes | 113,399 ms | 7,745 ms |

Both passed built-in full decoded-timeline verification. Independent FFprobe confirmed 8,582 video frames, video start 0.060000 s (33 µs MP4 start rounding), video duration 229.150767 s, AAC start zero and duration 229.141000 s. Independent complete FFmpeg decoding passed using `-fps_mode passthrough -enc_time_base:v demux -f null -`. A first diagnostic using the null sink's default time base logged duplicate DTS caused by VFR quantization; repeating with the demux time base produced no errors. This is not an MP4 packet/timeline corruption claim. The shared route counted 71,182,540,800 uploaded and 17,795,635,200 read-back pixel bytes; NVIDIA route counted no application-managed raw pixel transfers. Neither claim describes hidden driver copies. Runs overlapped other validation and used debug builds; these times are not controlled release benchmarks or quality/bitrate-equivalent comparisons. CPU preflight time is outside the video conversion interval; source/output inspection and audio still use CPU.

Checks passed: formatting; host Clippy for core/GPU/native/UI/native app with all targets and warnings denied; 16 ordinary native tests, one native-app preference test, 13 core tests; both ignored native cancellation/switch/retry and injected-device-loss tests explicitly executed and passed; native build; WASM check for media-web/converter-web. Non-fatal GNU `.drectve` linker warnings remain. Browser runtime behavior was not retested because this change is native-only.

Still untested/unsupported: other OS/GPU/driver combinations; Intel/AMD hardware codec routes; multiple identical NVIDIA boards without PCI/UUID selector identity; exact hardware capability pre-probing for every input/output configuration (conversion fails visibly instead); NVIDIA/shared-wgpu display transforms and shared-wgpu Main 10; negative origins, HDR/wide/full-range color and multiple audio tracks; wgpu/native hardware-surface sharing, preview, complete release performance/resource profiling and spontaneous driver loss. These fixes do not mark all of M5 complete.

### M5 inspection reuse, stage reporting and CPU-staged NVIDIA/wgpu bridge — 2026-10-02

Implemented a fourth explicit route, `wgpu-nvidia` / **Shared GPU · NVIDIA**, combining FFmpeg NVDEC, the existing shared WGSL processor, and H.264 NVENC on a uniquely matched NVIDIA GPU. It is not hardware-surface interoperability: FFmpeg downloads NV12 and converts to CPU RGBA, wgpu uploads/readbacks RGBA, and FFmpeg converts to NV12 and uploads to CUDA for encoding. Audio, color conversions and full verification still use CPU. The original software-codec shared route remains explicitly selectable. Hardware failures do not silently fall back to it.

NativeSession now caches one successful input metadata/timestamp inspection, not pixels. Canonical path, file length, modification and creation times must match before reuse; changes invalidate the cache and the source is checked again before publishing. This is ordinary file-change detection, not protection against deliberately forged metadata. The first inspection and each fresh CLI process still perform full CPU inspection; cache reuse is session-local and never skips full output verification. Registered inspection jobs participate in cancellation and shutdown. Five job-stage events drive UI labels, and reports separate total, preflight, conversion-only and verification time.

Verified on Windows 11, FFmpeg/FFprobe 8.0.1, NVIDIA RTX 3050 Laptop (driver 616.64), shared wgpu 30.0.1 / Vulkan:

- `node tests/native-m5-hardware.mjs` passed seven independently decoded cases: direct NVIDIA CFR (60 frames), VFR/offset (36), HEVC input (30), HEVC Main 10 (24); shared NVIDIA CFR (60), VFR/offset (36), HEVC input (30). All preserved complete decoded timelines, AAC audio and checked source color tags. Shared decoded-video PSNR against a software resize reference was 36.304374 dB (one fixture, not universal color fidelity).
- Validation initially rejected missing `color_transfer` at the shared CUDA upload boundary. Explicit source `setparams` after upload and output color tags fixed it; the repeated fixture suite passed. Unsupported hardware configurations remain explicit errors with both codec diagnostic tails.
- The hardware harness cancelled shared NVIDIA after a partial output had actually started, found no final/partial output left behind, and successfully retried all 36 VFR frames. Missing/wrong adapter and unsupported hardware cases still reject without CPU fallback.
- Full user recording (`tmp/The Last of Us™ Part II 2025-06-26.mp4`): shared NVIDIA preserved all 8,582 VFR frames and AAC at 960×540. Output `tmp/m5-staged-user-nvidia.mp4`, 54,172,730 bytes; cold preflight 45,721 ms, conversion 79,175 ms, verification 9,922 ms, total 134,819 ms. Built-in full decoded timeline/color/A/V checks and independent complete FFmpeg decoding passed. Cold inspection remains a substantial CPU cost; this measurement overlapped other work and is not a controlled benchmark.
- During that actual shared conversion, one `nvidia-smi` sample reported 9% encoder, 15% decoder and 16% GPU utilization. Both NVIDIA codec engines were active; these are sampled values, not peaks or guarantees. Logical codec NV12 downloads/uploads totaled 26,693,452,800 / 6,673,363,200 bytes, separately from wgpu RGBA uploads/readbacks of 71,182,540,800 / 17,795,635,200 bytes. These counters do not measure hidden driver copies.
- Native UI via Computer Use: after Inspect on that recording, Direct FFmpeg · NVIDIA at 50% reused inspection and completed all 8,582 frames at 960×540. The UI visibly showed Preparing, disabled settings during work, restored controls and reported **21,224 ms total, 438 ms inspection/setup, 10,859 ms conversion, 9,926 ms verification**, `Inspection reused`, and the actual NVIDIA / CUDA / h264_nvenc identity. Output: `tmp/m5-cache-ui-user-nvidia.mp4` (51,863,801 bytes). Ordered delivery of all five stages is also covered by a session integration test; the brief Verifying/Publishing labels were not individually captured in this UI run.

Checks passed: formatting; host all-target Clippy with warnings denied for core/GPU/native/UI/native app; 18 ordinary native tests and one native-app preference test; both ignored cancellation/switch/retry and injected-device-loss tests explicitly run; native build; updated M4 real-FFmpeg interoperability harness; WASM check for media-web/converter-web. New cache tests verify reuse, invalidation, cancellation, session lifecycle, ordered stages and timing totals. Non-fatal GNU linker `.drectve` warnings remain. Browser runtime was not retested for these native-only changes.

Still untested/unsupported: zero-copy hardware-surface sharing, shared NVIDIA Main 10/HDR/display transforms, other hardware/drivers/OS, spontaneous driver loss, controlled release speed/quality/resource comparisons and persistent cache across restarts. First-input inspection and final output verification remain CPU work; shared NVIDIA still has four explicit device/host transfer boundaries. This does not mark all of M5 complete.

### M5 compact native layout — 2026-10-02

UI-only change: tighter spacing/buttons, inline file labels, compact header, side-by-side settings, selected-GPU summary and an in-window adapter picker. Default client size is now 960×760 logical pixels (minimum 900×740). All resize/profile/route controls, GPU summary, Convert/Cancel and ordinary status remain visible without main-page scrolling. Very long diagnostics use a bounded local scroll area; an unusually large GPU list can scroll within its picker rather than stretching the main layout. No codec or processing implementation changed.

Verified with Computer Use in the real Windows Dioxus 0.7.10 Native/Blitz window at the default size: dark and light themes, all main controls without scrolling, route switch to shared software codecs, seven choices visible together in the GPU picker, picker dismissal, source inspection/metadata, disabled settings plus visible Cancel during work, and full completion report. Initial CSS-grid file placement rendered incorrectly in Blitz; explicit flex file-row wrappers corrected it before final validation.

The checked-in VFR/offset fixture completed via shared WGSL / NVIDIA Dx12 / libx264 with all 36 frames at original 320×180 and AAC audio: `tmp/m5-compact-ui.mp4`, 94,503 bytes, inspection reused, 951 ms total (772 ms conversion, 177 ms verification). The native runner's full timeline/color/audio verification and an independent complete FFmpeg A/V decode passed. This validates the UI integration, not a new performance comparison or hardware-codec claim.

Checks passed: native build, native-app preference test, formatting, native-app all-target Clippy with warnings denied, and Git whitespace check. Existing non-fatal GNU `.drectve` linker warning remains. Untested: minimum-size live resizing, other DPI/font scaling/screen sizes, keyboard focus trapping in the picker, exceptionally large adapter lists and long-error scrolling. Browser runtime and the broader codec matrix were not rerun for this native UI-only change.

### M5 native frame-preview presenter baseline — 2026-10-02

Implemented source Preview and Preview last output after successful verification, with a seek position in seconds and an in-window GPU canvas. This is a **static SDR frame inspector, not playback/audio**. It uses a separate FFmpeg preview decode, never conversion frame resources. One 640×360 RGBA8 frame is 921,600 CPU bytes with one presenter upload on a new frame revision; Vello performs an additional GPU atlas copy when drawing. No preview pixel disk spool, no frames/GPU handles in Dioxus signals, no changes to conversion's frame-transfer path. One worker and CPU frame slot plus one reusable presenter texture are application-owned bounds, not guarantees about FFmpeg/driver memory.

API verification: local released dioxus-native 0.7.10 `use_wgpu` / `CustomPaintSource`; anyrender_vello 0.6.2 and Vello 0.6.0 registered-texture API. Vello requires Rgba8Unorm plus COPY_SRC and copies into its atlas; the first live test without COPY_SRC terminated with wgpu validation `Encoder is invalid`. After adding the required usage, repeated source/output presentation and seeking succeeded. The app adds an exact wgpu 26.0.1 alias for Blitz's existing renderer ABI only; processing stays wgpu 30.0.1, and no handles cross those boundaries. Preview uses Blitz's supplied renderer device, which need not be the user's selected conversion GPU.

Verified on the existing Windows 11 / Dioxus Native/Blitz / FFmpeg 8.0.1 setup:

- Computer Use: real source canvas displayed the VFR fixture's first frame; seeking 0.5 seconds visibly changed the burnt-in timecode from 00:00:00.000 / frame 0 to 00:00:00.500 / frame 15. Closing returned to the compact main window; opening output preview reused the presenter and displayed the resized encoded output. Representative captures showed the expected test pattern/colors; sustained pixel-exact compositor correctness is not established by screenshots.
- Same-window direct NVIDIA conversion after source preview: all 36 VFR frames, 160×90, 73,334 bytes at `tmp/m5-preview-output.mp4`, total 1,225 ms (600 ms inspection/setup, 432 ms conversion, 192 ms verification). Built-in decoded timeline/audio/color checks and an independent complete FFmpeg A/V decode passed. Preview last output was enabled only after that success. Not a controlled performance benchmark.
- Real decoder tests: exact 921,600-byte opaque frame bound, distinct seek frames, EOF rejection, cancel-before-start rejection, tagged HDR rejection, SDR 10-bit→RGBA8 display decode, and rotated/cropped/non-square-pixel fixture letterboxing with independently specified red/blue/green/yellow corner checks.
- App tests: checked decimal seek parsing rejects negative/nonfinite/exponent/overprecision/overflow input; service retains one frame, clears on cancel, drains a cancelled request at shutdown, releases worker ownership and rejects post-shutdown requests.

Checks passed: native build; 20 ordinary native tests (two existing GPU lifecycle tests remain ignored in this run); three native-app tests; native/native-app all-target Clippy with warnings denied; formatting; browser crates' WASM check; whitespace check. Existing non-fatal GNU linker `.drectve` warning remains. No real-browser rerun or full hardware matrix rerun for this independent preview addition.

Still pending: timed playback/pause/audio synchronization, exact decoded-PTS seeking/frame stepping, GPU decode/surface sharing for preview, true same-adapter hardware codec↔processor interoperability, controlled performance/resource profiling, renderer suspend/device-loss live validation, sustained image correctness, preview modal keyboard focus trapping, and other OS/GPU/DPI configurations. Cancelled preview publication/cleanup is tested at service level; the brief fixture jobs did not establish a live GUI mid-decode cancellation test. This baseline does not finish M5.

### M5 native playback controls and audio — 2026-10-02

Supersedes the static inspector's playback limitation above. Source/output preview now provides Play/Pause, a draggable timeline, elapsed/total time, Mute, and keyboard commands. Continuous FFmpeg preview decoding is separate from conversion and samples video at 15 fps into a 640×360 RGBA8 surface. Sampling precedes resize/color conversion; late preview frames can be dropped. Conversion still preserves every frame. Application ownership is one pending RGBA frame plus one latest-frame slot (921,600 bytes each), one reusable Blitz texture, a 250 ms native PCM queue and one 10 ms producer chunk. FFmpeg/pipe/device/driver memory is outside those bounds. No raw-frame disk spool. Preview CPU decode/color conversion, CPU→Blitz upload and Vello GPU atlas copy remain explicit; no hardware/zero-copy preview claim.

Audio uses CPAL 0.18.2 and the default native output device, with an audio-consumption clock adjusted by reported device latency; muted-at-start/video-only playback uses a monotonic clock. Pause stops media consumption and keeps buffers bounded. Seeking/muting cancels and joins old codec workers before replacement; Close clears the frame and prevents cancelled publication. Missing devices/configurations and codec/output failures are visible, with explicit Mute retry rather than silent audio fallback. Short audio EOF waits for the device tail and handles sub-sample duration rounding.

Runtime environment: Windows 11, Dioxus Native/Blitz 0.7.10, presenter wgpu 26.0.1 (the renderer ABI), processing wgpu 30.0.1 unchanged, FFmpeg/FFprobe 8.0.1, Rust 1.98.1 GNU. The selected conversion GPU remains NVIDIA RTX 3050 Laptop/Dx12; preview uses the renderer's independent device and does not claim that same adapter. Default/minimum native window dimensions remain unchanged; no scrolling was needed for this player.

Verified evidence:

- Real native output-device test with the CC0 VFR/AAC fixture: duration 2,021,333 µs, 48,000 Hz stereo, 97,024 consumed sample frames, peak queued interleaved samples 24,000/24,000 (250 ms), nonzero peak amplitude 0.163198, zero underruns, 30 displayed preview frames, zero dropped; final rerun elapsed 2.928 s including startup/cleanup. This verifies delivery to the device callback and a completed media clock, not independently recorded speaker audibility or physical A/V synchronization.
- Real Computer Use session with the user's local 3:49 recording: source frame displayed while initially paused, Play advanced both image and elapsed time, Pause held at 0:23 across later captures, paused drag moved to 1:54, resume advanced there, Mute switched to video-only while continuing, Unmute restored the audio-clock route, and playing backward drag resumed near 0:37. Closing the active player returned to the compact main window; process enumeration then found no FFmpeg preview children.
- The first timeline drag exposed a real crash at `dioxus-native-dom-0.7.10/src/events.rs:194` (`element_coordinates` is unimplemented). Replaced it with supported client coordinates and a delegated native event-loop viewport adapter; paused and playing drag then passed. The local launcher keeps the existing renderer/windowing ABI and embedded assets, without a webview or unsupported mounted-geometry API. Keyboard handling is at the app root while the modal is open because clicking the custom slider did not focus it in this Blitz build.
- Backend test exercises initial pause, a 0.5-second seek, resumed advancing position, paused clock stability, complete EOF, end-position clamping and cancellation. Service tests exercise bounded latest-frame ownership, rapid replacement, cleared publication and shutdown rejection/drain. Color/geometry/HDR/10-bit display tests from the inspector still pass.
- Native NVIDIA conversion regression after the player change: VFR/AAC fixture, 36 frames, 160×90 H.264/NVENC with AAC, 73,334 bytes at `tmp/m5-player-output.mp4`. Built-in complete output validation passed; total 1,351 ms (752 ms preflight, 428 ms conversion, 169 ms verification). Independent complete FFmpeg A/V decode and FFprobe frame/codec/geometry inspection passed. This is a correctness regression, not a benchmark or a live converted-output player test.

Checks passed on the final code: `cargo build --locked -p converter-native`; native/native-app tests (21 ordinary native + four app tests; two existing GPU lifecycle tests still ignored, audio-device test explicitly run separately and passed); native/native-app all-target Clippy with warnings denied; shared core/GPU checks/tests/lints (13 core tests); browser-crate WASM check; formatting and whitespace checks. The existing non-fatal GNU linker `.drectve` warning remains.

Still untested/limited: the final root-level keyboard shortcut fix and continuous converted-output playback were not live-tested: Computer Use reported active user-input interruption, so GUI input stopped to avoid interfering with the user's other windows. Earlier static-output presenter evidence remains separate. Also pending: independently listened/recorded audio, measured physical A/V drift and fine offset accuracy, exact decoded-PTS seeking/frame stepping, output-device changes, unusual long audio-only tails/edit lists, arbitrary DPI/multi-monitor/accessibility/focus trapping, renderer suspend/device-loss and long-run memory profiling. Preview is SDR, 8-bit display, CPU-decoded and sampled at 15 fps; conversion depth/timing/hardware routes are unchanged. Full M5 hardware codec↔wgpu surface interop remains pending. No real-browser rerun for this native-only change.

### M5 four-route release comparison and NVIDIA pixel-color correction — 2026-10-03

Implemented `tests/native-m5-release-bench.mjs` plus three unit tests for statistics, rotated route order and independent NV12/RGBA transfer accounting. Commands are in the README. It builds the release CLI and generates CC0 seeded testsrc2/noise/sine sources under ignored `tmp`: 1920×1080, 180 frames / 6 seconds and 3840×2160, 90 frames / 3 seconds, 30 fps, BT.709 limited-range H.264 with 48 kHz mono AAC. The requested output is 50% resize, H.264/AAC. One warm-up per route/workload is excluded; three timed rounds run serially in rotated order. No preview playback or second conversion was started by the harness during timing. Fresh CLI processes cannot reuse session inspection; filesystem cache state, external system load, power and thermal state were not controlled or measured.

Environment: Windows `10.0.26300` x64, Intel i5-11400H, NVIDIA RTX 3050 Laptop / Dx12 (`10de:25a2`), NVIDIA driver 616.64 / wgpu driver `32.0.16.1664`; Rust 1.98.1 GNU, Dioxus Native/Blitz 0.7.10, processing wgpu 30.0.1, renderer wgpu 26.0.1 unchanged, FFmpeg/FFprobe 8.0.1 Gyan essentials. Exact adapter key, NVIDIA UUID, tool versions, commands, hashes, all run reports and validation results are in `tmp/m5/release-bench/run-1791051561592-12164/evidence.json` (`status: passed`). Source SHA-256: 1080p `0b2c5810200e90d5fc093a7b00f70363cf316ab2ebc0db285efedc4426283dba`; 4K `0f7cd40564f312c0c418b203c46c59b71c913278523fa5753a74c0b5802a876f`.

**Failure found and corrected before accepting results.** The first fixture-generation attempt retained range/matrix but lacked transfer/primaries; explicit libx264 VUI parameters fixed the fixture rather than weakening metadata assertions. The next run passed counts/timing/audio/tags, but shared NVIDIA output failed decoded pixel fidelity: 23.839968 dB PSNR (luma 22.476987). That failed run is retained at `tmp/m5/release-bench/run-1791051042850-18232`; its older runner left `status: running`, which is **not** passing evidence. The revised runner now persists failed status/error stacks. Independent single-PSNR decoding confirmed the same error. A separate first-frame diagnostic found only small software-versus-NVDEC RGB differences, pointing to the encoder-side conversion rather than the resize shader. Selecting full-range RGB → limited-range NV12 with an explicit BT.709 matrix **before** CUDA upload fixed the pixels; output tags/setparams alone had not selected the conversion math. The same 1080p job then measured 39.762522 dB. No additional transfers, frame persistence, fallback or changes to direct/software routes were introduced.

**Verified release results.** All 32 outputs (eight excluded warm-ups plus 24 timed jobs) passed independent complete A/V decoding, exact frame counts and dimensions, every video timestamp within 20 µs, expected H.264/YUV420P and AAC/48 kHz, non-silent audio, duration/offset bounds, all specified SDR color tags and exact transfer-count assertions. First timed output per route/workload was fully compared with an uncompressed bilinear resize of the decoded source using PSNR and SSIM; all eight quality checks passed. External validation and fixture generation are excluded from reported job timings. Total includes native inspection/setup and mandatory full output verification; conversion includes codec/processing bridges and muxing. Stage and process-wall ranges are retained in JSON. Stage medians need not sum to the total median.

| Input → output | Route | Total median [min–max], ms | Conversion median [min–max], ms | PSNR dB / SSIM | Output bytes |
|---|---|---:|---:|---:|---:|
| 1080p → 960×540 | Direct CPU | 3,236 [3,195–5,774] | 1,423 [1,422–2,603] | 44.61 / 0.995775 | 1,054,287 |
| 1080p → 960×540 | Direct NVIDIA | 7,530 [4,350–8,361] | 2,121 [1,376–2,321] | 39.49 / 0.983087 | 2,941,196 |
| 1080p → 960×540 | Shared wgpu / software codecs | 12,220 [7,590–13,785] | 9,102 [5,771–10,344] | 39.03 / 0.981878 | 1,591,236 |
| 1080p → 960×540 | Shared wgpu / staged NVIDIA | 15,019 [14,524–21,551] | 9,178 [8,978–18,087] | 39.76 / 0.982982 | 2,844,053 |
| 4K → 1920×1080 | Direct CPU | 6,805 [4,537–6,807] | 3,017 [1,893–3,089] | 45.64 / 0.996245 | 1,623,311 |
| 4K → 1920×1080 | Direct NVIDIA | 6,806 [5,878–7,559] | 1,573 [1,327–1,576] | 41.11 / 0.988979 | 5,515,053 |
| 4K → 1920×1080 | Shared wgpu / software codecs | 23,386 [18,530–23,657] | 19,466 [15,138–19,835] | 40.00 / 0.987544 | 2,490,154 |
| 4K → 1920×1080 | Shared wgpu / staged NVIDIA | 24,850 [24,361–30,005] | 17,958 [17,593–23,508] | 40.94 / 0.988642 | 5,360,225 |

These are current-route baselines, **not matched-quality or matched-bitrate benchmarks**: libx264 veryfast CRF20 differs from NVENC p4 VBR CQ20, and the routes have different color/chroma/resize implementations. Broad timing ranges and only three timed samples limit conclusions. NVIDIA conversion was faster than software at 4K, but cold-session total time was effectively equal because setup/inspection/verification remained substantial. Both staged-wgpu routes were slower than direct routes on these short synthetic jobs. Keep direct FFmpeg/CUDA for resize-only work; no universal speedup or reason to change defaults is established.

Known pixel transfers per conversion, excluding independent verification/preview: shared software codecs upload 1,492,992,000 RGBA bytes and read back 373,248,000 bytes for 1080p; 4K doubles both counts. Staged NVIDIA adds 559,872,000 NV12 download bytes and 139,968,000 NV12 upload bytes for 1080p, also doubled at 4K. The direct CUDA route reports zero application-managed pixel download/upload; that does not prove zero internal driver/codec copies. No per-bridge time attribution, CPU utilization, power, VRAM/RAM peak or opaque FFmpeg/driver memory measurement was performed.

**Hardware regressions passed.** `node tests/native-m5-hardware.mjs` completed all eight cases: direct NVIDIA CFR, VFR/non-zero origin, HEVC input and genuine HEVC Main 10 output; shared NVIDIA CFR, VFR, HEVC input and a new tiny explicitly tagged BT.709 source. Counts, dimensions, decoded timing (maximum below 1 µs), audio signals and matching tags passed. Main 10 retained fine luma precision. Direct/shared CFR PSNR versus the software output was 35.855602 / 36.304374 dB; tagged shared PSNR versus the uncompressed reference was 33.769062 dB. Wrong/missing adapter rejection, unsupported tiny HEVC hardware failure without fallback, cancellation after staged output began, retry and zero remaining partial outputs passed. Evidence: `tmp/m5/hardware-3024/evidence.json`; generated media remains ignored.

**Checks passed:** release CLI build; debug native-app build; host shared-core/GPU checks; 13 core, 21 native and four native-app tests; three benchmark unit tests; native/core/GPU/app all-target Clippy with warnings denied; browser-crate WASM check and Clippy; JavaScript syntax, Rust formatting and Git whitespace checks. The existing non-fatal GNU `.drectve` linker warning remains. Two ignored GPU lifecycle tests and the ignored audio-output-device test were not rerun in this continuation; hardware cancellation/retry above is separate evidence. No browser runtime rerun for this native-only change.

**Still untested / M5 remaining:** a Computer Use retry enumerated the converter, but activation failed with `failed to activate captured window` even after refreshing the app/window and one recovery retry. The skill-required stop was respected; this does not prove an app UI defect. Final root-level keyboard shortcuts and continuous converted-output GUI playback remain pending, as do independently recorded physical A/V sync, broad DPI/focus/device-loss and long-duration resource coverage. Most importantly, true same-adapter native codec-surface ↔ wgpu import/export with synchronization remains **unimplemented**; staged CUDA/NV12 CPU bridges are not that proof. This benchmark and player do not finish full M5. No M6 work was started.

### M5 compact inline native player and keyboard repair — 2026-10-03

At the user's request, replaced the preview overlay with a permanent 320×180 main-window box beside the processing/conversion column. Convert/Cancel and the source/output fields remain available; Play/Pause, Mute, elapsed/total time and a 320px draggable timeline are below the image. Close returns that box to its empty state. Source and last verified output share a keyed player; the replacement starts in a component effect after the old component's cancellation/drop. The decoder's 640×360 RGBA8 frame bound, 15 fps preview sampling, PCM bounds and conversion/device/transfer paths are unchanged. Only the display scene scales to half size; no new frame readback or conversion shader was introduced.

Real Computer Use became available for this retry. It exposed two issues before acceptance: the previous root DOM keyboard handler did nothing in Blitz, and merely changing canvas bounds to 320×180 cropped the registered 640×360 image. Player commands now enter through the existing winit adapter and a bounded 16-command metadata queue; handled keys are not also delegated to a focused DOM button. A 640×360 canvas with a whole-scene CSS scale inside a 320×180 clipped viewport shows the complete image. The preview timeline's client-coordinate mapping follows its new right-column placement and the shell's 1100px maximum width. The main-window release signal also finishes a drag released over another control; this outside-release behavior is implemented but not live-verified here.

Verified on the existing Windows / Dioxus Native/Blitz 0.7.10 / renderer wgpu 26.0.1 / FFmpeg 8.0.1 environment:

- The permanent empty box, then the source player, appeared beside all processing and conversion controls in the default-width native window; a maximized layout was also inspected. The final scaled source image displayed all six fixture color columns, including the previously cropped right-hand markers. This is representative visual correctness, not pixel-exact compositor or arbitrary DPI proof. Long path text can wrap in this Blitz build despite the requested CSS ellipsis.
- Used a reproducible CC0 extended M2 fixture, generated by `ffmpeg -v error -nostdin -n -stream_loop 9 -i fixtures/m2-h264-aac.mp4 -c copy tmp/m5-player-validation-source.mp4`: H.264/AAC, 640×360, 600 frames, container duration 20.192 seconds. No user recording or downloaded media was needed; the source remains ignored.
- Right-arrow while paused sought from 0:00 to 0:05 and changed the image. Space advanced playback and the timeline/image, restarted from the beginning after EOF, then paused at 0:09. Later captures held the same paused position while editing the output path, including its spaces: player shortcut handling did not trap normal path typing. The adapter's unit test additionally verifies inactive routing, bounded repeat input, no repeated Space toggle, Escape priority, queue clearing and delegation of modified/unrecognized keys. Home/End/Left/Escape were not separately live-tested in this final inline layout.
- The final source player and Convert/Cancel remained concurrently visible, with no preview overlay or scrolling used to reach these controls. Before accurate further coordinate tests could be completed, Computer Use reported `user input was detected in this window; call get_window_state before continuing`. UI input stopped to avoid interference. The attempted conversion click did not start a conversion or create `tmp/m5-inline-output.mp4`; it is not a passing output-playback test. Earlier partially off-screen captures also made coordinate testing unreliable; no broken converter-button behavior is inferred from that attempt.

Checks passed on final code: native-app build, all five native-app tests, native-app all-target Clippy with warnings denied, host shared-core/GPU checks, browser-crate WASM check, Rust formatting and Git whitespace checks. Existing non-fatal GNU `.drectve` linker warning remains. No dependencies changed; no real-browser or complete hardware benchmark rerun for this UI-only continuation.

Remaining: continuous converted-output playback/path replacement in the live inline box, live drag-position/drag-outside-release checks after the layout move, minimum-size/DPI/multi-monitor coverage, complete keyboard accessibility, physical A/V synchronization, renderer loss/suspend and long-run profiling. Native codec-surface↔wgpu synchronization/import/export remains unimplemented; this UI continuation does not complete full M5. The earlier root-keyboard limitation is superseded by the tested native shortcut route, not by a claim that all keys/accessibility work.

### M5 interrupted inline-player verification completed — 2026-10-03

Follow-up real Computer Use verification in the same environment, using the existing debug executable and CC0 600-frame extended fixture above. Maximized the native window to obtain a complete 1536×816 capture; all controls and the inline player remained visible without scrolling. This supersedes the immediately preceding pending drag and converted-output playback checks; no implementation or dependency changes were needed.

- Paused timeline drag moved from 0:09 to 0:15, with the corresponding image and thumb position. A subsequent drag released below/outside the player settled in the paused state; Play then advanced the image/time through EOF without a stuck dragging state. This checks outside-release recovery, not exact seeking accuracy for arbitrary out-of-bounds trajectories.
- Selected 50% resize and Direct FFmpeg NVIDIA, then started conversion through the UI. It completed with 600 frames, 320×180 H.264/NVENC and AAC, 1,612,226 bytes. UI reported 3,062 ms total: 1,148 ms inspection/setup, 1,538 ms conversion, 376 ms verification. These are single debug-session timings, not benchmark results. FFprobe independently confirmed video frame count/geometry, AAC 48 kHz mono, and 20.197 s container duration; complete independent FFmpeg audio/video decoding exited successfully with no errors.
- Clicking Preview last output replaced the already-open source player with `tmp/m5-inline-output.mp4`, initially paused at 0:00 with the complete image. Play advanced the converted image and timeline through observed 0:02 and 0:09 positions; Pause held at 0:16 across a later capture. Audio-enabled status was shown without an error; this is not independently listened/recorded audio or physical A/V sync evidence.
- Close returned the permanent preview box to its empty state. Process enumeration then found the converter but no FFmpeg/FFprobe processes, confirming preview worker cleanup for this run. The converter was left open and idle.

The final-code build/tests/lints listed above remain applicable; only this report changed during verification, and `git diff --check` passed. Still pending: minimum-size/arbitrary DPI/multi-monitor and complete keyboard/accessibility coverage, measured physical A/V sync, device-loss/suspend and long-run resource profiling. True native codec-surface↔wgpu import/export and synchronization remains unimplemented; these successful UI checks do not complete full M5. No browser rerun, commit or push was performed.

### Native bitrate selection and pre-conversion size estimate — 2026-10-04

Implemented native Recommended default plus 1/2/4/8/16/32 Mbps buttons, with profile-specific video bitrate resolved in shared core. Native source Inspect supplies cached metadata for the approximate decimal-MB estimate; bitrate/resize/profile changes recompute it without another decode, and input path changes clear it. Includes track offsets/latest endpoint, 192 kbps AAC when present and 2% muxing allowance. Unknown duration withholds the size. This is a VBR planning estimate, not a guaranteed size, compression ratio, maximum rate or quality guarantee. Native format buttons were compacted into one row to accommodate settings. All four native runners apply the target; direct H.264/H.265 software and NVIDIA codecs use ABR/VBR rather than simultaneously imposing CRF/CQ. CLI `--bitrate recommended|MBPS` exercises the same policy; older no-bitrate calls remain CRF/CQ20. Browser bitrate controls/encoding are unchanged.

Verified on the existing Windows / RTX 3050 Laptop Dx12 / Rust 1.98.1 GNU / Dioxus Native 0.7.10 / processing wgpu 30.0.1 / renderer wgpu 26.0.1 / FFmpeg 8.0.1 environment:

- `node tests/native-bitrate.mjs` passed 12 real conversions of a deterministic CC0 640×360/180-frame/6-second seeded texture/noise + sine fixture. For each of Direct CPU, Direct NVIDIA, shared wgpu/software and shared wgpu/NVIDIA, tested 1 Mbps and 4 Mbps H.264/AAC targets. Reports retained exact requested numeric rates, all 180 frames, dimensions/audio and built-in complete per-frame timing validation; independent FFprobe and full FFmpeg A/V decoding passed every output. Increasing rate materially increased bytes on every route: 1 Mbps outputs 823,134–913,383 bytes; 4 Mbps outputs 3,351,121–3,546,147 bytes. Measured video rates ranged 0.980–1.100 Mbps and 4.350–4.610 Mbps, illustrating real VBR variation rather than exact rates.
- Recommended CPU H.264 resolved to 691,200 bps at original size; recommended NVIDIA half-size hit the 250,000-bps floor. Recommended CPU HEVC Main 10 resolved to 483,840 bps; NVIDIA HEVC Main 10 with explicit 2 Mbps also passed. An invalid zero-Mbps request failed without publishing a file. Evidence and generated media: `tmp/m5/bitrate-24688/evidence.json` (`status: passed`). Earlier `tmp/m5/bitrate-5616` is a failed harness run: a floating-point expected-value equality for HEVC (483840.00000000006 versus 483840) stopped it; corrected the test to integer-factor arithmetic, not the encoding policy, then reran the complete gate.
- Core tests cover rational-frame-rate/geometry/codec recommendation, fixed-rate invariance, range/zero rejection, audio/no-audio estimates, zero-duration/overflow handling. Native tests cover common-origin/audio-tail duration, unknown duration and separating target-rate arguments from legacy quality mode.
- Checks passed: native app and CLI builds; 14 core, 23 ordinary native and five app tests (three existing hardware/audio tests ignored); host core/GPU check; browser-crate WASM check; core/native/app all-target Clippy with warnings denied; formatting, JavaScript syntax and whitespace checks. Existing non-fatal GNU `.drectve` linker warning remains. No dependencies changed.

Not verified: final live native button interactions, estimate display and no-scroll layout. Computer Use initialization failed twice with `failed to write kernel assets: The system cannot find the path specified. (os error 3)` before app enumeration; no desktop inputs or automatic repair were attempted. The Computer Use skill required stopping GUI automation after the failed recovery. Estimate accuracy on arbitrary content, subjective quality, long-duration rate control, browser bitrate wiring and non-Windows/non-NVIDIA behavior remain untested. This change does not implement native codec-surface↔wgpu interoperability or finish full M5. No commit/push was performed.

#### Live bitrate UI retry — 2026-10-04

Computer Use now initialized successfully and enumerated real apps. Launched the existing built `converter-native.exe` through the supported runtime, selected its unique returned window and inspected the actual Blitz UI; no repair was performed in this retry. This supersedes the preceding pending live estimate/display checks, not the remaining backend/quality/platform limits.

- Default 960×760 logical client window (962×792 captured with frame): Original size, H.264 and Recommended were initially selected. All seven bitrate choices appeared as compact buttons; no expanded HTML select options. Controls visibly disabled during source inspection.
- Entered the deterministic `tmp/m5/bitrate-24688/source.mp4` path and clicked Inspect. Metadata showed 640×360, H.264, yuv420p, 180 frames and AAC. The pre-conversion estimate then showed 0.69 Mbps / ~0.7 MB. Clicking 4 Mbps visibly selected it and immediately changed the estimate to 4.00 Mbps / ~3.2 MB. Restoring Recommended restored the original estimate.
- Selecting H.265 Main 10 changed Recommended to 0.48 Mbps / ~0.5 MB and disabled shared routes as expected. Selecting 50% resize changed it to the 0.25-Mbps floor / ~0.3 MB. No conversion or extra inspection was needed to update these settings.
- Opened the source preview: complete image, timeline/Play/Mute, settings, job controls and footer were simultaneously visible without scrolling in the default window. Light mode also rendered the bitrate/estimate/player controls visibly; restored dark mode, closed preview and restored H.264 / Original / Recommended. The app remains open and idle with the fixture inspected and no output path selected.

No new conversion was performed through the GUI in this retry; the 12 CLI/backend conversions above remain the encoding evidence. Minimum-window size, arbitrary DPI, long diagnostic wrapping, every fixed-rate button individually, and general accessibility remain untested layout/input coverage. Only documentation changed; whitespace check passed. No commit/push performed.

#### Source-adaptive bitrate and selectable native FPS — 2026-10-04

Replaced fixed native Mbps buttons with Smaller / Recommended / Higher, recalculated from the inspected video-stream bitrate, source/output display dimensions, rational FPS and codec. Custom accepts decimal Mbps. Inspect now displays FPS with CFR or VFR-average labeling. An in-window output-FPS chooser defaults to Original; explicit fixed rates resample frames, not playback speed or audio timing. Estimates react to all these settings without repeating inspection. Core owns the policies; FFmpeg resampling and verification stay in media-native. No dependency changes, extra conversion pixel transfers or raw-frame persistence.

Verified on Windows / NVIDIA RTX 3050 Laptop GPU (Dx12 processing, UUID-scoped CUDA/NVENC) / Rust 1.98.1 GNU / Dioxus Native 0.7.10 / FFmpeg 8.0.1, with the same wgpu versions as above:

- `node tests/native-frame-rate.mjs`: **38 real conversions passed**, final evidence `tmp/m5/frame-rate-20140/evidence.json`. On all four routes, the 60-frame CFR fixture retained 60 frames at Original, produced 30 at 15 FPS, 120 at 60 FPS, and 60 at exact 30000/1001 FPS. The 36-frame offset VFR fixture retained all original timestamps at Original, produced 30 / 119 / 59 frames respectively at those fixed rates. The harness independently checked every decoded frame PTS against the rational grid/common origin, counts, audio duration and complete FFmpeg A/V decoding; built-in publication checks also verified geometry, color and track coverage. Two 25-FPS HEVC Main 10 CPU/NVIDIA outputs each had 50 frames and passed full decoding. Four two-frame/no-audio 30-FPS cases retained exactly two frames, including both shared routes. Invalid zero FPS rejected without publication.
- The initial 34-case gate (`tmp/m5/frame-rate-6792`) passed, but an additional two-frame test exposed microsecond-rounded EOF planning (66,667 µs at 30 FPS incorrectly planned three frames). The conversion failed validation and published no output. Corrected planning to use exact stream duration ticks/time base, forced the planned first grid point and trimmed only excess rounded bridge EOF frames. Added unit and four-route runtime regressions, then reran the complete expanded gate above. No missing-tail padding or relaxed verification. The earlier `frame-rate-4180` attempt failed in the harness because it invoked a nonexistent CLI `inspect` command; fixed the harness to independently use FFprobe before any conversions.
- `node tests/native-bitrate.mjs` rerun passed all 12 cases (`tmp/m5/bitrate-3336/evidence.json`): fixed 1/4-Mbps targets materially changed size on all four routes; adaptive original/half-size H.264 and HEVC cases, invalid bitrate rejection and complete output decoding passed. The source-rate cap, FPS/pixel/codec scaling, custom override and missing-rate fallback also have core unit coverage.
- Live Computer Use/Blitz checks: default 960×760 logical client window showed 30.000 FPS (CFR) after Inspect. At original size / Original FPS, buttons showed 0.41 / 0.69 / 1.04 Mbps. Choosing 15 FPS immediately changed them to 0.25 / 0.35 / 0.52 Mbps; selecting 50% resize changed them to 0.25 / 0.25 / 0.38 Mbps (the documented floor). Custom 3.5 Mbps visibly selected Custom and showed an estimated ~2.8 MB. A GUI NVIDIA conversion of the 180-frame/6-second synthetic source completed with **180 input → 90 output frames** at 15 FPS. Independent FFprobe confirmed 640×360 H.264, 15/1 FPS, 90 frames, 6-second video plus AAC; full A/V decoding passed. Output `tmp/m5/frame-rate-6792/gui-15fps-custom.mp4` was 3,286,768 bytes, measured video rate 4,269,158 bps: VBR was not a strict 3.5-Mbps cap. The inline output preview, settings, completion controls and footer fit without main-window scrolling in both dark and light mode. These UI checks preceded the endpoint precision correction; the corrected backend was subsequently covered by the complete 38-case gate, and the final app rebuilt successfully.
- Final checks passed: CLI/native app builds; 16 core, 26 ordinary native and six app tests; native/core/GPU all-target Clippy with warnings denied; browser/core/GPU WASM checks; formatting, JS syntax and whitespace checks. Three existing runtime tests remained ignored in the ordinary suite; the active-job cancellation/retry test was separately run and passed for its existing Original-FPS CPU/shared routes. Existing non-fatal GNU `.drectve` linker warning remains.

Still untested: every UI FPS button individually, custom-invalid GUI error presentation (parser unit-tested), arbitrary DPI/minimum-window layout, fixed-FPS cancellation during active conversion, long-duration rate control, subjective resampling quality, physical/sample-exact A/V sync, missing exact-duration inputs beyond preflight rejection, non-NVIDIA/non-Windows runtimes, and browser FPS/bitrate controls. Recommendations are heuristics, not scene analysis or perceptual-quality guarantees; fractional timing can move video endpoints by up to one target frame. Browser controls are unchanged, and WASM compilation is not browser runtime evidence. No native codec-surface↔wgpu interop, milestone advancement, commit or push performed.

#### Browser adaptive bitrate, size estimate and output FPS — 2026-10-04

Browser controls now implement Smaller / Recommended / Higher, custom decimal Mbps, approximate output size and Original-default output FPS, superseding the native-only status above. The shared core policy resolves source-video-rate/geometry/FPS/codec recommendations and checked rational frame grids; browser codec glue implements bounded one-frame lookahead for duplicate/drop resampling. Numeric bitrate reaches both WebCodecs VBR and the optional FFmpeg WASM `-b:v` encoder. Original retains source PTS; audio playback speed is unchanged. Source inspection shows FPS and CFR/VFR-average (Mediabunny samples up to 256 packets for this classification). No dependencies upgraded, WebCodecs pixel readbacks added, or raw-frame persistence introduced.

Verified on Windows, Rust 1.98.1 GNU / edition 2024, Dioxus Web 0.7.10, processing wgpu 30.0.1, Mediabunny 1.58.1, pinned FFmpeg WASM core 0.12.10 / class 0.12.15, independent FFmpeg/FFprobe 8.0.1:

- `node tests/browser-output-settings.mjs chromium 8084`: **26 conversions passed**, final evidence `tmp/browser-output-settings/chromium-19968/evidence.json`. Browser was isolated headless Chromium/Edge 154.0.4258.53, selected adapter `intel / gen-12lp (BrowserWebGpu)`. Enabled WebCodecs profiles: VP8/Opus, VP8 video-only and H.264/AAC; HEVC output probe rejected. Every enabled profile passed Original / 15 / 60 / exact 30000/1001 FPS with complete built-in output re-decode and independent FFprobe per-frame timing/count plus full FFmpeg A/V decode. Audio-preserving CFR counts were 60 / 31 / 121 / 60; video-only counts 60 / 30 / 120 / 60. The difference comes from the existing common origin including AAC priming at about −21.333 ms, not speed changes or lost frames.
- `node tests/browser-output-settings.mjs firefox 8084`: **22 conversions passed**, final evidence `tmp/browser-output-settings/firefox-9300/evidence.json`. Browser was isolated normal-window Firefox 157.0 (build 20260924084938); adapter identity was redacted. Both WebM profiles passed the same FPS matrix; WebCodecs H.264/AAC was disabled because AAC encoding was unsupported, and HEVC output was unsupported. The optional FFmpeg WASM H.264/AAC route passed in both browsers: Original / 15 / fractional FPS, VFR input explicitly resampled to 15 FPS, and custom 1/4-Mbps targets. This is not Firefox WebCodecs MP4 support.
- In both browsers, the offset VFR fixture retained 36 frames at Original and produced 119 at 60 FPS; its FFmpeg fixed-15-FPS output had 30. Active fixed-FPS cancellation returned all application-held frame/sample references to zero; same-context retry passed (120 source frames → 241 target frames including priming-origin coverage). Two-frame/no-audio regressions produced exactly two frames at 30 and 30000/1001 FPS. Explicit main-thread WebCodecs fallback passed a fully verified 60-FPS conversion (60 → 121). The FFmpeg raw bridge still rejects Original VFR; explicit fixed FPS is the supported alternative.
- Adaptive labels changed after resize and output-FPS changes; invalid custom zero disabled conversion; decimal 3.5 Mbps displayed an estimate (~1.9 MB for the four-second audio source). Custom targets changed real output size: Chromium VP8/Opus 1/4-Mbps outputs were 573,817 / 2,085,693 bytes, Firefox 593,422 / 2,103,077; FFmpeg WASM Chromium 568,414 / 2,093,034, Firefox 568,346 / 2,100,861. All decoded successfully. These seeded textured-fixture measurements demonstrate effective rate control, not exact-rate or file-size caps. Saved `settings.png` screenshots were visually inspected: labels, estimate, custom field, profile capability reasons and preview rendered without overlap at the captured narrow Chromium and desktop Firefox sizes. Broad responsive/accessibility coverage remains untested.
- Existing Chromium worker regression passed all three profiles with both acceleration preferences, cancel/restart and output playback/seek, plus cancellation and **five repeated 30-frame M1 marker checks**. Evidence is under `tmp/m33-chromium-worker`. Hardware execution remains unknown despite successful probes. WebCodecs conversions reported zero explicit pixel readbacks; Firefox used its existing VideoFrame→ImageBitmap ingress compatibility path. FFmpeg WASM retained its explicit RGBA readback/ring and software encoder. Lifecycle counters returned to zero; opaque codec/browser memory is not measured.
- Build checks passed: Dioxus web build/serve; 17 core, 26 ordinary native and six native-app tests; five worker-transport tests; host core/GPU/UI/native checks; host core/GPU/native/app and WASM media-web/UI/web-app all-target Clippy with warnings denied; formatting, JS syntax and whitespace checks. Three existing native runtime tests remained ignored in the ordinary suite. Existing non-fatal GNU `.drectve` linker warning persists. Shared-grid extraction did not change the native FPS policy.

Earlier attempts are retained as failed evidence, not passing gates. `chromium-18928` / `firefox-21880` exposed sample ownership closing lookahead during duplication; fixed by giving each encoder sample its own cloned frame handle. `chromium-12596` then exposed the fractional-MP4 verifier's assumed 5-µs clock; corrected tolerance to the actual file's time resolution and two independently rounded endpoint ticks, while still verifying every frame. Firefox headless could not obtain a WebGPU canvas and was replaced with a normal isolated window. `chromium-24204` / `firefox-20372` failed the short-fixture expectation because copying two B-frame packets preserved presentation positions 0 and 100 ms (four target grid frames), not two adjacent displayed frames; corrected the fixture generator to re-encode two presentation-order frames and reran the complete expanded gates above.

Still untested: HEVC output on capable hardware, every FPS preset individually, long-duration rate/estimate accuracy, scene/perceptual quality, pixel-perfect duplicate/drop image selection, sample-exact/physical A/V sync, arbitrary metadata/edit lists, broad non-Windows/browser/GPU matrices, missing-duration cases, minimum-size/DPI/accessibility and full process-memory profiling. No universal CFR claim from the sampled source label, exact output-size guarantee, zero-internal-copy/hardware-codec claim, native codec-surface interop, milestone advancement, commit or push performed.

#### Shared output-settings refactor — 2026-10-05

Extracted common `VideoSettings`, finite decimal-Mbps validation, integer/rational FPS parsing and track-coverage estimation into media-core. Browser/native helpers now supply metadata and backend audio rates rather than duplicating those policies. Shared UI catalogs supply bitrate/FPS options and labels; native/browser layout remains separate. Native jobs retain optional bitrate for legacy CRF/CQ semantics. The CLI now accepts decimal Mbps through the same parser. Browser unknown included-audio timing consistently withholds size. No dependency, codec, processing, frame-transfer, scheduling or cancellation changes.

Verified on the existing Windows / Rust 1.98.1 GNU / Dioxus 0.7.10 / processing wgpu 30.0.1 / FFmpeg 8.0.1 environment:

- Complete browser settings gates rerun against the rebuilt isolated server on port 8085: **26 Chromium and 22 Firefox conversions passed**, evidence `tmp/browser-output-settings/chromium-14320/evidence.json` and `firefox-26516/evidence.json`. Recorded adapters were Intel/gen-12lp and browser-redacted respectively. The existing profile restrictions remain unchanged. Includes Original/fixed/fractional grids, adaptive labels and decimal/invalid custom settings, effective 1/4-Mbps size changes, VFR, active cancellation/retry, FFmpeg WASM, two-frame EOF, full re-decode and independent FFprobe/FFmpeg checks, plus main-thread fallback.
- Native bitrate gate passed **12 real conversions** across CPU, NVIDIA, shared-wgpu/software and shared-wgpu/NVIDIA, plus adaptive rates/HEVC and invalid-zero rejection without a file: `tmp/m5/bitrate-6972/evidence.json`. Earlier `bitrate-18776` completed its 12 conversions but failed a harness assertion expecting the old CLI wording; updated the assertion to the shared range message and reran the entire gate, without relaxing rejection/publication checks.
- New decimal CLI smoke run used the M2 fixture with `--bitrate 3.5 --fps 15`. It reported the exact 3,500,000-bps target, 60 input → 30 output frames and 527,619 bytes. Independent FFprobe confirmed H.264, 15/1 FPS, 30 frames and AAC; full FFmpeg A/V decoding passed. File: `tmp/settings-refactor/custom-decimal.mp4`.
- **20 core, 26 ordinary native, six native-app and one shared-UI tests passed**, plus five worker-transport tests. New core tests cover parser ranges/non-finite/malformed values, fractional FPS, settings resolution, negative/common track origins, audio tails, omitted audio, unknown endpoints and overflow. UI test checks preset command values round-trip through the common parser and labels. Three existing native runtime tests remained ignored in the ordinary suite. CLI build, Dioxus web build/serve, host core/GPU/native/UI/app and WASM media-web/UI/web-app all-target Clippy (warnings denied), formatting, JS syntax and whitespace checks passed. Existing non-fatal GNU `.drectve` linker warning persists.

Native live UI interaction and the full 38-case native FPS gate were not rerun during this extraction; the earlier evidence remains historical, with current native unit tests and the explicit 15-FPS CLI conversion covering the changed shared policy boundary. Broad long-run/DPI/accessibility, unknown-duration browser inputs and hardware/physical A/V-sync limits remain as above. No new generic backend framework, milestone advancement, commit or push performed.

#### Color adjustments — first shared/browser slice, 2026-10-05

Implemented, not the complete cross-platform acceptance gate: validated integer `ColorAdjustments` in media-core with a legacy-neutral `VideoSettings.color` default; brightness −100..100, contrast/saturation 0..200%; canonical encoded-RGB reference and a fused resize/adjustment shader. An immutable uniform is created for each adjusted submission, so queued work cannot read subsequently changed slider values. Neutral uniforms bypass slider math. sRGB attachments explicitly cancel their automatic output encoding so the encoded-channel formula, including neutral output, does not get an unintended extra transfer function. Unorm input views remain explicit.

Browser controls now include numeric slider values, Reset color, same-frame Before/After and a paused source-position slider. Before affects only preview; conversion snapshots the stored adjustments and applies them once per source frame before FPS duplication/drop. WebCodecs and the existing FFmpeg WASM raw-ring backend consume the same adjusted GPU output; FFmpeg does not filter it again. This does not introduce a GPU-free browser FFmpeg route.

Preview ownership: one context-owned original VideoFrame, no pixels in Dioxus state/worker messages, latest-revision coalescing with a 34 ms delay and at most one producer/render command in flight. Preview fits within 640×360 without upscaling and borrows only the idle export surface. Serialized commands and GPU drain precede canvas reconfiguration; active conversion disables editing/preview scheduling and restores its own output dimensions. Cache frames close on source/position replacement, conversion/M1 start and context cleanup. Source replacement invalidates status revisions and hides old pixels until the current result. The reported cache-load counter counts input opens for this cache, not all decoder-internal frames or geometry inspection decodes. Conversion cleanup counters exclude the separate paused-preview cache, which may reopen after a finished job.

Verified on Windows, Rust 1.98.1 GNU / edition 2024, Dioxus 0.7.10, processing wgpu 30.0.1, Mediabunny 1.58.1 and native FFmpeg 8.0.1:

- Shared reference/default/validation tests passed. **21 core, 27 ordinary native, six native-app and one shared-UI tests**, plus five worker-transport tests passed. Four native hardware/audio tests were ignored by the ordinary suite; the new diagnostic GPU test was also executed explicitly and passed on **Intel UHD Graphics / Vulkan**. It checks four opaque RGB patches × six settings × Unorm/sRGB targets against the scalar formula within **two code values/channel**, including neutral, grayscale, combined settings, clipping and zero contrast. Readback is test-only; this is shader correctness, not native UI/codec integration or Main 10 proof.
- Final browser color gate: **Edge/Chromium 154.0.4258.53** headless isolated CDP and **Firefox 157.0** isolated normal-window BiDi passed `tests/browser-color-preview.mjs`. Evidence: `tmp/color-preview/chromium-23316/evidence.json` and `firefox-14384/evidence.json`. Chromium reports Intel/gen-12lp; Firefox's browser adapter identity remains redacted. Uses the existing documented CC0 `fixtures/m2-h264-aac.mp4`, no user media or new downloaded fixture.
- Both execution contexts passed paused-preview screenshot checks for brightness **10**, contrast **150%**, saturation **0%**, with nonblank grayscale content. Independent FFmpeg decoding checked first-export-frame RGB channel spread ≤4 code values (a grayscale check, **not** a lossy preview/reference parity tolerance). Worker and main-thread VP8 video-only exports each retained **60 frames**. Worker VP8/Opus passed in both browsers, and H.264/AAC additionally passed in Chromium at **15 FPS / 31 frames** under the established AAC-priming/common-origin policy. Both browsers passed adjusted FFmpeg WASM H.264/AAC at **Original / 60 frames**. Each export passed complete browser re-decode, independent FFprobe frame count/rational-grid checks, included-audio checks and full FFmpeg A/V decoding. HEVC output remained unavailable on these browser configurations.
- Slider updates reused the cached source without a new cache load. Sixty rapid brightness events produced **one** additional render in Chromium and **two** in Firefox, not sixty commands; cache loads stayed constant. Source-position changes, neutral defaults and Reset passed. Export remained adjusted while Before displayed the neutral source, proving that comparison did not replace the saved export snapshot.
- Color exports reported zero WebCodecs conversion pixel readbacks and zero job-held frames/samples after cleanup. Existing input texture leases remained bounded at four. FFmpeg WASM retains its explicit **60 VideoFrame.copyTo readbacks**, bounded 4 MiB ring and terminated encoder worker for the 60-frame job; color added no second pixel pass/readback. Firefox retains its existing VideoFrame→ImageBitmap compatibility bridge. Internal browser/driver/codec copies and hardware execution are unknown.
- Neutral regressions rerun during this slice: **26 Chromium / 22 Firefox** bitrate/FPS conversions passed, including VFR, fixed/fractional FPS, effective bitrate changes, cancellation/retry, FFmpeg WASM and main-thread fallback. Evidence: `tmp/browser-output-settings/chromium-23436/evidence.json`, `firefox-25208/evidence.json`. Chromium worker interop also passed all three enabled profiles × both acceleration preferences, cancel/restart, playback and **five M1 runs**. These neutral gates preceded the final loading-visibility/UI/backend-exposure changes; the final color gate above ran on the final browser build.
- Dioxus web build/serve, host core/GPU/native/UI/app and WASM media-web/UI/web-app all-target Clippy with warnings denied, formatting, JS syntax and whitespace checks passed. Added the already pinned wasm-bindgen-futures dependency to the web app for preview delay; no dependency versions changed. Existing non-fatal GNU `.drectve` linker warnings persist.

One intermediate UI regression was detected rather than accepted: loading hid the preview, and assigning an empty inline style did not clear its visibility property. Explicit visible/hidden values on the enclosing preview panel fixed it; final screenshot checks passed in both browsers. Earlier passing color evidence (`chromium-24808`, `firefox-22684`) predates that change; intermediate failed color runs under `tmp/color-preview` are not acceptance evidence.

Still pending/unverified: native slider UI/Blitz presenter and native job integration; exact direct-CPU FFmpeg encoded-RGB mapping, CUDA/NVIDIA adjustment capabilities and Main 10 precision; CLAHE and its temporal history. No native color controls or silent CPU/hardware fallback were added. Also pending: numeric same-position preview/export parity with measured lossy error, adjusted rotation/PAR/VFR and device-loss/cancel stress coverage, rapid replacement/seek stress, continuous color-preview playback/separate simultaneous preview/export targets, comparative throughput, long-run residency, wide DPI/accessibility and broader GPU/browser/codec matrices. This first slice does **not** complete the color feature or advance M5. No commit or push performed.

#### Native color continuation — 2026-10-05

Supersedes the native-integration pending status above, but not the remaining UI/parity/stress gates. Native jobs and CLI now snapshot `ColorAdjustments`; reports include the actual snapshot. The native Color button opens compact custom mouse/keyboard tracks, numeric values, Reset and source Before/After. Opening color editing pauses source playback. Source replacement cancels a stale source player; output preview forces neutral settings so color is never reapplied to an already converted file. The palette uses the existing native logical-coordinate approach rather than unsupported mounted/element-coordinate APIs. It adds one small main-window button, not a permanently expanded control section. Its live layout/interaction remains unverified below.

Presenter: the original decoded slot remains unchanged on slider updates. One reusable original texture plus one registered output texture live on Blitz's wgpu 26 device. A narrow API-version adapter compiles **the same media-gpu WGSL** and uses shared uniform serialization; no shader math duplication, wgpu 30 texture sharing, new processing device or per-slider CPU pixel loops/readbacks. It updates on renderer redraw using the latest settings; a measured native hard 30 Hz coalescing cap/queue latency is still pending. Renderer suspension clears both textures/handles; while active they are bounded presenter-lifetime resources, not claimed freed on every Close. Preview decoding now explicitly converts BT.709 limited/unspecified SDR to encoded sRGB before the existing 640×360 scaled/letterboxed RGBA8 preview. It remains sampled 8-bit display approximation, including synthetic bars, not exact selected-output geometry/Main 10 parity.

Conversion: adjusted CPU jobs use explicit zscale BT.709↔sRGB boundaries, GBR16 resize and a `geq` expression for the canonical saturation→contrast→brightness order, clamping once. They return to limited BT.709 YUV at the selected 8/10-bit depth before encoding. Actual FFmpeg filter support is checked before output creation; neutral jobs retain old filters/quality/depth. Adjusted shared-GPU jobs normalize the existing RGBA8 ingress/egress around the shared shader. Shared GPU/NVIDIA is still the **explicitly selected CPU-staged NVDEC/wgpu/NVENC bridge**, with the same counted transfer boundaries; additional CPU color conversion is not hardware surface sharing. Direct NVIDIA non-neutral settings fail before inspection/output creation and explain the supported CPU/shared alternatives. No automatic CPU staging or route fallback. Filter semantics were checked against the installed FFmpeg 8.0.1 and [official geq/zscale documentation](https://ffmpeg.org/ffmpeg-filters.html#geq), then tested rather than assumed equivalent from slider values.

Verified:

- `tests/native-color.mjs` passed **seven actual adjusted conversions** plus three rejection cases: evidence `tmp/color-native/18256/evidence.json`. Settings brightness **10**, contrast **150%**, saturation **0%**. CPU and shared-wgpu/software each exported the 60-frame H.264/AAC fixture; shared-wgpu/NVIDIA exported 320×180 at **15 FPS / 30 frames** on **NVIDIA GeForce RTX 3050 Laptop GPU / Vulkan**, retaining CUDA/NVENC identity and counted staging. CPU Main 10 exported the real 10-bit fixture as **HEVC / yuv420p10le / 24 frames**. CPU transformed/PAR/orientation fixture exported 24 frames at 50%; CPU VFR retained 36 frames; shared-wgpu VFR at 50%/15 FPS exported 30 frames. All seven kept audio, passed independent FFprobe geometry/full-frame timestamp-grid checks, complete FFmpeg A/V decode and the runner's full output validation. Independently decoded first-frame RGB was nonblank and had **zero grayscale channel spread** in all seven. This checks actual effect/timing/depth, not every pixel's lossy reference fidelity or preview parity.
- Non-neutral direct NVIDIA and brightness 101/contrast 201 were rejected without final or partial files. The initial `color-native/4980` run failed a harness check because raw FFmpeg output was accidentally decoded as UTF-8, yielding NaN pixel comparisons; fixed the binary encoding and reran the whole gate. Only `18256` is passing acceptance evidence.
- Six settings × four non-8-bit-grid RGB16 patches passed the actual FFmpeg expression against core's scalar reference within **two 16-bit code values/channel**. This isolates formula/precision from transfer, resize and codec loss; it does not assert lossless 10-bit output. The existing wgpu 30 Unorm/sRGB test was explicitly rerun and passed on Intel UHD/Vulkan.
- New `converter-native` renderer-ABI GPU diagnostic explicitly ran and passed on **Intel UHD / Vulkan / wgpu 26.0.1**, rendering the actual shared shader through `ColorPass` for neutral, combined grayscale and clipping with ≤2 8-bit code values/channel error and opaque alpha. Diagnostic readback is test-only. It validates the presenter shader/device API, not Vello composition or visible widget interaction.
- **21 core, 28 ordinary native, six native-app and one shared-UI tests** passed. Four native runtime tests and one app GPU test were ignored in the ordinary suites; the GPU tests above were executed separately. Native preview service test additionally applies 60 color changes and Reset while asserting the same decoded frame revision/allocation is retained; cancellation/replacement/shutdown checks still pass. Existing preview geometry/color, VFR and playback lifecycle tests pass with sRGB normalization.
- Neutral four-route bitrate regression passed **12 actual conversions**, including CPU/NVIDIA Main 10 and adaptive rates: `tmp/m5/bitrate-14524/evidence.json`. Native app/CLI builds, host and WASM all-target Clippy (warnings denied), formatting, JS syntax and whitespace checks pass. Added only existing pinned media-gpu/pollster dependency edges to native app/diagnostic tests; no version upgrades. Known GNU `.drectve` warnings remain.

Live native visual gate is **not verified**: current Computer Use initialization returned `failed to write kernel assets: The system cannot find the path specified. (os error 3)` twice. No native screenshot, mouse drag, keyboard track interaction, Before/Reset/output preview interaction, dark/light or no-scroll layout claim is made. The documented unofficial recovery requires quitting the host app, so no automatic repair/restart was attempted. User testing or a repaired fresh Computer Use session is still required.

Still pending: native live UI gate and palette/DPI coordinate validation; numeric same-position preview/export and cross-route transfer/resize parity (the presenter is explicitly approximate); full non-neutral seek/replacement/cancellation/device-loss stress; native bounded render-latency profiling, long-run residency/performance; direct NVIDIA GPU color capability implementation. CLAHE/temporal histogram work remains unimplemented and must wait for the slider acceptance gates. No milestone completion, commit or push.

#### Computer Use restored / native color UI smoke check — 2026-10-05

After the user reported repairing Computer Use, the current bundled 26.930.21537 plugin successfully imported `@oai/sky`, enumerated real Windows apps, launched the built native converter and captured its window. Mouse clicks, typing and slider drags worked. This supersedes the preceding runtime-unavailable blocker, not the other pending acceptance gates.

Verified visually in the approximately 960×790 captured Windows window using the existing CC0 `fixtures/m2-h264-aac.mp4`: Color opens a compact left-side palette with the paused source preview visible on the right. Saturation 0% changes the same preview to grayscale; Before restores the original colors while the stored saturation remains 0%; Reset restores 0/100/100 and After. Brightness 20 and contrast 50% change both numeric labels and visible pixels. Done closes the palette without discarding the adjustments. Dark and light palettes are readable, and Reset also works in light mode. The session was left in dark mode with neutral settings and the fixture preview open. No conversion was run during this UI smoke check; the independent export tests above remain the conversion evidence.

Limits observed: with the player open, the bottom informational footer is partly clipped at this window size, although the tested controls remain visible. The accessibility tree exposes many unnamed controls and does not provide useful slider labels/values in this capture; keyboard and screen-reader behavior were not validated. Arbitrary DPI/window sizes, numeric preview/export parity, last-output bypass, rapid seek/cancel/device loss and measured update latency remain pending. No full no-scroll/layout or complete color-feature acceptance claim, code change, commit or push is made from this smoke check.

#### Inline native color controls / layout corrections — 2026-10-05

Replaced the color modal with a collapsed-by-default expandable box in the main conversion column. Expansion pauses/opens source preview; collapse keeps settings. The preview and other controls remain visible and usable. Replaced unsupported disclosure glyphs with ASCII +/− (the rendered collapse indicator is an ASCII hyphen). Added control IDs, associated visible labels and ARIA names/value text/expanded state. Slider edits, Reset and comparison disable during active jobs; output-preview comparison disables because output bypasses adjustments. Preview path height is bounded, spacing/route hints are tighter, metadata/status are locally bounded, and GPU/last-route labels truncate rather than expand the window.

Real Computer Use exposed non-working DOM slider keyboard input. Fixed through a focused-slider native event adapter: arrows change by one, Home/End select endpoints, and only small commands enter a bounded 16-entry queue. Tab, outside clicks, lost window focus, collapse and jobs clear slider routing; it does not steal player seeking or path typing. Viewport-based drag geometry now matches inline CSS at 900/960px and centered wider layouts; endpoint/midpoint/clamping and independent-channel keyboard policy have unit coverage. No codec, shader, dependency-version or conversion transfer changes in this layout correction.

Verified with the built native app on the same Windows / Dioxus 0.7.10 / Blitz wgpu 26.0.1 environment:

- Live dark/light collapsed and expanded layouts at the default **960×760 logical client size**, including source metadata, estimate, paused player, adjusted settings, busy conversion and completion. All tested controls and the bottom footer stayed visible without main-window scrolling. The earlier clipped-footer result is superseded for these states. Two attempted border drags did not change the captured window size, so minimum-size/DPI runtime coverage is **not** claimed.
- Saturation drag to 0 produced grayscale. Focused Home selected 0; Right produced 1 without seeking playback. Before restored the colored original without changing export saturation, collapse retained settings, and Reset returned 0/100/100. Final light-mode controls and output-preview bypass passed on the final build. Brightness End set 100 while last-output preview stayed unchanged; comparison was disabled for that output. Left neutral and collapsed, with the verified output preview open.
- Actual **UI-triggered CPU export** to ignored `tmp/color-native-inline-ui.mp4`, using CC0 M2 input, Original size/FPS, recommended bitrate and brightness 0 / contrast 100% / saturation 0%, while Before showed the original. Report: **60 input → 60 output**, **640×360**, **180,928 bytes**, total **7,567 ms**. The app independently verified output before publication. Separate FFprobe confirmed H.264/yuv420p, 30/1 FPS, 60 frames and AAC; full FFmpeg A/V decode passed. Independently decoded first-frame RGB: 230,400 pixels, maximum channel spread **0**, range 0..242 (nonblank grayscale). This is not numeric lossy preview/export parity or a performance benchmark.
- Final native build, **7 native-app unit tests**, all-target native-app Clippy with warnings denied, formatting and whitespace checks passed. One existing real-GPU diagnostic remained ignored in this ordinary run; its earlier explicit result remains historical. Browser/codecs were not rerun because this slice changes native UI/window handling only.

Remaining limitation: pinned Blitz 0.2.4's accessibility builder ignores ARIA roles/names/slider values; it maps HTML tags and text runs only. App markup is labeled and keyboard behavior above works, but complete native screen-reader naming/navigation is **not fixed** or claimed; resolving that requires renderer work, not asserting that extra ARIA attributes suffice. Broad DPI/minimum sizes, complete Tab-only navigation, adjusted cancel/seek/device-loss stress, numeric parity and latency remain pending. CLAHE still unimplemented; no commit/push or milestone-completion claim.

#### Native decoded-pixel parity and bounded preview scheduling — 2026-10-08

This continuation closes the tested **native** numeric/stress gaps below, not the whole cross-platform color acceptance gate. Environment: Windows, Rust 1.98.1/GNU, Dioxus 0.7.10, Blitz wgpu 26.0.1, processing wgpu 30.0.1, FFmpeg 8.0.1. Fixtures are existing CC0 synthetic media; no user video or downloaded asset was needed. Dependencies and conversion transfer boundaries were not changed.

Fixed a scheduling issue exposed while adding rapid-seek tests: each native seek previously spawned another worker waiting to join the canceled predecessor. Preview now owns **one persistent player worker and one replaceable pending source/seek command**. The active playback pipeline is canceled/drained before taking the latest request; callbacks check cancellation while holding the frame-slot lock. Pixels remain in the single service-owned slot, not worker commands or UI signals. Slider changes retain one latest settings request; a 34 ms UI timer publishes changed settings and requests redraw, including a trailing change with no subsequent event. Published policies are separated by at least 34 ms (at most 29.4 Hz); this is **not** a measured visible-latency or total renderer-FPS guarantee. No extra decode, upload, CPU pixel loop or diagnostic readback was added to slider updates.

Verified:

- CPU pre-encoder versus actual decoded preview: Original-size **640×360**, source positions **0 and 500,000 µs**, four settings (neutral; brightness/contrast/saturation **10/150/50**, **−15/70/130**, **0/100/0**). Every RGB channel and opaque alpha were checked on all 230,400 pixels/case. Maximum error **2 eight-bit code values/channel** in all eight cases; mean absolute error **0.1505–0.5199**. Actual FFmpeg GBR16 expression/normalization is tested before lossy encoding; the ≤2 gate was not loosened.
- Actual Blitz `ColorPass` diagnostic rendered the decoded 640×360 fixture through shared WGSL for five settings, including neutral, combined color/grayscale and extreme clipping, on **Intel UHD / Vulkan**. Maximum error **0** for neutral, **1** for each adjusted setting; alpha opaque. Readback is diagnostic-only and excluded from conversion/preview production paths. This checks renderer-device shader output, not screen color management.
- `tests/native-color.mjs` passed **10 actual exports and three rejection cases**: `tmp/color-native/7184/evidence.json`. It reran the previous seven audio/timing/geometry/depth cases, then compared combined **10/150/50** exports at **0 and 0.5 s**, Original size/FPS, on CPU direct, shared-wgpu/software, and shared-wgpu/NVIDIA. The explicit NVIDIA adapter was **RTX 3050 Laptop GPU / Vulkan**. All three new outputs had 60 frames, complete 30 FPS timestamp grids, and full FFmpeg A/V decode; the runner independently checks audio/output validity before publication. Independently normalized output pixels were compared to the scalar adjustment of the same-position normalized source, with predeclared **lossy** bounds mean ≤6 and p95 ≤15 code values/channel:

  | Route | Mean error, 0 / 0.5 s | p95, 0 / 0.5 s | p99, 0 / 0.5 s | Maximum, 0 / 0.5 s |
  |---|---|---|---|---|
  | CPU direct | 1.3234 / 1.5155 | 7 / 9 | 23 / 26 | 88 / 107 |
  | Shared GPU/software | 1.3080 / 1.5028 | 7 / 9 | 23 / 26 | 88 / 93 |
  | Shared GPU/NVIDIA | 1.1252 / 1.2730 | 6 / 7 | 20 / 21 | 78 / 75 |

  Lossy edge/chroma outliers are reported, not hidden by a claim of pixel equality or substituted for the stricter pre-encoder gate. These measurements cover this geometry, fixture, profile and bitrate, not every preset or codec. Initial `color-native/22864` failed because the test's zscale chain omitted an explicit planar RGB format before RGBA. Fixed the harness and reran all cases; only `7184` is passing evidence for the new numeric gate.
- **40** rapid alternating source/seek requests with color changes preserved one worker identity; the final paused request at 500,000 µs matched independently decoded pixels exactly. Cancellation prevented later publication; shutdown joined the worker and rejected new starts. A deterministic-clock cadence test verifies coalescing and final Reset publication without another input. Existing 60-setting test verifies the decoded allocation/revision stays unchanged. These assert application-owned bounds, not opaque codec/driver memory residency.
- Explicit ignored runtime test `adjusted_jobs_cancel_lose_device_and_retry_without_partial_output` passed: `tmp/color-lifecycle-16900`. Combined 10/150/50 on CPU direct, shared-wgpu/software and explicitly selected **NVIDIA/Vulkan staged shared** each canceled only after observing an active partial file, removed partial/final files, and retried a 60-frame job successfully. A shared-wgpu job then destroyed the actual device after two processed frames, observed `GPU device lost (Destroyed)`, removed partial/final output, and succeeded with a fresh device. Destruction uses the default adapter, not an assertion of NVIDIA-specific loss recovery or spontaneous driver-failure recovery. Earlier CPU/shared run also passed under `tmp/color-lifecycle-8540`.
- Computer Use on the rebuilt native executable: source Preview loaded the fixture paused; a saturation drag ended at 50% and rendered the final value without further input; seeking to 0.5 s replaced the image while preserving settings. Before showed the same-position original, and Reset restored neutral settings/After. Dark-mode expanded default-size layout remained usable with the footer visible. This focused smoke check does not rerun light/DPI/keyboard/output-bypass coverage or provide a millisecond screen-latency measurement.
- Native library **29 ordinary tests passed / five ignored**, native app **nine passed / one ignored**; the adjusted lifecycle and actual Blitz GPU diagnostics above were additionally run explicitly. Native build and all-target native library/app Clippy with warnings denied, formatting, JavaScript syntax and whitespace checks passed. Known GNU linker `.drectve` warnings remain nonfatal.

Still untested/pending: browser numeric same-position preview/export parity and full rapid color/seek/replacement stress (browser code/build/runtime was not changed or rerun here); measured input→GPU→visible latency, sustained residency/throughput, selected-output geometry preview fidelity, Main 10 display precision, wide DPI/native accessibility and new GPUs/codecs. The native preview remains an 8-bit scaled/letterboxed approximation; the numeric comparisons above deliberately use matching Original 640×360 geometry. Direct NVIDIA non-neutral color remains explicitly unsupported; shared NVIDIA still uses the documented CPU-staged bridge. **CLAHE remains unimplemented**, and neither full color acceptance nor M5 completion is claimed. No commit or push was requested in this continuation.

#### Native seek slider transient reset correction — 2026-10-08

User reported a brief jump to the beginning when releasing the seek slider. Cause: replacing `PlaybackControl` reset duration to zero while reprobe/decoder startup ran, making the timeline percentage zero despite the requested position. The service also cleared the displayed image on every seek. Same-path active restarts now inherit the known duration and keep the current single frame until the requested frame replaces it; different sources, Close/cancel and reopening still clear pixels/metadata. Drag completion snapshots the new player state before relinquishing the drag position. This does not present an old frame as the new seek result or add a frame cache/decoder.

Verified: new deterministic loading-state regression blocks target-frame publication and asserts unchanged source allocation/revision, nonzero inherited duration and exact requested timeline position; then checks the new frame against independent fixture decode. Existing rapid replacement, cancellation/shutdown and color tests still pass: **10 native-app tests passed / one GPU diagnostic ignored**. Native build, all-target app Clippy with warnings denied, formatting and whitespace checks passed. No browser code or conversion scheduling changed.

Live transition verification remains pending: Computer Use enumerated and closed the old test build, then launched the rebuilt app, but fresh state capture failed with `foreground window did not report a process id`, including the prescribed refreshed-target retry. No screenshot-based claim about the transient transition is made. No commit or push performed.

#### Native automatic paused and full-screen preview — 2026-10-08

Implemented automatic source preview after 600 ms of stable nonempty file selection; busy jobs defer it. One attempt per settled path prevents repeated probe/decode opens and respects Close or explicit last-output preview. The source Preview button is removed. Full screen/Exit full screen changes the existing component and native winit window to borderless fullscreen, with fitted 16:9 image, timeline, Play/Pause, Mute, and source Before/After. Escape returns to the main window without closing the player; Close/source replacement also exits fullscreen. Fullscreen sizing updates do not change the player key or invoke `start`, so they do not intentionally restart the decoder or reset position. The same registered presenter and textures are used. No higher-resolution preview or selected-output geometry fidelity is claimed.

Verified by tests: stable-path debounce uses only the latest selection, defers during busy state, trims surrounding whitespace, does not reopen after one attempt, and permits a new attempt after reselection. Fullscreen fit and timeline endpoint/midpoint math pass for 960×760, 1280×720 and 1920×1080 logical viewports. Existing rapid seek/source replacement, same-path no-reset, color/keyboard and cleanup tests pass: **12 app tests passed / one GPU diagnostic ignored**. All-target native-app Clippy passed with warnings denied. No browser code or conversion changes in this slice.

Runtime UI verification is **pending**, not passed: Computer Use enumerated the converter but capture/activation failed with `foreground window did not report a process id` on both initial and refreshed-target attempts. The first native build was blocked by the running executable (`Access is denied`); the user then closed it for rebuilding. Actual fullscreen monitor coverage, fitting/visibility, Escape, playback/seek continuity and automatic paused load still need live UI checks on the rebuilt executable. The minimal public Blitz bootstrap preserves renderer context but does not install unused Dioxus document/history providers; future head/navigation APIs require explicit handling. No commit or push requested.

#### Native live automatic/full-screen preview check and layout fix — 2026-10-08

After the user fully restarted Codex, Computer Use capture and ordinary window input worked again. This supersedes the preceding capture blocker for the checks below. Tested the built native app on Windows with the existing CC0 `m2-h264-aac.mp4`, a 960×760 logical client window and 1920×1080 borderless fullscreen.

Found/fixed: the initial fixed-position player remained offset against its old flex containing block in pinned Blitz. The image was clipped and controls were off-screen despite the OS window entering fullscreen. Changed fullscreen to a normal-flow, full-viewport player: hide surrounding controls with CSS (do not unmount them), remove the shell width limit, and center the existing player. This preserves the same component key/player/presenter; no decoder, shader or conversion change. The bad initial fullscreen screenshot is failure evidence, not acceptance evidence.

Verified on the corrected build:

- Typing the complete fixture path automatically loaded frame 0 paused without clicking Inspect or Preview. Play was shown, duration 2 s, and the image remained paused across subsequent observations. The source Preview button is absent.
- Inline midpoint drag loaded the 1.000 s/frame-30 marker. Fullscreen and Escape preserved paused position instead of resetting playback. Fullscreen image/path/header/timeline/Play/Mute/Before/After/exit controls now fit visibly without clipping or background conversion controls/footer showing through.
- Fullscreen End seeking showed the requested nonzero timeline position and the retained old image while loading; the current frame then replaced it with the **1.933 s/frame-58** marker. The decoded preview samples at 15 FPS, so End clamps to its last available sampled frame; this is not a promise of exact original-frame PTS selection.
- Tab followed by Escape exited fullscreen and kept the source player open, paused at the same frame/position. Inline low-saturation adjustment (1%) changed the image; entering and leaving fullscreen preserved both the adjusted image and position.
- Corrected native build, all-target native-app Clippy with warnings denied, formatting, whitespace checks and **12 app tests passed / one GPU diagnostic ignored**. The known GNU linker `.drectve` warning remains nonfatal.

Still unverified in this live pass: fullscreen **mouse** timeline/buttons (including Exit full screen, Mute and Before/After), active playback across toggles, light/multiple-monitor/DPI layouts, file-dialog selection, Close/reselection and last-output continuity. Computer Use rejected fullscreen coordinate inputs with `outside window bounds ... width: 6, height: 6` despite capturing the complete 1920×1080 window. A fresh activation restored ordinary-window mouse input; fullscreen coordinates remained rejected, so no mouse-control success is claimed there. Keyboard End/Tab/Escape succeeded. This is an automation-observed bounds discrepancy, not proof that physical mouse input is broken. Existing policy tests cover debounce/reselection and no repeated opens, but do not replace the pending live checks. No conversion or browser checks were rerun. No commit/push or complete M5/color acceptance claim.

#### Native bordered expanded preview — 2026-10-08

User reported a physical monitor input-search pause when entering/leaving borderless fullscreen and requested bordered fullscreen instead. Replaced the OS fullscreen request with ordinary `Window::set_maximized(true)`, keeping native decorations and the desktop work area. Store the previous maximized state once and restore it on exit; no `set_fullscreen` call remains in the adapter. Labels now read Expand preview / Exit expanded view. Same player/layout, codecs and conversion routes; no dependency changes. Architecture and README updated.

Verified: locked native build, formatting, native all-target Clippy with warnings denied, whitespace check and **12 app tests passed / one GPU diagnostic ignored**. Computer Use on Windows loaded CC0 `m2-h264-aac.mp4` automatically paused, then clicked Expand preview: the decorated maximized window captured at 1920×1032 on the 1920×1080 display, with complete image/timeline/buttons visible. Clicking Exit expanded view restored the original 960×760 client window and same paused frame. Re-entering and pressing Escape also restored the original window with preview still open.

Untested: whether the physical monitor resync has disappeared (window capture cannot establish link stability), already-maximized entry/exit restoration, active playback continuity and broader DPI/multi-monitor coverage. Ordinary title-bar maximization was observed, but Computer Use then rejected coordinates with the same 6×6 bounds error after a refreshed activation/retry; stopped that runtime check rather than claiming success. Previously pending expanded mouse seek/Mute/Before checks remain pending; the exit-button check above is now verified for the new bordered mode. No browser/conversion changes or complete M5 acceptance claim. No commit/push requested.

#### Separate native preview window — 2026-10-08

Implemented at user request: Expand preview now requests one separate maximized, bordered Blitz window titled Diaxus · Video Preview. Main controls/size and the inline playback owner remain unchanged. Reuse the player component with explicit non-owning mount/drop behavior: no second decoder/audio worker and no cancel on secondary close. Return inline, Close, native window CloseRequested and Escape remove only the extra view; main-window close removes both and reaches normal session shutdown. Ignore stale dropped-window events. Each view has its own renderer/presenter textures; both use the single cached CPU frame slot and shared transform math, not cross-device texture handles. A second visible presenter may add one preview upload per changed frame; conversion is untouched. Shared metadata/settings and path-tagged Before requests are bounded; window-local viewport math keeps main/secondary seek coordinates independent. Secondary paused redraws respond to changed frame/color revisions, not an unconditional redraw loop. README and architecture record the superseding window policy.

Verified by executable checks: locked native build (existing nonfatal GNU linker `.drectve` warning), formatting, native all-target Clippy with warnings denied, whitespace check and **13 app tests passed / one GPU diagnostic ignored**. The added fixture-backed presenter ownership test attaches/drops a secondary presenter five times and checks the same playback-control allocation, paused/muted state, position and frame revision survive; a paused color update changes only color revision and reaches both views without replacing decoded pixels. Explicit service shutdown still drains the worker and clears the frame. This test does not create an OS window or GPU surface.

Live two-window UI verification remains **pending**: Computer Use enumerated the existing converter, but activation/capture returned `foreground window did not report a process id`; selecting a fresh returned window and retrying capture produced the same error. Stopped input attempts. Earlier single-window maximization screenshots do not validate this implementation. Actual separate-window creation/layout, mouse controls, Return inline/Escape/native-close behavior, playback/audio continuity, repeated surface/resource teardown, simultaneous main controls/theme/color updates, source/output replacement and monitor/DPI/link stability are untested runtime coverage. No browser/conversion regression runs or commit/push requested.

Follow-up (2026-10-08): Arman reported testing the separate preview himself and that it works. This is user-reported basic runtime success, not independently observed coverage of every case above. Added video-image click to the shared native player: both inline and separate view use the existing Play/Pause action, ignore input until ready or while dragging, and keep seek/control clicks outside the video toggle area. Added pointer cursor, tooltip and inline hint. Formatting, whitespace checks, 13 app tests (one GPU diagnostic ignored) and all-target Clippy with warnings denied passed. Updating the normal executable was initially blocked by Windows `Access is denied` while removing the running `converter-native.exe`. After Arman closed the windows, `cargo build --locked -p converter-native` succeeded in 21.11 seconds with only the existing nonfatal GNU linker `.drectve` warning; the updated executable is ready. Live verification of this new click behavior is pending; no browser behavior or codec/conversion changes.

#### Browser color numeric and lifecycle follow-up — 2026-10-08

Environment: Windows, Codex in-app Chromium (`Chrome/155.0.0.0` user agent), WebGPU `intel / gen-12lp (BrowserWebGpu)`, Dioxus 0.7.10 / processing wgpu 30.0.1. GPU hardware codec execution is unknown. Used existing CC0 `fixtures/m2-h264-aac.mp4` (640×360, 60 frames, 30 FPS, two seconds, AAC) and the existing rotated/PAR `m35-geometry-color.mp4` for replacement-preview testing. No user media or new dependencies.

Implemented `tests/browser-color-numeric.mjs`, a transport-injected acceptance runner compatible with the supported Browser runtime's tab-scoped CDP capability and file chooser. It does not open a remote-debugging socket. Independent encoded-RGB reference math and histogram-based maximum/MAE/p95 comparison are unit tested. Export methods specifically target the 60-frame M2 fixture and original FPS, independently decode the first output frame, inspect all 60 timestamps and color tags, require audio for audio profiles, fully decode the file, and require the application's full re-decode/cleanup result. They do not numerically compare all output images.

Declared thresholds before testing: preview maximum ≤2 code values/channel; lossy output MAE ≤6 and p95 ≤15, separately retaining maximum error. Reference is the same-position/geometry neutral GPU image in encoded sRGB, not a CSS-rescaled screenshot. Main-thread tests use the explicitly enabled owned-frame diagnostic snapshot described in architecture.md; normal worker preview retains zero diagnostic readbacks. Test-only CPU comparisons/readbacks are outside conversion metrics.

Verified results (brightness 10, contrast 150%, saturation 50%, original geometry/FPS and recommended bitrate unless stated):

| Context / boundary | Maximum | MAE | p95 |
|---|---:|---:|---:|
| WebCodecs worker preview | 1 | 0.00968 | 0 |
| WebCodecs main fallback preview | 1 | 0.00968 | 0 |
| Worker/main VP8 video-only export | 105 | 3.43369 | 12 |
| Worker H.264/AAC export with Before selected | 88 | 1.87831 | 8 |
| Fresh-device H.264/AAC retry | 88 | 1.87831 | 8 |
| Corrected FFmpeg WASM H.264/AAC export | 109 | 1.79178 | 10 |
| Corrected FFmpeg WASM neutral export | 146 | 2.05382 | 11 |
| Rotated/PAR replacement preview (154×360 diagnostic image) | 1 | 0.04518 | 0 |

All listed lossy exports passed the unchanged MAE/p95 limits; their large isolated edge/chroma maxima are **not** a two-code-value shader pass. Independently verified 60-frame original-rate grids within 1.1 ms, 640×360 YUV420 8-bit output, full-file decoding and audio presence on AAC cases. The application independently verified its complete timestamp/duration timeline and decoded audio; physical A/V synchronization remains untested. AAC output video begins at the shared priming-adjusted origin, so the independent grid check preserves its first PTS instead of requiring an incorrect zero origin.

Verified interaction/lifecycle:

- In both worker and main fallback, 60 synchronous brightness input events published one render with unchanged cache-load count (2→2), leaving the final value 19. Five rapid seeks ended at 1.25 s and published one new cached image (loads 2→3). Same-position Before/After numerical comparison had maximum 1; Before retained saved export settings. Reset plus return to 0 s restored the original image exactly (maximum 0). These are burst/coalescing tests, not sustained interactive latency measurements.
- Worker source replacement loaded the rotated/PAR fixture and passed the combined preview comparison above. Its displayed input is 180×421, codec-safe output 180×420; the bounded preview is 154×360. No oriented export was rerun in this pass.
- Adjusted worker/main early cancellation occurred after one decoded frame and before GPU submission; no partial download was published, all stage live counters and application-held frames/samples ended at zero, and the adjusted retry passed. This does not establish cancellation during a saturated GPU queue.
- Controlled main-context device destruction after four completed GPU frames failed visibly, discarded four encoder callbacks, and cleaned all held frames/samples. Retry used GPU generation 2 and passed H.264/AAC pixel/timeline/audio validation without silently changing route.
- WebCodecs conversions reported 60 ingress copies/canvas captures, zero conversion pixel readbacks, no bitmap fallback, zero final leases and peak four leases. Normal preview retained one cached frame. The explicit main diagnostic retained one replaceable CPU snapshot and reported one readback per preview render.
- FFmpeg WASM retained its intentional 60 RGBA readbacks (55,296,000 cumulative bytes), four-slot/4 MiB live ring, peak four ready slots, and no raw OPFS spool. Adjusted run sampled 32 MiB peak allocated WASM linear memory, terminated its software encoder worker, finalized streamed OPFS output and cleaned frames/samples to zero. Total browser/process memory and hardware execution were not measured.

Real defect fixed: FFmpeg WASM raw-ring output formerly omitted matrix/transfer/primaries/range tags and relied on FFmpeg's default RGB→YUV matrix. Its encoder now explicitly converts full-range sRGB RGBA to limited BT.709-matrix YUV420 and tags BT.709 primaries plus **sRGB transfer**. Adjusted and neutral independent export comparisons above pass with the new tags. This is the existing WebCodecs→wgpu→RGBA-ring→FFmpeg software encoder, **not** a GPU-free direct FFmpeg browser route.

Verification pitfalls retained as evidence: the first VP8 comparison used FFmpeg's implicit matrix and failed (MAE 9.52/p95 27). FFprobe reports BT.709 on the Matroska stream but `bt470bg` on its decoded VP8 frame. Tests now explicitly honor and assert stream matrix/range/transfer before comparing in sRGB. At 8 Mbps, explicit BT.709 decoding yielded MAE 1.22/p95 5 versus MAE 8.02/p95 27 with BT.601. Reading an HTML WebGPU canvas after presentation returned a cleared image; a subsequent CSS/compositor capture was not a valid pixel-exact reference either. Those abandoned attempts are not counted as application passes. One reload also exceeded the 30-second worker startup budget and fell back visibly; fresh-context worker tests subsequently passed. Reload/startup soak coverage remains pending.

**Historical failing gate (resolved in the texel-center follow-up below):** brightness −20, contrast 200%, saturation 200% produced maximum **3**, MAE 0.00803, p95 0 against the captured neutral 8-bit reference. This exceeded the unchanged ≤2 limit. Input/reference quantization amplified by extreme controls was a possible explanation, not established proof. Zero saturation (10/150/0) passed at maximum 1; zero contrast and ±100 brightness clipping passed at maximum 0. The failed evidence remains retained.

Build checks passed: 21 media-core tests, one ui test, three numeric-runner unit tests; locked host core/GPU check; locked wasm32 media-web/converter-web all-target Clippy with warnings denied; formatting, JavaScript syntax and whitespace checks. `dx serve --web --port 8084 --open false --interactive false --cross-origin-policy` built/served successfully. Cargo checks explicitly used 1.98.1 Windows GNU; no toolchain/dependency upgrades.

Evidence (ignored local artifacts): `tmp/color-worker-final-20261008/evidence.json`, `tmp/color-main-diagnostic-20261008/evidence.json`, `tmp/color-recovery-20261008/evidence.json`, `tmp/color-wasm-fixed-20261008/evidence.json`, `tmp/color-replacement-20261008/evidence.json`; retained extreme failure in `tmp/color-numeric-final-20261008/evidence.json`. Raw neutral diagnostic reference and exported outputs are beside the corresponding JSON. Use the runner's `createColorAcceptance({ send, selectFile, navigate, directory })` with the supported Browser runtime; supply tab CDP `send`, ordinary tab navigation, and a file-chooser callback. Main numeric URLs must include `execution=main&diagnostic-preview=1`; normal worker URLs omit those flags. `node --test tests/browser-color-numeric.test.mjs` runs only reference/metric tests, not real-browser acceptance.

Remaining at that point: strict extreme-control reference failure (subsequently resolved below); Firefox/other-browser numeric reruns; exhaustive VFR/fixed-FPS/oriented exports with adjustments; active/saturated-GPU cancellation; long-running/repeated replacement/EOF/context-loss stress; physical A/V and visible latency; expanded-native-window/DPI/audio continuity checks already listed elsewhere. CLAHE and native codec-surface interop remain unimplemented and out of this slice. No complete M5/color acceptance or commit/push claim.

#### Extreme-color texel-center correction — 2026-10-08

Verified on Windows, in-app Chromium 155 / Intel gen12lp, using the deterministic CC0 M2 H.264/AAC fixture (640×360, 60 frames, 30 fps). The original-size main-thread diagnostic now reads the exact GPU input texture in addition to the owned output frame. This isolates sampler/color correctness from neutral-preview rounding and browser presentation. Diagnostic mode performs two preview-only readbacks, destroys its mapped staging buffer and clears snapshots; normal preview/conversion performs none.

**Diagnosis:** the old shader still failed against exact ingress: brightness −20, contrast 200%, saturation 200% had maximum error **4**, MAE 0.008255, p95 0; 34 pixels exceeded two code values, concentrated at sharp boundaries. For example, input RGB (95,11,0) should produce (145,0,0), but produced (148,0,0) beside a bright neighbor. Neutral-reference quantization alone therefore did not explain the discrepancy. Preserved failing evidence: `tmp/color-ingress-20261008/evidence.json` and raw pixels.

**Correction:** shared WGSL uses `textureLoad` within 0.0001 texel of an exact texel center and retains bilinear `textureSampleLevel` elsewhere. At an exact center these should agree; the correction avoids tiny coordinate/filter-weight quantization bleeding a neighbor into an otherwise exact sample. No canonical color-math, two-code-value threshold, uniform layout, production pass or CPU transfer changed.

Verified results:

- Exact-ingress extreme preview: maximum **1**, MAE **0.000078125**, p95 **0**. Neutral, zero contrast and ±100 brightness clipping: maximum **0**. Positive extreme brightness, ordinary combined settings and zero saturation: maximum **1**.
- Normal dedicated-worker extreme preview, with no diagnostic URL/readbacks: maximum **1**, same MAE/p95. Both main and worker extreme H.264/AAC exports passed separate lossy limits (MAE **2.20270**, p95 **6**, limits 6/15). Isolated lossy maximum **239** is not a shader-reference result. All 60 rational-grid timestamps, audio presence, full-file decode, metadata normalization and application re-decode/cleanup checks passed.
- Native real-GPU regression passed on Intel UHD / Vulkan for both `Rgba8Unorm` and `Rgba8UnormSrgb`: a dense 640×360 eight-color edge raster, seven settings per attachment including the formerly failing extreme case, all RGB errors ≤2 and opaque alpha. This is processing-shader evidence, not a live Blitz presentation or monitor-color test.
- M1 dedicated-worker resize regression passed: **30/30** frames to 160×90 VP8 and back, orientation/RGB markers and timestamps within ±1 µs, zero owned frames after cleanup. Peak live frames 8, decoder submissions 4, callbacks 6, encoder submissions 4, verifier submissions 4/callbacks 5. Conversion had zero explicit CPU pixel readbacks; verification had 30. Elapsed 199.1 ms includes verification and is not a throughput claim. Hardware codec execution/browser-internal copies remain unknown.

Build checks passed: Rust formatting; locked wasm32 media-web/converter-web and host converter-native Clippy (all targets, warnings denied); 21 core tests, one UI test, three numeric-runner tests; the explicitly enabled native GPU test above; JavaScript syntax and whitespace checks. Dioxus web build/serve succeeded using pinned Rust 1.98.1 Windows GNU. No dependency/toolchain upgrade.

Evidence: `tmp/color-centers-final-20261008/evidence.json` (exact-ingress main), `tmp/color-centers-worker-20261008/evidence.json` (normal worker), raw references/exports beside them, and worker `m1-status.txt` / `validation.png`. The runner accepts `reference: 'ingress'` only for original-size unrotated square-pixel comparisons and retains the old rounded-preview metric separately.

Still untested in this follow-up: Firefox color numeric/export and other GPU reruns, adjusted resized/oriented and VFR/fixed-FPS exports, extended replacement/EOF/device-loss and saturated-GPU stress, physical A/V/latency, and expanded-native-window/DPI/audio continuity. The subsequent user-run Firefox M1 regression below covers neutral resize only. No full M5/color acceptance, CLAHE implementation, commit or push is claimed.

#### User-run Firefox resize regression — 2026-10-08

Arman ran M1 in the Firefox session discussed in this chat and supplied the complete PASS summary after reloading the restarted Dioxus development server. This is user-run browser evidence, not agent-observed automation. Computer Use found the Firefox window but refused capture because browser URL-policy enforcement is unsupported for that Windows browser. Exact Firefox version and GPU identity were not included in the supplied result; do not infer them from earlier tests.

- **30/30** frames decoded → GPU-resized → encoded → re-decoded; output **160×90 VP8**; timestamps within **±1 µs**; orientation and RGB markers passed.
- Dedicated worker; startup probe **707.5 ms**, context reused for this command=false. Total **250.0 ms** includes verification, not conversion throughput.
- **30** VideoFrame→texture API copies and **30** additional VideoFrame→ImageBitmap compatibility conversions; zero explicit CPU pixel readbacks in conversion, **30** test-only readbacks. Browser-internal copies and codec hardware execution remain unknown.
- Peak owned live frames **6**, decoder submissions **4**, callbacks **5**, encoder submissions **3**, verifier submissions **4**, verifier callbacks **5**. Cleanup: **0** application-owned live frames.
- Device generation **1**; input pool slots **4**, lifetime allocations **4**, reuses **26**, leases live/peak **0/4**, ingress copies/canvas captures **30/30**. CPU submission/bridge **85.5 ms**, cumulative submitted-work completion latency **159.5 ms**, slot-reuse wait **0.5 ms**, final drain **0.0 ms**; completion latencies may overlap and are not pure GPU execution. Timestamp queries unavailable/not requested.

This confirms the shared-shader neutral downscale/orientation round trip on that Firefox session. It does not establish adjusted-color numeric accuracy, adjusted exports, audio behavior or stress acceptance. Those Firefox checks remain pending.

#### Browser source preview playback — 2026-10-08

Implemented Play/Pause, image click-to-toggle, focused-image Space/Enter, Mute and seeking. Browser-native source audio supplies playback time; a persistent WebCodecs/Mediabunny preview decoder feeds the existing shared wgpu resize/color shader through the serialized worker/main transport. One awaited preview request at a time with a 34 ms minimum interval; one cached original and one lookahead, with the pinned library's separate bounded decoder queues. No encoder, raw-frame spool, full-video frame buffer or production pixel readback was added. Backward/large seeks recreate the iterator; late preview frames may be skipped without changing conversion's frame policy. Pause/EOF retires lookahead, replacement/errors/conversion clear the source cache, and source replacement/page exit revokes the audio Blob URL. Native media errors remain visible in a dedicated alert, rather than being overwritten by a paused-preview status.

Verified in real in-app Chromium on Windows / Intel gen12lp, both **dedicated worker** and explicit **main-thread fallback**, with the CC0 M2 fixture and its ignored ten-repeat stream-copy fixture (`ffmpeg -n -stream_loop 9 -i fixtures/m2-h264-aac.mp4 -c copy tmp/preview-click-20261008.mp4`, 600 video frames, 20.192 s, 2,783,219 bytes):

- Playback advanced the timeline and processed-image marker, retaining **one decoder stream** during uninterrupted forward playback; cached frames=1, lookahead=1, explicit pixel readbacks=0. Clicking the image paused and kept the timeline stable; paused lookahead=0.
- Paused seek to **4.00 s** produced decoded source-frame time **3.988020 s**, within one fixture frame. A playing seek to approximately **8.11 s** produced frame time **8.085416 s** with a live grayscale adjustment. These are decoded-PTS checks, not monitor/audio synchronization measurements.
- EOF changed Pause to Play and retired lookahead; Play restarted near zero. Replacing the source during playback stopped the old session and loaded the new input paused at zero.
- Convert during playback stopped the preview/audio clock and produced a fully re-decoded **60-frame 640×360 WebM/VP8/Opus** with Original timestamps and saved saturation=0. Both paths reported **95 decoded input audio samples**, **102 Opus packets**, audible-data peak **0.0424** in output verification, maximum container timeline quantization **667 µs**, **0** remaining conversion frame/sample references and **0** conversion pixel readbacks. The rebuilt worker run measured 367.8 ms conversion + 26.7 ms diagnostic verification (394.7 ms total); these are short-fixture observations, not playback performance guarantees.
- The FFmpeg WASM page also played the source through the same preview, reached EOF and accepted Space/Enter and Mute; this was a preview smoke test, not another FFmpeg export acceptance run.
- M1 after playback passed **30/30** frames at 160×90 VP8, orientation/RGB markers and ±1 µs timestamps, zero owned live frames and no compatibility bitmaps on this Chromium path. Elapsed **237.1 ms** includes verification. Peak live frames 7, decoder submissions 4/callbacks 6, encoder submissions 4, verifier submissions 4/callbacks 5; zero conversion readbacks, 30 test-only readbacks.

Also corrected the pipeline asset's HTML script type to `module`; the initial classic-script load had thrown `Cannot use import.meta outside a module` while dynamic import subsequently worked. Fresh loads use the module type. Grouped transport/seek controls across the full monitor width at the observed narrow viewport and bounded the diagnostic preview text locally; actual screenshots verify this layout. A clean Dioxus serve build was rerun rather than treating hot-reloaded RSX alone as the final build.

Checks passed: formatting, whitespace and JS syntax; locked wasm32 media-web/converter-web all-target Clippy with warnings denied; host core/GPU/web check; 21 core tests, one UI test; clock lifecycle/offset/EOF/error/stale-source tests plus three numeric-reference tests. Existing Windows GNU linker `.drectve` warnings did not fail the host tests. No dependency changes. The transport-injected `tests/browser-preview-playback.mjs` uses the supported Browser runtime and persists per-case evidence; it opens no external browser-control socket.

Evidence (ignored): `tmp/browser-playback-rebuilt-20261008/evidence.json`, `tmp/browser-playback-rebuilt-main-20261008/evidence.json`; earlier timestamp-specific runs in `tmp/browser-playback-worker-pts-20261008` and `tmp/browser-playback-main-pts-20261008`; final grouped-layout `tmp/browser-playback-rebuilt-20261008/player-controls.png`; M1 text in `tmp/browser-playback-worker-final-20261008/m1.txt`.

**Skipped at Arman's request (2026-10-08):** remaining Firefox checks, including playback and adjusted-color numeric/export reruns. These remain unverified, not passed. Earlier user-run Firefox M1 and visual preview evidence is unchanged. Computer Use had rejected the Firefox browser URL-policy boundary before this testing-scope decision.

**Not verified:** physical preview sound/long-run A/V drift, unusual native-media origin/edit-list mapping, video-only and HEVC native audio-clock compatibility, hidden-tab throttling, autoplay-denial UI in a real browser, long-run memory/renderer loss and broad DPI/GPU coverage. Preview native-media buffers/internal copies/hardware codec execution remain unknown. No full M5 completion, CLAHE, native changes, commit or push is claimed.

#### Native video-click live check — 2026-10-08

Computer Use launched the rebuilt executable and successfully captured/controlled the ordinary Windows converter. The CC0 two-second M2 fixture loaded automatically paused; clicking its image started playback. For an unambiguous pause check, generated ignored `tmp/preview-click-20261008.mp4` with `ffmpeg -n -stream_loop 9 -i fixtures/m2-h264-aac.mp4 -c copy ...` (ten repetitions, no re-encode; FFprobe duration 20.192 s, 2,783,219 bytes). The preview displays 20 s.

Verified live: clicking the inline image changed Play to Pause and advanced the timeline; clicking it again changed Pause to Play at approximately 18 s before end-of-stream. A subsequent timeline click sought to approximately 4 s with Play still shown and a changed decoded marker, without toggling playback. Expand preview then created a distinct maximized, bordered Diaxus · Video Preview window while the converter remained open. The second window captured at 1920×1032, showed complete image/controls and retained the same paused approximately 4 s position and marker. This supersedes the earlier capture blocker for these checks only.

Still unverified: separate-window image-click start/pause and mouse seek/buttons. Both initial and fresh-window/activation/screenshot-ID retries were rejected by Computer Use with `outside window bounds ... width: 6, height: 6` despite the full-size screenshot. An attempted keyboard Escape was not injected: the tool reported user input detected and required re-observation, so no Escape success is claimed in this pass. Stopped further input; both windows remained open. Physical audio continuity, monitor-link stability, repeated renderer teardown and broader DPI/GPU coverage remain pending. No application fix or build rerun was needed; no conversion/browser test or commit/push performed.

#### Adjusted browser resize/orientation numeric gate — 2026-10-08

Continued the slider/preview acceptance work, not CLAHE or a new milestone. Windows, in-app Chromium 155.0.0.0, Intel gen-12lp / BrowserWebGpu; existing pinned dependencies unchanged. Firefox checks were skipped at Arman's request. Reused checked-in CC0 M2 and M3.5 geometry fixtures; generated evidence/output files only under ignored `tmp/browser-color-geometry-20261008`.

`tests/browser-color-numeric.mjs` now has an independent test-only geometry oracle: forward-rotate texels, reflect the oriented raster, bilinearly resize with clamped edges, then apply the color chain without intermediate rounding. Diagnostic main-thread tests use actual normalized RGBA8 ingress, so this verifies shared geometry/color after browser crop/PAR normalization, not an independent decoder/color-normalization implementation. Worker tests use the same fixture/frame's recorded main-thread ingress and require a neutral geometry match before adjusted comparisons. No production CPU transform/readback was introduced. Two new unit tests cover asymmetric rotation/reflection and resize-before-adjustment/clamping. The shader maximum-error limit remains **2 code values/channel**; lossy MAE ≤6 and p95 ≤15 remain unchanged.

Verified preview results (full raster, not screenshots):

| Source / output | Context | Neutral / combined / extreme maximum error |
|---|---|---|
| M2 640×360 → 320×180 | Main diagnostic and worker | 1 / 1 / 2 |
| M3.5 normalized 421×180, clockwise 90° then horizontal flip → 90×210 | Main diagnostic and worker | 1 / 1 / 2 |

Combined settings are brightness +10, contrast 125%, saturation 60%; extreme settings are brightness 0, contrast/saturation 200%. Main diagnostics count two preview-only readbacks; ordinary worker preview and WebCodecs conversion report zero explicit pixel readbacks. The worker comparison against an already-rounded neutral preview initially gave maximum 3 for extreme resize; the independent unrounded-ingress oracle gives maximum 2 in both contexts. This reference-rounding effect is recorded, not used to relax the shader tolerance.

Verified **320×180**, combined settings, **8 Mbps custom target** exports:

| Backend / context | Profile | MAE / p95 / maximum vs pre-encoder reference |
|---|---|---|
| WebCodecs main | H.264/AAC | 2.8460 / 13 / 113 |
| WebCodecs main | VP8/Opus | 2.4906 / 13 / 110 |
| WebCodecs worker | H.264/AAC | 2.8460 / 13 / 113 |
| WebCodecs worker | VP8/Opus | 2.4906 / 13 / 110 |
| FFmpeg WASM worker | H.264/AAC | 2.4196 / 13 / 110 |

Each passed full browser re-decode plus independent FFprobe/FFmpeg: all 60 frames on the 30 fps timeline (≤1.1 ms container tolerance), audio track present, full A/V decode, correct dimensions, limited-range BT.709 matrix/primaries with the reported accepted transfer, no residual display matrix, and zero application-owned frame/sample references after conversion. Optional absent H.264 SAR signaling is not treated as an error; any explicit non-1:1 SAR fails, and browser re-decode independently validates square-pixel display/coded geometry. WebCodecs retained zero conversion readbacks; FFmpeg retained its explicitly counted existing raw-ring readbacks, not the WebCodecs fast-path claim. This is correctness coverage at the selected bitrate, not performance or hardware-codec proof.

**Failing/open lossy boundaries, retained in evidence:**

- M2 resized VP8/Opus at the recommended 250 kbps target: MAE 4.3116, p95 18, maximum 113. Frame/timing/audio/cleanup validation passed, but the lossy pixel gate failed. The separate 8 Mbps runs above passed; no recommendation or bitrate-policy change is claimed.
- M3.5 90×210 combined H.264/AAC at 8 Mbps: MAE 5.2539, p95 28, maximum 180. All 24 frames, 24 fps timestamps, geometry, audio/decode and cleanup checks passed; pixel acceptance did not. An additional grayscale case (+10 brightness, 125% contrast, 0% saturation) still had MAE 2.9895, p95 16, maximum 137.
- To isolate the latter, the runner's `isolateEncoder` diagnostic encoded the already numerically verified worker preview directly with `avc1.42001e`, 90×210, 8 Mbps, 24 fps, then decoded/copied that single frame. It bypassed demux, GPU processing and the conversion job. MAE 2.9641, p95 16, maximum 137 reproduced the discrepancy; returned color space was limited BT.709 matrix/primaries and sRGB transfer. This narrows the error to the canvas-frame/codec round-trip boundary, not a proven cause within it. The grayscale result means chroma subsampling alone is not a sufficient explanation. No shader change or looser acceptance threshold was justified. This diagnostic uses test-only frame copies and is not conversion throughput.

Passed evidence: `main-half-high-final`, `worker-half-exact`, `worker-oriented-exact`, `ffmpeg-half` (each has `evidence.json`). Open/failing evidence: `main-half`, `main-oriented`, `oriented-gray`, and diagnostic-only `codec-isolation`. Earlier `main-half-high` failed a too-strict test assertion on absent optional H.264 SAR, corrected before the final rerun. Earlier `worker-half` used the rounded-preview reference described above; its failed extreme result remains recorded. Failed runs were not relabeled passed.

Checks passed this continuation: six Node tests (five numeric-reference tests and one clock lifecycle test), locked wasm32 media-web/converter-web all-target Clippy with warnings denied, Rust formatting and whitespace/JS syntax checks. Changes in this continuation are acceptance tooling/documentation only; no production renderer, codec, native UI or dependency changes. High-detail oriented export pixel acceptance remains open, along with the previously recorded native selected-geometry/10-bit preview approximation, physical A/V synchronization, broader platform/DPI/long-run coverage and skipped Firefox checks. No complete color acceptance gate, M5 completion, CLAHE, commit or push is claimed.

#### Non-aligned export capture and adaptive bitrate fixes — 2026-10-08

Arman requested fixes to the two boundaries above. Same Windows/Chromium 155/Intel gen-12lp environment and licensed fixtures; no dependency upgrade or native codec-route change. Firefox checks remain skipped, not passed.

**Cause isolation / production fix:** the 90×210 grayscale preview still passed the shader maximum-error gate. Encoding a CPU-backed diagnostic copy reduced p95 from 16 to 3. Discarding alpha or copying to another exact-size canvas did not fix GPU-backed encoding. A 96×224 backing with an explicit unchanged 90×210 visible crop did. Production now renders directly to 16-aligned backing, with the original viewport and one opaque cropped VideoFrame capture—no extra canvas, render pass or CPU readback. This validates a non-aligned GPU-backed frame/codec boundary workaround; it does not identify the exact Chromium/driver implementation defect. Actual encoder/display dimensions, pixels' orientation, timestamps and audio are unchanged. Padded dimensions are checked against the device limit and included in reconfiguration identity; pool/GPU completion rules stay intact. Preview remains exact-size, so paused images do not gain padding borders. Telemetry reports capture visible/backing dimensions. The FFmpeg ring reads the captured visible rectangle, not raw padding bytes.

**Bitrate fix:** source-aware Recommended previously reduced bitrate linearly with area and capped it at the fallback heuristic, starving this detailed half-size clip. Core now uses sqrt(output/source area) with the existing FPS/codec factors and no heuristic cap for a known source rate. Integer fixed-point arithmetic, explicit custom bitrate, preset multipliers and 0.25–120 Mbps limits are retained. Unknown-source heuristic is unchanged. Both UIs/estimates consume this shared policy; the fixture now recommends **503,098 bps**, not a hard-coded 8 Mbps. This is a measured improvement for this fixture, not universal quality or exact-size assurance.

Final verified results:

| Case | Context/backend | Raw RGB MAE / p95 / maximum | Declared-4:2:0-reference MAE / p95 / maximum |
|---|---|---|---|
| 90×210 grayscale H.264/AAC, 8 Mbps | WebCodecs worker/main | 0.6656 / 3 / 45 | Not needed |
| 90×210 combined H.264/AAC, 8 Mbps | WebCodecs worker/main | 4.0172 / 21 / 133 | 2.3950 / 11 / 77 |
| 90×210 grayscale H.264/AAC, 8 Mbps | FFmpeg WASM | 0.1021 / 1 / 3 | Not needed |
| 90×210 combined H.264/AAC, 8 Mbps | FFmpeg WASM | 3.5900 / 20 / 133 | 1.7832 / 10 / 65 |
| 320×180 combined VP8/Opus, Recommended 503,098 bps | WebCodecs worker/main | 3.2424 / 15 / 110 | Not used; original RGB gate passes |

Combined remains +10 brightness / 125% contrast / 60% saturation; grayscale changes saturation to 0. The ≤2 pre-encoder gate passes, with no shader/tolerance change. Every export passed full browser re-decode plus independent FFprobe/FFmpeg for all 24 portrait frames at 24 fps or 60 landscape frames at 30 fps, dimensions, orientation baked into pixels, accepted SDR matrix/range/transfer, audio presence/full decoding and zero application-held frame/sample references after cleanup. WebCodecs reports zero conversion pixel readbacks; FFmpeg WASM retains its explicitly counted RGBA ring readback per frame. Neither capture padding nor codec support proves hardware execution/internal zero-copy.

**Important acceptance clarification:** saturated high-detail output necessarily loses spatial chroma information in 4:2:0. Its raw RGB p95 **still exceeds 15** and is retained above. A new, explicitly selected lossy reference applies the declared BT.709 matrix, limited range, sRGB transfer and left chroma sampling in FFmpeg/zscale without a codec, then reconstructs RGB. Against that representable reference, unchanged MAE ≤6/p95 ≤15 limits pass. This changes the lossy reference domain, not its numeric threshold; it does not relabel the old raw RGB result passed. The helper rejects other formats, siting or transfers. The pre-encoder shader oracle remains independent/unrounded and unchanged. The raw-versus-sampled comparison separates unavoidable profile loss from unintended conversion error; it is not evidence for 4:4:4 preservation or an arbitrary footage quality guarantee.

Regression checks: M1 passed worker and explicit main-thread runs (30/30 frames, 160×90 VP8, markers/orientation and ±1 µs timestamps, owned frames zero, 30 ingress copies, zero conversion readbacks, 30 diagnostic readbacks). Backing was 160×96; times 234.8/229.2 ms including verification are not throughput claims. Eight playback/seek/EOF/restart/replacement/conversion-handoff cases passed after the final rebuild. Portrait cancellation and same-page retry passed with cleanup zero; controlled device destruction after four frames cleaned all resources and retry advanced device generation to 2, with verified 90×210 H.264 output. Shader preview remained exact-size across export reconfiguration. An early browser-control attempt timed out before page initialization; subsequent stable reload and the final FFmpeg rerun passed, not silently counted as that attempt succeeding.

A final screenshot-only inspection after repeated context recreation selected the existing main-thread fallback with the visible reason `worker startup exceeded 30 seconds`. Source inspection, profile availability, 320×180 resolution and the 0.50 Mbps recommendation/size estimate still displayed. The worker-only numeric harness could not capture this non-diagnostic main context, so this final attempt is not an additional pixel/export pass. Saved `recommended-final.png` and closed the test tab. The startup-timeout cause was not diagnosed here and remains separate reliability coverage; it does not replace the successful worker/explicit-main runs above.

Evidence: ignored `tmp/browser-codec-fix-20261008/{storage-probe,alpha-probe,pad-probe,copy-probe,aligned-oriented,rate-probe,final-oriented-worker,final-oriented-main,final-recommended-worker,final-recommended-main,final-ffmpeg,recovery,cancel,playback}/evidence.json` and `m1-{worker,main}.txt`. Earlier failing files remain intact. Some final color runs preceded the telemetry-only display addition; final FFmpeg/M1/playback/recovery reports include the visible/backing telemetry.

Build checks passed: 22 core tests plus one UI test, six Node numeric/clock tests, host core/GPU/web all-target Clippy and wasm32 media-web/converter-web all-target Clippy (warnings denied), locked native/GPU check, Rust formatting, JS syntax and whitespace. Native policy is shared/tested/buildable but its live UI/export quality was not rerun this turn. Other GPUs/browsers, physical A/V drift, arbitrary DPI/long-run stress and HEVC output remain unverified; Firefox skipped. CLAHE, a complete M5 acceptance claim, commit and push remain out of scope.

#### Adjusted timing/Main 10 acceptance and worker startup diagnostics — 2026-10-08

Arman authorized the next validation/reliability slice. No CLAHE, native codec-surface interop, dependency upgrade or codec/transform change. Reused checked-in CC0 fixtures only. Environment: Windows; Rust 1.98.1 GNU; FFmpeg 8.0.1 essentials; native shared routes selected NVIDIA GeForce RTX 3050 Laptop GPU / Vulkan; browser in-app Chromium 155.0.0.0, Intel gen-12lp / BrowserWebGpu. Firefox remained skipped at the user's request.

**Implemented:** `tests/native-color-timing.mjs`, injected-runtime `tests/browser-color-timing.mjs` and `tests/browser-startup-soak.mjs`. The pure test oracle derives complete CFR grids from exact stream ticks with BigInt, including first-index rounding and exclusive-end ceiling, rather than rounded FFprobe duration text or the application's reported count. Original compares every normalized input PTS. Browser AAC priming packets and native audio stream starts have different demux origins; each oracle uses its actually exposed common A/V origin. A two-frame EOF regression protects against an extra rounded-endpoint frame. Browser numeric tooling accepts explicit expected timestamp arrays for VFR/fractional FPS without weakening its pixel or timing gates.

**Verified exports:** all use brightness +10 / contrast 80% / saturation 60%.

| Input and route | Output policy | Frames | Numeric evidence |
|---|---|---|---|
| 36-frame nonzero-origin VFR, native direct CPU/shared software/shared NVIDIA | H.264/AAC; Original, 15, 30000/1001 | 36 / 30 / 59 on each route | First-frame RGB16-normalized lossy MAE 1.63–1.67, p95 7.88–8.06 in 8-bit-equivalent code values |
| 24-frame Main 10 gradient, native direct CPU | HEVC Main 10/AAC; Original, 15, 30000/1001 | 24 / 15 / 30 | Original-size first-frame lossy MAE 0.1963, p95 0.5097 (8-bit equivalent) |
| Same Main 10 input, native direct CPU | 50% → 80×48; 15 FPS | 15 | Depth/geometry/timing/audio/full decode verified; resized pixel parity not measured in this case |
| 36-frame VFR, Chromium worker and explicit main | H.264/AAC and VP8/Opus; Original, 15, 30000/1001 | 36 / 30 / 59 per profile/context | Pre-encoder combined preview maximum 1; first-frame H.264 MAE 2.72–2.97/p95 11–12; VP8 MAE 2.95–3.80/p95 11–13 |
| 60-frame M2 CFR, Chromium FFmpeg WASM worker | H.264/AAC, 8 Mbps; 15 and 30000/1001 | 31 / 60 | Preview maximum 1; first-frame lossy MAE 0.9724/0.9992, p95 4/4 |

The FFmpeg 15 FPS result has 31 frames, not 30: the common browser origin includes AAC priming, and the selected exact grid covers that interval. Native Main 10 uses its actual native stream origin; it does not acquire that extra browser interval. These are checked grid policies, not playback speed changes.

All 13 native exports independently passed every output PTS/count, geometry, square pixels, no residual display matrix, BT.709 limited SDR tags, AAC start/duration (≤25 ms tolerance), non-silent decoded PCM (peak 0.1507–0.2310) and full independent A/V decode. Maximum native frame-grid error was 367 µs. Each of the 12 WebCodecs exports and two final FFmpeg exports passed full browser output re-decode/timeline/audio checks and independent FFprobe/FFmpeg full decoding, exact expected complete grids (1.1 ms independent tolerance), geometry and stream color tags. WebCodecs cleanup was zero application-held frames/samples, with zero conversion pixel readbacks; GPU leases stayed at the four-slot bound. FFmpeg retained its explicit per-output-frame RGBA readbacks (31/60), one-frame-plus-4-MiB ring, worker termination and zero application ownership after cleanup. None of this proves browser-internal copy counts or codec hardware execution. Existing native CPU-staged bridge accounting is unchanged.

**Main 10 precision gate:** a new Rust test runs the actual direct-filter RGB16 prefix on the 160×96 checked-in Main 10 fixture, then compares all 46,080 encoded-RGB channel samples against the pure color reference. Maximum pre-encoder error was **1/65535** (unchanged ≤2 gate); 45,887 adjusted samples contained sub-8-bit precision. Deliberately quantizing the normalized input to 8-bit before applying the reference produced maximum disagreement **102/65535**, demonstrating this fixture distinguishes an 8-bit intermediate. Exported Original-size Main 10 luma had 11,718 codes not divisible by four, and the half-size export had 2,870; those counts alone would not prove preserved precision without the pre-encoder/reference test. Codec output remains lossy; native preview remains 8-bit, not a 10-bit display proof. Numeric export comparisons here sample the first frame, not every frame's pixels; every frame's timing and full decoding are verified.

**Startup investigation / limited fix:** removed a second `navigator.gpu.requestAdapter` formerly awaited solely to obtain a GPU label. Identity is now read synchronously from the actual processing device's `adapterInfo`, with selected wgpu adapter information as fallback. This avoids an unnecessary startup dependency and possible mismatched identity; it does not establish that request caused the earlier timeout. Worker bootstrap emits fixed phase progress through wasm import/init, codec imports, actual WebGPU adapter/device requests, pipeline creation and surface capture. Window transport retains at most 12 cumulative records, separates them from job progress, includes them in conversion/M1 summaries (not the compact preview trace), and names the last stage on the unchanged 30-second deadline. Timed-out workers are terminated; fallback remains visible and late replies are ignored.

Six real fresh-page worker startups passed complete 30/30 M1 decode→GPU→encode→re-decode with owned frames zero, 30 ingress API copies, zero conversion readbacks and 30 diagnostic readbacks. Startup times were 240.0, 283.0, 270.8, 286.1, 310.3 and 312.2 ms. Color matrix startup traces also exercised the real adapter/device/pipeline/capture phases. A deterministic **transport unit test**, not a real driver-hang test, stalled the device-request phase, accelerated only the fixed deadline, verified its named failure/fallback/termination and ignored late ready. The prior intermittent real-browser timeout did not reproduce; its original cause and arbitrary driver/browser stall recovery remain unverified. No general runtime watchdog, silent job restart or enlarged timeout was added.

The pre-existing `native-color.mjs` smoke/first-frame comparisons and rejection checks were also rerun successfully. Build checks: 22 core + 30 native + one UI tests passed; five native device/audio integration tests were ignored in this default suite (their earlier evidence is unchanged). New Main 10 diagnostic passed explicitly. Fifteen Node tests passed. Locked wasm32 media-web/converter-web and host core/GPU/native/native-app all-target Clippy passed with warnings denied; formatting passed. Test executable linking retained the previously recorded nonfatal GNU `.drectve` warnings. The native UI itself was not operated in this turn.

Evidence remains ignored under `tmp/color-timing/{native-19320,browser/worker,browser/main,ffmpeg-final,ffmpeg-final-build,ffmpeg-proof,startup,startup-final}/evidence.json`, Main 10 `--nocapture` test output, `tmp/color-timing/verified-browser.png`, and baseline rerun `tmp/color-native/23524/evidence.json`. After omitting the full trace from compact preview status, two additional fresh M1 startups and the two FFmpeg cases passed again on the final build; live preview status retained only its existing execution/startup summary. Earlier manual `browser-worker` and `ffmpeg` attempts also passed; reusable-runner files are the acceptance records, not extra distinct export cases. Saved the final export screenshot and closed the temporary browser tab.

**Handoff / still untested:** sliders/preview and the specified timing/depth cases are verified in these environments. Optional CLAHE is the next distinct feature slice, not implemented here. Native exact selected-output/10-bit preview fidelity, physical sound/long A/V drift and visible latency, long-run/DPI/accessibility/new-platform coverage, skipped Firefox checks and the historical timeout's cause remain explicit limitations. Direct NVIDIA non-neutral effects remain unsupported; direct native codec↔wgpu surface sharing remains the larger unfinished M5 work. This is not complete M5 or unrestricted cross-platform color acceptance. No commit/push requested in this turn.

## CLAHE shared-engine foundation — 2026-10-09

**Implemented scope:** validated off/default-50 or enabled/0–100 Strength policy in core, a four-slot finite timestamp-history plan, and GPU-resident histogram/cut/mapping/application in media-gpu. Existing application jobs, preview services and UI do **not** enable this engine yet. No codec/backend route, dependency version, conversion readback, hardware fallback or source/preview behavior changed. Added only test dependencies using already-pinned serde_json/pollster; lockfile package edges updated. This is implementation progress, not finished CLAHE/M5.

**Verified environment:** Windows, Rust 1.98.1 GNU, edition 2024, wgpu 30.0.1; NVIDIA GeForce RTX 3050 Laptop GPU (vendor 0x10de, device 9634), Vulkan, driver info 616.64; and Intel UHD Graphics (vendor 0x8086, device 39528), DX12, driver 31.0.101.5333. Ran both explicitly ignored real-adapter diagnostics with `cargo test --locked -p media-gpu actual_gpu_clahe -- --ignored --nocapture --test-threads=1`; shader parsing/pipeline creation and actual compute execution passed on both adapters, with the same numerical results below. Readbacks belong only to those diagnostics. No real browser or native converter window was operated for CLAHE.

**Fixture/reference:** procedural 130×129 encoded float RGB, two-by-two uneven tiles; flat black/white/near-black/gray, a deterministic low-contrast colored/noisy raster, a +0.4 scene cut, a fixed gray probe patch with histogram variation elsewhere, and sixteen 1/1023-spaced gray levels. Source generators, exact timestamp grids and independent clipping/CDF/interpolation oracle are checked into `crates/media-gpu/src/equalization_tests.rs`; no external video needed. Fixture provenance/license is documented in `fixtures/README.md`. The oracle is test-only, not a production CPU fallback.

| Actual engine diagnostic | Result |
|---|---|
| First-frame full-raster float reference | Maximum absolute channel error 0.00048828125 |
| VFR four-slot ring wraps, nine frames (0, 9, 33, 54, 67, 80, 94, 110, 140 ms) | Maximum error 0.00048828125 vs independently weighted raw mappings |
| Strength change at the same source PTS | Reused statistics; 50% result passed reference |
| CLAHE followed by the existing manual-slider shader, combined/extreme settings | Maximum error 0.0019602776, below unchanged 2/255 pre-encoder gate |
| Flat identity | Black/white/gray/near-black within 0.0005 float tolerance |
| Scene cut vs explicit fresh history; repeated reset/run | Exact output equality |
| Synthetic mapping flicker | Fixed 100-pixel gray patch; sixteen 17 ms frames, first four excluded; mean absolute temporal delta 0.0017223033 stabilized vs 0.014807359 isolated, ratio 0.11631401 (~88.4% reduction) |
| Sub-8-bit spacing | All sixteen 1/1023-spaced input levels remained distinct at 1% Strength |
| Off/zero Strength | Returned bypass, no processing passes/output generated |
| Scratch/generation guards | Wrong generation, overflow geometry and 8192² over-budget allocation rejected |

Core tests cover disabled-strength validation, serde invalid/noninteger/negative values and round trip, VFR/duplicate timestamps, ring wrap, backward seek, a 100 ms gap, negative timestamp rejection and maximum i64 timestamp. Engine allocation for this fixture was **281,632 bytes**: fixed four-slot histograms/mappings, four epochs and one RGBA16Float output. Four compute passes per new source frame, application only for duplicate PTS. This count excludes future upstream geometry and downstream slider passes. Conversion fast-path CPU readbacks added: **zero**; this engine is not yet in conversion. Half-float output/sub-8-bit spacing does not establish full native Main 10 or integer RGB16 precision.

**Build checks:** 25 core tests passed; both GPU diagnostics are intentionally ignored in the ordinary suite and passed when explicitly selected. Locked host core/GPU all-target and wasm32 core/GPU/media-web Clippy passed with warnings denied. Locked host core/GPU/media-native/converter-native and wasm32 converter-web checks passed. Final formatting and diff whitespace checks passed. The synthetic flicker result is a single controlled fixture, not proof of general noise/exposure/cut robustness. Diagnostic wall time includes upload/readback and is not a whole-job throughput measurement.

**Still unimplemented/untested:** shared settings transport and job snapshot integration, real browser compute-limit negotiation and shader execution, native wgpu 26 presenter adaptation, preview source-frame preroll and same-position history parity, Enable/Strength/Reset/Before controls, route capability UI, supported conversion before FPS duplication/drop, encoder re-decode/timestamps/audio, Main 10 CLAHE, direct CPU/NVIDIA FFmpeg parity, cancellation/device-loss stress, full memory/throughput and broader noise/exposure/scene-cut fixtures. Firefox checks remain skipped by request. Existing direct-route restrictions stay unchanged; no preview-only option or silent route switch is exposed.

## CLAHE native conversion continuation — 2026-10-09

This supersedes the preceding engine-only pending statements **only for shared settings and native shared-GPU conversion**. `VideoSettings` now carries validated equalization with a legacy-compatible off/default-50 default; native immutable job snapshots also carry device generation. Experimental native CLI `--equalization-strength 0..100` is available on `wgpu` and explicitly staged `wgpu-nvidia`. Application UI/preview still leaves it off. Browser dispatch rejects active policy rather than silently discarding it from the old command protocol.

Native enabled flow is neutral resize into RGBA16Float → shared GPU CLAHE → existing manual-slider shader → existing RGBA8 codec bridge. BT.709/encoded-sRGB boundaries are active for equalization even with neutral manual sliders. All source frames enter before the downstream FFmpeg FPS filter. Off/zero keeps the previous fused render with no equalization scratch. Direct CPU/NVIDIA routes reject active equalization before inspection/output creation, no automatic fallback. Existing shared Main 10 rejection is unchanged. Added telemetry exposes policy, allocated scratch bytes and nominal four compute passes per source frame; total enabled processing also includes geometry and final slider render passes.

**Actual export verification:** `node tests/native-clahe.mjs` passed eight 160×90 H.264/AAC exports of the CC0 `m35-vfr-offset.mp4` fixture on NVIDIA RTX 3050 Laptop/Vulkan, FFmpeg 8.0.1. Manual sliders were brightness 10, contrast 80%, saturation 60%; enabled Strength 50%. Both shared software and shared NVIDIA routes passed Original (36 output frames), 15 FPS (30), and 60 FPS (119). All six enabled jobs processed **36 original source frames** and reported 144 compute passes, rather than advancing history 30/119 times. Two additional software-route exports with off/zero Strength passed; independently decoded first-frame bytes were exactly equal. Enabled first-frame bytes differed from off, proving the effect reached export, but this is **not** an absolute independent lossy pixel-parity gate.

Every exported output PTS/count was independently checked against the rational grid; largest error was **667 µs**, below the unchanged 1.1 ms fixture gate. Full independent video/audio decoding, square pixels, geometry and BT.709 limited SDR metadata passed. AAC start/duration stayed within 25 ms, and independently decoded audio was non-silent (peak 0.2309994). Native NVIDIA mode retained its explicit NVDEC/NVENC CPU-staged bridge, not new hardware-surface interoperability. Reports showed exactly **8,294,400 CPU→GPU bytes** and **2,073,600 GPU→CPU bytes** per job, based on all 36 source frames; CLAHE introduced no additional CPU pixel readback. Equalization plus geometry scratch was **312,336 bytes** per enabled job, zero for off/zero. Engine ceiling remains 128 MiB; native engine plus geometry have a separate 192 MiB ceiling, excluding existing source/export/readback textures and codec memory. 4K allocation/performance was not run.

Four CLI rejection checks passed without published/partial output: direct CPU, direct NVIDIA, invalid Strength 101 and shared Main 10. The pure direct-route rejection test uses a nonexistent input to prove rejection happens before inspection. Explicit Rust lifecycle test `cargo test --locked -p media-native clahe -- --include-ignored --nocapture --test-threads=1` passed cancellation and actual device destruction **after two source frames**, partial cleanup and a fresh 60-frame enabled retry. The test cancellation hook is compiled only for tests. Submitted-work cancellation while a map is still outstanding, long-running stress and spontaneous device loss are not proved by this injection.

Build verification: 26 core, 31 native and one UI test passed; six native runtime integration tests remained ignored in the ordinary suite, with the new CLAHE lifecycle test explicitly run separately. Locked host core/GPU/native/native-app all-target and wasm32 media-web/web-app Clippy passed with warnings denied. Formatting and whitespace checks passed. The known nonfatal GNU `.drectve` linker warnings persist in test builds. Evidence: ignored `tmp/clahe/native-17352/evidence.json` and `tmp/clahe/lifecycle-24880/` outputs. No commit/push was requested.

**Remaining:** browser compute capability/transport/conversion integration, browser/native history-aware preview and cache invalidation, compact Enable/Strength/Before/Reset controls, native wgpu 26 presenter adaptation, exact preview/output and independent lossy CLAHE pixel parity, direct FFmpeg parity, Main 10 CLAHE, large-resolution memory/throughput and broader temporal/cancellation coverage. No native window or real browser CLAHE session was tested. Firefox stays skipped by request; this is not full CLAHE or M5 completion.

## CLAHE browser wiring / shared stage — 2026-10-09

**Implemented, runtime pending:** browser job commands now append validated `enabled/strength` policy; legacy commands default off. Active conversion uses a job-local stage on the actual processing device/queue, neutral selected geometry → CLAHE → manual sliders → existing ordered canvas capture. `EqualizationStage` in media-gpu now owns geometry/engine scratch for both native and browser, replacing the native-only geometry implementation. Standard compute/storage limits are requested if the selected adapter supports the complete set, otherwise the old graphics-only limits remain; active CLAHE then rejects insufficient limits, with no second device/CPU fallback. Added per-job scratch and nominal recorded-frame/pass telemetry. No production pixel readback was added. Existing frame/import guards, four-slot input pool and final drain remain authoritative; job-local equalization is dropped after drain even on errors/cancellation.

Code inspection establishes that the existing browser scheduler calls `processFrame` for every original source sample before FPS lookahead/resampling, including dropped-source cases; duplicated output consumes the one prepared pending frame. The scheduler was not rewritten. This is **not** runtime proof of timestamp/history or encoder correctness on this changed browser build. Active preview requests reject explicitly instead of presenting an isolated frame as history-equivalent. Application controls still leave equalization off. Browser WebCodecs and FFmpeg WASM share this Rust processing boundary, but neither changed route has a real-browser CLAHE acceptance result yet.

**Verified regressions:** 27 core tests passed, including validated command round-trip/invalid values. Locked host core/GPU/native/native-app all-target and wasm32 media-web/web-app Clippy passed with warnings denied. Host/wasm checks passed. The two actual GPU engine diagnostics again passed on NVIDIA/Vulkan and Intel/DX12 with unchanged prior numerical/flicker results. The shared-stage refactor passed all eight native CLAHE/off/zero exports again (`tmp/clahe/native-16972/evidence.json`): full timeline/video/audio/copy/scratch checks unchanged. Native actual two-frame cancellation/device destruction/cleanup/fresh retry passed (`tmp/clahe/lifecycle-14764/`). Seven worker-transport tests passed; these exercise the JavaScript transport, not real codecs or the Rust browser parser/pipeline. Formatting and diff whitespace checks passed.

Full locked wasm build (`cargo build --locked -p converter-web --target wasm32-unknown-unknown`) also passed, beyond type checking. This does not establish browser shader/codec execution or asset-bundle runtime acceptance.

### Real-browser CLAHE verification — 2026-10-09 (supersedes the tooling block below)

Paused-preview implementation update (subsequent slice, 2026-10-09): see the experimental preview record below; it supersedes the preceding preview-unimplemented statements only for that explicitly limited subset.

After the user's runtime recovery, the supported in-app Browser connected and operated the Dioxus-served changed application on localhost:8084. No installation repair or standalone browser automation was performed here. Tested environment: Windows x64; in-app Chromium user agent Chrome/155.0.0.0; actual processing adapter `intel / gen-12lp (BrowserWebGpu)`. Hardware codec execution and hidden copies remain unknown.

New `tests/browser-clahe.mjs` accepts the supported Browser's tab-scoped CDP, navigation/click and file-chooser callbacks. It resolves already loaded wasm/worker-host modules and sends experimental policies through the real serialized worker/main transport, without a second wasm/device or JS CLAHE processor. This is a developer conversion test, not an end-user UI test. Temporary diagnostic globals/query changes were confined to the test tab; production controls still leave CLAHE off.

Fixture: checked-in CC0 `m35-vfr-offset.mp4`, 36 VFR/non-zero-origin H.264 frames with AAC. Output: 50% (160×90), 8 Mbps VBR, brightness 10 / contrast 80% / saturation 60%. Worker and main-thread each ran MP4/H.264/AAC and WebM/VP8/Opus, with six exports per combination: off, enabled Strength 0, enabled Strength 50 Original, enabled 15 FPS, enabled 60 FPS, fresh-job repeat Original. **All 24 independently inspected exports passed.**

| Output policy | Frames | Largest independent PTS error |
|---|---:|---:|
| MP4 Original | 36 | 4 µs |
| MP4 15 / 60 FPS | 30 / 119 | 0.334 µs |
| WebM Original / fixed FPS | 36 / 30 / 119 | 333.334 µs |

The independent exact-ticks/rational-grid oracle includes browser AAC priming, not the native stream-start assumption. Every job processed all 36 source frames before resampling; enabled jobs recorded 144 nominal compute passes and allocated **312,336 bytes** geometry/engine scratch. Off/zero reported no CLAHE scratch/passes. Exact output dimensions, all PTS, browser full timeline/duration re-decode and independent FFmpeg full audio/video decode passed. AAC: 95 packets / 2.026667 s audio; Opus: 102 packets / about 2.040 s coverage. Independent decoded audio peaks: 0.22864 AAC / 0.23111 Opus. Physical A/V listening/sync was not tested. Existing Opus/FFmpeg header caveats remain.

All-frame independently decoded RGB bytes matched exactly for off versus zero and enabled versus fresh-job repeat, within each context/profile. Enabled differed from disabled. This proves actual effect delivery, bypass and scoped job-history determinism, **not** an independent browser pre-encoder CLAHE/reference error bound, exact preview parity or perceptual quality.

**12 failure/retry scenarios passed:** active cancellation after processed-frame progress, injected codec failure after five GPU-processed frames, and controlled device destruction after four completed frames, in each profile/context. Errors reported zero application-held frames/samples; every same-page retry passed full browser re-decode, all 36 frames, enabled CLAHE and zero live GPU leases. Device-loss retries used generation 2. No automatic rerun, spontaneous driver loss or permanent GPU stall was tested. An initial test assertion expected `lost` instead of the actual injected `loss` message; correcting that assertion required no production change, and the full matrix was rerun successfully.

All successful jobs reported zero application-held frames/samples, four peak/zero live input leases, 36 ingress copies/captures, no bitmap conversions and **zero conversion pixel readbacks**. Exact capture was 160×90 on 160×96 backing. Final short conversion/finalization samples: MP4 194–410 ms; WebM 172–346 ms. These are not controlled throughput comparisons, pure GPU time or process-memory bounds. Runner pixel decoding is diagnostic-only. M1's 30-frame round trip also passed in both contexts after compute-limit negotiation changed.

Ignored evidence: `tmp/clahe/browser-recovery-pass-20261009/evidence.json` and `tmp/clahe/browser-webm-20261009/evidence.json`, plus the actual exported files. The checked-in runner preserves metadata, complete PTS errors, telemetry, environment and recovery reports. Syntax/whitespace checks passed.

**Remaining:** history-aware browser/native preview/preroll and cache invalidation, native wgpu 26 presenter adaptation, Enable/Strength/Before/Reset controls and route explanations, independent browser pre-encoder CLAHE pixel oracle/preview-output parity, FFmpeg WASM CLAHE runtime (this server session lacks shared-memory isolation), direct FFmpeg parity/Main 10, large-output/long-run memory and broader temporal/platform coverage. Firefox stays skipped. This is not full CLAHE or M5 acceptance.

**Browser validation unavailable:** the supported Browser skill was read and its connection attempted through the provided tool. Initialization failed with `failed to write kernel assets: The system cannot find the path specified. (os error 3)` before browser selection or app interaction. This is a tooling failure, not an observed CLAHE/browser pipeline failure. No browser was operated, no workaround/repair was automatically applied, and no standalone automation substituted for the supported runtime. Real worker/main startup (including off/M1 regression after the limits-request change), actual browser compute/shader execution, re-decoded CLAHE output pixels/timestamps/audio, FFmpeg WASM, cancellation/device-loss and large-output memory/performance remain **untested**. Firefox stays skipped by request. Native history-aware preview and browser preview/UI remain unimplemented; feature/M5 acceptance is still open.

## Experimental browser CLAHE paused preroll — 2026-10-09

Implemented one bounded paused-preview slice; no UI controls enabled. Active preview carries the complete shared settings, runs sequential decoded predecessors from the core 100,000 µs history window through the existing geometry → CLAHE → slider stage, then renders the one cached original current frame. Shader/math is unchanged. JavaScript selects/owns decoded samples only; there is no CPU equalization, raw-frame history spool or additional processing device. The loop closes each sample/frame/output on normal iteration, early current/future-frame stop and GPU error, and disposes decoder input. Existing GPU submission guards may retain up to four imported lifetimes until completion; decoder internals are separately bounded by the existing library, not the one-frame JS loop. A 64-predecessor ceiling fails explicitly instead of dropping frames. Existing final GPU drain retires the stage before another preview/conversion command.

Current gates are intentionally restrictive: paused only, selected output must fit 640×360 **without additional preview downscaling**, off/zero keeps the old path. Playback reports `pause first`; larger selected geometry reports the exact unsupported reason rather than equalizing a lower-resolution preview and claiming matching statistics. Each request rebuilds the temporal maps; current original pixels are reused on same-position parameter changes, but predecessor decode/maps are not yet cached across Strength/slider changes. Source/geometry/history cannot leak across requests because stage ownership ends at each drained command. This is a foundation for cache integration, not the final cache/preview policy.

Real tests: supported in-app Chromium 155 on Windows/Intel gen-12lp, Dioxus build/serve, experimental commands through the actual worker/main transport. `tests/browser-clahe-preview.mjs` exercised the checked-in 36-frame VFR/offset fixture, 50% output 160×90, Strength 50 and combined 10/80/60 sliders, positions 0 → 0.4 → 0.8 → 0.4 → repeat 0.4 s. **Ten paused requests passed** across both contexts; all images were nonblank/exact-sized and both seek-back/repeat images matched their earlier decoded preview bytes exactly. The 0.4 s case reported one predecessor inside the finite window. Active playback rejection and subsequent paused retry passed on both. Each same-page preview→enabled conversion handoff then re-decoded all 36 frames with complete browser PTS/audio checks, zero application-held frames/samples, zero live leases and 36 source frames/144 nominal compute passes, proving no retained preview history polluted the new conversion job. No independent preview/output pixel-error gate is claimed.

Developer diagnostics captured two explicit preview readbacks in the main context (ingress/output); worker snapshots used test-only canvas frame readback. They are not conversion readbacks. Production preview has no newly added pixel readback. Evidence: ignored `tmp/clahe/preview-handoff-main-20261009/evidence.json` and `tmp/clahe/preview-handoff-worker-20261009/evidence.json`, including hashes, preview summaries and handoff reports. An initial harness raced the ordinary UI's trailing source-preview update and captured its 320×180 baseline instead of experimental 160×90; waiting for the UI preview/cache to settle corrected the harness, with no processor tolerance change.

Checks: wasm media-web/web-app check and Clippy with warnings denied passed, actual Dioxus browser build passed, formatting/whitespace passed. Three tests of the actual extracted preroll function verify VFR/window/current-frame boundaries, GPU-error cleanup and the 64-frame ceiling with closed instrumented owners. Together with seven transport tests, ten JS tests passed. These owner mocks do not replace the actual browser results above.

Still required before exposing controls at this preroll-only point: independent same-position pre-encoder/conversion parity, exact large-selected-output processing plus separate display scaling, cached mapping/decoder reuse for parameters/Before, cancellation/stale-command/device-loss preview stress, playback history, native wgpu 26 presenter/preroll adaptation, compact controls/route explanations and FFmpeg WASM CLAHE coverage. Firefox remains skipped; Main 10/direct FFmpeg parity, broad temporal/high-resolution/performance/platform work remains unverified. Full CLAHE/M5 acceptance is not claimed.

### Paused-preview statistics cache continuation — 2026-10-09

This supersedes the preceding per-request-history-rebuild statements. Successful paused requests now retain one bounded GPU stage after draining submitted work. Same-source/position/geometry Strength and slider changes reuse statistics; temporary off/zero Before bypass retains but does not advance the maps. Source generation/file size/modification identity, requested position and selected geometry form the key. Geometry reconfiguration, clear-preview/error, conversion/M1 start and device-session replacement drop the stage. No history is shared between devices or converted jobs, no predecessor image spool is retained, and no production pixel readback was added. Cached preview scratch may remain during bypass; off/zero conversion still has no CLAHE allocation. These are ownership/code-path properties, not device-loss runtime evidence.

**Verified:** supported in-app Chromium 155, Windows x64, Intel gen-12lp BrowserWebGpu; same CC0 VFR fixture, 160×90 selected output, Strength 50 and combined 10/80/60 sliders. The strengthened `tests/browser-clahe-preview.mjs` passed **12 recorded cases in each of main and worker contexts**: five initial position/repeat checks; four same-position Strength/slider/bypass/restoration checks; two geometry changes; explicit clear/rebuild. Both cache-load and preroll-input-open counters stayed unchanged for parameter/bypass changes. Restoring Strength/sliders after the temporary Before bypass matched the original preview SHA-256 exactly. Seek-back/repeat identity, geometry invalidation and clear invalidation passed. Active playback rejection followed by a paused retry also passed. Each context then exported/re-decoded all 36 source frames, reported 144 nominal compute passes and zero application-held frames/samples/live leases, confirming an independent conversion history after preview. Counter/hash checks establish this scoped cache reuse and determinism, not an independent pixel oracle or a performance speedup.

Diagnostic main-context ingress/output readbacks and worker canvas snapshots remained test-only. Evidence: ignored `tmp/clahe/cache-pixels-main-20261009/evidence.json` and `tmp/clahe/cache-pixels-worker-20261009/evidence.json`. Locked wasm media-web/web-app Clippy with warnings denied, actual Dioxus build, formatting and whitespace checks passed; all ten preroll/transport JavaScript tests passed. Host shared-core/GPU code was unchanged in this cache slice; previous host results were not rerun here.

**Still untested/unimplemented:** preview source replacement/device-generation runtime stress, rapid stale-command/cancel/device-loss interactions, independent preview/export numerical parity, exact large-output statistics with separate display scaling, playback history, native presenter/preroll, UI Enable/Strength/Before controls and FFmpeg WASM CLAHE. The paused-only/exact-640×360 gate is unchanged. Firefox remains skipped by request. Full CLAHE/M5 acceptance is still open.

### Browser CLAHE preview/pre-encoder parity — 2026-10-09

Subsequent update: the diagnostic capture ceiling is now 1920×1080 (8,294,400 raw RGBA bytes per frame, still at most eight selected frames), solely to test full-selected-output display scaling. Normal conversion/preview still performs none of these diagnostic captures. See the large-selected-output record below; it supersedes only the earlier small selected-geometry restriction, not playback/native/feature acceptance.

**Implemented diagnostic boundary:** `?diagnostic-parity=0,7,14,24,35` requests selected original source-frame indices, capped at eight and output ≤640×360. Conversion captures the already GPU-processed owned frame before FPS lookahead/resampling/encoding; preview captures its owned output. Both attach decoded source PTS and exact geometry. RGBA buffers are bounded to 921,600 bytes each, converted to transportable base64 and never used for production processing. Normal jobs return no parity snapshots and perform none of these readbacks. The test-only readback count is separate from the existing production-transfer telemetry; its awaits are included in job time, explicitly not throughput evidence. This adds no CPU equalization, raw-frame history spool or second device.

**Verified:** `tests/browser-clahe-parity.mjs` passed on supported in-app Chromium 155 / Windows x64 / Intel gen-12lp BrowserWebGpu, worker and main-thread execution. Fixture: checked-in CC0 `m35-vfr-offset.mp4`, 36 H.264 VFR frames with non-zero origin/AAC, 320×180 → 160×90, MP4/H.264/AAC, 8 Mbps VBR and no-preference acceleration (actual hardware execution unknown). Each context ran nine jobs: Strength 25 with neutral sliders, Strength 50 with 10/80/60 sliders, Strength 80 with −10/120/140 sliders; each at Original/15/60 FPS. Five selected source indices (0,7,14,24,35), including beginning/middle/end, were independently requested as paused previews; exact decoded source PTS was asserted before comparing all RGBA8 channels. **All 90 preview versus pre-encoder comparisons had maximum/MAE/p95 error 0**, below the unchanged ≤2-channel-value gate. This is actual codec-decoded source/GPU path consistency, not an independent implementation of the histogram algorithm or a comparison against lossy decoded export pixels.

All eighteen outputs passed full browser video/audio re-decode and complete timestamp/duration checks: 36 Original / 30 at 15 FPS / 119 at 60 FPS frames. Every conversion processed all 36 source frames, recorded 144 nominal compute passes, allocated 312,336 CLAHE/geometry scratch bytes and reported zero application-held frames/samples/live input leases. Existing ingress/canvas captures were 36 per job with four peak leases; five pre-encoder diagnostic readbacks were reported separately. Hardware codec execution, internal copies and physical A/V listening remain unknown/unperformed. No throughput or broader memory bound is inferred from this short diagnostic run.

Both ordinary cache regression runners also passed twelve cases and preview→conversion handoff again with parity capture absent, keeping normal WebCodecs conversion pixel readbacks at zero. Fourteen JavaScript tests passed, including opt-in transport, selection/count/geometry/storage bounds, borrowed-frame ownership, actual preroll boundaries/cleanup and transport recovery. Locked wasm media-web/web-app Clippy with warnings denied, formatting, syntax and whitespace checks passed; actual Dioxus rebuild passed. No shared core/GPU/native algorithm changed, and host suites were not rerun in this diagnostic-only slice.

Evidence: ignored `tmp/clahe/parity-main-20261009/evidence.json`, `tmp/clahe/parity-worker-20261009/evidence.json` (environment, per-frame errors/PTS, summaries) and `tmp/clahe/parity-cache-regression-{main,worker}-20261009/evidence.json`. Test raster bytes are not checked into Git. **Remaining:** independent browser CLAHE numeric oracle, exact large-output/statistics plus separate preview scaling, playback/native presenter/history, controlled preview source/cancel/device/stale-command stress, scene-cut/broad temporal/high-resolution fixtures, controls/route explanations and FFmpeg WASM CLAHE. Firefox stays skipped. Full CLAHE/M5 acceptance is not claimed.

### Full-selected-resolution browser CLAHE preview — 2026-10-09

Subsequent lifecycle update: cancellation and scoped identity/parameter checks now pass as recorded below; device-loss/UI-revision stress and native preview remain pending.

**Implemented:** selected geometry and CLAHE history are no longer limited to the display raster. For a larger selected output, the shared GPU processor runs geometry → CLAHE → manual sliders/clipping at that resolution into RGBA8, then applies a separate neutral bilinear display resize to ≤640×360. Same queue/device/shader contract; no CPU processing/readback added to normal preview. One selected-resolution adjusted texture is retained with the bounded mapping cache. Its bytes plus engine/geometry scratch must fit the checked 192 MiB ceiling and device texture limits. Bypass retains the maps but applies selected geometry before display scaling; conversion drops preview state and does not allocate the new texture/pass. Playback remains explicitly rejected. No UI controls enabled.

**Verified:** supported in-app Chromium 155 / Windows / Intel gen-12lp; worker and main contexts. Deterministic temporary HD fixture is derived from checked-in CC0 `m35-vfr-offset.mp4`, not user media: `ffmpeg -v error -y -copyts -i fixtures/m35-vfr-offset.mp4 -vf scale=1920:1080 -c:v libx264 -preset ultrafast -crf 18 -c:a copy -fps_mode passthrough -avoid_negative_ts disabled tmp/clahe/large-fixture/m35-vfr-hd.mp4`. FFmpeg 8.0.1; 1,809,476 bytes; SHA-256 `7407b0a991e9db1501fabcc88719cd3a606e02441686aa48ee5d889ef74c2695`. Upscaled synthetic detail is not real-camera quality evidence. Application implicit-upscale rejection remains intact; the first attempted use of the original 320×180 fixture correctly rejected 720p, so the derived fixture was used instead. No large fixture is added to Git.

Four actual MP4/H.264/AAC exports at 8 Mbps VBR / Original FPS covered selected 1280×720 and 1920×1080 in each context, Strength 80 with brightness −10/contrast 120%/saturation 140%. Each output passed full video/audio re-decode, the complete 36-frame timeline, non-silent audio and zero application-held frames/samples/live leases; all 36 source frames advanced CLAHE. Two positions per job (indices 0 and 14) had exact source-PTS comparison. An independent CPU bilinear display oracle consumed actual full-resolution pre-encoder RGB, with **no further sliders**; all eight 640×360 previews passed. 720p: maximum channel error **1**, MAE ≤0.02520, p95 **0**; 1080p: maximum/MAE/p95 **0**. Eight temporary Before → saved-settings restorations matched exact preview bytes and reported history reuse. This tests output-stage/display ordering and history consistency, not an independent CLAHE histogram oracle or lossy export-pixel parity. 1080p conversion reported 35,291,152 engine/geometry scratch bytes; its preview adds 8,294,400 RGBA8 bytes, within budget.

Main/worker small-preview cache regressions each passed twelve cases plus full conversion handoff again. Fourteen JS tests, 27 core tests, locked host core/GPU all-target Clippy, wasm media-web/web-app Clippy (warnings denied), native library/app check, actual Dioxus build, formatting and whitespace checks passed. Two existing host GPU diagnostics remained ignored in the ordinary suite; the changed display stage was exercised in the real browser, not claimed as a native runtime test. A rebuild-time file-chooser race and the browser command's short default diagnostic timeout affected initial harness runs; waiting for app mount and allowing a bounded 60-second diagnostic command corrected the runner, with no shader tolerance change.

Evidence: ignored `tmp/clahe/scale-{main,worker}-{720,1080}-20261009/evidence.json` and `tmp/clahe/scale-cache-regression-{main,worker}-20261009/evidence.json`. Diagnostic raster capture is opt-in and separately counted; test-only timing is not throughput evidence. **Still unverified/unimplemented:** 4K/long-run/high-memory behavior, orientation/odd-size large-output coverage, actual selected-resolution native presenter, playback history, controlled preview cancellation/stale/source/device stress, independent browser histogram oracle, CLAHE controls/unsupported-route UI, FFmpeg WASM CLAHE and direct/Main 10 parity. Firefox remains skipped. Full CLAHE/M5 completion is not claimed.

### Paused CLAHE preview cancellation and scoped lifecycle — 2026-10-09

**Fixed:** preview callbacks previously did not check execution cancellation between GPU frames. They now capture the cancellation generation, reject before submission or after producing a frame (closing that output), and check again after the existing final drain. Error/cancel clears the cached source and equalization stage only after submitted work retires. The JavaScript preroll/render finally paths still own sample/frame cleanup; no new CPU transfer or processing device. A first-GPU-frame-prepared progress message is emitted once per preview command, not once per pixel/frame history record. Conversion's existing callback/cancellation behavior is unchanged.

**Verified:** strengthened `tests/browser-clahe-preview.mjs` with `lifecycle:true`, supported in-app Chromium 155 / Windows / Intel gen-12lp, worker and main, CC0 VFR fixture at 160×90 with Strength 50 and 10/80/60 sliders. Both retained the earlier twelve cache/seek/geometry cases. File-identity replacement used a new File with identical fixture bytes but different name/modification time, retaining the same requested source key: history rebuilt; decoded preview hash remained identical as expected. Returning to the original identity rebuilt again. Six concurrently queued Strength edits serialized successfully, reused maps and ended with pixels exactly matching the final requested settings. This is command-boundary evidence, not actual UI coalescer/latest-revision publication or different-content replacement evidence.

In each context, a cancellation was triggered **after the first real GPU-processed preview frame was prepared**. Active preview rejected `CANCELLED: preview stopped and drained`; the queued stale preview rejected before startup. The next preview rebuilt source/history and matched the original pixel hash exactly. Each subsequent enabled conversion fully re-decoded all 36 frames with audio/timeline checks, 144 nominal compute passes, zero application-held frames/samples and zero live GPU leases. Diagnostic preview snapshots remained test-only; normal WebCodecs conversion pixel readbacks remained zero. This covers already-submitted work at an explicit frame boundary, not a permanently stalled queue or spontaneous device loss.

Locked wasm media-web/web-app Clippy with warnings denied, actual Dioxus rebuild, formatting, JS syntax, fourteen preroll/diagnostic/transport tests and whitespace checks passed. No host algorithm changed in this slice; host suites were not rerun. Evidence: ignored `tmp/clahe/preview-lifecycle-{main,worker}-20261009/evidence.json`. An initial test captured its `prepared` flag before awaiting completion; moving that observation after the awaited result fixed the harness, not production cancellation.

**Remaining:** controlled preview device loss/retry, different-content/source/seek UI revision races, large-output cancellation stress, 4K/long runs, playback/native CLAHE preview, independent browser histogram oracle, compact controls/route explanations and unsupported-route coverage. Firefox remains skipped. No full CLAHE/M5 acceptance is claimed.

### Browser CLAHE preview controlled device-loss recovery — 2026-10-09

**Implemented:** the actual processing wgpu 30.0.1 device registers its released `set_device_lost_callback`; the callback sets a safe atomic flag, never wraps non-Send browser resources in unsafe Send/Sync. A new command excludes flagged sessions, requesting a fresh device/generation on the same canvas only for the explicitly requested retry. Failed preview drains then closes source/statistics and retires a lost session. No automatic rerun or CPU/route fallback. Existing opt-in `?failure=device-loss-once` now destroys the actual device before the second GPU callback of active CLAHE preview, after one frame was prepared; off/neutral source-selection previews do not consume the injection. Conversion's existing four-frame injection remains unchanged. Prepared is not a claim that all first-frame GPU work finished before destruction; existing submitted-work guards remain authoritative.

**Verified:** supported in-app Chromium 155 / Windows / Intel gen-12lp, main and worker; same CC0 VFR fixture, exact 160×90 paused output, Strength 50 with 10/80/60 sliders. Both failed at the injected device-loss boundary. Same-page retries rebuilt history on **device generation 2** and exactly matched the earlier preview pixel hash. Subsequent MP4/H.264/AAC conversion processed/re-decoded all 36 frames with full timestamp/audio checks, zero application-held frames/samples/live leases, 144 nominal compute passes and no production pixel readbacks. Previous twelve cache cases, file-identity invalidation, six rapid serialized edits and cancel/stale/retry cases also passed again in both contexts. Diagnostic preview snapshots remain test-only.

Locked wasm media-web/web-app Clippy with warnings denied, actual Dioxus rebuild, formatting/JS syntax/whitespace checks and fourteen JS diagnostic/preroll/transport tests passed. No shared/native algorithm changed and host suites were not rerun. Evidence: ignored `tmp/clahe/preview-loss-{main,worker}-20261009/evidence.json`, with the injected failure, retry generation/summary and subsequent conversion report. Device-loss callback API was verified against the installed wgpu 30.0.1 source and its browser backend; no development-only API assumed.

**Still untested/unimplemented:** spontaneous driver/process loss, permanent GPU stalls, large-output cancellation/device-loss and long-run memory stress, different-content source/seek/UI revision races, native/playback CLAHE preview, independent browser histogram oracle, compact controls/route explanations and FFmpeg WASM/direct/Main 10 parity. Firefox remains skipped. No full CLAHE/M5 completion is claimed.

### Native paused CLAHE worker/presenter handoff — 2026-10-09

**Implemented:** experimental service policy reaches the native Presenter, behind default None with no visible CLAHE controls. One lazy persistent worker per renderer holds one latest metadata request, one completed texture and one original/finite-statistics cache. Source/seek/selected-geometry changes cancel old preparation; parameter-only edits coalesce and reuse maps. File length/modification identity is checked before cache reuse and after preparation. Ticket checks suppress stale results. Weak controls invalidate pending/active/completed requests and wake off-thread cache cleanup on Close/source/seek. Renderer suspension cancels/joins the worker; resume increments generation and creates a fresh one lazily. CPU decode/upload and completion polling remain off paint callbacks. A completed worker image is GPU-copied, centered, into one registered 640×360 display texture with opaque black display-only bars; old registry handles are explicitly unregistered on replacement. No new processing device, CPU equalization/readback, raw history array or conversion copy. GPU work retains its recorded resources through completion; worker results are not mutated while handed off.

**Verified:** real Windows Intel UHD Graphics / Vulkan, FFmpeg 8.0.1. `cargo test --locked -p converter-native latest_request_cache_invalidation_and_shutdown -- --include-ignored --nocapture --test-threads=1` passes: 500000 µs VFR preroll uploads exactly three frames; 30 rapid Strength requests coalesce to the final saved setting and restore identical decoded pixels without new uploads; off/Before and restoration preserve identity; invalidated seek output is absent and explicit retry matches; tagged HDR fails and returning to supported source succeeds; dropping with queued work cancels/drains/joins. A 160×80 selected geometry produces a 640×320 display image; the presenter's actual GPU copy helper preserves every image byte centered in 640×360, with exact opaque-black top/bottom 20-pixel bars. All these readbacks are diagnostic only. This exercises worker and real GPU copy code, **not a Blitz window or texture registry runtime**.

Service regression verifies paused-only playback rejection, idempotent policy updates, exact requested seek metadata, Close invalidation and return to normal playback. Ordinary native-app suite: **14 passed, 3 explicit GPU tests ignored**; the new worker GPU test is separately selected above. The existing same-source seek test deliberately held the frame mutex while calling start; the new metadata access exposed that test-only reentrancy deadlock. A separate test-only publication gate now blocks decoded publication without holding the service's data mutex, retaining the deterministic old-frame/timeline assertion. No production mutex bypass or weakened assertion was introduced. Formatting/whitespace and locked native-app all-target Clippy with warnings denied pass.

**Pending:** actual live CLAHE activation/presentation and registry cleanup, UI seek/settings/source races, multi-window/DPI/suspend/resume, actual renderer device destruction/recovery, memory/long-run stress and controls/conversion snapshot wiring. No native GUI/Computer Use/browser runtime was tested in this slice; browser/shared shader code is unchanged. Close invalidation does not wait for GPU work on the UI thread, but suspension/drop joins and may wait for the adapter's 30-second completion deadline on a stalled device; permanently stalled driver behavior remains untested. Default None leaves ordinary playback unchanged. CLAHE/M5 acceptance is not complete.

### Native paused CLAHE GPU adapter and cross-ABI pixel parity (earlier evidence) — 2026-10-09

**Implemented:** native HistoryPass prepares the exact preroll on a caller-supplied renderer-wgpu-26 device/queue using shared EqualizationStage/PreviewColorStage. One source texture is overwritten in queue order after each submission drains; only the selected original plus finite GPU statistics remains. Same-position Strength/sliders reuse mappings; temporary off/zero bypass preserves them. Manual sliders/clipping precede independent display resizing, all on that device. No CPU processing/readback is added to the adapter. Source limits/no-upscale, combined scratch budget, bounded display format/size and generation checks precede submission. Atomic preparation drains before success/error publication; GPU completion has a polled 30-second deadline. The implementation is callable/tested but **not connected to the live player**: blocking preparation belongs in its worker, never inside a paint callback. Existing player and controls remain unchanged.

**Verified:** actual Windows Intel UHD Graphics / Vulkan (vendor 32902/device 39528), FFmpeg 8.0.1, Rust 1.98.1 GNU. `cargo test --locked -p converter-native exact_preroll_renderer_matches_full_conversion_and_reuses_maps -- --include-ignored --nocapture --test-threads=1` passed. Independently full-decoded/probed CC0 VFR/non-zero-origin fixture: all 36 frames chronologically processed through the processing-wgpu-30 shared conversion composition (EqualizationStage plus manual shader), with test-only pre-encoder captures at indices 0/7/14/24/35. Native renderer-wgpu-26 prepared each position from bounded preroll and compared selected 160×90 and 320×180 outputs with Strength 25/neutral, 50/brightness +10/contrast 80%/saturation 60%, and 80/brightness −10/contrast 120%/saturation 140%. **All 30 full-raster comparisons had maximum channel error zero**. Another 15 320×180 → 160×90 display checks used an independent CPU half-size bilinear oracle after selected-resolution clipping/sliders: maximum channel error **1**, within unchanged ≤2 gate. This is pre-encoder parity, not lossy decoded-output parity or an independent CLAHE histogram oracle.

Every settings/off/zero/restoration sequence reused the prepared source/maps with exactly identical restored bytes and no extra upload: **1–3 uploaded frames per prepared position** (0–2 predecessors). Scratch: **369936 bytes** at 160×90 and **1496080 bytes** at 320×180, excluding original/display textures. Wrong generation rejects; pre-cancel rejects. Diagnostic readbacks/full raw reference buffers exist only in the explicit test, not normal preview/conversion. Test-only processing wgpu 30 was added as a dev-dependency; Cargo.lock adds its existing resolved version to the native test dependency list without external upgrades.

Ordinary native-app suite: **13 passed / 2 real-GPU tests ignored**; the new cross-ABI GPU test was separately run above. Native preview/library regression subset: **8 passed**. Locked native app all-target Clippy with warnings denied, formatting and whitespace checks passed. No browser build/runtime or native GUI/Computer Use was exercised; shared GPU source/wasm paths were not changed. **Remaining:** bounded live worker/renderer handoff and current-frame cache ownership, actual source/seek/settings revision races, suspend/resume/device loss and submitted cancellation stress, natural scene cuts/high-resolution/long-run coverage, playback integration and compact CLAHE controls. This slice does not enable CLAHE UI or complete M5.

### Native exact timestamped preview ingress (earlier evidence) — 2026-10-09

**Implemented:** separate `media-native::preview::stream_history`, not yet mounted in the player. A borrowed callback consumes one reused source-resolution encoded-sRGB RGBA8 buffer, exact decoded ticks/time base and video-relative microseconds. No letterbox bars, CPU equalization or full-clip timeline/pixel history. The requested preceding 100 ms streams before the first frame at/after the requested position; at most 64 predecessors are delivered. FFmpeg `showinfo` observes the same filtered frames as the raw pipe; ordinal, integer PTS, time base, dimensions and RGBA format are checked. The stderr observer never blocks, metadata queue capacity is 66, lines are capped at 4 KiB and a frame at 64 MiB. Input seeking is container-relative, followed by exact video-tick trim to handle audio/video origin differences. Unsupported SAR/display rotation rejects explicitly. Callback errors/cancel/early selection drop the owned decoder, killing/reaping decode-ahead through the existing ChildGuard.

**Verified:** Windows, FFmpeg 8.0.1, Rust 1.98.1 GNU; no GPU is used in this ingress test. Checked-in CC0 `m35-vfr-offset.mp4` (320×180, 36 frames, video origin 1.25 s/audio 1.228 s) was independently fully decoded and probed. Requests at 0, 1, 400000, 500000, 777777 and 1900000 µs matched every streamed frame's exact integer PTS and all RGBA bytes against that full reference; selected positions match the first actual frame at/after each request. One reused buffer is 230400 bytes. Initial testing exposed an incorrect absolute input-seek assumption; the video-tick trim corrected it before these passing runs. A temporary generated CC0 gray 32×32 / 1000 fps / 200-frame H.264 fixture verified exactly 64 delivered predecessors, constant pixel-buffer address and explicit bound failure; the temporary file is removed by its test owner. Malformed PTS, changed geometry/time base, callback cancellation/error, EOF, unsupported geometry and a fresh retry pass.

`cargo test --locked -p media-native --lib preview -- --nocapture`: **8 passed**, including the four new history tests plus existing letterboxed decoder/player and CPU color parity regressions. Locked native library/app all-target Clippy with warnings denied, formatting and whitespace checks pass. No browser/wasm/shared-GPU code changed or browser/GUI session was tested. This proves a bounded decoder ingress, **not native CLAHE presentation**. Selected-geometry renderer integration, cached original/maps, native preview/export parity, stale UI/source/seek races, device-loss/long-run stress and controls remain pending. CPU preview decoding/color normalization is explicit; FFmpeg-internal memory is not bounded by the application buffer and no hardware-decode claim follows.

### Native CLAHE renderer-ABI foundation (earlier evidence) — 2026-10-09

**Implemented:** private `media-gpu-blitz` compiles the actual shared media-gpu Rust source against pinned renderer wgpu 26.0.1; its default `renderer-abi-26` feature selects only API-descriptor/limit-type differences and diagnostic instance/polling signatures. No duplicate processor/math/shader, cross-version texture handle, unsafe Send/Sync, CPU equalization or extra device is introduced. Processing wgpu 30.0.1 remains unchanged. Native ColorPass now delegates to the same shared ResizePipeline compiled for the renderer ABI, removing its duplicated pipeline setup/pass. Cargo.lock records the new local package without upgrading existing external versions. This is a used implementation/ABI boundary, not empty scaffolding; the adapter is intentionally non-publishable because its library source is shared outside its package directory.

**Actual GPU verification:** `cargo test --locked -p media-gpu-blitz -- --include-ignored --nocapture --test-threads=1` passed both independent full-raster CLAHE diagnostics on Windows: Intel UHD Graphics / DX12 / driver 31.0.101.5333 and NVIDIA RTX 3050 Laptop / Vulkan / driver 616.64. All previous gates were retained, including uneven tile dimensions, VFR/ring wraps, duplicate PTS/Strength reuse, flat/off/zero identity, cuts/reset, combined sliders, sub-8-bit spacing and generation/scratch rejection. Engine scratch **281,632 bytes**; raw/temporal float-reference max error **0.00048828125**; combined-slider max **0.0019602776**. Controlled stationary-patch mean temporal delta: stabilized **0.0017223033**, isolated **0.014807359**, ratio **0.11631401**. The processing-wgpu-30 diagnostics were explicitly rerun and passed on the same adapters with the same values. This is a synthetic engine/reference gate, not natural-video flicker or live renderer integration evidence.

The explicitly selected native `actual_blitz_abi_shader_matches_reference` test passed on Intel/Vulkan after presenter refactoring: actual FFmpeg-decoded CC0 frame, five neutral/combined/extreme slider policies, maximum channel error **0** for neutral and **1** otherwise, opaque alpha preserved (unchanged ≤2 gate). It creates a real renderer-ABI GPU device but no application window. Ordinary suites: **27 core + 13 native-app tests passed**; two engine and one presenter runtime tests remained ignored there and were separately run as above. Locked host core/GPU/renderer-adapter/native/native-app all-target Clippy and wasm media-web/web-app Clippy passed with warnings denied; formatting/whitespace passed. Known nonfatal GNU `.drectve` linker warnings persist in native test linking. No browser, native window, native CLAHE codec export or Computer Use session was tested in this slice.

**Still required for native CLAHE preview:** exact decoded source PTS and finite preceding samples, actual selected geometry rather than the existing letterboxed display pixels, renderer-device generation/history/cache ownership, same-position pre-encoder/conversion parity, Before/Strength/slider/source/seek invalidation and cancellation/device-loss cleanup. The existing preview decoder explicitly reports requested seek positions—not exact frame PTS—and that cannot be fed to temporal equalization as if validated. Native CLAHE processing/UI remains disabled until this path exists. Browser playback, controls, broad platform/high-resolution stress and unsupported direct/Main 10 parity also remain open. No full CLAHE/M5 acceptance is claimed.

## MPG input inspection blocker - 2026-10-30

The selected local MPEG-PS/MPEG-1 input (352x288, yuv420p, MP2 audio) fails
native inspection before conversion with "native M4 harness requires a known
frame count". Its container does not expose nb_frames. Direct CPU conversion
also rejects before creating output; this is not an RTX SDK failure. The source
additionally has 178:163 sample aspect ratio, outside the current square-pixel
shared/RTX route. Counting decoded frames and proving pixel-aspect normalization
with preview/export/timing/audio parity are needed before this input can be
accepted. No format/geometry gate was bypassed. The ordinary window's active AI
capability reason hides the actual inspection failure; that UI issue remains
unfixed. Evidence: evidence/ai-upscale-20261030/mpg-inspection/report.json. Source
media, filename and screenshots remain local/untracked; MPG support is not claimed.


## Native size-control presentation and AAC timestamp fault - 2026-10-30

When AI is enabled, native/web ordinary resize controls are now replaced by an
AI size explanation; inactive 75%/exact choices no longer look selected. Native
shows scale and resolved dimensions. Stored ordinary settings are untouched and
return on AI Off. The user confirmed the fix and the normal native window's AI
size label was observed. Native/browser builds, matching-target Clippy and format
checks pass; no new browser runtime acceptance is claimed.

A subsequent 1280x720/25fps source failed export verification: expected audio
start0, actual60000us. Its first AAC PTS is3600 ticks at44100Hz (81.633ms), then
falls to1024 (23.220ms); DTS starts0 and remains sequential. This is a malformed
source presentation timeline, not an RTX SDK evaluation failure. A separate,
explicit local remux sets AAC PTS=DTS without re-encoding. Packet verification
proves all3202 video and5518 audio payload SHA256s, all DTS and durations unchanged;
only the first audio PTS changes3600->0. Original file remains untouched.

That repaired derivative completes release CLI RTX2x export:3202 inferred/output
frames at2560x1440, total331312ms (5m31s, not a comparative benchmark). Independent
every-frame timestamps/endpoints, full A/V decode and non-silent audio pass. The
verified output is saved to the user's chosen local destination. No user media,
filename or screenshots are tracked. Redacted evidence:
evidence/ai-upscale-20261030/audio-timestamps/report.json.

Native inspection now checks a bounded prefix of32 AAC packet timestamps before
video scanning/GPU work. Backward/repeated PTS reject with a source-specific repair
message. Negative encoder priming and valid positive audio origins remain allowed.
This is not a full-track timestamp scan or automatic repair. Existing sync limits
are unchanged. Normal-window original-source early error and repaired-source
inspection/preview were observed. Status errors remain visible separately from AI
availability reasons. Native tests61 passed/18 ignored; native all-target Clippy
with-D warnings and formatting pass. Initial compiler file-map/executable locks
required sequential retries, which succeeded. Larger legacy-format/audio-repair
support remains a separate decision; no AI task checkbox advances (24/32).

### Native legacy inputs: AAC original and MPEG direct software, 2026-11-01

The newer native-legacy-inputs implementation supersedes the preceding AAC
prefix-only rejection. Complete bounded packet classification now admits only the
verified isolated leading fault and shares owned, packet-verified audio preparation
between inspection, playback and export. All20 synthetic fixture/route exports pass
independent decode/timing/audio checks, with valid priming, offsets and gaps retained.
Seek/start/tail samples match an equivalent audio-only reference exactly.

The ordinary native app directly exported the original private AAC input with
RTX2x/Shared GPU NVIDIA on RTX3050Laptop, Vulkan/SDK1.1:3202frames in/out,
2560x1440,70253292bytes,345639ms total, h264_nvenc. Independent complete decoding,
all video timestamps, audio alignment/endpoints and unchanged original identity pass.
Output bytes match the previously verified repaired-source export. Isolated automatic
audio preparation cost903ms/2034300bytes; final owner retirement removed scratch.
UI timing overlapped development work and is not a performance benchmark.

A new original CC0 MPEG-1/MP2 fixture proves missing-count inspection and direct
software export:50frames,384x288,2s video; complete packet evidence establishes
MP2 coverage despite inaccurate container estimates. Independent full decode,
timeline, audio coverage and sampled aspect/image checks pass unchanged tolerances.
Known limited range is preserved with FFmpeg8.0.1's experimental `write_colr` mux
option when other color tags are unknown; matrix/transfer/primaries are not invented.

Native tests71passed/20ignored and native app tests20passed/6ignored; formatting
and native all-target Clippy with-D warnings pass. MPEG shared/RTX geometry,
WMV/WMA policy, hybrid NVIDIA routing and full family/UI acceptance remain pending.
Evidence: `docs/evidence/native-legacy-inputs-20261101/{aac,mpeg}`. The separate
ai-upscale task progress remains24/32; no browser runtime claim is added.


### Native legacy inputs: ordinary-window acceptance complete (2026-11-02)

The final Computer Use retry succeeded. All eight required video families now
have ordinary native-window source inspection, visible preview and conversion
coverage through software and NVIDIA codec choices. The non-square-SAR RTX
preview UI fix was verified by seeking WMV3 and restoring its image after export.
Source Browse exposes MP4/MPG/MPEG/WMV; Save As remains MP4-only. WMV metadata
reports Windows audio decoding and bounded inferred final-frame durations.

Thirteen additional window-driven RTX2x exports passed independent complete AV
decode, every video PTS/end, PCM waveform/end, count/geometry/SAR and encoder
checks: video error <=0.908ms and audio endpoint error <=16.259ms. Together with
the earlier five exports this is18 window-driven outputs, separate from the
120-case complete backend matrix. No new perceptual-quality or zero-copy claim.

The same RTX3050LaptopGPU now has LUID0:78380 after restart. The stale saved
identity correctly rejected; a direct helper handshake established the current
identity, followed by normal UI configuration and runtime validation under the
existing local SDK evaluation approval. No automatic identity refresh or consent
policy change is implemented. That setup usability limitation remains separate.

Native-legacy-inputs is25/25 complete; ai-upscale remains24/32 with its task file
unchanged. Full evidence, profile/fixture limits and reproducibility:
`docs/evidence/native-legacy-inputs-20261101/window-acceptance.md`. No automatic
archive, spec synchronization, commit or push was performed.
