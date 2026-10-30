"""Fixture-only image gate, separate from timing/audio verification.

All supplied test excerpts contain spatial structure on every frame. This is not
a generic video-validity heuristic and must not reject users' legitimate flat
frames. Diagnostic 64x36 decode is excluded from production transfer metrics.
"""
import argparse
import json
from pathlib import Path
import subprocess


def ranges(file):
    raw = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(file), '-an',
        '-vf', 'scale=64:36', '-fps_mode', 'passthrough', '-pix_fmt', 'rgb24',
        '-f', 'rawvideo', 'pipe:1'], timeout=60)
    length = 64*36*3
    assert len(raw) % length == 0 and 0 < len(raw) <= length*150
    return [max(max(frame[c::3])-min(frame[c::3]) for c in range(3))
        for frame in (raw[i:i+length] for i in range(0, len(raw), length))]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', default='independent-structure.json')
    args = parser.parse_args()
    assert Path(args.output).name == args.output
    target = args.directory/args.output
    assert not target.exists(), 'Preserve prior evidence'
    matrix = json.loads((args.directory/'native.json').read_text(encoding='utf-8'))
    report = {'status': 'running', 'scope': 'Known structured fixtures only; not model fidelity, temporal flicker or generic flat-frame rejection', 'cases': []}
    try:
        for i, item in enumerate(matrix['cases']):
            source = Path(item['request']['fixture']).resolve()
            source_ranges = ranges(source)
            output_ranges = ranges(args.directory/matrix['artifacts'][i]['name'])
            row = {'source': str(source), 'source_channel_ranges': source_ranges, 'output_channel_ranges': output_ranges}
            report['cases'].append(row)
            assert min(source_ranges) > 32, 'Fixture has no guaranteed spatial-structure oracle'
            assert min(output_ranges) > 16, 'Structured fixture became flat during export'
        report['status'] = 'passed'
    except BaseException as error:
        report['status'] = 'failed'
        report['error'] = repr(error)
        raise
    finally:
        target.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({'status': report['status'], 'cases': len(report['cases'])}))


if __name__ == '__main__':
    main()
