param(
    [Parameter(Mandatory)][string]$SdkRoot,
    [Parameter(Mandatory)][string]$Output,
    [string]$Compiler = 'cl.exe'
)
$ErrorActionPreference = 'Stop'
# Run from an x64 Visual Studio developer shell. No installation or execution.
$sdkDirectory = (Resolve-Path -LiteralPath $SdkRoot).Path
$compilerCommand = Get-Command $Compiler -ErrorAction Stop
$outputFile = [System.IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $outputFile) { throw 'Preserve prior evidence: output executable already exists.' }
if (-not (Test-Path -LiteralPath ([System.IO.Path]::GetDirectoryName($outputFile)) -PathType Container)) {
    throw 'Create the explicit build output directory first.'
}
$library = Join-Path $sdkDirectory 'lib/Windows/x64/nvsdk_ngx_s.lib'
$headers = Join-Path $sdkDirectory 'include'
foreach ($path in @($library, (Join-Path $headers 'nvsdk_ngx.h'), (Join-Path $headers 'nvsdk_ngx_helpers_vsr.h'))) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required SDK input is absent: $path" }
}
$objectFile = [System.IO.Path]::ChangeExtension($outputFile, '.obj')
if (Test-Path -LiteralPath $objectFile) { throw 'Output object already exists.' }
$arguments = @('/nologo', '/std:c++20', '/EHsc', '/W4', '/MT', '/O2',
    "/I$headers", (Join-Path $PSScriptRoot 'main.cpp'), "/Fo$objectFile", "/Fe$outputFile",
    '/link', $library, 'd3d11.lib', 'dxgi.lib', 'dxguid.lib', 'version.lib',
    'shlwapi.lib', 'wintrust.lib', 'crypt32.lib', 'advapi32.lib', 'shell32.lib', 'ole32.lib', 'user32.lib')
& $compilerCommand.Source @arguments
if ($LASTEXITCODE -ne 0) { throw "RTX probe compilation failed: $LASTEXITCODE" }
Get-FileHash -LiteralPath $outputFile -Algorithm SHA256
