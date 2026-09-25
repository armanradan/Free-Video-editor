$ErrorActionPreference = 'Stop'

$repository = Split-Path -Parent $PSScriptRoot
$fixtureDirectory = Join-Path $repository 'fixtures'
$output = Join-Path $fixtureDirectory 'm4-10bit-sdr.mp4'
New-Item -ItemType Directory -Force -Path $fixtureDirectory | Out-Null

$video = "nullsrc=s=160x96:r=24,format=yuv420p10le,geq=lum='64+mod(X*7+Y*3+N*11,877)':cb='200+mod(X*5+Y*2+N*3,601)':cr='200+mod(X*3+Y*4+N*5,601)'"
$arguments = @(
    '-hide_banner', '-loglevel', 'error', '-y',
    '-f', 'lavfi', '-i', $video,
    '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=1',
    '-t', '1', '-map', '0:v:0', '-map', '1:a:0',
    '-c:v', 'libx265', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p10le',
    '-x265-params', 'lossless=1:pools=1:frame-threads=1:wpp=0:log-level=error:colorprim=bt709:transfer=bt709:colormatrix=bt709:range=limited',
    '-tag:v', 'hvc1', '-color_range', 'tv', '-colorspace', 'bt709',
    '-color_trc', 'bt709', '-color_primaries', 'bt709',
    '-c:a', 'aac', '-b:a', '96k', '-ar', '48000', '-movflags', '+faststart',
    '-metadata', 'title=Diaxus deterministic 10-bit SDR fixture',
    $output
)
& ffmpeg @arguments
if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed with exit code $LASTEXITCODE" }

$file = Get-Item -LiteralPath $output
$manifest = [ordered]@{
    schema = 1
    name = $file.Name
    purpose = 'M4 native HEVC Main 10 BT.709 SDR input/output precision and audio interoperability'
    license = 'CC0-1.0'
    generated_utc = '2026-09-25'
    generator = (& ffmpeg -version | Select-Object -First 1)
    command = 'pwsh -File tools/generate-m4-10bit-fixture.ps1'
    expected = [ordered]@{
        container = 'MP4'
        video_codec = 'HEVC Main 10 (hvc1)'
        pixel_format = 'yuv420p10le'
        width = 160
        height = 96
        frame_rate = '24/1'
        frame_count = 24
        color = 'BT.709 limited-range SDR'
        luma_chroma = 'Deterministic 10-bit gradients with samples not divisible by four'
        audio = 'AAC LC, mono, 48000 Hz, 440 Hz tone'
        duration_seconds = 1.0
    }
    size_bytes = $file.Length
    sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $output).Hash.ToLowerInvariant()
}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $fixtureDirectory 'm4-10bit-sdr.json') -Encoding utf8NoBOM
Write-Output "Generated $($file.Name) and manifest."
