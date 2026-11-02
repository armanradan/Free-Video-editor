"""Generate a tiny original CC0 progressive MPEG-1/MP2 compatibility fixture."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def run(command):
    result = subprocess.run(command, check=True, capture_output=True, timeout=60)
    if result.stderr:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("fixtures/native-mpeg1.mpg"))
    args = parser.parse_args()
    assert not args.output.exists() and not args.output.with_suffix(".json").exists()
    command = ["ffmpeg", "-hide_banner", "-v", "error", "-n", "-f", "lavfi", "-i",
               "testsrc2=size=352x288:rate=25:duration=2", "-f", "lavfi", "-i",
               "sine=frequency=659:sample_rate=44100:duration=2", "-vf", "setsar=178/163",
               "-c:v", "mpeg1video", "-threads", "1", "-q:v", "8", "-maxrate", "1150k",
               "-bufsize", "327680", "-bf", "0", "-c:a", "mp2", "-b:a", "128k",
               "-output_ts_offset", "1.25", "-f", "mpeg", str(args.output)]
    run(command)

    def probe(*options):
        return json.loads(run(["ffprobe", "-v", "error", *options, "-of", "json", str(args.output)]))

    metadata = probe("-show_streams", "-show_format")
    video = metadata["streams"][0]
    frames = probe("-select_streams", "v:0", "-show_frames", "-show_entries",
                   "frame=best_effort_timestamp,duration,width,height,interlaced_frame")["frames"]
    audio = probe("-select_streams", "a:0", "-show_packets", "-show_entries", "packet=pts,duration")["packets"]
    assert video["codec_name"] == "mpeg1video" and "nb_frames" not in video
    assert video["sample_aspect_ratio"] == "178:163" and len(frames) == 50
    assert all(f["duration"] == 3600 and f["interlaced_frame"] == 0 for f in frames)
    assert [f["best_effort_timestamp"] for f in frames] == [112500 + i * 3600 for i in range(50)]
    assert metadata["streams"][1]["codec_name"] == "mp2"
    assert audio[0]["pts"] > 0 and all(p["duration"] > 0 for p in audio)
    run(["ffmpeg", "-v", "error", "-xerror", "-i", str(args.output), "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
    report = {"license": "CC0-1.0", "provenance": "Original FFmpeg testsrc2 and sine; no third-party media",
              "command": command, "ffmpeg": run(["ffmpeg", "-version"]).decode().splitlines()[0],
              "sha256": hashlib.sha256(args.output.read_bytes()).hexdigest(), "bytes": args.output.stat().st_size,
              "metadata": metadata, "decoded_video_frames": frames, "audio_packets": audio,
              "exact_video_duration_ticks": 180000,
              "exact_audio_duration_ticks": audio[-1]["pts"] + audio[-1]["duration"] - audio[0]["pts"],
              "known_metadata_discrepancy": "Container estimates 1.98s video and 1.933067s audio; decoded frames cover2s and audio packets cover2.011433s. r_frame_rate50 is not decoded25fps."}
    args.output.with_suffix(".json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "passed", "frames": len(frames), "audio_packets": len(audio), "bytes": report["bytes"]}))


if __name__ == "__main__":
    main()
