"""Actual native AAC normalization exports and independent decode/timing checks.

Requires the 192x144 aac_feasibility.py fixture set. No downloads or SDK use.
"""
import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import subprocess


def run(args):
    return subprocess.run(list(map(str, args)), capture_output=True, check=True, timeout=120).stdout


def probe(path, *args):
    return json.loads(run(["ffprobe", "-v", "error", *args, "-of", "json", path]))


def frames(path):
    stream = probe(path, "-select_streams", "v:0", "-show_streams")["streams"][0]
    tb = Fraction(stream["time_base"])
    return [int(f["best_effort_timestamp"]) * tb for f in probe(
        path, "-select_streams", "v:0", "-show_frames", "-show_entries", "frame=best_effort_timestamp")["frames"]]


def pcm(path):
    return run(["ffmpeg", "-v", "error", "-i", path, "-map", "0:a:0", "-f", "f32le", "pipe:1"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--adapter", required=True)
    parser.add_argument("--nvidia", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    report = {"status": "running", "cases": [], "scope": "Synthetic integration; independent FFprobe and full FFmpeg decode"}
    original_hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in args.fixtures.glob("*.mp4")}
    try:
        for route in (["direct", "wgpu", "nvidia", "wgpu-nvidia"] if args.nvidia else ["direct", "wgpu"]):
            reference_pcm = None
            for name in ["reference", "leading-pts", "negative-priming", "positive-offset", "intentional-gap"]:
                source = args.fixtures / (name + ".mp4")
                output = args.output / f"{route}-{name}.mp4"
                command = [args.cli, "convert", "--input", source, "--output", output,
                           "--route", route, "--adapter-key", args.adapter]
                native = json.loads(run(command))
                assert native["input"]["audio_normalization"] == (
                    {"LeadingPts": {"before": 3600, "after": 0}} if name == "leading-pts" else None)
                src = probe(source, "-show_streams")["streams"]
                v = next(s for s in src if s["codec_type"] == "video")
                a = next(s for s in src if s["codec_type"] == "audio")
                if name == "leading-pts":
                    # Fixture truth comes from its known-correct source, not
                    # the anomalous header or the converter's own report.
                    a = probe(args.fixtures / "reference.mp4", "-select_streams", "a:0", "-show_streams")["streams"][0]
                origin = min(Fraction(v["start_time"]), Fraction(a["start_time"]))
                expected = [p - origin for p in frames(source)]
                actual = frames(output)
                assert len(expected) == len(actual) == native["output_frame_count"]
                error = max(abs(a - b) for a, b in zip(expected, actual, strict=True))
                assert error <= Fraction(11, 10000), (route, name, float(error))
                audio = probe(output, "-select_streams", "a:0", "-show_streams")["streams"][0]
                start_error = abs(Fraction(audio["start_time"]) - (Fraction(a["start_time"]) - origin))
                end_error = abs(Fraction(audio["start_time"]) + Fraction(audio["duration"]) -
                                (Fraction(a["start_time"]) - origin + Fraction(a["duration"])))
                assert start_error <= Fraction(25, 1000), (route, name, "audio origin")
                assert end_error <= Fraction(25, 1000), (route, name, "audio endpoint", float(end_error))
                run(["ffmpeg", "-v", "error", "-xerror", "-i", output, "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
                samples = pcm(output)
                if name == "reference": reference_pcm = samples
                if name == "leading-pts": assert samples == reference_pcm, (route, "corrected PCM alignment")
                report["cases"].append({"route": route, "fixture": name, "command": list(map(str, command)),
                    "native": native, "independent": {"video_pts_max_error_seconds": float(error),
                    "audio_start_error_seconds": float(start_error), "audio_end_error_seconds": float(end_error),
                    "full_decode": "passed", "decoded_pcm_sha256": hashlib.sha256(samples).hexdigest(),
                    "corrected_pcm_equals_valid_reference": name == "leading-pts"}})
                print(json.dumps({"route": route, "fixture": name, "status": "passed"}), flush=True)
        assert original_hashes == {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in args.fixtures.glob("*.mp4")}
        report["originals_unchanged"] = True
        report["status"] = "passed"
    except BaseException as error:
        report["status"] = "failed"
        report["error"] = repr(error)
        raise
    finally:
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
