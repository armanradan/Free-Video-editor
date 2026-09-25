// Inject one mid-job codec failure, then verify cleanup and same-page restart.
// node tests/firefox-recovery-interop.mjs [app-port]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const appPort = Number(process.argv[2] ?? 8084);
const ffmpegBackend = process.argv[3] === "ffmpeg-wasm";
const outputDirectory = path.resolve(ffmpegBackend ? "tmp/m38-recovery-firefox" : "tmp/m36-recovery-firefox");
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
const evidence = { modes: [], completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));

async function runScenario(mode, failureMode) {
  const { context } = await send("browsingContext.create", { type: "tab" });
  const evaluate = async expression => {
    const result = await send("script.evaluate", { expression, target: { context }, awaitPromise: true });
    if (result.type === "exception") throw Error(JSON.stringify(result));
    return result.result.value;
  };
  const status = 'document.querySelector("#status")?.textContent';
  const waitFor = async (expression, timeout = 60_000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const value = await evaluate(expression);
      if (value) return value;
      await delay(75);
    }
    throw Error(`Timed out: ${expression}; status=${await evaluate(status)}`);
  };
  const terminal = () => waitFor(`!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(${status}) && ${status}`);
  try {
    const main = mode === "main" ? "&execution=main" : "";
    await send("browsingContext.navigate", { context, url: `http://127.0.0.1:${appPort}/?verify=full&failure=${failureMode}${main}${ffmpegBackend ? "&backend=ffmpeg-wasm" : ""}`, wait: "complete" });
    await waitFor('!!document.querySelector("#source-file")');
    const element = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
    await send("input.setFiles", { context, element: { sharedId: element.result.sharedId }, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
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
    const playback = JSON.parse(await evaluate(`new Promise((resolve,reject)=>{const v=document.createElement("video");v.src=document.querySelector("#download").href;v.onerror=()=>reject(Error("playback failed"));v.onloadedmetadata=()=>resolve(JSON.stringify({width:v.videoWidth,height:v.videoHeight,duration:v.duration}));})`));
    assert.deepEqual([playback.width, playback.height], [320, 180]);
    return { mode, failureMode, failure, restart, playback, execution: await evaluate('document.querySelector("#execution-context").textContent') };
  } finally {
    await send("browsingContext.close", { context }).catch(() => {});
  }
}

try {
  const session = await send("session.new", { capabilities: { alwaysMatch: {} } });
  evidence.browser = session.capabilities;
  for (const failureMode of ["codec-once", "device-loss-once"]) {
    evidence.modes.push(await runScenario("worker", failureMode));
    evidence.modes.push(await runScenario("main", failureMode));
  }
  evidence.completed = true;
  save();
  console.log(`PASS ${ffmpegBackend ? "M3.8 FFmpeg" : "M3.6"} Firefox recovery: codec + WebGPU device loss, worker + main-thread cleanup and restart`);
} catch (error) {
  evidence.error = error.stack;
  save();
  throw error;
} finally {
  await send("session.end").catch(() => {});
  socket.close();
}
