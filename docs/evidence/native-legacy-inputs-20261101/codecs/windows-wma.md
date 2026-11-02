# Windows WMA audio integration (2026-11-01/02)

The user approved built-in Windows Media Foundation audio decoding/timing after
the independent reference exposed FFmpeg WMA clock differences. The application
now embeds a project-source-built helper, with no JSON import or runtime download.
FFmpeg still handles original video, lossless PCM demux and MP4 encoding. No
Windows COM/unsafe types enter Rust or media-core; the native Rust unsafe-code
prohibition remains. See `tools/native-wma/README.md` for build/protocol details.

Before publication, the application verifies every intermediate PCM payload,
size, PTS and duration against the Windows decoder at the exact100ns time base.
It preserves observed PTS and real gaps. Bounded scratch ownership/cancellation
and source-stamp revalidation apply to both playback and conversion. The native
metadata line identifies Windows audio decoding. This UI text is compiled and
tested, but new ordinary-window WMA interaction remains task6.1.

## Actual acceptance

`windows-wma-exports.json` contains ten direct/shared software exports: generated
WMV1/WMA1, WMV2/WMA2, private WMV3 Main/WMA Pro and WMV3 Main/WMA2 excerpts, and
the generated positive-offset/interior-gap control. `windows-vc1-exports.json`
adds direct/shared VC-1 Advanced/WMA2. All twelve outputs pass complete A/V
decoding, every video PTS/frame count and Windows-referenced audio waveform/end
checks. Frame counts are30,30,48,60,35 and30 respectively per route. Maximum
video PTS error is0.014ms; maximum audio endpoint error is18.503ms under the
unchanged25ms gate. Waveform RMSE is at most0.005845 normalized float amplitude;
all best lags are0 except37samples (~0.839ms) for the gap control, within the
independent1ms search envelope. Original hashes/size/mtime remain unchanged.

The audio waveform reference joins contiguous decoded samples within the
independently observed2ms ASF chunk jitter envelope, preserving larger gaps as
silence. Hard placement at every quantized boundary would invent/drop samples
in the reference waveform. This diagnostic policy does not alter production PCM
or timestamps; exact intermediate PTS/duration/payload checks are separate. AAC
encoder priming/padding explains bounded output tail differences. This is audio
clock/sample validation, not a perceptual listening study.

The new CC0 offset/gap control begins video at1.25s and Windows audio at1.35s;
its observed interior gap is149.5692ms. Its full sample/block manifest is
`wma-offset-gap-manifest.json`. This proves the path does not flatten all WMA
audio to an uninterrupted sample clock. The standalone playback command also
passes independently constructed PCM-marker tests for initial silence,150ms
gap, initial/forward/backward seek and tail silence, without using another copy
of the same FFmpeg filter as the expected result.

`windows-wmapro-rtx.json` records an actual48-frame WMA Pro excerpt RTX2x export:
normalized788x576 input,1576x1152 output, RTX3050Laptop/Vulkan, SDK1.1 High3,
driver621.01, evaluated helper SHA256
e9188624f8a4605f4429f83f8c74463a22db9bfd374a36f590a23f37cf5cac95.
The software-codec route uses libx264, not NVENC. Complete independent A/V checks
pass (video PTS error0, audio end9.274ms, waveform lag0/RMSE0.0033), total12023ms.
Explicit staged GPU transfers remain; this makes no real-time/zero-copy claim.

`windows-wmapro-full-preparation.json` additionally proves full private WMA Pro
audio preparation (not full video export):8,007,825 stereo24bit samples at44100Hz,
2402blocks,181.581723s endpoint,48,274,724 scratch bytes including helper/framing,
2510ms measured preparation. Every payload/PTS/duration verifies; source stamp
is unchanged and final-owner retirement removes scratch.

## Tested boundaries and limits

Windows x64, installed MSVC14.51.36231, Windows SDK10.0.26100.0, FFmpeg8.0.1
essentials. PCM mono/stereo8-192kHz integer16/24/32bit; negotiated format changes,
negative/backwards timestamps, inconsistent durations and more than2ms chunk
overlap reject.64MiB total scratch plus32MiB disk reserve,1MiB per PCM block,
2million protocol records,32live weak cache entries. Long audio can exceed the
scratch envelope and rejects explicitly. Windows decoder internal allocations
are opaque; application PCM/metadata retention is bounded.

WMV video final-duration fallback is unchanged: declared rate must match the
complete progressive timestamp grid; no observed PTS changes. Actual VFR endings
with missing duration still reject. WMV3 Main and progressive VC-1 Advanced are
the evaluated profiles; Simple/other/missing WMV3/VC-1 profiles reject. The local
VC-1 fixture repeats one upstream frame; it does not prove temporal/B-frame VC-1.
Source protection metadata and encrypted codec tags reject in policy tests;
no actual DRM/encrypted file was available. Actual generated interlaced MPEG-2
rejects by decoded evidence. Existing HDR/wide-color negative checks pass.

BT.601/709 limited-range metadata are interpreted before effects/AI. Untagged
legacy inputs assume BT.601 matrix and existing BT.709 working primaries/transfer,
explicitly recorded as assumptions rather than original mastering metadata.
A known YUV fixture checks actual601 pixels against independent matrix math
and the evaluated zscale display-referred BT.1886-to-sRGB transfer (within2LSB).
An initial test incorrectly used inverse709 camera OETF; primary zimg source
and actual runtime values established the display-referred EOTF reference:
https://github.com/sekrit-twc/zimg/blob/release-3.0.6/src/zimg/colorspace/gamma.cpp

Native/app tests pass82+20, with22+6explicit opt-in tests ignored; separately
executed full private preparation and actual RTX export are recorded above.
Formatting and strict OpenSpec validation pass. Hybrid NVIDIA route planning,
full fixed-FPS family matrix, new ordinary-window acceptance and integrated
failure/cancel/retry matrix remain pending in tasks4.3/5/6. No ai-upscale task
checkbox changed.
