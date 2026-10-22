# Spec Delta

## Purpose

Enable explicit AI video enlargement with validated processing backends while preserving predictable geometry, media timing, preview behavior and bounded resource ownership across browser and native applications.

## ADDED Requirements

### Requirement: AI processing is explicit and disabled by default
The system SHALL default AI upscaling to Off, including when loading older settings without an AI field. Enabling AI SHALL require an explicit backend and supported enlargement. With AI Off, existing resizing and its no-implicit-upscale restriction MUST remain unchanged.

#### Scenario: Existing settings remain compatible
- **WHEN** a saved command lacks AI settings or the user leaves AI Off
- **THEN** it uses the existing resize, codec, color and timing behavior without loading an inference runtime or model

#### Scenario: Ordinary resize cannot silently become AI enlargement
- **WHEN** AI is Off and requested dimensions exceed the normalized source
- **THEN** the job is rejected with the existing enlargement restriction rather than invoking AI automatically

### Requirement: Backends are platform-specific and validation-gated
The system SHALL distinguish portable model inference from NVIDIA RTX Video processing. Only validated platform/backend/model/format/route combinations SHALL be selectable. Unavailable candidates MUST carry a reason and MUST NOT be silently replaced by another engine, model, GPU or CPU fallback.

#### Scenario: Candidate has not passed its gates
- **WHEN** a candidate runtime cannot import the selected model or has no release evidence for the platform
- **THEN** it is unavailable with a specific reason and is not advertised as supported

#### Scenario: RTX is native only
- **WHEN** the browser application lists AI choices
- **THEN** it does not offer RTX SDK execution or depend on a local helper

#### Scenario: Native RTX device is unavailable
- **WHEN** the selected native device or installed SDK/runtime cannot execute RTX upscaling
- **THEN** the app reports the failed requirement and does not switch GPU or run Real-ESRGAN instead

### Requirement: AI output geometry is resolved before processing
The system SHALL initially offer validated 2x and 4x enlargements of the normalized, oriented source, preserving aspect ratio. It MUST display codec-safe output dimensions and reject overflow, unsupported geometry, device/memory limits and unavailable exact encoder configurations before conversion. AI geometry MUST NOT weaken ordinary resize validation.

#### Scenario: Explicit enlargement
- **WHEN** a supported 2x AI configuration is selected for a 640x360 source
- **THEN** the resolved output is 1280x720 and codec support and bitrate estimates use those output dimensions

#### Scenario: Excessive output request
- **WHEN** a 4x result exceeds the selected backend's proven geometry or resource limits
- **THEN** conversion is unavailable with a specific limit reason rather than silently reducing size or changing the model

### Requirement: Model and runtime acquisition is deliberate and verifiable
The system SHALL identify required assets, versions, provenance, license notices and download sizes before explicit acquisition. It MUST verify model integrity before use, reject corrupted/incompatible assets, and avoid downloading models merely because the app starts. Valid cached assets SHALL be reusable without uploading source media.

#### Scenario: First use requires weights
- **WHEN** the user selects a model that is not cached
- **THEN** the app explains the required download and waits for an explicit acquisition action before fetching it

#### Scenario: Corrupt cache
- **WHEN** cached weights fail the expected integrity check
- **THEN** they are not executed and the app offers an explicit recovery action

#### Scenario: Native SDK installation is missing
- **WHEN** RTX is selected without its required runtime components
- **THEN** the app gives installation guidance without automatically accepting third-party terms or redistributing unreviewed SDK binaries

### Requirement: Processing preserves source timing and audio policy
AI conversion SHALL preserve every source frame by default and carry its checked timestamps, durations and geometry through inference. Explicit fixed output FPS SHALL retain existing duplicate/drop and audio-coverage semantics without changing playback speed. The completed output MUST pass independent decode and timing/audio checks before acceptance evidence is claimed.

#### Scenario: Original variable frame rate
- **WHEN** a supporting codec route converts a VFR source with AI and Original FPS
- **THEN** it preserves the source-frame timeline and audio relationship rather than replacing them with an assumed frame rate

#### Scenario: Fixed FPS with downstream temporal effects
- **WHEN** fixed FPS duplicates or drops output frames while CLAHE is enabled
- **THEN** each source frame advances the upstream processing/history once and duplicated outputs do not run AI or advance history again

### Requirement: Preview uses the selected AI policy independently of export availability
Source preview SHALL use the selected backend, model, scale and color settings, applying display scaling only after selected-output processing. Before SHALL bypass AI and color effects for comparison without modifying saved conversion settings. Preview readiness MUST remain independent of output encoder availability.

#### Scenario: Paused parameter edit
- **WHEN** the user changes Strength or a manual color slider at a fixed position after AI preparation
- **THEN** the preview reuses the cached AI result when its upstream identity is unchanged and displays the revised downstream effects

#### Scenario: Encoder unavailable but preview supported
- **WHEN** source decoding and AI preview are supported but the selected output profile or FPS is not
- **THEN** preview and Play/Pause remain usable while conversion displays the export restriction

#### Scenario: Conversion snapshots settings
- **WHEN** the user starts conversion and later changes preview selections
- **THEN** the active job retains its original immutable AI/color settings

### Requirement: Combined effects and precision have explicit capabilities
AI SHALL run before CLAHE and manual saturation, contrast and brightness, leaving those controls independent. Unsupported combinations MUST be rejected before conversion, not omitted or approximated silently. Initial AI support SHALL be opaque 8-bit SDR; 10-bit/HDR inputs or outputs MUST NOT be silently reduced to that baseline.

#### Scenario: Combined settings supported
- **WHEN** a validated backend/route supports AI, CLAHE and all three manual controls
- **THEN** preview and export apply the declared order and Before/After changes only the comparison view

#### Scenario: Unsupported RTX effect bridge
- **WHEN** RTX upscaling cannot compose with the chosen CLAHE route
- **THEN** the app explains the incompatible combination rather than dropping CLAHE or changing processing backend

#### Scenario: Main 10 selected
- **WHEN** initial AI processing is requested with a 10-bit source or output profile
- **THEN** it is explicitly unavailable until that complete precision path has been separately validated

### Requirement: Inference resource use is bounded and observable
The system SHALL bound tile work, in-flight frames, retained preview results and application-controlled allocations using checked budgets. It MUST process without persisting a raw-frame sequence and report application-visible allocation/transfer counts separately from opaque runtime memory and test-only readbacks. Memory exhaustion MUST fail explicitly without unannounced CPU fallback.

#### Scenario: Large image requires tiling
- **WHEN** a validated model cannot fit a full frame within its configured budget
- **THEN** bounded overlapping tiles produce the output without retaining a full sequence of decoded frames

#### Scenario: Runtime budget exceeded
- **WHEN** a known allocation exceeds the checked inference budget
- **THEN** processing rejects it with a resource error and does not continue accumulating work

### Requirement: Cancellation and device changes retire inference safely
Cancellation, source replacement, seek invalidation and device loss SHALL prevent stale results from publication, drain or retire submitted work safely, and release owned frames, tensors and sessions. A retry MUST use a healthy context with compatible resource identity. Pause MUST remain responsive and MUST NOT restart playback solely because preview processing failed.

#### Scenario: Cancel during tiled inference
- **WHEN** the user cancels while a tile or GPU submission is active
- **THEN** no more source input is submitted, stale results are ignored, partial output is unpublished and owned resources are released after safe retirement

#### Scenario: Device loss followed by explicit retry
- **WHEN** the processing device is lost during AI work
- **THEN** the failed job reports loss and an explicit retry constructs or obtains a healthy generation rather than reusing invalid tensors

#### Scenario: Seek replaces active preview preparation
- **WHEN** a newer source or seek supersedes an AI preview request
- **THEN** only the current request's result can replace the displayed image and pending requests remain bounded

### Requirement: Color reset and AI disable have separate scopes
Reset color SHALL disable CLAHE and restore neutral manual sliders without changing AI backend/model/scale, FPS, bitrate or profile selections. Explicit AI Off MUST release AI-owned caches after safe drain and restore the retained ordinary resize selection and its existing no-upscale validation.

#### Scenario: Reset color during AI preview
- **WHEN** the user resets color while 2x AI is enabled
- **THEN** AI remains enabled at 2x while CLAHE and the manual controls return to neutral

#### Scenario: Disable AI after an enlargement
- **WHEN** the user explicitly turns AI Off
- **THEN** ordinary resize selections return unchanged and any invalid enlargement in those selections is rejected rather than clamped

### Requirement: Backend acceptance is evidence-based
Every selectable AI combination SHALL have reproducible reference-image, tile-seam, video timing/audio, preview/export and lifecycle evidence, plus an identified quality/performance evaluation. Runtime comparisons MUST use the same portable model and preprocessing; RTX comparisons MUST identify its distinct model/settings. Untested combinations SHALL remain explicitly unverified.

#### Scenario: A backend imports but produces incorrect images
- **WHEN** model import succeeds but numerical or tile-boundary checks fail
- **THEN** that combination does not pass the selectable-backend release gate

#### Scenario: A backend cannot be tested on available hardware
- **WHEN** the target GPU/browser/runtime is unavailable
- **THEN** its result is recorded as untested rather than a passing skip or evidence of universal support
