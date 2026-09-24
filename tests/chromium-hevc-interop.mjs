// Focused HEVC input/output capability and conversion validation.
// node tests/chromium-hevc-interop.mjs [debug-port] [app-port]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const debugPort = Number(process.argv[2] ?? 9227);
const appPort = Number(process.argv[3] ?? 8084);
const outputDirectory = path.resolve("tmp/m36-hevc-chromium");
fs.mkdirSync(outputDirectory, { recursive: true });
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
const evidence = { browser: version, completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));

try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full` });
  await waitFor('!!document.querySelector("#source-file")');
  const document = await send("DOM.getDocument");
  const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
  const choose = async file => {
    await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [path.resolve(file)] });
    await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
    return waitFor(`(${status}.includes("Output resolves") || ${status}.startsWith("FAILED:")) && ${status}`);
  };

  evidence.h264Probe = await choose("fixtures/m2-h264-aac.mp4");
  assert.match(evidence.h264Probe, /Output resolves to 320×180/);
  evidence.hevcOutput = await evaluate(`({
    enabled: !document.querySelector('#output-profile option[value="mp4-h265-aac"]').disabled,
    reason: Array.from(document.querySelectorAll('.note')).map(node=>node.textContent).find(text=>text.startsWith('H.265/HEVC MP4 unavailable:')) ?? 'supported'
  })`);
  if (evidence.hevcOutput.enabled) {
    await evaluate('{const s=document.querySelector("#output-profile");s.value="mp4-h265-aac";s.dispatchEvent(new Event("change",{bubbles:true}));document.querySelector("#convert").click();}');
    evidence.hevcOutput.conversion = await terminal();
    assert.match(evidence.hevcOutput.conversion, /MP4\/H\.265\/AAC/);
    assert.match(evidence.hevcOutput.conversion, /full re-decode PASS/);
  }

  evidence.hevcInput = { probe: await choose("fixtures/m36-h265-aac.mp4") };
  if (evidence.hevcInput.probe.includes("Output resolves")) {
    evidence.hevcInput.metadata = await waitFor('document.querySelector("#source-metadata")?.textContent');
    assert.match(evidence.hevcInput.metadata, /Video: H\.265\/HEVC \((?:hvc1|hev1)\./);
    await evaluate('{const s=document.querySelector("#output-profile");s.value="webm-vp8-video-only";s.dispatchEvent(new Event("change",{bubbles:true}));document.querySelector("#convert").click();}');
    evidence.hevcInput.conversion = await terminal();
    assert.match(evidence.hevcInput.conversion, /^PASS: 30 H\.265\/HEVC input frames/);
    assert.match(evidence.hevcInput.conversion, /full re-decode PASS/);
  } else {
    assert.match(evidence.hevcInput.probe, /cannot decode the selected H\.265\/HEVC track/);
  }
  evidence.completed = true;
  save();
  console.log(`PASS focused Chromium HEVC: input=${evidence.hevcInput.probe.includes("Output resolves") ? "decoded and converted" : "capability-gated unavailable"}; output=${evidence.hevcOutput.enabled ? "encoded and verified" : "capability-gated unavailable"}`);
} catch (error) {
  evidence.error = error.stack;
  save();
  throw error;
} finally {
  if (targetId) await send("Target.closeTarget", { targetId }, true).catch(() => {});
  socket.close();
}
