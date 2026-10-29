param(
    [ValidateRange(1024, 65535)]
    [int]$Port = 8084
)

$ErrorActionPreference = 'Stop'
$edgeExecutable = Join-Path ${env:ProgramFiles(x86)} 'Microsoft/Edge/Application/msedge.exe'
if (-not (Test-Path -LiteralPath $edgeExecutable -PathType Leaf)) {
    throw 'Microsoft Edge was not found in its standard Windows installation folder.'
}
$edgeVersion = (Get-Item -LiteralPath $edgeExecutable).VersionInfo.ProductVersion
if ($edgeVersion -notmatch '^156\.') {
    throw "Edge $edgeVersion has no completed AI acceptance record. This launcher requires evaluated Edge 156."
}
$repositoryDirectory = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$profileDirectory = Join-Path $repositoryDirectory 'tmp/ai-upscale/edge-nvidia-profile'
$url = "http://127.0.0.1:$Port/"
$response = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 5
if ($response.Headers['Cross-Origin-Opener-Policy'] -ne 'same-origin' -or
    $response.Headers['Cross-Origin-Embedder-Policy'] -ne 'require-corp') {
    throw 'The local app must be served with --cross-origin-policy for AI.'
}
# A dedicated profile starts a separate browser process. Passing this GPU option
# to an already running default-profile Edge process would not change its GPU.
# Asset import/acquisition remains explicit in this separate browser profile.
$browserArguments = @(
    "--user-data-dir=`"$profileDirectory`"",
    '--force-high-performance-gpu',
    '--use-angle=d3d11',
    '--no-first-run',
    '--no-default-browser-check',
    $url
)
Start-Process -FilePath $edgeExecutable -ArgumentList $browserArguments
Write-Output "Opened $url in a dedicated Edge profile. Load a source and verify Selected GPU says nvidia / ampere."
Write-Output 'Model/runtime setup is separate from your normal browser profile. No assets are downloaded by this launcher.'
