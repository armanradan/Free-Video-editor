# Spec Delta

## Purpose

Allow native users to process supported original legacy media directly while preserving verified presentation timing, audio synchronization and display aspect ratio.

## ADDED Requirements

### Requirement: Automatic isolated leading AAC timestamp normalization
The native app SHALL accept an original AAC source with an isolated incorrect first presentation timestamp when decode timestamps, packet durations and subsequent presentation timestamps establish one unambiguous correction. It MUST apply only that correction automatically and leave the source unchanged.

#### Scenario: Verified leading packet fault
- **WHEN** AAC at time base 1/44100 has initial PTS 3600, 1024, 2048 and DTS 0, 1024, 2048 with consistent durations and an otherwise valid complete timeline
- **THEN** selecting the original permits preview and conversion using first audio PTS 0 without a user-created repaired copy
- **AND** every later audio timestamp and every source file byte remains unchanged

#### Scenario: Valid timeline is retained
- **WHEN** a valid source has negative AAC priming, a positive audio offset, an intentional gap or an edit-list-derived delay
- **THEN** processing preserves that timing and does not replace the track timeline with decode timestamps or an uninterrupted sample clock

#### Scenario: Ambiguous corruption
- **WHEN** timestamps are missing, decode order is inconsistent or a fault cannot be uniquely classified as the supported isolated leading anomaly
- **THEN** the app reports the unsupported timing boundary without guessing, silently dropping audio or weakening synchronization verification

### Requirement: Missing frame-count metadata
Native inspection SHALL obtain an exact decoded frame count and presentation timeline for supported progressive MPEG-PS and ASF/WMV input even when container frame-count metadata is absent. Inspection MUST remain cancellable with bounded metadata memory and explicit resource limits.

#### Scenario: MPEG-1 with MP2 audio
- **WHEN** a progressive 8-bit SDR MPEG-PS file contains MPEG-1 video and MP2 audio but no declared frame count
- **THEN** inspection succeeds from decoded evidence and Original-FPS conversion preserves every decoded video frame and audio coverage

#### Scenario: Interrupted inspection
- **WHEN** inspection is cancelled or exceeds its declared limits
- **THEN** child processes terminate and no successful inspection or partial export is published

### Requirement: Preserve legacy display geometry
Native shared GPU processing SHALL normalize supported non-square pixel sources to square pixels using the source display aspect ratio before effects or AI. Preview and export MUST resolve the same geometry, and AI admission MUST use the actual normalized dimensions and unchanged resource limits.

#### Scenario: Rational pixel aspect
- **WHEN** a supported 352 by 288 source has sample aspect ratio 178:163
- **THEN** preview and export preserve its display aspect within documented integer rounding and apply 2x or 4x to the same normalized input dimensions
- **AND** output reports square pixels without stretching the source as though its coded pixels were square

#### Scenario: Normalized geometry exceeds RTX limits
- **WHEN** normalization would exceed the selected RTX scale or memory envelope
- **THEN** AI remains unavailable with the specific geometry or resource reason

### Requirement: Explicit native route compatibility
The native source picker SHALL expose MP4, MPG, MPEG and WMV originals. Both software and NVIDIA routes MUST accept supported progressive 8-bit SDR H.264/HEVC in MP4, MPEG-1/2 with MP2 in MPEG-PS, and WMV1/2/3 or VC-1 with supported WMA audio in ASF/WMV. The app MUST report normalization and actual decoder/encoder selection while retaining existing AI/device/resource gates.

#### Scenario: Software route across input families
- **WHEN** the user selects any accepted MP4, MPEG or WMV source with software codecs
- **THEN** direct software conversion works and shared software conversion can apply an accepted RTX scale
- **AND** audio is decoded and encoded into the MP4 output with verified timing

#### Scenario: NVIDIA encoding with software decoding
- **WHEN** an accepted MPEG or WMV source lacks a verified NVDEC path and the user selects NVIDIA
- **THEN** the app uses software decoding and NVIDIA encoding, with RTX processing when selected on a shared route
- **AND** it reports that decoder/encoder combination without rejecting the input solely because NVDEC is unavailable

#### Scenario: NVIDIA decoding is supported
- **WHEN** the selected source configuration has a verified NVDEC path on the selected GPU
- **THEN** the NVIDIA route uses that decoder and NVIDIA encoding and reports both accurately

#### Scenario: Encoder failure or unsupported source profile
- **WHEN** NVIDIA encoding fails or the input is encrypted or outside the accepted codec/profile envelope
- **THEN** the app reports the precise failing boundary without silently switching to software encoding or claiming that the entire container family is supported

### Requirement: Consistent normalized timeline and ownership
Preview, seeking, export and output verification SHALL use the same source-bound timing policy. The app MUST preserve valid A/V offsets and coverage, invalidate normalization when the source changes, bound internal scratch storage and clean owned temporary resources after failure or session retirement.

#### Scenario: Original source export and seek
- **WHEN** an accepted anomalous AAC original is previewed, sought and exported through a supported direct or shared route
- **THEN** each operation uses the established corrected audio origin and preserves the video timeline
- **AND** independently decoded output meets the existing synchronization and endpoint tolerances

#### Scenario: Replacement or failure
- **WHEN** the source changes or preparation/export is cancelled or fails
- **THEN** stale prepared audio cannot be used for the new source and owned intermediate files and processes are released
- **AND** insufficient scratch space produces an actionable error before conversion starts
