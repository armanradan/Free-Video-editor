"""Independent CPU model/geometry/manual-color oracle for diagnostic video rasters.

Input is the explicitly read-back GPU ingress, not a CPU re-decode with potentially
different YUV conversion. No conversion-throughput claim uses these diagnostics.
"""
import argparse
import json
from pathlib import Path
import re
from reference import load_verified_model


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--assets', type=Path, required=True)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError('Do not overwrite prior diagnostic evidence')
    model, _ = load_verified_model(args.assets)
    import numpy as np
    import torch
    report = {'status': 'running', 'gate_max_rgba8_lsb': 2,
              'scope': 'first pre-encoder frame; independent CPU PyTorch x4, optional post-downscale, orientation and manual color; CLAHE excluded', 'cases': []}
    try:
        with torch.inference_mode():
            for context in ['main', 'worker']:
                data = json.loads((args.directory / f'{context}.json').read_text(encoding='utf-8'))
                for item in data['cases']:
                    request = item['request']
                    if request['clahe']:
                        raise ValueError('This oracle does not approximate CLAHE')
                    rotation, flip = re.search(r'rotation (\d+)°, flip=(true|false)', item['summary']).groups()
                    for frame in item['parityFrames']:
                        ingress = frame['ingress']
                        raw = np.frombuffer((args.directory / ingress['dataFile']).read_bytes(), dtype=np.uint8)
                        rgba = raw.reshape(ingress['height'], ingress['width'], 4)
                        # Forward texel permutation, independent of shader inverse UVs.
                        rgb = np.rot90(rgba[:, :, :3], k=-int(rotation)//90)
                        if flip == 'true':
                            rgb = np.fliplr(rgb)
                        tensor = torch.from_numpy(rgb.copy().transpose(2, 0, 1)[None]).float()/255
                        output = model(tensor).clamp(0, 1).numpy()[0].transpose(1, 2, 0)
                        output = np.floor(output*255+0.5)
                        if request['scale'] == '2x':
                            h, w = output.shape[:2]
                            output = np.floor(output.reshape(h//2, 2, w//2, 2, 3).mean(axis=(1, 3))+0.5)
                        b, c, s = map(float, request.get('color', '0/100/100').split('/'))
                        output /= 255
                        y = output @ np.array([.2126, .7152, .0722])
                        output = (y[:, :, None] + s/100*(output-y[:, :, None])-.5)*c/100+.5+b/100
                        output = np.floor(output.clip(0, 1)*255+.5).astype(np.uint8)
                        actual = np.frombuffer((args.directory/frame['dataFile']).read_bytes(), dtype=np.uint8)
                        actual = actual.reshape(frame['height'], frame['width'], 4)
                        if output.shape != actual[:, :, :3].shape or not (actual[:, :, 3] == 255).all():
                            raise ValueError('Wrong reference geometry/alpha')
                        error = np.abs(actual[:, :, :3].astype(np.int16)-output.astype(np.int16))
                        result = {'context': context, 'request': request, 'rotation': int(rotation), 'flip': flip == 'true',
                            'index': frame['index'], 'max_rgba8_lsb': int(error.max()), 'mae_rgba8_lsb': float(error.mean())}
                        report['cases'].append(result)
                        if result['max_rgba8_lsb'] > report['gate_max_rgba8_lsb']:
                            raise ValueError(f'Pixel reference gate failed: {result}')
        report['status'] = 'passed'
    except Exception as error:
        report['status'] = 'failed'
        report['error'] = str(error)
        raise
    finally:
        args.output.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8', newline='\n')


if __name__ == '__main__':
    main()
