# M1 deterministic VP8 fixture

`m1-vp8.ivf` is a 30-frame, 320×180, 30 fps VP8 elementary stream in an IVF packet envelope. It is generated locally by `tools/generate-m1-fixture.ps1` with FFmpeg's `libvpx` encoder. The app strips the IVF packet headers and submits the compressed VP8 payloads to WebCodecs; no container demuxer is used.

The source is generated from solid-color and geometric filters: red/green/blue/yellow corner orientation patches and a changing decimal frame identifier. It contains no third-party media. The generated fixture and manifest are dedicated to the public domain under CC0-1.0.

Regenerate from the repository root:

```powershell
pwsh -File tools/generate-m1-fixture.ps1
```

The script fixes encoder threading, lag, keyframe interval, bitrate controls, and all source filters. `m1-vp8.json` records the FFmpeg version, codec configuration, packet offsets/sizes/timestamps, and SHA-256.

## M2 MP4/H.264/AAC fixture

`m2-h264-aac.mp4` is a two-second, 60-frame, 640×360 MP4 generated entirely from FFmpeg's `testsrc2` video and `sine` audio filters. It exercises MP4 demux, H.264 initialization/B-frame decode, timestamp conversion, the explicit audio-omission warning, VP8 encode, and WebM finalization. It contains no third-party media and is dedicated to the public domain under CC0-1.0.

`m2-h264-aac.json` records the complete FFmpeg 8.0.1 command, stream configuration, size, and SHA-256. Re-run its `command` from the repository root with the recorded FFmpeg build to regenerate it.
