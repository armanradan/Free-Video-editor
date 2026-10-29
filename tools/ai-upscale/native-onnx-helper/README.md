# Optional DirectML inference helper

This source-only Windows x64 helper is used by `media-native::ai_inference`.
It is an opt-in inference component, separate from codec and preview code. It
never uses the NVIDIA RTX Video SDK and is never a browser service. Normal app
launch and AI Off do not start it. Supply a locally compiled helper, existing
runtime directory and imported allowlisted model cache explicitly.

Build with the already installed MinGW-w64 G++ and **unmodified official**
[ONNX Runtime v1.23.0 C API headers](https://github.com/microsoft/onnxruntime/tree/v1.23.0/include/onnxruntime/core/session).
`build.ps1` verifies both header hashes before invoking the specified compiler:

```powershell
tools/ai-upscale/native-onnx-helper/build.ps1 -Compiler '<g++.exe>' -Headers '<local-header-directory>' -Output '<ignored-output.exe>'
```

Headers and ONNX Runtime are MIT licensed; retain
`../ONNX-Runtime-LICENSE.txt`. DirectML is an external runtime dependency with
its own supplied notices; this source does not redistribute its DLL. The helper
expects ONNX Runtime DirectML 1.23.0, DirectML DLL 1.15.4.0 and C API 23. Rust's
read-only discovery checks x64/product metadata before launching; the child
checks the actual C API version before session creation. No installation or
license acceptance is automated.

The MinGW compatibility block selects the official header's existing-Windows-SAL
branch only during inclusion, avoiding conflicting non-MSVC annotation stubs.
It leaves downloaded headers unchanged. The Windows x64 calling convention and
numerical behavior were exercised with the actual compiled helper. No MSVC build,
other architecture or general compiler compatibility is claimed.

Protocol v2 starts with one bounded JSON identity line, then little-endian binary
messages. Request: six u32 fields (magic `0x41495131`, version 2, operation,
width, height, payload bytes). Operation 1 accepts one FP32 RGB NCHW tile, each
dimension 1..196, all finite in [0,1]; output is raw unclamped x4 FP32 RGB NCHW.
Operation 2 is stop and requires zero dimensions/bytes. Response: four u32 fields
(magic `0x41494f31`, version 2, status 0, output bytes), then three u64 fields
(input/output tensor transfer payload bytes and sampled process-local GPU peak),
then output bytes. Invalid messages fail with a precise stderr boundary and
nonzero exit. There is no implicit engine/device/CPU fallback or restart.

Rust owns the shared core 128-interior/34-halo tile plan, normalization and
cropped RGBA8 assembly; the child owns one DirectML device/session and one tile
run at a time. Only normalized opaque SDR source <=577x579 is admitted by the
parent. Pipes carry tensors, never GPU handles or per-frame filenames. A frame
has one synchronous mutable owner and no queued sequence. Cancellation/error
retires the child; Drop kills/reaps it and joins pipe workers. Startup and frame
deadlines are 60 seconds; normal shutdown allows two seconds. Output is published
by the diagnostic CLI only after inference and session close succeed.

The child checks OS DXGI headroom with a 256 MiB workspace estimate and a
1536 MiB process-local admission guard before work. These are estimates and
samples, not a strict runtime allocation ceiling. Explicit CPU I/O binding
requires counted FP32 host staging in both directions, including overlaps. No
zero-copy, hardware codec, renderer-device sharing or total-VRAM claim follows.

Reproduce numeric, provider placement, malformed protocol and the complete
25-tile envelope with existing local assets:

```powershell
python tools/ai-upscale/native_helper.py --helper '<helper.exe>' --runtime '<onnxruntime-capi-directory>' --model tmp/ai-upscale/reference/model.onnx --reference tmp/ai-upscale/reference --cli '<native-convert.exe>' --cache tmp/ai-upscale/model-cache --output '<new-ignored-result-directory>'
$env:DIAXUS_AI_HELPER = '<helper.exe>'
$env:DIAXUS_ORT_DIRECTORY = '<onnxruntime-capi-directory>'
$env:DIAXUS_AI_MODEL_CACHE = '<verified-model-cache>'
cargo test --locked -p media-native ai_inference::tests::actual_helper -- --ignored --nocapture
```

The latter opt-in test explicitly requires the assessed NVIDIA DXGI ordinal 1
and tests repeated frames, exact reference pixels, stale generations, active
request cancellation, retired rejection, fresh-child retry and actual child
crash cleanup. The standalone runner needs the previously generated
`reference-envelope` beside `reference`. It stores diagnostic raw input/output
under ignored storage only; the runtime adapter itself uses pipes.

The separate `ai-upscale-pilot` Rust feature enables `ai-video-pilot` on the native
CLI. `native_video.py --cli '<feature-built-native-convert.exe>' --helper
'<helper.exe>' --runtime '<runtime-directory>' --cache '<model-cache>' --output
'<new-ignored-result-directory>'` runs 2x/4x × Original/15/60 with existing
CLAHE/manual processing and software codecs. Set `AI_UPSCALE_NATIVE_VIDEO=1` and
run `node tools/ai-upscale/verify_video.mjs '<result-directory>'` for independent
complete timing/geometry/audio/decode checks. Normal `convert_job` remains gated.
The AI NVDEC/NVENC route is unavailable because its actual tiny Original-VFR test
shifted video PTS; the CLI returns that reason before inspection or output. The
source-only helper still uses the selected NVIDIA DirectML GPU with software
codecs on the passing shared-wgpu route. No automatic codec fallback is performed.
