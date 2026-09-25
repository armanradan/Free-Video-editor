// M4 headless Windows/native correctness and route comparison.
// Run from the repository root: node tests/native-m4-interop.mjs
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
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
  "--output", vfrOutput, "--resize", "50"]).stdout);
assert.equal(vfr.route, "direct-ffmpeg");
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
const geometryInput = path.resolve("fixtures/m35-geometry-color.mp4");
const geometryOutput = path.join(directory, `direct-geometry-${process.pid}.mp4`);
const geometry = JSON.parse(run(executable, ["convert", "--input", geometryInput,
  "--output", geometryOutput, "--route", "direct", "--resize", "50"]).stdout);
assert.equal(geometry.frames_processed, 24);
assert.deepEqual([geometry.input.width, geometry.input.height], [316, 180]);
assert.deepEqual([geometry.input.display_width, geometry.input.display_height], [180, 421]);
assert.deepEqual([geometry.output_width, geometry.output_height], [90, 210]);
const geometryProbe = JSON.parse(run("ffprobe", ["-v", "error", "-show_entries",
  "stream=codec_name,codec_type,width,height,sample_aspect_ratio,color_range,color_space,color_transfer,color_primaries,nb_frames:stream_side_data=rotation",
  "-of", "json", geometryOutput]).stdout);
const geometryVideo = geometryProbe.streams.find(stream => stream.codec_type === "video");
const geometryAudio = geometryProbe.streams.find(stream => stream.codec_type === "audio");
assert.deepEqual([geometryVideo.width, geometryVideo.height], [90, 210]);
assert.equal(Number(geometryVideo.nb_frames), 24);
assert.equal(geometryVideo.sample_aspect_ratio, "1:1");
assert.equal(geometryVideo.side_data_list?.length ?? 0, 0);
for (const tag of ["color_range", "color_space", "color_transfer", "color_primaries"])
  assert.equal(geometryVideo[tag], geometry.input[tag], tag);
assert.equal(geometryAudio.codec_name, "aac");
run("ffmpeg", ["-v", "error", "-i", geometryOutput, "-f", "null", "-"]);
const rgb = spawnSync("ffmpeg", ["-v", "error", "-i", geometryOutput,
  "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"],
{ maxBuffer: 90 * 210 * 3 + 1024 });
assert.equal(rgb.status, 0, rgb.stderr.toString());
assert.equal(rgb.stdout.length, 90 * 210 * 3);
const pixel = (x, y) => [...rgb.stdout.subarray((y * 90 + x) * 3, (y * 90 + x) * 3 + 3)];
const corners = { TL: pixel(8, 8), TR: pixel(81, 8), BL: pixel(8, 201), BR: pixel(81, 201) };
assert.ok(corners.TL[0] > 150 && corners.TL[0] > corners.TL[1] * 2 && corners.TL[0] > corners.TL[2] * 2);
assert.ok(corners.TR[2] > 150 && corners.TR[2] > corners.TR[0] * 2 && corners.TR[2] > corners.TR[1] * 2);
assert.ok(corners.BL[1] > 70 && corners.BL[1] > corners.BL[0] * 2 && corners.BL[1] > corners.BL[2] * 2);
assert.ok(corners.BR[0] > 150 && corners.BR[1] > 150 && corners.BR[2] < 80);
const gpuGeometryOutput = path.join(directory, `wgpu-geometry-rejected-${process.pid}.mp4`);
const rejectedGpuGeometry = spawnSync(executable, ["convert", "--input", geometryInput,
  "--output", gpuGeometryOutput, "--route", "wgpu"], { encoding: "utf8" });
assert.notEqual(rejectedGpuGeometry.status, 0);
assert.match(rejectedGpuGeometry.stderr, /square-pixel/);
assert.equal(fs.existsSync(gpuGeometryOutput), false);
const taggedInput = path.resolve("fixtures/m36-h265-aac.mp4");
const taggedOutputs = {};
const taggedOutputFiles = {};
for (const route of ["direct", "wgpu"]) {
  const output = path.join(directory, `${route}-tagged-${process.pid}.mp4`);
  const report = JSON.parse(run(executable, ["convert", "--input", taggedInput,
    "--output", output, "--route", route, "--resize", "50"]).stdout);
  assert.equal(report.input.codec, "hevc");
  assert.equal(report.frames_processed, 30);
  const probe = JSON.parse(run("ffprobe", ["-v", "error", "-show_entries",
    "stream=codec_name,codec_type,width,height,color_range,color_space,color_transfer,color_primaries,nb_frames",
    "-of", "json", output]).stdout);
  const video = probe.streams.find(stream => stream.codec_type === "video");
  const audio = probe.streams.find(stream => stream.codec_type === "audio");
  assert.deepEqual([video.width, video.height, Number(video.nb_frames)], [160, 90, 30]);
  assert.equal(video.codec_name, "h264");
  assert.equal(audio.codec_name, "aac");
  for (const tag of ["color_range", "color_space", "color_transfer", "color_primaries"])
    assert.equal(video[tag], report.input[tag], `${route} ${tag}`);
  run("ffmpeg", ["-v", "error", "-ss", "0.5", "-i", output, "-frames:v", "1", "-f", "null", "-"]);
  taggedOutputs[route] = { frames: report.frames_processed, color: {
    range: video.color_range, space: video.color_space,
    transfer: video.color_transfer, primaries: video.color_primaries } };
  taggedOutputFiles[route] = output;
}
const taggedComparison = run("ffmpeg", ["-hide_banner", "-i", taggedOutputFiles.direct,
  "-i", taggedOutputFiles.wgpu, "-lavfi", "psnr", "-f", "null", "-"]).stderr;
const taggedPsnrDb = Number(taggedComparison.match(/PSNR y:[^\n]*average:([\d.]+)/)?.[1]);
assert.ok(Number.isFinite(taggedPsnrDb) && taggedPsnrDb > 30,
  `Tagged HEVC route outputs diverged: PSNR=${taggedPsnrDb}`);
const hdrInput = path.resolve("fixtures/m35-hdr-tagged.mp4");
for (const route of ["direct", "wgpu"]) {
  const output = path.join(directory, `${route}-hdr-rejected-${process.pid}.mp4`);
  const rejected = spawnSync(executable, ["convert", "--input", hdrInput,
    "--output", output, "--route", route], { encoding: "utf8" });
  assert.notEqual(rejected.status, 0);
  assert.match(rejected.stderr, /BT\.709 limited-range SDR; unsupported matrix: bt2020nc/);
  assert.equal(fs.existsSync(output), false);
}
const tenBitInput = path.resolve("fixtures/m4-10bit-sdr.mp4");
const tenBitManifest = JSON.parse(fs.readFileSync(path.resolve("fixtures/m4-10bit-sdr.json"), "utf8"));
assert.equal(fs.statSync(tenBitInput).size, tenBitManifest.size_bytes);
assert.equal(createHash("sha256").update(fs.readFileSync(tenBitInput)).digest("hex"), tenBitManifest.sha256);
const tenBitOutput = path.join(directory, `direct-main10-${process.pid}.mp4`);
const tenBit = JSON.parse(run(executable, ["convert", "--input", tenBitInput,
  "--output", tenBitOutput, "--route", "direct",
  "--profile", "mp4-h265-main10-aac", "--resize", "50"]).stdout);
assert.equal(tenBit.input.pixel_format, "yuv420p10le");
assert.equal(tenBit.input.codec_profile, "Main 10");
assert.equal(tenBit.frames_processed, 24);
const tenBitProbe = JSON.parse(run("ffprobe", ["-v", "error", "-show_entries",
  "stream=codec_name,codec_type,codec_tag_string,profile,pix_fmt,width,height,sample_aspect_ratio,color_range,color_space,color_transfer,color_primaries,nb_frames:format=duration",
  "-of", "json", tenBitOutput]).stdout);
const tenBitVideo = tenBitProbe.streams.find(stream => stream.codec_type === "video");
const tenBitAudio = tenBitProbe.streams.find(stream => stream.codec_type === "audio");
assert.deepEqual([tenBitVideo.codec_name, tenBitVideo.profile, tenBitVideo.codec_tag_string,
  tenBitVideo.pix_fmt, tenBitVideo.width, tenBitVideo.height, Number(tenBitVideo.nb_frames)],
["hevc", "Main 10", "hvc1", "yuv420p10le", 80, 48, 24]);
assert.equal(tenBitVideo.sample_aspect_ratio, "1:1");
assert.equal(tenBitAudio.codec_name, "aac");
for (const tag of ["color_range", "color_space", "color_transfer", "color_primaries"])
  assert.equal(tenBitVideo[tag], tenBit.input[tag], `Main 10 ${tag}`);
assert.ok(Math.abs(Number(tenBitProbe.format.duration) - 1) < 0.05);
run("ffmpeg", ["-v", "error", "-i", tenBitOutput, "-f", "null", "-"]);
run("ffmpeg", ["-v", "error", "-ss", "0.5", "-i", tenBitOutput, "-frames:v", "1", "-f", "null", "-"]);
const tenBitTimes = frameTimes(tenBitOutput);
assert.equal(tenBitTimes.length, 24);
assert.ok(tenBitTimes.every((time, index) => Math.abs(time - index / 24) < 0.001));
const tenBitVolume = run("ffmpeg", ["-hide_banner", "-i", tenBitOutput,
  "-map", "0:a:0", "-af", "volumedetect", "-f", "null", "-"]).stderr;
const tenBitAudioMaxDb = Number(tenBitVolume.match(/max_volume:\s*([-\d.]+) dB/)?.[1]);
assert.ok(Number.isFinite(tenBitAudioMaxDb) && tenBitAudioMaxDb > -60);
const tenBitRaw = spawnSync("ffmpeg", ["-v", "error", "-i", tenBitOutput, "-frames:v", "1",
  "-pix_fmt", "yuv420p10le", "-f", "rawvideo", "pipe:1"], { maxBuffer: 80 * 48 * 4 });
assert.equal(tenBitRaw.status, 0, tenBitRaw.stderr.toString());
let nonEightBitStepLuma = 0;
for (let sample = 0; sample < 80 * 48; sample++)
  if (tenBitRaw.stdout.readUInt16LE(sample * 2) % 4 !== 0) nonEightBitStepLuma++;
assert.ok(nonEightBitStepLuma > 1000, `only ${nonEightBitStepLuma} fine luma samples`);
for (const [route, profile] of [["wgpu", "mp4-h265-main10-aac"],
  ["direct", "mp4-h264-aac"]]) {
  const output = path.join(directory, `${route}-${profile}-10bit-rejected-${process.pid}.mp4`);
  const rejected = spawnSync(executable, ["convert", "--input", tenBitInput,
    "--output", output, "--route", route, "--profile", profile], { encoding: "utf8" });
  assert.notEqual(rejected.status, 0);
  assert.match(rejected.stderr, route === "wgpu" ? /RGBA8 bridge/ : /unsupported pixel format: yuv420p10le/);
  assert.equal(fs.existsSync(output), false);
}
const overwrite = spawnSync(executable, ["convert", "--input", input, "--output", directOutput,
  "--route", "direct"], { encoding: "utf8" });
assert.notEqual(overwrite.status, 0);
assert.match(overwrite.stderr, /refusing to overwrite output/);
const evidence = { fixture: "fixtures/m2-h264-aac.mp4 (CC0-1.0)", adapter: selected,
  direct, gpu, directInspection, gpuInspection, decodedVideoPsnrDb: psnrDb,
  staleAdapterFallback: fallback.adapter_fallback, directVfr: { report: vfr,
    frameTimelineMaxErrorUs, avOffsetErrorUs, audioMaxDb: vfrAudioMaxDb }, gpuVfrRejected: true,
  directGeometry: { report: geometry, corners }, gpuGeometryRejected: true,
  taggedHevcInputs: taggedOutputs, taggedHevcRoutePsnrDb: taggedPsnrDb,
  hdrRejectedOnBothRoutes: true, main10: { report: tenBit, nonEightBitStepLuma,
    audioMaxDb: tenBitAudioMaxDb }, main10GpuRejected: true, tenBitInputToEightBitRejected: true,
  overwriteRejected: true,
  limitations: "Single debug-build run; software FFmpeg encode; GPU route performs explicit upload/readback and blocks per frame." };
fs.writeFileSync(path.join(directory, `evidence-${process.pid}.json`), JSON.stringify(evidence, null, 2));
console.log(JSON.stringify(evidence, null, 2));
