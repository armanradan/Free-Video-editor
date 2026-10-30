"""Larger native RTX export/admission gate. Existing local assets; no downloads."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('cli', 'configuration', 'fixtures', 'output'):
        parser.add_argument('--'+name, required=True, type=Path)
    parser.add_argument('--licensed',type=Path,help='Run Original-FPS real-content comparisons from an acquired licensed manifest instead of synthetic matrix')
    parser.add_argument('--route',choices=('wgpu','wgpu-nvidia'),default='wgpu')
    args = parser.parse_args()
    config = json.loads(args.configuration.read_text())
    helper = config['helper']['configuration']
    args.output.mkdir(exist_ok=False)
    report = {'status': 'running-not-release-accepted', 'cases': [], 'artifacts': [], 'rejections': []}
    try:
        plans=[]
        if args.licensed:
            manifest=json.loads((args.licensed/'manifest.json').read_text())
            report['source_manifest']=manifest
            for clip in manifest['clips']:
                hd=(args.licensed/clip['name']).resolve(strict=True)
                assert hashlib.sha256(hd.read_bytes()).hexdigest()==clip['sha256']
                sd=(args.output/f'{hd.stem}-640x268-source.mp4').resolve()
                subprocess.run(['ffmpeg','-v','error','-n','-i',str(hd),'-vf','scale=640:268:flags=lanczos,setsar=1','-c:v','libx264','-crf','18','-color_primaries','bt709','-color_trc','bt709','-colorspace','bt709','-color_range','tv','-c:a','copy',str(sd)],check=True,timeout=60)
                plans.extend([(sd,'640x268','2x','original'),(sd,'640x268','4x','original'),(hd,'1280x534','2x','original')])
        else:
            for size,scale in [('640x360','2x'),('640x360','4x'),('1280x720','2x'),('1280x720','4x')]:
                for fps in (['original'] if size=='1280x720' and scale=='4x' else ['original','15/1','60/1']):
                    plans.append(((args.fixtures/f'synthetic-{size}.mp4').resolve(strict=True),size,scale,fps))
        for source,size,scale,fps in plans:
                index = len(report['cases'])
                output = args.output/f'{source.stem}-{scale}-{fps.replace("/", "-")}.mp4'
                command = [str(args.cli.resolve()), 'rtx-video-pilot', '--helper', helper['executable'],
                    '--cache', helper['cache_parent'], '--rtx-sdk', helper['sdk_root'],
                    '--dxgi-index', str(helper['dxgi_index']), '--adapter-name', helper['exact_adapter_name'],
                    '--luid-high', str(helper['expected_luid_high']), '--luid-low', str(helper['expected_luid_low']),
                    '--adapter-key', config['processing_adapter'], '--input', str(source),
                    '--output', str(output.resolve()), '--scale', scale, '--fps', fps, '--route', args.route,
                    '--equalization-strength', '50', '--brightness', '10', '--contrast', '115', '--saturation', '80']
                started = time.perf_counter()
                result = subprocess.run(command, capture_output=True, timeout=150)
                wall = time.perf_counter()-started
                label = output.stem
                (args.output/f'{label}.stdout').write_bytes(result.stdout)
                (args.output/f'{label}.stderr').write_bytes(result.stderr)
                if size == '1280x720' and scale == '4x':
                    assert result.returncode != 0 and not output.exists()
                    assert b'scratch' in result.stderr or b'memory admission' in result.stderr, result.stderr
                    report['rejections'].append({'command':command,'exit':result.returncode,'error':result.stderr.decode(errors='replace')})
                    continue
                result.check_returncode()
                data = json.loads(result.stdout)
                assert data['frames_processed'] == data['ai']['source_frames'] == 24
                assert data['adapter']['key'] == config['processing_adapter']
                assert not data['ai']['identity']['cpu_fallback']
                assert data['ai']['gpu_to_host_bytes'] == data['ai']['host_to_gpu_bytes']*int(scale[0])**2
                case = {'request': {'fixture':str(source),'scale':scale,'fps':fps,'clahe':True},
                    'inputFrameCount':24,'aiSourceFrames':24,'wall_seconds':wall,'command_argv':command,'report':data}
                report['cases'].append(case)
                report['artifacts'].append({'name':output.name,'bytes':output.stat().st_size,
                    'sha256':hashlib.sha256(output.read_bytes()).hexdigest()})
                print(json.dumps({'size':size,'scale':scale,'fps':fps,'wall_seconds':wall,'status':'exported'}),flush=True)
                (args.output/'native.json').write_text(json.dumps(report,indent=2))
        report['status']='native-video-pilot-passed-not-release-accepted'
    except BaseException as error:
        report['status']='failed'
        report['error']=repr(error)
        raise
    finally:
        (args.output/'native.json').write_text(json.dumps(report,indent=2))


if __name__ == '__main__':
    main()
