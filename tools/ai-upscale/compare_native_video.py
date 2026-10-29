"""Compare repeated tiny native video matrices using file and decoded-frame hashes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('first', type=Path)
    parser.add_argument('second', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('Preserve existing evidence: output already exists')
    matrices = [json.loads((directory/'native.json').read_text(encoding='utf-8'))
                for directory in [args.first, args.second]]
    assert all(r['status'] == 'native-video-pilot-passed-not-release-accepted' for r in matrices)
    assert 0 < len(matrices[0]['cases']) <= 24
    report = {'status': 'running', 'cases': []}
    try:
        for index, (left, right) in enumerate(zip(matrices[0]['cases'], matrices[1]['cases'], strict=True)):
            assert left['request'] == right['request']
            assert 0 < left['inputFrameCount'] == right['inputFrameCount'] <= 256
            file_hashes, decoded_hashes = [], []
            for directory, matrix in zip([args.first,args.second],matrices,strict=True):
                file = directory/matrix['artifacts'][index]['name']
                with file.open('rb') as stream:
                    file_hashes.append(hashlib.file_digest(stream,'sha256').hexdigest())
                decoded = subprocess.run(['ffmpeg','-v','error','-i',str(file),'-map','0:v:0',
                    '-pix_fmt','rgba','-fps_mode','passthrough','-f','framehash','-hash','sha256','pipe:1'],
                    capture_output=True,timeout=60,check=True)
                decoded_hashes.append(hashlib.sha256(decoded.stdout).hexdigest())
            assert decoded_hashes[0] == decoded_hashes[1], (index, decoded_hashes)
            report['cases'].append({'request':left['request'], 'file_sha256':file_hashes,
                'exact_file_repeat':file_hashes[0]==file_hashes[1], 'decoded_framehash_sha256':decoded_hashes[0]})
        report['status'] = 'passed'
    finally:
        args.output.write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps({'status':report['status'],'cases':len(report['cases']),
        'bit_exact_files':sum(c['exact_file_repeat'] for c in report['cases'])}))


if __name__ == '__main__':
    main()
