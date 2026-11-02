# Native AAC timing fixtures

These tiny files are original FFmpeg `testsrc2` and 997 Hz `sine` signals,
dedicated to the public domain under CC0-1.0. There is no private or third-party media.

- `reference.mp4`: correct H.264/AAC timeline, first audio PTS/DTS zero.
- `leading-pts.mp4`: identical encoded media; a test-only MP4 audio `ctts` table
  changes only packet zero PTS from 0 to3600 at time base1/44100. Its next PTS
  are1024,2048; all DTS, durations and payloads match the reference.
- `negative-priming.mp4`: normal AAC priming with an MP4 edit list/skip-samples.
- `positive-offset.mp4`: legitimate250ms source offset.
- `intentional-gap.mp4`: legitimate200ms audio timestamp gap at packet20.
- `reference.m4a`: stream-copied correct audio-only reference for seek tests.

Generate a new output directory with:

```powershell
python tools/native-legacy-inputs/aac_feasibility.py --output tmp/native-legacy-inputs/aac-new
```

The tool refuses to overwrite. Full packet manifests, payload hashes, exact commands,
FFmpeg version and sample comparisons are recorded in
`docs/evidence/native-legacy-inputs-20261101/aac/synthetic-feasibility.json`.
The leading anomaly reproduces the previous app rejection; the rejection evidence
is retained separately. The MP4 mutation operates only on self-generated bounded
fixtures, not on user inputs. Production uses verified audio-only stream copy.

`--size 192x144` generates a separate ignored set large enough for the evaluated
NVIDIA codec routes. Run `verify_aac_routes.py --help` for the actual four-route
integration runner; it independently verifies decoded timing/audio and checks
that corrected-source output samples equal correct-reference output samples.
