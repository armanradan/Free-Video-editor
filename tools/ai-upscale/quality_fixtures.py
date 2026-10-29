"""Explicitly acquire tiny licensed evaluation excerpts into ignored tmp storage.

FFmpeg seeks over HTTP ranges; the complete movie and its soundtrack are never
stored. This is an evaluation command, never an application startup dependency.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess


SOURCE = 'https://download.blender.org/demo/movies/ToS/tears_of_steel_720p.mov'
CASES = (
    ('tos-text-texture', 45, ['rendered text', 'fine architectural texture']),
    ('tos-face', 60, ['live-action face', 'shallow focus', 'organic texture']),
    ('tos-motion', 120, ['live-action motion', 'cloth', 'ropes']),
)


def run(command):
    return subprocess.check_output(command, stderr=subprocess.PIPE, timeout=120)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--frames', type=int, default=8)
    args = parser.parse_args()
    if not 8 <= args.frames <= 120:
        parser.error('Use 8 to 120 frames for bounded evaluation excerpts')
    duration = f'{args.frames / 12:.9f}'
    destination = args.output.resolve()
    ignored_root = Path('tmp/ai-upscale').resolve()
    if not destination.is_relative_to(ignored_root) or destination == ignored_root:
        parser.error('Use a fresh directory under ignored tmp/ai-upscale')
    destination.mkdir(parents=True, exist_ok=False)
    manifest = {
        'schema': 1, 'created_utc': datetime.now(timezone.utc).isoformat(),
        'status': 'acquiring-not-quality-accepted',
        'source': SOURCE,
        'source_license': 'CC-BY-3.0',
        'license_url': 'https://creativecommons.org/licenses/by/3.0/',
        'source_license_statement': 'https://mango.blender.org/about/',
        'attribution': 'Tears of Steel (2012), Blender Institute; directed by Ian Hubert, produced by Ton Roosendaal.',
        'credits': 'https://mango.blender.org/about/',
        'changes': 'Short excerpts, 192x80 Lanczos downsample, 12 FPS, H.264 8-bit SDR; original audio removed and replaced with an original synthetic sine.',
        'audio': {'provenance': 'Original FFmpeg sine signal', 'license': 'CC0-1.0'},
        'soundtrack_excluded': True,
        'soundtrack_notice': 'https://mango.blender.org/production/soundtrack-files-released/',
        'limitations': [f'{args.frames} consecutive frames per excerpt cover limited scenes, not universal temporal-quality acceptance.',
                       'No pristine low-resolution/high-resolution training pair or perceptual improvement claim.',
                       'Full movie is not retained; excerpt hashes identify the acquired test inputs.'],
        'ffmpeg': run(['ffmpeg', '-version']).decode('utf-8').splitlines()[0],
        'clips': [],
    }
    report = destination / 'manifest.json'
    try:
        for name, seconds, subjects in CASES:
            target = destination / (name + '.mp4')
            command = ['ffmpeg', '-v', 'error', '-n', '-rw_timeout', '15000000',
                       '-ss', str(seconds), '-i', SOURCE, '-f', 'lavfi', '-i',
                       f'sine=frequency=660:sample_rate=48000:duration={duration}',
                       '-map', '0:v:0', '-map', '1:a:0', '-t', duration,
                       '-vf', 'fps=12,scale=192:80:flags=lanczos,setsar=1,format=yuv420p',
                       '-c:v', 'libx264', '-threads', '1', '-preset', 'slow', '-crf', '18',
                       '-color_primaries', 'bt709', '-color_trc', 'bt709', '-colorspace', 'bt709',
                       '-color_range', 'tv', '-c:a', 'aac', '-b:a', '96k',
                       '-movflags', '+faststart', str(target)]
            run(command)
            probe = json.loads(run(['ffprobe', '-v', 'error', '-show_streams',
                                    '-show_frames', '-of', 'json', str(target)]))
            video = next(s for s in probe['streams'] if s['codec_type'] == 'video')
            frames = [f for f in probe['frames'] if f['media_type'] == 'video']
            if len(frames) != args.frames or (video['width'], video['height'], video['pix_fmt']) != (192, 80, 'yuv420p'):
                raise ValueError('Excerpt failed exact frame/geometry/precision checks')
            manifest['clips'].append({
                'name': target.name, 'source_start_seconds': seconds, 'subjects': subjects,
                'bytes': target.stat().st_size,
                'sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
                'command_argv': command, 'video': video,
                'video_pts': [f['pts_time'] for f in frames], 'video_frames': len(frames),
                'audio': next(s for s in probe['streams'] if s['codec_type'] == 'audio'),
            })
            print(json.dumps({'clip': name, 'status': 'acquired', 'frames': len(frames),
                              'bytes': target.stat().st_size}), flush=True)
        manifest['status'] = 'acquired-not-quality-accepted'
    except Exception as error:
        manifest['status'] = 'failed'
        manifest['error'] = str(error)
        if isinstance(error, subprocess.CalledProcessError):
            manifest['stderr'] = error.stderr.decode('utf-8', errors='replace')
        raise
    finally:
        report.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8', newline='\n')


if __name__ == '__main__':
    main()
