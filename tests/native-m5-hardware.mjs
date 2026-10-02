// Explicit NVIDIA runtime gate. Requires nvidia-smi, CUDA/NVENC FFmpeg and a
// compatible NVIDIA device; failure is not converted into a passing skip.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

function run(program, args) {
  const result = spawnSync(program, args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  assert.equal(result.status, 0, `${program} ${args.join(" ")}\n${result.stderr}`);
  return result.stdout;
}
run("cargo", ["build", "--locked", "-p", "media-native", "--bin", "native-convert"]);
const metadata = JSON.parse(run("cargo", ["metadata", "--no-deps", "--format-version", "1"]));
const executable = path.join(metadata.target_directory, "debug", process.platform === "win32" ? "native-convert.exe" : "native-convert");
const adapters = JSON.parse(run(executable, ["list-gpus"]));
const selected = adapters.find(adapter => adapter.vendor === 0x10de);
assert.ok(selected, "NVIDIA adapter is required for this hardware runtime gate");
const directory = path.resolve(`tmp/m5/hardware-${process.pid}`);
fs.mkdirSync(directory, { recursive: true });
// The checked-in 160x96 HEVC source is below this GPU's NVDEC minimum height.
// Derive a tiny 320x192 fixture, keeping its CC0 provenance and 10-bit samples.
const main10Source = path.join(directory, "main10-source.mp4");
run("ffmpeg", ["-v", "error", "-nostdin", "-n", "-i", "fixtures/m4-10bit-sdr.mp4",
  "-vf", "scale=320:192", "-c:v", "libx265", "-preset", "ultrafast",
  "-x265-params", "log-level=error", "-pix_fmt", "yuv420p10le", "-c:a", "copy", main10Source]);
const frames = file => run("ffprobe", ["-v", "error", "-select_streams", "v:0",
  "-show_entries", "frame=best_effort_timestamp_time", "-of", "csv=p=0", file])
  .split(/\r?\n/).filter(line => line.trim()).map(line => Number(line.split(",")[0]));
const evidence = [];
let decodedVideoPsnrDb;
let sharedDecodedVideoPsnrDb;
for (const [name, input, profile, expectedFrames] of [
  ["cfr", "fixtures/m2-h264-aac.mp4", "mp4-h264-aac", 60],
  ["vfr", "fixtures/m35-vfr-offset.mp4", "mp4-h264-aac", 36],
  ["hevc-input", "fixtures/m36-h265-aac.mp4", "mp4-h264-aac", 30],
  ["main10", main10Source, "mp4-h265-main10-aac", 24],
  ["shared-cfr", "fixtures/m2-h264-aac.mp4", "mp4-h264-aac", 60],
  ["shared-vfr", "fixtures/m35-vfr-offset.mp4", "mp4-h264-aac", 36],
  ["shared-hevc-input", "fixtures/m36-h265-aac.mp4", "mp4-h264-aac", 30],
]) {
  const shared = name.startsWith("shared-");
  const output = path.join(directory, `${name}.mp4`);
  const report = JSON.parse(run(executable, ["convert", "--input", input, "--output", output,
    "--route", shared ? "wgpu-nvidia" : "nvidia", "--adapter-key", selected.key, "--resize", "50", "--profile", profile]));
  assert.equal(report.route, shared ? "shared-wgpu-nvidia" : "nvidia-ffmpeg");
  assert.equal(report.video_encoder, name === "main10" ? "hevc_nvenc" : "h264_nvenc");
  assert.equal(report.hardware_gpu.name, selected.name);
  assert.match(report.hardware_gpu.uuid, /^GPU-/);
  const inputPixels = report.input.width * report.input.height * expectedFrames;
  const outputPixels = report.output_width * report.output_height * expectedFrames;
  assert.equal(report.explicit_cpu_to_gpu_bytes, shared ? inputPixels * 4 : 0);
  assert.equal(report.explicit_gpu_to_cpu_bytes, shared ? outputPixels * 4 : 0);
  assert.equal(report.codec_gpu_to_cpu_bytes, shared ? inputPixels * 1.5 : 0);
  assert.equal(report.codec_cpu_to_gpu_bytes, shared ? outputPixels * 1.5 : 0);
  if (shared) assert.equal(report.adapter.name, report.hardware_gpu.name);
  assert.equal(report.inspection_reused, false, "standalone CLI processes cannot share a session cache");
  assert.ok(report.total_ms >= report.preflight_ms + report.elapsed_ms + report.verification_ms);
  assert.equal(report.frames_processed, expectedFrames);
  const probe = JSON.parse(run("ffprobe", ["-v", "error", "-show_streams", "-of", "json", output]));
  const video = probe.streams.find(stream => stream.codec_type === "video");
  const audio = probe.streams.find(stream => stream.codec_type === "audio");
  assert.equal(Number(video.nb_frames), expectedFrames);
  assert.equal(audio.codec_name, "aac");
  assert.equal(video.width, report.output_width);
  assert.equal(video.height, report.output_height);
  for (const tag of ["color_range", "color_space", "color_transfer", "color_primaries"])
    if (report.input[tag] && report.input[tag] !== "unknown") assert.equal(video[tag], report.input[tag], `${name} ${tag}`);
  if (name === "main10") {
    assert.equal(video.profile, "Main 10");
    assert.equal(video.pix_fmt, "yuv420p10le");
    assert.equal(video.codec_tag_string, "hvc1");
    const pixels = spawnSync("ffmpeg", ["-v", "error", "-i", output, "-frames:v", "1",
      "-pix_fmt", "yuv420p10le", "-f", "rawvideo", "pipe:1"], { maxBuffer: 160 * 96 * 4 });
    assert.equal(pixels.status, 0, pixels.stderr.toString());
    let fineLuma = 0;
    for (let sample = 0; sample < 160 * 96; sample++)
      if (pixels.stdout.readUInt16LE(sample * 2) % 4 !== 0) fineLuma++;
    assert.ok(fineLuma > 1000, `Main 10 lost fine luma precision: ${fineLuma}`);
  }
  if (name === "cfr" || name === "shared-cfr") {
    const reference = path.join(directory, "software-reference.mp4");
    if (!fs.existsSync(reference)) run(executable, ["convert", "--input", input, "--output", reference, "--route", "direct", "--resize", "50"]);
    const comparison = spawnSync("ffmpeg", ["-hide_banner", "-i", reference, "-i", output,
      "-lavfi", "psnr", "-f", "null", "-"], { encoding: "utf8" });
    assert.equal(comparison.status, 0);
    const psnr = Number(comparison.stderr.match(/PSNR y:[^\n]*average:([\d.]+)/)?.[1]);
    if (shared) sharedDecodedVideoPsnrDb = psnr; else decodedVideoPsnrDb = psnr;
    assert.ok(Number.isFinite(psnr) && psnr > 30,
      `${name} image diverges from software reference: ${psnr} dB`);
  }
  const before = frames(input), after = frames(output);
  assert.equal(after.length, before.length);
  const origin = Math.min(report.input.video_start_us, report.input.audio_start_us) / 1e6;
  const maxErrorUs = Math.max(...before.map((time, index) => Math.abs(time - origin - after[index]) * 1e6));
  assert.ok(maxErrorUs <= 1000, `${name}: frame timing changed by ${maxErrorUs} µs`);
  run("ffmpeg", ["-v", "error", "-i", output, "-f", "null", "-"]);
  const volume = spawnSync("ffmpeg", ["-hide_banner", "-i", output, "-map", "0:a:0",
    "-af", "volumedetect", "-f", "null", "-"], { encoding: "utf8" });
  assert.equal(volume.status, 0);
  const maxDb = Number(volume.stderr.match(/max_volume:\s*([-\d.]+) dB/)?.[1]);
  assert.ok(Number.isFinite(maxDb) && maxDb > -60);
  evidence.push({ name, report, maxErrorUs, audioMaxDb: maxDb });
}
// Never silently run on another device or fall back to software.
for (const key of ["missing-adapter", adapters.find(adapter => adapter.vendor !== 0x10de)?.key].filter(Boolean)) {
  const output = path.join(directory, `invalid-${evidence.length}.mp4`);
  const result = spawnSync(executable, ["convert", "--input", "fixtures/m2-h264-aac.mp4",
    "--output", output, "--route", "nvidia", "--adapter-key", key], { encoding: "utf8" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /unavailable|uniquely identifiable NVIDIA/);
  assert.equal(fs.existsSync(output), false);
}
// The negative hardware capability case must clean up and expose diagnostics.
const unsupported = path.join(directory, "unsupported-tiny-hevc.mp4");
const rejection = spawnSync(executable, ["convert", "--input", "fixtures/m4-10bit-sdr.mp4",
  "--output", unsupported, "--route", "nvidia", "--profile", "mp4-h265-main10-aac"], { encoding: "utf8" });
assert.notEqual(rejection.status, 0);
assert.match(rejection.stderr, /NVIDIA FFmpeg.*no CPU fallback/);
assert.match(rejection.stderr, /height|hwaccel|encoder/);
assert.equal(fs.existsSync(unsupported), false);
// Exercise the new hardware/wgpu pair during real output, not just preflight.
const longInput = path.join(directory, "cancel-source.mp4");
run("ffmpeg", ["-v", "error", "-nostdin", "-n", "-stream_loop", "19", "-i", "fixtures/m2-h264-aac.mp4",
  "-c", "copy", longInput]);
const cancelOutput = path.join(directory, "cancelled-staged.mp4");
const child = spawn(executable, ["convert", "--input", longInput, "--output", cancelOutput,
  "--route", "wgpu-nvidia", "--resize", "50", "--cancel-after-ms", "3000"]);
let cancellationError = "", sawPartial = false;
child.stderr.on("data", bytes => { cancellationError += bytes.toString(); });
child.stdout.resume();
const cancelled = new Promise(resolve => child.on("close", code => resolve(code)));
const interval = setInterval(() => {
  if (fs.readdirSync(directory).some(file => file.startsWith("cancelled-staged.") && file.endsWith(".partial.mp4"))) sawPartial = true;
}, 20);
const cancelledCode = await cancelled;
clearInterval(interval);
assert.notEqual(cancelledCode, 0);
assert.match(cancellationError, /cancelled/);
assert.ok(sawPartial, "cancellation must occur after codec output started");
assert.equal(fs.existsSync(cancelOutput), false);
const retryOutput = path.join(directory, "staged-retry.mp4");
const retry = JSON.parse(run(executable, ["convert", "--input", "fixtures/m35-vfr-offset.mp4",
  "--output", retryOutput, "--route", "wgpu-nvidia", "--resize", "50"]));
assert.equal(retry.frames_processed, 36);
run("ffmpeg", ["-v", "error", "-i", retryOutput, "-fps_mode", "passthrough", "-enc_time_base:v", "demux", "-f", "null", "-"]);
assert.equal(fs.readdirSync(directory).filter(file => file.includes(".partial.")).length, 0);
fs.writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
console.log(JSON.stringify({ directory, cases: evidence.map(({ name, report, maxErrorUs, audioMaxDb }) =>
  ({ name, frames: report.frames_processed, encoder: report.video_encoder, elapsedMs: report.elapsed_ms, maxErrorUs, audioMaxDb })),
  decodedVideoPsnrDb, sharedDecodedVideoPsnrDb, wrongAdapterRejected: true, unsupportedHardwareRejectedWithoutFallback: true,
  stagedCancellationAfterOutputStarted: sawPartial, stagedRetry: true, noPartialOutputs: true }, null, 2));
