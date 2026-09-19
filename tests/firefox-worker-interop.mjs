// Real-browser acceptance harness. Start isolated Firefox with BiDi port 9226
// and dx serve on 8084 first. No browser profile/settings are changed here.
// node tests/firefox-worker-interop.mjs worker|main [output-directory]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const mode = process.argv[2] ?? "worker";
assert.ok(["worker", "main"].includes(mode));
const outputDirectory = path.resolve(process.argv[3] ?? `tmp/m33-${mode}`);
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
try {
  const session = await send("session.new", { capabilities: { alwaysMatch: {} } });
  ({ context } = await send("browsingContext.create", { type: "tab" }));
  const evaluate = async expression => {
    const result = await send("script.evaluate", { expression, target: { context }, awaitPromise: true });
    if (result.type === "exception") throw Error(JSON.stringify(result));
    return result.result.value;
  };
  const waitFor = async (expression, timeout = 30_000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const value = await evaluate(expression);
      if (value) return value;
      await delay(100);
    }
    throw Error(`Timed out: ${expression}; status=${await evaluate('document.querySelector("#status")?.textContent')}`);
  };
  const status = 'document.querySelector("#status").textContent';
  const terminal = () => waitFor(`!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(${status}) && ${status}`, 45_000);
  const click = selector => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
  const armCancel = prefix => evaluate(`{
    const observer = new MutationObserver(()=>{
      if (document.querySelector("#status").textContent.startsWith(${JSON.stringify(prefix)})) {
        observer.disconnect(); document.querySelector("#cancel").click();
      }
    });
    observer.observe(document.querySelector("#status"), {subtree:true,childList:true,characterData:true});
  }`);
  await send("browsingContext.navigate", {
    context, url: `http://127.0.0.1:8084/${mode === "main" ? "?execution=main" : ""}`, wait: "complete",
  });
  await waitFor('!!document.querySelector("#convert")');
  await click("#convert");
  assert.match(await terminal(), /select an MP4 file first/);
  const element = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
  await send("input.setFiles", { context, element: { sharedId: element.result.sharedId }, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await waitFor(`${status}.startsWith("Ready.")`, 45_000);
  const execution = await evaluate('document.querySelector("#execution-context").textContent');
  assert.match(execution, mode === "worker" ? /dedicated worker/ : /main-thread compatibility fallback/);
  const profiles = JSON.parse(await evaluate('JSON.stringify(Array.from(document.querySelector("#output-profile").options).filter(o=>!o.disabled).map(o=>o.value))'));
  const accelerations = ["no-preference", "prefer-hardware"];
  const evidence = { browser: session.capabilities, mode, execution, ready: await evaluate(status), profiles: [], m1: [] };
  for (const acceleration of accelerations) {
    await evaluate(`{ const select = document.querySelector("#codec-acceleration"); select.value=${JSON.stringify(acceleration)}; select.dispatchEvent(new Event("change", {bubbles:true})); }`);
    for (const profile of profiles) {
    await evaluate(`{ const select = document.querySelector("#output-profile"); select.value=${JSON.stringify(profile)}; select.dispatchEvent(new Event("change", {bubbles:true})); }`);
    await click("#convert");
    await waitFor(`${status}.startsWith("Converting")`);
    await click("#cancel");
    const cancellation = await terminal();
    assert.match(cancellation, /^CANCELLED:/);
    assert.match(cancellation, /Cleanup: 0 application-held frame references, 0 samples/);
    await evaluate(`globalThis.__heartbeat = {ticks:0,maxGap:0,last:performance.now()}; globalThis.__heartbeatTimer = setInterval(()=>{const now=performance.now();__heartbeat.maxGap=Math.max(__heartbeat.maxGap,now-__heartbeat.last);__heartbeat.last=now;__heartbeat.ticks++},50)`);
    await click("#convert");
    const summary = await terminal();
    assert.match(summary, /^PASS: 60 H.264 input frames/);
    assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
    assert.match(summary, new RegExp(`Codec acceleration: requested=${acceleration}, selected=(?:${acceleration}|no-preference)`));
    assert.match(summary, /GPU telemetry: device generation \d+; bounded input texture pool slots=4/);
    const leasePeak = Number(summary.match(/leases live\/peak=0\/(\d+)/)?.[1]);
    assert.ok(leasePeak >= 1 && leasePeak <= 4, `unexpected GPU lease peak ${leasePeak}`);
    assert.match(summary, /ingress copies=60; canvas captures=60/);
    const responsiveness = JSON.parse(await evaluate('clearInterval(__heartbeatTimer); JSON.stringify(__heartbeat)'));
    assert.ok(responsiveness.ticks > 0, "window timers must continue during conversion");
    const output = JSON.parse(await evaluate(`(async()=>{ const a=document.querySelector("#download"); const bytes=new Uint8Array(await (await fetch(a.href)).arrayBuffer()); let binary="";for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));return JSON.stringify({name:a.download,data:btoa(binary)}) })()`));
    fs.writeFileSync(path.join(outputDirectory, `${acceleration}-${profile}-${output.name}`), Buffer.from(output.data, "base64"));
    const playback = JSON.parse(await evaluate(`new Promise((resolve,reject)=>{const v=document.createElement("video");v.src=document.querySelector("#download").href;v.onerror=()=>reject(Error("Playback failed"));v.onloadedmetadata=()=>{v.currentTime=v.duration/2;v.onseeked=()=>{resolve(JSON.stringify({duration:v.duration,width:v.videoWidth,height:v.videoHeight,seek:v.currentTime}));v.removeAttribute("src");v.load()}}})`));
    assert.equal(playback.width, 320);
    assert.equal(playback.height, 180);
    evidence.profiles.push({ profile, acceleration, summary, cancellation, responsiveness, playback });
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
    console.log(`M1 ${mode} ${iteration + 1}/5 PASS`);
  }
  evidence.gpu = await evaluate('document.querySelector("#selected-gpu").textContent');
  fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));
  console.log(`PASS ${mode}: ${profiles.length} profiles × two acceleration preferences, cancel/restart, playback, five M1 rounds; ${evidence.gpu}`);
} finally {
  if (context) await send("browsingContext.close", { context });
  await send("session.end");
  socket.close();
}
