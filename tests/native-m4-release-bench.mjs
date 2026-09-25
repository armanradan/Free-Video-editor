// Reproducible M4 release-mode native route comparison. Run from the repo root:
// node tests/native-m4-release-bench.mjs
// Synthetic FFmpeg sources and local outputs stay under ignored tmp/m4.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

function run(program, args) {
  const result = spawnSync(program, args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  assert.equal(result.error, undefined, `${program}: ${result.error}`);
  assert.equal(result.status, 0, `${program} ${args.join(" ")}\n${result.stderr}`);
  return result;
}

function probe(file) {
  return JSON.parse(run("ffprobe", ["-v", "error", "-show_entries",
    "stream=codec_name,codec_type,pix_fmt,width,height,nb_frames,avg_frame_rate,sample_rate:format=duration,size",
    "-of", "json", file]).stdout);
}

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function audioMaximum(file) {
  const output = run("ffmpeg", ["-hide_banner", "-i", file, "-map", "0:a:0",
    "-af", "volumedetect", "-f", "null", "-"]).stderr;
  const maximum = Number(output.match(/max_volume:\s*([-\d.]+) dB/)?.[1]);
  assert.ok(Number.isFinite(maximum) && maximum > -60, `audio silent or missing: ${maximum}`);
  return maximum;
}

const cases = [
  { name: "1080p", width: 1920, height: 1080, frames: 60 },
  { name: "4k", width: 3840, height: 2160, frames: 30 },
];
const directory = path.resolve("tmp/m4/release-bench", `run-${Date.now()}-${process.pid}`);
fs.mkdirSync(directory, { recursive: true });
run("cargo", ["build", "--release", "--locked", "-p", "media-native", "--bin", "native-convert"]);
const metadata = JSON.parse(run("cargo", ["metadata", "--no-deps", "--format-version", "1"]).stdout);
const executable = path.join(metadata.target_directory, "release",
  process.platform === "win32" ? "native-convert.exe" : "native-convert");
const adapters = JSON.parse(run(executable, ["list-gpus"]).stdout);
const adapter = adapters.find(candidate => candidate.device_type === "DiscreteGpu")
  ?? adapters.find(candidate => candidate.device_type === "IntegratedGpu") ?? adapters[0];
assert.ok(adapter, "shared-wgpu route needs an adapter");

const evidence = {
  provenance: "Synthetic FFmpeg testsrc2 + seeded noise + sine; generated locally, CC0-1.0",
  operatingSystem: `${os.platform()} ${os.release()} ${os.arch()}`,
  rustc: run("rustc", ["--version"]).stdout.trim(),
  ffmpeg: run("ffmpeg", ["-version"]).stdout.split(/\r?\n/)[0],
  adapter,
  build: "cargo build --release --locked -p media-native --bin native-convert",
  cases: [],
};

for (const spec of cases) {
  const input = path.join(directory, `${spec.name}-input.mp4`);
  const filter = "noise=all_seed=17:alls=12:allf=t+u";
  const generationArgs = ["-hide_banner", "-loglevel", "error", "-n", "-f", "lavfi", "-i",
    `testsrc2=size=${spec.width}x${spec.height}:rate=30`, "-f", "lavfi", "-i",
    "sine=frequency=523:sample_rate=48000", "-vf", filter, "-frames:v", String(spec.frames),
    "-c:v", "libx264", "-preset", "ultrafast", "-crf", "24", "-pix_fmt", "yuv420p",
    "-c:a", "aac", "-b:a", "128k", "-ar", "48000", "-ac", "1", "-shortest",
    "-movflags", "+faststart", input];
  run("ffmpeg", generationArgs);
  const inputProbe = probe(input);
  const inputVideo = inputProbe.streams.find(stream => stream.codec_type === "video");
  const inputAudio = inputProbe.streams.find(stream => stream.codec_type === "audio");
  assert.equal(Number(inputVideo.nb_frames), spec.frames);
  assert.equal(inputVideo.width, spec.width);
  assert.equal(inputVideo.height, spec.height);
  assert.equal(inputVideo.codec_name, "h264");
  assert.equal(inputAudio.codec_name, "aac");
  const outputs = {};
  for (const route of ["direct", "wgpu"]) {
    const file = path.join(directory, `${spec.name}-${route}.mp4`);
    const args = ["convert", "--input", input, "--output", file, "--route", route,
      "--resize", "50", "--profile", "mp4-h264-aac"];
    if (route === "wgpu") args.push("--adapter-key", adapter.key);
    const wallStart = performance.now();
    const report = JSON.parse(run(executable, args).stdout);
    const wallMs = Math.round(performance.now() - wallStart);
    const info = probe(file);
    const video = info.streams.find(stream => stream.codec_type === "video");
    const audio = info.streams.find(stream => stream.codec_type === "audio");
    assert.equal(video.codec_name, "h264");
    assert.equal(video.pix_fmt, "yuv420p");
    assert.equal(video.width, spec.width / 2);
    assert.equal(video.height, spec.height / 2);
    assert.equal(Number(video.nb_frames), spec.frames);
    assert.equal(audio.codec_name, "aac");
    assert.equal(audio.sample_rate, "48000");
    assert.ok(Math.abs(Number(info.format.duration) - spec.frames / 30) < 0.06);
    assert.equal(report.frames_processed, spec.frames);
    if (route === "wgpu") {
      assert.equal(report.adapter.key, adapter.key);
      assert.equal(report.explicit_cpu_to_gpu_bytes, spec.frames * spec.width * spec.height * 4);
      assert.equal(report.explicit_gpu_to_cpu_bytes, spec.frames * spec.width * spec.height);
    }
    run("ffmpeg", ["-v", "error", "-i", file, "-f", "null", "-"]);
    outputs[route] = { file, report, wallMs, probe: info,
      audioMaxDb: audioMaximum(file), sha256: sha256(file) };
  }
  const comparison = run("ffmpeg", ["-hide_banner", "-i", outputs.direct.file,
    "-i", outputs.wgpu.file, "-lavfi", "psnr", "-f", "null", "-"]).stderr;
  const psnrDb = Number(comparison.match(/PSNR y:[^\n]*average:([\d.]+)/)?.[1]);
  assert.ok(Number.isFinite(psnrDb) && psnrDb > 25,
    `decoded route outputs diverged: ${spec.name} PSNR ${psnrDb}`);
  const item = { spec, input: { file: input, bytes: fs.statSync(input).size,
    sha256: sha256(input), generationArgs, probe: inputProbe }, outputs, decodedVideoPsnrDb: psnrDb };
  evidence.cases.push(item);
  fs.writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
  console.log(`${spec.name}: direct ${outputs.direct.report.elapsed_ms} ms, `
    + `wgpu ${outputs.wgpu.report.elapsed_ms} ms, PSNR ${psnrDb.toFixed(2)} dB`);
}
console.log(`Evidence and outputs: ${directory}`);
