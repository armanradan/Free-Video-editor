"""Actual GPU/pipe checks against the separately compiled, installed SDK sample.

No raw-frame sequence is persisted. Process deadlines kill/reap on any failure.
Reference tolerance is pinned before execution: exact RGBA8 byte equality.
"""
import argparse
import hashlib
import json
from pathlib import Path
import queue
import struct
import subprocess
import threading
import time
import numpy as np

REQUEST, RESPONSE = 0x31585452, 0x31595452
HEADER = struct.Struct('<10I')
MAX_OUTPUT = 192 * 108 * 16 * 4


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


class Helper:
    def __init__(self, exe, args, label, report):
        self.log_path = args.output / (label + '.jsonl')
        self.log = self.log_path.open('wb')
        self.process = subprocess.Popen([str(exe), str(args.sdk), str(args.dxgi_index),
            args.adapter_name, str(args.output / (label + '-cache'))], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=self.log, cwd=args.output,
            creationflags=subprocess.CREATE_NO_WINDOW)
        self.messages = queue.Queue(maxsize=2)
        self.report = report
        self.label = label
        self.started = time.perf_counter()

        def reader():
            try:
                while True:
                    raw = self.process.stdout.read(HEADER.size)
                    if not raw:
                        raise EOFError('helper stdout closed')
                    assert len(raw) == HEADER.size, 'truncated response'
                    h = HEADER.unpack(raw)
                    assert h[0:2] == (RESPONSE, 1) and h[9] <= max(8192,args.max_width*args.max_height*64), 'invalid bounded response'
                    payload = self.process.stdout.read(h[9])
                    assert len(payload) == h[9], 'truncated payload'
                    self.messages.put((h, payload), timeout=5)
            except Exception as error:
                try:
                    self.messages.put(error, timeout=5)
                except queue.Full:
                    pass

        self.reader = threading.Thread(target=reader, daemon=True)
        self.reader.start()
        try:
            h, data = self.receive()
            assert h[:9] == (RESPONSE, 1, 0, 0, 0, args.max_width, args.max_height, 0, 1) and 0 < len(data) <= 8192
            self.identity = json.loads(data)
            assert self.identity['pid'] == self.process.pid and self.identity['sdk'] == '1.1.0'
            assert self.identity['adapter_name'] == args.adapter_name and self.identity['dxgi_index'] == args.dxgi_index
        except Exception:
            self.finish(kill=True)
            raise

    def receive(self):
        value = self.messages.get(timeout=90)
        if isinstance(value, Exception):
            raise value
        return value

    def write(self, header, payload=b''):
        # A watchdog bounds even a stalled synchronous pipe write.
        watchdog = threading.Timer(90, self.process.kill)
        watchdog.start()
        try:
            self.process.stdin.write(HEADER.pack(*header) + payload)
            self.process.stdin.flush()
        finally:
            watchdog.cancel()

    def frame(self, pixels, scale, request_id, generation=7):
        height, width, channels = pixels.shape
        assert channels == 4
        self.write((REQUEST, 1, 1, request_id, generation, width, height, scale, 1, pixels.size), pixels.tobytes())
        h, data = self.receive()
        assert h == (RESPONSE, 1, 1, request_id, generation, width*scale, height*scale, scale, 1, pixels.size*scale*scale)
        result = np.frombuffer(data, dtype=np.uint8).reshape(height*scale, width*scale, 4)
        assert np.all(result[:, :, 3] == 255)
        assert result[:, :, :3].max() > 0
        return result

    def finish(self, kill=False, expected=0):
        try:
            if kill:
                self.process.kill()
            code = self.process.wait(timeout=90)
        except BaseException:
            self.process.kill()
            self.process.wait(timeout=10)
            raise
        finally:
            self.process.stdin.close()
            self.process.stdout.close()
            self.log.close()
            self.reader.join(timeout=6)
        records = [json.loads(line) for line in self.log_path.read_text(encoding='utf-8').splitlines() if line.startswith('{')]
        self.report['runs'].append({'label': self.label, 'exit_code': code,
            'wall_seconds_including_startup': time.perf_counter() - self.started, 'records': records})
        if not kill:
            assert code == expected, (self.label, code, records)
        return records

    def close(self):
        self.write((REQUEST, 1, 2, 0, 0, 0, 0, 0, 0, 0))
        h, payload = self.receive()
        assert h == (RESPONSE, 1, 2, 0, 0, 0, 0, 0, 0, 0) and not payload
        return self.finish()


def pattern(width, height, variant):
    y, x = np.indices((height, width), dtype=np.uint32)
    return np.stack(((x*(7 if variant else 3)+y*5) % 256,
        ((x//5+y//7+variant) % 2)*180+30, (y*9+x*2+variant*103) % 256,
        np.full_like(x, 255)), axis=2).astype(np.uint8)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--helper', type=Path, required=True)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--sdk', type=Path, required=True)
    parser.add_argument('--dxgi-index', type=int, required=True)
    parser.add_argument('--negative-adapter-index', type=int, required=True)
    parser.add_argument('--adapter-name', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--max-width', type=int, default=192)
    parser.add_argument('--max-height', type=int, default=108)
    parser.add_argument('--size', action='append', help='Additional WxH reference case, retaining original regression cases')
    parser.add_argument('--steady-frames', type=int, default=0, help='Additional fixed 1280x720 2x frames to exercise stable resource reuse')
    args = parser.parse_args()
    for key in ['helper', 'reference', 'sdk']:
        setattr(args, key, getattr(args, key).resolve(strict=True))
    args.output = args.output.resolve()
    args.output.mkdir(exist_ok=False)
    report = {'status': 'running', 'tolerance': {'rgba8_max_error': 0, 'rgba8_mae': 0},
        'scope': 'bounded helper + installed SDK sample, not video/UI/product acceptance',
        'helper_sha256': digest(args.helper), 'reference_sha256': digest(args.reference),
        'sample_source_sha256': digest(args.sdk / 'samples/RTX_Video_API/rtx_video_api_dx11_impl.cpp'),
        'runs': [], 'cases': []}
    active = []
    try:
        helper = Helper(args.helper, args, 'helper', report); active.append(helper)
        reference = Helper(args.reference, args, 'sample', report); active.append(reference)
        request_id = 0
        # Sequential comparison retains just one input and two outputs.
        sizes = [(192, 108), (191, 107), (64, 36)] + [tuple(map(int,size.split('x'))) for size in args.size or []]
        for width, height in sizes:
            for scale in [2, 4]:
                for variant in [0, 1]:
                    request_id += 1
                    source = pattern(width, height, variant)
                    result = helper.frame(source, scale, request_id)
                    trusted = reference.frame(source, scale, request_id)
                    delta = np.abs(result.astype(np.int16)-trusted.astype(np.int16))
                    row = {'width': width, 'height': height, 'scale': scale, 'pattern': variant,
                        'output_sha256': hashlib.sha256(result).hexdigest(),
                        'max_error': int(delta.max()), 'mae': float(delta.mean())}
                    report['cases'].append(row)
                    assert row['max_error'] == 0, row
        if args.steady_frames:
            assert args.max_width >= 1280 and args.max_height >= 720
            source = pattern(1280, 720, 0)
            expected = hashlib.sha256(helper.frame(source, 2, request_id + 1)).hexdigest()
            trusted = next(row for row in report['cases'] if row['width'] == 1280
                and row['height'] == 720 and row['scale'] == 2 and row['pattern'] == 0)
            assert expected == trusted['output_sha256']
            request_id += 1
            for _ in range(args.steady_frames - 1):
                request_id += 1
                assert hashlib.sha256(helper.frame(source, 2, request_id)).hexdigest() == expected
            report['steady_frames'] = args.steady_frames
            report['steady_output_sha256'] = expected
        helper_records = helper.close(); active.remove(helper)
        reference_records = reference.close(); active.remove(reference)
        for records, extra_frames in [(helper_records, args.steady_frames), (reference_records, 0)]:
            identity = next(r for r in records if r.get('stage') == 'ready')
            assert identity['adapter_name'] == args.adapter_name and identity['dxgi_index'] == args.dxgi_index
            assert identity['vendor_id'] == 0x10de and identity['in_flight'] == 1
            modules = [r for r in records if r.get('stage') == 'loaded-runtime' and (r['name'].lower() == 'nvngx_vsr.dll' or '\\nvidia\\ngx\\models\\vsr\\' in r['path'].lower())]
            paths = {r['path'] for r in modules}
            assert len(paths) == 1, paths
            assert digest(Path(next(iter(paths)))) == digest(args.sdk / 'bin/Windows/x64/rel/nvngx_vsr.dll')
            closed = next(r for r in records if r.get('stage') == 'closed')
            expected_uploads=sum(w*h*4*4 for w,h in sizes)
            extra_uploads = extra_frames * 1280 * 720 * 4
            assert closed['frames'] == len(sizes)*4 + extra_frames and closed['host_to_gpu_bytes'] == expected_uploads + extra_uploads
            assert closed['gpu_to_host_bytes'] == expected_uploads*10 + extra_uploads*4 and closed['sampled_local_peak'] > 0
        assert all(any(r.get('stage') == stage and r['ngx_result'] == '0x00000001' for r in helper_records)
                   for stage in ['release-feature', 'destroy-parameters', 'shutdown-device'])
        first = next(r for r in helper_records if r.get('stage') == 'ready')
        second = next(r for r in reference_records if r.get('stage') == 'ready')
        for field in ['device_id', 'luid_high', 'luid_low', 'color', 'quality']:
            assert first[field] == second[field], field
        for label, mutate, payload, expected in [
            ('bad-version', {1:2}, b'', 'magic/version'),
            ('oversized', {5:0xffffffff, 9:0xffffffff}, b'', 'geometry/scale/color/length'),
            ('bad-color', {8:2}, b'', 'geometry/scale/color/length'),
            ('bad-scale', {7:3}, b'', 'geometry/scale/color/length'),
            ('bad-length', {9:1}, b'', 'geometry/scale/color/length'),
            ('zero-generation', {4:0}, b'', 'operation/id/generation'),
            ('nonopaque', {}, bytes(64*36*4), 'nonopaque'),
            ('truncated-payload', {}, bytes(8), 'truncated frame payload'),
        ]:
            process = Helper(args.helper, args, label, report); active.append(process)
            header = [REQUEST, 1, 1, 1, 7, 64, 36, 2, 1, 64*36*4]
            for index, value in mutate.items(): header[index] = value
            process.write(header, payload)
            process.process.stdin.close()
            records = process.finish(expected=1); active.remove(process)
            assert expected in records[-1]['error'], records
            assert not any(r.get('stage') == 'evaluate' for r in records)
            assert all(any(r.get('stage') == stage and r['ngx_result'] == '0x00000001' for r in records)
                       for stage in ['release-feature', 'destroy-parameters', 'shutdown-device'])
        for label, generation, request_id in [('changed-generation', 8, 2), ('repeated-id', 7, 1)]:
            process = Helper(args.helper, args, label, report); active.append(process)
            source = pattern(64, 36, 0)
            process.frame(source, 2, 1)
            process.write((REQUEST, 1, 1, request_id, generation, 64, 36, 2, 1, source.size))
            records = process.finish(expected=1); active.remove(process)
            assert 'operation/id/generation' in records[-1]['error']
            assert sum(r.get('stage') == 'evaluate' for r in records) == 1
        process = Helper(args.helper, args, 'truncated-header', report); active.append(process)
        process.process.stdin.write(b'RTX'); process.process.stdin.close()
        records = process.finish(expected=1); active.remove(process)
        assert 'truncated header' in records[-1]['error']
        process = Helper(args.helper, args, 'clean-eof', report); active.append(process)
        process.process.stdin.close()
        h, data = process.receive()
        assert h[2] == 2 and not data
        process.finish(); active.remove(process)
        process = Helper(args.helper, args, 'fresh-retry', report); active.append(process)
        pixels = process.frame(pattern(192, 108, 0), 4, 1)
        assert hashlib.sha256(pixels).hexdigest() == next(r['output_sha256'] for r in report['cases']
            if r['width'] == 192 and r['scale'] == 4 and r['pattern'] == 0)
        process.close(); active.remove(process)
        empty_sdk = args.output / 'empty-sdk'; empty_sdk.mkdir()
        for label, sdk, index, reason in [
            ('wrong-adapter', args.sdk, args.negative_adapter_index, 'identity mismatch'),
            ('missing-runtime', empty_sdk, args.dxgi_index, 'missing SDK runtime'),
        ]:
            started = time.perf_counter()
            result = subprocess.run([str(args.helper), str(sdk), str(index), args.adapter_name,
                str(args.output / (label+'-cache'))], input=b'', capture_output=True, timeout=90,
                creationflags=subprocess.CREATE_NO_WINDOW, cwd=args.output)
            (args.output / (label+'.jsonl')).write_bytes(result.stderr)
            records = [json.loads(line) for line in result.stderr.decode('utf-8').splitlines() if line.startswith('{')]
            report['runs'].append({'label': label, 'exit_code': result.returncode,
                'wall_seconds_including_startup': time.perf_counter()-started, 'records': records})
            assert result.returncode == 1 and not result.stdout and reason in records[-1]['error']
            assert not any(r.get('stage') == 'initialize' for r in records)
        report['status'] = 'helper-sample-gate-passed-not-product-accepted'
    except BaseException as error:
        report['status'] = 'failed'
        report['error'] = repr(error)
        raise
    finally:
        for process in active:
            if process.process.poll() is None: process.finish(kill=True)
        (args.output / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({'status': report['status'], 'cases': len(report['cases']), 'runs': len(report['runs'])}))


if __name__ == '__main__':
    main()
