# Approved WMV final-frame policy decision

The user approved the bounded, reported WMV fallback on2026-11-01. The design,
delta specification and task3.5 now include it. The original issue and decision
are retained below; broader WMV audio/color acceptance remains in progress.

Actual normal CLI admission of `fixtures/native-codecs/wmv1-wmav1.wmv` fails before
output creation with `Unknown-count source requires an explicit final decoded
frame duration`. All30decoded frames have known PTS but no duration. The source
was generated at25fps with observed PTS1250,1290,...,2410 in timebase1/1000.
The original user WMV2 also omits frame durations and reports average rate0/0.
Container duration cannot generally be substituted: ASF estimates can include
the origin or extend to an audio tail beyond the last video frame.

Approved explicit design revision:

- Only for a missing final WMV duration, use the declared rational rate if its
  complete decoded timestamp grid is consistent within the source time-base
  quantization. Retain every observed PTS; do not reconstruct interior timestamps.
- Label the resulting terminal duration as inferred in source/report/UI metadata.
  Do not claim that the original supplied an exact duration.
- Keep checked arithmetic, total-frame/line bounds, declared-count mismatches,
  cancellation and unchanged output-verification tolerances.
- Reject an ambiguous rate, a VFR ending without explicit terminal duration or
  contradictory reliable terminal evidence. Do not silently choose the last
  observed interval or the longest container track.

This does not resolve WMA decoded sample anchoring by itself. The recorded WMA
Pro decode has sample counts but lacks ffprobe frame timestamps; FFmpeg's actual
audio filter path supplies a sample-clock timeline. That behavior still needs
explicit packet/decoder alignment, offset/gap and tail verification before app
acceptance. No WMA timestamp reconstruction is authorized by this proposal.

Implementation checks cumulative and individual intervals in exact source ticks,
requires at least three frames and at least two ticks per period, and rounds
the final period to nearest tick with half ties upward. An explicitly nonpositive
duration is contradictory evidence and rejects. Task3.5 and later acceptance
tasks remain unchecked until their complete behavior is verified.
