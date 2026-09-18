$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$fixtureDirectory = Join-Path $repository 'fixtures'
$output = Join-Path $fixtureDirectory 'm1-vp8.ivf'

New-Item -ItemType Directory -Force -Path $fixtureDirectory | Out-Null
$filter = "drawbox=x=0:y=0:w=28:h=28:color=red:t=fill,drawbox=x=292:y=0:w=28:h=28:color=green:t=fill,drawbox=x=0:y=152:w=28:h=28:color=blue:t=fill,drawbox=x=260:y=152:w=60:h=28:color=yellow:t=fill,drawtext=fontfile='C\:/Windows/Fonts/consola.ttf':text='%{n}':x=130:y=58:fontsize=54:fontcolor=white"

& ffmpeg -hide_banner -loglevel error -y -f lavfi -i 'color=c=#202020:s=320x180:r=30:d=1' -vf $filter -frames:v 30 -an -c:v libvpx -threads 1 -deadline best -cpu-used 0 -lag-in-frames 0 -auto-alt-ref 0 -g 30 -keyint_min 30 -b:v 500k -minrate 500k -maxrate 500k -bufsize 1000k -pix_fmt yuv420p -f ivf $output
if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed with exit code $LASTEXITCODE" }

$bytes = [IO.File]::ReadAllBytes($output)
$headerSize = [BitConverter]::ToUInt16($bytes, 6)
$packetOffset = [int]$headerSize
$packets = @()
$index = 0
while ($packetOffset -lt $bytes.Length) {
    $size = [BitConverter]::ToUInt32($bytes, $packetOffset)
    $timestamp = [BitConverter]::ToUInt64($bytes, $packetOffset + 4)
    $dataOffset = $packetOffset + 12
    $packets += [ordered]@{ index = $index; offset = $dataOffset; size = $size; timestamp_ticks = $timestamp; timestamp_us = $index * 33333; duration_us = 33333; keyframe = ($index -eq 0) }
    $packetOffset = $dataOffset + $size
    $index++
}
$ffmpegVersion = (& ffmpeg -version | Select-Object -First 1)
$manifest = [ordered]@{
    schema = 1
    license = 'CC0-1.0'
    generator = $ffmpegVersion
    codec = 'vp8'
    coded_width = 320
    coded_height = 180
    time_base = [ordered]@{ numerator = 1; denominator = 30 }
    frame_count = $packets.Count
    frame_duration_us = 33333
    sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $output).Hash.ToLowerInvariant()
    packets = $packets
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $fixtureDirectory 'm1-vp8.json') -Encoding utf8NoBOM
