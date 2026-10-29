"""Bounded offline inspection of encoded AI/bilinear sequences, not a fidelity metric."""
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
    data = json.loads((args.directory / 'main.json').read_text(encoding='utf-8'))
    report = {'status': 'measured', 'cases': [],
              'scope': 'Full 64-frame encoded-output differences plus contact sheets; no motion compensation or high-resolution ground truth. Genuine motion, CLAHE and compression affect these values. No intrinsic flicker or perceptual-improvement threshold is asserted.',
              'attribution': 'Tears of Steel (2012), Blender Institute, directed by Ian Hubert; CC BY 3.0. Downsampled excerpts, synthetic audio, bilinear/AI processing and color changes.',
              'source': 'https://mango.blender.org/about/',
              'license': 'https://creativecommons.org/licenses/by/3.0/'}
    for index in range(0, len(data['cases']), 2):
        baseline, ai = data['cases'][index:index+2]
        assert baseline['request']['bilinear'] and not ai['request']['bilinear']
        files = [args.directory / data['artifacts'][i]['name'] for i in (index, index+1)]
        streams = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_streams', '-of', 'json', str(files[0])]))['streams']
        width, height = streams[0]['width'], streams[0]['height']
        def frames(file):
            raw = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(file), '-map', '0:v:0', '-pix_fmt', 'rgb24', '-f', 'rawvideo', '-'], timeout=60)
            result = np.frombuffer(raw, dtype=np.uint8).reshape(-1,height,width,3)
            assert len(result) == 64
            return result
        b, a = [frames(file) for file in files]
        sequence = []
        for frame in range(1,64):
            bd = b[frame].astype(np.int16)-b[frame-1].astype(np.int16)
            ad = a[frame].astype(np.int16)-a[frame-1].astype(np.int16)
            sequence.append({'index':frame,'bilinear_delta_l1_lsb':float(np.abs(bd).mean()),
                             'ai_delta_l1_lsb':float(np.abs(ad).mean()),
                             'delta_difference_l1_lsb':float(np.abs(ad-bd).mean())})
        label=Path(ai['request']['fixture']).stem+'-'+ai['request']['scale']
        image=args.output/(label+'-all64-bilinear-above-ai.png')
        subprocess.run(['ffmpeg','-v','error','-n','-i',str(files[0]),'-i',str(files[1]),
                        '-filter_complex','[0:v]scale=192:80[b];[1:v]scale=192:80[a];[b][a]vstack,tile=8x8[out]',
                        '-map','[out]','-frames:v','1',str(image)],check=True,timeout=60)
        report['cases'].append({'request':ai['request'],'frames':64,'frame_differences':sequence,
                                'overview':image.name,'overview_display_scaling':'192x80 per image, bilinear above AI; eight columns, temporal row-major order'})
    (args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8',newline='\n')
    print(json.dumps({'status':report['status'],'cases':len(report['cases'])}))


if __name__=='__main__':
    main()
