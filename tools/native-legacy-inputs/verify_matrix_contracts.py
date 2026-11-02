"""Additional independent packet endpoints, dimensions, NVENC/B-frame and copy
contracts for already decoded legacy matrix outputs. Never re-encodes media.
"""
import argparse
from fractions import Fraction
import json
import math
from pathlib import Path
from verify_windows_wma import reference, ffmpeg_reference, video, video_end, run

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--root", type=Path, required=True)
parser.add_argument("--wma-helper", type=Path, required=True)
parser.add_argument("--report", type=Path, required=True)
args = parser.parse_args()
sources = {"h264":Path("fixtures/m2-h264-aac.mp4"), "hevc":Path("fixtures/m36-h265-aac.mp4"),
    "mpeg1":Path("fixtures/native-mpeg1.mpg"), "mpeg2":Path("fixtures/native-codecs/mpeg2video-mp2.mpg"),
    "wma1":Path("fixtures/native-codecs/wmv1-wmav1.wmv"), "wma2":Path("fixtures/native-codecs/wmv2-wmav2.wmv"),
    "private-pro":args.root/"codec-fixtures-manifest/private-wmv-0.wmv",
    "private-wma2":args.root/"codec-fixtures-manifest/private-wmv-1.wmv",
    "vc1-advanced":args.root/"codec-fixtures-manifest/vc1-wma2.wmv",
    "offset-gap":Path("fixtures/native-codecs/wmv2-wma2-offset-gap.wmv")}
truth = {}
for label, source in sources.items():
    _, pts = video(source)
    _, _, _, audio = (reference(args.wma_helper, source) if label not in ("h264","hevc","mpeg1","mpeg2") else ffmpeg_reference(source))
    truth[label] = (pts, video_end(source), min(pts[0],Fraction(audio[0][0],10_000_000)))

report = {"status":"running", "scope":"Independent packet endpoints, dimensions, selected codec plans, actual NVENC identity/no B reorder, logical bridge byte counts; complete decoded A/V/image checks are in the corresponding matrix reports", "cases":[]}
try:
    for directory in ["matrix-mp4-mpeg-original","matrix-mp4-mpeg-fixed15","matrix-wmv-original","matrix-wmv-fixed15","matrix-rtx2x"]:
        matrix = json.loads((args.root/directory/"report.json").read_text())
        assert matrix["status"] == "passed"
        for case in matrix["cases"]:
            native = case["native"]
            label, route, fps = case["label"],case["route"],case["fps"]
            rtx = directory == "matrix-rtx2x"
            name = f"{label}-{route}-{fps}" if rtx else f"{label}-{route}"
            output = args.root/directory/f"{name}.mp4"
            metadata=json.loads(run(["ffprobe","-v","error","-select_streams","v:0","-show_streams","-show_packets","-show_entries",
                "stream=width,height,time_base,sample_aspect_ratio,has_b_frames:stream_tags=encoder:packet=pts,dts,duration","-of","json",output]))
            stream=metadata["streams"][0];packets=metadata["packets"];base=Fraction(stream["time_base"])
            assert (stream["width"],stream["height"]) == (native["output_width"],native["output_height"])
            assert Fraction(stream["sample_aspect_ratio"].replace(":","/")) == 1
            if rtx:
                assert (stream["width"],stream["height"]) == (native["input"]["display_width"]*2,native["input"]["display_height"]*2)
            pts,end,origin=truth[label]
            if fps=="original":
                expected_end=end-origin;expected_count=len(pts)
            else:
                rate=Fraction(fps);first=math.floor((pts[0]-origin)*rate);last=math.ceil((end-origin)*rate)
                expected_end=Fraction(last)/rate;expected_count=last-first
            assert len(packets)==native["output_frame_count"]==expected_count
            actual_end=max((int(p["pts"])+int(p["duration"]))*base for p in packets)
            error=abs(actual_end-expected_end);assert error<=Fraction(1,1000),(label,route,fps,float(error))
            gpu=route in ("nvidia","wgpu-nvidia")
            if gpu:
                assert stream["tags"]["encoder"].split()[-1]==native["video_encoder"]=="h264_nvenc"
                mode="nvdec" if label in ("h264","hevc") else "software"
                assert native["video_decode"]["decoder"]==mode
                download=(native["input"]["width"]*native["input"]["height"]*3//2*len(pts)) if mode=="nvdec" and route=="wgpu-nvidia" else 0
                upload=(stream["width"]*stream["height"]*3//2*(len(pts) if route=="wgpu-nvidia" else len(packets))) if route=="wgpu-nvidia" or mode=="software" else 0
                assert native["codec_gpu_to_cpu_bytes"]==download
                assert native["codec_cpu_to_gpu_bytes"]==upload
                if rtx:
                    assert stream["has_b_frames"]==0 and all(p["pts"]==p["dts"] for p in packets)
            report["cases"].append({"label":label,"route":route,"fps":fps,"rtx2x":rtx,"frames":len(packets),
                "video_packet_end_error_seconds":float(error),"actual_video_encoder":stream.get("tags",{}).get("encoder"),
                "decoded_reordering_depth":stream["has_b_frames"],"codec_bridge_counts_verified":gpu,"status":"passed"})
    assert len(report["cases"])==120
    report["status"]="passed"
finally:
    with args.report.open("x",encoding="utf-8") as file:
        json.dump(report,file,indent=2);file.write("\n")
print(json.dumps({"status":report["status"],"cases":len(report["cases"])}))
