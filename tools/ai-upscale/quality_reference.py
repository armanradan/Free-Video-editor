"""Offline matching-model/reference comparison on the acquired licensed clips.

CPU readbacks/uploads here are diagnostic. Timings include host IO and cannot be
compared to a browser GPU-resident pipeline or presented as product throughput.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

import numpy as np
from preview_pixels import resize
from reference import load_verified_model
from native_ort import dxgi_adapters


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('assets', 'fixtures', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--adapter-name', required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    source = json.loads((args.fixtures / 'manifest.json').read_text(encoding='utf-8'))
    report = {'status': 'running', 'scope': 'Offline CPU PyTorch versus untiled DirectML FP32; neutral x4 versus CPU bilinear visual baseline',
              'limits': ['No high-resolution ground-truth quality metric',
                         'CPU bilinear timing is an offline reference, not the shared GPU resize baseline',
                         'Inference measurements include explicit CPU input/output staging',
                         'The recorded clip lengths cover limited scenes, not universal temporal quality'], 'cases': []}
    session = None
    try:
        import torch
        import onnxruntime as ort
        if ort.__version__ != '1.23.0':
            raise ValueError('This evaluation requires the assessed ONNX Runtime 1.23.0')
        adapters = dxgi_adapters()
        selected = next(a for a in adapters if a['index'] == 1)
        if selected['name'] != args.adapter_name or selected['vendor_id'] != 0x10de:
            raise ValueError('DXGI ordinal 1 does not match the explicitly selected NVIDIA adapter')
        report['selected_adapter'] = selected
        report['pytorch_version'] = torch.__version__
        report['numpy_version'] = np.__version__
        model_path = args.assets / 'reference/model.onnx'
        expected_hash = '1793a6e7fdf15a53eed213ba269ea768b5373858ae433d56ee8e1b0424377cc5'
        if hashlib.sha256(model_path.read_bytes()).hexdigest() != expected_hash:
            raise ValueError('Untrusted model')
        started = time.perf_counter()
        model, _ = load_verified_model(args.assets)
        report['pytorch_load_seconds'] = time.perf_counter() - started
        options = ort.SessionOptions()
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        options.enable_mem_pattern = False
        options.add_session_config_entry('session.disable_cpu_ep_fallback', '1')
        options.enable_profiling = True
        options.profile_file_prefix = str((args.output / 'ort-profile').resolve())
        started = time.perf_counter()
        session = ort.InferenceSession(str(model_path), options,
                                       providers=[('DmlExecutionProvider', {'device_id': 1})])
        report['directml_load_seconds'] = time.perf_counter() - started
        report['runtime'] = ort.__version__
        report['providers'] = session.get_providers()
        report['device_id'] = 1
        report['model_sha256'] = expected_hash
        for clip in source['clips']:
            file = args.fixtures / clip['name']
            if hashlib.sha256(file.read_bytes()).hexdigest() != clip['sha256']:
                raise ValueError('Fixture changed after acquisition')
            width, height = clip['video']['width'], clip['video']['height']
            pixels = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(file), '-map', '0:v:0',
                                               '-pix_fmt', 'rgb24', '-f', 'rawvideo', '-'], timeout=30)
            frames = np.frombuffer(pixels, dtype=np.uint8).reshape(-1, height, width, 3)
            if len(frames) != clip['video_frames']:
                raise ValueError('Independent decoded frame count mismatch')
            case = {'fixture': clip['name'], 'frames': [], 'bilinear_seconds': 0,
                    'directml_seconds': 0, 'pytorch_seconds': 0}
            previous_ai = previous_bilinear = None
            with torch.inference_mode():
                for index, rgb in enumerate(frames):
                    tensor = np.ascontiguousarray(rgb.transpose(2, 0, 1)[None], dtype=np.float32) / 255
                    started = time.perf_counter()
                    trusted = model(torch.from_numpy(tensor)).numpy()
                    case['pytorch_seconds'] += time.perf_counter() - started
                    started = time.perf_counter()
                    actual = session.run(None, {session.get_inputs()[0].name: tensor})[0]
                    case['directml_seconds'] += time.perf_counter() - started
                    error = np.abs(actual - trusted)
                    maximum, mae = float(error.max()), float(error.mean())
                    if maximum > .01 or mae > .001:
                        raise ValueError(f'FP32 gate failed: {clip["name"]} frame {index}: {maximum}, {mae}')
                    ai = np.floor(actual[0].transpose(1, 2, 0).clip(0, 1) * 255 + .5).astype(np.uint8)
                    started = time.perf_counter()
                    bilinear = resize(rgb, width * 4, height * 4)
                    case['bilinear_seconds'] += time.perf_counter() - started
                    item = {'index': index, 'max_abs': maximum, 'mae': mae}
                    if previous_ai is not None:
                        # Uncompensated frame differences include genuine motion;
                        # report them descriptively, never as a flicker score.
                        item['ai_frame_difference_lsb'] = float(np.abs(ai.astype(float) - previous_ai).mean())
                        item['bilinear_frame_difference_lsb'] = float(np.abs(bilinear.astype(float) - previous_bilinear).mean())
                    previous_ai, previous_bilinear = ai, bilinear
                    case['frames'].append(item)
                    if index == 0:
                        comparison = np.concatenate((bilinear, ai), axis=1)
                        target = args.output / (file.stem + '-bilinear-left-model-right.png')
                        subprocess.run(['ffmpeg', '-v', 'error', '-n', '-f', 'rawvideo', '-pixel_format', 'rgb24',
                                        '-video_size', f'{width * 8}x{height * 4}', '-i', '-', '-frames:v', '1',
                                        '-update', '1', str(target)], input=comparison.tobytes(), check=True, timeout=30)
            report['cases'].append(case)
            print(json.dumps({'fixture': clip['name'], 'frames': len(case['frames']),
                              'max_abs': max(f['max_abs'] for f in case['frames'])}), flush=True)
        profile = Path(session.end_profiling())
        events = [e for e in json.loads(profile.read_text(encoding='utf-8')) if e.get('args', {}).get('provider')]
        providers = sorted({e['args']['provider'] for e in events})
        if providers != ['DmlExecutionProvider']:
            raise ValueError(f'Unexpected node providers: {providers}')
        report['profile'] = {'providers': providers, 'node_events': len(events)}
        report['status'] = 'passed-reference-not-release-accepted'
    except Exception as error:
        report['status'] = 'failed'
        report['error'] = str(error)
        raise
    finally:
        session = None
        (args.output / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8', newline='\n')


if __name__ == '__main__':
    main()
