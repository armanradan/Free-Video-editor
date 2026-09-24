// Independent FFmpeg validation of the deterministic Chromium harness outputs.
// Requires ffprobe/ffmpeg on PATH. Run after all three browser modes complete.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

function run(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8", timeout: 30_000, windowsHide: true });
  assert.equal(result.status, 0, `${command}: ${result.error ?? result.stderr}`);
  return result;
}
for (const mode of ["worker", "main", "fallback"]) {
  const directory = path.resolve(`tmp/m33-chromium-${mode}`);
  const browser = JSON.parse(fs.readFileSync(path.join(directory, "evidence.json"), "utf8"));
  assert.equal(browser.completed, true, `${mode}: browser acceptance did not finish`);
  const report = [];
  for (const output of browser.profiles) {
    const input = output.outputPath;
    const probe = run("ffprobe", ["-v", "error", "-count_frames", "-show_entries",
      "stream=codec_name,codec_type,codec_tag_string,profile,width,height,sample_rate,channels,nb_read_frames,start_time,duration,extradata_size:format=duration,size", "-of", "json", input]);
    const metadata = JSON.parse(probe.stdout);
    const video = metadata.streams.find(stream => stream.codec_type === "video");
    const audio = metadata.streams.find(stream => stream.codec_type === "audio");
    const expectedVideoCodec = output.profile === "mp4-h264-aac"
      ? "h264"
      : output.profile === "mp4-h265-aac" ? "hevc" : "vp8";
    const isMp4 = expectedVideoCodec !== "vp8";
    assert.equal(video.codec_name, expectedVideoCodec);
    assert.equal(video.width, 320);
    assert.equal(video.height, 180);
    assert.equal(Number(video.nb_read_frames), 60);
    assert.ok(Math.abs(Number(metadata.format.duration) - 2) < 0.075);
    assert.ok(Math.abs(Number(video.start_time)) < 0.05);
    const decode = run("ffmpeg", ["-v", "warning", "-i", input, "-f", "null", "-"]);
    const entry = { profile: output.profile, metadata, probeWarnings: probe.stderr, decodeWarnings: decode.stderr, audioSeeks: [] };
    if (output.profile === "webm-vp8-video-only") {
      assert.equal(audio, undefined);
    } else {
      assert.equal(audio.codec_name, isMp4 ? "aac" : "opus");
      assert.equal(Number(audio.sample_rate), 48_000);
      assert.equal(audio.channels, 1);
      assert.equal(Number(audio.nb_read_frames), isMp4 ? 95 : 102);
      assert.ok(Math.abs(Number(video.start_time) - Number(audio.start_time)) < 0.05);
      for (const seconds of [0.1, 1, 1.8]) {
        const seek = run("ffmpeg", ["-hide_banner", "-ss", String(seconds), "-i", input,
          "-t", "0.05", "-map", "0:a:0", "-af", "volumedetect", "-f", "null", "-"]);
        const peak = Number(seek.stderr.match(/max_volume: ([\d.-]+) dB/)?.[1]);
        assert.ok(Number.isFinite(peak) && peak > -80, `No non-silent audio near ${seconds}s`);
        entry.audioSeeks.push({ seconds, peakDb: peak });
      }
    }
    if (isMp4) {
      const bytes = fs.readFileSync(input);
      const boxes = [];
      for (let offset = 0; offset + 8 <= bytes.length;) {
        let size = bytes.readUInt32BE(offset);
        if (size === 1) size = Number(bytes.readBigUInt64BE(offset + 8));
        if (size === 0) size = bytes.length - offset;
        assert.ok(size >= 8 && offset + size <= bytes.length, "Invalid top-level MP4 box");
        boxes.push({ type: bytes.toString("ascii", offset + 4, offset + 8), offset });
        offset += size;
      }
      const moov = boxes.find(box => box.type === "moov");
      const mdat = boxes.find(box => box.type === "mdat");
      assert.ok(moov && mdat && moov.offset < mdat.offset, "MP4 must be fast-start");
      entry.boxes = boxes;
    }
    report.push(entry);
    console.log(`PASS independent ${mode}/${output.acceleration}/${output.profile}: 60 frames; audio=${audio?.codec_name ?? "none"}; ${entry.audioSeeks.length} non-silent audio seeks; warnings=${Boolean(probe.stderr || decode.stderr)}`);
  }
  fs.writeFileSync(path.join(directory, "independent-inspection.json"), JSON.stringify(report, null, 2));
}
