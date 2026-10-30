# Proposal

## Why

Native conversion currently rejects usable original files: a verified isolated first-AAC timestamp fault requires an external repaired copy, and MPEG-PS input fails when frame-count metadata is absent. Users should open these originals directly while the app preserves a verified audio/video timeline and display geometry.

## What Changes

- Automatically recognize and normalize the verified isolated leading AAC PTS anomaly during native processing, leaving the source untouched and retaining legitimate priming, offsets and gaps.
- Derive an exact decoded frame count when container metadata omits it, using bounded, cancellable inspection.
- Accept progressive 8-bit SDR MP4 (H.264/HEVC), MPEG-PS (MPEG-1/2 with MP2 audio), and ASF/WMV (WMV1/2/3 or VC-1 with WMA audio) in both software and NVIDIA routes, including rational non-square pixel aspect normalization and matching preview.
- Expose accepted MP4/MPG/MPEG/WMV inputs in native Browse and show applied normalization or precise unsupported-case reasons.
- Separate decoder selection from encoder selection: the NVIDIA route retains NVIDIA encoding while choosing verified NVDEC or software decoding for the source. Expose the actual decoder/encoder combination; never silently substitute software encoding after a NVIDIA encoder failure.

## Capabilities

### New Capabilities

- `native-input-compatibility`: Native source inspection, evidence-based timestamp normalization, legacy display geometry and consistent preview/export behavior.

### Modified Capabilities

None. The main spec inventory is empty. The active `ai-upscale` change remains separate; its runtime, scale, memory and device acceptance gates continue to apply after input normalization.

## Impact

Primarily `media-native` inspection/cache, audio command construction, independent decoder/encoder planning, preview/playback and shared decoder ingress; native application source selection/status; small synthetic fixtures and independent verification tools. Existing FFmpeg/ffprobe and GPU processing are reused; no new runtime download or installer is planned. Browser compatibility, DRM/encrypted media, arbitrary corrupt-file recovery, interlaced/rotated/HDR inputs, new RTX scales and hardware surface sharing are outside this change. Output remains MP4; input container support does not imply every possible embedded codec/profile.
