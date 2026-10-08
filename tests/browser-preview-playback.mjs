// Real-browser acceptance; inject the supported Browser runtime's tab CDP,
// genuine UI clicks, file chooser and navigation. No browser socket is opened.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

export async function runPlaybackAcceptance({ send, click, selectFile, navigate, url, directory }) {
  fs.mkdirSync(directory, { recursive: true });
  const evidence = { status: "running", url, cases: [] };
  const save = () => fs.writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
  const evaluate = async expression => {
    const result = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    assert.ok(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const snapshot = () => evaluate(`({position:Number(document.querySelector('#source-preview-position')?.value),
    button:document.querySelector('#source-preview-play')?.textContent,
    preview:document.querySelector('#source-preview-status')?.textContent,
    error:document.querySelector('#source-preview-error')?.textContent,
    status:document.querySelector('#status')?.textContent,
    gpu:document.querySelector('#selected-gpu')?.textContent})`);
  const wait = async predicate => {
    const end = Date.now() + 30000;
    let state;
    do {
      state = await snapshot();
      if (predicate(state)) return state;
      assert.ok(!/playback (failed|unavailable)/i.test(state.preview ?? ""), state.preview);
      assert.ok(!state.error, state.error);
      await new Promise(resolve => setTimeout(resolve, 50));
    } while (Date.now() < end);
    throw Error(`Playback acceptance timeout: ${JSON.stringify(state)}`);
  };
  const record = (name, state) => { evidence.cases.push({ name, ...state }); save(); };
  const renders = state => Number(state.preview?.match(/renders=(\d+)/)?.[1] ?? 0);
  const frameTime = state => Number(state.preview?.match(/frame time=([\d.-]+)/)?.[1] ?? NaN);
  const set = (id, value) => evaluate(`(()=>{const n=document.getElementById(${JSON.stringify(id)});
    n.value=${JSON.stringify(String(value))};n.dispatchEvent(new Event('input',{bubbles:true}));return true})()`);
  try {
    await navigate(url);
    await selectFile(path.resolve("tmp/preview-click-20261008.mp4"));
    await wait(s => s.button === "Play" && /Paused source preview/.test(s.preview));
    await click("#source-preview-play");
    const moving = await wait(s => s.button === "Pause" && s.position > .35 && /Playing source preview/.test(s.preview));
    assert.match(moving.preview, /stream opens=1;/);
    assert.match(moving.preview, /explicit pixel readbacks=0/);
    record("moving shared-GPU preview, one decoder stream", moving);
    await click(".preview-viewport");
    const paused = await wait(s => s.button === "Play" && /playback lookahead=0/.test(s.preview));
    await new Promise(resolve => setTimeout(resolve, 200));
    const still = await snapshot();
    assert.ok(Math.abs(still.position - paused.position) < .01, "paused timeline moved");
    record("image click pauses and retires lookahead", still);
    await set("source-preview-position", 4);
    record("paused seek retains requested timeline and rerenders", await wait(s => Math.abs(s.position - 4) < .01 && s.button === "Play" && renders(s) > renders(still) && Math.abs(frameTime(s) - 4) < .05));
    await click("#source-preview-play");
    await wait(s => s.button === "Pause" && s.position > 4.1);
    const beforeSeek = await snapshot();
    await set("source-preview-position", 8);
    await set("color-saturation", 0);
    record("playing seek with live color policy", await wait(s => s.button === "Pause" && s.position > 8.05 && renders(s) > renders(beforeSeek) && frameTime(s) >= 8 && Math.abs(frameTime(s) - s.position) < .2 && /Playing source preview/.test(s.preview)));
    await set("source-preview-position", 20.1);
    record("EOF pauses and retires decoder lookahead", await wait(s => s.button === "Play" && s.position > 19.9 && /Paused source preview/.test(s.preview) && /playback lookahead=0/.test(s.preview)));
    await click("#source-preview-play");
    record("restart from EOF", await wait(s => s.button === "Pause" && s.position > .1 && s.position < 2));
    await selectFile(path.resolve("fixtures/m2-h264-aac.mp4"));
    record("source replacement stops old playback", await wait(s => s.button === "Play" && s.position === 0 && /Paused source preview/.test(s.preview)));
    await click("#source-preview-play");
    await wait(s => s.button === "Pause" && s.position > .1);
    await click("#convert");
    const converted = await wait(s => /PASS:/.test(s.status) && /Paused source preview/.test(s.preview));
    assert.equal(converted.button, "Play");
    assert.match(converted.status, /60 frames/);
    assert.match(converted.status, /cleanup.*(?:0|zero)|live.*0/i);
    record("conversion owns GPU after playback stops", converted);
    evidence.status = "passed"; save();
    return evidence;
  } catch (error) { evidence.status = "failed"; evidence.error = String(error); save(); throw error; }
}
