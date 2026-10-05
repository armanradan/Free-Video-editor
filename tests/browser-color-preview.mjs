// Real-browser gate: node tests/browser-color-preview.mjs chromium|firefox [app-port].
// Uses isolated Chromium CDP :9227 / Firefox BiDi :9226, not normal browser profiles.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
const browser = process.argv[2] ?? "chromium";
const appPort = Number(process.argv[3] ?? 8084);
const directory = path.resolve(`tmp/color-preview/${browser}-${process.pid}`);
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
  const preview = 'document.querySelector("#source-preview-status")?.textContent';
  const counts = value => [...value.matchAll(/(?:cache loads|renders)=(\d+)/g)].map(match => Number(match[1]));
  const update = async (id, value) => {
    const previous = await evaluate(preview);
    await set(id, value, "input");
    return wait(`${preview} !== ${JSON.stringify(previous)} && /cached frames=1/.test(${preview}) && ${preview}`);
  };
  const pixels = output => {
    const result = spawnSync("ffmpeg", ["-v","error","-i",output,"-frames:v","1","-f","rawvideo","-pix_fmt","rgb24","-"], {timeout:30000,maxBuffer:32*1024*1024});
    assert.equal(result.status,0,String(result.stderr));
    return result.stdout;
  };
  const gray = bytes => {
    assert.ok(bytes.length > 0);
    let darkest=255, brightest=0;
    for(let i=0;i<bytes.length;i+=3) assert.ok(Math.max(...bytes.subarray(i,i+3))-Math.min(...bytes.subarray(i,i+3))<=4,`not gray at ${i}: ${[...bytes.subarray(i,i+3)]}`);
    for(let i=0;i<bytes.length;i+=3) { darkest=Math.min(darkest,bytes[i]);brightest=Math.max(brightest,bytes[i]); }
    assert.ok(brightest-darkest > 20,"blank preview/output is not a color test");
  };
  const screenshot = async name => {
    const image = await send(chromium ? "Page.captureScreenshot" : "browsingContext.captureScreenshot", chromium ? {format:"png",captureBeyondViewport:true} : {context,origin:"document",format:{type:"image/png"}});
    const file=path.join(directory,name+".png");fs.writeFileSync(file,Buffer.from(image.data,"base64"));return file;
  };
  for (const main of [false,true]) {
    await navigate("webcodecs",main);
    assert.equal(await evaluate('document.querySelector("#color-contrast").value'),"100");
    await selectFile("fixtures/m2-h264-aac.mp4"); await ready();
    const initial=await wait(`/cached frames=1/.test(${preview}) && ${preview}`);
    const desaturated=await update("#color-saturation","0");
    assert.equal(counts(initial)[0],counts(desaturated)[0],"slider decoded source again");
    await update("#color-brightness","10");await update("#color-contrast","150");
    const rect=await evaluate('(()=>{const c=document.querySelector("#worker-preview canvas")??document.querySelector("#export-canvas");const r=c.getBoundingClientRect();return {x:r.x+scrollX,y:r.y+scrollY,w:r.width,h:r.height,viewport:innerWidth};})()');
    const png=await screenshot(main?"main-gray-preview":"worker-gray-preview");
    evidence.previewStyles=await evaluate('Array.from(document.querySelectorAll(".preview-panel,#worker-preview,#worker-preview canvas,#export-canvas")).map(n=>({node:n.id||n.className,style:n.getAttribute("style"),visibility:getComputedStyle(n).visibility,display:getComputedStyle(n).display}))');save();
    const width=Number(run("ffprobe",["-v","error","-select_streams","v:0","-show_entries","stream=width","-of","csv=p=0",png]).trim());
    const scale=width/rect.viewport;
    const crop=spawnSync("ffmpeg",["-v","error","-i",png,"-vf",`crop=${Math.floor(rect.w*scale*.8)}:${Math.floor(rect.h*scale*.8)}:${Math.floor((rect.x+rect.w*.1)*scale)}:${Math.floor((rect.y+rect.h*.1)*scale)}`,"-f","rawvideo","-pix_fmt","rgb24","-"],{timeout:30000,maxBuffer:32*1024*1024});
    assert.equal(crop.status,0,String(crop.stderr));gray(crop.stdout);
    evidence.cases.push({name:main?"main-preview":"worker-preview",initial,desaturated,rect,screenshot:png});
    await evaluate('(()=>{document.querySelector("#color-compare").click();return true;})()');await delay(300);
    assert.equal(await evaluate('document.querySelector("#color-saturation").value'),"0","Before changed saved export settings");
    await set("#output-profile","webm-vp8-video-only");await ready();
    const result=await convert(main?"main-adjusted":"worker-adjusted",60,"original","webm-vp8-video-only");
    gray(pixels(result.output));
    if(!main) {
      const profiles=await evaluate('Array.from(document.querySelector("#output-profile").options).filter(n=>!n.disabled&&n.value!=="webm-vp8-video-only").map(n=>n.value)');
      for(const profile of profiles) {
        await set("#output-profile",profile);await set("#output-fps","15");await ready();
        const audio=await convert(`adjusted-audio-${profile}`,30,"15",profile);gray(pixels(audio.output));
      }
      await set("#output-fps","original");await ready();
      await wait(`/cached frames=1/.test(${preview}) && ${preview}`);
      const prior=await evaluate(preview);
      await evaluate('(()=>{const n=document.querySelector("#color-brightness");for(let i=0;i<60;i++){n.value=String(i%20);n.dispatchEvent(new Event("input",{bubbles:true}));}return true;})()');
      await wait(`${preview} !== ${JSON.stringify(prior)} && /cached frames=1/.test(${preview})`);
      await delay(400);
      const latest=await evaluate(preview);
      assert.equal(counts(prior)[0],counts(latest)[0]);
      assert.ok(counts(latest)[1]-counts(prior)[1]<60,"unbounded task per slider event");
      evidence.cases.push({name:"rapid-slider-coalescing",prior,latest});
      await update("#source-preview-position","0.75");
    }
    await evaluate('(()=>{document.querySelector("#color-reset").click();return true;})()');await delay(300);
    assert.deepEqual(await evaluate('["brightness","contrast","saturation"].map(id=>document.querySelector("#color-"+id).value)'),["0","100","100"]);
  }
  await navigate("ffmpeg-wasm");await selectFile("fixtures/m2-h264-aac.mp4");await ready();
  await wait(`/cached frames=1/.test(${preview})`);
  await update("#color-saturation","0");await update("#color-brightness","10");await update("#color-contrast","150");
  const ffmpeg=await convert("ffmpeg-adjusted",60,"original","mp4-h264-aac");gray(pixels(ffmpeg.output));
  evidence.status="passed";
} catch (error) { evidence.status = "failed"; evidence.error = String(error.stack ?? error); throw error; }
finally {
  save(); console.log(`Evidence: ${path.join(directory, "evidence.json")}`);
  if (chromium && targetId) await send("Target.closeTarget", { targetId }, true).catch(()=>{});
  if (!chromium && context) await send("browsingContext.close", { context }).catch(()=>{});
  if (!chromium && context) await send("session.end").catch(()=>{});
  socket.close();
}
