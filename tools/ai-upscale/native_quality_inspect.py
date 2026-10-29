"""Bounded encoded-output temporal inspection for the native three-engine matrix."""
import argparse
import json
from pathlib import Path
import subprocess
import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    engines = ('bilinear', 'rtx', 'onnx')
    matrices = {e: json.loads((args.directory/e/'native.json').read_text(encoding='utf-8')) for e in engines}
    assert all(m['status'] == 'native-video-pilot-passed-not-release-accepted' for m in matrices.values())
    report = {'status': 'measured-visual-inspection-required', 'cases': [],
              'scope': 'Encoded RGB frame differences include genuine motion, CLAHE and compression; no motion compensation, high-resolution ground truth, intrinsic-flicker threshold or superiority claim. RTX High3 and Real-ESRGAN are different models.',
              'sheet_order': 'Bilinear above RTX above ONNX within each chronological cell; left-to-right, then next row.',
              'detail_order': 'Bilinear left, RTX middle, ONNX right; unchanged selected resolution.'}
    for index, base in enumerate(matrices['bilinear']['cases']):
        if base['request']['fps'] != 'original':
            continue
        items = [matrices[e]['cases'][index] for e in engines]
        assert len({(c['request']['fixture'], c['request']['scale'], c['request']['fps'], c['report']['video_bitrate_bps']) for c in items}) == 1
        files = [args.directory/e/matrices[e]['artifacts'][index]['name'] for e in engines]
        size = (base['report']['output_width'], base['report']['output_height'])
        count = base['inputFrameCount']
        assert count in (4, 64) and size[0] <= 768 and size[1] <= 432
        frames = []
        for file in files:
            raw = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(file), '-map', '0:v:0',
                                           '-pix_fmt', 'rgb24', '-fps_mode', 'passthrough', '-f', 'rawvideo', '-'], timeout=60)
            pixels = np.frombuffer(raw, dtype=np.uint8).reshape(count, size[1], size[0], 3)
            frames.append(pixels)
        differences = []
        for frame in range(1, count):
            deltas = [p[frame].astype(np.int16)-p[frame-1].astype(np.int16) for p in frames]
            differences.append({'index': frame,
                                **{e+'_delta_l1_lsb': float(np.abs(d).mean()) for e, d in zip(engines, deltas)},
                                'rtx_minus_bilinear_delta_l1_lsb': float(np.abs(deltas[1]-deltas[0]).mean()),
                                'onnx_minus_bilinear_delta_l1_lsb': float(np.abs(deltas[2]-deltas[0]).mean())})
        label = Path(base['request']['fixture']).stem+'-'+base['request']['scale']
        sheet = args.output/(label+'-all-frames.png')
        detail = args.output/(label+'-detail.png')
        inputs = [arg for file in files for arg in ('-i', str(file))]
        graph = '[0:v]scale=192:-2[b];[1:v]scale=192:-2[r];[2:v]scale=192:-2[o];[b][r][o]vstack=inputs=3,tile='+('8x8' if count == 64 else '4x1')+'[out]'
        subprocess.run(['ffmpeg', '-v', 'error', '-n', *inputs, '-filter_complex', graph,
                        '-map', '[out]', '-frames:v', '1', str(sheet)], check=True, timeout=60)
        selected = count//2
        graph = ';'.join(f'[{i}:v]select=eq(n\\,{selected})[{e}]' for i, e in enumerate(engines))
        graph += ';[bilinear][rtx][onnx]hstack=inputs=3[out]'
        subprocess.run(['ffmpeg', '-v', 'error', '-n', *inputs, '-filter_complex', graph,
                        '-map', '[out]', '-frames:v', '1', str(detail)], check=True, timeout=60)
        report['cases'].append({'fixture': base['request']['fixture'], 'scale': base['request']['scale'],
                                'frames': count, 'width': size[0], 'height': size[1], 'frame_differences': differences,
                                'sheet': sheet.name, 'detail': detail.name, 'detail_frame': selected})
        print(json.dumps({'fixture': label, 'frames': count, 'status': 'measured'}), flush=True)
    (args.output/'report.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')


if __name__ == '__main__':
    main()
