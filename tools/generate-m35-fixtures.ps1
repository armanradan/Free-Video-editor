$ErrorActionPreference = 'Stop'

$repository = Split-Path -Parent $PSScriptRoot
$fixtureDirectory = Join-Path $repository 'fixtures'
$workDirectory = Join-Path $repository 'tmp/m35-fixture-generation'
New-Item -ItemType Directory -Force -Path $fixtureDirectory, $workDirectory | Out-Null

function Invoke-Ffmpeg {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
    & ffmpeg -hide_banner -loglevel error -y @Arguments
    if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed with exit code $LASTEXITCODE" }
}

function Write-Manifest {
    param([string]$Name, [hashtable]$Details)
    $path = Join-Path $fixtureDirectory $Name
    $file = Get-Item -LiteralPath $path
    $manifest = [ordered]@{
        schema = 1
        name = $Name
        purpose = $Details.purpose
        license = 'CC0-1.0'
        generated_utc = '2026-09-20'
        generator = (& ffmpeg -version | Select-Object -First 1)
        command = $Details.command
        expected = $Details.expected
        size_bytes = $file.Length
        sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
    }
    $jsonName = [IO.Path]::ChangeExtension($Name, '.json')
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $fixtureDirectory $jsonName) -Encoding utf8NoBOM
}

$geometryBase = Join-Path $workDirectory 'm35-geometry-base.mp4'
$geometryOutput = Join-Path $fixtureDirectory 'm35-geometry-color.mp4'
$geometryFilter = "drawbox=x=0:y=0:w=54:h=48:color=red:t=fill,drawbox=x=266:y=0:w=54:h=48:color=green:t=fill,drawbox=x=0:y=144:w=54:h=48:color=blue:t=fill,drawbox=x=266:y=144:w=54:h=48:color=yellow:t=fill,setsar=4/3"
Invoke-Ffmpeg @('-f', 'lavfi', '-i', 'testsrc2=size=320x192:rate=24:duration=1', '-f', 'lavfi', '-i', 'sine=frequency=523:sample_rate=48000:duration=1', '-map', '0:v:0', '-map', '1:a:0', '-vf', $geometryFilter, '-c:v', 'libx264', '-preset', 'slow', '-crf', '18', '-pix_fmt', 'yuv420p', '-g', '24', '-keyint_min', '24', '-sc_threshold', '0', '-bsf:v', 'h264_metadata=crop_left=2:crop_right=2:crop_top=6:crop_bottom=6', '-x264-params', 'colorprim=bt709:transfer=bt709:colormatrix=bt709', '-c:a', 'aac', '-b:a', '96k', '-movflags', '+faststart', $geometryBase)
Invoke-Ffmpeg @('-display_rotation:v', '-90', '-display_hflip:v', '-i', $geometryBase, '-map', '0', '-c', 'copy', '-movflags', '+faststart', $geometryOutput)
Write-Manifest 'm35-geometry-color.mp4' @{
    purpose = 'M3.5 crop, non-square-pixel, clockwise rotation, horizontal flip, BT.709 SDR, marker-color, and audio synchronization input'
    command = "tools/generate-m35-fixtures.ps1 (FFmpeg testsrc2 320x192; SPS crop 2/2/6/6; SAR 4:3; BT.709; rotate 90 degrees clockwise and flip horizontally)"
    expected = [ordered]@{
        frame_count = 24; frame_rate = '24/1'; coded_width = 318; coded_height = 180
        visible_rect = [ordered]@{ x = 0; y = 0; width = 318; height = 180 }
        source_sps_crop = [ordered]@{ left = 2; right = 2; top = 6; bottom = 6; decoder_behavior = 'normalized into the decoded coded/visible size' }
        pixel_aspect_ratio = '421:316'; square_pixel_width = 421; square_pixel_height = 180
        rotation_clockwise = 90; flip_horizontal_after_rotation = $true
        display_width = 180; display_height = 421; half_output_width = 90; half_output_height = 210
        color = 'BT.709 limited-range SDR'; audio = 'AAC LC, mono, 48000 Hz, 523 Hz tone'
    }
}

$timingOutput = Join-Path $fixtureDirectory 'm35-vfr-offset.mp4'
Invoke-Ffmpeg @('-f', 'lavfi', '-i', 'testsrc2=size=320x180:rate=30:duration=2', '-f', 'lavfi', '-i', 'sine=frequency=660:sample_rate=48000:duration=2', '-map', '0:v:0', '-map', '1:a:0', '-vf', "select='not(mod(n,2))+not(mod(n,5))'", '-fps_mode', 'vfr', '-c:v', 'libx264', '-preset', 'slow', '-crf', '20', '-pix_fmt', 'yuv420p', '-g', '30', '-keyint_min', '30', '-sc_threshold', '0', '-x264-params', 'colorprim=bt709:transfer=bt709:colormatrix=bt709', '-c:a', 'aac', '-b:a', '96k', '-output_ts_offset', '1.25', '-movflags', '+faststart', $timingOutput)
Write-Manifest 'm35-vfr-offset.mp4' @{
    purpose = 'M3.5 variable-frame-rate input with non-zero video/audio timestamps and deterministic audio'
    command = "tools/generate-m35-fixtures.ps1 (30 fps testsrc2 with frames selected when n mod 2 = 0 or n mod 5 = 0; timestamps retained; output offset 1.25 s)"
    expected = [ordered]@{
        frame_count = 36; coded_width = 320; coded_height = 180
        video_start_seconds = 1.25; video_duration_seconds = 1.966667
        frame_durations_seconds = @(0.033333, 0.066667)
        audio_start_seconds = 1.228; audio_duration_seconds = 2.021333
        audio = 'AAC LC, mono, 48000 Hz, 660 Hz tone'
    }
}

$hdrOutput = Join-Path $fixtureDirectory 'm35-hdr-tagged.mp4'
Invoke-Ffmpeg @('-f', 'lavfi', '-i', 'testsrc2=size=160x90:rate=12:duration=0.5', '-c:v', 'libx264', '-preset', 'slow', '-crf', '20', '-pix_fmt', 'yuv420p', '-x264-params', 'colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc', '-an', '-movflags', '+faststart', $hdrOutput)
Write-Manifest 'm35-hdr-tagged.mp4' @{
    purpose = 'M3.5 deterministic BT.2020/PQ tagged input that the SDR-only pipeline must reject before conversion'
    command = 'tools/generate-m35-fixtures.ps1 (FFmpeg testsrc2 tagged BT.2020 primaries, SMPTE ST 2084 transfer, BT.2020 non-constant matrix)'
    expected = [ordered]@{ frame_count = 6; coded_width = 160; coded_height = 90; result = 'reject as unsupported HDR' }
}

$firstH264 = Join-Path $workDirectory 'm35-resolution-320x180.h264'
$secondH264 = Join-Path $workDirectory 'm35-resolution-352x198.h264'
$resolutionOutput = Join-Path $fixtureDirectory 'm35-resolution-change.mp4'
Invoke-Ffmpeg @('-f', 'lavfi', '-i', 'testsrc2=size=320x180:rate=15:duration=0.6', '-c:v', 'libx264', '-preset', 'slow', '-crf', '20', '-pix_fmt', 'yuv420p', '-g', '9', '-keyint_min', '9', '-sc_threshold', '0', '-f', 'h264', $firstH264)
Invoke-Ffmpeg @('-f', 'lavfi', '-i', 'testsrc2=size=352x198:rate=15:duration=0.6', '-c:v', 'libx264', '-preset', 'slow', '-crf', '20', '-pix_fmt', 'yuv420p', '-g', '9', '-keyint_min', '9', '-sc_threshold', '0', '-f', 'h264', $secondH264)
$concatInput = "concat:$firstH264|$secondH264"
Invoke-Ffmpeg @('-framerate', '15', '-i', $concatInput, '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=1.2', '-map', '0:v:0', '-map', '1:a:0', '-c:v', 'copy', '-c:a', 'aac', '-b:a', '96k', '-movflags', '+faststart', $resolutionOutput)
Write-Manifest 'm35-resolution-change.mp4' @{
    purpose = 'M3.5 H.264 stream that changes decoded geometry mid-stream and must fail clearly rather than resize implicitly'
    command = 'tools/generate-m35-fixtures.ps1 (concatenated 320x180 and 352x198 Annex-B H.264 sequences, muxed with deterministic AAC)'
    expected = [ordered]@{ frame_count = 18; initial_width = 320; initial_height = 180; changed_width = 352; changed_height = 198; result = 'reject mid-stream geometry change' }
}

Write-Output 'Generated M3.5 fixtures and manifests.'
