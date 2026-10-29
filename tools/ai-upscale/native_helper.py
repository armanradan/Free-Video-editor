"""Opt-in actual native tile/CLI diagnostic. All pixel transfers are explicit.

Requires existing verified assets, runtime, compiled helper and native CLI.
No downloads/installs. Refuses overwriting previous evidence.
"""
import argparse
import json
import struct
import subprocess
from pathlib import Path
import time

import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('helper', 'runtime', 'model', 'reference', 'cli', 'cache', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    base = [str(args.helper.resolve()), str(args.runtime.resolve()),
            str(args.model.resolve()), '1', 'NVIDIA GeForce RTX 3050 Laptop GPU']
    report = {'kind': 'explicit-native-pilot', 'cases': []}
    numeric = subprocess.run(base + [str((args.output / 'profile').resolve()), '--numeric',
                                    str(args.reference.resolve())], capture_output=True, timeout=90)
    (args.output / 'numeric.stdout').write_bytes(numeric.stdout)
    (args.output / 'numeric.stderr').write_bytes(numeric.stderr)
    numeric.check_returncode()
    report['identity'], report['numeric'] = [json.loads(line) for line in numeric.stdout.splitlines()]
    profiles = list(args.output.glob('profile*.json'))
    events = [event for path in profiles for event in json.loads(path.read_text(encoding='utf-8'))
              if event.get('args', {}).get('provider')]
    providers = sorted({event['args']['provider'] for event in events})
    assert providers == ['DmlExecutionProvider'], providers
    report['profile'] = {'providers': providers, 'node_events': len(events)}
    # Each invalid request runs in its own owned process. communicate waits/reaps.
    for name, request, reason in (
        ('wrong-version', struct.pack('<6I', 0x41495131, 1, 2, 0, 0, 0), 'Wrong protocol'),
        ('oversized-tile', struct.pack('<6I', 0x41495131, 2, 1, 197, 1, 197*12), 'Invalid bounded'),
        ('truncated-tile', struct.pack('<6I', 0x41495131, 2, 1, 1, 1, 12) + b'1234', 'Truncated'),
    ):
        result = subprocess.run(base + ['-'], input=request, capture_output=True, timeout=60)
        assert result.returncode != 0 and reason in result.stderr.decode(), (name, result.stderr)
        report['cases'].append({'case': name, 'reaped': True, 'error': result.stderr.decode().strip()})
    # Full envelope traverses the actual Rust shared-core tile plan and assembly.
    shape = (577, 579)
    reference = args.reference.parent / 'reference-envelope'
    source = np.load(reference / '577x579-input.npy')[0].transpose(1, 2, 0)
    rgba = np.concatenate((np.rint(source * 255).astype(np.uint8),
                           np.full((shape[1], shape[0], 1), 255, dtype=np.uint8)), axis=2)
    input_path = args.output / 'input.rgba'
    output_path = args.output / 'output.rgba'
    input_path.write_bytes(rgba.tobytes())
    started = time.perf_counter()
    result = subprocess.run([str(args.cli.resolve()), 'ai-frame-pilot', '--helper', base[0],
                             '--ort-directory', base[1], '--cache', str(args.cache.resolve()),
                             '--dxgi-index', '1', '--adapter-name', base[4], '--width', '577',
                             '--height', '579', '--rgba', str(input_path.resolve()),
                             '--output', str(output_path.resolve())], capture_output=True, timeout=90)
    (args.output / 'frame.stdout').write_bytes(result.stdout)
    (args.output / 'frame.stderr').write_bytes(result.stderr)
    result.check_returncode()
    frame = json.loads(result.stdout)
    expected = np.rint(np.clip(np.load(reference / '577x579-fp32.npy')[0], 0, 1)*255).astype(np.uint8).transpose(1, 2, 0)
    actual = np.frombuffer(output_path.read_bytes(), dtype=np.uint8).reshape(579*4, 577*4, 4)
    error = np.abs(actual[:, :, :3].astype(np.int16) - expected.astype(np.int16))
    assert error.max() <= 1 and np.all(actual[:, :, 3] == 255)
    # Independent arithmetic, no copy of the shared planner's crop implementation.
    staged_pixels = sum((min(x+128+34, 577)-max(x-34, 0)) *
                        (min(y+128+34, 579)-max(y-34, 0))
                        for y in range(0, 579, 128) for x in range(0, 577, 128))
    assert frame['host_to_gpu_bytes'] == staged_pixels*12
    assert frame['gpu_to_host_bytes'] == staged_pixels*12*16
    report['envelope'] = {'source': list(shape), 'tiles': 25, 'max_lsb': int(error.max()),
                          'mae_lsb': float(error.mean()), 'wall_seconds': time.perf_counter()-started,
                          **frame}
    report['status'] = 'passed'
    (args.output / 'result.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
