param(
    [Parameter(Mandatory)][string]$Compiler,
    [Parameter(Mandatory)][string]$Headers,
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'
# Explicit local inputs only. This script never installs or downloads anything.
$pins = @{
    'onnxruntime_c_api.h' = '71125e66180a991d65c9bdbad4aa20daaa1f7a48c7a5c0fa5f18f250ac839a02'
    'onnxruntime_ep_c_api.h' = 'b92778f50c36ecdf53d0344e0129a78b306afed653f81bba8a9597e3e6e9546f'
}
foreach ($name in $pins.Keys) {
    $path = Join-Path $Headers $name
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $pins[$name]) {
        throw "Unverified official ONNX Runtime 1.23.0 header: $name"
    }
}
& $Compiler '-std=c++20' '-O2' '-Wall' '-Wextra' '-Werror' '-municode' '-static-libgcc' '-static-libstdc++' '-D_WIN32_WINNT=0x0A00' '-I' $Headers (Join-Path $PSScriptRoot 'main.cpp') '-o' $Output '-ldxgi' '-lbcrypt' '-lole32'
if ($LASTEXITCODE -ne 0) { throw "Helper compilation failed: $LASTEXITCODE" }
