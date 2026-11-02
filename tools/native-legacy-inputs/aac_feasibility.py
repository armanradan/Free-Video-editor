"""Tiny CC0 AAC timing fixtures and packet/sample-level feasibility checks.

Run from the repo root: python tools/native-legacy-inputs/aac_feasibility.py
Uses installed FFmpeg/ffprobe, no downloads. Generated audio/video is CC0-1.0.
The MP4 mutation is confined to self-generated, bounded test fixtures.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess


def run(argv):
    return subprocess.run(argv, check=True, capture_output=True, timeout=60).stdout


def sha(data):
    return hashlib.sha256(data).hexdigest()


def probe(path):
    return json.loads(run([
        "ffprobe", "-v", "error", "-select_streams", "a:0",
        "-show_packets", "-show_streams", "-show_data_hash", "sha256",
        "-of", "json", str(path),
    ]))


def boxes(data, begin, end):
    while begin < end:
        size, kind = struct.unpack_from(">I4s", data, begin)
        if size < 8 or begin + size > end:
            raise ValueError("Unsupported fixture box layout")
        yield begin, size, kind
        begin += size
    assert begin == end


def leading_pts_anomaly(reference, destination, packet_count):
    """Insert audio ctts at end of stbl; moov follows mdat so offsets stay fixed."""
    data = bytearray(reference.read_bytes())
    assert len(data) < 1024 * 1024
    top = list(boxes(data, 0, len(data)))
    moov = next(x for x in top if x[2] == b"moov")
    assert all(o < moov[0] for o, _, k in top if k == b"mdat")
    candidates = []

    def visit(offset, size, kind, ancestors):
        chain = ancestors + [offset]
        if kind == b"stbl":
            children = list(boxes(data, offset + 8, offset + size))
            stsd = next(x for x in children if x[2] == b"stsd")
            if b"mp4a" in data[stsd[0]:stsd[0] + stsd[1]]:
                assert not any(k == b"ctts" for _, _, k in children)
                candidates.append((offset + size, chain))
        elif kind in (b"moov", b"trak", b"mdia", b"minf"):
            for child in boxes(data, offset + 8, offset + size):
                visit(*child, chain)

    visit(*moov, [])
    assert len(candidates) == 1
    insertion, ancestors = candidates[0]
    ctts = struct.pack(">I4sIIIIII", 32, b"ctts", 0, 2, 1, 3600, packet_count - 1, 0)
    for offset in ancestors:
        struct.pack_into(">I", data, offset, struct.unpack_from(">I", data, offset)[0] + len(ctts))
    data[insertion:insertion] = ctts
    with destination.open("xb") as file:
        file.write(data)


def pcm(path):
    return run(["ffmpeg", "-v", "error", "-i", str(path), "-map", "0:a:0",
                "-f", "f32le", "-c:a", "pcm_f32le", "pipe:1"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("tmp/native-legacy-inputs/aac-feasibility"))
    parser.add_argument("--size", choices=["96x80", "192x144"], default="96x80")
    args = parser.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    commands = []

    def ffmpeg(arguments):
        command = ["ffmpeg", "-hide_banner", "-v", "warning", "-n", *map(str, arguments)]
        commands.append(command)
        run(command)

    reference = root / "reference.mp4"
    sources = ["-f", "lavfi", "-i", f"testsrc2=size={args.size}:rate=25:duration=1.4",
               "-f", "lavfi", "-i", "sine=frequency=997:sample_rate=44100:duration=1.4"]
    encode = ["-c:v", "libx264", "-threads", "1", "-pix_fmt", "yuv420p", "-bf", "0",
              "-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709",
              "-c:a", "aac"]
    ffmpeg([*sources, *encode, "-use_editlist", "0", reference])
    ref = probe(reference)
    anomaly = root / "leading-pts.mp4"
    leading_pts_anomaly(reference, anomaly, len(ref["packets"]))
    ffmpeg([*sources, *encode, root / "negative-priming.mp4"])
    ffmpeg(["-copyts", "-i", reference, "-map", "0", "-c", "copy",
            "-output_ts_offset", "0.25", root / "positive-offset.mp4"])
    ffmpeg(["-copyts", "-i", reference, "-map", "0", "-c", "copy", "-bsf:a",
            "setts=pts=PTS+if(gte(N\\,20)\\,8820\\,0):dts=DTS+if(gte(N\\,20)\\,8820\\,0)",
            root / "intentional-gap.mp4"])
    manifest = {}
    for name in ["reference", "leading-pts", "negative-priming", "positive-offset", "intentional-gap"]:
        file = root / (name + ".mp4")
        metadata = probe(file)
        manifest[name] = {"sha256": sha(file.read_bytes()), "bytes": file.stat().st_size,
                          "audio": metadata}
    bad = manifest["leading-pts"]["audio"]["packets"]
    assert [p["pts"] for p in bad[:3]] == [3600, 1024, 2048]
    assert [p["dts"] for p in bad[:3]] == [0, 1024, 2048]
    assert manifest["negative-priming"]["audio"]["packets"][0]["pts"] < 0
    assert manifest["positive-offset"]["audio"]["packets"][0]["pts"] > 0
    gap = manifest["intentional-gap"]["audio"]["packets"]
    assert gap[20]["pts"] - gap[19]["pts"] > 1024
    prepared = root / "prepared.m4a"
    before = sha(anomaly.read_bytes())
    ffmpeg(["-copyts", "-i", anomaly, "-map", "0:a:0", "-c:a", "copy", "-bsf:a",
            "setts=pts=if(eq(N\\,0)\\,DTS\\,PTS)", prepared])
    after = probe(prepared)
    assert before == sha(anomaly.read_bytes())
    assert len(bad) == len(after["packets"])
    assert after["streams"][0]["time_base"] == ref["streams"][0]["time_base"]
    changes = []
    for i, (a, b) in enumerate(zip(bad, after["packets"], strict=True)):
        for field in ["data_hash", "dts", "duration", "size"]:
            assert a[field] == b[field], (i, field, a[field], b[field])
        if a["pts"] != b["pts"]:
            changes.append({"index": i, "before": a["pts"], "after": b["pts"]})
        assert b["pts"] == ref["packets"][i]["pts"]
    assert changes == [{"index": 0, "before": 3600, "after": 0}]
    # A correct audio-only oracle avoids comparing different container seek
    # indexes/decoder startup histories during preview seek regression tests.
    ffmpeg(["-copyts", "-i", reference, "-map", "0:a:0", "-c:a", "copy", root / "reference.m4a"])
    expected, actual = pcm(reference), pcm(prepared)
    assert expected == actual
    controls = {}
    for name in ["negative-priming", "positive-offset", "intentional-gap"]:
        source = root / (name + ".mp4")
        copied = root / (name + ".m4a")
        original_hash = sha(source.read_bytes())
        ffmpeg(["-copyts", "-i", source, "-map", "0:a:0", "-c:a", "copy", copied])
        original_packets = manifest[name]["audio"]["packets"]
        copied_metadata = probe(copied)
        assert len(original_packets) == len(copied_metadata["packets"])
        assert manifest[name]["audio"]["streams"][0]["time_base"] == copied_metadata["streams"][0]["time_base"]
        for a, b in zip(original_packets, copied_metadata["packets"], strict=True):
            for field in ["pts", "dts", "duration", "data_hash", "side_data_list"]:
                assert a.get(field) == b.get(field), (name, field)
        control_pcm = pcm(source)
        assert control_pcm == pcm(copied)
        assert original_hash == sha(source.read_bytes())
        controls[name] = {"all_packet_timing_payload_and_skip_metadata_equal": True,
                          "decoded_pcm_equal": True, "pcm_bytes": len(control_pcm),
                          "pcm_sha256": sha(control_pcm), "original_unchanged": True}
    report = {"status": "passed", "license": "CC0-1.0",
              "provenance": "Original FFmpeg testsrc2 and sine; only first AAC ctts offset mutated",
              "ffmpeg": run(["ffmpeg", "-version"]).decode().splitlines()[0],
              "commands": commands, "fixtures": manifest, "changes": changes,
              "prepared_audio": after, "original_unchanged": True,
              "reference_audio": {"sha256": sha((root / "reference.m4a").read_bytes()),
                                  "audio": probe(root / "reference.m4a")},
              "decoded_f32le": {"bytes": len(actual), "sha256": sha(actual), "exact_reference_match": True},
              "valid_control_remux": controls,
              "controls_policy": "Valid controls must bypass preparation; no global timestamp reconstruction.",
              "scope": "Synthetic feasibility only; no production normalization or private-source acceptance."}
    (root / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "report": str(root / "report.json"),
                      "packets": len(bad), "decoded_pcm_bytes": len(actual)}))


if __name__ == "__main__":
    main()
