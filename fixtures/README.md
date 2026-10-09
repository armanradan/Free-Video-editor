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

## M4 10-bit SDR fixture

`m4-10bit-sdr.mp4` is a one-second, 24-frame, 160×96 HEVC Main 10 (`hvc1`) and AAC MP4. FFmpeg `geq` generates deterministic 10-bit Y, U, and V gradients with values between 8-bit code steps; libx265 encodes the fixture losslessly. BT.709 limited-range SDR tags distinguish bit depth from HDR. It contains no third-party media and is CC0-1.0. Its JSON manifest records the generation command, size, and SHA-256. Regenerate it with `pwsh -File tools/generate-m4-10bit-fixture.ps1` using the recorded FFmpeg build.

## Procedural CLAHE engine fixtures

The images generated in `crates/media-gpu/src/equalization_tests.rs` are original synthetic test data, dedicated to CC0-1.0, with no external media. They are ephemeral normalized encoded-RGB float arrays, not a downloaded video. The checked-in generator is the reproducible manifest: 130×129 dimensions, flat black/white/gray/near-black, modular low-contrast colored/noisy patterns, a +0.4 scene cut, sixteen 1/1023-spaced gray levels and a fixed 100-pixel gray patch with varying histogram noise elsewhere. It records exact VFR and 17 ms timestamp sequences, strengths and comparison metrics. Reproduce with `cargo test --locked -p media-gpu actual_gpu_clahe -- --ignored --nocapture` on an available compute-capable adapter. All readbacks are diagnostic; no source/output media files or large fixtures are persisted. This is an engine numeric/flicker test, not codec/A/V or general scene-cut acceptance.

## M5 release comparison fixtures

`tests/native-clahe.mjs` reuses the checked-in CC0 `m35-vfr-offset.mp4`, without downloading/generating a new source. It runs native shared software/NVIDIA CLAHE at 50% geometry and Original/15/60 FPS, checks every output timestamp, audio/full decode, copy/scratch telemetry and off/zero/direct/depth rejection. Outputs/evidence stay under ignored `tmp/clahe/native-*`. The Rust CLAHE lifecycle test reuses CC0 `m2-h264-aac.mp4` and keeps outputs under ignored `tmp/clahe/lifecycle-*`.

`tests/browser-output-settings.mjs` reuses the CC0 M2/VFR fixtures, generates a two-frame/no-audio **re-encoded presentation-order** excerpt, and generates a four-second 640×360/30-FPS `testsrc2` + seeded temporal noise (`all_seed=42`) + 523 Hz sine source. These contain no third-party media and are CC0-1.0; exact generation commands live in the harness. Copying only two B-frame packets would create a different presentation timeline and is intentionally avoided. Sources, outputs, screenshots and real-browser/independent FFmpeg evidence remain under ignored `tmp/browser-output-settings`.

`tests/native-frame-rate.mjs` reuses the licensed `m2-h264-aac.mp4` and `m35-vfr-offset.mp4` fixtures above and generates a two-frame/no-audio excerpt of the former to catch fractional-duration EOF rounding (same CC0 license; command in harness). No new input media is checked in. The gate writes only ignored `tmp/m5/frame-rate-*` outputs/evidence. It independently checks all decoded output timestamps and frame counts at Original, 15, 60 and exact 30000/1001 FPS, audio coverage, and complete A/V decoding on all four native routes, plus two HEVC Main 10 fixed-FPS cases and four short/no-audio regressions.

`tests/native-bitrate.mjs` generates an ignored 640×360, 180-frame/6-second H.264/AAC source from FFmpeg `testsrc2`, temporal noise (`all_seed=42`, strength 10) and 523 Hz/48 kHz sine. It contains no third-party media and is CC0-1.0. The command is kept in the harness. The texture/noise makes 1-versus-4-Mbps size differences meaningful; it does not establish perceptual quality or estimate accuracy on arbitrary footage. Sources, output reports and independent FFmpeg/FFprobe checks stay under ignored `tmp/m5/bitrate-*`.

`tests/native-m5-hardware.mjs` also generates a tiny 320×192/30-frame BT.709 limited-range H.264/AAC `testsrc2`/523 Hz sine source under ignored `tmp/m5`. It is CC0-1.0 with no third-party media. Its shared-NVIDIA output is compared to an uncompressed bilinear resize, so correct tags cannot conceal a wrong RGB→NV12 matrix. The generation command lives with the hardware regression.

`tests/native-m5-release-bench.mjs` generates 1920×1080/180-frame and 3840×2160/90-frame 30 fps H.264/AAC sources from FFmpeg `testsrc2`, seeded temporal noise (`all_seed=17`) and a 523 Hz, 48 kHz mono sine. They contain no third-party media and are CC0-1.0. Source SPS/VUI explicitly declares BT.709 limited SDR, checked before conversion; libx264 VUI parameters are necessary because output flags alone did not retain all tags in the tested FFmpeg build. The benchmark's `evidence.json` records source generation arguments, exact versions, hashes, dimensions, timestamps/duration and output validation. Large generated fixtures and compressed outputs remain under ignored `tmp/m5/release-bench`, never checked into Git. These short high-frequency synthetic sources are not representative camera footage or long-duration memory/thermal tests.
