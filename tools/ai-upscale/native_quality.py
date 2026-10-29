"""Explicit release native RTX/Real-ESRGAN/bilinear comparison; existing assets only."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('cli', 'rtx-helper', 'onnx-helper', 'sdk', 'ngx-cache', 'runtime', 'model-cache', 'fixtures', 'output'):
        parser.add_argument('--' + key, type=Path, required=True)
    parser.add_argument('--luid-high', type=int, required=True)
    parser.add_argument('--luid-low', type=int, required=True)
    parser.add_argument('--engine', choices=('bilinear', 'rtx', 'onnx'), action='append')
    parser.add_argument('--fps', choices=('original', '15/1', '60/1'), action='append')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    source_manifest = json.loads((args.fixtures/'manifest.json').read_text(encoding='utf-8'))
    clips = []
    for clip in source_manifest['clips']:
        file = args.fixtures/clip['name']
        assert digest(file) == clip['sha256'], file
        assert clip['video_frames'] == 64
        clips.append((file.resolve(), 64))
    clips.append((Path('fixtures/ai-upscale-cut.mp4').resolve(), 4))
    name = 'NVIDIA GeForce RTX 3050 Laptop GPU'
    adapters = json.loads(subprocess.check_output([str(args.cli), 'list-gpus']))
    adapter = next(a for a in adapters if a['name'] == name and a['backend'] == 'Vulkan')
    summary = {'status': 'running-not-release-accepted', 'source_manifest': source_manifest,
               'adapter': adapter, 'cli_sha256': digest(args.cli),
               'rtx_helper_sha256': digest(args.rtx_helper), 'onnx_helper_sha256': digest(args.onnx_helper),
               'scope': 'Same source/output geometry, CLAHE50, brightness10/contrast115/saturation80, software libx264/AAC and bitrate/FPS policy. RTX High3 is a different model from Real-ESRGAN. CPU wall timers include waits/transfers; no pure GPU/codec timing claim.',
               'cases': [], 'artifacts': []}
    for engine in args.engine or ('bilinear', 'rtx', 'onnx'):
        directory = args.output/engine
        directory.mkdir()
        matrix = {'status': 'running-not-release-accepted', 'cases': [], 'artifacts': []}
        for file, frame_count in clips:
            for scale in ('2x', '4x'):
                for fps in args.fps or ('original', '15/1', '60/1'):
                    index = len(matrix['cases'])
                    output = directory/f'{index}.mp4'
                    command = [str(args.cli.resolve()), {'bilinear': 'bilinear-video-pilot', 'rtx': 'rtx-video-pilot', 'onnx': 'ai-video-pilot'}[engine],
                               '--input', str(file), '--output', str(output.resolve()), '--scale', scale,
                               '--fps', fps, '--route', 'wgpu', '--adapter-key', adapter['key'],
                               '--equalization-strength', '50', '--brightness', '10', '--contrast', '115', '--saturation', '80']
                    if engine != 'bilinear':
                        command += ['--helper', str((args.rtx_helper if engine == 'rtx' else args.onnx_helper).resolve()),
                                    '--cache', str((args.ngx_cache if engine == 'rtx' else args.model_cache).resolve()),
                                    '--dxgi-index', '1', '--adapter-name', name]
                    if engine == 'rtx':
                        command += ['--rtx-sdk', str(args.sdk.resolve()), '--luid-high', str(args.luid_high), '--luid-low', str(args.luid_low)]
                    elif engine == 'onnx':
                        command += ['--ort-directory', str(args.runtime.resolve())]
                    started = time.perf_counter()
                    result = subprocess.run(command, capture_output=True, timeout=120)
                    wall_seconds = time.perf_counter()-started
                    (directory/f'{index}.stderr').write_bytes(result.stderr)
                    (directory/f'{index}.stdout').write_bytes(result.stdout)
                    result.check_returncode()
                    report = json.loads(result.stdout)
                    assert report['frames_processed'] == frame_count
                    assert report['ai']['source_frames'] == (0 if engine == 'bilinear' else frame_count)
                    assert report['equalization_compute_passes'] == frame_count*4
                    assert report['adapter']['key'] == adapter['key']
                    times = report['gpu_wall_times']
                    assert times['postprocess_and_encoder_pipe_us'] > 0
                    if engine == 'bilinear':
                        assert report['ai']['identity'] is None
                        assert times['helper_startup_us'] == times['inference_roundtrip_us'] == 0
                    else:
                        assert report['ai']['identity']['cpu_fallback'] is False
                        assert times['helper_startup_us'] > 0 and times['inference_roundtrip_us'] > 0
                    if engine == 'rtx':
                        identity = report['ai']['identity']
                        assert identity['runtime'] == 'RTX Video SDK 1.1.0'
                        assert identity['sdk_configuration'] == 'sdk-1.1-sdr-v1/high3'
                        assert (identity['luid_high'], identity['luid_low']) == (args.luid_high, args.luid_low)
                        assert report['ai']['helper_memory_sampling_available']
                        assert report['ai']['sampled_helper_local_peak'] > 0
                    case = {'request': {'fixture': str(file), 'scale': scale, 'fps': fps, 'clahe': True, 'bilinear': engine == 'bilinear', 'engine': engine},
                            'inputFrameCount': frame_count, 'aiSourceFrames': report['ai']['source_frames'],
                            'wall_seconds': wall_seconds, 'command_argv': command, 'report': report}
                    matrix['cases'].append(case)
                    matrix['artifacts'].append({'name': output.name, 'bytes': output.stat().st_size})
                    (directory/'native.json').write_text(json.dumps(matrix, indent=2)+'\n', encoding='utf-8')
                    print(json.dumps({'engine': engine, 'fixture': file.stem, 'scale': scale, 'fps': fps, 'seconds': round(wall_seconds, 3)}), flush=True)
        matrix['status'] = 'native-video-pilot-passed-not-release-accepted'
        (directory/'native.json').write_text(json.dumps(matrix, indent=2)+'\n', encoding='utf-8')
        summary['cases'].extend(matrix['cases'])
        summary['artifacts'].extend({'name': str(Path(engine)/a['name']), 'bytes': a['bytes']} for a in matrix['artifacts'])
        (args.output/'summary.json').write_text(json.dumps(summary, indent=2)+'\n', encoding='utf-8')
    summary['status'] = 'measured-independent-video-and-visual-inspection-required'
    (args.output/'summary.json').write_text(json.dumps(summary, indent=2)+'\n', encoding='utf-8')


if __name__ == '__main__':
    main()
