# Design

## Context

See proposal.md for motivation. Current `media-native::inspect_source` rejects non-increasing PTS in the first 32 AAC packets and requires `nb_frames` before its existing video timestamp scan. `InspectionCache` binds SourceInfo/timeline to a canonical path and file stamp. Shared export obtains audio from a second original-file input; direct export and playback build separate FFmpeg commands. Playback currently uses asynchronous resampling with `first_pts=0`. AI preview/history independently reject non-square pixels. Native Browse filters only MP4.

Recorded evidence in `docs/evidence/ai-upscale-20261030/audio-timestamps/report.json` establishes one leading AAC PTS error: 3600 instead of DTS 0, followed by PTS=DTS at 1024-sample intervals. Explicit remux changed only that first PTS; 5518 audio and 3202 video packet payloads were identical. The original is still unsupported. MPG evidence records MPEG-1 352x288, SAR178:163, MP2 and absent frame count. Neither record proves integrated automatic compatibility.

## Goals / Non-Goals

**Goals:** Make source compatibility an owned native policy shared by inspection, preview and export. Begin with the AAC original, then missing-count/legacy geometry. Maintain source identity, cancellation, exact integer timing and independent output verification.

**Non-Goals:** Generic timestamp reconstruction, automatic corruption concealment, browser demux changes, additional RTX envelopes, DRM/encrypted media, interlacing/rotation/HDR support or GPU surface sharing. Existing ai-upscale acceptance remains independent; no task there is completed by this change. NVIDIA route input compatibility is expanded here, without claiming that every accepted input can be decoded by NVDEC.

## Decisions

### 1. Classify the complete audio timeline before correction

Replace the rejection-only AAC prefix check with a native timing decision: unchanged or a specifically evidenced first-packet correction. Stream packet PTS/DTS/duration and relevant skip/priming/edit-list metadata through a bounded parser; retain aggregates and the first few records, not all packet JSON. Validate checked integer arithmetic, stable time base, monotonic decode order and all later presentation timestamps. The supported anomaly must have an incorrect leading PTS that conflicts with the next packet, a consistent first DTS/duration connecting to the subsequent timeline, and no conflicting priming/edit-list explanation. Valid gaps/offsets stay unchanged. Ambiguous anomalies reject with evidence. Prefix inspection alone cannot authorize normalization.

Blind `PTS=DTS` for all packets and `N/sample_rate` reconstruction were rejected because they can erase real timing. The private example proves a narrow correction, not a generic media repair rule.

### 2. Prepare private audio only, without touching the original

For a classified fault, use an app-owned audio-only stream-copy intermediate on the AAC/MP4 path, altering only packet zero's PTS. Existing FFmpeg packet timestamp filtering is the initial mechanism; exact expression/time-base/mux behavior must pass packet-level tests before integration. Video always decodes from the original. Verify copied audio payload hashes, count, DTS, durations and all unchanged PTS before publishing the prepared audio. No video duplicate or user-visible repaired source is required.

Bind preparation to the same inspected file stamp and immutable timing decision. Share one prepared audio owner between playback and conversion; delayed consumers retain it safely. Explicit disk admission accounts for audio payload plus container overhead, imposes a fixed scratch ceiling and checks available space. Preparation is cancellable, writes to an owned unique path and cleans incomplete files; session/source retirement releases completed scratch once consumers finish. Handle abandoned session directories conservatively using ownership markers. Never delete arbitrary user paths.

A private audio intermediate is chosen over decoded sample-clock filters because packet preservation is directly testable and legitimate later gaps stay intact. Its cost is audio-only disk I/O and startup latency for affected sources. Healthy sources have no intermediate. If the selected container cannot retain the required packet semantics, record a feasibility blocker rather than broadening normalization.

### 3. One timing decision for all native consumers

Carry the decision and normalized audio metadata in inspected native state, without adding filesystem policy to media-core. Factor native audio-input planning so direct CPU/NVIDIA, shared software/NVIDIA and preview/playback choose the same original or prepared audio source and consistent origin offsets. Preserve source video timestamps and use normalized audio expectations in the existing verifier. Seeking must account for separate audio/video origins; test initial playback, forward/backward seek and terminal coverage. A cache hit must revalidate file identity. Reports expose the correction and scratch cost, never imply hardware audio execution.

### 4. Count during the existing decoded timeline scan

Allow the video scan to operate without a declared frame count for MPEG-PS and ASF/WMV; count actual decoded frames and preserve existing metadata-count mismatch checks when metadata is present. Reuse the current timeline storage budget and cancellation ownership; do not add a second full decode merely to count. Derive the last frame duration from reliable decoded/container evidence, not a fabricated average-rate endpoint. Missing or contradictory timing fails explicitly. Test MPEG-PS/ASF start offsets and rational timestamp conversion.

### 5. Normalize pixel aspect once before shared processing

Keep coded geometry and rational SAR separately. Define square-pixel normalization at coded height, with checked rational width rounded to the nearest even integer (halfway rounds upward); report the resulting dimensions and rounding error. For 352x288 SAR178:163 this yields384x288, then768x576 at2x or1536x1152 at4x. Retain original display aspect metadata for audit. Reject overflow/invalid ratios and unsupported display transforms.

Use existing native decoder/color ingress for this bounded pre-AI normalization, shared by history/AI preview and conversion. Do not create a second enhancement processor. Retain explicit CPU staging/copy accounting and perform RTX memory/dimension admission using the normalized input. Direct FFmpeg geometry follows the same display-resolution policy. Structured fixtures must establish visual aspect parity, not just `setsar=1` metadata. Scale fitting for the display happens after selected output processing as before.

### 6. Plan decoding independently from NVIDIA encoding

The current native route uses one hardware flag for decoder and encoder setup, copy counters and acceptance checks. Split these native decisions into actual decode mode and encode mode without adding platform policy to media-core. Both direct and shared NVIDIA routes select NVENC independently of whether the source uses NVDEC. The software route uses software decoding/encoding. MP4 H.264/HEVC, MPEG-PS MPEG-1/2 and ASF WMV1/2/3/VC-1 are required input families; audio targets include AAC, MP2 and WMA1/2/Pro, with actual installed decoder capability and profile tests before acceptance. Output stays within existing MP4 profiles; no WMV/MPEG output is added.

Resolve decode capability before partial output creation. Use NVDEC only for configurations verified on the selected device; otherwise plan software decoding explicitly, retaining NVENC and the selected RTX settings. This is the advertised NVIDIA route policy, not a retry after a failed export. Unexpected decoder initialization/runtime failure reports the boundary; do not silently restart with a different decoder or switch away from NVIDIA encoding. Missing NVENC still blocks the NVIDIA route. Report chosen decoder, encoder, GPU and reasons; count NVDEC downloads only when used and NVENC uploads according to the actual bridge. Preserve exact GPU identity, current RTX budgets and the validated no-B-frame timing policy.

Remove the codec-name-only H.264/HEVC NVIDIA admission rule after the hybrid paths pass tests. Keep geometry, bit depth, color, SDK and memory gates. MPEG and WMV inputs do not grant new hardware decoder capabilities, but their lack of NVDEC support alone cannot reject NVIDIA encoding. Verify both direct hybrid and shared RTX hybrid paths; sharing only the UI policy is insufficient.

### 7. Expose supported source families accurately

Extend only the input picker to MP4/MPG/MPEG/WMV; output remains MP4. Detect actual container/codec/profile rather than trusting extensions. Validate missing ASF durations/counts, WMA sample timing, variable-rate video, legacy color tags and pixel aspect in small fixtures. Resolve color from defensible source metadata/codec rules and record any assumption; do not relabel pixels as BT.709 merely to pass current gates. Exclude DRM/encrypted, interlaced and unsupported HDR/profile cases explicitly. Show concise source normalization and actual decode/encode mode through existing status boundaries. No consent changes or provisioning.

## Risks / Trade-offs

- [Priming/edit-list ambiguity] -> Require complete packet evidence and regression controls with valid negative priming, nonzero origins and gaps; reject ambiguous cases.
- [Muxer changes timestamps beyond the correction] -> Verify prepared packet payloads and exact integer timing before publishing; keep the original immutable.
- [Extra inspection/scratch latency] -> Stream bounded metadata, cache by source stamp, prepare audio only for classified sources, report costs and reject disk exhaustion cleanly.
- [Preview and export diverge] -> Route all audio source/origin decisions through one native plan; validate audible sample alignment and seek coverage independently of video-only checks.
- [MPEG-PS duration/rate metadata differs from decoded evidence] -> Use decoded PTS/count with checked endpoints and preserve Original VFR; no guessed frame count or global rate reset.
- [Pixel aspect normalization changes AI memory needs] -> Admit actual normalized geometry, retain current budgets and document explicit integer rounding.
- [Private-file dependency in validation] -> Generate tiny redistributable synthetic anomaly/MPEG fixtures with provenance and retain only redacted private-file evidence.
- [WMV/VC-1 and WMA decoder/profile variability] -> Probe installed capabilities and run real decoded fixtures for each target; preserve unsupported profiles as explicit blockers rather than dropping the WMV requirement or declaring support from file extensions.
- [Hardware flag conflates transfer accounting and failure behavior] -> Test independent decode/encode plans, actual counters, NVENC identity and no software-encoder substitution on failure across direct/shared paths.

## Migration Plan

No saved configuration migration or new dependencies is planned. Implement and validate AAC acceptance first, then missing-count/geometry, independent codec planning, WMV/MPEG coverage and native UI integration. Keep failure fixtures and existing valid-source controls. Run actual original-source export plus independent full decode/timing checks; run the MPG original through software and NVIDIA-encode routes, with and without accepted RTX. Verify the MP4/MPEG/WMV matrix in both route families. Preserve ai-upscale changes and pending tasks. Rollback restores the precise rejection policy and invalidates prepared compatibility cache entries, without changing any original file or weakening output verification.
