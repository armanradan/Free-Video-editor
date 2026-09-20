// Attach to an already running, isolated Chromium debugging session. Never
// launch a browser or inspect existing tabs/profiles. See README for setup.
// node tests/chromium-worker-interop.mjs worker|main|fallback [debug-port] [app-port]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const mode = process.argv[2] ?? "worker";
assert.ok(["worker", "main", "fallback"].includes(mode));
const debugPort = Number(process.argv[3] ?? 9227);
const appPort = Number(process.argv[4] ?? 8084);
const fullVerification = process.env.VERIFY_OUTPUT !== "skip";
const outputDirectory = path.resolve(`tmp/m33-chromium-${mode}`);
fs.mkdirSync(outputDirectory, { recursive: true });
const version = await fetch(`http://127.0.0.1:${debugPort}/json/version`, { signal: AbortSignal.timeout(5000) })
  .then(response => response.json()).catch(error => {
    throw Error(`No accessible Chromium debugging session on port ${debugPort}. Start the isolated browser described in README.`, { cause: error });
  });
const socket = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  const timeout = setTimeout(() => { socket.close(); reject(Error("Browser connection timed out")); }, 5000);
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
socket.onclose = () => {
  for (const waiter of pending.values()) { clearTimeout(waiter.timeout); waiter.reject(Error("Browser disconnected")); }
  pending.clear();
};
const send = (method, params = {}, browser = false) => new Promise((resolve, reject) => {
  const id = ++nextId;
  const timeout = setTimeout(() => { pending.delete(id); reject(Error(`Timed out: ${method}`)); }, 45_000);
  pending.set(id, { resolve, reject, timeout });
  socket.send(JSON.stringify({ id, method, params, ...(!browser && sessionId ? { sessionId } : {}) }));
});
const evaluate = async expression => {
  const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
};
const statusExpression = 'document.querySelector("#status")?.textContent';
const waitFor = async (expression, timeout = 45_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const result = await evaluate(expression);
    if (result) return result;
    await new Promise(resolve => setTimeout(resolve, 20));
  }
  throw Error(`Timed out: ${expression}; status=${await evaluate(statusExpression)}`);
};
const terminal = () => waitFor(`!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(${statusExpression}) && ${statusExpression}`);
const click = selector => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
const evidence = { browser: version, mode, profiles: [], m1: [], completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));

try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  if (mode === "fallback") {
    // Deliberate test-only constructor failure, not a claimed browser limitation.
    await send("Page.addScriptToEvaluateOnNewDocument", { source: `globalThis.Worker = class { constructor() { throw Error("Injected worker startup failure for interoperability test"); } };` });
  }
  const query = new URLSearchParams();
  if (mode === "main") query.set("execution", "main");
  if (fullVerification) query.set("verify", "full");
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?${query}` });
  await send("Page.bringToFront");
  await waitFor('!!document.querySelector("#convert")');
  await click("#convert");
  assert.match(await terminal(), /select an MP4 file first/);
  const document = await send("DOM.getDocument");
  const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await waitFor(`${statusExpression}.startsWith("Ready.")`);
  evidence.ready = await evaluate(statusExpression);
  evidence.execution = await evaluate('document.querySelector("#execution-context").textContent');
  assert.match(evidence.execution, mode === "worker" ? /dedicated worker/ : /main-thread compatibility fallback/);
  if (mode === "fallback") assert.match(evidence.execution, /Injected worker startup failure/);
  const profiles = await evaluate('Array.from(document.querySelector("#output-profile").options).filter(option=>!option.disabled).map(option=>option.value)');
  const accelerations = ["no-preference", "prefer-hardware"];
  evidence.enabledProfiles = profiles;
  save();

  // A DOM observer clicks the real Cancel button as soon as frame progress is
  // rendered, avoiding a fixed sleep that misses short hardware-accelerated jobs.
  const armCancel = prefix => evaluate(`{
    const observer = new MutationObserver(()=>{
      if (document.querySelector("#status").textContent.startsWith(${JSON.stringify(prefix)})) {
        observer.disconnect(); document.querySelector("#cancel").click();
      }
    });
    observer.observe(document.querySelector("#status"), {subtree:true,childList:true,characterData:true});
  }`);
  for (const acceleration of accelerations) {
    await evaluate(`{const select=document.querySelector("#codec-acceleration");select.value=${JSON.stringify(acceleration)};select.dispatchEvent(new Event("change",{bubbles:true}));}`);
    for (const profile of profiles) {
    await evaluate(`{const select=document.querySelector("#output-profile");select.value=${JSON.stringify(profile)};select.dispatchEvent(new Event("change",{bubbles:true}));}`);
    await armCancel("Converting");
    await click("#convert");
    const cancellation = await terminal();
    assert.match(cancellation, /^CANCELLED:/);
    assert.match(cancellation, /Cleanup: 0 application-held frame references, 0 samples/);
    const visibility = await evaluate('document.visibilityState');
    await evaluate(`globalThis.__heartbeat={ticks:0,maxGap:0,last:performance.now()};globalThis.__heartbeatTimer=setInterval(()=>{const now=performance.now();__heartbeat.maxGap=Math.max(__heartbeat.maxGap,now-__heartbeat.last);__heartbeat.last=now;__heartbeat.ticks++},50)`);
    await click("#convert");
    const summary = await terminal();
    assert.match(summary, /^PASS: 60 H.264 input frames/);
    assert.match(summary, fullVerification
      ? /Diagnostic verification: full re-decode PASS/
      : /Diagnostic verification: skipped for normal conversion/);
    assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
    assert.match(summary, new RegExp(`Codec acceleration: requested=${acceleration}, selected=(?:${acceleration}|no-preference)`));
    assert.match(summary, /GPU telemetry: device generation \d+; bounded input texture pool slots=4/);
    const leasePeak = Number(summary.match(/leases live\/peak=0\/(\d+)/)?.[1]);
    assert.ok(leasePeak >= 1 && leasePeak <= 4, `unexpected GPU lease peak ${leasePeak}`);
    assert.match(summary, /ingress copies=60; canvas captures=60/);
    const responsiveness = await evaluate('clearInterval(__heartbeatTimer); __heartbeat');
    assert.ok(responsiveness.ticks > 0, "window timers should run during conversion");
    const output = await evaluate(`(async()=>{
      const a=document.querySelector("#download");const bytes=new Uint8Array(await(await fetch(a.href)).arrayBuffer());
      let binary="";for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));
      return {name:a.download,data:btoa(binary)};
    })()`);
    const outputPath = path.join(outputDirectory, `${acceleration}-${profile}-${output.name}`);
    fs.writeFileSync(outputPath, Buffer.from(output.data, "base64"));
    const playback = await evaluate(`new Promise((resolve,reject)=>{
      const v=document.createElement("video");v.src=document.querySelector("#download").href;
      v.onerror=()=>reject(Error("Output playback failed"));
      v.onloadedmetadata=()=>{v.currentTime=v.duration/2;v.onseeked=()=>{
        resolve({duration:v.duration,width:v.videoWidth,height:v.videoHeight,seek:v.currentTime});v.removeAttribute("src");v.load();
      }};
    })`);
    assert.equal(playback.width, 320);
    assert.equal(playback.height, 180);
    evidence.profiles.push({ profile, acceleration, summary, cancellation, visibility, responsiveness, playback, outputPath });
    save();
    console.log(JSON.stringify(evidence.profiles.at(-1)));
    }
  }
  await armCancel("Processed");
  await click(".regression button");
  evidence.m1Cancellation = await terminal();
  assert.match(evidence.m1Cancellation, /^CANCELLED:/);
  assert.match(evidence.m1Cancellation, /live frames=0/);
  for (let iteration = 0; iteration < 5; iteration++) {
    await click(".regression button");
    const summary = await terminal();
    assert.match(summary, /^PASS: 30\/30 frames/);
    assert.match(summary, /Cleanup: 0 application-owned live frames/);
    evidence.m1.push(summary);
    save();
    console.log(`M1 ${mode} ${iteration + 1}/5 PASS`);
  }
  evidence.gpu = await evaluate('document.querySelector("#selected-gpu").textContent');
  assert.ok(profiles.includes("mp4-h264-aac"), `MP4 validation remains blocked: ${evidence.ready}`);
  evidence.completed = true;
  console.log(`PASS Chromium ${mode}: all three profiles × both acceleration preferences, cancel/restart, playback, five M1 runs; ${evidence.gpu}`);
} catch (error) {
  evidence.error = error.stack;
  throw error;
} finally {
  save();
  if (targetId) await send("Target.closeTarget", { targetId }, true).catch(() => {});
  socket.close();
}
