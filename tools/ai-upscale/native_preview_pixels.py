"""Independent display scaling of actual native pre-encoder rasters; diagnostic only."""
import argparse
import json
from pathlib import Path
import numpy as np
from preview_pixels import resize

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--directory',type=Path,required=True)
parser.add_argument('--worker-directory',type=Path)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
if args.output.exists():raise ValueError('Preserve prior evidence')
data=json.loads((args.directory/'report.json').read_text(encoding='utf-8'))
report={'status':'running','max_lsb_gate':2,'cases':[]}
try:
    for item in data['cases']:
        size=item['selected'];display=item['display'];index=item['index']
        raw=(args.directory/f'{index}-export.rgba').read_bytes()
        selected=np.frombuffer(raw,dtype=np.uint8).reshape(size['height'],size['width'],4)
        expected=resize(selected,display['width'],display['height'])
        paths=[('service',args.directory/f'{index}-preview.rgba')]
        if args.worker_directory:
            worker=json.loads((args.worker_directory/'report.json').read_text(encoding='utf-8'))
            for case in worker['cases']:
                if all(case[k]==item[k] for k in ['fixture','scale','strength','selected_us']):
                    paths.append(('renderer',args.worker_directory/f"{case['index']}-renderer.rgba"))
        for context,path in paths:
            actual=np.frombuffer(path.read_bytes(),dtype=np.uint8).reshape(expected.shape)
            error=np.abs(actual.astype(np.int16)-expected.astype(np.int16))
            result={'context':context,'fixture':item['fixture'],'scale':item['scale'],
                'strength':item['strength'],'selected_us':item['selected_us'],
                'max_lsb':int(error.max()),'mae_lsb':float(error.mean())}
            report['cases'].append(result)
            assert result['max_lsb']<=2,result
            assert np.all(actual[:,:,3]==255)
        original=np.frombuffer((args.directory/f"{item['fixture']}-original-{item['source_index']}.rgba").read_bytes(),dtype=np.uint8).reshape(item['input']['height'],item['input']['width'],4)
        expected_before=resize(original,display['width'],display['height'])
        before_paths=[('service-before',args.directory/f'{index}-before.rgba')]
        if args.worker_directory:
            for case in worker['cases']:
                if all(case[k]==item[k] for k in ['fixture','scale','strength','selected_us']):
                    before_paths.append(('renderer-before',args.worker_directory/f"{case['index']}-before.rgba"))
        for context,path in before_paths:
            actual=np.frombuffer(path.read_bytes(),dtype=np.uint8).reshape(expected_before.shape)
            error=np.abs(actual.astype(np.int16)-expected_before.astype(np.int16))
            result={'context':context,'fixture':item['fixture'],'scale':item['scale'],
                'strength':item['strength'],'selected_us':item['selected_us'],
                'max_lsb':int(error.max()),'mae_lsb':float(error.mean())}
            report['cases'].append(result)
            assert result['max_lsb']<=2,result
            assert np.all(actual[:,:,3]==255)
    if args.worker_directory and worker.get('cold_before'):
        case=worker['cold_before'];size=case['input'];display=case['display']
        original=np.frombuffer((args.directory/f"{case['fixture']}-original-0.rgba").read_bytes(),dtype=np.uint8).reshape(size['height'],size['width'],4)
        expected=resize(original,display['width'],display['height'])
        actual=np.frombuffer((args.worker_directory/'cold-before.rgba').read_bytes(),dtype=np.uint8).reshape(expected.shape)
        error=np.abs(actual.astype(np.int16)-expected.astype(np.int16))
        report['cases'].append({'context':'renderer-cold-before-missing-helper','max_lsb':int(error.max()),'mae_lsb':float(error.mean())})
        assert error.max()<=2
        assert np.all(actual[:,:,3]==255)
    report['status']='passed'
except Exception as error:
    report['status']='failed';report['error']=str(error);raise
finally:
    args.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'status':report['status'],'comparisons':len(report['cases']),
    'max_lsb':max(c['max_lsb'] for c in report['cases'])}))
