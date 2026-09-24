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

## M3.5 metadata fixtures

Four small CC0-1.0 fixtures are generated entirely from FFmpeg filters by `tools/generate-m35-fixtures.ps1`; their JSON manifests record the generator, intended metadata, byte length, and SHA-256. Regenerate them from the repository root with:

```powershell
pwsh -File tools/generate-m35-fixtures.ps1
```

- `m35-geometry-color.mp4` contains 24 H.264 frames plus a 523 Hz AAC tone. Its H.264 clean-aperture crop is normalized by the tested decoders to a 318×180 decoded frame, its MP4 metadata produces a 421×180 square-pixel image, and it carries a 90° clockwise rotation followed by a horizontal flip. Red/green/blue/yellow corner patches make crop, orientation, and representative SDR color observable after conversion.
- `m35-vfr-offset.mp4` contains 36 VFR frames with alternating 33,333/66,667 µs durations. Video starts at 1.250 s and AAC (including priming) at 1.228 s, exercising a shared non-zero origin and preserved A/V offset.
- `m35-hdr-tagged.mp4` is a six-frame BT.2020/PQ-tagged input used only to verify the explicit HDR rejection boundary. Its pixels are synthetic SDR test pixels; the metadata, not visual HDR fidelity, is what this negative fixture tests.
- `m35-resolution-change.mp4` concatenates 320×180 and 352×198 H.264 sequences. It verifies the current fail-fast mid-stream geometry policy and cleanup rather than adaptive reconfiguration.

The geometry fixture deliberately records both source/container intent and observed decoded geometry. WebCodecs decoders may normalize coded padding/crop before exposing `VideoFrame`; the application additionally accepts and validates a non-full `visibleRect` when a decoder exposes one, but the tested H.264 decoders exposed the cropped image as a full visible rectangle.

## M3.6 HEVC input/output fixture

`m36-h265-aac.mp4` is a one-second, 30-frame, 320×180 MP4 generated from FFmpeg's `testsrc2` and `sine` filters. Its HEVC Main-profile video uses the `hvc1` sample entry and BT.709 SDR metadata; its audio is mono AAC at 48 kHz. It validates both H.265 input decode and the capability-gated H.265 output profile without third-party media. The file is CC0-1.0, its manifest records its hash and expected streams, and `tools/generate-hevc-fixture.ps1` regenerates both.

## M3.6 large-input validation fixture

`tests/large-input-fixture.mjs` reproducibly copies the CC0 M2 fixture into ignored `tmp/m36-streaming` and appends a valid 257 MiB MP4 `free` box. The resulting logical 269,763,667-byte file retains the original 60-frame H.264/AAC tracks and is sparse on filesystems that support sparse extension. It exists only to cross the former 256 MiB policy boundary without checking a large binary into the repository; it does not represent a long-duration or large-compressed-output workload.
