// Long logical-raw-video FFmpeg ring test: node tests/chromium-m38-long.mjs [9227] [8084]
import "./generate-m38-long-fixture.mjs";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const browserPort = Number(process.argv[2] ?? 9227);
const appPort = Number(process.argv[3] ?? 8084);
const browser = await fetch(`http://127.0.0.1:${browserPort}/json/version`).then(response => response.json());
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
const waitFor = async (expression, timeout = 600_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await evaluate(expression);
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 250));
  }
  throw Error(`Timed out: ${expression}; status=${await evaluate('document.querySelector("#status")?.textContent')}`);
};
const evidence = { browser: browser.Browser };
try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?backend=ffmpeg-wasm&verify=full` });
  await waitFor('!!document.querySelector("#source-file")');
  evidence.isolated = await evaluate('crossOriginIsolated');
  assert.equal(evidence.isolated, true);
  const document = await send("DOM.getDocument");
  const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: input.nodeId,
    files: [path.resolve("tmp/m38-long/synthetic-2400f-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor('!document.querySelector("#output-profile").disabled');
  evidence.resolvedSize = await evaluate('document.querySelector("#resolved-size").textContent');
  assert.match(evidence.resolvedSize, /640×360/);
  await evaluate('window.__m38Heartbeat={ticks:0,maxGapMs:0,last:performance.now()};window.__m38Timer=setInterval(()=>{const now=performance.now();const h=window.__m38Heartbeat;h.maxGapMs=Math.max(h.maxGapMs,now-h.last);h.last=now;h.ticks++},50)');
  await evaluate('document.querySelector("#convert").click()');
  evidence.summary = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
  evidence.heartbeat = await evaluate('clearInterval(window.__m38Timer);window.__m38Heartbeat');
  assert.match(evidence.summary, /^PASS: 2400 /, evidence.summary);
  assert.match(evidence.summary, /raw RGBA live ring=2211840000 bytes/);
  assert.match(evidence.summary, /consumed=2211840000; no raw OPFS spool/);
  assert.match(evidence.summary, /full re-decode PASS/);
  assert.ok(evidence.heartbeat.ticks > 50, JSON.stringify(evidence.heartbeat));
  evidence.opfsEntries = await evaluate('(async()=>{const names=[];for await(const [name] of (await navigator.storage.getDirectory()).entries())if(name.startsWith("diaxus-"))names.push(name);return names})()');
  assert.equal(evidence.opfsEntries.filter(name => name.endsWith(".rgba.partial")).length, 0);
  const dataUrl = await evaluate('new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=reject;fetch(document.querySelector("#download").href).then(r=>r.blob()).then(blob=>reader.readAsDataURL(blob),reject)})');
  fs.writeFileSync("tmp/m38-long/chromium-output.mp4", Buffer.from(dataUrl.split(",")[1], "base64"));
  await evaluate('{const observer=new MutationObserver(()=>{const progress=Number(document.querySelector("#status")?.textContent.match(/^Converting (\\d+)%/)?.[1] ?? 0);if(progress>=10){observer.disconnect();document.querySelector("#cancel").click()}});observer.observe(document.querySelector("#status"),{subtree:true,childList:true,characterData:true});document.querySelector("#convert").click()}');
  evidence.cancellation = await waitFor('!document.querySelector("#convert").disabled && document.querySelector("#status")?.textContent.startsWith("CANCELLED:") && document.querySelector("#status").textContent');
  assert.match(evidence.cancellation, /Cleanup: 0 application-held frame references, 0 samples/);
  const retryDocument = await send("DOM.getDocument");
  const retryInput = await send("DOM.querySelector", { nodeId: retryDocument.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: retryInput.nodeId,
    files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor('document.querySelector("#source-metadata")?.textContent.includes("m2-h264-aac.mp4") && !document.querySelector("#output-profile").disabled');
  await evaluate('document.querySelector("#convert").click()');
  evidence.retry = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
  assert.match(evidence.retry, /^PASS: 60 /, evidence.retry);
  console.log(JSON.stringify({ browser: evidence.browser, isolated: evidence.isolated,
    resolvedSize: evidence.resolvedSize, heartbeat: evidence.heartbeat,
    summary: evidence.summary.slice(0, 800), opfsEntries: evidence.opfsEntries,
    cancellation: evidence.cancellation.slice(0, 130), retry: evidence.retry.slice(0, 100) }, null, 2));
} finally {
  fs.writeFileSync("tmp/m38-long/chromium-evidence.json", JSON.stringify(evidence, null, 2));
  if (targetId) await send("Target.closeTarget", { targetId }, true);
  socket.close();
}
