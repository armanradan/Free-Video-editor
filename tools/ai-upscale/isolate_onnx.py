"""Produce operator-isolation graphs from the exact export; not replacement models."""
import hashlib
import json
from pathlib import Path
import sys

import numpy as np
import onnx
from onnx import helper, numpy_helper, TensorProto
from onnx.reference import ReferenceEvaluator

source, output = map(Path, sys.argv[1:])
expected = json.loads(Path(__file__).with_name('candidate.json').read_text())['onnx_export']['sha256']
assert hashlib.sha256(source.read_bytes()).hexdigest() == expected
graph = onnx.load(source)
weights = {item.name: item for item in graph.graph.initializer}
output.mkdir(parents=True, exist_ok=False)
report = []
for op in ['Conv', 'PRelu', 'DepthToSpace', 'Resize']:
    node = next(n for n in graph.graph.node if n.op_type == op)
    channels = 64 if op == 'PRelu' else 48 if op == 'DepthToSpace' else 3
    initializers = [weights[name] for name in node.input[1:] if name in weights]
    constants = [n for n in graph.graph.node if n.op_type == 'Constant' and n.output[0] in node.input]
    isolated = onnx.NodeProto()
    isolated.CopyFrom(node)
    isolated.input[0] = 'input'
    isolated.output[0] = 'output'
    isolated.name = f'isolate_{op}'
    model = helper.make_model(helper.make_graph(constants + [isolated], f'isolate_{op}',
        [helper.make_tensor_value_info('input', TensorProto.FLOAT, [1, channels, 'height', 'width'])],
        [helper.make_tensor_value_info('output', TensorProto.FLOAT, [1, 64 if op == 'Conv' else 3 if op == 'DepthToSpace' else channels, None, None])],
        initializer=initializers), opset_imports=[helper.make_opsetid('', 17)])
    model.ir_version = graph.ir_version
    onnx.checker.check_model(model)
    destination = output / f'{op}.onnx'
    onnx.save(model, destination)
    cases = []
    for width, height in [(1, 1), (17, 19)]:
        values = ((np.arange(channels * height * width) * 17) % 256).astype(np.float32)
        values = values.reshape(1, channels, height, width) / 255
        reference = ReferenceEvaluator(model).run(None, {'input': values})[0]
        stem = f'{op}-{width}x{height}'
        np.save(output / f'{stem}-input.npy', values, allow_pickle=False)
        np.save(output / f'{stem}-expected.npy', reference, allow_pickle=False)
        cases.append({'stem': stem, 'shape': list(values.shape), 'output_shape': list(reference.shape)})
    report.append({'operator': op, 'channels': channels, 'cases': cases,
                   'sha256': hashlib.sha256(destination.read_bytes()).hexdigest()})
(output / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
