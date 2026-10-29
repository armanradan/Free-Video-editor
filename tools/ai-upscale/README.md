# AI upscale feasibility tools

Native RTX task5.3 now passes the staged CLAHE/manual bridge and seven active
failure/cleanup/retry cases, including real blocked pipe reads/writes. Normal
helpers exclude the diagnostic fault controls. See the [native RTX guide](native-rtx-helper/README.md)
for explicit reproduction; input remains<=192x108 and native preview/UI/release
acceptance stays open. Earlier continuation notes below are historical.


Native RTX task5.2 now passes explicit staged video exports at2x/4x with
Original/15/60FPS, preserved VFR/audio policy, independent decode and exact file/
pixel repeats. See [the native helper/video guide](native-rtx-helper/README.md).
Input remains at most192x108; task5.3 effects/lifecycle and preview/UI gates remain
open. The older task5.1 continuation below describes the earlier state.


Native RTX continuation: the [bounded helper](native-rtx-helper/README.md) now
passes task5.1 with exact unmodified SDK sample equivalence (12 comparisons,
17 process runs). It remains optional and source-only; native video/preview/UI
integration is still pending. Browser work is paused by user direction.


## Explicit NVIDIA browser launch — 2026-10-26

`./tools/ai-upscale/launch-nvidia-browser.ps1` opens the running local app in a
separate Edge 156 profile under ignored `tmp/ai-upscale/edge-nvidia-profile`, with
`--force-high-performance-gpu --use-angle=d3d11`. It verifies the server isolation
headers, installs/downloads nothing and changes no OS/default profile settings.
The user must load a source, confirm `nvidia / ampere` and select H.264/AAC
explicitly. Existing model/runtime cache is reused; a new profile needs explicit
setup. The launcher does not guarantee GPU selection on another machine.

For candidate probes set `AI_UPSCALE_BROWSER_ARGS='["--force-high-performance-gpu"]'`
and use the existing numeric/tile/video modes. For ordinary UI acceptance set
`AI_UI_BROWSER_ARGS` to that JSON array. `AI_UI_HEADED=1` runs a visible Edge
window without `--enable-unsafe-webgpu`; `AI_UI_PREVIEW_ONLY=1` also checks
missing-model/retry/limits and NVIDIA unsupported-WebM recovery to H.264.
This instrumentation observes the real application and does not replace its
capabilities. The new GPU is accepted only for tested paths; the SDK is separate.

The 24-case isolated NVIDIA benchmark used no experimental compiler override.
Its 64-frame 192×80 AI conversions took 3.26–3.68 seconds. The earlier Intel
benchmark explicitly disabled DXC, so the 10–14× comparison is between observed
GPU/configuration combinations, not an isolated hardware-only effect. All use
identical weights, preprocessing, effects and requested sizes. Startup, pure GPU
hardware timing, opaque memory and larger-source throughput have distinct limits.

The NVIDIA WebM matrix passed browser-side checks but the independent verifier
failed because FFprobe did not expose VP8 packet durations. The first attempt
also contained an invalid profile id (harness error); both are preserved under
ignored evaluation storage. The H.264 subset retains real artifacts/source
lineage and independently passes Original/15/60. No missing duration is guessed,
and NVIDIA WebM is explicitly unavailable in ordinary UI/backend policy.


## Browser controls acceptance — 2026-10-25

Normal `dx build --web --locked --package converter-web` enables the optional
ONNX engine. This is scoped to Windows Edge 156 / Intel gen-12lp, secure isolated
origin, exact imported model/runtime and opaque 8-bit SDR sources up to 577×579.
Native ordinary controls remain gated. See the newest interop report section for
all acceptance qualifications and evidence; older pilot statements below are
historical. No experimental compiler argument is needed for the accepted ordinary
UI, default FP32 or tiled checks.

`tests/ai-enabled-ui.mjs <evaluation-root> <public-dir> <fresh-output-dir>` drives
ordinary controls, both themes and main/worker, including actual H.264/AAC exports.
`AI_UI_PREVIEW_ONLY=1` exercises controls plus missing-model/reimport/retry/Off and
oversized-source rejection without exporting. `AI_UI_SKIP` is a JSON array of
context-theme names to skip. Runtime acquisition explicitly requested in this
harness is fulfilled using existing hash-pinned local assets. It does not prove
fresh CDN availability. The server observes results, without replacing ordinary
commands or capabilities. UI exports can be independently checked using
`AI_UPSCALE_VERIFY_CONTEXTS='["main-light","main-dark","worker-light","worker-dark"]'`
with `verify_video.mjs`.

For raw default-compiler references, clear other diagnostic modes and set
`AI_UPSCALE_PREALLOCATED=1`; `AI_UPSCALE_TILE=1` separately tests the shared tiled
path. Leave `AI_UPSCALE_BROWSER_ARGS` unset to check installed browser defaults.
A current release diagnostic wasm/bindgen probe and bundled media-probe are
required as documented below. The accepted Oct 25 raw/tile runs use Edge
156.0.4314.42, ORT Web 1.30.0 and no experimental Dawn argument.

`quality_fixtures.py --frames 64` creates bounded 64-frame licensed excerpts;
default remains eight frames. `quality_reference.py` compares every frame with
the trusted matching-weight CPU model and exact native DirectML provider.
`quality_temporal.py --directory <benchmark> --output <fresh-directory>` produces
bounded offline encoded-output differences and chronological contact sheets.
These are not ground-truth fidelity or motion-compensated flicker metrics.
Source attribution/license and changes are retained in the fixture manifest.

For same-size bilinear comparison, a diagnostic VIDEO case may specify
`"bilinear":true`; it uses the requested scale with the same downstream effects
and codec but no inference. The comparator is diagnostic-only. AI Off retains
ordinary no-upscale sizing. The Oct 25 isolated comparison used the explicit
`--disable-dawn-features=use_dxc` argument, with default-compiler checks separately
recorded. Structured setup/processing/decoder/encoder/verification timers overlap
and are not pure hardware execution. The Intel result is much slower than
bilinear; do not promise real-time preview or superior image fidelity.

## Licensed short-clip release evaluation

See also [the native three-engine comparison](#native-release-quality-and-performance-comparison)
for release RTX/DirectML/shared-GPU bilinear measurements on the existing64-frame
excerpts and cut fixture.

These developer commands do not enable ordinary AI controls. The explicit
acquisition command retains only three eight-frame 192x80 excerpts under ignored
storage, with hashes, timestamps, source/license/attribution and exact FFmpeg
recipes. It reads remote ranges instead of retaining the complete movie. The
film's original soundtrack is removed; original synthetic sine audio is added.
Retain the generated provenance manifest with any derived test outputs.

```powershell
python -X utf8 tools/ai-upscale/quality_fixtures.py --output tmp/ai-upscale/quality-clips-new
cargo build --locked --release -p media-native --bin native-convert --features ai-upscale-pilot
python -X utf8 tools/ai-upscale/native_video.py --cli '<cargo-target-dir>/release/native-convert.exe' --helper '<helper.exe>' --runtime '<existing-runtime-directory>' --cache tmp/ai-upscale/model-cache --fixture '<absolute-excerpt.mp4>' --output '<fresh-native-result-directory>'
$env:AI_UPSCALE_NATIVE_VIDEO = '1'
node tools/ai-upscale/verify_video.mjs '<native-result-directory>'
```

Repeat the matrix command for each acquired excerpt. It independently obtains the
decoded source count rather than assuming four frames. Both scales run at
Original/15/60 with CLAHE/manual effects. The verifier uses native stream
presentation origin (AAC priming is trimmed) and browser packet origin (priming
is retained). Do not use a priming packet as native audible start. Independent
evidence must retain any earlier failures rather than overwrite them.

Build the release browser WASM/glue using the commands below, then run the same
local excerpts in main and worker contexts:

```powershell
$env:AI_UPSCALE_VIDEO = '1'
$env:AI_UPSCALE_TILE = '0'
$env:AI_UPSCALE_CODEC = '0'
$env:AI_UPSCALE_PREVIEW = '0'
$env:AI_UPSCALE_VIDEO_FAILURES = '0'
$env:AI_UPSCALE_VIDEO_RECOVERY = '0'
$env:AI_UPSCALE_VIDEO_FFMPEG = '0'
$env:AI_UPSCALE_VIDEO_PARITY = '1'
$env:AI_UPSCALE_FIXTURE_DIR = (Resolve-Path tmp/ai-upscale/quality-clips-new).Path
$env:AI_UPSCALE_VIDEO_CASES = '[{"fixture":"tos-face.mp4","scale":"2x","fps":"original","clahe":true,"color":"10/115/80"}]'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-public-directory>' '<fresh-browser-result-directory>'
$env:AI_UPSCALE_NATIVE_VIDEO = '0'
node tools/ai-upscale/verify_video.mjs '<browser-result-directory>'
```

Expand the case list to both scales and all three FPS settings. Additional
fixture directories must be inside the evaluation root; the server retains its
path containment check. For AI Off regression, use scale `off`, neutral color,
CLAHE false and `AI_UPSCALE_VIDEO_PARITY=0`. An Off-only matrix skips model/runtime
provisioning entirely. These exports run the real media-web processor through a
diagnostic harness; they are not ordinary UI acceptance.

The following offline reference uses existing pinned dependencies/assets,
explicitly verifies NVIDIA DXGI ordinal 1, and rejects CPU node fallback. It
compares all acquired frames against CPU PyTorch and writes a neutral first-frame
bilinear/model visual comparison. It installs/downloads nothing:

```powershell
& tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/quality_reference.py --assets tmp/ai-upscale --fixtures tmp/ai-upscale/quality-clips-new --adapter-name 'NVIDIA GeForce RTX 3050 Laptop GPU' --output '<fresh-reference-directory>'
```

This is not a shared-GPU bilinear benchmark. Reference CPU IO, model staging and
browser diagnostic readbacks are excluded from production throughput claims.
The small samples do not prove long-run temporal quality; text deformation and
fine-detail smoothing remain observed limitations. See interop-report.md for
the actual release results, memory/copy counters and outstanding gates.

## Gated selection and ordinary AI Off UI regressions

The active sampled-preview diagnostic uses `AI_UPSCALE_VIDEO=1`,
`AI_UPSCALE_PREVIEW=1`, `AI_UPSCALE_PREVIEW_ACTIVE=1` and
`AI_UPSCALE_VIDEO_CASES='[]'`. Leave other preview/history/failure flags unset.
After rebuilding the release tile-browser WASM with matching wasm-bindgen below,
run `browser.mjs` against existing local pinned ORT/model bytes into a fresh
directory. Both scales/contexts exercise real inference-dispatch cancellation,
actual runtime input validation, device destruction during inference submission,
explicit retry and different decoded-source replacement/restore. Successful
selected pixels match exactly; separate canvas readbacks check unchanged display
on cancel/runtime error. Lost-device display contents are not asserted. This mode
exports no video: its results are preview/ownership evidence. The sampled command
flag does not prove ordinary AI On audio-clock/UI coalescing or real-time playback.
Unset `AI_UPSCALE_PREVIEW_ACTIVE` before other probes.

`tests/browser-preview-lifecycle-ui.mjs` runs the built ordinary app with actual
decode/GPU/audio-clock work in both contexts. Its server delays a completed
preview response (or rejects delivery after successful work) to exercise rapid
Pause/Play, current/retired errors, explicit recovery, source replacement and
conversion handoff, plus a burst of explicit seeks while playing. It independently
decodes both resulting 60-frame exports:

```powershell
node tests/browser-preview-lifecycle-ui.mjs tmp/ai-upscale '<built-public-directory>' '<fresh-lifecycle-result-directory>'
```

It uses the retained local `tmp/preview-click-20261008.mp4` playback fixture and
checked-in `fixtures/m2-h264-aac.mp4`. It adds no production transport hook and
does not establish AI On UI acceptance or real codec-failure coverage.

The diagnostic AI handoff uses the existing release-WASM harness below. Set
`AI_UPSCALE_VIDEO=1`, `AI_UPSCALE_PREVIEW=1`,
`AI_UPSCALE_PREVIEW_LIFECYCLE=1`, `AI_UPSCALE_VIDEO_CASES='[]'`, and leave the
other preview/history/failure modes unset. Set the pinned ORT directory and Edge
as below, then run `browser.mjs` and `verify_video.mjs` into a fresh directory.
Both scales exercise preview to conversion without explicit clear, exact
restored preview pixels, cleanup failure rejection before startup and fresh
context retry. The fault is injected after actual ORT release; it tests response
failure policy, not real runtime cleanup failure. It deliberately creates three
successive devices per context, with no concurrent device fallback. Runtime
assets are served from existing local verified bytes; there are no new downloads.
Unset `AI_UPSCALE_PREVIEW_LIFECYCLE` before other probes. Raw pixel snapshots are
diagnostics only; eight exports require the independent verifier.

`cargo test --locked -p ui` dispatches actual shared selector handlers in both
panel variants. Its validated metadata is test-only and grants no app capability.
`tests/ai-setup-ui.mjs` additionally checks real built-app disabled choices and
retained ordinary resize. `tests/ai-selection-preview-ui.mjs` exercises actual
decoded AI Off CLAHE/Before/Play-Pause independently of FFmpeg export readiness:

```powershell
node tests/ai-setup-ui.mjs tmp/ai-upscale '<built-public-directory>' '<fresh-ui-result-directory>'
node tests/ai-selection-preview-ui.mjs tmp/ai-upscale '<built-public-directory>' '<fresh-preview-result-directory>'
```

The installed Edge is selected explicitly by these runners; playback uses
`--autoplay-policy=no-user-gesture-required`. The worker nonblack-pixel check is
a separate diagnostic readback. Neither test proves AI On playback acceptance.

## Native AI preview bridge probe

The ordinary native conversion handler now reserves preview admission, retires
playback/pending commands and waits on its conversion thread for AI/history cache
retirement. All conversion exits release admission; a visible preview reopens
paused. The actual diagnostic test covers both AI scales, active preparation,
stale-command rejection, pre-cancelled conversion, a fresh verified conversion,
paused pixel restoration and concurrent renderer close:

```powershell
cargo test --locked -p converter-native --bin converter-native actual_ai_conversion_handoff -- --ignored --nocapture
```

Use the explicit helper/runtime/cache environment below. A dev-only dependency
feature exposes the existing pilot conversion function to this test; it grants
no ordinary app AI capability. Read the printed fresh result directory's
`report.json`. Both input and output AI coverage now use exact packet endpoints;
short MP4 decode-duration headers are not treated as presentation coverage.
`verify_video.mjs` independently checks the final endpoint as well as frame PTS.
The ordinary AI Off shared-GPU path retains its legacy duration policy and
rejects the tiny cut fixture's invalid last duration; its successful window
handoff evidence uses the existing CFR fixture. See interop-report.md for that
separate limitation and the actual ordinary AI Off success/error checks.

The source-only native preview service and renderer worker tests use the same
explicit `DIAXUS_AI_HELPER`, `DIAXUS_ORT_DIRECTORY`, and
`DIAXUS_AI_MODEL_CACHE` settings as the native inference/video probes below.
They install/download nothing. Run from the repository root:

```powershell
cargo test --locked -p media-native --lib actual_ai_preview -- --ignored --nocapture
cargo test --locked -p converter-native --bin converter-native actual_ai_renderer -- --ignored --nocapture
```

Each prints a fresh ignored output directory. Compare its real display pixels
with full chronological pre-encoder processing and independent display scaling:

```powershell
& tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/native_preview_pixels.py --directory <native-service-result-directory> --worker-directory <renderer-result-directory> --output <fresh-pixels-json>
```

The service stages one bounded <=640x360 RGBA8 display image into a new renderer-
owned wgpu-26 texture. Native processing stays on wgpu 30; no GPU handles cross
that boundary. Tests cover 2x/4x, finite history/cut/expiry, downstream cache edits,
replacement, actual renderer destruction and fresh generation retry. Inference
staging, repeated cached-x4 processing uploads, all display readbacks (including
unused preroll images), and each renderer image upload are explicit counters.
These are headless actual-GPU tests, not ordinary window/UI or playback acceptance.

`AI_UPSCALE_PREVIEW=1` with `AI_UPSCALE_VIDEO=1`,
`AI_UPSCALE_VIDEO_CASES='[]'` and `AI_UPSCALE_VIDEO_FAILURES=0` runs the
paused-preview parity probe. Set `AI_UPSCALE_PREVIEW_SCALE` to `2x` or `4x`;
`AI_UPSCALE_PREVIEW_FIXTURE` selects a repository fixture (default tiny VFR).
Use `fixtures/ai-upscale-preview.mp4` for the 4x display-bound case, or
`fixtures/ai-upscale-rotated.mp4` for orientation. Run the normal `browser.mjs`
command below into a fresh directory, then:

```powershell
& tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/preview_pixels.py --directory <result-directory> --output <result-directory>/pixels.json
node tools/ai-upscale/verify_video.mjs <result-directory>
```

The isolated harness enables explicit diagnostic ingress/output pixel readbacks.
It checks cached paused edits, Before/After, Off and rejected FFmpeg export with
working preview against independent pixels and actual matching exports. It does
not validate temporal preroll, playback, native preview or ordinary UI exposure.
Set `AI_UPSCALE_PREVIEW=0` for conversion/failure-only runs.
Set `AI_UPSCALE_PREVIEW_HISTORY=1` together with `AI_UPSCALE_PREVIEW=1` to
use `browser-preview-history.mjs` instead: the tiny cut fixture checks exact PTS,
100-ms predecessors/expiry, seek backward, Strength and Before/After cache reuse,
2x/4x invalidation, invalid model rejection and chronological export parity.
Run the same `preview_pixels.py` and `verify_video.mjs` commands afterward.
The default paused probe requires `AI_UPSCALE_PREVIEW_HISTORY=0`.


`AI_UPSCALE_VIDEO_FAILURES=1` with `AI_UPSCALE_VIDEO=1` and
`AI_UPSCALE_VIDEO_CASES='[]'` exercises five actual-video failure/retry cases in
main and worker. The diagnostic server instruments only its served AI boundary;
it never changes cached model/runtime bytes or production source behavior. Tests
cancel after a model dispatch, invoke real ORT missing-input validation, inject
setup/tile OOM scope responses, and destroy the device after tile submission.
OOM is simulated at the WebGPU error-scope boundary; it does not exhaust VRAM.
`ownership()` on the isolated Rust harness checks held leases, submissions and
AI credits/stages; JS instrumentation counts caller tensors, sessions and explicitly
destroyed AI resources. Every failure must reject output and every fresh retry
must pass independent `verify_video.mjs` validation. No driver pool/GC or physical
OOM recovery guarantee is implied.

## Shared Rust tile adapter and codec-policy probes

`tile-browser` is a locked source-only release-WASM harness around media-web's
actual processing context and shared media-gpu packing/unpacking. Production
commands continue rejecting AI On. The harness alone uploads fixture pixels and
reads back results; the adapter performs no CPU pixel IO. It uses the existing
verified asset cache APIs, 128 interiors/34 clipped halos, one context/session,
one frame/tile, caller-owned tensor buffers and a per-job admission ledger.
See interop-report.md for measured peaks, numerical checks and lifecycle limits.

```powershell
cargo build --locked --offline --manifest-path tools/ai-upscale/tile-browser/Cargo.toml --target wasm32-unknown-unknown --release
<wasm-bindgen-0.2.128> <cargo-target-dir>/wasm32-unknown-unknown/release/ai_upscale_tile_browser_probe.wasm --target web --out-dir tmp/ai-upscale/tile-browser-wasm --out-name probe
$env:AI_UPSCALE_TILE = '1'
$env:AI_UPSCALE_ORT_DIST = (Resolve-Path tmp/ai-upscale/ort-1.30/node_modules/onnxruntime-web/dist).Path
$env:AI_UPSCALE_BROWSER_ARGS = '["--disable-dawn-features=use_dxc"]'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' tmp/ai-upscale/browser-tile-result
cargo test --locked -p media-gpu ai_tile::tests -- --ignored --nocapture
```

For codec-only checks, build the existing browser media glue with the installed
esbuild into ignored storage. The harness delegates to actual codec queries,
records their configurations and checks that codec success cannot enable AI.
Set AI_UPSCALE_TILE to 0 before this separate mode. Playwright Chromium cannot
decode this H.264 fixture; the explicitly selected installed Edge passes.

```powershell
apps/web/node_modules/@esbuild/win32-x64/esbuild.exe apps/web/assets/m2.js --bundle --format=esm --outfile=tmp/ai-upscale/media-probe.mjs
$env:AI_UPSCALE_TILE = '0'
$env:AI_UPSCALE_CODEC = '1'
$env:AI_UPSCALE_BROWSER_EXE = 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' tmp/ai-upscale/browser-codec-result
```

The executable environment override is explicit harness configuration, not a
product path. Every run needs a new output directory; failed contexts cause a
nonzero process exit. No new downloads/installations are needed for these runs.

Provisioning is now explicit local model import, per the user's decision. The
shared product manifest is `crates/media-core/src/ai-upscale-model.json`; it pins
the same export and runtime asset hashes as these probes. Product APIs remain
gated and the native cache CLI enables no inference. Reproduce real cache tests:

```powershell
node tests/ai-assets-browser.mjs tmp/ai-upscale tmp/ai-upscale/provisioning-browser-new.json
cargo build --locked -p media-native --bin native-convert
node tests/ai-assets-native.mjs <cargo-target-dir>/debug/native-convert.exe tmp/ai-upscale/reference/model.onnx tmp/ai-upscale/provisioning-native-new
$env:AI_UPSCALE_ASSETS = '1'
$env:AI_UPSCALE_ORT_DIST = (Resolve-Path tmp/ai-upscale/ort-1.30/node_modules/onnxruntime-web/dist).Path
$env:AI_UPSCALE_BROWSER_ARGS = '["--disable-dawn-features=use_dxc"]'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' tmp/ai-upscale/browser-cached-runtime-new
Remove-Item Env:AI_UPSCALE_ASSETS
```

The first probe uses real CacheStorage/offline mode and an actual one-byte origin
quota override in its isolated context, plus separately labeled injected failure
checks. Runtime files are local HTTP mirrors of pinned bytes; no external model or
runtime is downloaded. The second creates only bounded test cache/corruption files
under a new project tmp subdirectory. Native cancellation during disk write and
Windows ACL/disk-full cases are not claimed. The final probe creates the existing
processing device and runs all references using verified cached code/model bytes.

Latest tile/memory continuation (no new dependency downloads required):

```powershell
./tmp/ai-upscale/venv/Scripts/python.exe tools/ai-upscale/reference.py --assets tmp/ai-upscale --output tmp/ai-upscale/reference-envelope --case 577x579
$env:AI_UPSCALE_MEMORY = '1'
$env:AI_UPSCALE_ORT_DIST = (Resolve-Path tmp/ai-upscale/ort-1.30/node_modules/onnxruntime-web/dist).Path
$env:AI_UPSCALE_BROWSER_ARGS = '["--disable-dawn-features=use_dxc"]'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' tmp/ai-upscale/browser-memory-result
Remove-Item Env:AI_UPSCALE_MEMORY
./tmp/ai-upscale/venv/Scripts/python.exe tools/ai-upscale/native_memory.py --reference tmp/ai-upscale/reference-envelope --output tmp/ai-upscale/native-memory-result --adapter-name 'NVIDIA GeForce RTX 3050 Laptop GPU'
```

Output directories must be new. The larger reference uses roughly 70 MB; do not
retain a raw video-frame sequence. Browser calibration runs four 128/256 forward/
reverse tile policies per context under a 300 s watchdog (ordinary probes remain
120 s). A 384 MiB guard counts requested buffers/known textures, including runtime
pools, GPU assembly, output reservation and 8 MiB diagnostic staging. It is not
driver/physical VRAM accounting. Native DXGI sampling observes this process's
local/nonlocal usage every nominal 25 ms and may miss peaks. Native transfers are
explicit host staging. See interop evidence for residual allocations after release,
the earlier rejected whole-frame diagnostic readback and all numeric results.

These are offline task-group-1 probes, not application backends. They do not change
the application, download at startup, or enable AI controls. All source/model data
must stay in the ignored `tmp/ai-upscale` directory. The user explicitly authorized
isolated open-source evaluation downloads on 2026-10-22; no driver, C++ SDK/toolchain
installation or NVIDIA license acceptance is implied.

Run from the repository root in PowerShell. Prerequisite inventory alone:

```powershell
./tools/ai-upscale/preflight.ps1 -SdkPath 'D:/Libraries/RTX_Video_SDK_v1.1.0'
```

The SDK path is an explicit evaluation argument, never a product default. The
probe inventories files/hashes and tool availability without loading vendor DLLs.
Python module presence is queried in the specified interpreter; it does not search
every Python environment or establish runtime compatibility.

## Portable model acquisition and reference

`candidate.json` allowlists one strong-denoise x4 compact general model. SHA-256
values were measured from official HTTPS downloads, not publisher signatures.
Keep the upstream BSD-3-Clause notice with derived assets. The official upstream
`scripts/pytorch2onnx.py` hardcodes RRDBNet; using it unchanged for these compact
weights would instantiate the wrong network. `reference.py` instead loads the
hash-verified official SRVGG source, removing only its BasicSR registry import and
decorator. Upstream `inference_realesrgan.py` at the pinned commit specifies 32 body
convolutions and 64 features for this model. Strength is 1 (no WDN weight blending),
not the upstream CLI's default strength 0.5. Internal/output x4 is evaluated; x2
would need a separate tested final shared downscale and remains unavailable.

After deliberate acquisition approval, reproduce in an isolated environment:

```powershell
uv venv tmp/ai-upscale/venv --python 3.12
uv pip install --python tmp/ai-upscale/venv/Scripts/python.exe torch==2.8.0+cpu --index-url https://download.pytorch.org/whl/cpu
uv pip install --python tmp/ai-upscale/venv/Scripts/python.exe -r tools/ai-upscale/requirements-evaluation.txt --extra-index-url https://download.pytorch.org/whl/cpu
$candidate = Get-Content tools/ai-upscale/candidate.json -Raw | ConvertFrom-Json
foreach ($asset in $candidate.assets) {
    $destination = Join-Path tmp/ai-upscale $asset.file
    Invoke-WebRequest $asset.url -OutFile $destination
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $asset.sha256) {
        throw "Asset integrity failed: $($asset.file)"
    }
}
./tmp/ai-upscale/venv/Scripts/python.exe tools/ai-upscale/reference.py --assets tmp/ai-upscale --output tmp/ai-upscale/reference --tiles
```

Use a new output directory for each run; existing evidence is never overwritten by
the reference/native probes. `reference.json` records graph/hash, FP32 tensor hashes,
output sizes, pre-clamp extrema, tile/edge/seam/order errors, time and CPU process
memory. Assertions enforce max error <=0.01 / MAE <=0.001, including odd dimensions.
The generated pattern is original CC0 test material; see `fixtures/ai-upscale.json`.
FP32 references are generated by CPU PyTorch independently of ONNX inference.
Do not clamp before comparison: observed raw outputs extend below 0 and above 1.

The halo is model-derived: 34 stride-1 padded 3x3 convolutions give radius 34.
Clip tile halos at actual source boundaries so each layer retains its upstream
zero-padding behavior. Globally reflecting or zero-padding the source once is a
different operation. All tensor/image files, full-frame arrays and readbacks are
diagnostics; their memory/copy behavior is not a production inference claim.

## Native ONNX Runtime / DirectML

```powershell
./tmp/ai-upscale/venv/Scripts/python.exe tools/ai-upscale/native_ort.py --reference tmp/ai-upscale/reference --output tmp/ai-upscale/native-ort --adapter-name 'NVIDIA GeForce RTX 3050 Laptop GPU'
```

The probe enumerates actual DXGI adapters, selects one exact hardware name, passes
that ordinal to DirectML, disables CPU fallback and audits provider profiling events.
On this machine NVIDIA is DXGI ordinal 1, not 0. Inputs/outputs explicitly stage
FP32 host data; the opaque runtime/driver's copies and memory are not bounded here.
Releasing the Python session plus process exit is observed cleanup, not a measurement
of every driver allocation. Successful reference inference alone does not prove
preview/export, runtime tile policy, lifecycle or video acceptance.

## Browser ONNX Runtime

```powershell
New-Item -ItemType Directory -Path tmp/ai-upscale/browser -Force | Out-Null
Copy-Item tools/ai-upscale/browser-package.json tmp/ai-upscale/browser/package.json
Copy-Item tools/ai-upscale/browser-package-lock.json tmp/ai-upscale/browser/package-lock.json
npm ci --prefix tmp/ai-upscale/browser
$env:PLAYWRIGHT_BROWSERS_PATH = Join-Path (Get-Location) tmp/ai-upscale/chromium
./tmp/ai-upscale/browser/node_modules/.bin/playwright.ps1 install chromium
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' tmp/ai-upscale/browser-result
```

The current package/lockfiles pin ONNX Runtime Web **1.30.0**, which passes the five
numeric/device reference cases in main and worker on the tested Chromium/Intel
environment. The original `browser-1.23-package*.json` files preserve the failed
1.23.0 evaluation. Keep them separate; do not describe its old failure as the current
runtime result. An existing evaluation tree containing 1.23.0 can explicitly select
the separate 1.30 installation via `AI_UPSCALE_ORT_DIST=<absolute dist directory>`.

Use a built application containing the `initialize_worker` WASM export. The probe
calls the real Rust GpuSession constructor on an OffscreenCanvas in main and worker
contexts; this is not the ordinary HTML-canvas UI/export acceptance test. It tracks
all device requests and checks exact JavaScript GPUDevice identity. The native
WebGPU EP receives `{name: 'webgpu', device}`; setting `env.webgpu.device` alone was
insufficient in the 1.23 build. CPU EP fallback is explicitly disabled through
session configuration. Input uploads and output readbacks are diagnostics only.
After releasing a successful ORT session, a separate four-byte GPU copy/readback
proves that the original processing device remains usable. This is not full lifecycle
or production memory acceptance.

On the evaluated Playwright Chromium 141, default D3D12 startup fails loading
`dxil.dll` (Windows error 87). A separate test permits the FXC path:

```powershell
$env:AI_UPSCALE_BROWSER_ARGS = '["--disable-dawn-features=use_dxc"]'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' tmp/ai-upscale/browser-fxc-result
```

This flag is diagnostic, not an application or user-browser configuration change.
FXC plus 1.30.0 passes the numeric GPU-buffer gate, without exception instrumentation
or runtime patches. The unchanged 1.23.0 graph fails strict session creation because
its PRelu nodes require CPU execution; earlier fallback-enabled observations expose
a GPU download exception as well. Both version-specific results remain recorded.

For operator isolation on 1.23.0, generate only diagnostic subgraphs from the pinned
export, then set `AI_UPSCALE_ISOLATE=1` and point `AI_UPSCALE_ORT_DIST` at that runtime:

```powershell
./tmp/ai-upscale/venv/Scripts/python.exe tools/ai-upscale/isolate_onnx.py tmp/ai-upscale/reference/model.onnx tmp/ai-upscale/isolated-ops-final
$env:AI_UPSCALE_ISOLATE = '1'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' tmp/ai-upscale/operator-result
```

Clear `AI_UPSCALE_ISOLATE` before a full-model run. These Conv/PRelu/DepthToSpace/Resize
graphs use actual original parameters and ONNX ReferenceEvaluator outputs at 1x1
and 17x19. They never substitute for model acceptance. `AI_UPSCALE_DECODE_CPP=1`
optionally instruments only the pinned 1.23 Emscripten module response to inspect
bounded exception fields; normal pilot tests leave it unset and do not patch runtime
files. Logs capture CPU placement, shader validation and failures under ignored storage.

## Burn import and native diagnostic

```powershell
cargo run --locked --manifest-path tools/ai-upscale/burn-import/Cargo.toml --release -- tmp/ai-upscale/reference/model.onnx tmp/ai-upscale/burn-generated
cargo run --locked --manifest-path tools/ai-upscale/burn-native/Cargo.toml --release -- tmp/ai-upscale/reference tmp/ai-upscale/burn-generated/model.bpk 'NVIDIA GeForce RTX 3050 Laptop GPU' tmp/ai-upscale/native-burn.json
```

These isolated crates have their own lockfiles and do not modify the application
workspace dependency graph. The native source includes the generated model at the
documented ignored location. Only generate it from the allowlisted export before
compiling. Importer 0.22.0 / Burn 0.22.1 resolves CubeCL 0.11.2 and wgpu 30.0.1,
matching the processing ABI; Blitz remains a separate wgpu 26 device. Dependency
compatibility and generated Resize/DepthToSpace code are not numerical proof.
The native probe explicitly selects a discrete Vulkan device and verifies its name
and vendor before loading weights; it never registers its textures on Blitz.
Browser Burn was subsequently executed independently and fails the existing device's
256-invocation workgroup contract (its first copy kernel requires 1024). Numerical
Resize/pixel-shuffle behavior is still unproved; neither Burn platform is enabled.

## Browser Burn on the actual Rust processing device

The isolated `burn-browser` crate enables only the diagnostic handle-access feature
in media-web. Default apps retain their original device/limits and do not depend on
Burn. No raw WebGPU handle is cast into a different Rust device or ABI. The generated
model and BPK must be produced by the earlier allowlisted import first.

```powershell
cargo build --locked --manifest-path tools/ai-upscale/burn-browser/Cargo.toml --target wasm32-unknown-unknown --release
<wasm-bindgen-0.2.128> <cargo-target-dir>/wasm32-unknown-unknown/release/ai_upscale_burn_browser_probe.wasm --target web --out-dir tmp/ai-upscale/burn-browser-wasm --out-name probe
$env:AI_UPSCALE_BURN = '1'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' tmp/ai-upscale/browser-burn-result
```

The version-matched wasm-bindgen CLI was acquired from its official GitHub release
into ignored test storage; its archive matches the publisher's SHA-256 checksum
`8fd8e2165da16b21ee3f5efd19e7f97d8d27cb7832f54edaef4b18e830283ec0`.
The reference and BPK hashes remain unchanged. CPU tensor uploads/readbacks in this
harness are diagnostics. Input/output device identity is checked before comparison;
validation errors or panics retire the context and remain failures, never usable
images. The controller preserves worker progress and bounds both contexts to 120 s.
## Integration diagnostics (2026-10-23)

The optional native bounded tile helper, pinned-header build, protocol and actual
reference/lifecycle commands are documented in
[native-onnx-helper/README.md](native-onnx-helper/README.md).
It is a counted CPU-staged DirectML frame adapter, not native video/preview.

`tile-browser` compiles the actual shared Rust tile/inference/video code with
`gpu-setup-diagnostic`. Build release wasm and run matching wasm-bindgen 0.2.128
to `tmp/ai-upscale/tile-browser-wasm` with output name `probe`. Bundle the current
`apps/web/assets/m2.js` to `tmp/ai-upscale/media-probe.mjs` with the existing
esbuild. The harness uses only existing locally cached model/runtime assets.

```powershell
$env:AI_UPSCALE_ORT_DIST = '<existing onnxruntime-web-1.30.0/dist>'
$env:AI_UPSCALE_BROWSER_EXE = '<installed msedge.exe>'
$env:AI_UPSCALE_VIDEO = '1'
$env:AI_UPSCALE_VIDEO_CASES = '[{"fixture":"ai-upscale-vfr.mp4","scale":"2x","fps":"60/1","clahe":true,"color":"10/115/80"}]'
node tools/ai-upscale/browser.mjs tmp/ai-upscale '<built-Dioxus-public-directory>' '<new-ignored-result-directory>'
node tools/ai-upscale/verify_video.mjs '<result-directory>'
```

Use `AI_UPSCALE_TILE=1` instead of VIDEO for tile ownership/budget diagnostics;
`AI_UPSCALE_CODEC=1` for exact codec recommendation probes without inference;
`AI_UPSCALE_PREALLOCATED=1` for raw caller-owned tensor reference probes.
Clear unrelated mode variables between runs. `AI_UPSCALE_VIDEO_RECOVERY=1`
injects actual device destruction between AI source frames then explicitly
retries on the retained canvas. `AI_UPSCALE_VIDEO_PARITY=1` writes separate raw
first-frame ingress/output diagnostic snapshots; `video_pixels.py --assets
tmp/ai-upscale --directory '<result-directory>' --output '<new-result.json>'`
checks those against trusted PyTorch and independent orientation/post-scale/color
math. These readbacks are excluded from fast-path performance. That CPU oracle
does not implement CLAHE; use non-CLAHE cases for it.

`AI_UPSCALE_VIDEO_FFMPEG=1` selects the existing optional FFmpeg WASM encoder,
configured from already installed local worker/core assets with OPFS output.
Use only its supported fixed-FPS H.264/AAC cases. It explicitly reads back output
RGBA into the existing bounded live ring; inference remains upstream on WebGPU.
Clear this variable before WebCodecs/WebM cases. WebM video-only grid checking
uses video origin, since excluded audio cannot extend its presentation grid.

`video_fixture.py --output fixtures` generates the original tiny CC0 VFR and
rotated sources plus manifests. Existing checked-in files need no regeneration.
The independent video verifier derives exact presentation end from integer
packet PTS+duration before applying the existing rational grid oracle; container
stream duration alone can omit a B-frame tail. New result directories preserve
earlier failures. No diagnostic result enables ordinary AI conversion or UI.

## Native release quality and performance comparison

`native_quality.py` runs the existing64-frame licensed clips and CC0 cut fixture
through native RTX High3, DirectML Real-ESRGAN and a diagnostic shared-wgpu
bilinear comparator. It hashes existing source clips, uses the same explicit
NVIDIA Vulkan adapter, software codecs, source-aware bitrate, CLAHE50 and manual
brightness10/contrast115/saturation80. This evaluated-machine script selects
RTX3050Laptop/DXGI1 and requires its verified current LUID. No GPU fallback,
asset acquisition or installation is attempted. RTX and Real-ESRGAN are different
models; this is not matching-weight runtime equivalence.

Build the explicit release CLI first, **before starting any measurements**:

```powershell
cargo build --locked --release -p media-native --features ai-upscale-pilot --bin native-convert
```

Run with explicit existing assets and a fresh output directory:

```powershell
tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/native_quality.py --cli '<release-native-convert.exe>' --rtx-helper '<normal-rtx-helper.exe>' --onnx-helper '<native-onnx-helper.exe>' --sdk '<assessed-sdk-root>' --ngx-cache '<existing-ngx-cache-parent>' --runtime '<assessed-ort-directory>' --model-cache '<validated-model-cache>' --fixtures tmp/ai-upscale/quality-temporal-20261025 --output '<fresh-ignored-output-directory>' --luid-high '<current-high>' --luid-low '<current-low>'
```

Default output is72 files:3 engines x4 sources x2 scales x3 FPS policies.
`--fps original` selects the24-file Original-only subset. Run the timing subset
without concurrent compilation, tests or verification. Per-case commands,
reports, wall times and asset hashes preserve exactly what ran. Scope/driver
caches/background activity still limit these single-run measurements. The CLI
`bilinear-video-pilot` entry point is feature gated, requires AI Off, Original
resize and explicit Shared GPU software codecs, and is bounded to192x108 input.
It is never a product resize/transport option; ordinary Off rejects enlargement.

After measurement, independently verify each engine directory:

```powershell
$env:AI_UPSCALE_NATIVE_VIDEO='1'
node tools/ai-upscale/verify_video.mjs '<output>/bilinear'
node tools/ai-upscale/verify_video.mjs '<output>/rtx'
node tools/ai-upscale/verify_video.mjs '<output>/onnx'
tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/native_quality_inspect.py --directory '<output>' --output '<fresh-inspection-directory>'
```

Inspection requires all three engine matrices in identical case order. It
produces bounded chronological contact sheets, selected-resolution triptychs
and all consecutive encoded-frame RGB differences for Original FPS. No motion
compensation, high-resolution ground truth, perceptual-improvement or intrinsic
flicker threshold is asserted. Retain attribution/license/changes from the
fixture manifest. These diagnostics persist encoded videos/inspection images,
not raw-frame sequences. They neither enable controls nor inspect a native UI.

`gpu_wall_times` contains CPU wall intervals, not pure kernel or codec timings:
processing-device setup, helper startup, decode pipe waits, inference/IO,
postprocessing plus encoder pipe writes, codec drain and helper shutdown.
Conversion/preflight/verification/total remain in the existing fields. Pipeline
stages overlap and not every setup/header/mux operation is partitioned. RTX's
final helper DXGI sampled peak is exposed after successful clean shutdown;
sampling can miss transient peaks and excludes other process/device allocations.
Transfer byte counts remain distinct from elapsed intervals.

The isolated Oct30 comparison finds about3s RTX startup versus0.164–0.335s for
64-frame inference/IO on these tiny inputs. Total RTX exports take5.18–6.19s,
DirectML5.10–5.66s and bilinear1.81–2.07s. No total RTX speedup or real-time
preview is claimed. Fresh SDK startup per seek is still an optimization target,
not an implemented fix. The current native source cap remains192x108.

## Native window preview diagnostic

### Ordinary native local configuration and job handoff

The ordinary AI panel can now **Import local AI configuration** or clear it.
Import selects existing local assets and an exact processing GPU; it loads no
inference runtime, accepts no SDK terms and grants no release capability. A
configuration change returns AI to Off and retires its preview cache. Asset
changes also clear the configuration; checking a model hash does not change it.
The file is bounded to64KiB, rejects unknown fields/backends, and requires
absolute paths and a matching actual Vulkan adapter. It contains no credentials.

Copy [the RTX example](native-rtx-configuration.example.json) outside tracked
source and replace every placeholder with your existing normal helper, assessed
SDK root and existing cache parent. Use `native-convert list-gpus` for the exact
processing adapter key. Replace the example DXGI ordinal and both LUID parts
with the independently verified current NVIDIA identity. Zero LUID is a
placeholder, not an accepted device. Do not infer a DXGI ordinal from a Vulkan
list index. SDK/runtime execution retains its identity checks and never falls
back. The example itself does not provision assets or accept NVIDIA terms.

For ONNX, use the same root fields with `helper.backend` set to `onnx` and
`helper.configuration` containing `executable`, `runtime_directory`,
`model_cache`, `dxgi_index` and `exact_adapter_name`. Those must reference the
existing validated helper/runtime/model cache. The ordinary app retains a
separate exact per-scale capability gate; importing either configuration still
leaves native2x/4x unavailable pending ordinary-window and release acceptance.
RTX configuration changes the native panel's backend/model description; it does
not offer RTX in the browser. No configuration can supply acceptance metadata.

The ordinary job entry point freezes that configuration with settings and exact
acceptance metadata, checks the explicitly selected processing GPU, and applies
source/output limits to the actual inspected input before output creation. It
uses the same assets for preview and export. The app requires the explicit
Shared GPU software-codec route for AI export; it never changes routes. AI Off
continues through the existing entry point without touching configured assets.

The opt-in `actual_ai_conversion_handoff_retires_and_retries` test now exercises
this entry point with **test-only acceptance metadata**, for either
`DIAXUS_TEST_PREVIEW_BACKEND=rtx` or `onnx` and the existing asset variables.
It retires active preview/cache ownership before conversion, rejects stale
requests and pre-cancelled outputs, exports both scales, and restores identical
paused preview pixels. This is service/renderer integration, not an ordinary
window interaction test. Independently verify its two files with:

```powershell
tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/verify_native_handoff.py '<new-handoff-evidence-directory>'
```

The adapter writes fresh `native.json` and invokes the existing full FFprobe/
FFmpeg timing/audio verifier. It refuses to overwrite evidence. Native UI
inspection, SDK consent/provisioning UX and representative release acceptance
remain pending; native controls are not enabled by these tests.

The ordinary native renderer can be exercised explicitly with
`converter-native --ai-preview-check`. This opt-in diagnostic uses only
`fixtures/ai-upscale-cut.mp4`, does not grant a product capability, and disables
export in both its button and command handler. Restart without the flag for the
ordinary app. Product 2x/4x controls remain unavailable.

For ONNX (the default `DIAXUS_AI_PREVIEW_BACKEND=onnx`), set
`DIAXUS_AI_HELPER`, `DIAXUS_ORT_DIRECTORY` and `DIAXUS_AI_MODEL_CACHE` to
existing evaluated local assets. Also explicitly set `DIAXUS_AI_ADAPTER` to the
exact adapter name, `DIAXUS_AI_DXGI_INDEX` to its verified DXGI ordinal, and
`DIAXUS_AI_PREVIEW_SCALE` to `2` or `4`. There are no default local asset paths,
GPU substitutions, installations or downloads. The processing Vulkan adapter
must match the helper's actual provider identity. The renderer keeps its own
device/ABI and receives a counted bounded display image.

For the locally authorized RTX SDK evaluation instead, set
`DIAXUS_AI_PREVIEW_BACKEND=rtx`, `DIAXUS_RTX_HELPER` to the built normal helper,
`DIAXUS_RTX_SDK` to the evaluated SDK root and `DIAXUS_RTX_CACHE` to an existing
cache parent. Set `DIAXUS_RTX_LUID_HIGH`/`DIAXUS_RTX_LUID_LOW` to the verified
current NVIDIA adapter LUID, and the same explicit `DIAXUS_AI_ADAPTER`,
`DIAXUS_AI_DXGI_INDEX` and `DIAXUS_AI_PREVIEW_SCALE` variables above. LUIDs can
change across system sessions; do not copy an old value as device discovery.
Only the assessed SDK1.1 SDR High3 configuration and inputs <=192x108 are
evaluated. Invalid backend/configuration never falls back to ONNX. HDR and
Main10 reject before normalization. This selects the diagnostic preview;
ordinary native 2x/4x controls remain unavailable.

Set `DIAXUS_TEST_PREVIEW_BACKEND=rtx` for the two ignored service/renderer tests
below to use these explicit RTX assets; unset it or use `onnx` for DirectML.
Run native test binaries serially on Windows: rebuilding a running test
executable can fail with linker permission denied. Fresh seeks currently create
a fresh helper/SDK session; cached downstream edits avoid inference, but this
does not establish responsive seeking or real-time playback.

Open Color adjustments to exercise Before/After, Strength, manual edits and
Reset color. Seek and Play/Pause use the existing player. Successful presentation
prints transfer metadata to stderr. Before with a valid enhanced cache preserves
it and its finite CLAHE statistics; cold Before decodes the selected original
without loading a model/helper. Display fitting happens after selected-size
processing and never enlarges model input. This diagnostic is not ordinary-launch
acceptance, synchronized real-time playback, a conversion-handoff test, or a
quality/performance result on representative video.

Run the actual `media-native` `actual_ai_preview` and `converter-native`
`actual_ai_renderer` ignored tests explicitly with those existing asset variables
to produce new service/renderer evidence directories. Compare their raw diagnostic
rasters with:

```powershell
tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/native_preview_pixels.py --directory '<new-service-directory>' --worker-directory '<new-renderer-directory>' --output '<new-report.json>'
```

The NumPy oracle independently resizes native chronological pre-encoder pixels
and normalized original-source pixels. It checks enhanced preview, warm/cold
Before, opaque alpha and the unchanged 2-LSB limit. Diagnostic raw files belong
only under ignored test storage.
