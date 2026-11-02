"""Explicit local licensed RTX legacy matrix; no downloads or provisioning.

Runs accepted2x geometry through both shared codec routes, Original and15fps,
then independent complete A/V/timestamp/image checks. Stops on first failure.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys
from verify_windows_wma import probe, run

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--cli", type=Path, required=True)
parser.add_argument("--helper", type=Path, required=True)
parser.add_argument("--wma-helper", type=Path, required=True)
parser.add_argument("--sdk", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--source", action="append", required=True, help="redacted-label=local-path")
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
report = {"status": "running", "scope": "RTX2x; Original and15fps; shared software/NVIDIA; complete independent A/V/timeline and middle-frame image checks", "cases": []}
current = None
try:
    for value in args.source:
        label, source = value.split("=", 1)
        assert label.replace("-", "").isalnum()
        source = Path(source)
        audio = probe(source, "a:0")["streams"][0]["codec_name"]
        clock = "windows" if audio in ("wmav1", "wmav2", "wmapro") else "ffmpeg"
        for route in ["wgpu", "wgpu-nvidia"]:
            for fps in ["original", "15"]:
                name = f"{label}-{route}-{fps}"
                current = {"label": label, "route": route, "fps": fps, "scale": "2x"}
                output = args.output / f"{name}.mp4"
                native = json.loads(run([args.cli, "rtx-video-pilot", "--input", source, "--output", output,
                    "--helper", args.helper, "--rtx-sdk", args.sdk, "--cache", args.output,
                    "--dxgi-index", "1", "--adapter-name", "NVIDIA GeForce RTX 3050 Laptop GPU",
                    "--luid-high", "0", "--luid-low", "78387", "--adapter-key",
                    "Vulkan:10de:25a2:NVIDIA GeForce RTX 3050 Laptop GPU", "--scale", "2x", "--route", route, "--fps", fps]))
                native_path = args.output / f"{name}.native.json"
                native_path.write_text(json.dumps(native, indent=2)+"\n", encoding="utf-8")
                evidence = args.output / f"{name}-evidence"
                run([sys.executable, Path(__file__).with_name("verify_windows_wma.py"), "--cli", args.cli,
                    "--helper", args.wma_helper, "--audio-clock", clock, "--output", evidence,
                    "--source", f"{label}={source}", "--route", route, "--fps", fps, "--images",
                    "--existing-output", output, "--native-report", native_path])
                check = json.loads((evidence / "report.json").read_text())["cases"][0]
                assert check["native"]["ai"]["identity"]["runtime"] == "RTX Video SDK 1.1.0"
                assert (native["output_width"],native["output_height"]) == (native["input"]["display_width"]*2,native["input"]["display_height"]*2)
                report["cases"].append({**current, **check})
                (args.output / "report.json").write_text(json.dumps(report, indent=2)+"\n", encoding="utf-8")
                print(json.dumps({**current, "status": "passed"}), flush=True)
    report["status"] = "passed"
except BaseException as error:
    report["status"] = "failed"
    report["failed_case"] = current
    if isinstance(error, subprocess.CalledProcessError):
        # Source labels/paths in this matrix are local evaluation fixtures, not
        # user titles. Avoid serializing whole commands/private source paths.
        report["error"] = error.stderr.decode(errors="replace")[:8192] if error.stderr else f"child exit{error.returncode}"
    else:
        report["error"] = str(error)
    raise
finally:
    (args.output / "report.json").write_text(json.dumps(report, indent=2)+"\n", encoding="utf-8")
