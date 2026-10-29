# Encoded-output inspection

Inspected with the local image viewer on2026-10-30: all eight chronological
contact sheets (three licensed excerpts plus the CC0 cut source, both scales)
and the three4x detail triptychs at frame32. Generated files remain under ignored
`tmp/ai-upscale/rtx-quality-20261030/inspection/`; each sheet places bilinear,
RTX High3 and ONNX Real-ESRGAN vertically within a frame cell. Detail triptychs
place them left/middle/right. No native window or real-time video playback was
inspected by these image-viewer calls.

- Text/texture: thin wire outlines and signs change across engines. ONNX gives
  more pronounced reconstructed outlines than bilinear; RTX's differences are
  more modest in the selected frame. This does not establish faithful recovery
  of small glyphs or missing texture from a192x80 input.
- Face: the selected face/hand/brain shot remains recognizable in all outputs.
  ONNX shows smoother areas and more pronounced outlines; RTX remains closer to
  the bilinear appearance. This is a subjective observation, not a fidelity or
  perceptual-quality ranking.
- Motion: hair, cloth and ropes retain visible motion progression in the
  chronological sheets. Fine edge/detail appearance differs between engines.
  Static sheets do not establish absence of visible flicker during playback.
- Cuts: the text/texture and face excerpts include scene transitions; the CC0
  fixture changes from dark to light bars. These transitions remain at the same
  frame positions in the three rows. Independent timing/decode checks provide
  the frame-preservation evidence; sheets alone would not prove it.

The images have no high-resolution ground truth. All outputs use CLAHE50 and
brightness10/contrast115/saturation80, plus lossy H.264 encoding, so differences
cannot be attributed solely to inference. The numeric report contains all
consecutive encoded-frame RGB differences at both scales, without motion
compensation or an intrinsic-flicker acceptance threshold. The inputs are tiny
five-second excerpts, not broad content or HD/4K quality acceptance.

Licensed excerpts: Tears of Steel(2012), Blender Institute, directed by Ian
Hubert, produced by Ton Roosendaal, CC BY3.0. Sources were excerpted, downsampled,
processed and re-encoded; original soundtrack was excluded and replaced with a
CC0 synthetic sine. See `source-manifest.json` for exact source, attribution,
license URLs, changes and hashes. No new assets were downloaded for this run.
