// M3.4 long-run stability check against an already-running isolated Chromium
// debugging session and dx serve on port 8084.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const debugPort = Number(process.argv[2] ?? 9227);
const appPort = Number(process.argv[3] ?? 8084);
const inputPath = path.resolve(process.argv[4] ?? "tmp/user-test/Input.mp4");
const outputDirectory = path.resolve("tmp/m34-long");
fs.mkdirSync(outputDirectory, { recursive: true });
assert.ok(fs.statSync(inputPath).size > 50 * 1024 * 1024, "Long-run input should exercise materially more than the tiny fixture");

const version = await fetch(`http://127.0.0.1:${debugPort}/json/version`, { signal: AbortSignal.timeout(5000) }).then(response => response.json());
const socket = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let nextId = 0;
let sessionId;
let targetId;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  const waiter = pending.get(message.id);
  if (!waiter) return;
  pending.delete(message.id);
  clearTimeout(waiter.timer);
  message.error ? waiter.reject(Error(JSON.stringify(message.error))) : waiter.resolve(message.result);
};
const send = (method, params = {}, browser = false, timeout = 180_000) => new Promise((resolve, reject) => {
  const id = ++nextId;
  const timer = setTimeout(() => { pending.delete(id); reject(Error(`Timed out: ${method}`)); }, timeout);
  pending.set(id, { resolve, reject, timer });
  socket.send(JSON.stringify({ id, method, params, ...(!browser && sessionId ? { sessionId } : {}) }));
});
const evaluate = async expression => {
  const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
};
const waitFor = async (expression, timeout = 180_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await evaluate(expression);
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 250));
  }
  throw Error(`Timed out; status=${await evaluate('document.querySelector("#status")?.textContent')}`);
};
const evidence = { browser: version, inputPath, inputBytes: fs.statSync(inputPath).size, completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));
try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full` });
  await send("Page.bringToFront");
  await waitFor('!!document.querySelector("#source-file")');
  const document = await send("DOM.getDocument");
  const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [inputPath] });
  await waitFor('document.querySelector("#status").textContent.startsWith("Ready.")');
  await evaluate(`{
    const profile=document.querySelector("#output-profile");profile.value="webm-vp8-opus";profile.dispatchEvent(new Event("change",{bubbles:true}));
    const acceleration=document.querySelector("#codec-acceleration");acceleration.value="no-preference";acceleration.dispatchEvent(new Event("change",{bubbles:true}));
    globalThis.__longHeartbeat={ticks:0,maxGap:0,last:performance.now()};
    globalThis.__longHeartbeatTimer=setInterval(()=>{const now=performance.now();__longHeartbeat.maxGap=Math.max(__longHeartbeat.maxGap,now-__longHeartbeat.last);__longHeartbeat.last=now;__longHeartbeat.ticks++},50);
    document.querySelector("#convert").click();
  }`);
  evidence.summary = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(document.querySelector("#status").textContent) && document.querySelector("#status").textContent');
  evidence.responsiveness = await evaluate('clearInterval(__longHeartbeatTimer); __longHeartbeat');
  evidence.execution = await evaluate('document.querySelector("#execution-context").textContent');
  evidence.gpu = await evaluate('document.querySelector("#selected-gpu").textContent');
  assert.match(evidence.summary, /^PASS:/);
  assert.match(evidence.summary, /Diagnostic verification: full re-decode PASS/);
  assert.match(evidence.summary, /Codec acceleration: requested=no-preference, selected=no-preference/);
  assert.match(evidence.summary, /Cleanup: 0 application-held frame references, 0 samples/);
  const leasePeak = Number(evidence.summary.match(/leases live\/peak=0\/(\d+)/)?.[1]);
  assert.ok(leasePeak >= 1 && leasePeak <= 4, `unexpected GPU lease peak ${leasePeak}`);
  const frames = Number(evidence.summary.match(/PASS: (\d+) H\.264/)?.[1]);
  assert.ok(frames > 1000, `Expected a long run, got ${frames} frames`);
  const stageFrames = Number(evidence.summary.match(/decoded-video 0\/1\/(\d+)/)?.[1]);
  const ingressCopies = Number(evidence.summary.match(/ingress copies=(\d+)/)?.[1]);
  assert.equal(stageFrames, frames);
  assert.equal(ingressCopies, frames);
  assert.ok(evidence.responsiveness.ticks > 0);
  const output = await evaluate(`(async()=>{
    const a=document.querySelector("#download");const buffer=await(await fetch(a.href)).arrayBuffer();
    return {name:a.download,bytes:buffer.byteLength,duration:Number(document.querySelector("#status").textContent.match(/Output: \\d+ bytes, \\d+ frames, ([0-9.]+) seconds/)?.[1])};
  })()`);
  evidence.output = output;
  evidence.completed = true;
  console.log(evidence.summary);
  console.log(JSON.stringify({ frames, output, responsiveness: evidence.responsiveness, execution: evidence.execution, gpu: evidence.gpu }, null, 2));
} catch (error) {
  evidence.error = error.stack;
  throw error;
} finally {
  save();
  if (targetId) await send("Target.closeTarget", { targetId }, true).catch(() => {});
  socket.close();
}
