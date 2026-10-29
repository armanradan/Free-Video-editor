// Ordinary AI Off preview regression after introducing explicit AI snapshots.
// Reuses the existing decoded-preview/playback acceptance rather than mocking GPU IO.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
import {runFfmpegPreviewUi} from './browser-ffmpeg-preview-ui.mjs';
const [rootArg,publicArg,outputArg]=process.argv.slice(2);
if(!outputArg)throw Error('Expected evaluation root, built public directory, fresh output directory');
const root=path.resolve(rootArg),publicDir=path.resolve(publicArg),output=path.resolve(outputArg);
fs.mkdirSync(output,{recursive:false});
const {chromium}=await import(pathToFileURL(path.join(root,'browser/node_modules/playwright/index.mjs')));
const server=http.createServer((req,res)=>{
  const relative=decodeURIComponent(new URL(req.url,'http://localhost').pathname).replace(/^\/+/, '')||'index.html';
  const file=path.resolve(publicDir,relative);
  if(!file.startsWith(publicDir+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404);res.end();return;}
  res.setHeader('Cross-Origin-Opener-Policy','same-origin');res.setHeader('Cross-Origin-Embedder-Policy','require-corp');
  res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':/\.m?js$/.test(file)?'text/javascript; charset=utf-8':file.endsWith('.css')?'text/css; charset=utf-8':'text/html; charset=utf-8');
  fs.createReadStream(file).pipe(res);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let browser;
const report={status:'running',contexts:[]};
try{
  browser=await chromium.launch({executablePath:process.env.AI_UPSCALE_BROWSER_EXE??'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',headless:true,
    args:['--enable-unsafe-webgpu','--use-angle=d3d11','--autoplay-policy=no-user-gesture-required']});
  report.browser=browser.version();
  for(const mode of ['main','worker']){
    const context=await browser.newContext();const page=await context.newPage();const cdp=await context.newCDPSession(page);
    const errors=[];page.on('pageerror',error=>errors.push(String(error)));
    const result=await runFfmpegPreviewUi({send:(...args)=>cdp.send(...args),context:mode,directory:path.join(output,mode),
      navigate:url=>page.goto(url.replace('http://127.0.0.1:8084',`http://127.0.0.1:${server.address().port}`)),
      selectFile:async file=>{await page.locator('#source-file').setInputFiles(file);await page.locator('.color-controls > summary').click();}});
    await page.locator('#clahe-enable').click();
    await page.waitForFunction(()=>/CLAHE paused preroll:/.test(document.querySelector('#source-preview-status').textContent));
    await page.locator('#color-compare').click();
    await page.waitForFunction(()=>/Before/.test(document.querySelector('#color-compare').textContent)&&!/CLAHE paused preroll:/.test(document.querySelector('#source-preview-status').textContent));
    await page.locator('#color-compare').click();
    await page.waitForFunction(()=>/After/.test(document.querySelector('#color-compare').textContent)&&/CLAHE paused preroll:/.test(document.querySelector('#source-preview-status').textContent));
    assert.deepEqual(await page.evaluate(()=>caches.keys()),[],'AI Off acquired optional assets');
    assert.deepEqual(errors,[]);
    await page.screenshot({path:path.join(output,`${mode}-preview.png`),fullPage:true});
    report.contexts.push({mode,status:result.status,cases:[...result.cases,'Before/After request scope preserves saved CLAHE','AI Off optional caches remain empty'],errors});
    await context.close();
  }
  report.status='passed';
}catch(error){report.status='failed';report.error=String(error);throw error;}
finally{fs.writeFileSync(path.join(output,'report.json'),JSON.stringify(report,null,2));await browser?.close();server.close();}
console.log(JSON.stringify({status:report.status,contexts:report.contexts.length}));
