// Real built Dioxus UI and CacheStorage. CDN mirrors preserve exact pinned bytes;
// no inference, media upload, or external model/runtime download is required.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import {pathToFileURL} from 'node:url';
const [rootArg,publicArg,outputArg]=process.argv.slice(2);
if(!outputArg)throw Error('Expected evaluation root, built public folder and fresh output folder');
const root=path.resolve(rootArg),publicDir=path.resolve(publicArg),output=path.resolve(outputArg);
fs.mkdirSync(output,{recursive:false});
const {chromium}=await import(pathToFileURL(path.join(root,'browser/node_modules/playwright/index.mjs')));
const manifest=JSON.parse(fs.readFileSync('crates/media-core/src/ai-upscale-model.json','utf8'));
const server=http.createServer((req,res)=>{
  const relative=decodeURIComponent(new URL(req.url,'http://localhost').pathname).replace(/^\/+/, '')||'index.html';
  const file=path.resolve(publicDir,relative);
  if(!file.startsWith(publicDir+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404);res.end();return;}
  res.setHeader('Cross-Origin-Opener-Policy','same-origin');res.setHeader('Cross-Origin-Embedder-Policy','require-corp');
  res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':/\.m?js$/.test(file)?'text/javascript':file.endsWith('.css')?'text/css; charset=utf-8':'text/html; charset=utf-8');
  fs.createReadStream(file).pipe(res);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let browser;
const report={status:'running',contexts:[]};
try{
  browser=await chromium.launch({executablePath:process.env.AI_UPSCALE_BROWSER_EXE??'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    headless:true,args:['--enable-unsafe-webgpu','--use-angle=d3d11']});
  report.browser=browser.version();
  for(const mode of ['main','worker']){
    const context=await browser.newContext({viewport:{width:1280,height:800}});
    const requests=[],errors=[];let delayRuntime=false;
    await context.route('https://cdn.jsdelivr.net/npm/onnxruntime-web@1.30.0/dist/**',async route=>{
      const asset=manifest.browser_runtime.assets.find(item=>item.url===route.request().url());
      assert.ok(asset,'Unpinned runtime request');
      if(delayRuntime)await new Promise(resolve=>setTimeout(resolve,300));
      try{await route.fulfill({body:fs.readFileSync(path.join(root,'ort-1.30/node_modules/onnxruntime-web/dist',asset.filename)),
        contentType:asset.filename.endsWith('.wasm')?'application/wasm':'text/javascript',headers:{'Access-Control-Allow-Origin':'*','Cross-Origin-Resource-Policy':'cross-origin'}});}catch(error){if(!route.request().failure())throw error;}
    });
    context.on('request',request=>requests.push({url:request.url(),method:request.method()}));
    const page=await context.newPage();page.on('pageerror',error=>errors.push(String(error)));
    await page.goto(`http://127.0.0.1:${server.address().port}/${mode==='main'?'?execution=main':''}`);
    await page.locator('#ai-expand').waitFor();
    assert.equal(await page.locator('#ai-settings').count(),0);
    assert.deepEqual(await page.evaluate(()=>caches.keys()),[]);
    assert.equal(requests.filter(r=>r.url.includes('onnxruntime-web@')).length,0);
    await page.locator('#ai-expand').click();
    assert.match(await page.locator('#ai-availability').innerText(),/acceptance.*incomplete/);
    assert.match(await page.locator('#ai-2x').innerText(),/^2\u00d7/);
    assert.doesNotMatch(await page.locator('#ai-settings').innerText(),/[\u00c3\u00c2\u0623\u0622]|\u00e2[\u20ac\u02c6\u2020]/);
    assert.equal(await page.locator('#ai-2x').isDisabled(),true);
    assert.equal(await page.locator('#ai-4x').isDisabled(),true);
    assert.doesNotMatch(await page.locator('#ai-settings').innerText(),/RTX/);
    // Removing disabled in DevTools cannot dispatch an unavailable AI job.
    await page.locator('#ai-2x').evaluate(button=>{button.disabled=false;button.click();});
    await page.locator('#ai-4x').evaluate(button=>{button.disabled=false;button.click();});
    assert.equal(await page.locator('#ai-off').getAttribute('aria-pressed'),'true');
    await page.locator('#resize-preset').selectOption('percent-75');
    await page.locator('#ai-off').click();
    assert.equal(await page.locator('#resize-preset').inputValue(),'percent-75','AI Off changed retained ordinary resize');
    assert.equal(await page.locator('#ai-off').getAttribute('aria-pressed'),'true');
    await page.locator('#ai-check-model').click();
    await page.waitForFunction(()=>/Setup failed:/.test(document.querySelector('#ai-setup-status').textContent));
    await page.locator('#ai-model-file').setInputFiles({name:'truncated.onnx',mimeType:'application/octet-stream',buffer:Buffer.alloc(100)});
    await page.locator('#ai-import-model').click();
    await page.waitForFunction(()=>/size mismatch/.test(document.querySelector('#ai-setup-status').textContent));
    await page.locator('#ai-model-file').setInputFiles(path.join(root,'reference/model.onnx'));
    await page.locator('#ai-import-model').click();
    await page.waitForFunction(()=>/Model hash verified/.test(document.querySelector('#ai-setup-status').textContent));
    const corrupt=fs.readFileSync(path.join(root,'reference/model.onnx'));corrupt[0]^=1;
    await page.locator('#ai-model-file').setInputFiles({name:'corrupt.onnx',mimeType:'application/octet-stream',buffer:corrupt});
    await page.locator('#ai-import-model').click();
    await page.waitForFunction(()=>/checksum mismatch/.test(document.querySelector('#ai-setup-status').textContent));
    await page.locator('#ai-model-file').setInputFiles(path.join(root,'reference/model.onnx'));

    await page.locator('#ai-check-model').click();
    await page.waitForFunction(()=>/Stored model hash verified/.test(document.querySelector('#ai-setup-status').textContent));
    delayRuntime=true;
    const started=page.waitForRequest(request=>request.url().includes('onnxruntime-web@'));
    await page.locator('#ai-acquire-runtime').click();
    await started;
    await page.locator('#ai-cancel-setup').click();
    await page.waitForFunction(()=>/CANCELLED/.test(document.querySelector('#ai-setup-status').textContent));
    assert.equal(await page.evaluate(async()=>(await(await caches.open('diaxus-ai-runtime-ort-web-1.30.0')).keys()).length),0);
    delayRuntime=false;
    await page.locator('#ai-acquire-runtime').click();
    await page.waitForFunction(()=>/Runtime Web 1.30.0 verified/.test(document.querySelector('#ai-setup-status').textContent),null,{timeout:60000});
    assert.equal(requests.filter(r=>r.url.includes('onnxruntime-web@')).length,manifest.browser_runtime.assets.length+1);
    assert.equal(await page.locator('#ai-off').getAttribute('aria-pressed'),'true');
    await page.screenshot({path:path.join(output,`${mode}-setup.png`),fullPage:true});
    await page.setViewportSize({width:375,height:812});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
    await page.screenshot({path:path.join(output,`${mode}-setup-mobile.png`),fullPage:true});
    await page.locator('#ai-remove-model').click();
    await page.waitForFunction(()=>/Stored model removed/.test(document.querySelector('#ai-setup-status').textContent));
    await page.locator('#ai-remove-runtime').click();
    await page.waitForFunction(()=>/Stored runtime removed/.test(document.querySelector('#ai-setup-status').textContent));
    assert.equal(await page.evaluate(async()=>{let n=0;for(const name of await caches.keys())n+=(await(await caches.open(name)).keys()).length;return n;}),0);
    assert.ok(requests.every(r=>r.method==='GET'),'Unexpected upload/write request');
    assert.deepEqual(errors,[]);
    report.contexts.push({mode,requests,errors,status:'passed',cases:['collapsed Off startup without cache/download',
      'both unavailable scales reject DOM bypass','AI Off retains ordinary resize','browser never offers RTX','UTF-8 and narrow layout','missing/truncated recovery',
      'exact local model import/check','full-size checksum rejection preserves valid cache','cancelled runtime acquisition leaves no partial assets','explicit pinned runtime acquisition','explicit model/runtime removal']});
    await context.close();
  }
  report.status='passed';
}catch(error){report.status='failed';report.error=String(error);throw error;}
finally{fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2));await browser?.close();server.close();}
console.log(JSON.stringify({status:report.status,contexts:report.contexts.length}));
