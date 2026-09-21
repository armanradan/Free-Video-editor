// Attach to an already running isolated Chromium debugging session.
// node tests/chromium-m35-interop.mjs [debug-port] [app-port]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const debugPort = Number(process.argv[2] ?? 9227);
const appPort = Number(process.argv[3] ?? 8084);
const outputDirectory = path.resolve("tmp/m35-chromium");
fs.mkdirSync(outputDirectory, { recursive: true });
const version = await fetch(`http://127.0.0.1:${debugPort}/json/version`, { signal: AbortSignal.timeout(5000) })
  .then(response => response.json()).catch(error => {
    throw Error(`No accessible isolated Chromium debugging session on port ${debugPort}.`, { cause: error });
  });
const socket = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject(Error("Browser connection timed out")), 5000);
  socket.onopen = () => { clearTimeout(timeout); resolve(); };
  socket.onerror = () => { clearTimeout(timeout); reject(Error("Browser connection failed")); };
});

let nextId = 0;
let sessionId;
let targetId;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  const waiter = pending.get(message.id);
  if (!waiter) return;
  pending.delete(message.id);
  clearTimeout(waiter.timeout);
  message.error ? waiter.reject(Error(JSON.stringify(message.error))) : waiter.resolve(message.result);
};
const send = (method, params = {}, browser = false) => new Promise((resolve, reject) => {
  const id = ++nextId;
  const timeout = setTimeout(() => { pending.delete(id); reject(Error(`Timed out: ${method}`)); }, 60_000);
  pending.set(id, { resolve, reject, timeout });
  socket.send(JSON.stringify({ id, method, params, ...(!browser && sessionId ? { sessionId } : {}) }));
});
const evaluate = async expression => {
  const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
};
const statusExpression = 'document.querySelector("#status")?.textContent';
const waitFor = async (expression, timeout = 60_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const result = await evaluate(expression);
    if (result) return result;
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  throw Error(`Timed out: ${expression}; status=${await evaluate(statusExpression)}`);
};
const terminal = () => waitFor(`!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(${statusExpression}) && ${statusExpression}`);
const click = selector => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
const evidence = { browser: version, profiles: [], rejected: [], completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));

try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full` });
  await waitFor('!!document.querySelector("#convert")');
  const document = await send("DOM.getDocument");
  const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
  const selectFile = async name => {
    await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [path.resolve(`fixtures/${name}`)] });
    return waitFor(`/^(Ready\.|FAILED:)/.test(${statusExpression}) && ${statusExpression}`);
  };
  const selectProfile = profile => evaluate(`{const s=document.querySelector("#output-profile");s.value=${JSON.stringify(profile)};s.dispatchEvent(new Event("change",{bubbles:true}));}`);
  const enabledProfiles = () => evaluate('Array.from(document.querySelector("#output-profile").options).filter(option=>!option.disabled).map(option=>option.value)');
  const playback = () => evaluate(`new Promise((resolve,reject)=>{
    const v=document.createElement("video");v.muted=true;v.src=document.querySelector("#download").href;
    v.onerror=()=>reject(Error("Output playback failed"));
    v.onloadedmetadata=()=>{v.currentTime=Math.min(0.2,Math.max(0,v.duration/4));};
    v.onseeked=()=>{const c=document.createElement("canvas");c.width=v.videoWidth;c.height=v.videoHeight;
      const x=c.getContext("2d");x.drawImage(v,0,0);const at=(px,py)=>Array.from(x.getImageData(px,py,1,1).data.slice(0,3));
      resolve({duration:v.duration,width:v.videoWidth,height:v.videoHeight,corners:[at(10,10),at(v.videoWidth-11,10),at(10,v.videoHeight-11),at(v.videoWidth-11,v.videoHeight-11)]});
      v.removeAttribute("src");v.load();};
  })`);

  const geometryReady = await selectFile("m35-geometry-color.mp4");
  assert.match(geometryReady, /^Ready\./);
  const geometryProfiles = await enabledProfiles();
  assert.ok(geometryProfiles.length > 0);
  for (const profile of geometryProfiles) {
    await selectProfile(profile);
    await click("#convert");
    const summary = await terminal();
    assert.match(summary, /^PASS: 24 H\.264 input frames/);
    assert.match(summary, /24-frame timestamp\/duration timeline PASS/);
    assert.match(summary, /input coded 318×180, visible 318×180\+0,0, PAR 421:316, rotation 90°, flip=true; baked square-pixel output 90×210/);
    assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
    const media = await playback();
    assert.deepEqual([media.width, media.height], [90, 210]);
    const [topLeft, topRight, bottomLeft, bottomRight] = media.corners;
    assert.ok(topLeft[0] > 150 && topLeft[0] > topLeft[1] * 1.5 && topLeft[0] > topLeft[2] * 1.5, `top-left should be red: ${topLeft}`);
    assert.ok(topRight[2] > 110 && topRight[2] > topRight[0] * 1.4 && topRight[2] > topRight[1] * 1.4, `top-right should be blue: ${topRight}`);
    assert.ok(bottomLeft[1] > 70 && bottomLeft[1] > bottomLeft[0] * 1.4 && bottomLeft[1] > bottomLeft[2] * 1.4, `bottom-left should be green: ${bottomLeft}`);
    assert.ok(bottomRight[0] > 130 && bottomRight[1] > 90 && bottomRight[2] < 100, `bottom-right should be yellow: ${bottomRight}`);
    evidence.profiles.push({ fixture: "geometry", profile, summary, media });
    save();
  }

  const timingReady = await selectFile("m35-vfr-offset.mp4");
  assert.match(timingReady, /^Ready\./);
  const timingProfiles = await enabledProfiles();
  assert.deepEqual(timingProfiles, geometryProfiles, "both accepted SDR fixtures should expose the same profiles");
  for (const profile of timingProfiles) {
    await selectProfile(profile);
    await click("#convert");
    const summary = await terminal();
    assert.match(summary, /^PASS: 36 H\.264 input frames/);
    assert.match(summary, /36-frame timestamp\/duration timeline PASS/);
    assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
    const media = await playback();
    assert.deepEqual([media.width, media.height], [160, 90]);
    evidence.profiles.push({ fixture: "timing", profile, summary, media });
    save();
  }

  const hdr = await selectFile("m35-hdr-tagged.mp4");
  assert.match(hdr, /^FAILED: HDR input is not supported by the M3\.5 SDR pipeline/);
  evidence.rejected.push({ fixture: "hdr", status: hdr });

  const resolutionReady = await selectFile("m35-resolution-change.mp4");
  assert.match(resolutionReady, /^Ready\./);
  await selectProfile((await enabledProfiles())[0]);
  await click("#convert");
  const resolution = await terminal();
  assert.match(resolution, /^FAILED: Video decode\/process\/encode pipeline failed: Mid-stream geometry change is unsupported/);
  assert.match(resolution, /Cleanup: 0 application-held frame references, 0 samples/);
  evidence.rejected.push({ fixture: "resolution-change", status: resolution });

  evidence.gpu = await evaluate('document.querySelector("#selected-gpu").textContent');
  evidence.execution = await evaluate('document.querySelector("#execution-context").textContent');
  evidence.enabledProfiles = geometryProfiles;
  evidence.completed = true;
  save();
  console.log(`PASS M3.5 Chromium: ${geometryProfiles.length} profiles × geometry and VFR/non-zero-origin; HDR and resolution-change rejection; ${evidence.gpu}`);
} catch (error) {
  evidence.error = error.stack;
  save();
  throw error;
} finally {
  if (targetId) await send("Target.closeTarget", { targetId }, true).catch(() => {});
  socket.close();
}
