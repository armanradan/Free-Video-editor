"""Independent direct-software WMV endpoint/audio acceptance; no private media."""
import argparse
from array import array
from fractions import Fraction
import json
import math
from pathlib import Path
import subprocess


def run(args):
    result = subprocess.run(list(map(str, args)), capture_output=True, check=True, timeout=90)
    if result.stderr:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def probe(path, *args):
    return json.loads(run(["ffprobe", "-v", "error", *args, "-of", "json", path]))


def timeline(path, kind):
    stream = probe(path, "-select_streams", kind, "-show_streams")["streams"][0]
    frames = probe(path, "-select_streams", kind, "-show_frames", "-show_entries", "frame=best_effort_timestamp,duration")["frames"]
    base = Fraction(stream["time_base"])
    return stream, [int(f["best_effort_timestamp"]) * base for f in frames]


def audio_clock(path):
    text = run(["ffmpeg", "-v", "error", "-nostdin", "-copyts", "-i", path,
                "-map", "0:a:0", "-c:a", "pcm_s16le", "-f", "framecrc", "-"]).decode()
    base = Fraction(next(line.split(":", 1)[1].strip() for line in text.splitlines() if line.startswith("#tb 0:")))
    packets = [[part.strip() for part in line.split(",")] for line in text.splitlines() if line and not line.startswith("#")]
    return int(packets[0][2]) * base, (int(packets[-1][2]) + int(packets[-1][3])) * base


def pcm(path):
    samples = array("f")
    samples.frombytes(run(["ffmpeg", "-v", "error", "-i", path, "-map", "0:a:0", "-ar", "48000", "-ac", "2", "-f", "f32le", "-"]))
    return samples


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    report = {"status": "running", "scope": "Generated WMV1/WMA1 and WMV2/WMA2, direct software Original FPS only", "cases": []}
    try:
        for name in ["wmv1-wmav1", "wmv2-wmav2"]:
            source = Path("fixtures/native-codecs") / (name + ".wmv")
            output = args.output / (name + ".mp4")
            native = json.loads(run([args.cli, "convert", "--input", source, "--output", output, "--route", "direct"]))
            src, before = timeline(source, "v:0")
            dst, after = timeline(output, "v:0")
            audio_start, audio_end = audio_clock(source)
            out_audio_start, out_audio_end = audio_clock(output)
            origin = min(before[0], audio_start)
            assert len(before) == len(after) == 30
            error = max(abs(b - origin - a) for b, a in zip(before, after, strict=True))
            assert error <= Fraction(1, 1000)
            assert native["input"]["inferred_final_frame_duration_ticks"] == 40
            assert Fraction(dst["duration"]) == Fraction(6, 5)
            # AAC's priming/padding affects track headers; verify audible sample
            # alignment separately against the same decoded source samples.
            reference, actual = pcm(source), pcm(output)
            expected_lag = round((audio_start - origin - out_audio_start) * 48000)

            def mse(lag, step):
                lo = max(0, -lag * 2)
                hi = min(len(reference), len(actual) - lag * 2)
                assert hi > lo
                indices = range(lo, hi, step)
                return sum((reference[i] - actual[i + lag * 2]) ** 2 for i in indices) / len(indices)

            # ASF timestamps are quantized to1ms. Require audible waveform
            # alignment within that1ms budget, tighter than native's25ms AAC
            # header/coverage tolerance; do not demand sub-tick precision.
            lag = min(range(expected_lag - 48, expected_lag + 49), key=lambda lag: mse(lag, 16))
            rmse = math.sqrt(mse(lag, 1))
            assert rmse < 0.02
            assert abs(out_audio_end - (audio_end - origin)) <= Fraction(25, 1000)
            run(["ffmpeg", "-v", "error", "-xerror", "-i", output, "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
            report["cases"].append({"fixture": name, "native": native, "video_pts_max_error_seconds": float(error),
                "decoded_source_audio_start_seconds": float(audio_start), "decoded_source_audio_end_seconds": float(audio_end),
                "decoded_output_audio_start_seconds": float(out_audio_start), "decoded_output_audio_end_seconds": float(out_audio_end),
                "expected_pcm_lag_samples": expected_lag, "measured_pcm_lag_samples": lag, "aligned_float_pcm_rmse": rmse,
                "waveform_alignment_error_seconds": (lag - expected_lag) / 48000,
                "decoded_audio_end_error_seconds": float(out_audio_end - (audio_end - origin)),
                "full_decode": "passed"})
        report["status"] = "passed"
    finally:
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "cases": len(report["cases"])}))


if __name__ == "__main__":
    main()
