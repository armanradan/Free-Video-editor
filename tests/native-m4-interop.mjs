// M4 headless Windows/native correctness and route comparison.
// Run from the repository root: node tests/native-m4-interop.mjs
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

function run(program, args) {
  const result = spawnSync(program, args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  assert.equal(result.status, 0, `${program} ${args.join(" ")}\n${result.stderr}`);
  return result;
}

run("cargo", ["build", "--locked", "-p", "media-native", "--bin", "native-convert"]);
const metadata = JSON.parse(run("cargo", ["metadata", "--no-deps", "--format-version", "1"]).stdout);
const executable = path.join(metadata.target_directory, "debug", process.platform === "win32" ? "native-convert.exe" : "native-convert");
const adapters = JSON.parse(run(executable, ["list-gpus"]).stdout);
const selected = adapters.find(adapter => adapter.device_type === "DiscreteGpu")
  ?? adapters.find(adapter => adapter.device_type === "IntegratedGpu")
  ?? adapters[0];
assert.ok(selected, "M4 GPU route requires an available wgpu adapter");

const directory = path.resolve("tmp/m4/interop");
fs.mkdirSync(directory, { recursive: true });
const input = path.resolve("fixtures/m2-h264-aac.mp4");
const directOutput = path.join(directory, `direct-${process.pid}.mp4`);
const gpuOutput = path.join(directory, `wgpu-${process.pid}.mp4`);
const preference = path.join(directory, `adapter-${process.pid}.json`);
const saved = JSON.parse(run(executable, ["save-gpu", "--adapter-key", selected.key, "--preference", preference]).stdout);
assert.equal(saved.key, selected.key);

const convert = (route, output, additional = []) => JSON.parse(run(executable, ["convert",
  "--input", input, "--output", output, "--route", route,
  "--profile", "mp4-h264-aac", "--resize", "50", ...additional]).stdout);
const direct = convert("direct", directOutput);
const gpu = convert("wgpu", gpuOutput, ["--preference", preference]);
assert.equal(direct.frames_processed, 60);
assert.equal(gpu.frames_processed, 60);
assert.equal(gpu.adapter.key, selected.key);
assert.equal(direct.explicit_cpu_to_gpu_bytes, 0);
assert.equal(gpu.explicit_cpu_to_gpu_bytes, 60 * 640 * 360 * 4);
assert.equal(gpu.explicit_gpu_to_cpu_bytes, 60 * 320 * 180 * 4);

function inspect(file) {
  const probe = JSON.parse(run("ffprobe", ["-v", "error", "-show_entries",
    "stream=codec_name,codec_type,width,height,nb_frames:format=duration,size", "-of", "json", file]).stdout);
  const video = probe.streams.find(stream => stream.codec_type === "video");
  const audio = probe.streams.find(stream => stream.codec_type === "audio");
  assert.equal(video.codec_name, "h264");
  assert.equal(video.width, 320);
  assert.equal(video.height, 180);
  assert.equal(Number(video.nb_frames), 60);
  assert.equal(audio.codec_name, "aac");
  assert.equal(Number(audio.nb_frames), 95);
  assert.ok(Math.abs(Number(probe.format.duration) - 2) < 0.05);
  run("ffmpeg", ["-v", "error", "-ss", "1", "-i", file, "-frames:v", "1", "-f", "null", "-"]);
  const volume = run("ffmpeg", ["-hide_banner", "-i", file, "-map", "0:a:0", "-af", "volumedetect", "-f", "null", "-"]).stderr;
  const maximum = Number(volume.match(/max_volume:\s*([-\d.]+) dB/)?.[1]);
  assert.ok(Number.isFinite(maximum) && maximum > -60, `audio appears silent: ${maximum} dB`);
  return { bytes: Number(probe.format.size), durationSeconds: Number(probe.format.duration), audioMaxDb: maximum };
}
const directInspection = inspect(directOutput);
const gpuInspection = inspect(gpuOutput);
const comparison = run("ffmpeg", ["-hide_banner", "-i", directOutput, "-i", gpuOutput,
  "-lavfi", "psnr", "-f", "null", "-"]).stderr;
const psnrDb = Number(comparison.match(/PSNR y:[^\n]*average:([\d.]+)/)?.[1]);
assert.ok(Number.isFinite(psnrDb) && psnrDb > 30, `GPU and direct decoded video diverged: PSNR=${psnrDb}`);
const staleOutput = path.join(directory, `fallback-${process.pid}.mp4`);
const fallback = convert("wgpu", staleOutput, ["--adapter-key", "missing-native-adapter"]);
assert.match(fallback.adapter_fallback, /unavailable/);
assert.ok(fallback.adapter);
const vfrInput = path.resolve("fixtures/m35-vfr-offset.mp4");
const vfrOutput = path.join(directory, `direct-vfr-${process.pid}.mp4`);
const vfr = JSON.parse(run(executable, ["convert", "--input", vfrInput,
  "--output", vfrOutput, "--route", "direct", "--resize", "50"]).stdout);
assert.equal(vfr.frames_processed, 36);
assert.equal(vfr.input.variable_frame_rate, true);
const vfrProbe = JSON.parse(run("ffprobe", ["-v", "error", "-show_entries",
  "stream=codec_name,codec_type,width,height,start_time,nb_frames", "-of", "json", vfrOutput]).stdout);
const vfrVideo = vfrProbe.streams.find(stream => stream.codec_type === "video");
const vfrAudio = vfrProbe.streams.find(stream => stream.codec_type === "audio");
assert.equal(vfrVideo.codec_name, "h264");
assert.equal(vfrVideo.width, 160);
assert.equal(vfrVideo.height, 90);
assert.equal(Number(vfrVideo.nb_frames), 36);
assert.equal(vfrAudio.codec_name, "aac");
const frameTimes = file => run("ffprobe", ["-v", "error", "-select_streams", "v:0",
  "-show_entries", "frame=best_effort_timestamp_time", "-of", "csv=p=0", file]).stdout
  .split(/\r?\n/).filter(line => line.trim()).map(line => Number(line.split(",")[0]));
const before = frameTimes(vfrInput);
const after = frameTimes(vfrOutput);
assert.equal(before.length, 36);
assert.equal(after.length, 36);
const frameTimelineMaxErrorUs = Math.max(...before.map((seconds, index) =>
  Math.abs((seconds - 1.228) - after[index]) * 1_000_000));
assert.ok(frameTimelineMaxErrorUs <= 1000, `VFR timeline changed by ${frameTimelineMaxErrorUs} µs`);
const avOffsetErrorUs = Math.abs(((Number(vfrVideo.start_time) - Number(vfrAudio.start_time))
  - (1.250 - 1.228)) * 1_000_000);
assert.ok(avOffsetErrorUs <= 1000, `A/V offset changed by ${avOffsetErrorUs} µs`);
run("ffmpeg", ["-v", "error", "-i", vfrOutput, "-f", "null", "-"]);
const vfrVolume = run("ffmpeg", ["-hide_banner", "-i", vfrOutput, "-map", "0:a:0",
  "-af", "volumedetect", "-f", "null", "-"]).stderr;
const vfrAudioMaxDb = Number(vfrVolume.match(/max_volume:\s*([-\d.]+) dB/)?.[1]);
assert.ok(Number.isFinite(vfrAudioMaxDb) && vfrAudioMaxDb > -60);
const gpuVfrOutput = path.join(directory, `wgpu-vfr-rejected-${process.pid}.mp4`);
const rejectedGpuVfr = spawnSync(executable, ["convert", "--input", vfrInput,
  "--output", gpuVfrOutput, "--route", "wgpu"], { encoding: "utf8" });
assert.notEqual(rejectedGpuVfr.status, 0);
assert.match(rejectedGpuVfr.stderr, /zero-origin|frame rates|CFR/);
assert.equal(fs.existsSync(gpuVfrOutput), false);
const overwrite = spawnSync(executable, ["convert", "--input", input, "--output", directOutput,
  "--route", "direct"], { encoding: "utf8" });
assert.notEqual(overwrite.status, 0);
assert.match(overwrite.stderr, /refusing to overwrite output/);
const evidence = { fixture: "fixtures/m2-h264-aac.mp4 (CC0-1.0)", adapter: selected,
  direct, gpu, directInspection, gpuInspection, decodedVideoPsnrDb: psnrDb,
  staleAdapterFallback: fallback.adapter_fallback, directVfr: { report: vfr,
    frameTimelineMaxErrorUs, avOffsetErrorUs, audioMaxDb: vfrAudioMaxDb }, gpuVfrRejected: true, overwriteRejected: true,
  limitations: "Single debug-build run; software FFmpeg encode; GPU route performs explicit upload/readback and blocks per frame." };
fs.writeFileSync(path.join(directory, `evidence-${process.pid}.json`), JSON.stringify(evidence, null, 2));
console.log(JSON.stringify(evidence, null, 2));
