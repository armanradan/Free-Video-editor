// Isolated Edge/Chromium CDP session: node tests/chromium-m37-spike.mjs [9227] [8084]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { ensureLargeInputFixture } from "./large-input-fixture.mjs";

const debugPort = Number(process.argv[2] ?? 9227);
const appPort = Number(process.argv[3] ?? 8084);
const browser = await fetch(`http://127.0.0.1:${debugPort}/json/version`).then(r => r.json());
const socket = new WebSocket(browser.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let id = 0;
let sessionId;
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
const waitFor = async (expression, timeout = 180_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await evaluate(expression);
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error(`Timed out: ${expression}; status=${await evaluate('document.querySelector("#status")?.textContent')}`);
};
const evidence = { browser: browser.Browser, cases: [] };
let targetId;
try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  for (const backend of ["webcodecs", "ffmpeg-wasm"]) {
    await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full&backend=${backend}` });
    await waitFor('!!document.querySelector("#source-file")');
    const baselineEntries = backend === "ffmpeg-wasm"
      ? await evaluate('(async()=>{const names=[];for await(const [name] of (await navigator.storage.getDirectory()).entries())if(name.startsWith("diaxus-"))names.push(name);return names})()')
      : [];
    const document = await send("DOM.getDocument");
    const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
    await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
    await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
    await waitFor('!document.querySelector("#output-profile").disabled');
    await evaluate('{const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
    await waitFor('document.querySelector("#resolved-size")?.textContent.includes("320×180") && !document.querySelector("#output-profile").disabled');
    await evaluate('{const s=document.querySelector("#output-profile");s.value="mp4-h264-aac";s.dispatchEvent(new Event("change",{bubbles:true}));}');
    await evaluate('document.querySelector("#convert").click()');
    const summary = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
    assert.match(summary, /^PASS:/, summary);
    const bytes = await evaluate('fetch(document.querySelector("#download")?.href).then(r=>r.arrayBuffer()).then(b=>b.byteLength).catch(()=>0)');
    const dataUrl = await evaluate('new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=reject;fetch(document.querySelector("#download").href).then(r=>r.blob()).then(blob=>reader.readAsDataURL(blob),reject);})');
    fs.mkdirSync("tmp/m37-chromium", { recursive: true });
    fs.writeFileSync(`tmp/m37-chromium/${backend}.mp4`, Buffer.from(dataUrl.split(",")[1], "base64"));
    const resources = await evaluate('performance.getEntriesByType("resource").filter(e=>/ffmpeg|m2-|converter-web/.test(e.name)).map(e=>({name:e.name.split("/").pop(),transferSize:e.transferSize,decodedBodySize:e.decodedBodySize,durationMs:e.duration}))');
    const jsHeapBytes = await evaluate('performance.memory?.usedJSHeapSize ?? null');
    const gpu = await evaluate('document.querySelector("#selected-gpu")?.textContent');
    evidence.cases.push({ backend, summary, bytes, resources, jsHeapBytes, gpu });
    console.log(backend, summary.slice(0, 350));
    assert.match(summary, /^PASS:/);
    assert.match(summary, /full re-decode PASS/);
    assert.ok(bytes > 0);
    if (backend === "ffmpeg-wasm") {
      assert.match(summary, /raw RGBA live ring=13824000 bytes/);
      assert.match(summary, /consumed=13824000; no raw OPFS spool/);
      await evaluate('{const observer=new MutationObserver(()=>{if(/^Converting/.test(document.querySelector("#status")?.textContent)){observer.disconnect();document.querySelector("#cancel").click();}});observer.observe(document.querySelector("#status"),{subtree:true,childList:true,characterData:true});}');
      await evaluate('document.querySelector("#convert").click()');
      const cancelled = await waitFor('!document.querySelector("#convert").disabled && document.querySelector("#status")?.textContent.startsWith("CANCELLED:") && document.querySelector("#status").textContent');
      evidence.cancellation = cancelled;
      assert.match(cancelled, /Cleanup: 0 application-held frame references, 0 samples/);
      await evaluate('document.querySelector("#convert").click()');
      const retry = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
      evidence.retry = retry;
      assert.match(retry, /^PASS:/);
      const document = await send("DOM.getDocument");
      const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
      await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [ensureLargeInputFixture()] });
      await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
      evidence.largeInput = await waitFor('document.querySelector("#source-metadata")?.textContent.includes("m2-h264-aac-plus-free-box") && !document.querySelector("#output-profile").disabled');
      await evaluate('document.querySelector("#convert").click()');
      evidence.largeInputSummary = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
      assert.match(evidence.largeInputSummary, /^PASS:/, evidence.largeInputSummary);
      assert.match(evidence.largeInputSummary, /source File mounted via WORKERFS/);
      assert.match(evidence.largeInputSummary, /no raw OPFS spool/);
      evidence.opfsEntries = await evaluate('(async()=>{const names=[];for await(const [name] of (await navigator.storage.getDirectory()).entries())if(name.startsWith("diaxus-"))names.push(name);return names})()');
      assert.equal(evidence.opfsEntries.filter(name => name.endsWith(".rgba.partial") && !baselineEntries.includes(name)).length, 0);
    }
  }
} finally {
  fs.mkdirSync("tmp/m37-chromium", { recursive: true });
  fs.writeFileSync("tmp/m37-chromium/evidence.json", JSON.stringify(evidence, null, 2));
  if (targetId) await send("Target.closeTarget", { targetId }, true);
  socket.close();
}
