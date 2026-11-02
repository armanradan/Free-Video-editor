# Native independent decode/NVENC planning (2026-11-02)

Native routes now choose decoding independently of NVIDIA encoding. Software
routes use software codecs. NVIDIA routes plan software decode for legacy
MPEG/WMV/VC-1, retaining NVENC. Evaluated H.264/HEVC420 configurations require a
successful selected-device NVDEC decode/download probe. An unexpected probe or
runtime decoder failure stops rather than triggering a retry with another codec.
All NVIDIA jobs additionally encode a real frame at the requested output size
on the selected device before partial creation; discovery/name lists alone do
not grant capability. These probe transfers are setup diagnostics, excluded
from conversion counters. Output stream metadata must identify the requested
NVENC encoder; no software encoder substitution is accepted.

Source/video processing still uses the original and existing shared processor.
CPU-decoded legacy video normalizes rational pixel aspect before shared effects
or RTX. Direct hybrid routes scale/convert in CPU and explicitly upload NV12 or
P010 to selected CUDA/NVENC. Shared hybrid routes retain the staged RGBA bridge
and NVENC upload; NVDEC downloads are counted only for the actual NVDEC plan.
The native completion line reports actual software/NVDEC decode and encoder.
RTX no-B-frame policy and existing size/memory/device gates are retained.

## Verified so far

- MPEG-1 direct/shared hybrid:50frames,384x288, complete independent decoding,
  PTS/end/audio and three sampled image checks pass. Maximum PTS/end error0.912ms,
  audio end0.434ms, sampled RGB RMSE<=5.467. Both reports identify software decode
  plus h264_nvenc, zero NVDEC download and8,294,400NVENC upload bytes.
- MPEG-2/MP2 direct/shared hybrid:30frames and native full output verification
  pass. Independent complete family-matrix/image acceptance remains task5.3.
- H.264 and HEVC controls pass actual selected-device NVDEC/NVENC preflight and
  shared export. Reports identify NVDEC and nonzero actual download counters.
- `wma-matrix.json`:12 direct/shared NVIDIA exports across WMV1/2/3, WMA1/2/Pro,
  VC-1 Advanced and a positive-offset/interior-gap control. All independently
  pass complete A/V decode, every video PTS/count and Windows-referenced audio
  waveform/endpoint checks. Each output's stream tag identifies h264_nvenc,
  its plan is software decoding and its NVDEC download count is zero.
- `aac-regression.json`:all20 anomaly/healthy/control route exports pass again,
  including exact corrected/reference decoded PCM equality per route. Initial
  invocation mistakenly used96x80 fixtures; selected-device NVENC preflight
  correctly rejected those dimensions. The rerun uses the existing192x144
  controls required by the checker. No gate/tolerance was weakened.
- MPEG-1 RTX2x/4x shared hybrid:50frames at768x576/1536x1152 independently pass
  full A/V/timeline/aspect and sampled images (RGB RMSE<=5.545). Source SAR178:163
  is normalized352x288->384x288 before either AI scale. NVIDIA hardware download
  counters remain zero; NVENC uploads are33,177,600/132,710,400bytes respectively.
- `wmapro-rtx.json`:actual48-frame WMV3 Main/WMA Pro RTX2x hybrid at1576x1152
  independently passes (video PTS error0, audio endpoint9.274ms, waveform lag0,
  RMSE0.0033). Windows audio, software video decoding and NVENC are identified.

RTX3050Laptop/Vulkan, driver621.01, SDK1.1 High3 and evaluated helper SHA256
e9188624f8a4605f4429f83f8c74463a22db9bfd374a36f590a23f37cf5cac95 remain the
actual tested environment. CPU staging/audio and opaque driver/SDK allocations
remain; these are not surface-sharing or real-time performance claims.

## Original MPG hybrid RTX export

The actual original MPG also passes RTX2x with planned software decoding and
NVENC:6941/6941frames,768x576,86,302,510bytes,total163245ms (conversion138553ms).
Complete independent A/V decoding, every frame PTS/end/audio endpoint and three
image samples pass: maximum video PTS/end error0.778ms, audio origin0/end0.245ms,
sampled RGB RMSE<=3.923. Stream encoder metadata identifies h264_nvenc; NVDEC
download0, NVENC upload4,605,714,432bytes. Source SHA256/size/exact UTC mtime
match the prior verified original identity (original-identity.json). Timings
co-ran other validation and are observations, not controlled benchmarks.

## Failure and retry

The separately executed opt-in `actual_hybrid_preflight_cancel_cleanup_and_retry`
passes on the actual selected NVIDIA device (`lifecycle.json`): an unavailable
UUID makes the NVDEC preflight fail explicitly without retry; too-small96x72
NVENC output fails before partial creation on both hybrid routes. Active jobs
are cancelled only after a nonempty encoder partial is observed. Both routes
remove their partials, complete fresh50-frame NVENC retries, preserve exact
transfer accounting and retain verified encoder identity. No FFmpeg process
referring to the unique test-owned source/output directory remains afterward.
This tests an unavailable device and rejected encoder configuration, not an
installation with the NVENC library physically removed.

Native/app tests83+20pass (23+6explicit opt-ins ignored by the default run), plus
the opt-in lifecycle run above. Matching-target lint with warnings denied,
formatting and shared core/GPU host checks pass. The full Original/fixed-FPS
family/RTX/image matrix and ordinary-window
acceptance remain tracked separately; this report does not complete them.

To reproduce the lifecycle run, set a new absolute `DIAXUS_TEST_HYBRID_DIR`
and an explicit `DIAXUS_TEST_HYBRID_ADAPTER`, then run:
`cargo test -p media-native actual_hybrid_preflight_cancel_cleanup_and_retry --lib -- --ignored`.
Independent WMA checking uses `verify_windows_wma.py` with `--route nvidia`
and/or `--route wgpu-nvidia`; existing RTX outputs may be supplied with
`--existing-output` and `--native-report`. Private inputs remain outside git.
