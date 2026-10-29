"""Compare paused preview with actual pre-encoder exports and independent Before resize.

All rasters are explicit diagnostics, excluded from conversion/preview throughput.
No shader/model implementation is duplicated here.
"""
import argparse
import json
from pathlib import Path
import numpy as np


def resize(rgba, width, height):
    # Independent center-aligned bilinear sampling in encoded RGB with clamp edges.
    ys=(np.arange(height)+.5)*rgba.shape[0]/height-.5
    xs=(np.arange(width)+.5)*rgba.shape[1]/width-.5
    y0=np.floor(ys).astype(int);x0=np.floor(xs).astype(int)
    fy=(ys-y0)[:,None,None];fx=(xs-x0)[None,:,None]
    y1=np.clip(y0+1,0,rgba.shape[0]-1);x1=np.clip(x0+1,0,rgba.shape[1]-1)
    y0=np.clip(y0,0,rgba.shape[0]-1);x0=np.clip(x0,0,rgba.shape[1]-1)
    values=rgba.astype(float)
    top=values[y0[:,None],x0[None,:]]*(1-fx)+values[y0[:,None],x1[None,:]]*fx
    bottom=values[y1[:,None],x0[None,:]]*(1-fx)+values[y1[:,None],x1[None,:]]*fx
    return np.floor(np.clip(top*(1-fy)+bottom*fy,0,255)+.5).astype(np.uint8)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    if args.output.exists():raise ValueError('Preserve prior evidence')
    report={'status':'running','gate_max_lsb':2,'cases':[]}
    def raster(frame):
        return np.frombuffer((args.directory/frame['dataFile']).read_bytes(),dtype=np.uint8).reshape(frame['height'],frame['width'],4)
    try:
        for context in ['main','worker']:
            data=json.loads((args.directory/(context+'.json')).read_text(encoding='utf-8'))
            temporal=data['previewStatus']=='finite-history-ai-preview-pilot'
            assert temporal or data['previewStatus']=='tiny-preview-cache-before-ffmpeg-independence-pilot'
            exports={} if temporal else {item['request']['label']:item['parityFrames'][0] for item in data['cases']}
            for item in data['previewCases']:
                label=item['request']['label'];frame=item['parityFrames'][0];actual=raster(frame)
                if label in ['before','off']:
                    source=({'rotation':0,'flip':False} if temporal else data['rejectedExport']['source'])
                    original=np.rot90(raster(frame['ingress']),k=-int(source['rotation'])//90)
                    if source['flip']:original=np.fliplr(original)
                    expected=resize(original,frame['width'],frame['height'])
                    reference='independent neutral normalized source resize'
                else:
                    key='clahe-80' if label in ['after-restore','preview-with-rejected-ffmpeg-export'] else label
                    source=next(frame for case in data['cases'] if case['request']['scale']==item['request']['scale'] and case['request']['strength']==item['request']['strength'] for frame in case['parityFrames'] if frame['sourceTimestamp']==item['parityFrames'][0]['sourceTimestamp']) if temporal else exports[key]
                    assert frame['sourceTimestamp']==source['sourceTimestamp']
                    expected=resize(raster(source),frame['width'],frame['height'])
                    reference='actual selected-resolution pre-encoder export, then independent display resize'
                error=np.abs(actual.astype(np.int16)-expected.astype(np.int16))
                result={'context':context,'label':label,'reference':reference,'width':frame['width'],
                        'height':frame['height'],'max_lsb':int(error.max()),'mae_lsb':float(error.mean())}
                report['cases'].append(result)
                assert result['max_lsb']<=2,result
                assert np.all(actual[:,:,3]==255)
            assert not data['finalOwnership']['aiCreditHeld']
        report['status']='passed'
    except Exception as error:
        report['status']='failed';report['error']=str(error);raise
    finally:
        args.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'status':report['status'],'cases':len(report['cases'])}))


if __name__=='__main__':main()
