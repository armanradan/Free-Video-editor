"""Adapt actual native handoff exports to the independent timing/audio verifier."""
import argparse
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    report = json.loads((args.directory / 'report.json').read_text(encoding='utf-8'))
    assert report['status'] == 'passed-diagnostic-handoff-not-ordinary-ai-acceptance'
    assert sorted(c['scale'] for c in report['cases']) == [2, 4]
    matrix = {'status': 'native-video-pilot-passed-not-release-accepted', 'cases': [], 'artifacts': []}
    for case in report['cases']:
        conversion = case['conversion']
        assert all(case[key] for key in (
            'active_preparation_retired', 'cancelled_conversion_unpublished',
            'fresh_paused_preview_pixels_equal', 'stale_request_rejected'))
        matrix['cases'].append({
            'request': {'fixture': 'ai-upscale-cut.mp4', 'scale': f"{case['scale']}x",
                        'fps': 'original', 'clahe': True},
            'report': conversion, 'inputFrameCount': conversion['frames_processed'],
            'aiSourceFrames': conversion['ai']['source_frames']})
        matrix['artifacts'].append({'name': f"{case['scale']}x.mp4"})
    destination = args.directory / 'native.json'
    with destination.open('x', encoding='utf-8') as stream:
        json.dump(matrix, stream, indent=2)
    subprocess.run(['node', str(Path(__file__).with_name('verify_video.mjs')),
                    str(args.directory)], check=True,
                   env=dict(os.environ, AI_UPSCALE_NATIVE_VIDEO='1'))


if __name__ == '__main__':
    main()
