# Native legacy display geometry and RTX: evaluated2026-11-01

Windows, FFmpeg8.0.1, RTX3050Laptop/Vulkan, driver621.01, SDK1.1 High3,
evaluated helper e9188624f8a4605f4429f83f8c74463a22db9bfd374a36f590a23f37cf5cac95,
LUID0:78387. Existing SDK consent, scale limits and384MiB admission remain unchanged.

Checked rational display width rounds once to the nearest multiple of two,
ties upward, at unchanged coded height. Equivalent square SAR preserves coded
dimensions; invalid/overflowing/zero-result geometry rejects. For352x288 at
SAR178:163, normalized ingress is384x288. The existing rotated geometry fixture's
display size becomes180x422 (previous whole-pixel rounding180x421); direct
rotation remains supported, shared display transforms remain rejected.

Conversion, history/AI preview and ordinary no-transform preview use the same
CPU decoder ingress normalization before effects/AI. Actual RTX2x/4x selected
preview processing matches independent full-source chronological decode and the
conversion processor with0RGBA LSB error. `preview-parity.json` records selected
sizes768x576 and1536x1152, selected80ms with two predecessors, display480x360
and measured transfers. Diagnostic pixel readbacks are excluded from throughput.
MPEG CPB side metadata is recognized as non-transform metadata; display matrices
remain an explicit rejection. The first parity attempt exposed that distinction.

Synthetic original-FPS exports pass complete independent decode, every video
timestamp, audio coverage and three sampled images: neutral shared384x288 and
actual RTX768x576/1536x1152. Video PTS/endpoint error<=0.912ms, audio origin error0
and endpoint error0.434ms. Image RGB RMSE vs independent bilinear references
is below5.85 (gate20); detailed outputs and hashes are in the adjacent reports.

The matching original private MPEG-1 input directly completed shared software-codec
RTX2x:6941frames in/out,768x576,85981343bytes,131544ms total (117625ms conversion).
Independent full A/V decode, all6941PTS, final video coverage, audio endpoints
and three sampled image/aspect checks pass. Video PTS/end error0.778ms, audio
start error0 and endpoint error0.245ms; sampled RGB RMSE<=3.955. Original SHA256,
length and exact UTC modification time remain unchanged. Filename/media stay
ignored; `private-original-identity.json` records redacted identity proof.
Elapsed time overlapped evaluation work and is not a controlled benchmark.

Ingress CPU decode/SAR bilinear scaling/color conversion, helper D3D staging,
wgpu upload/readback and CPU encoder bridging remain explicit costs. Normalized
RGBA input is442368bytes/frame instead of coded405504bytes; helper2x output is
1769472bytes/frame and4x7077888bytes/frame. Existing counters and memory admission
use actual normalized/enhanced dimensions. No shared-surface or zero-copy claim.
The first export attempt exposed one coded-width processor allocation and a
range-only MP4 tag loss; both failed before publishing output and were corrected.

Untagged MPEG/WMV input resolves to a BT.601 matrix with BT.709 working transfer
and primaries assumptions when those fields are absent. Raw source tags remain
unknown in reports; `color_interpretation` records assumptions. Explicit supported
BT.601/BT.709 tags take precedence. Effects/AI convert pixels to encoded sRGB,
then convert output to limited BT.709 and tag it accordingly. Neutral direct
conversion retains known source tags. This is an interpretation of absent color
metadata, not evidence of the original mastering primaries. HDR/wide/full-range
and unsupported pixel formats remain rejected. Range-only neutral MP4 retains
the evaluated experimental `write_colr` handling described in MPEG evidence.

Additional actual software-decoded RTX2x exports pass native verification for
MPEG-2/MP2, generated WMV2/WMA2 and normalized Advanced VC-1/WMA2. These initial
ingress results do not complete the independent family/NVIDIA matrix. Private
WMV3/WMA audio revealed a separate decoder-clock boundary; see codec evidence.
NVIDIA hybrid routing and ordinary-window family acceptance are still pending.
