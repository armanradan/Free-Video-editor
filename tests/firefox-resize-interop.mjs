// Attach to an already running isolated Firefox WebDriver BiDi session.
// node tests/firefox-resize-interop.mjs [app-port]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { ensureLargeInputFixture } from "./large-input-fixture.mjs";

const appPort = Number(process.argv[2] ?? 8084);
const outputDirectory = path.resolve("tmp/m36-resize-firefox");
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
let context;
const evidence = { cases: [], completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));

try {
  const session = await send("session.new", { capabilities: { alwaysMatch: {} } });
  evidence.browser = session.capabilities;
  ({ context } = await send("browsingContext.create", { type: "tab" }));
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
  const click = selector => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
  await send("browsingContext.navigate", { context, url: `http://127.0.0.1:${appPort}/?verify=full`, wait: "complete" });
  await waitFor('!!document.querySelector("#convert")');
  await delay(250);
  const element = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
  await send("input.setFiles", { context, element: { sharedId: element.result.sharedId }, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor(`${status}.includes("Output resolves to 320×180") && document.querySelector("#resolved-size")?.textContent.includes("320×180")`);
  evidence.sourceMetadata = await waitFor('document.querySelector("#source-metadata")?.textContent');
  assert.match(evidence.sourceMetadata, /display 640×360 · coded 640×360/);
  assert.match(evidence.sourceMetadata, /Video: H\.264\/AVC \(avc1\./);
  assert.match(evidence.sourceMetadata, /60 frames · 2\.000 s/);
  assert.match(evidence.sourceMetadata, /Audio: AAC · 1 channel · 48\.0 kHz/);
  const namedPresets = JSON.parse(await evaluate('JSON.stringify(Array.from(document.querySelector("#resize-preset").options).map(option=>option.value))'));
  assert.deepEqual(namedPresets.filter(value => /^(hd|fhd|dci|qhd|uhd)-/.test(value)), ["hd-720p", "fhd-1080p", "dci-2k", "qhd-1440p", "uhd-2160p"]);
  await evaluate('{const s=document.querySelector("#resize-preset");s.value="hd-720p";s.dispatchEvent(new Event("change",{bubbles:true}));}');
  evidence.namedPresetUpscale = await waitFor(`${status}.startsWith("FAILED:") && ${status}.includes("would upscale") && ${status}`);

  const setResize = async testCase => {
    await evaluate(`{const s=document.querySelector("#resize-preset");s.value=${JSON.stringify(testCase.mode)};s.dispatchEvent(new Event("change",{bubbles:true}));}`);
    if (testCase.mode === "exact") {
      await waitFor('!!document.querySelector("#resize-width")');
      await evaluate(`{const i=document.querySelector("#resize-width");i.value=${JSON.stringify(String(testCase.width))};i.dispatchEvent(new Event("change",{bubbles:true}));}`);
      await evaluate(`{const i=document.querySelector("#resize-height");i.value=${JSON.stringify(String(testCase.height))};i.dispatchEvent(new Event("change",{bubbles:true}));}`);
      await evaluate(`{const i=document.querySelector("#resize-aspect");if(i.checked!==${testCase.lock}){i.checked=${testCase.lock};i.dispatchEvent(new Event("change",{bubbles:true}));}}`);
    }
    return waitFor(`${status}.includes(${JSON.stringify(`Output resolves to ${testCase.expectedWidth}×${testCase.expectedHeight}`)}) && document.querySelector("#resolved-size")?.textContent.includes(${JSON.stringify(`${testCase.expectedWidth}×${testCase.expectedHeight}`)}) && ${status}`);
  };
  const enabledProfiles = async () => JSON.parse(await evaluate('JSON.stringify(Array.from(document.querySelector("#output-profile").options).filter(o=>!o.disabled).map(o=>o.value))'));
  const selectProfile = profile => evaluate(`{const s=document.querySelector("#output-profile");s.value=${JSON.stringify(profile)};s.dispatchEvent(new Event("change",{bubbles:true}));}`);
  const playback = async () => JSON.parse(await evaluate(`new Promise((resolve,reject)=>{const v=document.createElement("video");v.src=document.querySelector("#download").href;v.onerror=()=>reject(Error("Output playback failed"));v.onloadedmetadata=()=>{v.currentTime=v.duration/2;v.onseeked=()=>{resolve(JSON.stringify({width:v.videoWidth,height:v.videoHeight,duration:v.duration}));v.removeAttribute("src");v.load();};};})`));
  const cases = [
    { name: "original", mode: "original", expectedWidth: 640, expectedHeight: 360 },
    { name: "percent-75", mode: "percent-75", expectedWidth: 480, expectedHeight: 270 },
    { name: "percent-25", mode: "percent-25", expectedWidth: 160, expectedHeight: 90 },
    { name: "exact-locked", mode: "exact", width: 500, height: 500, lock: true, expectedWidth: 500, expectedHeight: 280 },
    { name: "exact-stretch-odd", mode: "exact", width: 501, height: 301, lock: false, expectedWidth: 500, expectedHeight: 300 },
  ];

  let expectedProfiles;
  for (const testCase of cases) {
    const ready = await setResize(testCase);
    assert.match(ready, new RegExp(`Output resolves to ${testCase.expectedWidth}×${testCase.expectedHeight}`));
    const profiles = await enabledProfiles();
    expectedProfiles ??= profiles;
    assert.deepEqual(profiles, expectedProfiles);
    for (const profile of profiles) {
      await selectProfile(profile);
      await click("#convert");
      const summary = await terminal();
      assert.match(summary, /^PASS: 60 H\.264 input frames/);
      assert.match(summary, new RegExp(`→ ${testCase.expectedWidth}×${testCase.expectedHeight}`));
      assert.match(summary, /Diagnostic verification: full re-decode PASS/);
      assert.match(summary, /Output storage: bounded OPFS stream/);
      assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
      const media = await playback();
      assert.deepEqual([media.width, media.height], [testCase.expectedWidth, testCase.expectedHeight]);
      evidence.cases.push({ ...testCase, profile, ready, summary, media });
      save();
    }
  }
  const hevcOutputReason = await evaluate('document.querySelector("#output-profile").querySelector("option[value=\\"mp4-h265-aac\\"]").disabled ? Array.from(document.querySelectorAll(".note")).map(n=>n.textContent).find(t=>t.startsWith("H.265/HEVC MP4 unavailable:")) : "supported"');

  await setResize(cases[1]);
  await selectProfile(expectedProfiles[0]);
  await evaluate(`{const observer=new MutationObserver(()=>{if(document.querySelector("#status").textContent.startsWith("Converting")){observer.disconnect();document.querySelector("#cancel").click();}});observer.observe(document.querySelector("#status"),{subtree:true,childList:true,characterData:true});}`);
  await click("#convert");
  evidence.cancellation = await terminal();
  assert.match(evidence.cancellation, /^CANCELLED:/);
  assert.match(evidence.cancellation, /Cleanup: 0 application-held frame references, 0 samples/);

  await evaluate(`{const s=document.querySelector("#resize-preset");s.value="exact";s.dispatchEvent(new Event("change",{bubbles:true}));}`);
  await waitFor('!!document.querySelector("#resize-width")');
  await evaluate(`{const w=document.querySelector("#resize-width");w.value="800";w.dispatchEvent(new Event("change",{bubbles:true}));const h=document.querySelector("#resize-height");h.value="500";h.dispatchEvent(new Event("change",{bubbles:true}));const a=document.querySelector("#resize-aspect");if(!a.checked){a.checked=true;a.dispatchEvent(new Event("change",{bubbles:true}));}}`);
  evidence.invalidUpscale = await waitFor(`${status}.startsWith("FAILED:") && ${status}.includes("would upscale") && ${status}`);
  assert.equal(await evaluate('!document.querySelector("#resolved-size")'), true);

  const hevcElement = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
  await send("input.setFiles", { context, element: { sharedId: hevcElement.result.sharedId }, files: [path.resolve("fixtures/m36-h265-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
  const hevcInputProbe = await waitFor(`(${status}.includes("Output resolves to 160×90") || ${status}.startsWith("FAILED:")) && ${status}`);
  evidence.hevc = {
    inputProbe: hevcInputProbe,
    outputEnabled: expectedProfiles.includes("mp4-h265-aac"),
    outputReason: hevcOutputReason,
  };
  if (hevcInputProbe.includes("Output resolves")) {
    const metadata = await waitFor('document.querySelector("#source-metadata")?.textContent');
    assert.match(metadata, /Video: H\.265\/HEVC \((?:hvc1|hev1)\./);
    await selectProfile("webm-vp8-video-only");
    await click("#convert");
    evidence.hevc.inputConversion = await terminal();
    assert.match(evidence.hevc.inputConversion, /^PASS: 30 H\.265\/HEVC input frames/);
    assert.match(evidence.hevc.inputConversion, /Diagnostic verification: full re-decode PASS/);
  } else {
    assert.match(hevcInputProbe, /cannot decode the selected H\.265\/HEVC track/);
  }

  const largeInput = ensureLargeInputFixture();
  const largeElement = await send("script.evaluate", { expression: 'document.querySelector("#source-file")', target: { context }, awaitPromise: true });
  await send("input.setFiles", { context, element: { sharedId: largeElement.result.sharedId }, files: [largeInput] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await evaluate(`{const s=document.querySelector("#resize-preset");s.value="percent-25";s.dispatchEvent(new Event("change",{bubbles:true}));}`);
  await waitFor(`${status}.includes("Output resolves to 160×90") && document.querySelector("#resolved-size")?.textContent.includes("160×90")`);
  await selectProfile("webm-vp8-video-only");
  await click("#convert");
  const largeSummary = await terminal();
  assert.match(largeSummary, /^PASS: 60 H\.264 input frames/);
  assert.match(largeSummary, /Output storage: bounded OPFS stream/);
  assert.doesNotMatch(largeSummary, /memory fallback/);
  evidence.largeInput = { bytes: fs.statSync(largeInput).size, summary: largeSummary };
  assert.ok(evidence.largeInput.bytes > 256 * 1024 * 1024);

  evidence.profiles = expectedProfiles;
  evidence.gpu = await evaluate('document.querySelector("#selected-gpu").textContent');
  evidence.execution = await evaluate('document.querySelector("#execution-context").textContent');
  evidence.completed = true;
  save();
  console.log(`PASS M3.6 resize/streaming Firefox: ${cases.length} size modes × ${expectedProfiles.length} profiles, >256 MiB sparse MP4, cancellation, invalid-upscale rejection; ${evidence.gpu}`);
} catch (error) {
  evidence.error = error.stack;
  save();
  throw error;
} finally {
  if (context) await send("browsingContext.close", { context }).catch(() => {});
  await send("session.end").catch(() => {});
  socket.close();
}
