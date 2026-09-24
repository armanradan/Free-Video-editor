$ErrorActionPreference = 'Stop'

$repository = Split-Path -Parent $PSScriptRoot
$fixtureDirectory = Join-Path $repository 'fixtures'
$output = Join-Path $fixtureDirectory 'm36-h265-aac.mp4'
New-Item -ItemType Directory -Force -Path $fixtureDirectory | Out-Null

$arguments = @(
    '-hide_banner', '-loglevel', 'error', '-y',
    '-f', 'lavfi', '-i', 'testsrc2=size=320x180:rate=30:duration=1',
    '-f', 'lavfi', '-i', 'sine=frequency=740:sample_rate=48000:duration=1',
    '-map', '0:v:0', '-map', '1:a:0',
    '-c:v', 'libx265', '-preset', 'slow', '-crf', '22', '-pix_fmt', 'yuv420p',
    '-tag:v', 'hvc1', '-g', '30', '-keyint_min', '30', '-sc_threshold', '0',
    '-x265-params', 'pools=1:frame-threads=1:wpp=0:keyint=30:min-keyint=30:scenecut=0:colorprim=bt709:transfer=bt709:colormatrix=bt709',
    '-c:a', 'aac', '-b:a', '96k', '-movflags', '+faststart',
    '-metadata', 'title=Diaxus deterministic HEVC fixture',
    $output
)
& ffmpeg @arguments
if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed with exit code $LASTEXITCODE" }

$file = Get-Item -LiteralPath $output
$manifest = [ordered]@{
    schema = 1
    name = $file.Name
    purpose = 'M3.6 MP4/H.265 input decode and capability-gated H.265 output interoperability'
    license = 'CC0-1.0'
    generated_utc = '2026-09-24'
    generator = (& ffmpeg -version | Select-Object -First 1)
    command = 'pwsh -File tools/generate-hevc-fixture.ps1'
    expected = [ordered]@{
        container = 'MP4'
        video_codec = 'HEVC Main profile (hvc1)'
        width = 320
        height = 180
        frame_rate = '30/1'
        frame_count = 30
        color = 'BT.709 limited-range SDR'
        audio = 'AAC LC, mono, 48000 Hz, 740 Hz tone'
        duration_seconds = 1.0
    }
    size_bytes = $file.Length
    sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $output).Hash.ToLowerInvariant()
}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $fixtureDirectory 'm36-h265-aac.json') -Encoding utf8NoBOM
Write-Output "Generated $($file.Name) and manifest."
