// Long logical-raw-video Firefox BiDi test: node tests/firefox-m38-long.mjs [8084]
import "./generate-m38-long-fixture.mjs";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const appPort = Number(process.argv[2] ?? 8084);
const socket = new WebSocket("ws://127.0.0.1:9226/session");
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let id = 0;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  const entry = pending.get(message.id);
  if (!entry) return;
  pending.delete(message.id);
  message.type === "error" ? entry.reject(Error(JSON.stringify(message))) : entry.resolve(message.result);
};
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const next = ++id;
  pending.set(next, { resolve, reject });
  socket.send(JSON.stringify({ id: next, method, params }));
});
const evidence = {};
let context;
try {
  evidence.browser = (await send("session.new", { capabilities: { alwaysMatch: {} } })).capabilities;
  ({ context } = await send("browsingContext.create", { type: "tab" }));
  const evaluate = async expression => {
    const result = await send("script.evaluate", { expression, target: { context }, awaitPromise: true });
    if (result.type === "exception") throw Error(JSON.stringify(result));
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
  await send("browsingContext.navigate", { context,
    url: `http://127.0.0.1:${appPort}/?backend=ffmpeg-wasm&verify=full`, wait: "complete" });
  await waitFor('!!document.querySelector("#source-file")');
  evidence.isolated = await evaluate('crossOriginIsolated');
  assert.equal(evidence.isolated, true);
  const input = await send("script.evaluate", { expression: 'document.querySelector("#source-file")',
    target: { context }, awaitPromise: true });
  await send("input.setFiles", { context, element: { sharedId: input.result.sharedId },
    files: [path.resolve("tmp/m38-long/synthetic-2400f-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor('!document.querySelector("#output-profile").disabled');
  evidence.resolvedSize = await evaluate('document.querySelector("#resolved-size").textContent');
  assert.match(evidence.resolvedSize, /640×360/);
  await evaluate('window.__m38Heartbeat={ticks:0,maxGapMs:0,last:performance.now()};window.__m38Timer=setInterval(()=>{const now=performance.now();const h=window.__m38Heartbeat;h.maxGapMs=Math.max(h.maxGapMs,now-h.last);h.last=now;h.ticks++},50)');
  await evaluate('document.querySelector("#convert").click()');
  evidence.summary = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
  evidence.heartbeat = JSON.parse(await evaluate('clearInterval(window.__m38Timer);JSON.stringify(window.__m38Heartbeat)'));
  assert.match(evidence.summary, /^PASS: 2400 /, evidence.summary);
  assert.match(evidence.summary, /raw RGBA live ring=2211840000 bytes/);
  assert.match(evidence.summary, /consumed=2211840000; no raw OPFS spool/);
  assert.match(evidence.summary, /full re-decode PASS/);
  assert.ok(evidence.heartbeat.ticks > 50, JSON.stringify(evidence.heartbeat));
  evidence.opfsEntries = JSON.parse(await evaluate('(async()=>{const names=[];for await(const [name] of (await navigator.storage.getDirectory()).entries())if(name.startsWith("diaxus-"))names.push(name);return JSON.stringify(names)})()'));
  assert.equal(evidence.opfsEntries.filter(name => name.endsWith(".rgba.partial")).length, 0);
  const dataUrl = await evaluate('new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=reject;fetch(document.querySelector("#download").href).then(r=>r.blob()).then(blob=>reader.readAsDataURL(blob),reject)})');
  fs.writeFileSync("tmp/m38-long/firefox-output.mp4", Buffer.from(dataUrl.split(",")[1], "base64"));
  await evaluate('{document.querySelector("#convert").click();setTimeout(()=>document.querySelector("#cancel").click(),5000)}');
  evidence.cancellation = await waitFor('!document.querySelector("#convert").disabled && document.querySelector("#status")?.textContent.startsWith("CANCELLED:") && document.querySelector("#status").textContent', 120_000);
  assert.match(evidence.cancellation, /Cleanup: 0 application-held frame references, 0 samples/);
  const retryInput = await send("script.evaluate", { expression: 'document.querySelector("#source-file")',
    target: { context }, awaitPromise: true });
  await send("input.setFiles", { context, element: { sharedId: retryInput.result.sharedId },
    files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor('document.querySelector("#source-metadata")?.textContent.includes("m2-h264-aac.mp4") && !document.querySelector("#output-profile").disabled');
  await evaluate('document.querySelector("#convert").click()');
  evidence.retry = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
  assert.match(evidence.retry, /^PASS: 60 /, evidence.retry);
  console.log(JSON.stringify({ browser: evidence.browser.browserName,
    version: evidence.browser.browserVersion, isolated: evidence.isolated,
    resolvedSize: evidence.resolvedSize, heartbeat: evidence.heartbeat,
    summary: evidence.summary.slice(0, 800), opfsEntries: evidence.opfsEntries,
    cancellation: evidence.cancellation.slice(0, 130), retry: evidence.retry.slice(0, 100) }, null, 2));
} finally {
  fs.writeFileSync("tmp/m38-long/firefox-evidence.json", JSON.stringify(evidence, null, 2));
  if (context) await send("browsingContext.close", { context });
  await send("session.end");
  socket.close();
}
