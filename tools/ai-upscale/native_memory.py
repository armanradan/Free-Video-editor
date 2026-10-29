"""Isolated DirectML tile/reference probe with sampled per-process DXGI usage.

DXGI reports this process's usage, not global nvidia-smi totals. Sampling can miss
transient peaks and does not provide a strict allocator bound or video acceptance.
"""
import argparse
import ctypes as c
import gc
import hashlib
import json
from pathlib import Path
import threading
import time
import uuid

from native_ort import dxgi_adapters


class VideoMemorySampler:
    def __init__(self, ordinal):
        self.ordinal = ordinal
        self.stop = threading.Event()
        self.ready = threading.Event()
        self.lock = threading.Lock()
        self.error = None
        self.latest = None
        self.peaks = {'local': 0, 'nonlocal': 0}
        self.samples = 0
        self.thread = threading.Thread(target=self._run, daemon=True)

    def _run(self):
        factory, adapter, adapter3 = c.c_void_p(), c.c_void_p(), c.c_void_p()

        def method(obj, index, result, *args):
            table = c.cast(obj, c.POINTER(c.POINTER(c.c_void_p))).contents
            return c.WINFUNCTYPE(result, c.c_void_p, *args)(table[index])

        def check(hr):
            if hr < 0:
                raise RuntimeError(f'DXGI HRESULT {hr & 0xffffffff:08x}')

        class Info(c.Structure):
            _fields_ = [(name, c.c_uint64) for name in
                        ('budget', 'current_usage', 'available_for_reservation', 'current_reservation')]

        try:
            iid = (c.c_ubyte * 16).from_buffer_copy(
                uuid.UUID('770aae78-f26f-4dba-a829-253c83d1b387').bytes_le)
            create = c.WinDLL('dxgi').CreateDXGIFactory1
            create.argtypes = [c.c_void_p, c.POINTER(c.c_void_p)]
            create.restype = c.c_int32
            check(create(c.byref(iid), c.byref(factory)))
            check(method(factory, 12, c.c_int32, c.c_uint32, c.POINTER(c.c_void_p))(
                factory, self.ordinal, c.byref(adapter)))
            iid3 = (c.c_ubyte * 16).from_buffer_copy(
                uuid.UUID('645967a4-1392-4310-a798-8053ce3e93fd').bytes_le)
            check(method(adapter, 0, c.c_int32, c.c_void_p, c.POINTER(c.c_void_p))(
                adapter, c.byref(iid3), c.byref(adapter3)))
            query = method(adapter3, 14, c.c_int32, c.c_uint32, c.c_int32, c.POINTER(Info))
            while True:
                sample = {}
                for group, name in [(0, 'local'), (1, 'nonlocal')]:
                    info = Info()
                    check(query(adapter3, 0, group, c.byref(info)))
                    sample[name] = {field: getattr(info, field) for field, _ in info._fields_}
                with self.lock:
                    self.latest = sample
                    self.samples += 1
                    for name in self.peaks:
                        self.peaks[name] = max(self.peaks[name], sample[name]['current_usage'])
                self.ready.set()
                if self.stop.wait(.025):
                    break
        except Exception as error:
            self.error = f'{type(error).__name__}: {error}'
            self.ready.set()
        finally:
            for obj in (adapter3, adapter, factory):
                if obj.value:
                    method(obj, 2, c.c_uint32)(obj)

    def __enter__(self):
        self.thread.start()
        if not self.ready.wait(10):
            self.stop.set()
            raise RuntimeError('DXGI sampling startup timed out')
        self.snapshot()
        return self

    def snapshot(self):
        if self.error:
            raise RuntimeError(self.error)
        with self.lock:
            return {'latest': self.latest, 'sampled_peak_bytes': dict(self.peaks),
                    'samples': self.samples, 'nominal_interval_ms': 25}

    def __exit__(self, *_):
        self.stop.set()
        self.thread.join(10)
        if self.thread.is_alive():
            raise RuntimeError('DXGI sampler did not retire')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--adapter-name', required=True)
    args = parser.parse_args()
    manifest = json.loads(Path(__file__).with_name('candidate.json').read_text())
    model_hash = hashlib.sha256((args.reference / 'model.onnx').read_bytes()).hexdigest()
    if model_hash != manifest['onnx_export']['sha256']:
        raise ValueError('ONNX model is not allowlisted')
    reference = json.loads((args.reference / 'reference.json').read_text())
    # Fixed reproducible calibration, not an arbitrary media/model loader.
    if [(i['width'], i['height']) for i in reference['cases']] != [(577, 579)]:
        raise ValueError('Expected the bounded 577x579 calibration reference')
    for item in reference['cases']:
        for suffix, field in [('input', 'input_sha256'), ('fp32', 'reference_sha256')]:
            file = args.reference / f"{item['width']}x{item['height']}-{suffix}.npy"
            if hashlib.sha256(file.read_bytes()).hexdigest() != item[field]:
                raise ValueError(f'Reference integrity failed: {file.name}')
    import numpy as np
    import onnxruntime as ort
    import psutil
    selected = [a for a in dxgi_adapters() if a['name'] == args.adapter_name and not a['flags'] & 2]
    if len(selected) != 1 or 'DmlExecutionProvider' not in ort.get_available_providers():
        raise RuntimeError('Exact hardware adapter/DirectML provider unavailable')
    args.output.mkdir(parents=True, exist_ok=False)
    report = {'runtime': ort.__version__, 'model_sha256': model_hash, 'selected': selected[0],
              'status': 'running', 'calibrations': [], 'admission_local_bytes': 1536*1024*1024,
              'memory_scope': 'DXGI per-process local/nonlocal sampled usage; 25 ms sampling may miss peaks. Not a strict allocator cap or whole-video budget.'}
    session = None
    with VideoMemorySampler(selected[0]['index']) as sampler:
        try:
            report['before_session'] = sampler.snapshot()
            options = ort.SessionOptions()
            options.enable_mem_pattern = False
            options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
            options.add_session_config_entry('session.disable_cpu_ep_fallback', '1')
            options.enable_profiling = True
            options.profile_file_prefix = str(args.output / 'ort-profile')
            session = ort.InferenceSession(str(args.reference / 'model.onnx'), options,
                providers=[('DmlExecutionProvider', {'device_id': str(selected[0]['index'])})])
            session.disable_fallback()
            report['actual_providers'] = session.get_providers()
            report['after_session_creation'] = sampler.snapshot()
            x = np.load(args.reference / '577x579-input.npy', allow_pickle=False)
            expected = np.load(args.reference / '577x579-fp32.npy', allow_pickle=False)
            height, width = x.shape[2:]

            def metrics(actual, other, mask=None):
                error = np.abs(actual - other)
                if mask is not None:
                    error = error[:, :, mask]
                if not np.isfinite(error).all():
                    raise ValueError('Nonfinite result')
                values = {'max_abs': float(error.max()), 'mae': float(error.mean())}
                if values['max_abs'] > .01 or values['mae'] > .001:
                    raise ValueError(f'Reference tolerance exceeded: {values}')
                return values

            previous_size = None
            for interior in (128, 256):
                previous_order = None
                for reverse in (False, True):
                    result = np.empty_like(expected)
                    positions = [(top, left) for top in range(0, height, interior)
                                 for left in range(0, width, interior)]
                    if reverse:
                        positions.reverse()
                    record = {'interior': interior, 'halo': 34, 'reverse': reverse,
                              'tiles': 0, 'host_upload_bytes': 0, 'host_readback_bytes': 0,
                              'max_tile_width': 0, 'max_tile_height': 0}
                    report['calibrations'].append(record)
                    started = time.perf_counter()
                    for top, left in positions:
                        memory = sampler.snapshot()['latest']['local']
                        if memory['current_usage'] > min(report['admission_local_bytes'], memory['budget']):
                            raise RuntimeError('Sampled local-memory admission budget exceeded')
                        bottom, right = min(top+interior, height), min(left+interior, width)
                        y0, x0 = max(0, top-34), max(0, left-34)
                        y1, x1 = min(height, bottom+34), min(width, right+34)
                        tile_input = np.ascontiguousarray(x[:, :, y0:y1, x0:x1])
                        tile = session.run(['output'], {'input': tile_input})[0]
                        if tile.shape != (1, 3, (y1-y0)*4, (x1-x0)*4):
                            raise ValueError('Wrong tile geometry')
                        result[:, :, top*4:bottom*4, left*4:right*4] = tile[
                            :, :, (top-y0)*4:(bottom-y0)*4, (left-x0)*4:(right-x0)*4]
                        record['tiles'] += 1
                        record['host_upload_bytes'] += tile_input.nbytes
                        record['host_readback_bytes'] += tile.nbytes
                        record['max_tile_width'] = max(record['max_tile_width'], x1-x0)
                        record['max_tile_height'] = max(record['max_tile_height'], y1-y0)
                        del tile, tile_input
                    record['inference_and_host_assembly_seconds'] = time.perf_counter()-started
                    record['numeric'] = metrics(result, expected)
                    y, xx = np.indices((height*4, width*4))
                    edge = (y < 4) | (xx < 4) | (y >= height*4-4) | (xx >= width*4-4)
                    seam = np.zeros_like(edge)
                    for boundary in range(interior*4, width*4, interior*4):
                        seam |= np.abs(xx-boundary) < 4
                    for boundary in range(interior*4, height*4, interior*4):
                        seam |= np.abs(y-boundary) < 4
                    record['edges'] = metrics(result, expected, edge)
                    record['seams'] = metrics(result, expected, seam)
                    if previous_order is not None:
                        record['order_difference'] = metrics(result, previous_order)
                    if previous_size is not None:
                        record['tile_size_difference'] = metrics(result, previous_size)
                    previous_order = result
                    record['memory'] = sampler.snapshot()
                    print(json.dumps(record), flush=True)
                previous_size = previous_order
            profile = Path(session.end_profiling())
            providers = {}
            for event in json.loads(profile.read_text()):
                provider = event.get('args', {}).get('provider')
                if provider:
                    providers[provider] = providers.get(provider, 0) + 1
            report['profile_provider_events'] = providers
            if not providers or set(providers) != {'DmlExecutionProvider'}:
                raise RuntimeError(f'Unexpected node provider placement: {providers}')
            report['status'] = 'tiled-memory-numeric-pilot-not-product-accepted'
        except Exception as error:
            report['status'] = 'failed'
            report['error'] = f'{type(error).__name__}: {error}'
            raise
        finally:
            session = None
            gc.collect()
            # Give the sampler an opportunity to observe session retirement.
            time.sleep(.1)
            report['after_session_release'] = sampler.snapshot()
            memory = psutil.Process().memory_info()
            report['host_peak_working_set_bytes'] = getattr(memory, 'peak_wset', None)
            (args.output / 'evidence.json').write_text(json.dumps(report, indent=2)+'\n')


if __name__ == '__main__':
    main()
