"""Independent complete MPEG timeline/audio and sampled display-geometry checks."""
import argparse
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import subprocess


TO_SRGB_709 = "zscale=matrixin=709:transferin=709:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full"
TO_SRGB_601 = "zscale=matrixin=smpte170m:transferin=709:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full"


def run(args):
    result = subprocess.run(list(map(str, args)), capture_output=True, timeout=180)
    if result.returncode or result.stderr:
        raise RuntimeError(f"exit={result.returncode}: " + result.stderr.decode(errors="replace"))
    return result.stdout


def probe(path, *args):
    return json.loads(run(["ffprobe", "-v", "error", *args, "-of", "json", path]))


def video(path):
    stream = probe(path, "-select_streams", "v:0", "-show_streams")["streams"][0]
    frames = probe(path, "-select_streams", "v:0", "-show_frames", "-show_entries",
                   "frame=best_effort_timestamp,duration")["frames"]
    tb = Fraction(stream["time_base"])
    pts = [int(f["best_effort_timestamp"]) * tb for f in frames]
    end = pts[-1] + int(frames[-1]["duration"]) * tb
    return stream, pts, end


def sample(path, index, filters):
    return run(["ffmpeg", "-v", "error", "-i", path, "-an", "-vf",
                f"select=eq(n\\,{index}),{filters},format=rgb24", "-frames:v", "1", "-fps_mode", "passthrough", "-f", "rawvideo", "-"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=int, choices=[1, 2, 4], required=True)
    parser.add_argument("--rtx", action="store_true")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    src, before, source_end = video(args.source)
    dst, after, output_end = video(args.output)
    assert (src["width"], src["height"], src["sample_aspect_ratio"]) == (352, 288, "178:163")
    assert len(before) == len(after)
    audio = probe(args.source, "-select_streams", "a:0", "-show_streams")["streams"][0]
    packets = probe(args.source, "-select_streams", "a:0", "-show_packets", "-show_entries", "packet=pts,duration")["packets"]
    audio_tb = Fraction(audio["time_base"])
    audio_start = int(packets[0]["pts"]) * audio_tb
    audio_end = max((int(p["pts"]) + int(p["duration"])) * audio_tb for p in packets)
    origin = min(before[0], audio_start)
    error = max(abs(b - origin - a) for b, a in zip(before, after, strict=True))
    assert error <= Fraction(1, 1000)
    video_end_error = output_end - (source_end - origin)
    assert abs(video_end_error) <= Fraction(1, 1000)
    width, height = 384 * args.scale, 288 * args.scale
    assert (dst["width"], dst["height"], dst["sample_aspect_ratio"]) == (width, height, "1:1")
    out_audio = probe(args.output, "-select_streams", "a:0", "-show_streams")["streams"][0]
    audio_start_error = Fraction(out_audio["start_time"]) - (audio_start - origin)
    audio_end_error = Fraction(out_audio["start_time"]) + Fraction(out_audio["duration"]) - (audio_end - origin)
    assert abs(audio_start_error) <= Fraction(25, 1000) and abs(audio_end_error) <= Fraction(25, 1000)
    run(["ffmpeg", "-v", "error", "-xerror", "-i", args.output, "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
    samples = []
    for index in [0, len(before) // 2, len(before) - 1]:
        reference_filter = "scale=384:288:flags=bilinear,setsar=1"
        if args.rtx:
            reference_filter += f",{TO_SRGB_601},format=gbrp,scale={width}:{height}:flags=bilinear"
        reference = sample(args.source, index, reference_filter)
        actual = sample(args.output, index, TO_SRGB_709 + ",format=gbrp" if args.rtx else "null")
        assert len(reference) == len(actual) == width * height * 3
        rmse = math.sqrt(sum((a - b) ** 2 for a, b in zip(reference, actual, strict=True)) / len(reference))
        assert rmse < 20
        assert max(actual) - min(actual) > 64
        samples.append({"index": index, "rgb_rmse_vs_independent_bilinear": rmse})
    report = {"status": "passed", "scope": "Original FPS, complete decode/timeline/audio, sampled image geometry; diagnostic readbacks excluded from native throughput",
        "frames": len(after), "output_size": [width, height], "sar": "1:1", "scale": args.scale,
        "video_pts_max_error_seconds": float(error), "video_end_error_seconds": float(video_end_error),
        "audio_start_error_seconds": float(audio_start_error), "audio_end_error_seconds": float(audio_end_error),
        "full_decode": "passed", "image_samples": samples,
        "source_sha256": hashlib.file_digest(args.source.open("rb"), "sha256").hexdigest(),
        "output_sha256": hashlib.file_digest(args.output.open("rb"), "sha256").hexdigest(),
        "output_bytes": args.output.stat().st_size}
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
