param(
    [Parameter(Mandatory)][string]$SdkRoot,
    [Parameter(Mandatory)][string]$Output,
    [switch]$SampleReference,
    [switch]$Diagnostic,
    [string]$Compiler = 'cl.exe'
)
$ErrorActionPreference = 'Stop'
$sdkDirectory = (Resolve-Path -LiteralPath $SdkRoot).Path
$outputFile = [System.IO.Path]::GetFullPath($Output)
$outputDirectory = [System.IO.Path]::GetDirectoryName($outputFile)
if (-not (Test-Path -LiteralPath $outputDirectory -PathType Container)) { throw 'Create the output directory first.' }
$objectFile = [System.IO.Path]::ChangeExtension($outputFile, '.obj')
foreach ($path in @($outputFile, $objectFile)) {
    if (Test-Path -LiteralPath $path) { throw "Preserve existing build output: $path" }
}
$library = Join-Path $sdkDirectory 'lib/Windows/x64/nvsdk_ngx_s.lib'
$arguments = @('/nologo', '/std:c++20', '/EHsc', '/W4', '/MT', '/O2', '/DNDEBUG',
    "/I$(Join-Path $sdkDirectory 'include')", (Join-Path $PSScriptRoot 'main.cpp'),
    "/Fo$objectFile", "/Fe$outputFile")
if ($Diagnostic) {
    if ($SampleReference) { throw 'Fault controls are only for the diagnostic helper, not the SDK reference.' }
    $arguments += '/DRTX_DIAGNOSTIC'
}
if ($SampleReference) {
    # Evaluation only: compile the external, unmodified NVIDIA sample source.
    # Keep its separate object and all proprietary runtime files outside Git.
    $sample = Join-Path $sdkDirectory 'samples/RTX_Video_API'
    $sampleObject = Join-Path $outputDirectory 'sample-reference.obj'
    if (Test-Path -LiteralPath $sampleObject) { throw 'Sample object already exists.' }
    & $Compiler /nologo /std:c++20 /EHsc /MT /O2 /DNDEBUG "/I$(Join-Path $sdkDirectory 'include')" /c (Join-Path $sample 'rtx_video_api_dx11_impl.cpp') "/Fo$sampleObject"
    if ($LASTEXITCODE -ne 0) { throw 'SDK sample compilation failed.' }
    $arguments += @('/DRTX_SAMPLE_REFERENCE', "/I$sample", $sampleObject)
}
$arguments += @('/link', $library, 'd3d11.lib', 'dxgi.lib', 'dxguid.lib', 'version.lib',
    'shlwapi.lib', 'wintrust.lib', 'crypt32.lib', 'advapi32.lib', 'shell32.lib', 'ole32.lib', 'user32.lib')
& $Compiler @arguments
if ($LASTEXITCODE -ne 0) { throw "RTX helper compilation failed: $LASTEXITCODE" }
Get-FileHash -LiteralPath $outputFile -Algorithm SHA256
