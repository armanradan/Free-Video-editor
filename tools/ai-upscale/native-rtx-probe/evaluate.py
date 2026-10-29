"""Run and independently inspect the explicit local RTX D3D11 feasibility probe."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
import numpy as np


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--sdk', type=Path, required=True)
    parser.add_argument('--dxgi-index', type=int, required=True)
    parser.add_argument('--adapter-name', required=True)
    parser.add_argument('--negative-adapter-index', type=int, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert 0 <= args.dxgi_index <= 15
    assert 0 <= args.negative_adapter_index <= 15
    assert args.dxgi_index != args.negative_adapter_index
    args.probe = args.probe.resolve(strict=True)
    args.sdk = args.sdk.resolve(strict=True)
    args.output = args.output.resolve()
    args.output.mkdir(exist_ok=False)
    report = {'status': 'running', 'scope': 'Synthetic SDR SDK feasibility only; no video, preview or product acceptance',
              'probe': str(args.probe), 'probe_sha256': sha256(args.probe), 'sdk': str(args.sdk),
              'runs': [], 'cases': []}

    def run(label, sdk, index, extra=(), expected=0):
        command = [str(args.probe), str(sdk), str(index), args.adapter_name,
                   str(args.output / label), *extra]
        started = time.perf_counter()
        result = subprocess.run(command, capture_output=True, timeout=120)
        elapsed = time.perf_counter() - started
        (args.output / (label + '.stdout.jsonl')).write_bytes(result.stdout)
        (args.output / (label + '.stderr.txt')).write_bytes(result.stderr)
        records = [json.loads(line) for line in result.stdout.decode('utf-8').splitlines() if line.strip()]
        report['runs'].append({'label': label, 'command': command, 'exit_code': result.returncode,
                               'wall_seconds': elapsed, 'records': records})
        assert result.returncode == expected, (label, result.returncode, records, result.stderr.decode('utf-8', errors='replace'))
        return records

    try:
        references = {}
        for label in ['first', 'repeat']:
            records = run(label, args.sdk, args.dxgi_index)
            assert records[-1]['status'] == 'pilot-passed-not-product-accepted'
            assert records[-1]['cases'] == 4
            assert records[-1]['input_uploads'] == 4
            assert records[-1]['diagnostic_readbacks'] == 12
            for stage in ['initialize', 'capabilities', 'vsr-available-query', 'scratch-size',
                          'create-vsr', 'release-feature', 'destroy-parameters', 'shutdown-device']:
                rows = [r for r in records if r.get('stage') == stage]
                assert len(rows) == 1 and rows[0]['ngx_result'] == '0x00000001', (stage, rows)
            evaluations = [r for r in records if r.get('stage') == 'evaluate']
            assert len(evaluations) == 8 and all(r['ngx_result'] == '0x00000001' for r in evaluations)
            adapter = next(r for r in records if r.get('stage') == 'adapter')
            assert adapter['dxgi_index'] == args.dxgi_index and adapter['name'] == args.adapter_name
            assert adapter['vendor'] == 0x10de
            vsr = [r for r in records if r.get('stage') == 'loaded-runtime' and r['name'] == 'nvngx_vsr.dll']
            assert len(vsr) == 1
            assert Path(vsr[0]['path']).resolve() == (args.sdk / 'bin/Windows/x64/rel/nvngx_vsr.dll').resolve()
            outputs = [r for r in records if r.get('stage') == 'output']
            assert len(outputs) == 8
            arrays = {}
            for item in outputs:
                assert item['scale'] in [2, 4] and item['pattern'] in [0, 1] and item['quality'] in [0, 3]
                assert (item['width'], item['height']) == (192 * item['scale'], 108 * item['scale'])
                file = args.output / label / item['file']
                assert file.parent == args.output / label
                assert file.stat().st_size == item['bytes'] == item['width'] * item['height'] * 4
                pixels = np.frombuffer(file.read_bytes(), dtype=np.uint8).reshape(item['height'], item['width'], 4)
                assert np.all(pixels[:, :, 3] == 255), 'nonopaque output'
                assert int(pixels[:, :, :3].sum()) == item['rgb_sum'] > 0
                digest = sha256(file)
                if label == 'first':
                    references[item['file']] = digest
                else:
                    assert references[item['file']] == digest, 'repeat output differs'
                arrays[item['scale'], item['pattern'], item['quality']] = pixels
                report['cases'].append({'run': label, **item, 'sha256': digest, 'alpha_min': 255, 'alpha_max': 255})
            for scale in [2, 4]:
                assert not np.array_equal(arrays[scale, 0, 3], arrays[scale, 1, 3])
                for pattern in [0, 1]:
                    delta = np.abs(arrays[scale, pattern, 3][:, :, :3].astype(np.int16)
                                   - arrays[scale, pattern, 0][:, :, :3].astype(np.int16))
                    assert delta.max() > 0, 'enhancement indistinguishable from SDK bicubic'
                    report.setdefault('sdk_bicubic_differences', []).append({
                        'run': label, 'scale': scale, 'pattern': pattern,
                        'maximum_lsb': int(delta.max()), 'mae_lsb': float(delta.mean()),
                        'different_rgb_channels': int(np.count_nonzero(delta))})
        records = run('wrong-adapter', args.sdk, args.negative_adapter_index, expected=1)
        assert 'identity mismatch' in records[-1]['error']
        assert not any(r.get('stage') == 'initialize' for r in records)
        empty = args.output / 'empty-sdk'
        empty.mkdir()
        records = run('missing-runtime', empty, args.dxgi_index, expected=1)
        assert 'missing SDK runtime' in records[-1]['error']
        assert not any(r.get('stage') == 'initialize' for r in records)
        records = run('missing-input', args.sdk, args.dxgi_index, ['--missing-input-test'], expected=1)
        error = next(r for r in records if r.get('stage') == 'evaluate-missing-resources')
        assert error['ngx_result'] == '0xbad00002', 'preserve actual NGX platform error, not generic unsupported GPU'
        for stage in ['release-feature', 'destroy-parameters', 'shutdown-device']:
            assert any(r.get('stage') == stage and r['ngx_result'] == '0x00000001' for r in records)
        assert not any(r.get('stage') == 'output' for r in records)
        records = run('fresh-after-failure', args.sdk, args.dxgi_index)
        assert records[-1]['status'] == 'pilot-passed-not-product-accepted'
        for r in records:
            if r.get('stage') == 'output':
                assert sha256(args.output / 'fresh-after-failure' / r['file']) == references[r['file']]
        report['runtime_sha256'] = {r['name']: sha256(Path(r['path'])) for r in records if r.get('stage') == 'loaded-runtime'}
        report['status'] = 'sdk-feasibility-passed-not-product-accepted'
        report['limitations'] = ['Different NVIDIA model from Real-ESRGAN; differences are not quality improvement proof',
                                'Synthetic clips only; no video PTS/audio/codec/native UI acceptance',
                                'No SDK-demo pixel equivalence or proprietary redistribution review',
                                'No performance, opaque memory, spontaneous loss or hardware support beyond selected device claim',
                                'Explicit D3D11 host upload and diagnostic readback; not a codec-surface bridge',
                                'High and SDK Bicubic run sequentially on one feature; temporal state not independently characterized']
    except Exception as error:
        report['status'] = 'failed'
        report['error'] = str(error)
        raise
    finally:
        (args.output / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps({'status': report['status'], 'runs': len(report['runs']), 'checked_outputs': len(report['cases'])}))


if __name__ == '__main__':
    main()
