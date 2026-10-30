param(
    [Parameter(Mandatory)][string]$Configuration,
    [Parameter(Mandatory)][string]$Application
)
# Developer/local deployment only. Installs configuration metadata, no SDK,
# model, executable, licence acceptance, GPU preference or test-result receipt.
$ErrorActionPreference = 'Stop'
$configurationPath = (Resolve-Path -LiteralPath $Configuration).Path
$applicationPath = (Resolve-Path -LiteralPath $Application).Path
if ([IO.Path]::GetExtension($applicationPath) -ne '.exe') { throw 'Select the built native application executable.' }
if ((Get-Item -LiteralPath $configurationPath).Length -gt 65536) { throw 'Configuration exceeds 64 KiB.' }
$config = Get-Content -LiteralPath $configurationPath -Raw | ConvertFrom-Json
if ($config.helper.backend -ne 'rtx' -or !$config.processing_adapter) { throw 'An explicit RTX configuration is required.' }
$assetDirectory = Join-Path ([IO.Path]::GetDirectoryName($applicationPath)) 'assets'
$destination = Join-Path $assetDirectory 'native-rtx.json'
if (Test-Path -LiteralPath $destination) { throw "Preserve existing deployment asset: $destination" }
New-Item -ItemType Directory -Path $assetDirectory -Force | Out-Null
Copy-Item -LiteralPath $configurationPath -Destination $destination
Write-Output "Deployed app-owned RTX setup metadata: $destination"
