# M1 deterministic VP8 fixture

`m1-vp8.ivf` is a 30-frame, 320×180, 30 fps VP8 elementary stream in an IVF packet envelope. It is generated locally by `tools/generate-m1-fixture.ps1` with FFmpeg's `libvpx` encoder. The app strips the IVF packet headers and submits the compressed VP8 payloads to WebCodecs; no container demuxer is used.

The source is generated from solid-color and geometric filters: red/green/blue/yellow corner orientation patches and a changing decimal frame identifier. It contains no third-party media. The generated fixture and manifest are dedicated to the public domain under CC0-1.0.

Regenerate from the repository root:

```powershell
pwsh -File tools/generate-m1-fixture.ps1
```

The script fixes encoder threading, lag, keyframe interval, bitrate controls, and all source filters. `m1-vp8.json` records the FFmpeg version, codec configuration, packet offsets/sizes/timestamps, and SHA-256.
