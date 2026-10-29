"""Diagnostic DirectML evaluation with explicit DXGI adapter and no CPU fallback."""
import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path
import time
import uuid


def dxgi_adapters():
    # DXGI ordinals are the device_id contract of this DirectML provider.
    class Luid(c.Structure):
        _fields_ = [('low', c.c_uint32), ('high', c.c_int32)]

    class Desc(c.Structure):
        _fields_ = [('name', c.c_wchar * 128), ('vendor', c.c_uint32),
                    ('device', c.c_uint32), ('subsystem', c.c_uint32),
                    ('revision', c.c_uint32), ('video', c.c_size_t),
                    ('system', c.c_size_t), ('shared', c.c_size_t),
                    ('luid', Luid), ('flags', c.c_uint32)]

    def method(obj, index, result, *args):
        table = c.cast(obj, c.POINTER(c.POINTER(c.c_void_p))).contents
        return c.WINFUNCTYPE(result, c.c_void_p, *args)(table[index])

    factory = c.c_void_p()
    iid = (c.c_ubyte * 16).from_buffer_copy(
        uuid.UUID('770aae78-f26f-4dba-a829-253c83d1b387').bytes_le)
    create = c.WinDLL('dxgi').CreateDXGIFactory1
    create.argtypes = [c.c_void_p, c.POINTER(c.c_void_p)]
    create.restype = c.c_int32
    hr = create(c.byref(iid), c.byref(factory))
    if hr < 0:
        raise RuntimeError(f'CreateDXGIFactory1 HRESULT {hr & 0xffffffff:08x}')
    adapters = []
    try:
        for index in range(32):
            adapter = c.c_void_p()
            hr = method(factory, 12, c.c_int32, c.c_uint32, c.POINTER(c.c_void_p))(
                factory, index, c.byref(adapter))
            if hr & 0xffffffff == 0x887a0002:  # DXGI_ERROR_NOT_FOUND
                break
            if hr < 0:
                raise RuntimeError(f'EnumAdapters1 HRESULT {hr & 0xffffffff:08x}')
            try:
                desc = Desc()
                hr = method(adapter, 10, c.c_int32, c.POINTER(Desc))(adapter, c.byref(desc))
                if hr < 0:
                    raise RuntimeError(f'GetDesc1 HRESULT {hr & 0xffffffff:08x}')
                adapters.append({'index': index, 'name': desc.name,
                    'vendor_id': desc.vendor, 'device_id': desc.device,
                    'dedicated_video_bytes': desc.video, 'flags': desc.flags,
                    'luid': f'{desc.luid.high & 0xffffffff:08x}:{desc.luid.low:08x}'})
            finally:
                method(adapter, 2, c.c_uint32)(adapter)
        else:
            raise RuntimeError('DXGI enumeration exceeded diagnostic bound')
    finally:
        method(factory, 2, c.c_uint32)(factory)
    return adapters


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--adapter-name', required=True)
    args = parser.parse_args()
    manifest = json.loads(Path(__file__).with_name('candidate.json').read_text())
    model_hash = hashlib.sha256((args.reference / 'model.onnx').read_bytes()).hexdigest()
    if model_hash != manifest['onnx_export']['sha256']:
        raise ValueError('ONNX model hash is not allowlisted')
    import numpy as np
    import onnxruntime as ort
    adapters = dxgi_adapters()
    selected = [a for a in adapters if a['name'] == args.adapter_name and not a['flags'] & 2]
    if len(selected) != 1:
        raise ValueError(f'Expected one exact hardware adapter, found {selected}')
    args.output.mkdir(parents=True, exist_ok=False)
    report = {'runtime': ort.__version__, 'model_sha256': model_hash,
              'available_providers': ort.get_available_providers(),
              'adapters': adapters, 'selected': selected[0], 'cases': [],
              'transfers': 'Explicit CPU input -> DirectML GPU and GPU -> CPU output per run; diagnostic references only',
              'status': 'running'}
    session = None
    try:
        assert 'DmlExecutionProvider' in report['available_providers']
        options = ort.SessionOptions()
        options.enable_mem_pattern = False
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        options.add_session_config_entry('session.disable_cpu_ep_fallback', '1')
        options.enable_profiling = True
        options.profile_file_prefix = str(args.output / 'ort-profile')
        started = time.perf_counter()
        session = ort.InferenceSession(str(args.reference / 'model.onnx'), options,
            providers=[('DmlExecutionProvider', {'device_id': str(selected[0]['index'])})])
        session.disable_fallback()
        report['load_seconds'] = time.perf_counter() - started
        report['actual_providers'] = session.get_providers()
        report['actual_provider_options'] = session.get_provider_options()
        for item in json.loads((args.reference / 'reference.json').read_text())['cases']:
            stem = f"{item['width']}x{item['height']}"
            x = np.load(args.reference / f'{stem}-input.npy', allow_pickle=False)
            expected = np.load(args.reference / f'{stem}-fp32.npy', allow_pickle=False)
            started = time.perf_counter()
            output = session.run(['output'], {'input': x})[0]
            elapsed = time.perf_counter() - started
            assert output.shape == expected.shape
            error = np.abs(output - expected)
            assert np.isfinite(error).all()
            result = {'input': stem, 'shape': list(output.shape),
                      'max_abs': float(error.max()), 'mae': float(error.mean()),
                      'seconds': elapsed, 'host_input_bytes': x.nbytes,
                      'host_output_bytes': output.nbytes}
            report['cases'].append(result)
            assert result['max_abs'] <= .01 and result['mae'] <= .001, result
        profile = Path(session.end_profiling())
        providers = {}
        for event in json.loads(profile.read_text()):
            provider = event.get('args', {}).get('provider')
            if provider:
                providers[provider] = providers.get(provider, 0) + 1
        report['profile_provider_events'] = providers
        assert providers.get('DmlExecutionProvider', 0) > 0, providers
        assert set(providers) == {'DmlExecutionProvider'}, providers
        report['status'] = 'numeric-gpu-probe-passed-not-product-accepted'
    except Exception as error:
        report['status'] = 'failed'
        report['error'] = f'{type(error).__name__}: {error}'
        raise
    finally:
        # ORT Python owns the native session; release it before reporting completion.
        del session
        import gc
        gc.collect()
        report['cleanup'] = 'Python session reference released and GC collected; opaque driver reclamation unmeasured'
        (args.output / 'evidence.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
