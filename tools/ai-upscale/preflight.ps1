# Read-only inventory. This does not load SDK DLLs, install packages, or prove inference support.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$SdkPath,
    [string]$PythonCommand = 'python'
)

$ErrorActionPreference = 'Stop'
$sdk = (Resolve-Path -LiteralPath $SdkPath).Path
$packageFiles = @(
    'samples/ReadMe.md',
    'samples/RTX_Video_API/rtx_video_api.h',
    'samples/RTX_Video_API/RTX_Video_API.vcxproj',
    'NVIDIA_RTX_Video_SDK_License.pdf',
    'bin/Windows/x64/rel/nvngx_vsr.dll'
)
$inventory = foreach ($relative in $packageFiles) {
    $path = Join-Path $sdk $relative
    $present = Test-Path -LiteralPath $path -PathType Leaf
    [ordered]@{
        relativePath = $relative
        present = $present
        bytes = $(if ($present) { (Get-Item -LiteralPath $path).Length } else { $null })
        sha256 = $(if ($present) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null })
    }
}

$python = Get-Command $PythonCommand -ErrorAction SilentlyContinue
$pythonInfo = $null
if ($python) {
    $probe = 'import importlib.util,json,sys;print(json.dumps({"version":sys.version,"executable":sys.executable,"modules":{m:bool(importlib.util.find_spec(m)) for m in ["torch","numpy","onnx","onnxruntime"]}}))'
    $raw = & $python.Source -c $probe
    if ($LASTEXITCODE -ne 0) { throw 'Python prerequisite probe failed' }
    $pythonInfo = $raw | ConvertFrom-Json
}

$nvidia = Get-Command nvidia-smi -ErrorAction SilentlyContinue
$gpu = @()
if ($nvidia) {
    $raw = & $nvidia.Source --query-gpu=name,uuid,pci.bus_id,driver_version,memory.total --format=csv,noheader,nounits
    if ($LASTEXITCODE -ne 0) { throw 'nvidia-smi inventory failed' }
    $gpu = @($raw | ConvertFrom-Csv -Header name,uuid,pciBusId,driverVersion,memoryMiB)
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vsCpp = @()
if (Test-Path -LiteralPath $vswhere) {
    $raw = & $vswhere -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json
    if ($LASTEXITCODE -ne 0) { throw 'Visual Studio component inventory failed' }
    $vsCpp = @($raw | ConvertFrom-Json | ForEach-Object { $_.installationPath })
}
$nvcc = Get-Command nvcc -ErrorAction SilentlyContinue
$cudaRoot = Join-Path $env:ProgramFiles 'NVIDIA GPU Computing Toolkit/CUDA'
$cudaCompilers = @()
if (Test-Path -LiteralPath $cudaRoot) {
    $cudaCompilers = @(Get-ChildItem -LiteralPath $cudaRoot -Directory | ForEach-Object {
        $candidate = Join-Path $_.FullName 'bin/nvcc.exe'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { $candidate }
    })
}
$os = Get-CimInstance Win32_OperatingSystem
[ordered]@{
    schemaVersion = 1
    observedAtUtc = [DateTime]::UtcNow.ToString('o')
    scope = 'Prerequisites only; no backend initialization or inference attempted'
    os = [ordered]@{ caption = $os.Caption; version = $os.Version; build = $os.BuildNumber; architecture = $os.OSArchitecture }
    sdkPath = $sdk
    sdkFiles = @($inventory)
    python = $pythonInfo
    nvidiaDevices = $gpu
    visualStudioWithCppComponent = $vsCpp
    nvccOnPath = $(if ($nvcc) { $nvcc.Source } else { $null })
    cudaCompilersInDefaultRoot = $cudaCompilers
    vulkanSdkEnvironmentPath = $env:VULKAN_SDK
    runtimeResult = 'untested'
} | ConvertTo-Json -Depth 8
