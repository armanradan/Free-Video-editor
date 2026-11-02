"""Generate original CC0 codec controls, inventory private clips without publishing them."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import subprocess


def run(command):
    result = subprocess.run(list(map(str, command)), check=True, capture_output=True, timeout=90)
    if result.stderr:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def describe(path):
    metadata = json.loads(run(["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", path]))
    metadata["format"]["filename"] = path.name
    metadata["format"].pop("tags", None)
    for stream in metadata["streams"]:
        stream.pop("tags", None)
    frames = json.loads(run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_frames",
        "-show_entries", "frame=best_effort_timestamp,duration,width,height,interlaced_frame",
        "-of", "json", path]))["frames"]
    audio = json.loads(run(["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_frames",
        "-show_entries", "frame=best_effort_timestamp,duration,nb_samples,sample_rate",
        "-of", "json", path]))["frames"]
    run(["ffmpeg", "-v", "error", "-xerror", "-i", path, "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
    return {"bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "metadata": metadata, "decoded_video_frames": frames, "decoded_audio_frames": audio,
            "full_decode": "passed"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--private-wmv-folder", type=Path)
    parser.add_argument("--vc1-source", type=Path, help="Local upstream LGPL test-vc1.c; no automatic download")
    parser.add_argument("--mp4-controls", type=Path, help="Existing fixture directory with H.264/HEVC controls")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    report = {"status": "running", "cases": [], "blocked": []}
    for video, audio, extension in [("mpeg2video", "mp2", "mpg"), ("wmv1", "wmav1", "wmv"), ("wmv2", "wmav2", "wmv")]:
        output = args.output / f"{video}-{audio}.{extension}"
        command = ["ffmpeg", "-hide_banner", "-v", "error", "-n", "-f", "lavfi", "-i",
                   "testsrc2=size=192x144:rate=25:duration=1.2", "-f", "lavfi", "-i",
                   "sine=frequency=659:sample_rate=44100:duration=1.2", "-c:v", video, "-threads", "1",
                   "-b:v", "500k", "-pix_fmt", "yuv420p", "-c:a", audio, "-b:a", "128k",
                   "-ar", "44100", "-ac", "2", "-output_ts_offset", "1.25", output]
        run(command)
        report["cases"].append({"label": output.stem, "license": "CC0-1.0",
            "provenance": "Original FFmpeg testsrc2 and sine", "command": list(map(str, command)), **describe(output)})
    if args.vc1_source:
        # Keep upstream source/notice locally. Extraction does not claim original authorship.
        source = args.vc1_source.read_bytes()
        array = re.search(r"vc1_clip\[VC1_CLIP_DATA_SIZE\]\s*=\s*\{([^}]+)\}", source.decode(), re.S)[1]
        clip = bytes(int(value, 16) for value in re.findall(r"0x([0-9a-fA-F]{2})", array))
        assert len(clip) == 20864
        elementary = args.output / "repeated.vc1"
        elementary.write_bytes(clip * 30)
        output = args.output / "vc1-wma2.wmv"
        run(["ffmpeg", "-v", "error", "-n", "-fflags", "+genpts", "-r", "25", "-i", elementary,
            "-f", "lavfi", "-i", "sine=frequency=659:sample_rate=44100:duration=1.2",
            "-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy", "-c:a", "wmav2", "-b:a", "128k", "-t", "1.2", output])
        report["cases"].append({"label": output.stem, "license": "LGPL-2.1-or-later video; CC0-1.0 generated audio",
            "provenance": "GStreamer/gstreamer-vaapi tests/internal/test-vc1.c; copyright 2010-2011 Splitted-Desktop Systems, Gwenole Beauchesne",
            "source_url": "https://github.com/GStreamer/gstreamer-vaapi/blob/97aabe8784c057a4bee33d6f541ab7d75ca3708e/tests/internal/test-vc1.c",
            "source_sha256": hashlib.sha256(source).hexdigest(),
            "construction": "Repeat the complete one-frame sequence30 times; explicit generated25fps timestamps; generated WMA2 audio. Not evidence for temporal/B-frame VC-1 behavior.",
            **describe(output)})
    if args.private_wmv_folder:
        for index, source in enumerate(sorted(args.private_wmv_folder.glob("*.wmv"))):
            output = args.output / f"private-wmv-{index}.wmv"
            run(["ffmpeg", "-v", "error", "-n", "-copyts", "-i", source, "-t", "2",
                 "-map", "0:v:0", "-map", "0:a:0", "-c", "copy", "-f", "asf", output])
            # No path or filename from private input enters the published report.
            report["cases"].append({"label": output.stem, "license": "Private user-provided local evaluation; not redistributable",
                "provenance": "User directed evaluation of adjacent external-drive inputs; source and excerpt excluded from git",
                **describe(output)})
    if args.mp4_controls:
        for name in ["m2-h264-aac.mp4", "m36-h265-aac.mp4"]:
            path = args.mp4_controls / name
            report["cases"].append({"label": name, "license": "See fixtures/README.md",
                "provenance": "Existing project synthetic MP4 control", **describe(path)})
    codecs = {s["codec_name"] for case in report["cases"] for s in case["metadata"]["streams"]}
    for codec in ["mpeg2video", "mp2", "wmv1", "wmv2", "wmv3", "vc1", "wmav1", "wmav2", "wmapro"]:
        if codec not in codecs:
            report["blocked"].append({"codec": codec, "reason": "No evaluated fixture; installed FFmpeg cannot generate this codec"})
    report["status"] = "partial-missing-fixture" if report["blocked"] else "fixtures-decoded-not-app-accepted"
    report["ffmpeg"] = run(["ffmpeg", "-version"]).decode().splitlines()[0]
    wanted = {"h264", "hevc", "mpeg1video", "mpeg2video", "wmv1", "wmv2", "wmv3", "vc1", "aac", "mp2", "wmav1", "wmav2", "wmapro"}
    report["installed_decoders"] = [line.strip() for line in run(["ffmpeg", "-hide_banner", "-decoders"]).decode().splitlines()
        if len(line.split()) >= 2 and line.split()[1].split("_")[0] in wanted]
    (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "codecs": sorted(codecs), "blocked": report["blocked"]}))


if __name__ == "__main__":
    main()
