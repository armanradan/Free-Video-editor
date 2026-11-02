# Complete native legacy codec matrix (2026-11-02)

All120 independently checked conversions pass:80 without AI and40 RTX2x jobs.
Inputs cover H.264/HEVC MP4, MPEG-1/2 with MP2, WMV1/WMA1, WMV2/WMA2,
WMV3 Main/WMA Pro, WMV3 Main/WMA2 and VC-1 Advanced/WMA2, plus a positive
audio-offset/interior-gap control. Every input runs at Original and15fps through
direct software, shared software, direct NVIDIA and shared NVIDIA. RTX uses
the two shared routes at both frame-rate settings. Separate hybrid evidence
also verifies MPEG-1 RTX4x. These are accepted dimensions/profiles, not every
possible codec profile or arbitrary HDR/interlaced/DRM input.

The five matrix JSON files retain native codec/adapter/SDK identity, source
count/geometry/color interpretation, actual transfer counters and independent
checks. Every output is fully decoded for A/V errors. Original FPS preserves
the complete decoded video PTS/count;15fps verifies the entire independently
calculated rational grid and presentation endpoints. WMA references use Windows
PCM/timing; AAC/MP2 use decoded FFmpeg sample clocks. Source SHA256/size/mtime
checks pass throughout. The independent audio waveform reference accounts for
the previously documented ASF chunk jitter; no production PTS/payload rewrite
is made. Existing1ms video/25ms audio verification limits are unchanged.

Each case includes an independent middle-frame RGB structure/geometry check
against nearby source frames scaled with FFmpeg bilinear conversion and source
matrix interpretation. Comparing nearby candidates permits intentional fixed-FPS
sampling; it does not omit the complete rational timestamp-grid check. The
diagnostic image gate is25RGB levels RMSE, observed maximum11.2382. These image
checks establish retained structure/aspect and broadly consistent color, not a
perceptual model-quality or temporal VC-1 claim. The local VC-1 fixture repeats
one upstream LGPL frame; its provenance/temporal limits remain in codec evidence.

`independent-contracts.json` additionally checks all120 outputs using actual
packet endpoints/dimensions, encoder tags and logical bridge bytes. NVIDIA RTX
outputs have no decoded reorder depth and all packet PTS equal DTS. Actual
NVENC tags identify h264_nvenc. H.264/HEVC NVIDIA plans use NVDEC; legacy NVIDIA
plans use software decode. NVDEC downloads are zero for those legacy plans;
NVENC upload counts follow actual pre-/post-FPS placement in each route.

`summary.json` records maxima: video PTS0.9071ms, video packet endpoint1ms,
audio endpoint18.5035ms, waveform RMSE0.005845. No tolerance was increased.
All complete A/V and source-identity checks pass. GPU/SDK environment remains
the evaluated RTX3050Laptop/Vulkan, driver621.01, SDK1.1 High3 configuration.
CPU staging/audio and opaque driver/SDK allocations remain; diagnostic work is
separate from conversion metrics. Runs co-ran builds/tests, so recorded times
are observations rather than controlled throughput benchmarks.

Reproduction uses `verify_windows_wma.py` (`--audio-clock ffmpeg` for AAC/MP2;
Windows helper for WMA), four `--route` choices, `--fps original|15`, `--images`;
`rtx_legacy_matrix.py` performs the40 explicitly licensed local SDK evaluations.
`verify_matrix_contracts.py` rechecks packet/encoder/copy contracts without
re-encoding. Scripts refuse existing result directories/files. Local media and
private source titles/paths are excluded from tracked evidence.
