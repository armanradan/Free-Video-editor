// Attach to an already running isolated Firefox WebDriver BiDi session.
// node tests/firefox-m35-interop.mjs [app-port]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const appPort = Number(process.argv[2] ?? 8084);
const outputDirectory = path.resolve("tmp/m35-firefox");
fs.mkdirSync(outputDirectory, { recursive: true });
const socket = new WebSocket("ws://127.0.0.1:9226/session");
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let nextId = 0;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  const waiter = pending.get(message.id);
  if (!waiter) return;
  pending.delete(message.id);
  message.type === "error" ? waiter.reject(Error(JSON.stringify(message))) : waiter.resolve(message.result);
};
const send = (method, params = {}) => new Promise((resolve, reject) => {
  pending.set(++nextId, { resolve, reject });
  socket.send(JSON.stringify({ id: nextId, method, params }));
});
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
let context;
const evidence = { profiles: [], rejected: [], completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));

try {
  const session = await send("session.new", { capabilities: { alwaysMatch: {} } });
  evidence.browser = session.capabilities;
  ({ context } = await send("browsingContext.create", { type: "tab" }));
  const evaluate = async expression => {
    const result = await send("script.evaluate", { expression, target: { context }, awaitPromise: true });
    if (result.type === "exception") throw Error(JSON.stringify(result));
    return result.result.value;
  };
  const waitFor = async (expression, timeout = 60_000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const value = await evaluate(expression);
      if (value) return value;
      await delay(75);
    }
    throw Error(`Timed out: ${expression}; status=${await evaluate('document.querySelector("#status")?.textContent')}`);
  };
  const status = 'document.querySelector("#status").textContent';
  const terminal = () => waitFor(`!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(${status}) && ${status}`);
  const click = selector => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
  await send("browsingContext.navigate", { context, url: `http://127.0.0.1:${appPort}/?verify=full`, wait: "complete" });
  await waitFor('!!document.querySelector("#convert")');
  await evaluate('{const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
  const element = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
  const selectFile = async name => {
    await send("input.setFiles", { context, element: { sharedId: element.result.sharedId }, files: [path.resolve(`fixtures/${name}`)] });
    return waitFor(`/^(Ready\.|FAILED:)/.test(${status}) && ${status}`);
  };
  const enabledProfiles = async () => JSON.parse(await evaluate('JSON.stringify(Array.from(document.querySelector("#output-profile").options).filter(o=>!o.disabled).map(o=>o.value))'));
  const selectProfile = profile => evaluate(`{const s=document.querySelector("#output-profile");s.value=${JSON.stringify(profile)};s.dispatchEvent(new Event("change",{bubbles:true}));}`);
  const playback = async () => JSON.parse(await evaluate(`new Promise((resolve,reject)=>{
    const v=document.createElement("video");v.muted=true;v.src=document.querySelector("#download").href;
    v.onerror=()=>reject(Error("Output playback failed"));v.onloadedmetadata=()=>{v.currentTime=Math.min(0.2,Math.max(0,v.duration/4));};
    v.onseeked=()=>{const c=document.createElement("canvas");c.width=v.videoWidth;c.height=v.videoHeight;const x=c.getContext("2d");x.drawImage(v,0,0);
      const at=(px,py)=>Array.from(x.getImageData(px,py,1,1).data.slice(0,3));resolve(JSON.stringify({duration:v.duration,width:v.videoWidth,height:v.videoHeight,corners:[at(10,10),at(v.videoWidth-11,10),at(10,v.videoHeight-11),at(v.videoWidth-11,v.videoHeight-11)]}));v.removeAttribute("src");v.load();};
  })`));

  assert.match(await selectFile("m35-geometry-color.mp4"), /^Ready\./);
  const geometryProfiles = await enabledProfiles();
  assert.ok(geometryProfiles.length > 0);
  for (const profile of geometryProfiles) {
    await selectProfile(profile);
    await click("#convert");
    const summary = await terminal();
    assert.match(summary, /^PASS: 24 H\.264 input frames/);
    assert.match(summary, /24-frame timestamp\/duration timeline PASS/);
    assert.match(summary, /rotation 90°, flip=true; baked square-pixel output 90×210/);
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

  assert.match(await selectFile("m35-vfr-offset.mp4"), /^Ready\./);
  const timingProfiles = await enabledProfiles();
  assert.deepEqual(timingProfiles, geometryProfiles);
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
  assert.match(await selectFile("m35-resolution-change.mp4"), /^Ready\./);
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
  console.log(`PASS M3.5 Firefox: ${geometryProfiles.length} profiles × geometry and VFR/non-zero-origin; HDR and resolution-change rejection; ${evidence.gpu}`);
} catch (error) {
  evidence.error = error.stack;
  save();
  throw error;
} finally {
  if (context) await send("browsingContext.close", { context }).catch(() => {});
  await send("session.end").catch(() => {});
  socket.close();
}
