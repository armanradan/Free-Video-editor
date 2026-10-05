// Explicit four-route Windows/NVIDIA gate, not a silently skipped hardware test.
// Generated testsrc2/noise/sine source is synthetic CC0; media stays in ignored tmp.
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
const directory = path.resolve(`tmp/m5/bitrate-${process.pid}`);
fs.mkdirSync(directory, { recursive: true });
const source = path.join(directory, "source.mp4");
run("ffmpeg", ["-v", "error", "-nostdin", "-n", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=30,noise=alls=10:allf=t:all_seed=42",
  "-f", "lavfi", "-i", "sine=frequency=523:sample_rate=48000", "-t", "6",
  "-c:v", "libx264", "-preset", "ultrafast", "-crf", "14", "-pix_fmt", "yuv420p",
  "-c:a", "aac", source]);
const evidence = { source, adapter, results: [], status: "running" };
const probe = file => JSON.parse(run("ffprobe", ["-v", "error", "-show_streams", "-show_format", "-of", "json", file]));
function convert(route, bitrate, profile = "mp4-h264-aac", resize = "original") {
  const output = path.join(directory, `${route}-${bitrate}-${profile}-${resize}.mp4`);
  const report = JSON.parse(run(executable, ["convert", "--input", source, "--output", output,
    "--route", route, "--bitrate", bitrate, "--profile", profile, "--resize", resize, "--adapter-key", adapter.key]));
  const info = probe(output);
  const video = info.streams.find(value => value.codec_type === "video");
  const audio = info.streams.find(value => value.codec_type === "audio");
  assert.equal(Number(video.nb_frames), 180);
  assert.equal(report.frames_processed, 180);
  assert.equal(video.width, resize === "50" ? 320 : 640);
  assert.equal(video.height, resize === "50" ? 180 : 360);
  assert.equal(video.codec_name, profile.includes("h265") ? "hevc" : "h264");
  if (profile.includes("h265")) assert.equal(video.profile, "Main 10");
  assert.equal(audio.codec_name, "aac");
  assert.equal(audio.sample_rate, "48000");
  assert.ok(Math.abs(Number(info.format.duration) - 6) < 0.1);
  // The runner itself verifies every output timestamp/geometry/color/audio duration
  // before publication; independently decode both tracks to check corruption.
  run("ffmpeg", ["-v", "error", "-nostdin", "-i", output, "-fps_mode", "passthrough", "-enc_time_base:v", "demux", "-f", "null", "-"]);
  if (bitrate !== "recommended") assert.equal(report.video_bitrate_bps, Number(bitrate) * 1_000_000);
  else assert.equal(report.video_bitrate_bps, Math.max(250_000, Math.ceil(video.width * video.height * 30 * (profile.includes("h265") ? 7 : 10) / 100)));
  evidence.results.push({ output, report, actualVideoBps: Number(video.bit_rate), actualBytes: Number(info.format.size) });
  console.log(`${route} ${bitrate} ${profile} ${resize}: target ${report.video_bitrate_bps}, actual ${video.bit_rate}, ${info.format.size} bytes`);
  return Number(info.format.size);
}
try {
  for (const route of ["direct", "nvidia", "wgpu", "wgpu-nvidia"]) {
    const low = convert(route, "1");
    const high = convert(route, "4");
    assert.ok(high > low * 1.2, `${route}: bitrate selection must materially change output size on the textured fixture`);
  }
  convert("direct", "recommended");
  convert("nvidia", "recommended", "mp4-h264-aac", "50");
  convert("direct", "recommended", "mp4-h265-main10-aac");
  convert("nvidia", "2", "mp4-h265-main10-aac");
  const invalidOutput = path.join(directory, "invalid.mp4");
  const invalid = spawnSync(executable, ["convert", "--input", source, "--output", invalidOutput, "--bitrate", "0"], { encoding: "utf8" });
  assert.notEqual(invalid.status, 0);
  assert.match(invalid.stderr, /bitrate must be between 0\.25 and 120 Mbps/);
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
