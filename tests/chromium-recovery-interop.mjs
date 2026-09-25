// Inject codec and WebGPU device-loss failures, then verify cleanup and same-page restart.
// node tests/chromium-recovery-interop.mjs [debug-port] [app-port]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const debugPort = Number(process.argv[2] ?? 9227);
const appPort = Number(process.argv[3] ?? 8084);
const ffmpegBackend = process.argv[4] === "ffmpeg-wasm";
const outputDirectory = path.resolve(ffmpegBackend ? "tmp/m38-recovery-chromium" : "tmp/m36-recovery-chromium");
fs.mkdirSync(outputDirectory, { recursive: true });
const version = await fetch(`http://127.0.0.1:${debugPort}/json/version`, { signal: AbortSignal.timeout(5000) }).then(response => response.json());
const socket = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let nextId = 0;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  const waiter = pending.get(message.id);
  if (!waiter) return;
  pending.delete(message.id);
  clearTimeout(waiter.timeout);
  message.error ? waiter.reject(Error(JSON.stringify(message.error))) : waiter.resolve(message.result);
};
const send = (method, params = {}, sessionId) => new Promise((resolve, reject) => {
  const id = ++nextId;
  const timeout = setTimeout(() => { pending.delete(id); reject(Error(`Timed out: ${method}`)); }, 60_000);
  pending.set(id, { resolve, reject, timeout });
  socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
});
const evidence = { browser: version, modes: [], completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));

async function runScenario(mode, failureMode) {
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  const evaluate = async expression => {
    const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }, sessionId);
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  };
  const status = 'document.querySelector("#status")?.textContent';
  const waitFor = async (expression, timeout = 60_000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const value = await evaluate(expression);
      if (value) return value;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw Error(`Timed out: ${expression}; status=${await evaluate(status)}`);
  };
  const terminal = () => waitFor(`!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(${status}) && ${status}`);
  try {
    await send("Page.enable", {}, sessionId);
    await send("Runtime.enable", {}, sessionId);
    const main = mode === "main" ? "&execution=main" : "";
    await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full&failure=${failureMode}${main}${ffmpegBackend ? "&backend=ffmpeg-wasm" : ""}` }, sessionId);
    await waitFor('!!document.querySelector("#source-file")');
    const document = await send("DOM.getDocument", {}, sessionId);
    const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" }, sessionId);
    await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [path.resolve("fixtures/m2-h264-aac.mp4")] }, sessionId);
    await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
    await waitFor(`${status}.includes("Output resolves to 320×180")`);

    await evaluate('document.querySelector("#convert").click()');
    const failure = await terminal();
    const expectedFailure = failureMode === "codec-once"
      ? /^FAILED: Video decode\/process\/encode pipeline failed: INJECTED: codec failure after 5 GPU-processed frames/
      : /^FAILED: Video decode\/process\/encode pipeline failed: INJECTED: WebGPU device loss after 4 completed frames/;
    assert.match(failure, expectedFailure);
    assert.match(failure, /decoded-video 0\//);
    assert.match(failure, /GPU 0\//);
    assert.match(failure, /video encoder callbacks 0\//);
    assert.match(failure, /decoded-audio 0\//);
    assert.match(failure, /Cleanup: 0 application-held frame references, 0 samples/);
    assert.equal(await evaluate('document.querySelector("#download") === null'), true);

    await evaluate('document.querySelector("#convert").click()');
    const restart = await terminal();
    assert.match(restart, /^PASS: 60 H\.264 input frames/);
    assert.match(restart, /Diagnostic verification: full re-decode PASS/);
    if (ffmpegBackend) assert.match(restart, /no raw OPFS spool/);
    assert.match(restart, /Cleanup: 0 application-held frame references, 0 samples/);
    assert.match(restart, new RegExp(`Execution: ${mode === "worker" ? "dedicated worker" : "main-thread fallback"}`));
    assert.match(restart, /reused for this command=true/);
    assert.match(restart, new RegExp(`GPU telemetry: device generation ${failureMode === "device-loss-once" ? 2 : 1};`));
    const playback = await evaluate(`new Promise((resolve,reject)=>{const v=document.createElement("video");v.src=document.querySelector("#download").href;v.onerror=()=>reject(Error("playback failed"));v.onloadedmetadata=()=>resolve({width:v.videoWidth,height:v.videoHeight,duration:v.duration});})`);
    assert.deepEqual([playback.width, playback.height], [320, 180]);
    return { mode, failureMode, failure, restart, playback, execution: await evaluate('document.querySelector("#execution-context").textContent') };
  } finally {
    await send("Target.closeTarget", { targetId }).catch(() => {});
  }
}

try {
  for (const failureMode of ["codec-once", "device-loss-once"]) {
    evidence.modes.push(await runScenario("worker", failureMode));
    evidence.modes.push(await runScenario("main", failureMode));
  }
  evidence.completed = true;
  save();
  console.log(`PASS ${ffmpegBackend ? "M3.8 FFmpeg" : "M3.6"} Chromium recovery: codec + WebGPU device loss, worker + main-thread cleanup and restart`);
} catch (error) {
  evidence.error = error.stack;
  save();
  throw error;
} finally {
  socket.close();
}
