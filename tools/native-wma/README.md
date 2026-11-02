# Embedded Windows WMA audio decoder

`crates/media-native/build.rs` builds `helper.cpp` with the installed MSVC x64
toolchain and Windows SDK; native Rust embeds that executable. No runtime download,
separate configuration or helper import is needed. The evaluated build used
MSVC14.51.36231 and Windows SDK10.0.26100.0, `/O2 /MT /W4 /WX`, linking only the
Windows system libraries mfplat, mfreadwrite, mfuuid and ole32. Browser targets
do not build this helper. Windows x64 is the evaluated host.

The helper reads only the selected source and sends PCM to stdout. COM and Media
Foundation are confined to this child process; the Rust crate retains its
`forbid(unsafe_code)` boundary. The existing child guard/cancellation watcher
terminates blocked source reads and reaps the process on error/cancel. Decoder
errors go to stderr. Original video and output encoding remain with FFmpeg.

Protocol v1 is little-endian: `DXWMA001`, four u32 fields (rate, channels,
integer bits/sample, block alignment), then records containing i64 PTS/duration
in100ns units and u32 payload length/flags followed by PCM. Flags0 means samples,
1 means explicit EOF (other fields zero),2 means a stream tick (no payload).
Supported PCM is mono/stereo,8–192kHz,16/24/32bit. This does not claim all WMA
profiles/channels or DRM support. Midstream format changes, backwards/negative
sample origins, invalid lengths/durations and overlaps beyond the evaluated2ms
ASF chunk jitter envelope reject. Gaps and PCM payloads are never flattened.

Rust retains one block (at most1MiB), writes timestamped PCM Matroska at100ns
time scale, then independently demuxes every block with ffprobe to verify exact
SHA256 payload, PTS, duration and size. It publishes only a fully verified plan.
The64MiB scratch cap includes the embedded helper and framing;32MiB free reserve
is required. Per-write checks enforce the cap if decoded length is unknown.
Weak source-stamped cache entries (at most32 live sources) allow consumers to
share one owner; the final owner removes only its own freshly reserved files.
Source replacement invalidates the plan. This costs audio-only disk I/O; long
sources can exceed the declared scratch envelope and reject explicitly.

PCM intermediates have no seek index. Playback linearly reads their bounded
contents and trims using absolute source timestamps before its existing audio
resampler, retaining genuine initial silence and gaps. Export subtracts the same
inspected common origin from video and prepared audio. Neither timestamps nor
sample payloads in the intermediate are rewritten to an uninterrupted clock.

Reproduction: `cargo test -p media-native legacy_audio --lib` and
`cargo test -p media-native windows_pcm_seek --lib`. For explicit local/private
sources, set absolute `DIAXUS_TEST_WMA_SOURCE` and a new absolute
`DIAXUS_TEST_WMA_REPORT`, then run the ignored
`actual_windows_wma_source_preparation` test. Independent export checking is in
`tools/native-legacy-inputs/verify_windows_wma.py`; its report excludes private
paths and PCM. The generated offset/gap control comes from
`wma_timing_fixture.py` (original FFmpeg testsrc2/sine, CC0).

Media Foundation API reference:
https://learn.microsoft.com/en-us/windows/win32/medfound/tutorial--decoding-audio
