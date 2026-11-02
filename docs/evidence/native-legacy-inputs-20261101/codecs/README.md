# Legacy codec fixtures: decoded, application acceptance pending

`manifest.json` records FFmpeg8.0.1 decoder inventory, codec/profile/color/geometry
metadata, hashes, complete decoded video timestamps/durations and audio frame
timestamps/sample counts. All listed files passed complete independent A/V decoding.
Decoder inventory alone does not establish app or NVDEC acceptance.

Reproduce the original CC0 MPEG-2/MP2, WMV1/WMA1 and WMV2/WMA2 controls with
`tools/native-legacy-inputs/codec_fixtures.py`. Generated clips are192x144/25fps,
1.2seconds with a positive timestamp offset. The manifest also covers the existing
project H.264/AAC and HEVC/AAC MP4 controls.

The user-directed adjacent external-drive files provide WMV3 Main/WMA Pro and
WMV2/WMV3 with WMA2. Two-second stream-copy excerpts and originals stay ignored,
with only redacted metadata/hashes recorded. They are private evaluation inputs,
not redistributable fixtures. The manifest contains no private path or title tags.

Advanced Profile VC-1 is obtained from the LGPL-2.1-or-later GStreamer VAAPI test
data at the pinned source URL/revision recorded in the manifest, copyright
2010-2011 Splitted-Desktop Systems, author Gwenole Beauchesne. Download the source
explicitly to an ignored local path and pass `--vc1-source`; the tool does not
download or install anything. It extracts the20864-byte one-frame sequence,
repeats it30times and muxes generated CC0 WMA2 audio into ASF. No upstream bytes
or derived media are redistributed in this repository. This fixture proves
progressive Advanced Profile decoding only, not temporal/B-frame VC-1 behavior.
Its actual decoded timeline is recorded, rather than assumed from the supplied
mux rate (which differs from the bitstream's own rate).

Discovered boundaries requiring implementation/evaluation in task3.5:

- WMV1/2 often omit every decoded video frame duration, including the final one.
  Some ASF durations include stream origins or exceed actual frame coverage.
- A private WMV2 has average rate0/0; decoded timestamps, not that field, must
  establish the presentation sequence.
- WMA2 can emit a timestamp-less delayed tail frame; WMA Pro yields decoded
  sample counts without frame timestamps even in the original source. Packet
  timestamps alone must not be mistaken for decoded sample alignment.
- Most legacy inputs omit matrix/transfer/primaries. The shared RGB ingress and
  output color policy need independent image verification, not a BT.709 relabel.

These are evidence, not waived requirements or completed compatibility. No
application family matrix, NVIDIA decoder support or shared RTX acceptance is
claimed by this fixture task.
