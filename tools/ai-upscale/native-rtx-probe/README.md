# Native RTX Video SDK feasibility probe

Source-only local diagnostic for task 1.5. **Actual SDK feasibility passes on the evaluated device; product integration remains pending.**
No proprietary headers, libraries, DLLs or models are copied into this directory.
SDK terms must be accepted separately. On 2026-10-29 the user authorized local
SDK compilation/evaluation, excluding redistribution and automatic prerequisite
installation. No native product capability follows from this diagnostic result.

The first route is D3D11, avoiding CUDA/Vulkan/MFC/ATL for this standalone probe.
The SDK's documented C++ build prerequisites still apply. Use an x64 developer
shell with compatible MSVC headers/libraries and the installed Windows SDK.
The user installed MSVC tools **14.51.36231** (compiler **19.51.36247**), rather
than the initially suggested v143. The standalone source actually compiles, links
and runs with this toolset and Windows SDK 10.0.26100.0. No additional toolchain,
CUDA, Vulkan SDK, MFC or ATL was installed by the assistant. The earlier LLVM
missing-`excpt.h` failure is preserved as historical evidence.

```powershell
./tools/ai-upscale/native-rtx-probe/build.ps1 -SdkRoot 'D:/Libraries/RTX_Video_SDK_v1.1.0' -Output '<new executable in ignored tmp>'
& '<probe.exe>' 'D:/Libraries/RTX_Video_SDK_v1.1.0' 1 'NVIDIA GeForce RTX 3050 Laptop GPU' '<new output directory>'
```

The DXGI ordinal is explicit and is checked against exact NVIDIA adapter name,
vendor, device and recorded LUID. Never assume ordinal 1 identifies NVIDIA on
another run or machine. A mismatch rejects before NGX initialization. No GPU,
CPU, model, precision or SDK API fallback is implemented.

The test uses opaque synthetic RGBA8 SDR 192x108 inputs with two distinct
patterns at 2x/4x. It compares quality High with the SDK's own Bicubic setting,
requires nonblack output, differing enhanced/bicubic and differing pattern
responses, writes bounded raw outputs under ignored storage, and reports exact
NGX hexadecimal statuses for initialization/capabilities/create/evaluate/cleanup.
These checks alone are not fidelity, SDK-demo equivalence, video timing,
preview/export or product acceptance. Unsupported sizes/quality are failures,
not a reason to silently substitute another model or relax a threshold.

D3D11 input upload and diagnostic readbacks are explicit: four source uploads,
four input readbacks and eight output readbacks on a complete run. Input
readbacks verify exact RGBA8 ingress. Mapping output staging completes GPU work
before read/use. Features, app-owned capability parameters and per-device NGX
state are explicitly retired, with a destructor safety net. This is not a
zero-copy codec path. Known-input media has no external source/license dependency.

Two complete evaluator suites pass: three successful fresh-process runs per
suite plus wrong-device, missing-runtime and actual SDK missing-resource errors.
All eight outputs repeat bit-for-bit within each suite, including a fresh run
after the SDK failure. Source ingress is exact RGBA8 and output alpha is 255.
The actual SDK VSR DLL is loaded from the explicit SDK root; the driver NGX
loader and TrueHDR capability DLL are also inventoried, without creating an HDR
feature. These results do not establish other devices/drivers or video support.
No installer is invoked by either file. SDK cache/log output belongs to each
fresh ignored run directory, and no output is registered as a converted video.


## Independent verification

```powershell
& tmp/ai-upscale/venv/Scripts/python.exe -X utf8 tools/ai-upscale/native-rtx-probe/evaluate.py --probe '<built-probe.exe>' --sdk 'D:/Libraries/RTX_Video_SDK_v1.1.0' --dxgi-index 1 --negative-adapter-index 0 --adapter-name 'NVIDIA GeForce RTX 3050 Laptop GPU' --output '<new ignored evaluation directory>'
```

Uses the already installed evaluation NumPy; installs/downloads nothing. The
negative adapter is explicit too: ordinal0 was Intel on the measured machine.
The runner validates exact dimensions, byte counts, opaque alpha, raw RGB sums,
SDK-Bicubic/High differences and repeated SHA-256 output identity. It preserves
commands, exit codes, raw status streams, runtime hashes and measured process
wall time. Wall time is not GPU inference or conversion throughput.

`--missing-input-test` on the executable deliberately sends null resources to
an initialized feature, which returns **0xbad00002**, the SDK platform error.
The probe reports that exact code and still releases the feature, destroys its
app-owned capability map and shuts down the selected device, all success codes.
Do not label this an unsupported GPU or assume every invalid argument has that
code. Wrong-device and absent runtime paths reject before initialization.

An initial strict repeat found a small SDK Bicubic variation (max29 LSB across
56 pixels), while High outputs were identical. The output is now explicitly
GPU-cleared to opaque black before every evaluation, ensuring defined initial
contents. Both complete strict-repeat suites then passed without relaxing the
checks. The first failure remains preserved; no universal driver-determinism or
causal explanation is asserted. A version-zero initialization attempt was
accepted by the SDK, so that was not a valid failure proof and was removed.

The passing probe uses no private DLL exports, ABI workaround or raw SDK-loader
replacement. An initial missing `user32.lib` link was fixed in the build script.
Two warnings originate from unused parameters in installed SDK helper headers.
SDK sample equivalence, bounded per-frame protocol, real video, cancellation,
color composition and ordinary native UI belong to the next tasks.
