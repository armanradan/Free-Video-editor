// Actual Chromium CacheStorage probe. Uses the existing isolated evaluation browser;
// no model/runtime downloads, inference, GPU creation or media uploads.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import {pathToFileURL} from 'node:url';
const [rootArg,outputArg]=process.argv.slice(2);
if(!outputArg)throw Error('Expected evaluation-root and new evidence-json path');
const root=path.resolve(rootArg);
process.env.PLAYWRIGHT_BROWSERS_PATH=path.join(root,'chromium');
const {chromium}=await import(pathToFileURL(path.join(root,'browser/node_modules/playwright/index.mjs')));
const manifest=JSON.parse(fs.readFileSync('crates/media-core/src/ai-upscale-model.json','utf8'));
const requests=[];
const methods=[];
const server=http.createServer((req,res)=>{
  requests.push(req.url);
  methods.push(req.method);
  if(req.url==='/'){res.setHeader('Content-Type','text/html');res.end('<title>AI cache diagnostic</title>');return;}
  const runtime=manifest.browser_runtime.assets.find(asset=>req.url===`/runtime/${asset.filename}`);
  const file=runtime?path.join(root,'ort-1.30/node_modules/onnxruntime-web/dist',runtime.filename):
    req.url==='/ai-assets.js'?'crates/media-web/src/ai-assets.js':req.url==='/model.onnx'?path.join(root,'reference/model.onnx'):null;
  if(!file){res.writeHead(404);res.end();return;}
  res.setHeader('Content-Type',/\.m?js$/.test(req.url)?'text/javascript':'application/octet-stream');
  fs.createReadStream(file).pipe(res);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let browser;
try{
  browser=await chromium.launch({channel:'chromium',headless:true});
  const context=await browser.newContext(),page=await context.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  const result=await page.evaluate(async manifest=>{
    const check=(condition,message)=>{if(!condition)throw Error(message);};
    const reject=async(action,expected)=>{try{await action();}catch(error){check(String(error).includes(expected),String(error));return;}throw Error(`Expected rejection: ${expected}`);};
    // Test mirrors preserve the pinned bytes/hashes; no external CDN is fetched.
    for(const asset of manifest.browser_runtime.assets)asset.url=new URL(`/runtime/${asset.filename}`,location.href).href;
    const module=await import('/ai-assets.js'),json=JSON.stringify(manifest);
    check((await caches.keys()).length===0,'Module import performed cache acquisition');
    const bytes=await(await fetch('/model.onnx')).arrayBuffer();
    const source=()=>new File([bytes],'model.onnx');
    const status=await module.importAiModel(source(),json,()=>false);
    const cached=await module.readAiModel(json);check(cached.byteLength===manifest.model.bytes,'Wrong cached length');
    const mutated=new Uint8Array(bytes.slice(0));mutated[0]^=1;
    await reject(()=>module.importAiModel(new File([mutated],'model.onnx'),json,()=>false),'checksum mismatch');
    await reject(()=>module.importAiModel(new File([bytes.slice(0,100)],'model.onnx'),json,()=>false),'size mismatch');
    await reject(()=>module.importAiModel(source(),JSON.stringify({...manifest,schema_version:999}),()=>false),'Incompatible AI model manifest');
    check((await module.readAiModel(json)).byteLength===bytes.byteLength,'Rejected input changed valid cache');
    const reused=await module.importAiModel(source(),json,()=>false);
    check(reused.includes('existing'),'Existing cache was not reused');
    await module.removeAiModel(json);
    await reject(()=>module.importAiModel(source(),json,()=>true),'CANCELLED');
    let checks=0;
    await reject(()=>module.importAiModel(source(),json,()=>++checks>=3),'CANCELLED');
    await reject(()=>module.readAiModel(json),'not cached');
    const originalPut=Cache.prototype.put;
    Cache.prototype.put=async()=>{throw new DOMException('injected quota exhaustion','QuotaExceededError');};
    try{await reject(()=>module.importAiModel(source(),json,()=>false),'quota exceeded');}
    finally{Cache.prototype.put=originalPut;}
    await reject(()=>module.readAiModel(json),'not cached');
    await module.importAiModel(source(),json,()=>false);
    const cache=await caches.open('diaxus-ai-model-v1');
    const key=new URL(`/__diaxus_ai__/models/${manifest.model.sha256}`,location.href).href;
    await cache.put(key,new Response(mutated));
    await reject(()=>module.readAiModel(json),'checksum mismatch');
    await module.removeAiModel(json);
    await module.importAiModel(source(),json,()=>false);
    await reject(()=>module.loadAiRuntime(json),'runtime is not cached');
    let cancelAfterPublish=false;
    const putBeforeCancel=Cache.prototype.put;
    Cache.prototype.put=async function(...args){await putBeforeCancel.apply(this,args);cancelAfterPublish=true;};
    try{await reject(()=>module.acquireAiRuntime(json,()=>cancelAfterPublish),'CANCELLED');}
    finally{Cache.prototype.put=putBeforeCancel;}
    check((await(await caches.open('diaxus-ai-runtime-ort-web-1.30.0')).keys()).length===0,'Cancelled runtime acquisition retained new entries');
    const runtimeStatus=await module.acquireAiRuntime(json,()=>false);
    check(runtimeStatus.includes('not loaded'),'Acquisition executed the runtime');
    globalThis.__cacheProbe={module,json,bytes};
    return {status:'cache-probe-passed-not-inference-acceptance',modelSha256:manifest.model.sha256,
      bytes:bytes.byteLength,importStatus:status,corruptInputRejected:true,truncatedInputRejected:true,
      corruptCacheRejected:true,incompatibleManifestRejected:true,explicitRemovalAndRecovery:true,cancelBeforeAndAfterHash:true,cancelledRuntimeEntriesRemoved:true,
      quotaError:'injected Cache.put error path plus separate actual CDP origin-quota test below',
      moduleImportCreatesNoCache:true,existingCacheReused:true,explicitRuntimeAcquisition:true,
      runtimeAssetsBytes:manifest.browser_runtime.assets.reduce((n,asset)=>n+asset.bytes,0)};
  },manifest);
  await context.setOffline(true);
  result.actualOfflineRead=await page.evaluate(async()=>{
    const {module,json}=globalThis.__cacheProbe;
    return (await module.readAiModel(json)).byteLength;
  });
  if(result.actualOfflineRead!==manifest.model.bytes)throw Error('Offline cache read failed');
  result.offlineVerifiedRuntimeModule=await page.evaluate(async()=>{
    const {module,json}=globalThis.__cacheProbe;
    const [ort,concurrent]=await Promise.all([module.loadAiRuntime(json),module.loadAiRuntime(json)]);
    if(ort!==concurrent)throw Error('Concurrent loading created separate runtime instances');
    return typeof ort.InferenceSession.create==='function';
  });
  if(!result.offlineVerifiedRuntimeModule)throw Error('Offline verified runtime import failed');
  result.realOfflineAcquisitionFailure=await page.evaluate(async()=>{
    const {module,json}=globalThis.__cacheProbe;
    await module.removeAiRuntime(json);
    try{await module.acquireAiRuntime(json,()=>false);}catch(error){
      if(!String(error).includes('explicitly retry'))throw error;
      const remaining=await(await caches.open('diaxus-ai-runtime-ort-web-1.30.0')).keys();
      if(remaining.length)throw Error('Failed acquisition retained new runtime entries');return true;
    }
    throw Error('Offline acquisition unexpectedly succeeded');
  });
  await context.setOffline(false);
  await page.evaluate(async()=>{const {module,json}=globalThis.__cacheProbe;await module.removeAiModel(json);});
  const cdp=await context.newCDPSession(page);
  const origin=`http://127.0.0.1:${server.address().port}`;
  await cdp.send('Storage.overrideQuotaForOrigin',{origin,quotaSize:1});
  try{
    result.realQuotaFailure=await page.evaluate(async()=>{
      const {module,json,bytes}=globalThis.__cacheProbe;
      try{await module.importAiModel(new File([bytes],'model.onnx'),json,()=>false);}catch(error){
        if(!String(error).includes('quota exceeded'))throw error;return true;
      }
      throw Error('One-byte quota override did not reject model publication');
    });
  }finally{await cdp.send('Storage.overrideQuotaForOrigin',{origin});await cdp.detach();}
  result.recoveredAfterQuota=await page.evaluate(async()=>{
    const {module,json,bytes}=globalThis.__cacheProbe;
    await module.importAiModel(new File([bytes],'model.onnx'),json,()=>false);
    return (await module.readAiModel(json)).byteLength;
  });
  result.requests=requests;result.httpMethods=[...new Set(methods)];result.browserVersion=browser.version();
  if(methods.some(method=>method!=='GET'))throw Error('Unexpected upload request');
  if(requests.some(url=>url.startsWith('https:')))throw Error('Unexpected external network request');
  fs.writeFileSync(outputArg,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify(result));
}finally{await browser?.close();server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
