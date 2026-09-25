// Isolated Chromium CDP layout smoke test: node tests/chromium-ui-compact.mjs [9227] [8084]
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
const waitFor = async expression => {
  for (let attempt = 0; attempt < 300; attempt += 1) {
    const value = await evaluate(expression);
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error(`Timed out: ${expression}; status=${await evaluate('document.querySelector("#status")?.textContent')}`);
};
const geometry = () => evaluate(`(() => {
  const rect = selector => { const r = document.querySelector(selector).getBoundingClientRect(); return { x:r.x, y:r.y, width:r.width, height:r.height }; };
  return { shell:rect(".shell"), controls:rect(".controls-panel"), monitor:rect(".monitor-panel"),
    viewport:innerWidth, documentWidth:document.documentElement.scrollWidth,
    advancedClosed:!document.querySelector(".advanced-settings").open,
    technicalClosed:!document.querySelector(".technical-notes").open };
})()`);

try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full` });
  await waitFor('!!document.querySelector("#source-file")');
  const desktop = await geometry();
  assert.ok(desktop.monitor.x > desktop.controls.x + desktop.controls.width);
  assert.ok(desktop.documentWidth <= desktop.viewport);
  assert.ok(desktop.advancedClosed && desktop.technicalClosed);
  const document = await send("DOM.getDocument");
  const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor('!!document.querySelector("#source-metadata") && !document.querySelector("#output-profile").disabled');
  assert.match(await evaluate('document.querySelector("#source-metadata").textContent'), /H\.264\/AVC/);
  await evaluate('document.querySelector("#convert").click()');
  const result = await waitFor('!document.querySelector("#convert").disabled && /^(PASS|FAILED):/.test(document.querySelector("#status")?.textContent) && document.querySelector("#status").textContent');
  assert.match(result, /^PASS:/, result);
  assert.equal(await evaluate('Array.from(document.querySelectorAll(".preview-panel canvas")).filter(canvas => getComputedStyle(canvas).display !== "none").length'), 1);
  const screenshot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  fs.mkdirSync("tmp/ui-compact", { recursive: true });
  fs.writeFileSync("tmp/ui-compact/desktop.png", Buffer.from(screenshot.data, "base64"));
  await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: false });
  const narrow = await geometry();
  assert.ok(narrow.monitor.y >= narrow.controls.y + narrow.controls.height);
  assert.ok(narrow.documentWidth <= narrow.viewport, JSON.stringify(narrow));
  const narrowScreenshot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  fs.writeFileSync("tmp/ui-compact/narrow.png", Buffer.from(narrowScreenshot.data, "base64"));
  console.log(JSON.stringify({ browser: browser.Browser, desktop, narrow, converted: result.startsWith("PASS:") }, null, 2));
} finally {
  if (targetId) await send("Target.closeTarget", { targetId }, true);
  socket.close();
}
