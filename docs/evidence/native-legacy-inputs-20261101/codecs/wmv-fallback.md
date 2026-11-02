# Approved WMV fallback and initial direct software results

**Superseded WMA timing interpretation:** these initial checks compare against
FFmpeg's own decoded clock. The later independent Windows decoder evaluation
matches PCM samples but supplies different source timing. Therefore these checks
do not establish correct WMA A/V alignment. The user subsequently approved the Windows WMA path;
its integrated acceptance is recorded in windows-wma.md; see `windows-decoder-reference.json` and
`wmapro-clock-boundary.json`. The approved video-only terminal fallback and its
observed-PTS guarantees remain valid independently.

The approved fallback is implemented only for WMV1/2/3 or VC-1 detected in ASF.
It requires at least three decoded frames, a positive declared rational rate
and a full constant grid. Cumulative positions, adjacent intervals and any
explicit durations must agree within less than one source tick. Periods shorter
than two ticks reject. The missing final period rounds to nearest tick, ties up.
No observed PTS is changed; present nonpositive durations reject. JSON exposes
`inferred_final_frame_duration_ticks`, and inspected UI metadata labels the estimate.
VFR inputs with an explicit final duration retain their observed timing.

Independent generated WMV1/WMA1 and WMV2/WMA2 direct exports pass:
30frames in/out,192x144,1.2s video, all video PTS exact, complete A/V decoding.
The inferred terminal period is40ticks at1/1000. `wmv-direct.json` records results.
Aligned floating-point PCM RMSE is below0.0003; waveform timing error is0.667ms,
and decoded padded audio endpoint error11.669ms. The independent waveform check
uses1ms ASF quantization and the endpoint check25ms. Production verification's
1ms video and25ms AAC coverage tolerances are unchanged. The first checker used
an unjustified16sample (~0.333ms) alignment budget and failed; diagnosis found
32sample offset against the timestamp-derived expectation. The revised1ms checker
reports this measured residual explicitly; its precise mux/decoder cause is not proven.

WMA inspection now observes full PCM frame timestamps/sample counts through the
ordinary FFmpeg decode pipeline with `-copyts -c:a pcm_s16le -f framecrc`; it
adds no setpts or clock-repair filter and creates no intermediate file. This
includes FFmpeg's ordinary codec-generated clocks when decoded WMA timestamps
are absent. It bounds metadata lines4096bytes and decoded frames2000000, checks
monotonic clocks, rejects overlaps beyond ASF1ms sample-clock quantization and
retains valid gaps. Reports include decoded sample coverage. This is an extra
CPU audio decode during inspection, not hardware execution or general repair.

Direct export now explicitly subtracts the inspected common A/V sample origin
instead of using the container's `start_at_zero` origin. This preserves observed
relative timing when ASF packet starts differ from decoded audio starts. The
complete20case AAC software/NVIDIA direct/shared route regression passes again.
Unknown-count video coverage uses the first actual decoded PTS when an ASF
header advertises an earlier start; raw source manifests retain that discrepancy.

Local private WMV3/WMA Pro, WMV3/WMA2(30fps) and WMV3/WMA2(25fps) two-second
excerpts pass native direct conversion; these are initial integrated results,
not the required independent family/image/RTX matrix. The private WMV2 excerpt
correctly rejects: its20frames include a1.3second presentation gap and an
additional67ms interval, with no final duration. The declared30fps does not
establish a constant grid. That VFR ending remains unsupported under the user's
explicitly approved policy; neither timestamp reconstruction nor tolerance
relaxation has been added to admit it.

The repeated upstream Advanced VC-1/WMA2 fixture also passes initial native direct
conversion, with actual decoded first PTS66ms (header0) and inferred20ms terminal
period from the validated50fps grid. This still does not test temporal/B-frame
VC-1 behavior. Broader color/profile handling, UI runtime verification, shared
geometry/RTX and NVIDIA hybrid routing remain unchecked tasks.
