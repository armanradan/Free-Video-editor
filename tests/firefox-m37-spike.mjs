// Isolated Firefox BiDi session: node tests/firefox-m37-spike.mjs [8084]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { ensureLargeInputFixture } from "./large-input-fixture.mjs";

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
const evidence = { cases: [] };
let context;
try {
  evidence.browser = (await send("session.new", { capabilities: { alwaysMatch: {} } })).capabilities;
  ({ context } = await send("browsingContext.create", { type: "tab" }));
  const evaluate = async expression => {
    const result = await send("script.evaluate", { expression, target: { context }, awaitPromise: true });
    if (result.type === "exception") throw Error(JSON.stringify(result));
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
  for (const backend of ["webcodecs", "ffmpeg-wasm"]) {
    await send("browsingContext.navigate", { context, url: `http://127.0.0.1:${appPort}/?backend=${backend}&verify=full`, wait: "complete" });
    await waitFor('!!document.querySelector("#source-file")');
    const element = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
    await send("input.setFiles", { context, element: { sharedId: element.result.sharedId }, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
    await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
    await waitFor('!document.querySelector("#output-profile").disabled');
    await evaluate('{const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
    await waitFor('document.querySelector("#resolved-size")?.textContent.includes("320×180") && !document.querySelector("#output-profile").disabled');
    const mp4Available = await evaluate('!document.querySelector("#output-profile").querySelector("option[value=mp4-h264-aac]").disabled');
    if (backend === "webcodecs") {
      evidence.cases.push({ backend, mp4Available });
      assert.equal(mp4Available, false);
      continue;
    }
    assert.equal(mp4Available, true);
    await evaluate('document.querySelector("#convert").click()');
    const summary = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
    const gpu = await evaluate('document.querySelector("#selected-gpu")?.textContent');
    evidence.cases.push({ backend, mp4Available, summary, gpu });
    console.log(summary.slice(0, 420));
    assert.match(summary, /^PASS:/);
    assert.match(summary, /full re-decode PASS/);
    assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
    const dataUrl = await evaluate('new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=reject;fetch(document.querySelector("#download").href).then(r=>r.blob()).then(blob=>reader.readAsDataURL(blob),reject);})');
    fs.mkdirSync("tmp/m37-firefox", { recursive: true });
    fs.writeFileSync("tmp/m37-firefox/ffmpeg-wasm.mp4", Buffer.from(dataUrl.split(",")[1], "base64"));
    await evaluate('{const observer=new MutationObserver(()=>{if(/^Converting/.test(document.querySelector("#status")?.textContent)){observer.disconnect();document.querySelector("#cancel").click();}});observer.observe(document.querySelector("#status"),{subtree:true,childList:true,characterData:true});}');
    await evaluate('document.querySelector("#convert").click()');
    evidence.cancellation = await waitFor('!document.querySelector("#convert").disabled && document.querySelector("#status")?.textContent.startsWith("CANCELLED:") && document.querySelector("#status").textContent');
    assert.match(evidence.cancellation, /Cleanup: 0 application-held frame references, 0 samples/);
    await evaluate('document.querySelector("#convert").click()');
    evidence.retry = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
    assert.match(evidence.retry, /^PASS:/);
    const largeInput = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
    await send("input.setFiles", { context, element: { sharedId: largeInput.result.sharedId }, files: [ensureLargeInputFixture()] });
    await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
    evidence.limit = await waitFor('document.querySelector("#status")?.textContent.includes("64 MiB memory cap") && document.querySelector("#status").textContent');
    evidence.limitUi = JSON.parse(await evaluate('JSON.stringify({selectorDisabled:document.querySelector("#output-profile").disabled,reason:Array.from(document.querySelectorAll(".note")).map(n=>n.textContent).find(t=>t.includes("64 MiB memory cap")),metadata:document.querySelector("#source-metadata")?.textContent})'));
    assert.equal(evidence.limitUi.selectorDisabled, true);
    assert.match(evidence.limitUi.reason, /64 MiB memory cap/);
    assert.match(evidence.limitUi.metadata, /m2-h264-aac-plus-free-box/);
  }
} finally {
  fs.mkdirSync("tmp/m37-firefox", { recursive: true });
  fs.writeFileSync("tmp/m37-firefox/evidence.json", JSON.stringify(evidence, null, 2));
  if (context) await send("browsingContext.close", { context });
  await send("session.end");
  socket.close();
}
