"""Generate tiny CC0 synthetic video fixtures; never fetch source media."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--width', type=int, default=64)
    parser.add_argument('--height', type=int, default=36)
    parser.add_argument('--name', default='ai-upscale-vfr')
    parser.add_argument('--no-rotated', action='store_true')
    parser.add_argument('--scene-cut', action='store_true', help='Invert the synthetic image from 100 ms')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    if not (2 <= args.width <= 577 and 2 <= args.height <= 579) or Path(args.name).name != args.name:
        parser.error('Expected bounded dimensions and a simple fixture name')
    base = args.output / (args.name+'.mp4')
    command = ['ffmpeg', '-v', 'error', '-n', '-f', 'lavfi', '-i',
               f'testsrc2=size={args.width}x{args.height}:rate=30:duration=0.266666667', '-f', 'lavfi', '-i',
               'sine=frequency=660:sample_rate=48000:duration=0.266666667',
               '-map', '0:v:0', '-map', '1:a:0', '-vf',
               ("negate=enable='gte(t,0.1)'," if args.scene_cut else "") +
               "select='eq(n,0)+eq(n,2)+eq(n,3)+eq(n,7)'", '-fps_mode', 'vfr',
               '-c:v', 'libx264', '-preset', 'slow', '-crf', '18', '-pix_fmt', 'yuv420p',
               '-g', '30', '-x264-params', 'colorprim=bt709:transfer=bt709:colormatrix=bt709',
               '-c:a', 'aac', '-b:a', '96k', '-output_ts_offset', '1.25',
               '-movflags', '+faststart', str(base)]
    subprocess.run(command, check=True)
    rotated = args.output / 'ai-upscale-rotated.mp4'
    rotate = ['ffmpeg', '-v', 'error', '-n', '-display_rotation:v', '-90', '-display_hflip:v',
              '-i', str(base), '-map', '0', '-c', 'copy', '-movflags', '+faststart', str(rotated)]
    if not args.no_rotated:
        subprocess.run(rotate, check=True)
    version = subprocess.check_output(['ffmpeg', '-version'], text=True).splitlines()[0]
    for path, recipe in ([(base, command)] if args.no_rotated else [(base, command), (rotated, rotate)]):
        probe = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-show_streams',
            '-show_frames', '-of', 'json', str(path)], text=True))
        video = next(stream for stream in probe['streams'] if stream['codec_type'] == 'video')
        frames = [frame for frame in probe['frames'] if frame['media_type'] == 'video']
        if len(frames) != 4:
            raise ValueError(f'Expected four VFR frames, got {len(frames)}')
        manifest = {'schema': 1, 'name': path.name, 'license': 'CC0-1.0',
                    'source_provenance': 'Original synthetic FFmpeg testsrc2 and sine signals; no downloaded media',
                    'generated_utc': '2026-10-23', 'generator': version, 'command_argv': recipe,
                    'size_bytes': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                    'expected': {'video_frames': len(frames), 'video_stream': video,
                        'video_pts': [frame['pts_time'] for frame in frames],
                        'audio_stream': next(stream for stream in probe['streams'] if stream['codec_type'] == 'audio')}}
        path.with_suffix('.json').write_text(json.dumps(manifest, indent=2)+'\n', encoding='utf-8', newline='\n')


if __name__ == '__main__':
    main()
