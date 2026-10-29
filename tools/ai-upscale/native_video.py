"""Explicit native AI video pilot matrix; no downloads or per-frame files."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('cli', 'helper', 'cache', 'output'):
        parser.add_argument('--' + key, type=Path, required=True)
    parser.add_argument('--backend', choices=('onnx', 'rtx'), default='onnx')
    parser.add_argument('--runtime', type=Path)
    parser.add_argument('--sdk', type=Path)
    parser.add_argument('--luid-high', type=int)
    parser.add_argument('--luid-low', type=int)
    parser.add_argument('--route', choices=('wgpu', 'wgpu-nvidia'), default='wgpu')
    parser.add_argument('--rtx-effects', action='store_true', help='Evaluate validated RTX CLAHE50 + brightness10/contrast115/saturation80')
    parser.add_argument('--fixture', default='ai-upscale-vfr.mp4')
    args = parser.parse_args()
    if args.backend == 'onnx' and args.runtime is None:
        parser.error('--runtime is required for ONNX')
    if args.backend == 'rtx' and (args.sdk is None or args.luid_high is None or args.luid_low is None):
        parser.error('--sdk, --luid-high and --luid-low are required for RTX')
    effects = args.backend == 'onnx' or args.rtx_effects
    args.output.mkdir(parents=True, exist_ok=False)
    fixture = (Path('fixtures') / args.fixture).resolve()
    source_probe = json.loads(subprocess.check_output([
        'ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_frames',
        '-show_entries', 'frame=pts', '-of', 'json', str(fixture)], text=True))
    source_frames = len(source_probe['frames'])
    if source_frames == 0:
        raise ValueError('Expected a nonempty video fixture')
    adapters = json.loads(subprocess.check_output([str(args.cli), 'list-gpus']))
    name = 'NVIDIA GeForce RTX 3050 Laptop GPU'
    adapter = next(item for item in adapters if item['name'] == name and item['backend'] == 'Vulkan')
    evidence = {'status': 'running-not-release-accepted',
                'cli': str(args.cli.resolve()),
                'cli_sha256': hashlib.sha256(args.cli.read_bytes()).hexdigest(),
                'helper_sha256': hashlib.sha256(args.helper.read_bytes()).hexdigest(),
                'backend': args.backend, 'cases': [], 'artifacts': []}
    for scale in ('2x', '4x'):
        for fps in ('original', '15/1', '60/1'):
            index = len(evidence['cases'])
            name_out = f'video-native-{index}.mp4'
            command = [str(args.cli.resolve()), 'rtx-video-pilot' if args.backend == 'rtx' else 'ai-video-pilot',
                       '--helper', str(args.helper.resolve()), '--cache', str(args.cache.resolve()),
                       '--dxgi-index', '1', '--adapter-name', name, '--adapter-key', adapter['key'],
                       '--input', str(fixture), '--output', str((args.output/name_out).resolve()),
                       '--scale', scale, '--fps', fps, '--route', args.route]
            if args.backend == 'onnx':
                command += ['--ort-directory', str(args.runtime.resolve())]
            else:
                command += ['--rtx-sdk', str(args.sdk.resolve()), '--luid-high', str(args.luid_high),
                            '--luid-low', str(args.luid_low)]
            if effects:
                command += ['--equalization-strength', '50', '--brightness', '10', '--contrast', '115', '--saturation', '80']
            started = time.perf_counter()
            result = subprocess.run(command, capture_output=True, timeout=90)
            wall_seconds = time.perf_counter() - started
            (args.output/f'case-{index}.stdout').write_bytes(result.stdout)
            (args.output/f'case-{index}.stderr').write_bytes(result.stderr)
            result.check_returncode()
            report = json.loads(result.stdout)
            assert report['frames_processed'] == report['ai']['source_frames'] == source_frames
            assert report['equalization_compute_passes'] == (source_frames * 4 if effects else 0)
            assert report['ai']['identity']['cpu_fallback'] is False
            assert report['adapter']['key'] == adapter['key']
            if args.backend == 'rtx':
                identity = report['ai']['identity']
                assert identity['runtime'] == 'RTX Video SDK 1.1.0'
                assert identity['sdk_configuration'] == 'sdk-1.1-sdr-v1/high3'
                assert (identity['luid_high'],identity['luid_low']) == (args.luid_high,args.luid_low)
                assert report['ai']['gpu_to_host_bytes'] == report['ai']['host_to_gpu_bytes'] * int(scale[0])**2
            evidence['cases'].append({'request': {'fixture': str(fixture), 'scale': scale,
                                                  'fps': fps, 'clahe': effects, 'backend': args.backend},
                                      'inputFrameCount': report['frames_processed'],
                                      'aiSourceFrames': report['ai']['source_frames'],
                                      'wall_seconds': wall_seconds, 'command_argv': command, 'report': report})
            evidence['artifacts'].append({'name': name_out, 'bytes': (args.output/name_out).stat().st_size})
            (args.output/'native.json').write_text(json.dumps(evidence, indent=2)+'\n', encoding='utf-8')
            print(json.dumps({'case': index, 'scale': scale, 'fps': fps, 'status': 'passed',
                              'frames': report['output_frame_count']}), flush=True)
    # A partial matrix must never look complete if a later command/assert fails.
    evidence['status'] = 'native-video-pilot-passed-not-release-accepted'
    (args.output/'native.json').write_text(json.dumps(evidence, indent=2)+'\n', encoding='utf-8')


if __name__ == '__main__':
    main()
