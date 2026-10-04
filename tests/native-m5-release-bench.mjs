// Serial release-mode comparison of the four existing native routes.
// Synthetic CC0 fixtures/results stay in ignored tmp/m5/release-bench.
// Usage: node tests/native-m5-release-bench.mjs [--runs=3] [--adapter-key=KEY]
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

export const routes = ["direct", "nvidia", "wgpu", "wgpu-nvidia"];

export function statistics(values) {
  assert.ok(values.length && values.every(value => Number.isFinite(value) && value >= 0));
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return { min: sorted[0], median: sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2,
    max: sorted.at(-1), samples: values.length };
}

export function orderForRound(round) {
  // Rotate first route to reduce order/warm-cache bias, not eliminate it.
  return routes.map((_, index) => routes[(index + round) % routes.length]);
}

export function expectedTransfers(spec, route) {
  const shared = route.startsWith("wgpu"), staged = route === "wgpu-nvidia";
  const inputPixels = spec.width * spec.height * spec.frames;
  const outputPixels = inputPixels / 4;
  return {
    explicit_cpu_to_gpu_bytes: shared ? inputPixels * 4 : 0,
    explicit_gpu_to_cpu_bytes: shared ? outputPixels * 4 : 0,
    codec_gpu_to_cpu_bytes: staged ? inputPixels * 1.5 : 0,
    codec_cpu_to_gpu_bytes: staged ? outputPixels * 1.5 : 0,
  };
}

function run(program, args) {
  const result = spawnSync(program, args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024,
    timeout: 600_000, windowsHide: true });
  assert.equal(result.error, undefined, `${program}: ${result.error}`);
  assert.equal(result.status, 0, `${program} ${args.join(" ")}\n${result.stderr}`);
  return result;
}

function probe(file) {
  return JSON.parse(run("ffprobe", ["-v", "error", "-show_entries",
    "stream=codec_name,codec_type,pix_fmt,width,height,nb_frames,sample_rate,start_time,duration,color_range,color_space,color_transfer,color_primaries:format=duration,size",
    "-of", "json", file]).stdout);
}

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function verify(file, spec) {
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
  for (const field of ["color_space", "color_transfer", "color_primaries"]) assert.equal(video[field], "bt709");
  assert.equal(video.color_range, "tv");
  assert.ok(Math.abs(Number(info.format.duration) - spec.frames / 30) < 0.08);
  assert.ok(Math.abs(Number(video.start_time) - Number(audio.start_time)) < 0.025);
  assert.ok(Math.abs(Number(video.duration) - Number(audio.duration)) < 0.08);
  const times = run("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_entries",
    "frame=best_effort_timestamp_time", "-of", "csv=p=0", file]).stdout
    .split(/\r?\n/).filter(line => line.trim()).map(line => Number(line.split(",")[0]));
  assert.equal(times.length, spec.frames);
  const maxTimestampErrorUs = Math.max(...times.map((value, index) => Math.abs(value - index / 30) * 1e6));
  assert.ok(maxTimestampErrorUs <= 20, `timestamp error ${maxTimestampErrorUs} µs`);
  run("ffmpeg", ["-v", "error", "-nostdin", "-i", file, "-fps_mode", "passthrough",
    "-enc_time_base:v", "demux", "-f", "null", "-"]);
  const volume = run("ffmpeg", ["-hide_banner", "-nostdin", "-i", file, "-map", "0:a:0",
    "-af", "volumedetect", "-f", "null", "-"]).stderr;
  const audioMaxDb = Number(volume.match(/max_volume:\s*([-\d.]+) dB/)?.[1]);
  assert.ok(Number.isFinite(audioMaxDb) && audioMaxDb > -60);
  return { probe: info, maxTimestampErrorUs, audioMaxDb };
}

function quality(input, output, spec) {
  // Compare to an uncompressed bilinear resize of the decoded source, not to
  // another lossy encoder. This is fidelity evidence, not matched-rate tuning.
  const filter = `[0:v]scale=${spec.width / 2}:${spec.height / 2}:flags=bilinear,format=yuv420p,split=2[rp][rs];`
    + "[1:v]split=2[op][os];[rp][op]psnr[p];[rs][os]ssim[s]";
  const result = run("ffmpeg", ["-hide_banner", "-nostdin", "-i", input, "-i", output,
    "-filter_complex", filter, "-map", "[p]", "-map", "[s]", "-an", "-f", "null", "-"]).stderr;
  const psnrDb = Number(result.match(/PSNR y:[^\n]*average:([\d.]+)/)?.[1]);
  const ssim = Number(result.match(/SSIM Y:[^\n]*All:([\d.]+)/)?.[1]);
  assert.ok(Number.isFinite(psnrDb) && psnrDb > 25, `PSNR diverged: ${psnrDb}`);
  assert.ok(Number.isFinite(ssim) && ssim > 0.8 && ssim <= 1, `SSIM diverged: ${ssim}`);
  return { psnrDb, ssim };
}

async function main() {
  const option = name => process.argv.slice(2).find(value => value.startsWith(`--${name}=`))?.slice(name.length + 3);
  const repetitions = Number(option("runs") ?? 3);
  assert.ok(Number.isInteger(repetitions) && repetitions >= 2 && repetitions <= 10, "--runs must be 2..10");
  const directory = path.resolve("tmp/m5/release-bench", `run-${Date.now()}-${process.pid}`);
  fs.mkdirSync(directory, { recursive: true });
  const save = () => fs.writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
  console.log("Building release native harness…");
  run("cargo", ["build", "--release", "--locked", "-p", "media-native", "--bin", "native-convert"]);
  const metadata = JSON.parse(run("cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"]).stdout);
  const executable = path.join(metadata.target_directory, "release", process.platform === "win32" ? "native-convert.exe" : "native-convert");
  const adapters = JSON.parse(run(executable, ["list-gpus"]).stdout);
  const key = option("adapter-key");
  const adapter = key ? adapters.find(item => item.key === key)
    : adapters.find(item => item.vendor === 0x10de && item.backend === "Dx12") ?? adapters.find(item => item.vendor === 0x10de);
  assert.ok(adapter?.vendor === 0x10de, "A uniquely resolvable NVIDIA adapter is required; no passing hardware skip");
  const evidence = {
    status: "running", provenance: "Locally generated FFmpeg testsrc2 + seeded noise + sine, CC0-1.0",
    operatingSystem: `${os.platform()} ${os.release()} ${os.arch()}`, cpu: os.cpus()[0]?.model,
    rustc: run("rustc", ["--version"]).stdout.trim(), ffmpeg: run("ffmpeg", ["-version"]).stdout.split(/\r?\n/)[0],
    gpuDriver: run("nvidia-smi", ["--query-gpu=name,uuid,driver_version", "--format=csv,noheader"]).stdout.trim(),
    adapter, repetitions, profile: "mp4-h264-aac", resize: "50",
    policy: "All runs serial, one warm-up per route/workload excluded, rotating timed order, fresh CLI/cold session inspection; timings exclude external validation and fixture generation. Current libx264 veryfast CRF20 and NVENC p4 VBR CQ20 are not equal quality/bitrate settings. CPU/power/VRAM/opaque memory/internal transfers unmeasured. No universal speedup claim.",
    cases: [],
  };
  save();
  try {
  for (const spec of [{ name: "1080p", width: 1920, height: 1080, frames: 180 },
    { name: "4k", width: 3840, height: 2160, frames: 90 }]) {
    const input = path.join(directory, `${spec.name}-source.mp4`);
    const generationArgs = ["-hide_banner", "-v", "error", "-nostdin", "-n", "-f", "lavfi", "-i",
      `testsrc2=size=${spec.width}x${spec.height}:rate=30`, "-f", "lavfi", "-i", "sine=frequency=523:sample_rate=48000",
      "-vf", "noise=all_seed=17:alls=12:allf=t+u", "-frames:v", String(spec.frames), "-c:v", "libx264",
      "-preset", "ultrafast", "-crf", "24", "-x264-params", "colorprim=bt709:transfer=bt709:colormatrix=bt709",
      "-pix_fmt", "yuv420p", "-color_range", "tv", "-colorspace", "bt709",
      "-color_trc", "bt709", "-color_primaries", "bt709", "-c:a", "aac", "-b:a", "128k", "-ar", "48000",
      "-ac", "1", "-shortest", "-movflags", "+faststart", input];
    run("ffmpeg", generationArgs);
    const inputProbe = probe(input);
    const sourceVideo = inputProbe.streams.find(item => item.codec_type === "video");
    assert.equal(Number(sourceVideo.nb_frames), spec.frames);
    for (const field of ["color_space", "color_transfer", "color_primaries"]) assert.equal(sourceVideo[field], "bt709", `fixture ${field}`);
    const item = { spec, input: { file: input, generationArgs, bytes: fs.statSync(input).size, sha256: sha256(input), probe: inputProbe }, runs: [], summary: {} };
    evidence.cases.push(item);
    save();
    for (let round = -1; round < repetitions; round++) {
      for (const route of orderForRound(round < 0 ? 0 : round)) {
        const warmup = round < 0;
        const file = path.join(directory, `${spec.name}-${route}-${warmup ? "warmup" : round + 1}.mp4`);
        const args = ["convert", "--input", input, "--output", file, "--route", route, "--resize", "50", "--profile", "mp4-h264-aac"];
        if (route !== "direct") args.push("--adapter-key", adapter.key);
        const started = performance.now();
        const report = JSON.parse(run(executable, args).stdout);
        const processWallMs = performance.now() - started;
        assert.equal(report.frames_processed, spec.frames);
        assert.equal(report.inspection_reused, false);
        assert.ok(report.total_ms >= report.preflight_ms + report.elapsed_ms + report.verification_ms);
        assert.equal(report.video_encoder, route.includes("nvidia") ? "h264_nvenc" : "libx264");
        for (const [field, bytes] of Object.entries(expectedTransfers(spec, route))) assert.equal(report[field], bytes, `${route} ${field}`);
        if (route.startsWith("wgpu")) assert.equal(report.adapter.key, adapter.key);
        if (route.includes("nvidia")) assert.equal(report.hardware_gpu.name, adapter.name);
        const validation = verify(file, spec);
        const runInfo = { route, round, warmup, file, sha256: sha256(file), processWallMs, report, validation };
        item.runs.push(runInfo);
        save();
        console.log(`${spec.name} ${route} ${warmup ? "warmup" : round + 1}: total ${report.total_ms} ms, conversion ${report.elapsed_ms} ms`);
      }
    }
    for (const route of routes) {
      const selected = item.runs.filter(run => run.route === route && !run.warmup);
      const summary = { quality: { ...quality(input, selected[0].file, spec), file: selected[0].file, round: selected[0].round } };
      for (const field of ["total_ms", "preflight_ms", "elapsed_ms", "verification_ms", "output_bytes"]) summary[field] = statistics(selected.map(run => run.report[field]));
      summary.processWallMs = statistics(selected.map(run => run.processWallMs));
      summary.conversionFps = spec.frames * 1000 / summary.elapsed_ms.median;
      summary.endToEndFps = spec.frames * 1000 / summary.total_ms.median;
      item.summary[route] = summary;
    }
    save();
  }
  evidence.status = "passed";
  save();
  console.log(JSON.stringify({ directory, summary: evidence.cases.map(item => ({ spec: item.spec, routes: item.summary })) }, null, 2));
  } catch (error) {
    evidence.status = "failed";
    evidence.failure = String(error.stack ?? error);
    save();
    throw error;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) await main();
