# Native AAC compatibility evidence

Task group1 and the native classifier/preparation tests use FFmpeg8.0.1 essentials
on Windows. Synthetic signals are CC0-1.0; private source media and paths are excluded.

`synthetic-feasibility.json` records complete packet metadata/hash manifests and
commands. Correcting only the leading AAC PTS preserves all62 payloads, DTS,
durations and later PTS. Full f32le decode matches the correct reference exactly
(253952bytes). Valid negative priming/edit-list,250ms offset and200ms gap controls
preserve packet timing, skip metadata and decoded samples through audio-only remux.
Healthy production inputs bypass preparation entirely.

`private-audio-feasibility.json` records the retry after the external source drive
was reconnected:5518audio packets, first PTS3600->0, all other PTS/DTS/durations,
payload hashes and source bytes unchanged. The private audio-only intermediate
is2034300bytes. This isolated test alone is not normal-window export acceptance.

Production streams complete packet metadata with4096-byte line and2000000-packet
limits, compares default and edit-list-disabled evidence before authorizing a
correction, and verifies the prepared packet/time-base fingerprint before use.
Scratch admission reserves payload+4MiB container allowance within64MiB and leaves
32MiB disk free. Disk admission is currently Windows-only. Source stamps bind
preparation/cache use to the original; a single weak metadata entry shares the
owned audio between preview/export without retaining scratch after consumers retire.
Failure/cancellation and normal retirement remove owned files. Abrupt process death
can leave marked scratch directories; no automatic deletion of old sessions is claimed.

The initial failure-cleanup test exposed an installed Windows FFmpeg launcher
returning exit0 for `-n` output-exists errors. Preparation now treats any stderr
from its `-v error` command as failure too, then still requires full packet verification.

The first independent route-check attempt incorrectly used the deliberately
anomalous header's audio start as its expected origin. The corrected checker derives
the expected audio timeline from the fixture's known correct reference; no production
verifier or tolerance was changed. The initial failed report is retained locally.

Preview seek experiments found small sample differences when comparing audio-only
and video-indexed MP4 seek startup histories (max observed7.47e-5 in float samples),
despite exact full decode. This does not establish a decoder-internal cause.
The regression uses a correct stream-copied audio-only oracle with equal packet
timing/payloads and requires exact samples at initial/forward/backward/tail positions.
The playback decoder now decodes100ms bounded preroll before trimming and resampling;
valid gaps/offsets are retained by subtracting only that preroll from timestamps.
Physical audible A/V synchronization is separate from sample/timestamp verification.

The synthetic anomaly also inflated the container duration from about1.423s to
1.505s. A new normalized-source EOF/cancel regression exposed that stale header
seeking past video EOF. For corrected sources, preview now uses video-track
coverage and the verified corrected audio packet endpoint; healthy source duration
policy is unchanged. The normalized pause/play/EOF/cancel test passes after this fix.

The ordinary native window selected the private original directly and completed
RTX2x with Shared GPU NVIDIA:3202frames in/out,2560x1440,70253292bytes,
345639ms total. Independent full A/V decode and all video timestamps pass;
audio endpoint error is0.00029s. The output is byte-identical to the previously
verified repaired-source hardware export. Original hash/size/mtime are unchanged.
See `original-ui-export.json`. The elapsed time overlapped development work and
is not a controlled performance comparison.

An isolated actual production preparation took903ms and2034300bytes of scratch;
packet verification passed and final owner retirement removed scratch. See
`private-production-preparation.json`. After reboot the GPU's volatile LUID changed;
local configuration was refreshed and the real automatic RTX test rerun. SDK/helper
hash and remembered terms approval were preserved; no capability result was imported.
