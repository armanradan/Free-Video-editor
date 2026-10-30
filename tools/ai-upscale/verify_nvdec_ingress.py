"""Diagnostic decoded-pixel comparison; bounded frames, no media downloads/files.

Compare software decode and explicitly selected CUDA/NVDEC through the native
route's sRGB normalization, before inference. No production transfer claim.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import threading

import numpy as np

FILTER = 'zscale=matrixin=709:transferin=709:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full,format=gbrp,format=rgba'


def read_frame(stream, size):
    data=bytearray()
    while len(data)<size:
        part=stream.read(size-len(data))
        if not part:
            break
        data.extend(part)
    assert len(data) in (0,size), 'partial diagnostic decoded frame'
    return data


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--uuid',required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('sources',type=Path,nargs='+')
    args=parser.parse_args()
    args.output.mkdir(exist_ok=False)
    report={'status':'running','tolerance':{'max_rgba8_error':0},'cuda_visible_devices':args.uuid,'cases':[]}
    try:
        for index,source in enumerate(args.sources):
            metadata=json.loads(subprocess.check_output(['ffprobe','-v','error','-select_streams','v:0','-show_entries','stream=width,height,codec_name','-of','json',str(source)]))['streams'][0]
            size=metadata['width']*metadata['height']*4
            assert 0<size<=1280*720*4
            processes=[]
            logs=[]
            timer=None
            try:
                for hardware in [False,True]:
                    log=(args.output/f'{index}-{hardware}.stderr').open('wb');logs.append(log)
                    command=['ffmpeg','-v','error','-nostdin']
                    if hardware:
                        command+=['-hwaccel','cuda','-hwaccel_device','0','-hwaccel_output_format','cuda']
                    command+=['-i',str(source),'-an','-vf',('hwdownload,format=nv12,' if hardware else '')+FILTER,
                        '-fps_mode','passthrough','-f','rawvideo','pipe:1']
                    processes.append(subprocess.Popen(command,stdout=subprocess.PIPE,stderr=log,
                        env={**os.environ,'CUDA_VISIBLE_DEVICES':args.uuid},creationflags=subprocess.CREATE_NO_WINDOW))
                def terminate():
                    for process in processes:
                        if process.poll() is None:
                            process.kill()
                timer=threading.Timer(90,terminate);timer.start()
                frames=0
                digest=hashlib.sha256()
                maximum=0
                while True:
                    left=read_frame(processes[0].stdout,size)
                    right=read_frame(processes[1].stdout,size)
                    assert len(left)==len(right)
                    if not left:
                        break
                    difference=np.abs(np.frombuffer(left,dtype=np.uint8).astype(np.int16)-np.frombuffer(right,dtype=np.uint8).astype(np.int16))
                    maximum=max(maximum,int(difference.max()))
                    assert maximum==0, f'NVDEC ingress differs from software decode: {maximum}'
                    digest.update(left);frames+=1
                assert frames>0
                assert all(process.wait(timeout=10)==0 for process in processes)
                report['cases'].append({'source':str(source),'metadata':metadata,'frames':frames,'max_rgba8_error':maximum,'decoded_sha256':digest.hexdigest()})
            finally:
                if timer:
                    timer.cancel()
                for process in processes:
                    if process.poll() is None:
                        process.kill()
                    process.wait()
                for log in logs:
                    log.close()
        report['status']='passed'
    except BaseException as error:
        report['status']='failed';report['error']=repr(error)
        raise
    finally:
        (args.output/'report.json').write_text(json.dumps(report,indent=2))
    print(json.dumps({'status':report['status'],'cases':len(report['cases'])}))


if __name__=='__main__':
    main()
