"""Create an original CC0 WMA2/WMV2 offset+gap control; no downloads."""
import argparse
import hashlib
import json
from pathlib import Path
from verify_windows_wma import reference, run

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--helper", type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
output = args.output / "wmv2-wma2-offset-gap.wmv"
command = ["ffmpeg", "-v", "error", "-n", "-f", "lavfi", "-i",
    "testsrc2=size=192x144:rate=25:duration=1.4", "-f", "lavfi", "-i",
    "sine=frequency=659:sample_rate=44100:duration=1.1", "-af",
    "asetpts=PTS+0.1/TB+if(gte(T\\,0.5)\\,0.15/TB\\,0)", "-c:v", "wmv2", "-threads", "1",
    "-b:v", "500k", "-pix_fmt", "yuv420p", "-c:a", "wmav2", "-b:a", "128k", "-ar", "44100",
    "-ac", "2", "-output_ts_offset", "1.25", output]
run(command)
rate, channels, bits, records = reference(args.helper, output)
gaps = [(pts - previous - duration) / 10_000_000
    for (previous, duration, _), (pts, _, _) in zip(records, records[1:])]
assert any(gap > 0.14 for gap in gaps)
assert records[0][0] == 13_500_000
report = {"license": "CC0-1.0", "provenance": "Original FFmpeg testsrc2/sine, positive audio offset and150ms interior gap",
    "ffmpeg": run(["ffmpeg", "-version"]).decode().splitlines()[0], "command": list(map(str, command)),
    "bytes": output.stat().st_size, "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
    "windows_reference": {"rate": rate, "channels": channels, "bits": bits,
        "sample_count": sum(len(block) // (channels * bits // 8) for _, _, block in records),
        "frames": [{"pts100ns": pts, "duration100ns": duration, "bytes": len(block),
            "pcm_sha256": hashlib.sha256(block).hexdigest()} for pts, duration, block in records]}}
(args.output / "manifest.json").write_text(json.dumps(report, indent=2)+"\n", encoding="utf-8")
print(json.dumps({"status": "passed", "max_gap_seconds": max(gaps), "first_audio_pts100ns": records[0][0]}))
