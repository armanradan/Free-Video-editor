// Capability regression: node tests/chromium-m38-isolation.mjs [9227] [8085]
import assert from "node:assert/strict";
import path from "node:path";

const browserPort = Number(process.argv[2] ?? 9227);
const appPort = Number(process.argv[3] ?? 8085);
const browser = await fetch(`http://127.0.0.1:${browserPort}/json/version`).then(r => r.json());
const socket = new WebSocket(browser.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let id = 0;
let sessionId;
let targetId;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  const entry = pending.get(message.id);
  if (!entry) return;
  pending.delete(message.id);
  message.error ? entry.reject(Error(JSON.stringify(message.error))) : entry.resolve(message.result);
};
const send = (method, params = {}, browserCommand = false) => new Promise((resolve, reject) => {
  const next = ++id;
  pending.set(next, { resolve, reject });
  socket.send(JSON.stringify({ id: next, method, params, ...(!browserCommand && sessionId ? { sessionId } : {}) }));
});
const evaluate = async expression => {
  const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
};
const waitFor = async expression => {
  for (let attempt = 0; attempt < 300; attempt++) {
    const value = await evaluate(expression);
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error(`Timed out: ${expression}`);
};
try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?backend=ffmpeg-wasm` });
  await waitFor('!!document.querySelector("#source-file")');
  assert.equal(await evaluate("crossOriginIsolated"), false);
  const document = await send("DOM.getDocument");
  const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: input.nodeId,
    files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  const reason = await waitFor('([...document.querySelectorAll(".profile-note")].map(e=>e.textContent).join(" | ").includes("cross-origin-isolated")) && [...document.querySelectorAll(".profile-note")].map(e=>e.textContent).join(" | ")');
  assert.match(reason, /COOP\/COEP headers/);
  assert.equal(await evaluate('document.querySelector("#output-profile").disabled'), true);
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?backend=webcodecs` });
  await waitFor('!!document.querySelector("#source-file")');
  const webDocument = await send("DOM.getDocument");
  const webInput = await send("DOM.querySelector", { nodeId: webDocument.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: webInput.nodeId,
    files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor('!document.querySelector("#output-profile").disabled');
  console.log(JSON.stringify({ browser: browser.Browser, isolated: false,
    ffmpegDisabledReason: reason, webcodecsEnabled: true }, null, 2));
} finally {
  if (targetId) await send("Target.closeTarget", { targetId }, true);
  socket.close();
}
