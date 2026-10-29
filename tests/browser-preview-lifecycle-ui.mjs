// Built ordinary app, actual decode/GPU/clock in main and worker contexts.
// Only completion delivery is delayed (or rejected after real successful work)
// by the test server; production source contains no scheduling test hooks.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';

const [rootArg, publicArg, outputArg] = process.argv.slice(2);
if (!outputArg) throw Error('Expected evaluation root, built public directory, fresh output directory');
const root = path.resolve(rootArg), publicDir = path.resolve(publicArg), output = path.resolve(outputArg);
fs.mkdirSync(output, {recursive: false});
const {chromium} = await import(pathToFileURL(path.join(root, 'browser/node_modules/playwright/index.mjs')));
const hook = `return task.then(async result => {
  const test = globalThis.__previewLifecycle;
  if (test && operation === 'preview' && profile.split('@')[2] === '1') {
    test.completed++;
    if (test.arm) {
      test.arm = false; test.held = true;
      await new Promise(resolve => { test.release = resolve; });
      test.held = false;
      if (test.reject) { test.reject = false; throw Error('injected retired preview completion'); }
    }
  }
  return result;
});`;
let instrumented = 0;
const server = http.createServer((req, res) => {
  const relative = decodeURIComponent(new URL(req.url, 'http://localhost').pathname).replace(/^\/+/, '') || 'index.html';
  const file = path.resolve(publicDir, relative);
  if (!file.startsWith(publicDir + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
    res.writeHead(404); res.end(); return;
  }
  res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
  res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
  res.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : /\.m?js$/.test(file) ? 'text/javascript; charset=utf-8' : file.endsWith('.css') ? 'text/css' : 'text/html; charset=utf-8');
  if (path.basename(file) === 'worker-host.js') {
    const source = fs.readFileSync(file, 'utf8');
    assert.equal(source.split('return task;').length, 2, 'transport hook anchor changed');
    instrumented++;
    res.end(source.replace('return task;', hook));
  } else fs.createReadStream(file).pipe(res);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const report = {status: 'running', scope: 'AI Off ordinary UI; response delivery fault after actual successful decode/GPU work; no AI playback acceptance', contexts: []};
let browser;
try {
  browser = await chromium.launch({executablePath: process.env.AI_UPSCALE_BROWSER_EXE ?? 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe', headless: true,
    args: ['--enable-unsafe-webgpu', '--use-angle=d3d11', '--autoplay-policy=no-user-gesture-required']});
  report.browser = browser.version();
  for (const mode of ['main', 'worker']) {
    const context = await browser.newContext(), page = await context.newPage();
    const errors = [], cases = [];
    page.on('pageerror', error => errors.push(String(error)));
    const state = () => page.evaluate(() => ({
      button: document.querySelector('#source-preview-play')?.textContent,
      position: Number(document.querySelector('#source-preview-position')?.value),
      preview: document.querySelector('#source-preview-status')?.textContent,
      error: document.querySelector('#source-preview-error')?.textContent,
      status: document.querySelector('#status')?.textContent,
      completed: globalThis.__previewLifecycle?.completed,
      gpu: document.querySelector('#selected-gpu')?.textContent,
      execution: document.querySelector('#execution-context')?.textContent,
    }));
    const record = async name => { cases.push({name, ...await state()}); };
    const wait = async predicate => {
      const deadline = Date.now() + 45000;
      let current;
      do {
        current = await state();
        if (predicate(current)) return current;
        assert.ok(Date.now() < deadline, JSON.stringify(current));
        await page.waitForTimeout(50);
      } while (true);
    };
    await page.goto(`http://127.0.0.1:${server.address().port}/?verify=full&output=memory${mode === 'main' ? '&execution=main' : ''}`);
    await page.locator('#source-file').setInputFiles(path.resolve('tmp/preview-click-20261008.mp4'));
    await wait(s => /Paused source preview/.test(s.preview));
    await page.evaluate(() => { globalThis.__previewLifecycle = {arm: true, held: false, completed: 0}; });
    await page.locator('#source-preview-play').click();
    await page.waitForFunction(() => globalThis.__previewLifecycle.held);
    await page.locator('#source-preview-play').click();
    await page.locator('#source-preview-play').click();
    assert.equal((await state()).button, 'Pause');
    await page.evaluate(() => globalThis.__previewLifecycle.release());
    await wait(s => s.button === 'Pause' && s.completed >= 3 && s.position > .1 && /Playing source preview/.test(s.preview) && !s.error);
    await record('rapid Pause/Play retains fresh producer and clock after old successful completion');

    await page.evaluate(() => { globalThis.__previewLifecycle.arm = true; globalThis.__previewLifecycle.reject = true; });
    await page.waitForFunction(() => globalThis.__previewLifecycle.held);
    await page.locator('#source-preview-play').click();
    await page.locator('#source-preview-play').click();
    const before = await state();
    await page.evaluate(() => globalThis.__previewLifecycle.release());
    await wait(s => s.button === 'Pause' && s.completed >= before.completed + 2 && s.position > before.position && !s.error);
    await record('late old-producer error cannot pause resumed playback or publish an error');

    await page.evaluate(() => { globalThis.__previewLifecycle.arm = true; globalThis.__previewLifecycle.reject = true; });
    await page.waitForFunction(() => globalThis.__previewLifecycle.held);
    await page.evaluate(() => globalThis.__previewLifecycle.release());
    await wait(s => s.button === 'Play' && /injected retired preview completion/.test(s.error) && /Paused source preview/.test(s.preview));
    const stopped = await state();
    await page.waitForTimeout(150);
    assert.equal((await state()).position, stopped.position, 'current error did not pause its clock');
    await record('current-producer error pauses playback and preserves actionable error');
    await page.locator('#source-preview-play').click();
    await wait(s => s.button === 'Pause' && s.position > stopped.position && !s.error && /Playing source preview/.test(s.preview));
    await record('explicit Play clears the error and retries on the current source');

    await page.evaluate(() => {
      const slider=document.querySelector('#source-preview-position');
      for(const seconds of [4,8,2,6]){
        slider.value=String(seconds);slider.dispatchEvent(new Event('input',{bubbles:true}));
      }
    });
    await wait(s => s.button === 'Pause' && s.position >= 6 && !s.error &&
      Number(s.preview?.match(/frame time=([\d.]+)/)?.[1]??0) >= 5.95);
    await record('burst of explicit seeks keeps playback running and reaches latest requested source position');

    await page.evaluate(() => { globalThis.__previewLifecycle.arm = true; globalThis.__previewLifecycle.reject = true; });
    await page.waitForFunction(() => globalThis.__previewLifecycle.held);
    await page.locator('#source-file').setInputFiles(path.resolve('fixtures/m2-h264-aac.mp4'));
    await page.evaluate(() => globalThis.__previewLifecycle.release());
    await wait(s => s.button === 'Play' && s.position === 0 && /Paused source preview/.test(s.preview) && !s.error);
    await record('source replacement ignores old completion error and resets paused timeline');

    await page.locator('#source-preview-play').click();
    await wait(s => s.button === 'Pause' && s.position > .1);
    await page.evaluate(() => { globalThis.__previewLifecycle.arm = true; globalThis.__previewLifecycle.reject = true; });
    await page.waitForFunction(() => globalThis.__previewLifecycle.held);
    await page.locator('#convert').click();
    await page.evaluate(() => globalThis.__previewLifecycle.release());
    await wait(s => /PASS:/.test(s.status) && /Paused source preview/.test(s.preview) && s.button === 'Play');
    const converted = await state();
    assert.match(converted.status, /60 frames/);
    assert.ok(!converted.error, converted.error);
    await page.locator('a[download]').evaluate(async node => {
      const response = await fetch(node.href);
      globalThis.__convertedBytes = Array.from(new Uint8Array(await response.arrayBuffer()));
    });
    const bytes = await page.evaluate(() => globalThis.__convertedBytes);
    const downloadName = await page.locator('#download').getAttribute('download');
    const extension = path.extname(downloadName);
    assert.ok(['.mp4', '.webm'].includes(extension), downloadName);
    const outputName = `${mode}${extension}`, outputFile = path.join(output, outputName);
    fs.writeFileSync(outputFile, Buffer.from(bytes));
    const decoded = spawnSync('ffmpeg', ['-v', 'error', '-i', outputFile, '-map', '0:v:0', '-map', '0:a:0', '-f', 'null', '-'], {encoding: 'utf8', timeout: 60000});
    assert.equal(decoded.status, 0, decoded.stderr);
    const inspected = spawnSync('ffprobe', ['-v', 'error', '-count_frames', '-show_streams', '-of', 'json', outputFile], {encoding: 'utf8', timeout: 60000});
    assert.equal(inspected.status, 0, inspected.stderr);
    const streams = JSON.parse(inspected.stdout).streams;
    const video = streams.find(s => s.codec_type === 'video'), audio = streams.find(s => s.codec_type === 'audio');
    assert.equal(Number(video.nb_read_frames), 60);
    assert.equal(video.width, 640); assert.equal(video.height, 360); assert.ok(audio);
    await record('conversion handoff ignores retired preview error and returns to paused source');
    assert.deepEqual(await page.evaluate(() => caches.keys()), []);
    assert.deepEqual(errors, []);
    await page.screenshot({path: path.join(output, `${mode}.png`), fullPage: true});
    report.contexts.push({mode, status: 'passed', cases, errors, outputName, outputBytes: bytes.length, optionalCaches: [],
      independentDecode: {status: 'passed', videoCodec: video.codec_name, audioCodec: audio.codec_name, frames: Number(video.nb_read_frames), width: video.width, height: video.height}});
    fs.writeFileSync(path.join(output, 'report.json'), JSON.stringify(report, null, 2));
    await context.close();
  }
  assert.ok(instrumented >= 2);
  report.status = 'passed';
} catch (error) { report.status = 'failed'; report.error = String(error); throw error; }
finally {
  fs.writeFileSync(path.join(output, 'report.json'), JSON.stringify(report, null, 2));
  await browser?.close(); server.close();
}
console.log(JSON.stringify({status: report.status, contexts: report.contexts.length}));
