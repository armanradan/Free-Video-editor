// Attach to an already running isolated Chromium debugging session.
// node tests/chromium-resize-interop.mjs [debug-port] [app-port]
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { ensureLargeInputFixture } from "./large-input-fixture.mjs";

const debugPort = Number(process.argv[2] ?? 9227);
const appPort = Number(process.argv[3] ?? 8084);
const outputDirectory = path.resolve("tmp/m36-resize-chromium");
fs.mkdirSync(outputDirectory, { recursive: true });
const version = await fetch(`http://127.0.0.1:${debugPort}/json/version`, { signal: AbortSignal.timeout(5000) })
  .then(response => response.json()).catch(error => { throw Error(`No isolated Chromium session on port ${debugPort}.`, { cause: error }); });
const socket = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let nextId = 0;
let sessionId;
let targetId;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  if (message.method === "Runtime.exceptionThrown") {
    console.error(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text);
    return;
  }
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
const statusExpression = 'document.querySelector("#status")?.textContent';
const waitFor = async (expression, timeout = 60_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const result = await evaluate(expression);
    if (result) return result;
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  throw Error(`Timed out: ${expression}; status=${await evaluate(statusExpression)}`);
};
const terminal = () => waitFor(`!document.querySelector("#convert").disabled && /^(PASS|FAILED|CANCELLED):/.test(${statusExpression}) && ${statusExpression}`);
const click = selector => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
const evidence = { browser: version, cases: [], completed: false };
const save = () => fs.writeFileSync(path.join(outputDirectory, "evidence.json"), JSON.stringify(evidence, null, 2));

try {
  ({ targetId } = await send("Target.createTarget", { url: "about:blank" }, true));
  ({ sessionId } = await send("Target.attachToTarget", { targetId, flatten: true }, true));
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full` });
  await waitFor('!!document.querySelector("#convert")');
  await new Promise(resolve => setTimeout(resolve, 250));
  evidence.beforeInput = await evaluate('({profileDisabled:document.querySelector("#output-profile").disabled,convertDisabled:document.querySelector("#convert").disabled,note:Array.from(document.querySelectorAll(".note")).map(node=>node.textContent).find(text=>text.startsWith("Select an input to check output profiles"))})');
  assert.equal(evidence.beforeInput.profileDisabled, true);
  assert.equal(evidence.beforeInput.convertDisabled, true);
  assert.ok(evidence.beforeInput.note);
  evidence.opfsBaseline = await evaluate(`(async()=>{const root=await navigator.storage.getDirectory();const names=[];for await(const name of root.keys())names.push(name);return names.filter(name=>name.startsWith("diaxus-")&&name.endsWith(".partial")).sort();})()`);
  const document = await send("DOM.getDocument");
  const input = await send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor(`${statusExpression}.includes("Output resolves to 640×360") && document.querySelector("#resolved-size")?.textContent.includes("640×360")`);
  assert.equal(await evaluate('document.querySelector("#resize-preset").value'), 'original');
  evidence.afterInput = await evaluate('({profileDisabled:document.querySelector("#output-profile").disabled,convertDisabled:document.querySelector("#convert").disabled,mp4Disabled:document.querySelector("#output-profile option[value=\\"mp4-h264-aac\\"]").disabled})');
  assert.equal(evidence.afterInput.profileDisabled, false);
  assert.equal(evidence.afterInput.convertDisabled, false);
  assert.equal(evidence.afterInput.mp4Disabled, false);
  evidence.sourceMetadata = await waitFor('document.querySelector("#source-metadata")?.textContent');
  assert.match(evidence.sourceMetadata, /display 640×360 · coded 640×360/);
  assert.match(evidence.sourceMetadata, /Video: H\.264\/AVC \(avc1\./);
  assert.match(evidence.sourceMetadata, /60 frames · 2\.000 s/);
  assert.match(evidence.sourceMetadata, /Audio: AAC · 1 channel · 48\.0 kHz/);
  const namedPresets = await evaluate('Array.from(document.querySelector("#resize-preset").options).map(option=>option.value)');
  assert.deepEqual(namedPresets.filter(value => /^(hd|fhd|dci|qhd|uhd)-/.test(value)), ["hd-720p", "fhd-1080p", "dci-2k", "qhd-1440p", "uhd-2160p"]);
  await evaluate('{const s=document.querySelector("#resize-preset");s.value="hd-720p";s.dispatchEvent(new Event("change",{bubbles:true}));}');
  evidence.namedPresetUpscale = await waitFor(`${statusExpression}.startsWith("FAILED:") && ${statusExpression}.includes("would upscale") && ${statusExpression}`);

  const setResize = async testCase => {
    await evaluate(`{const s=document.querySelector("#resize-preset");s.value=${JSON.stringify(testCase.mode)};s.dispatchEvent(new Event("change",{bubbles:true}));}`);
    if (testCase.mode === "exact") {
      await waitFor('!!document.querySelector("#resize-width")');
      await evaluate(`{const i=document.querySelector("#resize-width");i.value=${JSON.stringify(String(testCase.width))};i.dispatchEvent(new Event("change",{bubbles:true}));}`);
      await evaluate(`{const i=document.querySelector("#resize-height");i.value=${JSON.stringify(String(testCase.height))};i.dispatchEvent(new Event("change",{bubbles:true}));}`);
      await evaluate(`{const i=document.querySelector("#resize-aspect");if(i.checked!==${testCase.lock}){i.checked=${testCase.lock};i.dispatchEvent(new Event("change",{bubbles:true}));}}`);
    }
    return waitFor(`${statusExpression}.includes(${JSON.stringify(`Output resolves to ${testCase.expectedWidth}×${testCase.expectedHeight}`)}) && document.querySelector("#resolved-size")?.textContent.includes(${JSON.stringify(`${testCase.expectedWidth}×${testCase.expectedHeight}`)}) && ${statusExpression}`);
  };
  const enabledProfiles = () => evaluate('Array.from(document.querySelector("#output-profile").options).filter(option=>!option.disabled).map(option=>option.value)');
  const selectProfile = profile => evaluate(`{const s=document.querySelector("#output-profile");s.value=${JSON.stringify(profile)};s.dispatchEvent(new Event("change",{bubbles:true}));}`);
  const playback = () => evaluate(`new Promise((resolve,reject)=>{const v=document.createElement("video");v.src=document.querySelector("#download").href;v.onerror=()=>reject(Error("Output playback failed"));v.onloadedmetadata=()=>{v.currentTime=v.duration/2;v.onseeked=()=>{resolve({width:v.videoWidth,height:v.videoHeight,duration:v.duration});v.removeAttribute("src");v.load();};};})`);
  const cases = [
    { name: "original", mode: "original", expectedWidth: 640, expectedHeight: 360 },
    { name: "percent-75", mode: "percent-75", expectedWidth: 480, expectedHeight: 270 },
    { name: "percent-50", mode: "percent-50", expectedWidth: 320, expectedHeight: 180 },
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
      assert.match(summary, /removed on replacement\/page exit/);
      assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
      const media = await playback();
      assert.deepEqual([media.width, media.height], [testCase.expectedWidth, testCase.expectedHeight]);
      evidence.cases.push({ ...testCase, profile, ready, summary, media });
      save();
    }
  }
  evidence.opfsBeforePageExit = await evaluate(`(async()=>{const root=await navigator.storage.getDirectory();const names=[];for await(const name of root.keys())names.push(name);return names.filter(name=>name.startsWith("diaxus-")&&name.endsWith(".partial"));})()`);
  assert.equal(evidence.opfsBeforePageExit.length, evidence.opfsBaseline.length + 1);
  await evaluate('dispatchEvent(new PageTransitionEvent("pagehide",{persisted:false}))');
  evidence.opfsAfterPageExit = await waitFor(`(async()=>{const root=await navigator.storage.getDirectory();const names=[];for await(const name of root.keys())names.push(name);const current=names.filter(name=>name.startsWith("diaxus-")&&name.endsWith(".partial")).sort();return JSON.stringify(current)===${JSON.stringify(JSON.stringify(evidence.opfsBaseline))}&&current;})()`);
  assert.deepEqual(evidence.opfsAfterPageExit.sort(), evidence.opfsBaseline);
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
  evidence.invalidUpscale = await waitFor(`${statusExpression}.startsWith("FAILED:") && ${statusExpression}.includes("would upscale") && ${statusExpression}`);
  assert.equal(await evaluate('!document.querySelector("#resolved-size")'), true);

  await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [path.resolve("fixtures/m36-h265-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
  const hevcInputProbe = await waitFor(`(${statusExpression}.includes("Output resolves to 160×90") || ${statusExpression}.startsWith("FAILED:")) && ${statusExpression}`);
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
  await send("DOM.setFileInputFiles", { nodeId: input.nodeId, files: [largeInput] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await evaluate(`{const s=document.querySelector("#resize-preset");s.value="percent-25";s.dispatchEvent(new Event("change",{bubbles:true}));}`);
  await waitFor(`${statusExpression}.includes("Output resolves to 160×90") && document.querySelector("#resolved-size")?.textContent.includes("160×90")`);
  await selectProfile("webm-vp8-video-only");
  await click("#convert");
  const largeSummary = await terminal();
  assert.match(largeSummary, /^PASS: 60 H\.264 input frames/);
  assert.match(largeSummary, /Output storage: bounded OPFS stream/);
  assert.doesNotMatch(largeSummary, /memory fallback/);
  evidence.largeInput = { bytes: fs.statSync(largeInput).size, summary: largeSummary };
  assert.ok(evidence.largeInput.bytes > 256 * 1024 * 1024);

  evidence.opfsBeforeNavigation = await evaluate(`(async()=>{const root=await navigator.storage.getDirectory();const names=[];for await(const name of root.keys())names.push(name);return names.filter(name=>name.startsWith("diaxus-")&&name.endsWith(".partial"));})()`);
  assert.equal(evidence.opfsBeforeNavigation.length, evidence.opfsBaseline.length + 1);
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?cleanup-inspection=1` });
  await waitFor('document.readyState==="complete"');
  evidence.opfsAfterNavigation = await waitFor(`(async()=>{const root=await navigator.storage.getDirectory();const names=[];for await(const name of root.keys())names.push(name);const current=names.filter(name=>name.startsWith("diaxus-")&&name.endsWith(".partial")).sort();return JSON.stringify(current)===${JSON.stringify(JSON.stringify(evidence.opfsBaseline))}&&current;})()`);
  assert.deepEqual(evidence.opfsAfterNavigation, evidence.opfsBaseline);

  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full&output=memory` });
  await waitFor('!!document.querySelector("#convert")');
  await new Promise(resolve => setTimeout(resolve, 250));
  await evaluate('{const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
  const fallbackDocument = await send("DOM.getDocument");
  const fallbackInput = await send("DOM.querySelector", { nodeId: fallbackDocument.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: fallbackInput.nodeId, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor(`${statusExpression}.includes("Output resolves to 320×180") && document.querySelector("#resolved-size")?.textContent.includes("320×180")`);
  await selectProfile("webm-vp8-video-only");
  await click("#convert");
  const fallbackSummary = await terminal();
  assert.match(fallbackSummary, /^PASS: 60 H\.264 input frames/);
  assert.match(fallbackSummary, /Output storage: memory fallback/);
  assert.match(fallbackSummary, /memory compatibility mode was explicitly requested/);

  await send("DOM.setFileInputFiles", { nodeId: fallbackInput.nodeId, files: [largeInput] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor(`${statusExpression}.startsWith("Inspecting input")`);
  await waitFor(`${statusExpression}.includes("Output resolves to 320×180")`);
  await click("#convert");
  const fallbackRejection = await terminal();
  assert.match(fallbackRejection, /^FAILED:/);
  assert.match(fallbackRejection, /memory fallback accepts at most 256 MiB inputs/);
  evidence.memoryFallback = { summary: fallbackSummary, largeInputRejection: fallbackRejection };

  // Exercise the real automatic fallback branch: the primary API is absent in
  // this execution context and no output-mode test switch is supplied.
  await send("Page.navigate", { url: `http://127.0.0.1:${appPort}/?verify=full&execution=main` });
  await waitFor('!!document.querySelector("#convert")');
  await evaluate('Object.defineProperty(navigator.storage,"getDirectory",{configurable:true,value:undefined})');
  await evaluate('{const s=document.querySelector("#resize-preset");s.value="percent-50";s.dispatchEvent(new Event("change",{bubbles:true}));}');
  const automaticFallbackDocument = await send("DOM.getDocument");
  const automaticFallbackInput = await send("DOM.querySelector", { nodeId: automaticFallbackDocument.root.nodeId, selector: "#source-file" });
  await send("DOM.setFileInputFiles", { nodeId: automaticFallbackInput.nodeId, files: [path.resolve("fixtures/m2-h264-aac.mp4")] });
  await evaluate('{const i=document.querySelector("#source-file");i.dispatchEvent(new Event("input",{bubbles:true}));i.dispatchEvent(new Event("change",{bubbles:true}));}');
  await waitFor(`${statusExpression}.includes("Output resolves to 320×180")`);
  await selectProfile("webm-vp8-video-only");
  await click("#convert");
  evidence.automaticFallback = await terminal();
  assert.match(evidence.automaticFallback, /^PASS:/);
  assert.match(evidence.automaticFallback, /Output storage: memory fallback/);
  assert.match(evidence.automaticFallback, /origin-private file storage is unavailable/);

  evidence.boundaries = await evaluate(`(async()=>{
    const adapter=await navigator.gpu.requestAdapter();
    const dimensions=[8192,16384,32768,65536];
    const encoder=[];
    for(const dimension of dimensions){
      try{const result=await VideoEncoder.isConfigSupported({codec:"vp8",width:dimension,height:dimension,framerate:30,bitrate:2_000_000});encoder.push({dimension,supported:result.supported});}
      catch(error){encoder.push({dimension,supported:false,error:String(error)});}
    }
    return {gpuMaxTextureDimension2D:adapter.limits.maxTextureDimension2D,encoder};
  })()`);
  assert.ok(Number.isInteger(evidence.boundaries.gpuMaxTextureDimension2D));
  assert.ok(evidence.boundaries.encoder.some(item => !item.supported), "Expected an exact VP8 encoder dimension boundary");

  evidence.profiles = expectedProfiles;
  evidence.gpu = await evaluate('document.querySelector("#selected-gpu").textContent');
  evidence.execution = await evaluate('document.querySelector("#execution-context").textContent');
  evidence.completed = true;
  save();
  console.log(`PASS M3.6 resize/streaming Chromium: ${cases.length} size modes × ${expectedProfiles.length} profiles, >256 MiB sparse MP4, cancellation, invalid-upscale rejection; ${evidence.gpu}`);
} catch (error) {
  evidence.error = error.stack;
  save();
  throw error;
} finally {
  if (targetId) await send("Target.closeTarget", { targetId }, true).catch(() => {});
  socket.close();
}
