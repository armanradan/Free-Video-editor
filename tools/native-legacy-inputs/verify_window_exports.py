"""Independently check already-created window exports; never starts conversion.

The ignored manifest contains explicit source/output paths, expected size/frame
count and encoder. Report omits paths/private samples. Timing/audio oracles are
the same independent FFmpeg/Windows decoder checks used by the full matrix.
"""
import argparse
from array import array
from fractions import Fraction
import hashlib
import json
from pathlib import Path

from verify_windows_wma import (
    compare, ffmpeg_reference, pcm_timeline, probe, reference, run, video, video_end,
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--helper', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), 'Preserve existing evidence'
    results = []
    for case in json.loads(args.manifest.read_text(encoding='utf-8-sig')):
        source, output = Path(case['source']), Path(case['output'])
        identity = hashlib.sha256(source.read_bytes()).hexdigest()
        src, before = video(source)
        dst, after = video(output)
        src.update(probe(source, 'v:0')['streams'][0])
        dst.update(probe(output, 'v:0')['streams'][0])
        windows = src['codec_name'] in ('wmv1', 'wmv2', 'wmv3', 'vc1')
        rate, channels, bits, records = reference(args.helper, source) if windows else ffmpeg_reference(source)
        origin = min(before[0], Fraction(records[0][0], 10_000_000))
        assert len(before) == len(after) == case['frames']
        assert (dst['width'], dst['height']) == tuple(case['size'])
        assert dst['sample_aspect_ratio'] == '1:1'
        assert dst['tags']['encoder'].split()[-1] == case['encoder']
        if case['encoder'] == 'h264_nvenc':
            assert dst['decoded_b_frames'] == 0
        pts_error = max(abs(a - (b-origin)) for a, b in zip(after, before, strict=True))
        end_error = abs(video_end(output) - (video_end(source)-origin))
        assert pts_error <= Fraction(1, 1000) and end_error <= Fraction(1, 1000)
        expected, overlap = pcm_timeline(rate, channels, bits, records, origin)
        actual = array('f')
        actual.frombytes(run(['ffmpeg', '-v', 'error', '-copyts', '-i', output, '-map', '0:a:0',
            '-af', f'aresample={rate}:async=1:first_pts=0', '-ac', str(channels), '-ar', str(rate), '-f', 'f32le', '-']))
        lag, rmse = compare(expected, actual, channels, rate)
        audio_end = Fraction(records[-1][0]+records[-1][1], 10_000_000)-origin
        audio_error = abs(Fraction(len(actual), channels*rate)-audio_end)
        assert audio_error <= Fraction(25, 1000)
        run(['ffmpeg', '-v', 'error', '-xerror', '-i', output, '-map', '0:v:0', '-map', '0:a:0', '-f', 'null', '-'])
        assert identity == hashlib.sha256(source.read_bytes()).hexdigest()
        results.append(dict(label=case['label'], status='passed', frames=len(after), size=case['size'],
            encoder=case['encoder'], full_av_decode='passed', video_pts_max_error_seconds=float(pts_error),
            video_end_error_seconds=float(end_error), audio_end_error_seconds=float(audio_error),
            waveform_lag_samples=lag, waveform_rmse=rmse, quantized_chunk_overlap_max_samples=overlap,
            source_sha256=identity, output_sha256=hashlib.sha256(output.read_bytes()).hexdigest(),
            output_bytes=output.stat().st_size, audio_reference='Windows Media Foundation' if windows else 'FFmpeg'))
    args.report.write_text(json.dumps(dict(status='passed', scope='Already-created window outputs: complete AV decode, every video timestamp/end, independent PCM waveform/end, expected geometry/SAR/encoder. No new image-quality metric.', cases=results), indent=2)+'\n', encoding='utf-8')
    print(json.dumps(dict(status='passed', cases=len(results))))


if __name__ == '__main__':
    main()
