// Acceptance of ordinary app handlers with the optional ONNX engine compiled.
// Only transport results are observed; capability/settings/backend code is intact.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
const [rootArg,publicArg,outputArg]=process.argv.slice(2);
if(!outputArg)throw Error('Expected evaluation root, built public directory, fresh output');
const root=path.resolve(rootArg),publicDir=path.resolve(publicArg),output=path.resolve(outputArg);
fs.mkdirSync(output,{recursive:false});
const {chromium}=await import(pathToFileURL(path.join(root,'browser/node_modules/playwright/index.mjs')));
const manifest=JSON.parse(fs.readFileSync('crates/media-core/src/ai-upscale-model.json','utf8'));
const server=http.createServer((req,res)=>{
  const relative=decodeURIComponent(new URL(req.url,'http://localhost').pathname).replace(/^\/+/, '')||'index.html';
  const file=path.resolve(publicDir,relative);
  if(!file.startsWith(publicDir+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404);res.end();return;}
  res.setHeader('Cross-Origin-Opener-Policy','same-origin');res.setHeader('Cross-Origin-Embedder-Policy','require-corp');
  res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':/\.m?js$/.test(file)?'text/javascript; charset=utf-8':file.endsWith('.css')?'text/css':'text/html; charset=utf-8');
  if(path.basename(file)==='ai-inference.js'){
    const s=fs.readFileSync(file,'utf8');
    res.end(s.replace('export async function createAiSession(', 'async function actualCreateAiSession(')+`\nexport async function createAiSession(...args){console.info('AI UI session create started');try{const owner=await actualCreateAiSession(...args);console.info('AI UI session create finished');return owner;}catch(e){console.error('AI UI session create failed',String(e));throw e;}}`);
  }else if(path.basename(file)==='worker-host.js'){
    const s=fs.readFileSync(file,'utf8');assert.equal(s.split('return task;').length,2);
    res.end(s.replace('return task;',`return task.then(result=>{
      if(operation==='convert')globalThis.__aiUiConversion={...result};
      return result;
    });`));
  }else fs.createReadStream(file).pipe(res);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let browser;
const report={status:'running',scope:'Ordinary UI and backend commands; optional engine acceptance build; no diagnostic command substitution',contexts:[]};
try{
  const browserArgs=JSON.parse(process.env.AI_UI_BROWSER_ARGS??'[]');
  assert.ok(Array.isArray(browserArgs)&&browserArgs.every(a=>typeof a==='string'));
  browser=await chromium.launch({executablePath:'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',headless:process.env.AI_UI_HEADED!=='1',
    args:[...(process.env.AI_UI_HEADED==='1'?[]:['--enable-unsafe-webgpu']),'--use-angle=d3d11','--autoplay-policy=no-user-gesture-required',...browserArgs]});
  report.launch={headless:process.env.AI_UI_HEADED!=='1',extraArgs:browserArgs};
  report.browser=browser.version();
  for(const mode of ['main','worker'])for(const theme of ['light','dark']){
    if(JSON.parse(process.env.AI_UI_SKIP??'[]').includes(`${mode}-${theme}`))continue;
    const context=await browser.newContext({colorScheme:theme}),page=await context.newPage(),errors=[],cases=[],logs=[];
    page.on('console',m=>{logs.push(m.text());if(logs.length>100)logs.shift();});
    page.on('pageerror',error=>errors.push(String(error)));
    await page.route('https://cdn.jsdelivr.net/**',async route=>{
      const name=path.basename(new URL(route.request().url()).pathname);
      assert.ok(manifest.browser_runtime.assets.some(a=>a.filename===name),name);
      await route.fulfill({body:fs.readFileSync(path.join(root,'ort-1.30/node_modules/onnxruntime-web/dist',name)),
        headers:{'Content-Type':name.endsWith('.wasm')?'application/wasm':'text/javascript','Access-Control-Allow-Origin':'*','Cross-Origin-Resource-Policy':'cross-origin'}});
    });
    const value=()=>page.evaluate(()=>({status:document.querySelector('#status')?.textContent,
      preview:document.querySelector('#source-preview-status')?.textContent,error:document.querySelector('#source-preview-error')?.textContent,
      position:Number(document.querySelector('#source-preview-position')?.value),play:document.querySelector('#source-preview-play')?.textContent,
      size:document.querySelector('#resolved-size')?.textContent,estimate:document.querySelector('#output-estimate')?.textContent,
      gpu:document.querySelector('#selected-gpu')?.textContent}));
    const wait=async predicate=>{
      const end=Date.now()+90000;let s;
      do{s=await value();if(predicate(s))return s;assert.ok(!logs.some(l=>/panicked at|RuntimeError: unreachable/.test(l)),JSON.stringify(logs));fs.writeFileSync(path.join(output,'progress.json'),JSON.stringify({mode,theme,state:s,logs},null,2));assert.ok(Date.now()<end,JSON.stringify({s,logs}));await page.waitForTimeout(50);}while(true);
    };
    const record=async name=>cases.push({name,...await value()});
    const input=async(id,v)=>page.locator(id).evaluate((n,v)=>{n.value=String(v);n.dispatchEvent(new Event('input',{bubbles:true}));},v);
    const fixture=path.resolve(root,'quality-temporal-20261025/tos-face.mp4');
    await page.goto(`http://127.0.0.1:${server.address().port}/?verify=full&output=memory${mode==='main'?'&execution=main':''}`);
    await page.locator('#source-file').setInputFiles(fixture);
    await wait(s=>/Paused source preview/.test(s.preview));
    await page.locator('#output-profile').selectOption('mp4-h264-aac');
    await page.locator('#resize-preset').selectOption('percent-50');
    await wait(s=>/96×40/.test(s.status));
    await page.locator('#ai-expand').click();
    await page.locator('#ai-model-file').setInputFiles(path.join(root,'reference/model.onnx'));
    await page.locator('#ai-import-model').click();
    await page.waitForFunction(()=>/hash verified and stored/.test(document.querySelector('#ai-setup-status').textContent));
    await page.locator('#ai-acquire-runtime').click();
    await page.waitForFunction(()=>/verified and cached/.test(document.querySelector('#ai-setup-status').textContent));
    assert.equal(await page.locator('#ai-2x').isDisabled(),false);
    assert.equal(await page.locator('#ai-4x').isDisabled(),false);
    await page.locator('.color-controls > summary').click();
    const evidence={status:'ordinary-ai-ui-acceptance-passed',gpuErrors:[],cases:[],artifacts:[]};
    for(const scale of ['2x','4x']){
      const width=scale==='2x'?384:768,height=scale==='2x'?160:320;
      await page.locator(`#ai-${scale}`).click();
      await wait(s=>new RegExp(`${width}×${height}`).test(s.status)&&/Paused source preview/.test(s.preview));
      assert.equal(await page.locator('#resize-preset').isDisabled(),true);
      if(await page.locator('#clahe-enable').getAttribute('aria-pressed')!=='true')await page.locator('#clahe-enable').click();
      await wait(s=>s.preview.includes(`statistics/adjustments ${width}×${height}`));
      await input('#color-brightness',10);await input('#color-contrast',115);await input('#color-saturation',80);
      await page.locator('#color-compare').click();
      await wait(s=>!/CLAHE paused preroll/.test(s.preview));
      assert.equal(await page.locator(`#ai-${scale}`).getAttribute('aria-pressed'),'true');
      await page.locator('#color-compare').click();
      await wait(s=>s.preview.includes(`statistics/adjustments ${width}×${height}`));
      await page.locator('#color-reset').click();
      assert.equal(await page.locator(`#ai-${scale}`).getAttribute('aria-pressed'),'true');
      assert.equal(await page.locator('#clahe-enable').getAttribute('aria-pressed'),'false');
      await record(`${scale} controls, geometry/estimates, Before and Reset retain AI`);
      await page.locator('#source-preview-play').click();
      await wait(s=>s.play==='Pause'&&s.position>.25&&/Playing sampled source preview/.test(s.preview));
      await input('#source-preview-position',1);
      await wait(s=>s.play==='Pause'&&s.position>=1&&!s.error);
      await page.locator('#source-preview-play').click();
      await wait(s=>s.play==='Play'&&/Paused source preview/.test(s.preview));
      await record(`${scale} actual AI Play/Pause and seek recover without starvation`);
      await page.locator('#clahe-enable').click();
      await input('#color-brightness',10);await input('#color-contrast',115);await input('#color-saturation',80);
      await wait(s=>s.preview.includes(`statistics/adjustments ${width}×${height}`));
      await page.screenshot({path:path.join(output,`${mode}-${theme}-${scale}-on.png`),fullPage:true});
      if((await value()).gpu.includes('nvidia / ampere')){
        await page.locator('#output-profile').selectOption('webm-vp8-opus');
        await wait(s=>/NVIDIA AI WebM endpoint verification is pending/.test(s.status));
        assert.equal(await page.locator('#convert').isDisabled(),true);
        assert.equal(await page.locator('#source-preview-play').isDisabled(),false);
        await record(`${scale} unaccepted NVIDIA WebM export rejected with preview available`);
        await page.locator('#output-profile').selectOption('mp4-h264-aac');
        await wait(s=>/Ready\./.test(s.status)&&new RegExp(`${width}×${height}`).test(s.status));
      }
      if(process.env.AI_UI_PREVIEW_ONLY==='1'){await record(`${scale} preview-only controls acceptance`);continue;}
      await page.locator('#convert').click();
      await wait(s=>/PASS:/.test(s.status)&&/64/.test(s.status)&&s.play==='Play');
      const conversion=await page.evaluate(()=>globalThis.__aiUiConversion);
      assert.equal(conversion.aiSourceFrames,64);assert.equal(conversion.inputFrameCount,64);
      const bytes=await page.locator('#download').evaluate(async n=>Array.from(new Uint8Array(await(await fetch(n.href)).arrayBuffer())));
      const name=`video-${mode}-${theme}-${scale}.mp4`;fs.writeFileSync(path.join(output,name),Buffer.from(bytes));
      evidence.artifacts.push({name,bytes:bytes.length});
      evidence.cases.push({request:{fixture,scale,fps:'original',clahe:true,theme},...conversion});
      await record(`${scale} ordinary verified 64-frame AI export and paused handoff return`);
    }
    await page.locator('#ai-off').click();
    await wait(s=>/96×40/.test(s.status)&&s.preview.includes('statistics/adjustments 96×40'));
    assert.equal(await page.locator('#resize-preset').inputValue(),'percent-50');
    assert.equal(await page.locator('#resize-preset').isDisabled(),false);
    await record('Off restores retained ordinary resize');
    if(process.env.AI_UI_PREVIEW_ONLY==='1'){
      await page.locator('#ai-remove-model').click();
      await page.waitForFunction(()=>/Stored model removed/.test(document.querySelector('#ai-setup-status').textContent));
      await page.locator('#ai-2x').click();
      await wait(s=>/Source preview unavailable/.test(s.preview)&&/model/i.test(s.preview));
      await page.locator('#source-preview-play').click();
      await wait(s=>s.play==='Play'&&/model/i.test(s.error??''));
      await record('missing model fails explicitly and stops its clock');
      await page.locator('#ai-import-model').click();
      await page.waitForFunction(()=>/hash verified and stored/.test(document.querySelector('#ai-setup-status').textContent));
      await page.locator('#ai-2x').click();
      await wait(s=>/Paused source preview/.test(s.preview)&&!s.error);
      await page.locator('#source-preview-play').click();
      await wait(s=>s.play==='Pause'&&/Playing sampled/.test(s.preview));
      await page.locator('#source-preview-play').click();
      await wait(s=>s.play==='Play'&&/Paused source preview/.test(s.preview));
      await record('explicit asset repair and selection retry restore preview without automatic playback');
      await page.locator('#ai-off').click();
      await wait(s=>/96×40/.test(s.status)&&s.preview.includes('statistics/adjustments 96×40'));
      await page.locator('#source-file').setInputFiles(path.resolve('fixtures/m2-h264-aac.mp4'));
      await wait(s=>/320×180/.test(s.status)&&/Paused source preview/.test(s.preview));
      assert.equal(await page.locator('#ai-2x').isDisabled(),true);
      assert.equal(await page.locator('#ai-4x').isDisabled(),true);
      assert.match(await page.locator('#ai-availability').textContent(),/limits/);
      await record('over-envelope source disables both scales while ordinary preview remains usable');
    }
    await page.screenshot({path:path.join(output,`${mode}-${theme}.png`),fullPage:true});
    assert.deepEqual(errors,[]);
    evidence.pageErrors=errors;
    const key=`${mode}-${theme}`;fs.writeFileSync(path.join(output,`${key}.json`),JSON.stringify(evidence,null,2));
    report.contexts.push({mode,theme,status:'passed',cases,errors});
    fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2));
    await context.close();
  }
  report.status='passed';
}catch(error){report.status='failed';report.error=String(error);throw error;}
finally{fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2));await browser?.close();server.close();}
console.log(JSON.stringify({status:report.status,contexts:report.contexts.length}));
