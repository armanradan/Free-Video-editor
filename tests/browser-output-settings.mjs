// Real-browser gate: node tests/browser-output-settings.mjs chromium|firefox [app-port].
// Uses isolated Chromium CDP :9227 / Firefox BiDi :9226, not normal browser profiles.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
const browser = process.argv[2] ?? "chromium";
const appPort = Number(process.argv[3] ?? 8084);
const directory = path.resolve(`tmp/browser-output-settings/${browser}-${process.pid}`);
fs.mkdirSync(directory, { recursive: true });
const evidence = { browser, cases: [], status: "running" };
const run = (program, args) => {
  const result = spawnSync(program, args, { encoding: "utf8", timeout: 120_000, maxBuffer: 16 * 1024 * 1024 });
  assert.equal(result.status, 0, `${program}: ${result.stderr}`);
  return result.stdout;
};
const chromium = browser === "chromium";
const version = chromium ? await fetch("http://127.0.0.1:9227/json/version").then(response => response.json()) : null;
const socket = new WebSocket(chromium ? version.webSocketDebuggerUrl : "ws://127.0.0.1:9226/session");
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let sequence = 0, sessionId, targetId, context;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const value = JSON.parse(data), waiter = pending.get(value.id);
  if (!waiter) return;
  pending.delete(value.id); clearTimeout(waiter.timeout);
  value.error || value.type === "error" ? waiter.reject(Error(JSON.stringify(value))) : waiter.resolve(value.result);
};
const send = (method, params = {}, root = false) => new Promise((resolve, reject) => {
  const id = ++sequence;
  const timeout = setTimeout(() => { pending.delete(id); reject(Error(`Timeout: ${method}`)); }, 120_000);
  pending.set(id, { resolve, reject, timeout });
  socket.send(JSON.stringify({ id, method, params, ...(chromium && sessionId && !root ? { sessionId } : {}) }));
});
const evaluate = async expression => {
  const code = `(async()=>JSON.stringify(await (${expression})))()`;
  const result = await send(chromium ? "Runtime.evaluate" : "script.evaluate", chromium
    ? { expression: code, awaitPromise: true, returnByValue: true }
    : { expression: code, target: { context }, awaitPromise: true });
  if (result.exceptionDetails || result.type === "exception") throw Error(JSON.stringify(result));
  const value = result.result.value;
  return value === undefined ? undefined : JSON.parse(value);
};
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const wait = async (expression, timeout = 120_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = await evaluate(expression); if (value) return value; await delay(50); }
  throw Error(`Timeout: ${expression}; ${await evaluate('document.querySelector("#status")?.textContent')}`);
};
const status = 'document.querySelector("#status")?.textContent';
const ready = () => wait(`!document.querySelector("#convert").disabled`);
let currentSource;
function expectedCount(fps, profile) {
  const info = JSON.parse(run("ffprobe", ["-v", "error", "-show_streams", "-of", "json", currentSource]));
  const video = info.streams.find(value => value.codec_type === "video");
  if (fps === "original") return Number(video.nb_frames);
  const [num, den = 1] = fps.split("/").map(Number);
  const [tickNum, tickDen] = video.time_base.split("/").map(Number);
  const rate = num / den, start = Number(video.start_pts) * tickNum / tickDen;
  let origin = start;
  if (!profile.includes("video-only")) {
    const packets = JSON.parse(run("ffprobe", ["-v", "error", "-select_streams", "a:0", "-show_packets", "-show_entries", "packet=pts_time", "-of", "json", currentSource]));
    // Mediabunny exposes AAC priming packets; FFprobe stream start omits them.
    origin = Math.min(start, Number(packets.packets[0].pts_time));
  }
  const duration = Number(video.duration_ts) * tickNum / tickDen;
  return Math.ceil((start + duration - origin) * rate - 1e-6) - Math.round((start - origin) * rate);
}
const set = async (selector, value, event = "change") => {
  await evaluate(`(()=>{const node=document.querySelector(${JSON.stringify(selector)});node.value=${JSON.stringify(value)};node.dispatchEvent(new Event(${JSON.stringify(event)},{bubbles:true}));return true;})()`);
  await delay(150);
};
const selectFile = async file => {
  currentSource = path.resolve(file);
  if (chromium) {
    const document = await send("DOM.getDocument");
    const element = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
    await send("DOM.setFileInputFiles", { nodeId: element.nodeId, files: [path.resolve(file)] });
  } else {
    const element = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
    await send("input.setFiles", { context, element: { sharedId: element.result.sharedId }, files: [path.resolve(file)] });
  }
  await evaluate('(()=>{document.querySelector("#source-file").dispatchEvent(new Event("change",{bubbles:true}));return true;})()');
  await delay(150);
};
const navigate = async (backend, main = false) => {
  const url = `http://127.0.0.1:${appPort}/?verify=full${backend === "ffmpeg-wasm" ? "&backend=ffmpeg-wasm" : ""}${main ? "&execution=main" : ""}`;
  await send(chromium ? "Page.navigate" : "browsingContext.navigate", chromium ? { url } : { context, url, wait: "complete" });
  await wait('!!document.querySelector("#output-fps")');
};
let outputIndex = 0;
async function convert(name, expectedFrames, fps, profile) {
  expectedFrames = expectedCount(fps, profile);
  await ready();
  await evaluate('(()=>{document.querySelector("#convert").click();return true;})()');
  await delay(100);
  const summary = await wait(`!document.querySelector("#cancel").disabled || /^(PASS|FAILED|CANCELLED):/.test(${status})`)
    .then(() => wait(`document.querySelector("#cancel").disabled && /^(PASS|FAILED|CANCELLED):/.test(${status}) && ${status}`));
  assert.match(summary, /^PASS:/);
  assert.match(summary, /Diagnostic verification: full re-decode PASS/);
  assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
  assert.match(summary, new RegExp(`→ ${expectedFrames} output frames`));
  const bytes = await evaluate('(async()=>{const bytes=new Uint8Array(await (await fetch(document.querySelector("#download").href)).arrayBuffer());let binary="";for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(binary);})()');
  const output = path.join(directory, `${++outputIndex}-${name}.${profile.startsWith("mp4") ? "mp4" : "webm"}`);
  fs.writeFileSync(output, Buffer.from(bytes, "base64"));
  const info = JSON.parse(run("ffprobe", ["-v", "error", "-show_streams", "-show_format", "-of", "json", output]));
  const video = info.streams.find(value => value.codec_type === "video");
  const frameInfo = JSON.parse(run("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_frames", "-show_entries", "frame=best_effort_timestamp_time", "-of", "json", output]));
  assert.equal(frameInfo.frames.length, expectedFrames);
  if (fps !== "original") {
    const [num, den = 1] = fps.split("/").map(Number);
    const first = Number(frameInfo.frames[0].best_effort_timestamp_time);
    const index = Math.round(first * num / den);
    frameInfo.frames.forEach((frame, i) => assert.ok(Math.abs(Number(frame.best_effort_timestamp_time) - (index + i) * den / num) <= .0011));
  }
  if (!profile.includes("video-only")) assert.ok(info.streams.some(value => value.codec_type === "audio"));
  run("ffmpeg", ["-v", "error", "-i", output, "-fps_mode", "passthrough", "-enc_time_base:v", "demux", "-f", "null", "-"]);
  const result = { name, profile, fps, expectedFrames, output, bytes: Number(info.format.size), video, summary };
  evidence.cases.push(result); save(); console.log(`${browser} ${name}: ${expectedFrames} frames PASS`);
  return result;
}
const save = () => fs.writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
try {
  if (chromium) {
    evidence.version = version;
    ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
    ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
    await send("Page.enable"); await send("Runtime.enable");
  } else {
    evidence.version = (await send("session.new", { capabilities: { alwaysMatch: {} } })).capabilities;
    ({ context } = await send("browsingContext.create", { type: "tab" }));
  }
  await navigate("webcodecs");
  assert.equal(await evaluate('document.querySelector("#output-fps").value'), "original");
  assert.equal(await evaluate('document.querySelector("#video-bitrate").value'), "recommended");
  await selectFile("fixtures/m2-h264-aac.mp4"); await ready();
  assert.match(await evaluate('document.querySelector("#source-metadata").textContent'), /30\.000 fps \(CFR\)/);
  assert.match(await evaluate('document.querySelector("#output-estimate").textContent'), /Estimated/);
  const profiles = await evaluate('Array.from(document.querySelector("#output-profile").options).filter(option=>!option.disabled).map(option=>option.value)');
  evidence.profiles = profiles;
  for (const profile of profiles) {
    await set("#output-profile", profile);
    for (const [fps, count] of [["original", 60], ["15", 30], ["60", 120], ["30000/1001", 60]]) {
      await set("#output-fps", fps); await ready();
      await convert(`cfr-${profile}-${fps.replace("/", "_")}`, count, fps, profile);
    }
  }
  // High-detail CC0 source makes bitrate changes observable, not merely config echoes.
  const textured = path.join(directory, "textured.mp4");
  run("ffmpeg", ["-v", "error", "-n", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=30,noise=alls=10:allf=t:all_seed=42", "-f", "lavfi", "-i", "sine=frequency=523:sample_rate=48000", "-t", "4", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "14", "-pix_fmt", "yuv420p", "-c:a", "aac", textured]);
  await set("#output-fps", "original"); await selectFile(textured); await ready();
  const labels = () => evaluate('Array.from(document.querySelector("#video-bitrate").options).map(option=>option.textContent)');
  const fullLabels = await labels();
  await set("#resize-preset", "percent-50"); await ready();
  assert.notDeepEqual(await labels(), fullLabels);
  await set("#resize-preset", "original"); await ready();
  await set("#output-fps", "15"); await ready();
  assert.notDeepEqual(await labels(), fullLabels);
  await set("#output-fps", "original"); await ready();
  await set("#video-bitrate", "custom"); await set("#custom-bitrate", "0", "input");
  await wait('document.querySelector("#convert").disabled && document.querySelector("#output-estimate").textContent.includes("0.25")');
  await set("#custom-bitrate", "1", "input"); await ready();
  await set("#output-profile", profiles[0]);
  const low = await convert("custom-1Mbps", 120, "original", profiles[0]);
  await set("#custom-bitrate", "4", "input"); await ready();
  const high = await convert("custom-4Mbps", 120, "original", profiles[0]);
  assert.match(low.summary, /target video bitrate=1000000 bps/);
  assert.match(high.summary, /target video bitrate=4000000 bps/);
  assert.ok(high.bytes > low.bytes * 1.2, `${high.bytes} vs ${low.bytes}`);
  await set("#custom-bitrate", "3.5", "input"); await ready();
  assert.match(await evaluate('document.querySelector("#output-estimate").textContent'), /3\.50 Mbps/);
  const capture = await send(chromium ? "Page.captureScreenshot" : "browsingContext.captureScreenshot",
    chromium ? { format: "png", captureBeyondViewport: true } : { context, origin: "document" });
  fs.writeFileSync(path.join(directory, "settings.png"), Buffer.from(capture.data, "base64"));
  await set("#video-bitrate", "recommended"); await selectFile("fixtures/m35-vfr-offset.mp4"); await ready();
  assert.match(await evaluate('document.querySelector("#source-metadata").textContent'), /VFR average/);
  await convert("vfr-original", 36, "original", profiles[0]);
  await set("#output-fps", "60"); await ready();
  await convert("vfr-60", 119, "60", profiles[0]);
  // Cancellation while lookahead is live, followed by a same-context retry.
  await selectFile(textured); await ready();
  await evaluate('(()=>{document.querySelector("#convert").click();return true;})()');
  await wait(`${status}.startsWith("Converting")`);
  await evaluate('(()=>{document.querySelector("#cancel").click();return true;})()');
  const cancelled = await wait(`document.querySelector("#cancel").disabled && ${status}.startsWith("CANCELLED:") && ${status}`);
  assert.match(cancelled, /Cleanup: 0 application-held frame references, 0 samples/);
  evidence.cancelled = cancelled;
  await convert("fixed-fps-retry", 240, "60", profiles[0]);
  for (const backend of ["ffmpeg-wasm"]) {
    await navigate(backend); await selectFile("fixtures/m2-h264-aac.mp4"); await ready();
    for (const [fps, count] of [["original", 60], ["15", 30], ["30000/1001", 60]]) {
      await set("#output-fps", fps); await ready();
      await convert(`ffmpeg-${fps.replace("/", "_")}`, count, fps, "mp4-h264-aac");
    }
    await set("#output-fps", "15"); await selectFile("fixtures/m35-vfr-offset.mp4"); await ready();
    await convert("ffmpeg-vfr-15", 30, "15", "mp4-h264-aac");
    await set("#output-fps", "original"); await selectFile(textured); await ready();
    await set("#video-bitrate", "custom"); await set("#custom-bitrate", "1", "input"); await ready();
    const low = await convert("ffmpeg-custom-1Mbps", 120, "original", "mp4-h264-aac");
    await set("#custom-bitrate", "4", "input"); await ready();
    const high = await convert("ffmpeg-custom-4Mbps", 120, "original", "mp4-h264-aac");
    assert.ok(high.bytes > low.bytes * 1.2, `${high.bytes} vs ${low.bytes}`);
  }
  const short = path.join(directory, "two-frames.mp4");
  // Re-encode presentation-order frames: copying two B-frame packets is not a two-frame timeline.
  run("ffmpeg", ["-v", "error", "-n", "-i", "fixtures/m2-h264-aac.mp4", "-frames:v", "2", "-an", "-c:v", "libx264", "-bf", "0", short]);
  await navigate("webcodecs"); await selectFile(short); await ready();
  await set("#output-profile", "webm-vp8-video-only");
  for (const fps of ["30", "30000/1001"]) {
    await set("#output-fps", fps); await ready();
    await convert(`short-${fps.replace("/", "_")}`, 2, fps, "webm-vp8-video-only");
  }
  await navigate("webcodecs", true); await selectFile("fixtures/m2-h264-aac.mp4"); await ready();
  await set("#output-fps", "60"); await ready();
  await convert("main-thread-60", 121, "60", "webm-vp8-opus");
  assert.match(await evaluate('document.body.textContent'), /main-thread compatibility fallback/);
  evidence.gpu = await evaluate('document.querySelector("#selected-gpu").textContent');
  evidence.status = "passed";
} catch (error) { evidence.status = "failed"; evidence.error = String(error.stack ?? error); throw error; }
finally {
  save(); console.log(`Evidence: ${path.join(directory, "evidence.json")}`);
  if (chromium && targetId) await send("Target.closeTarget", { targetId }, true).catch(()=>{});
  if (!chromium && context) await send("browsingContext.close", { context }).catch(()=>{});
  if (!chromium && context) await send("session.end").catch(()=>{});
  socket.close();
}
