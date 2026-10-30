# Optional native RTX helper

Source-only Windows D3D11 / RTX Video SDK 1.1 adapter. Build explicitly with the
separately installed and licensed SDK from an x64 MSVC developer shell:

```powershell
./tools/ai-upscale/native-rtx-helper/build.ps1 -SdkRoot D:/Libraries/RTX_Video_SDK_v1.1.0 -Output <new-helper.exe>
./tools/ai-upscale/native-rtx-helper/build.ps1 -SdkRoot D:/Libraries/RTX_Video_SDK_v1.1.0 -Output <separate-directory/new-reference.exe> -SampleReference
```

Create output directories first. Builds refuse existing executables/objects.
The reference build compiles the installed, unmodified
`samples/RTX_Video_API/rtx_video_api_dx11_impl.cpp`; no NVIDIA source or binary is
vendored. For local evaluation, the sample executable needs the installed SDK's
`nvngx_vsr.dll` and `nvngx_truehdr.dll` beside it because the sample uses default
feature discovery. Local hard links avoid duplicate storage. Keep these files
under ignored evaluation storage, separate from the helper executable, and never
modify linked files. This is not a redistribution recipe or license grant.

Run `helper <sdk-root> <dxgi-index> <exact-adapter-name> <new-cache-directory>`.
It refuses non-NVIDIA/incorrect adapter identity and missing runtime; no GPU,
runtime, model, quality, CPU or format fallback occurs. One D3D11 device/context
belongs to each helper process. Startup identity includes PID, DXGI ordinal,
name, vendor/device, LUID, SDK version, quality and color policy. Actual loaded
module paths are audited separately. NGX failures preserve their exact numeric
status and SDK enum meaning; a platform error is not relabeled unsupported GPU.

The helper reuses one input/output/UAV/staging-readback set at fixed geometry.
Blocking readback completes before the next upload; changing geometry replaces
the set. Per-frame resource creation failed the original long-video gate after
71 evaluations. The evaluated replacement preserves the memory limits and passes
512 consecutive 720p 2x evaluations plus a complete 5,303-frame export. Add
`--size 1280x720 --steady-frames 512` to the reference runner (with matching
1280/720 maximums) to exercise deterministic output and stable resource reuse.
The 512 outputs must match the exact SDK-reference case hash. This runner retains
one input/output pair, not a raw-frame sequence. See rtx-streaming interop evidence.

## Binary protocol version 1

Stdin/stdout use binary mode. Every header has ten explicit little-endian u32s:
`magic, version, operation, request_id, generation, width, height, scale, color, payload_bytes`.
Request magic is `0x31585452`, response magic is `0x31595452`; version is 1.
Audit/error JSON lines use stderr exclusively. SDK sample internals may emit
additional diagnostic text on stderr in the reference executable.

- Startup response operation 0: width/height are maximum protocol input 1280/720, color 1,
  other identity fields zero, payload is UTF-8 JSON identity (at most 8192 bytes).
- Frame request operation 1: strictly increasing nonzero request ID, nonzero
  generation fixed for the process, width 1..1280, height 1..720, scale 2 or 4,
  color 1, tightly packed RGBA payload exactly width × height × 4 bytes. All alpha
  bytes must be 255. Validate all fields before allocating/reading payload.
- Frame response operation 1 echoes ID/generation/scale/color and returns exact
  enlarged width/height and RGBA8 payload. There is one synchronous frame in
  flight; no frame history, per-frame file output or internal frame queue.
- Shutdown request operation 2 has all remaining fields zero. Clean boundary EOF
  also shuts down. Successful SDK cleanup precedes empty operation-2 response.
- Invalid/truncated input, resource/SDK/pipe failure exits nonzero with a bounded
  error record on stderr. No success frame is sent for a failed evaluation.
  The caller must discard incomplete responses and retire/reap the process.

Color 1 means opaque encoded RGBA8, full RGB range, BT.709 primaries and sRGB
transfer, matching native `color::TO_SRGB`. It asserts the caller's normalization;
the helper does not infer input tags or convert YUV/HDR/transfer functions. VSR
quality is fixed to SDK High (3); TrueHDR is disabled. The protocol is deliberately
incompatible with the ONNX tensor helper. Only the supported declared color tag
is echoed; 10-bit/HDR/alpha are rejected.

At maximum protocol geometry, one input is3,686,400bytes and4x output is
58,982,400bytes. Explicit helper D3D11 storage is input+output+staging output;
driver/runtime allocations and alignment are additional. Protocol capacity is not
UI acceptance: current full preview/effects/export scope is2x<=1280x720 and
4x<=640x360, both output<=2560x1440. Larger4x720 fails existing staging/CLAHE
admission. Whole-frame RTX admission has its own checked ledger; it does not reuse
portable Real-ESRGAN's577x579 tile cap. The384MiB ledger bounds explicit staged
resources plus estimated opaque allowances, not total VRAM. Actual helper DXGI
samples can exceed that estimate and are reported separately. Runtime guards
retain256MiB free and reject helper process-local samples above1536MiB.

Module auditing includes NGX's cached VSR `.bin` path: only an exact SHA-256 match
to the supplied SDK VSR DLL passes the evaluation, regardless of filename. The
new source-built helper hash and all initial failures are in the Oct30 larger
envelope evidence. No cached runtime is redistributed.

The owner must enforce pipe/startup/frame/shutdown deadlines and cancellation by
killing/reaping its owned helper if necessary. The evaluation client does this;
Rust video integration now uses the shared bounded child owner (60-second
startup/frame deadline, 2-second shutdown deadline, bounded stderr capture and
kill/reap on retirement). Normal shutdown also verifies protocol completion.
Actual task5.3 pipe cancel/timeout/crash, controlled processing-device loss,
budget rejection and fresh retry now pass. Preview/UI exposure remains pending.

## Reproduce the gate

```powershell
tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/native-rtx-helper/evaluate.py --helper <helper.exe> --reference <reference.exe> --sdk D:/Libraries/RTX_Video_SDK_v1.1.0 --dxgi-index 1 --negative-adapter-index 0 --adapter-name 'NVIDIA GeForce RTX 3050 Laptop GPU' --output <new-evidence-directory>
```

Requires explicitly provisioned evaluation Python/NumPy. No downloads/installs.
The test pins **maximum RGBA8 error 0 and MAE 0**, then compares 12 sequential
outputs: 192×108, odd 191×107 and 64×36; two patterns, each at 2×/4×. Only one
input and two output images are retained for comparison; raw frames are not
persisted. It verifies binary identity/color/geometry, alpha, nonblack output,
runtime hashes, exact adapter/LUID, accounting, malformed messages, stale IDs/
generations, clean EOF/shutdown, actual missing-runtime/wrong-adapter startup
failure, cleanup and a fresh bit-exact retry. Protocol deadlines use 90 seconds
and kill/reap on failure; audit logs are evaluation files, not application logs.

This proves SDK sample equivalence on the tested machine, not equality to
Real-ESRGAN, colorimetric accuracy, visual/temporal quality, arbitrary geometry,
video timing/audio, production throughput, cancellation during pipe IO, native
preview or UI acceptance. See the latest interop evidence.

## Explicit native video pilot (task5.2)

Build the Rust diagnostic entry point, then supply the helper, SDK/cache paths,
DXGI name/ordinal/LUID and downstream processing adapter explicitly. No asset
download or installation occurs. For the evaluated machine's current session:

```powershell
cargo build -p media-native --bin native-convert --features ai-upscale-pilot
$cli = Join-Path $env:CARGO_TARGET_DIR 'debug/native-convert.exe'
$processingGpu = (& $cli list-gpus | ConvertFrom-Json) | Where-Object { $_.name -eq 'NVIDIA GeForce RTX 3050 Laptop GPU' -and $_.backend -eq 'Vulkan' }
& $cli rtx-video-pilot --input fixtures/ai-upscale-vfr.mp4 --output <new-output.mp4> --scale 4x --fps original --route wgpu --helper tmp/ai-upscale/rtx-helper-accepted-20261029/helper.exe --rtx-sdk D:/Libraries/RTX_Video_SDK_v1.1.0 --cache tmp/ai-upscale/rtx-video-cache-20261029 --dxgi-index 1 --adapter-name 'NVIDIA GeForce RTX 3050 Laptop GPU' --luid-high 0 --luid-low 78108 --adapter-key $processingGpu.key
```

Cache parent must already exist; output must be new. LUID/ordinal are session
identities: re-check the helper/probe audit after a device/driver/reboot change;
do not treat these example numbers as universal. The app checks helper PID,
quality/color/SDK protocol, exact DXGI/LUID and processing adapter name/vendor/
device. Resources remain separate; wgpu's descriptor does not provide a shared
device handle or prove a zero-copy bridge.

RTX supplies native2x/native4x directly. Existing native source-PTS/audio/FPS and
H.264 software-encoder policy is reused; fixed-FPS duplication does not repeat
inference. Every frame's helper upload/readback and downstream wgpu transfers
are separate report counters. `helper_memory_sampling_available=false` means
`sampled_helper_local_peak=0` is unavailable telemetry, not zero SDK memory;
helper-side sampled admission remains active. The shared job ledger uses a
conservative allowance, not measured opaque SDK allocation ownership.

Reproduce each six-file matrix and its independent verification:

```powershell
tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/native_video.py --backend rtx --cli $cli --helper <helper.exe> --sdk <sdk-root> --cache <existing-cache-parent> --luid-high 0 --luid-low 78108 --output <new-matrix-directory>
$env:AI_UPSCALE_NATIVE_VIDEO='1'
node tools/ai-upscale/verify_video.mjs <matrix-directory>
tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/compare_native_video.py <first-matrix-directory> <second-matrix-directory> <new-repeat-report.json>
```

Three actual matrices pass both scales × Original/15/60 FPS on the tiny CC0
64x36 VFR/audio fixture. All corresponding files are byte-identical and decoded
frame hashes repeat. Actual Rust pixels match the pinned SDK sample hashes and
the neutral native GPU preencoder preserves them at0LSB (pinned limit1LSB).
Oversized sources, wrong LUID/processing GPU, direct effect routes and
the failed AI NVDEC/NVENC timing route reject. Main10/HDR stays unsupported.
This is a debug-build export pilot, not ordinary native UI or quality/performance
acceptance. Task5.3 effects/lifecycle now passes; native preview/UI gates are next.

## Effects and active lifecycle (task5.3)

The explicit shared-wgpu RTX pilot supports CLAHE at the selected native2x/4x
output geometry, followed by independent brightness/contrast/saturation. Add
`--rtx-effects` to the matrix command above for CLAHE50 and manual10/115/80;
individual CLI effect flags remain available. Both scales x Original/15/60FPS
pass independent full A/V decode/grid checks with four source inference frames
and16 recorded CLAHE passes per export. Manual-after-CLAHE scalar references at
strength0/50/100 have max1LSB (pinned2LSB); zero bypass and increasing strength
are checked separately. No new CPU pixel readback is hidden in another route.

Build a separate fault-enabled helper for explicit evaluation only:

```powershell
./tools/ai-upscale/native-rtx-helper/build.ps1 -SdkRoot <sdk-root> -Output <new-diagnostic-helper.exe> -Diagnostic
$env:DIAXUS_RTX_HELPER='<normal-helper.exe>'
$env:DIAXUS_RTX_DIAGNOSTIC_HELPER='<diagnostic-helper.exe>'
$env:DIAXUS_RTX_SDK='<sdk-root>'
$env:DIAXUS_RTX_CACHE='<existing-cache-parent>'
$env:DIAXUS_RTX_LUID_HIGH='0'
$env:DIAXUS_RTX_LUID_LOW='78108'
cargo test -p media-native --features ai-upscale-pilot --lib actual_rtx_effect_order_before_fps_at_selected_geometry -- --ignored --nocapture
cargo test -p media-native --features ai-upscale-pilot --lib actual_rtx_pipe_cancel_timeout_crash_and_fresh_video_retry -- --ignored --nocapture
```

The latter test derives an ignored192x108 CC0 video from the tiny checked-in
fixture, preserving timestamps/audio. After one completed frame, the diagnostic
helper deliberately stalls before input payload or before response, or exits73
after real SDK evaluation/readback. Test-only thread-local controls record the
actual Rust worker IO stage and shorten the frame deadline to3s; production
remains60s. They never modify process-global environment or the normal helper.
Only a `RTX_DIAGNOSTIC` C++ build contains/reads the fault environment variables.
The normal binary excludes those strings and two-frame execution ignores them.

Seven actual failure cases (both directions of cancellation/timeout, crash,
controlled wgpu processing-device destruction, client budget rejection) remove
partials and allow a healthy explicit fresh-helper/generation retry with effects.
For the five helper fault cases, Windows tasklist independently confirms the
owned helper PID is gone after retirement. Killing/reaping a process is not an
orderly SDK shutdown; healthy retries and final normal shutdown are separate
evidence. These stalls bracket pipe IO, not a claim of interruption inside the
vendor's GPU kernel. Physical OOM/spontaneous driver loss remain unverified.

The local SDK/license/API/driver/deployment inventory is recorded with hashes in
`docs/evidence/ai-upscale-20261029/rtx-lifecycle/environment-license-deployment.json`.
User consent covers local evaluation only. SDK binaries/headers/diagnostic builds
are not distributed or committed; shipping license/deployment review remains
separate. Current input cap192x108 and ordinary native UI gate remain unchanged.


Larger gate (no downloads/installs in this runner): pass `--max-width 1280
--max-height 720 --size 640x360 --size 1280x720` to the evaluation runner.
This retains the12 tiny/odd regression images and adds8 larger sample comparisons.
See `native_rtx_envelope.py` for export/limit tests and the newest interop record
for actual outcomes. First-frame image structure is a separate gate from timing.
