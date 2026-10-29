"""Exact-model diagnostic exporter/reference and tile gate; never imported by the app.

The model source is loaded only after its allowlisted content hash is verified.
BasicSR's registry decorator/import alone are removed from the upstream AST; the
model definition and forward operation remain upstream code under its BSD license.
All image uploads/readbacks here are offline diagnostics, not production metrics.
"""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import time


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixture(width, height):
    """Original CC0 synthetic RGB fixture: edges, ramps, isolated points, texture."""
    import numpy as np
    y, x = np.indices((height, width), dtype=np.uint32)
    return np.stack(((x * 17 + y * 13) % 256,
                     ((x // 3 + y // 5) % 2) * 255,
                     (x * y * 7 + 31) % 256), axis=-1).astype(np.uint8)


def load_verified_model(assets):
    manifest = json.loads(Path(__file__).with_name('candidate.json').read_text())
    for asset in manifest['assets']:
        path = assets / asset['file']
        if sha256(path) != asset['sha256']:
            raise ValueError(f'Untrusted asset: {path}')
        if 'bytes' in asset and path.stat().st_size != asset['bytes']:
            raise ValueError(f'Wrong asset length: {path}')
    import torch
    torch.set_num_threads(4)
    torch.use_deterministic_algorithms(True)
    source = ast.parse((assets / 'srvgg_arch.py').read_text())
    source.body = [n for n in source.body if not
                   (isinstance(n, ast.ImportFrom) and n.module == 'basicsr.utils.registry')]
    for node in source.body:
        if isinstance(node, ast.ClassDef):
            node.decorator_list = []
    namespace = {}
    exec(compile(ast.fix_missing_locations(source), 'verified-srvgg_arch.py', 'exec'), namespace)
    model = namespace['SRVGGNetCompact'](num_in_ch=3, num_out_ch=3, num_feat=64,
                                       num_conv=32, upscale=4, act_type='prelu').eval()
    weights = torch.load(assets / 'realesr-general-x4v3.pth',
                         map_location='cpu', weights_only=True)
    key = 'params_ema' if 'params_ema' in weights else 'params'
    model.load_state_dict(weights[key], strict=True)
    return model, key


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--assets', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--tiles', action='store_true')
    parser.add_argument('--case', action='append', metavar='WIDTHxHEIGHT',
                        help='Generate only these diagnostic geometries (each dimension 1..1024)')
    args = parser.parse_args()
    geometries = [(1, 1), (3, 5), (17, 19), (33, 31), (257, 259)]
    if args.case:
        geometries = [tuple(map(int, geometry.split('x'))) for geometry in args.case]
        if any(len(geometry) != 2 or any(n < 1 or n > 1024 for n in geometry)
               for geometry in geometries):
            parser.error('Each case must be WIDTHxHEIGHT with dimensions in 1..1024')
    manifest = json.loads(Path(__file__).with_name('candidate.json').read_text(encoding='utf-8'))
    model, key = load_verified_model(args.assets)
    import numpy as np
    import onnx
    import psutil
    import torch
    # Refuse overwriting a previous result, including failed evaluations.
    args.output.mkdir(parents=True, exist_ok=False)
    report = {'candidate': manifest['id'], 'weight_key': key, 'versions': {
        'torch': torch.__version__, 'onnx': onnx.__version__, 'numpy': np.__version__},
        'reference_device': 'cpu', 'precision': 'float32', 'cases': [], 'tiles': []}

    def metrics(actual, expected, mask=None):
        error = np.abs(actual - expected)
        if mask is not None:
            error = error[:, :, mask]
        if not np.isfinite(error).all():
            raise ValueError('Nonfinite numeric result')
        return {'max_abs': float(error.max()), 'mae': float(error.mean()),
                'p99': float(np.quantile(error, .99))}

    def require(limits):
        assert limits['max_abs'] <= .01 and limits['mae'] <= .001, limits

    with torch.inference_mode():
        for width, height in geometries:
            pixels = fixture(width, height)
            x = torch.from_numpy(pixels.transpose(2, 0, 1).copy()).unsqueeze(0).float() / 255
            started = time.perf_counter()
            expected = model(x).numpy()
            inference_seconds = time.perf_counter() - started
            assert expected.shape == (1, 3, height * 4, width * 4)
            assert np.isfinite(expected).all()
            stem = f'{width}x{height}'
            np.save(args.output / f'{stem}-input.npy', x.numpy(), allow_pickle=False)
            np.save(args.output / f'{stem}-fp32.npy', expected, allow_pickle=False)
            (args.output / f'{stem}.ppm').write_bytes(
                f'P6\n{width} {height}\n255\n'.encode() + pixels.tobytes())
            report['cases'].append({'width': width, 'height': height,
                'output_shape': list(expected.shape), 'inference_seconds': inference_seconds,
                'input_sha256': sha256(args.output / f'{stem}-input.npy'),
                'reference_sha256': sha256(args.output / f'{stem}-fp32.npy'),
                'unclamped_min': float(expected.min()), 'unclamped_max': float(expected.max())})
            if args.tiles and width == 257:
                halo = manifest['tile_policy']['source_halo']
                previous_size = None
                for interior in manifest['tile_policy']['interiors']:
                    positions = [(top, left) for top in range(0, height, interior)
                                 for left in range(0, width, interior)]
                    previous = None
                    for reverse in [False, True]:
                        result = np.empty_like(expected)
                        started = time.perf_counter()
                        for top, left in (reversed(positions) if reverse else positions):
                            bottom, right = min(top + interior, height), min(left + interior, width)
                            y0, x0 = max(0, top - halo), max(0, left - halo)
                            y1, x1 = min(height, bottom + halo), min(width, right + halo)
                            tile = model(x[:, :, y0:y1, x0:x1].contiguous()).numpy()
                            result[:, :, top*4:bottom*4, left*4:right*4] = tile[
                                :, :, (top-y0)*4:(bottom-y0)*4, (left-x0)*4:(right-x0)*4]
                        elapsed = time.perf_counter() - started
                        numeric = metrics(result, expected)
                        require(numeric)
                        y, xx = np.indices((height*4, width*4))
                        edge = (y < 4) | (xx < 4) | (y >= height*4-4) | (xx >= width*4-4)
                        seam = np.zeros_like(edge)
                        for boundary in range(interior * 4, width * 4, interior * 4):
                            seam |= np.abs(xx - boundary) < 4
                        for boundary in range(interior * 4, height * 4, interior * 4):
                            seam |= np.abs(y - boundary) < 4
                        order = metrics(result, previous) if previous is not None else None
                        if order is not None:
                            require(order)
                        size_difference = metrics(result, previous_size) if previous_size is not None else None
                        if size_difference is not None:
                            require(size_difference)
                        report['tiles'].append({'interior': interior, 'halo': halo,
                            'reverse': reverse, 'tiles': len(positions), 'seconds': elapsed,
                            'numeric': numeric, 'edges': metrics(result, expected, edge),
                            'seams': metrics(result, expected, seam),
                            'interior_pixels': metrics(result, expected, ~(edge | seam)),
                            'order_difference': order, 'tile_size_difference': size_difference})
                        previous = result
                    previous_size = result
        onnx_path = args.output / 'model.onnx'
        torch.onnx.export(model, torch.zeros(1, 3, 17, 19), onnx_path,
            input_names=['input'], output_names=['output'], opset_version=17, dynamo=False,
            dynamic_axes={'input': {2: 'height', 3: 'width'},
                          'output': {2: 'height4', 3: 'width4'}})
        graph = onnx.load(onnx_path)
        onnx.checker.check_model(graph)
        assert sha256(onnx_path) == manifest['onnx_export']['sha256'], 'Export identity changed'
        report['onnx'] = {'sha256': sha256(onnx_path), 'bytes': onnx_path.stat().st_size,
            'opset': [{'domain': o.domain, 'version': o.version} for o in graph.opset_import],
            'operators': {op: sum(n.op_type == op for n in graph.graph.node)
                          for op in sorted({n.op_type for n in graph.graph.node})},
            'special_nodes': [{'op': n.op_type, 'attributes': {
                a.name: repr(onnx.helper.get_attribute_value(a)) for a in n.attribute}}
                for n in graph.graph.node if n.op_type in ['Resize', 'DepthToSpace']]}
        memory = psutil.Process().memory_info()
        report['cpu_process_memory'] = {'rss_bytes': memory.rss,
            'peak_working_set_bytes': getattr(memory, 'peak_wset', None),
            'scope': 'Whole CPU diagnostic process including references and export; not GPU memory'}
        report['status'] = 'reference-generated-not-a-runtime-pilot'
        (args.output / 'reference.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
