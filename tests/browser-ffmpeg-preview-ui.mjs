// Real UI regression through the supported tab-scoped Browser CDP capability.
// The caller supplies navigation/file-chooser adapters; no browser is launched here.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

export async function runFfmpegPreviewUi({send, navigate, selectFile, directory, context = 'worker'}) {
  fs.mkdirSync(directory, {recursive: true});
  const evidence = {status: 'running', context, cases: []};
  const evaluate = async expression => {
    const result = await send('Runtime.evaluate', {expression, awaitPromise: true, returnByValue: true});
    assert.ok(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const wait = async expression => {
    const deadline = Date.now() + 45000;
    while (!await evaluate(expression)) {
      assert.ok(Date.now() < deadline, `Timeout: ${expression}`);
      await new Promise(resolve => setTimeout(resolve, 50));
    }
  };
  try {
    await navigate(`http://127.0.0.1:8084/?backend=ffmpeg-wasm${context === 'main' ? '&execution=main' : ''}`);
    await wait("!!document.querySelector('#source-file')");
    await selectFile(path.resolve('fixtures/m35-vfr-offset.mp4'));
    await wait("/cached frames=1/.test(document.querySelector('#source-preview-status')?.textContent)");
    assert.equal(await evaluate("document.querySelector('#convert').disabled"), true);
    assert.equal(await evaluate("document.querySelector('#source-preview-play').disabled"), false);
    assert.match(await evaluate("document.querySelector('#status').textContent"), /FFmpeg WASM spike unavailable/);
    assert.equal(await evaluate("getComputedStyle(document.querySelector('.preview-panel')).visibility"), 'visible');
    evidence.cases.push('VFR Original export rejected, paused source preview available');

    await evaluate("document.querySelector('#clahe-enable').click()");
    await wait("/CLAHE paused preroll:/.test(document.querySelector('#source-preview-status').textContent)");
    assert.equal(await evaluate("document.querySelector('#convert').disabled"), true);
    evidence.cases.push('CLAHE preview works independently of rejected FFmpeg export');
    await evaluate("document.querySelector('#color-reset').click()");
    await wait("!/CLAHE paused preroll:/.test(document.querySelector('#source-preview-status').textContent)");

    // One intentional test-only output readback, never production conversion.
    if (context === 'worker') {
      const nonblack = await evaluate(`(async () => {
        const frame = new VideoFrame(document.querySelector('#worker-preview canvas'), {timestamp: 0});
        try { const pixels = new Uint8Array(frame.allocationSize({format: 'RGBA'}));
          await frame.copyTo(pixels, {format: 'RGBA'});
          return pixels.some((value, index) => index % 4 !== 3 && value > 30);
        } finally { frame.close(); }
      })()`);
      assert.equal(nonblack, true);
      evidence.cases.push('worker display contains nonblack decoded fixture pixels (test-only readback)');
    }
    await evaluate("document.querySelector('#source-preview-play').click()");
    await wait("document.querySelector('#source-preview-play').textContent.trim() === 'Pause' && Number(document.querySelector('#source-preview-position').value) > 0");
    await evaluate("document.querySelector('#source-preview-play').click()");
    await wait("document.querySelector('#source-preview-play').textContent.trim() === 'Play' && /Paused source preview:/.test(document.querySelector('#source-preview-status').textContent)");
    evidence.cases.push('play/pause works while export remains unavailable');

    await evaluate("document.querySelector('#output-fps').value = '30'; document.querySelector('#output-fps').dispatchEvent(new Event('change', {bubbles: true}))");
    await wait("!document.querySelector('#convert').disabled && !document.querySelector('#source-preview-play').disabled");
    evidence.cases.push('fixed-FPS export becomes available without removing preview');
    await evaluate("document.querySelector('#output-fps').value = 'original'; document.querySelector('#output-fps').dispatchEvent(new Event('change', {bubbles: true}))");
    await wait("document.querySelector('#convert').disabled && !document.querySelector('#source-preview-play').disabled && /cached frames=1/.test(document.querySelector('#source-preview-status').textContent)");
    evidence.cases.push('returning to rejected Original FPS keeps preview available');
    evidence.status = 'passed';
  } catch (error) {
    evidence.status = 'failed'; evidence.error = String(error); throw error;
  } finally {
    fs.writeFileSync(path.join(directory, 'evidence.json'), JSON.stringify(evidence, null, 2));
  }
  return evidence;
}
