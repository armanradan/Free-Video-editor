"""Compare native exports against Windows WMA PCM/timing, not FFmpeg's WMA clock.

Only small explicit local fixtures. Private paths/PCM never enter the report.
The original is read-only; all results go in a new output directory.
"""
import argparse
from array import array
import bisect
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess


def run(args):
    result = subprocess.run(list(map(str, args)), capture_output=True, check=True, timeout=90)
    if result.stderr:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def reference(helper, source):
    data = run([helper, source])
    assert data[:8] == b"DXWMA001" and len(data) <= 64 * 1024 * 1024
    rate, channels, bits, align = struct.unpack_from("<IIII", data, 8)
    assert channels in (1, 2) and bits in (16, 24, 32) and align == channels * bits // 8
    offset, records = 24, []
    while True:
        pts, duration, length, flags = struct.unpack_from("<qqII", data, offset)
        offset += 24
        if flags == 1:
            assert (pts, duration, length) == (0, 0, 0) and offset == len(data)
            break
        if flags == 2:
            assert length == duration == 0
            continue
        assert flags == 0 and 0 < length <= 1048576 and length % align == 0
        block = data[offset:offset + length]
        assert len(block) == length
        records.append((pts, duration, block))
        offset += length
    return rate, channels, bits, records


def ffmpeg_reference(source):
    text = run(["ffmpeg", "-v", "error", "-copyts", "-i", source, "-map", "0:a:0", "-vn",
        "-c:a", "pcm_s16le", "-f", "framecrc", "-"]).decode()
    base = Fraction(next(line.split(":", 1)[1].strip() for line in text.splitlines() if line.startswith("#tb 0:")))
    assert base.numerator == 1
    rows = [[part.strip() for part in line.split(",")] for line in text.splitlines() if line and not line.startswith("#")]
    channels = int(rows[0][4]) // (int(rows[0][3]) * 2)
    assert channels in (1, 2)
    pcm = run(["ffmpeg", "-v", "error", "-i", source, "-map", "0:a:0", "-vn", "-c:a", "pcm_s16le", "-f", "s16le", "-"])
    offset, records = 0, []
    for row in rows:
        pts, duration, length = int(row[2]), int(row[3]), int(row[4])
        assert duration > 0 and length == duration * channels * 2
        records.append((pts * base * 10_000_000, duration * base * 10_000_000, pcm[offset:offset+length]))
        offset += length
    assert offset == len(pcm)
    return base.denominator, channels, 16, records


def probe(path, kind, frames=False):
    args = ["ffprobe", "-v", "error", "-select_streams", kind, "-show_streams"]
    if frames:
        args += ["-show_frames", "-show_entries", "frame=best_effort_timestamp,duration,pict_type:stream=time_base,duration,avg_frame_rate,r_frame_rate"]
    return json.loads(run([*args, "-of", "json", path]))


def video(path):
    metadata = probe(path, "v:0", True)
    base = Fraction(metadata["streams"][0]["time_base"])
    stream = metadata["streams"][0]
    stream["decoded_b_frames"] = sum(frame.get("pict_type") == "B" for frame in metadata["frames"])
    return stream, [int(f["best_effort_timestamp"]) * base for f in metadata["frames"]]


def video_end(path):
    metadata = probe(path, "v:0", True)
    stream, frames = metadata["streams"][0], metadata["frames"]
    base = Fraction(stream["time_base"])
    pts = [int(frame["best_effort_timestamp"]) * base for frame in frames]
    if int(frames[-1].get("duration", 0)) > 0:
        return pts[-1] + int(frames[-1]["duration"]) * base
    for field in ("avg_frame_rate", "r_frame_rate"):
        value = stream.get(field, "0/1")
        if value == "0/0":
            continue
        rate = Fraction(value)
        if rate > 0 and len(pts) >= 3 and all(abs(p - pts[0] - n / rate) < base for n, p in enumerate(pts)):
            return pts[-1] + 1 / rate
    raise AssertionError("Independent source endpoint unavailable")


def image_check(source, output, native, source_pts, output_pts, origin):
    src = probe(source, "v:0")["streams"][0]
    ratio = Fraction(src.get("sample_aspect_ratio", "1:1").replace(":", "/"))
    width = 2 * math.floor(Fraction(src["width"]) * ratio / 2 + Fraction(1, 2))
    assert (width, src["height"]) == (native["input"]["display_width"], native["input"]["display_height"])
    legacy = src["codec_name"] in ("mpeg1video", "mpeg2video", "wmv1", "wmv2", "wmv3", "vc1")
    matrix = "bt709" if src.get("color_space") == "bt709" or (not legacy and src.get("color_space") not in ("smpte170m", "bt470bg")) else "bt601"
    processed = native["ai"]["identity"] is not None or any(native["color"][key] != default for key, default in [("brightness",0),("contrast",100),("saturation",100)]) or native["equalization"]["enabled"]
    def pixels(path, index, matrix):
        return run(["ffmpeg", "-v", "error", "-i", path, "-an", "-vf",
            f"select=eq(n\\,{index}),scale={native['output_width']}:{native['output_height']}:flags=bilinear:in_color_matrix={matrix}:in_range=limited:out_range=full,format=rgb24",
            "-frames:v", "1", "-f", "rawvideo", "-"])
    index = len(output_pts) // 2
    actual = pixels(output, index, "bt709" if processed else matrix)
    target = output_pts[index] + origin
    middle = min(bisect.bisect_left(source_pts, target), len(source_pts)-1)
    errors = []
    for candidate in range(max(0,middle-1), min(len(source_pts),middle+2)):
        expected = pixels(source,candidate,matrix)
        assert len(actual) == len(expected) == native["output_width"] * native["output_height"] * 3
        errors.append(math.sqrt(sum((a-b)**2 for a,b in zip(actual,expected,strict=True))/len(actual)))
    error = min(errors)
    assert max(actual) - min(actual) > 16, "Output image lost fixture structure"
    assert error < 25, ("Independent middle-frame image mismatch", error)
    return {"output_frame": index, "nearest_source_rgb_rmse": error, "display_geometry": "passed"}


def pcm_timeline(rate, channels, bits, records, origin):
    # Place native decoder samples at their observed timestamps. Preserve gaps
    # as silence and report overlaps. This is independent of the app's muxer.
    end = max(Fraction(pts + duration, 10_000_000) for pts, duration, _ in records) - origin
    samples = array("f", [0]) * (math.ceil(end * rate) * channels)
    filled_end, max_overlap = 0, 0
    width = bits // 8
    for pts, _, block in records:
        start = round((Fraction(pts, 10_000_000) - origin) * rate)
        assert start >= 0
        max_overlap = max(max_overlap, filled_end - start)
        # Compare audible PCM continuity within the independently observed2ms
        # ASF jitter envelope. Hard placement at each quantized chunk boundary
        # would invent repeated/dropped samples in the reference waveform.
        # Larger real gaps retain silence. Exact unsmoothed PTS are separately
        # checked block-for-block by the production intermediate verifier.
        if abs(start - filled_end) <= math.ceil(rate * 0.002):
            start = filled_end
        count = len(block) // (channels * width)
        for frame in range(count):
            for channel in range(channels):
                pos = (frame * channels + channel) * width
                value = int.from_bytes(block[pos:pos+width], "little", signed=True) / (1 << (bits - 1))
                index = (start + frame) * channels + channel
                if index < len(samples):
                    samples[index] = value
        filled_end = start + count
    return samples, max_overlap


def compare(reference, actual, channels, rate):
    def mse(lag, stride):
        lo = max(0, -lag * channels)
        hi = min(len(reference), len(actual) - lag * channels)
        assert hi > lo
        positions = range(lo, hi, stride)
        return sum((reference[i] - actual[i + lag * channels]) ** 2 for i in positions) / len(positions)
    # Only1ms alignment search, not arbitrary waveform shifting to hide errors.
    bound = math.ceil(rate / 1000)
    lag = min(range(-bound, bound + 1), key=lambda n: mse(n, 32))
    error = math.sqrt(mse(lag, 1))
    assert error < 0.02, (lag, error)
    return lag, error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--helper", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", action="append", required=True, help="redacted-label=local-path")
    parser.add_argument("--route", action="append", choices=["direct", "wgpu", "nvidia", "wgpu-nvidia"], default=[])
    parser.add_argument("--existing-output", type=Path, help="Verify one already completed export")
    parser.add_argument("--native-report", type=Path, help="Required with existing-output")
    parser.add_argument("--audio-clock", choices=["windows", "ffmpeg"], default="windows")
    parser.add_argument("--fps", default="original")
    parser.add_argument("--images", action="store_true")
    args = parser.parse_args()
    assert args.audio_clock != "windows" or args.helper
    if args.existing_output:
        assert args.native_report and len(args.source) == 1 and len(args.route) <= 1
    args.output.mkdir(parents=True, exist_ok=False)
    report = {"status": "running", "reference": "Windows Media Foundation observed PCM/timestamps" if args.audio_clock == "windows" else "FFmpeg decoded AAC/MP2 PCM/timestamps",
        "helper_sha256": hashlib.sha256(args.helper.read_bytes()).hexdigest() if args.helper else None,
        "waveform_reference": "Contiguous decoded PCM within2ms observed ASF chunk jitter; larger gaps retain silence. Production PCM/PTS remain unchanged.", "cases": []}
    try:
        for value in args.source:
            label, path = value.split("=", 1)
            assert label.replace("-", "").isalnum()
            source = Path(path)
            identity = hashlib.sha256(source.read_bytes()).hexdigest()
            stamp = (source.stat().st_size, source.stat().st_mtime_ns)
            rate, channels, bits, records = reference(args.helper, source) if args.audio_clock == "windows" else ffmpeg_reference(source)
            _, before = video(source)
            origin = min(before[0], Fraction(records[0][0], 10_000_000))
            samples, overlap = pcm_timeline(rate, channels, bits, records, origin)
            for route in args.route or ["direct"]:
                output = args.existing_output or args.output / f"{label}-{route}.mp4"
                native = (json.loads(args.native_report.read_text(encoding="utf-8-sig")) if args.existing_output else
                    json.loads(run([args.cli, "convert", "--input", source, "--output", output, "--route", route, "--fps", args.fps])))
                dst, after = video(output)
                actual_stream = probe(output,"v:0")["streams"][0]
                assert (actual_stream["width"],actual_stream["height"]) == (native["output_width"],native["output_height"])
                assert Fraction(actual_stream["sample_aspect_ratio"].replace(":","/")) == 1
                if args.fps == "original":
                    expected_pts = [p - origin for p in before]
                    expected_video_end = video_end(source) - origin
                else:
                    fps = Fraction(args.fps)
                    first = math.floor((before[0]-origin)*fps)
                    end = math.ceil((video_end(source)-origin)*fps)
                    expected_pts = [Fraction(n)/fps for n in range(first,end)]
                    expected_video_end = Fraction(end)/fps
                assert len(after) == len(expected_pts), (len(after), len(expected_pts))
                video_error = max(abs(b - a) for b, a in zip(expected_pts, after, strict=True))
                assert video_error <= Fraction(1, 1000)
                video_end_error = abs(video_end(output)-expected_video_end)
                assert video_end_error <= Fraction(1,1000), float(video_end_error)
                coverage = native["input"]["decoded_audio_coverage"]
                if route in ("nvidia", "wgpu-nvidia"):
                    assert native["video_encoder"] == "h264_nvenc"
                    encoder = probe(output,"v:0")["streams"][0]["tags"]["encoder"]
                    assert encoder.split()[-1] == "h264_nvenc"
                    expected_decoder = "nvdec" if native["input"]["codec"] in ("h264", "hevc") else "software"
                    assert native["video_decode"]["decoder"] == expected_decoder
                    if expected_decoder == "software":
                        assert native["codec_gpu_to_cpu_bytes"] == 0
                    if native["ai"]["identity"] is not None:
                        assert dst["decoded_b_frames"] == 0
                if args.audio_clock == "windows":
                    assert coverage["decoder"] == "Windows Media Foundation"
                    assert coverage["sample_count"] == sum(len(block) // (channels * bits // 8) for _, _, block in records)
                actual = array("f")
                actual.frombytes(run(["ffmpeg", "-v", "error", "-copyts", "-i", output, "-map", "0:a:0",
                    "-af", f"aresample={rate}:async=1:first_pts=0", "-ac", str(channels), "-ar", str(rate), "-f", "f32le", "-"]))
                lag, rmse = compare(samples, actual, channels, rate)
                expected_end = Fraction(records[-1][0] + records[-1][1], 10_000_000) - origin
                audio_end_error = abs(Fraction(len(actual), rate * channels) - expected_end)
                assert audio_end_error <= Fraction(25, 1000), float(audio_end_error)
                run(["ffmpeg", "-v", "error", "-xerror", "-i", output, "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
                assert identity == hashlib.sha256(source.read_bytes()).hexdigest()
                assert stamp == (source.stat().st_size, source.stat().st_mtime_ns)
                image = image_check(source, output, native, before, after, origin) if args.images else None
                report["cases"].append({"label": label, "route": route, "status": "passed", "source_sha256": identity,
                    "source_unchanged": True, "sample_rate": rate, "bits": bits, "windows_chunk_count": len(records),
                    "quantized_chunk_overlap_max_samples": overlap, "native": native,
                    "video_pts_max_error_seconds": float(video_error), "audio_end_error_seconds": float(audio_end_error),
                    "video_end_error_seconds": float(video_end_error), "decoded_b_frames":dst["decoded_b_frames"],
                    "waveform_lag_samples": lag, "waveform_rmse": rmse, "full_av_decode": "passed", "fps": args.fps, "image_check": image})
        report["status"] = "passed"
    finally:
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "cases": len(report["cases"])}))


if __name__ == "__main__":
    main()
