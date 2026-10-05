# Color adjustments and pre-conversion preview

Status: **planned, not implemented or runtime-verified**, 2026-10-05. This is a scoped browser/native follow-up within the current work, not completion of M5 or authorization to build other roadmap phases. Start implementation only on a subsequent user request.

## Scope and controls

Add one compact Color section to both versions:

| Control | UI range | Neutral | Processing value |
|---|---|---|---|
| Brightness | −100 to +100 | 0 | Additive offset −1 to +1 |
| Contrast | 0–200% | 100% | Multiplier 0–2 around mid-gray |
| Saturation | 0–200% | 100% | Multiplier 0–2 around grayscale |
| Local equalization (CLAHE) | Off / On | Off | Optional luma equalization |
| Equalization strength | 0–100% | 50% when enabled | Blend original/equalized luma; 0% is identity |

Show current numeric values, Reset color, and Before/After. Before means the **same frame with all color processing disabled and the selected geometry**, not a different source position or unresized image. Reset restores neutral sliders, disables equalization and restores its default strength; it leaves resize, FPS, bitrate and profile unchanged. Equalization never takes ownership of, disables or rewrites saturation/contrast/brightness: all three remain independently editable while it is enabled. Strong combined settings may amplify noise or clip detail; show the actual combined preview and an explanatory hint, not an automatic restriction. Preview starts paused; play/seek remain independently controlled. Do not add an image editor, LUTs, exposure/gamma, HDR grading, automatic enhancement, animation/keyframes, or a render graph.

Use existing supported opaque SDR inputs. Keep native direct Main 10 support: do not silently quantize a 10-bit conversion to an 8-bit intermediate when adjustments are enabled. Native preview remains an 8-bit display approximation, not proof of preserved 10-bit output. Existing shared-wgpu depth restrictions remain explicit.

## Shared color contract

`media-core` owns a small validated `ColorAdjustments` value, neutral detection and normalized shader/filter parameters. Prefer integer slider units for stable serialization/equality; reject out-of-range/non-finite external values instead of silently clamping settings. Attach this value to the shared output-settings policy with a neutral default, including legacy/library calls. Keep codec, UI and GPU handles out of core.

For the first implementation, define adjustments on **normalized, full-range, sRGB-encoded RGB**. This is a display-oriented SDR adjustment, not linear-light grading or exposure. Resolve actual texture sampling/output transfer behavior explicitly; never accidentally apply the formula to linear values from an sRGB texture or double-apply a transfer function. Normalize accepted BT.709 material at the platform boundary and validate cross-route handling rather than treating BT.709 and sRGB transfers as identical.

After existing crop/PAR/orientation and bilinear resize, apply optional equalization first, then saturation, contrast and brightness. The following slider math consumes the reconstructed equalized RGB when enabled, or the unchanged resized RGB when disabled:

```text
Y = dot(rgb, [0.2126, 0.7152, 0.0722])
saturated = Y + saturation * (rgb - Y)
adjusted = (saturated - 0.5) * contrast + 0.5 + brightness
output = clamp(adjusted, 0, 1)
```

Here Y is a weighted encoded-channel grayscale reference, not physical luminance. Clamp once at the end of the slider chain; define equalization reconstruction/gamut handling separately below. Preserve opaque alpha. Neutral sliders with equalization off (or zero strength) bypass adjustment work and preserve the existing route/color/depth behavior. Define a pure reference implementation for tests; do not add CPU pixel loops to the GPU conversion path. Keep transforms and shader code shared, with an explicitly tested FFmpeg mapping where that runner is used. An FFmpeg `eq` filter with identical slider numbers is **not** assumed equivalent to RGB math.

## Optional histogram equalization follow-up

Implement **contrast-limited adaptive histogram equalization (CLAHE)** as a separate slice after the three sliders and editable preview work end-to-end. Basic global equalization is not another initial mode. Core owns a disabled/default or enabled/strength policy; algorithm revision, tile geometry, histogram precision, clip-limit semantics and interpolation must be specified and tested before claiming cross-route parity. Initially expose only Enable and Strength, not a second panel of expert controls.

Build tiled histograms from the same normalized encoded-luma domain used by the shared color contract, limit/redistribute counts, build mappings and interpolate between tiles to avoid visible boundaries. Blend original and mapped luma by Strength, then reconstruct RGB with a documented chroma-preserving rule before applying the three manual sliders. Test near-black division/epsilon handling, black/white, gamut clipping and saturated patches; applying independent histograms to RGB channels is out of scope. This is a luma-focused operation, not a guarantee of perfectly preserved hue/saturation. Keep intermediates precise enough for supported Main 10 conversion; an 8-bit preview or histogram must not silently dictate 8-bit conversion precision.

GPU routes require additional bounded histogram/mapping/application passes, owned by media-gpu on the existing processing device. Unlike sliders alone, enabled CLAHE cannot be described as a single fused resize pass. Budget tile/bin storage and scratch textures against actual device limits; clear/reuse them only after prior submissions finish. Keep statistics/mappings GPU-resident with no conversion pixel readbacks or per-frame Dioxus state. Disabled or zero-strength CLAHE skips these passes. Browser FFmpeg WASM consumes the already adjusted frames; never equalize a second time in its encoder.

Video requires deterministic **temporal stabilization of histogram mappings**, plus scene-cut resets. Specify a finite, bounded, causal history and timestamp-aware weights during the implementation spike; do not use an unbounded running history or require future frames. Compute statistics once per source frame before output-FPS duplication/drop: duplicates must not advance history or apply equalization repeatedly. Source replacement, seek, device generation, algorithm-setting changes and cancellation reset/invalidate the relevant state. Test cuts and slow exposure changes rather than smoothing across unrelated scenes.

A paused single-frame preview must not silently claim equivalence to history-dependent conversion. Prefer bounded preceding-frame preroll using the same finite-history policy for a same-position comparison; expose any isolated-frame approximation until that parity is proven. Retain only the current original image and bounded histogram/mapping history, not a window of raw frames. Slider/Strength changes reuse cached statistics when their upstream inputs are unchanged; resize or algorithm changes invalidate them. Before/After bypasses both equalization and sliders without modifying the saved values.

Native direct CPU/NVIDIA FFmpeg support is separately capability-gated. Investigate the selected build's actual filters and their luma, clip, tile, precision and temporal semantics; do not assume a filter named CLAHE matches this contract. A CPU adapter may perform the required histogram processing, but native hardware routes must not silently download to CPU, change route or drop stabilization. If parity/support is unavailable, clearly disable that route for enabled equalization and offer an explicit supported route. Neutral/off behavior remains unchanged. Do not ship an option that preview displays but conversion ignores.

The follow-up acceptance gate covers off/zero-strength identity, flat images, clipping/redistribution, tile-edge continuity, color/gamut and 10-bit precision, combined settings with all sliders still usable, actual preview/output pixel parity, deterministic repeated runs and seeks, measured flicker on stationary/noisy/slow-exposure fixtures, scene cuts, VFR/fixed-FPS timing/audio preservation, queue/storage limits, cancellation/device-loss cleanup and whole-job throughput. Compare stabilized and unstabilized results against a recorded metric; do not call flicker solved from screenshots. Report extra GPU passes/memory separately, and mark unsupported or unavailable routes untested.

## Processing routes

- **Browser WebCodecs:** extend the existing media-gpu resize render pass with slider uniforms. Sliders alone should need no added processing pass, device, CPU pixel readback or frame persistence; enabled CLAHE adds the bounded GPU passes above. Preserve render/submit/present/capture ordering and frame guards.
- **Browser FFmpeg WASM:** adjust in that same GPU pass before the existing RGBA ring. Do not apply color twice in FFmpeg. Existing explicit readbacks/software encoding and copy accounting remain visible.
- **Native shared-wgpu/software and shared-wgpu/NVIDIA:** use the same processor and parameters. Keep existing counted transfers and hardware identity rules unchanged.
- **Native direct CPU FFmpeg:** implement an equivalent, explicitly color-normalized RGB filter chain, with adequate intermediate precision for Main 10. Validate supported APIs against the pinned FFmpeg build. Restore required YUV/range/color metadata before encoding; tags alone do not transform pixels.
- **Native direct NVIDIA:** treat exact color-filter support as a capability gate. Preserve the current decode/resize/encode hardware path for neutral settings. Non-neutral settings require a verified hardware-capable implementation or an explicit unsupported-route explanation with a user-selected alternative. Never silently insert CPU filters/downloads, switch to another route, or claim new hardware compatibility. Any staged alternative must report transfers and be selected explicitly.

## Editable preview ownership and scheduling

On selecting an accepted source, request a small representative decoded frame and show it before conversion. Cache **one unadjusted frame for the current position**, so slider changes rerender it without reopening codecs, re-inspecting the file or accumulating adjustment on adjustment. Seek/file changes replace the cache; playback may replace it at an independently bounded preview rate. No whole-video frame cache or adjusted-frame persistence.

Keep pixels/textures in platform preview services, never Dioxus signals or worker command payloads. Commands contain settings, source/position identity and generation only. Coalesce slider events into one latest pending update, initially capped at 30 preview updates/s, with at most one render in flight. Preserve a source/seek/device generation and settings revision; discard stale decode/render results. Snapshot uniforms per submission, so updates cannot overwrite parameters still used by submitted work. Release old frames/textures only after required GPU completion. File replacement, close, cancellation, device loss and shutdown drain ownership.

Browser preview runs in the established worker or explicit main-thread fallback and reuses the current processing device. Keep preview targets separate from conversion/export backing dimensions; a slider or CSS resize must never resize an active export canvas. Conversion preempts/drains preview commands, captures an immutable settings snapshot, and disables editing while running. Preview is not the conversion scheduler and must never drop conversion frames.

Native preview reuses the existing bounded SDR decode/player service and original frame surface. Add a presenter adapter that reuses the shared WGSL/math without passing textures between Blitz's wgpu 26 and processing wgpu 30 devices. Investigate presenter-side adjustment first; do not introduce per-slider CPU readbacks simply to connect incompatible devices. Its preview decode/upload remains explicitly separate from conversion copy metrics. Validate its result against the selected conversion route; disclose approximation or disable a mismatched route instead of implying WYSIWYG parity.

## Implementation order and acceptance gates

1. **Contract and fixtures:** implement shared settings, transport/default compatibility and pure reference tests. Use CC0 ramps, neutral gray, primary/secondary patches, clipped shadows/highlights and textured input; document commands/hashes. Cover neutral identity, zero saturation, zero contrast, extreme brightness, combined order, invalid values, opaque alpha and bounded values.
2. **Shared GPU plus paused preview:** implement uniforms and browser preview, then native presenter adaptation. First prove neutral output unchanged and slider updates reuse the cached source frame. Validate both browser execution contexts and native dark/light layout. Reuse compact controls/preset vocabulary, but select widgets actually supported by Blitz. Include keyboard values and accessibility labels.
3. **Conversion integration and FFmpeg parity:** snapshot settings for all frames, add CPU filter mapping and NVIDIA capability reporting. Test neutral/non-neutral, resized/oriented material, original/VFR/fixed FPS, audio, and native Main 10 precision. Unsupported hardware adjustments must fail before output creation, with no silent fallback.
4. **End-to-end validation:** independently decode exported output and compare it with a same-position, same-geometry preview/reference. Establish and record pre-encoder numeric tolerances (target ≤2 code values/channel for an 8-bit reference); measure lossy-codec differences separately rather than loosening shader checks. Cover combined sliders and clipping, stale rapid seek/slider updates, Before/After, Reset, source replacement, cancel/retry, device loss, EOF, queue peaks and cleanup. Count decode/cache reuse and confirm no additional WebCodecs conversion readbacks. Check every frame's timing/count, audio coverage, geometry, range/color tags and depth before publication. Separate preview/update latency from conversion throughput.
5. **Optional CLAHE:** after steps 1–4 pass, implement the equalization contract, bounded GPU statistics/mappings, temporal/preview history policy and explicit direct-route capabilities. Run the separate equalization acceptance gate above, including all three sliders enabled at the same time. Do not delay the initial slider feature's completion to silently include an unvalidated equalization implementation.

Record actual browser/OS/GPU, routes/profiles, settings, decoded pixel errors, depth, timestamps, ownership peaks and copies in `interop-report.md`. Compilation is not preview/output correctness. Unknown routes or unavailable hardware remain **untested**, not implicitly supported. A before/after screenshot alone is not colorimetric or encoder validation. Long-run profiling, arbitrary DPI, physical A/V sync and new GPU/browser platforms remain separate coverage.

## Ready-to-start boundary

The first implementation slice is the shared contract/reference tests, neutral-compatible GPU adjustment and a bounded paused editable preview. Do not label the feature complete until conversion uses those same settings and route-specific acceptance gates pass. Dependency upgrades, native codec-surface interoperability and unrelated M5 work are not part of this plan.
