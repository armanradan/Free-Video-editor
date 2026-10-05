// Explicit NVIDIA four-route gate; generated outputs remain under ignored tmp.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

function run(program, args) {
  const result = spawnSync(program, args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 120_000 });
  assert.equal(result.status, 0, `${program} ${args.join(" ")}\n${result.stderr}`);
  return result.stdout;
}
run("cargo", ["build", "--locked", "-p", "media-native", "--bin", "native-convert"]);
const metadata = JSON.parse(run("cargo", ["metadata", "--no-deps", "--format-version", "1"]));
const executable = path.join(metadata.target_directory, "debug", process.platform === "win32" ? "native-convert.exe" : "native-convert");
const adapter = JSON.parse(run(executable, ["list-gpus"])).find(value => value.vendor === 0x10de);
assert.ok(adapter, "NVIDIA adapter required");
const directory = path.resolve(`tmp/m5/frame-rate-${process.pid}`);
fs.mkdirSync(directory, { recursive: true });
const evidence = { adapter, results: [], status: "running" };
const probe = file => JSON.parse(run("ffprobe", ["-v", "error", "-show_streams", "-show_format", "-of", "json", file]));
const frames = file => JSON.parse(run("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_frames", "-show_entries", "frame=best_effort_timestamp_time", "-of", "json", file])).frames.map(frame => Number(frame.best_effort_timestamp_time));
try {
  for (const fixture of ["m2-h264-aac", "m35-vfr-offset"]) {
    const source = path.resolve(`fixtures/${fixture}.mp4`);
    const sourceInfo = probe(source);
    const sourceVideo = sourceInfo.streams.find(value => value.codec_type === "video");
    const sourceAudio = sourceInfo.streams.find(value => value.codec_type === "audio");
    const inputPts = frames(source);
    for (const route of ["direct", "nvidia", "wgpu", "wgpu-nvidia"]) {
      for (const fps of ["original", "15", "60", "30000/1001"]) {
        const output = path.join(directory, `${fixture}-${route}-${fps.replace("/", "_")}.mp4`);
        const report = JSON.parse(run(executable, ["convert", "--input", source, "--output", output, "--route", route, "--fps", fps, "--bitrate", "recommended", "--adapter-key", adapter.key]));
        const info = probe(output);
        const video = info.streams.find(value => value.codec_type === "video");
        const audio = info.streams.find(value => value.codec_type === "audio");
        const pts = frames(output);
        assert.equal(report.frames_processed, inputPts.length);
        assert.equal(report.output_frame_count, pts.length);
        assert.equal(audio.codec_name, "aac");
        const origin = Math.min(Number(sourceVideo.start_time), Number(sourceAudio.start_time));
        if (fps === "original") {
          assert.equal(pts.length, inputPts.length);
          pts.forEach((value, index) => assert.ok(Math.abs(value - (inputPts[index] - origin)) <= .001));
        } else {
          const [num, den = 1] = fps.split("/").map(Number);
          const rate = num / den;
          const first = Math.round((inputPts[0] - origin) * rate);
          const end = Math.ceil((Number(sourceVideo.start_time) + Number(sourceVideo.duration) - origin) * rate - 1e-8);
          assert.equal(pts.length, end - first);
          pts.forEach((value, index) => assert.ok(Math.abs(value - (first + index) / rate) <= .001, `${index}: ${value}`));
        }
        assert.ok(Math.abs(Number(audio.duration) - Number(sourceAudio.duration)) < .025);
        run("ffmpeg", ["-v", "error", "-nostdin", "-i", output, "-fps_mode", "passthrough", "-enc_time_base:v", "demux", "-f", "null", "-"]);
        evidence.results.push({ fixture, route, fps, report, frames: pts.length, averageFps: video.avg_frame_rate });
        console.log(`${fixture} ${route} ${fps}: ${inputPts.length} → ${pts.length}`);
      }
    }
  }
  for (const route of ["direct", "nvidia"]) {
    const output = path.join(directory, `hevc-${route}.mp4`);
    const report = JSON.parse(run(executable, ["convert", "--input", path.resolve("fixtures/m2-h264-aac.mp4"), "--output", output, "--route", route, "--profile", "mp4-h265-main10-aac", "--fps", "25", "--bitrate", "higher", "--adapter-key", adapter.key]));
    assert.equal(report.output_frame_count, 50);
    assert.equal(probe(output).streams.find(value => value.codec_type === "video").profile, "Main 10");
    run("ffmpeg", ["-v", "error", "-i", output, "-f", "null", "-"]);
    evidence.results.push({ route, profile: "hevc-main10", fps: 25, report });
  }
  const short = path.join(directory, "two-frames.mp4");
  run("ffmpeg", ["-v", "error", "-n", "-i", "fixtures/m2-h264-aac.mp4", "-map", "0:v:0", "-frames:v", "2", "-c:v", "libx264", short]);
  for (const route of ["direct", "nvidia", "wgpu", "wgpu-nvidia"]) {
    const output = path.join(directory, `two-frames-${route}.mp4`);
    const report = JSON.parse(run(executable, ["convert", "--input", short, "--output", output, "--route", route, "--fps", "30", "--adapter-key", adapter.key]));
    assert.equal(report.output_frame_count, 2, "microsecond-rounded EOF must not add a third frame");
    assert.equal(frames(output).length, 2);
    run("ffmpeg", ["-v", "error", "-i", output, "-f", "null", "-"]);
    evidence.results.push({ fixture: "two-frames", route, fps: 30, report });
  }
  const invalidOutput = path.join(directory, "invalid.mp4");
  const invalid = spawnSync(executable, ["convert", "--input", "fixtures/m2-h264-aac.mp4", "--output", invalidOutput, "--fps", "0"], { encoding: "utf8" });
  assert.notEqual(invalid.status, 0);
  assert.equal(fs.existsSync(invalidOutput), false);
  evidence.status = "passed";
} catch (error) {
  evidence.status = "failed";
  evidence.error = String(error.stack ?? error);
  throw error;
} finally {
  fs.writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
  console.log(`Evidence: ${path.join(directory, "evidence.json")}`);
}
