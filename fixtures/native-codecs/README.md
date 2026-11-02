# Native legacy codec controls

These small MPEG-2/MP2, WMV1/WMA1 and WMV2/WMA2 files are original
FFmpeg testsrc2/sine fixtures dedicated to the public domain under CC0-1.0.
Generate with `python tools/native-legacy-inputs/codec_fixtures.py --output NEW_DIRECTORY`.
The generator refuses an existing directory. It uses192x144,25fps,1.2seconds
and a positive source offset. Hashes, codec metadata and complete decoded frame
manifests are in `docs/evidence/native-legacy-inputs-20261101/codecs/manifest.json`.

Private WMV3/WMA Pro clips and the upstream LGPL VC-1 evaluation fixture remain
in ignored local evaluation storage; they are not part of these CC0 assets.
The codec evidence README describes their provenance, reproduction and limits.

`wmv2-wma2-offset-gap.wmv` adds a100ms positive audio offset and150ms interior
gap, with an overall1.25second source origin. Generate it with
`tools/native-legacy-inputs/wma_timing_fixture.py --helper BUILT_HELPER --output NEW_DIRECTORY`.
Its exact Windows PCM/timestamp manifest and creation command are in
`docs/evidence/native-legacy-inputs-20261101/codecs/wma-offset-gap-manifest.json`.
