# MPEG-1 direct software acceptance

FFmpeg8.0.1 essentials, Windows, native GNU build. `fixtures/native-mpeg1.mpg`
is original CC0 testsrc2/sine material; its adjacent manifest records the generator,
hash, every decoded video timestamp/duration and every MP2 packet timestamp/duration.

Inspection now derives missing frame counts during the existing decoded scan,
retains declared-count mismatch checks and uses the last explicit decoded duration
for the endpoint. It retains at most2,000,000 timestamps (about32MiB payload),
limits each metadata line to4096bytes and rejects missing final durations,
non-increasing timestamps, interlacing and geometry changes. Cancellation terminates
the child. Actual MPEG and rational VFR fixtures, malformed EOF, count mismatches,
arithmetic overflow and pre-cancellation pass regression tests.

The MPEG header reports1.98s video and1.933067s audio. Decoded video is exactly
50frames/2s at25fps, with origin1.25s; the complete MP2 packet coverage is
2.011433s from1.239089s. The scanner retains these actual endpoints rather than
estimating count or EOF from average rate. MPEG's50Hz `r_frame_rate` describes
fields here and is not evidence of50decoded frames per second.

`native-direct.json` and `independent-direct.json` record an actual direct software
export:50frames,384x288 square pixels,182079bytes. Independent full A/V decode
passes; all50video timestamps differ by at most0.000911111s, audio origin error
is zero and duration error0.000433333s, within existing tolerances. Three sampled
frames against independent source bilinear scaling have RGB RMSE5.5731 (gate10).

The first export lost known limited range in H.264/MP4. The direct encoder now
forwards known color fields and uses `+faststart+write_colr` when only limited range
is known. FFmpeg8.0.1 labels `write_colr` experimental; the evaluated output retains
`tv` without inventing matrix, transfer or primaries. This is a pinned runtime
behavior requiring reevaluation on FFmpeg updates. No BT.709 relabeling is added.

This proves direct software MPEG-1/MP2 only. Non-square shared GPU/RTX ingress,
WMV/WMA metadata policy, hybrid NVIDIA routing and the complete family matrix
remain separate unchecked tasks. Browser behavior is unchanged.
