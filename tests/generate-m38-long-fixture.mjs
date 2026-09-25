// Deterministic CC0 synthetic input: tiny compressed file, >2 GiB logical RGBA.
// Generated into ignored tmp/ so no large test artifact enters the repository.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const output = path.resolve("tmp/m38-long/synthetic-2400f-h264-aac.mp4");
fs.mkdirSync(path.dirname(output), { recursive: true });
if (!fs.existsSync(output)) {
  const args = ["-hide_banner", "-loglevel", "error", "-y",
    "-f", "lavfi", "-i", "color=c=red:s=640x360:r=30:d=80",
    "-f", "lavfi", "-i", "sine=frequency=880:sample_rate=48000:duration=80",
    "-map", "0:v:0", "-map", "1:a:0", "-frames:v", "2400",
    "-c:v", "libx264", "-preset", "ultrafast", "-crf", "30", "-pix_fmt", "yuv420p",
    "-g", "30", "-c:a", "aac", "-b:a", "96k", "-movflags", "+faststart", output];
  const generated = spawnSync("ffmpeg", args, { stdio: "inherit" });
  assert.equal(generated.status, 0, generated.error?.message);
}
const probe = spawnSync("ffprobe", ["-v", "error", "-select_streams", "v:0",
  "-show_entries", "stream=nb_frames,width,height,r_frame_rate", "-of", "json", output],
{ encoding: "utf8" });
assert.equal(probe.status, 0, probe.stderr);
const video = JSON.parse(probe.stdout).streams[0];
assert.equal(Number(video.nb_frames), 2400);
assert.equal(video.width, 640);
assert.equal(video.height, 360);
assert.equal(video.r_frame_rate, "30/1");
const rawBytes = 2400 * 640 * 360 * 4;
assert.ok(rawBytes > 2 * 1024 ** 3);
console.log(JSON.stringify({ output, compressedBytes: fs.statSync(output).size, rawBytes,
  license: "CC0-1.0; FFmpeg lavfi color and sine, no third-party media" }, null, 2));
