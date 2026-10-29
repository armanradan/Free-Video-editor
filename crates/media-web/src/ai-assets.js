// Model provisioning only. Importing this module performs no cache/network work
// and imports no inference runtime. Rust supplies the pinned manifest, not a user URL.
const CACHE = 'diaxus-ai-model-v1';
let importing = false;
let acquiringRuntime = false;
let loadedRuntime;
let runtimeLoadPromise;
const RUNTIME_CACHE='diaxus-ai-runtime-ort-web-1.30.0';

function spec(manifestJson) {
  const manifest=JSON.parse(manifestJson);
  const model=manifest.model;
  if(manifest.schema_version!==1 || manifest.acquisition!=='explicit-local-file-import'
      || !Number.isSafeInteger(model?.bytes) || model.bytes<1 || model.bytes>8*1024*1024
      || !/^[a-f0-9]{64}$/.test(model.sha256)) throw Error('Incompatible AI model manifest');
  return {manifest,model,key:new URL(`/__diaxus_ai__/models/${model.sha256}`,globalThis.location.href).href};
}

function cancelled(check) {
  if(check?.())throw Error('CANCELLED: AI model acquisition stopped');
}

async function verify(bytes,model) {
  if(bytes.byteLength!==model.bytes)throw Error('AI model size mismatch; import the exact exported model.onnx');
  const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),v=>v.toString(16).padStart(2,'0')).join('');
  if(hash!==model.sha256)throw Error('AI model checksum mismatch; remove the invalid cache and import the validated export');
}

export async function importAiModel(file,manifestJson,check) {
  const {model,key}=spec(manifestJson);
  if(importing)throw Error('AI model acquisition is already active');
  importing=true;
  let cache,published=false;
  try {
    cancelled(check);
    if(file.size!==model.bytes)throw Error('AI model size mismatch; import the exact exported model.onnx');
    const bytes=await file.arrayBuffer();cancelled(check);
    await verify(bytes,model);cancelled(check);
    cache=await caches.open(CACHE);cancelled(check);
    const previous=await cache.match(key);
    if(previous){
      await verify(await previous.arrayBuffer(),model);cancelled(check);
      return 'Verified existing offline AI model cache';
    }
    // Cache.put is atomic for this entry; no unverified or partial graph is stored.
    await cache.put(key,new Response(bytes,{headers:{'Content-Type':'application/octet-stream'}}));
    published=true;cancelled(check);
    return 'Imported and verified offline AI model cache; inference remains gated';
  } catch(error) {
    if(published&&check?.())await cache.delete(key);
    if(error?.name==='QuotaExceededError')throw Error('AI model cache quota exceeded; remove cached assets or free browser storage and retry');
    throw error;
  } finally { importing=false; }
}

export async function readAiModel(manifestJson) {
  const {model,key}=spec(manifestJson);
  const cache=await caches.open(CACHE),response=await cache.match(key);
  if(!response)throw Error('AI model is not cached; explicitly import the validated model.onnx');
  const bytes=await response.arrayBuffer();await verify(bytes,model);
  return new Uint8Array(bytes);
}

export async function removeAiModel(manifestJson) {
  if(importing)throw Error('Cancel/finish AI model acquisition before removing its cache');
  const {key}=spec(manifestJson);
  return (await caches.open(CACHE)).delete(key);
}

function runtimeSpec(manifestJson) {
  const {manifest}=spec(manifestJson),runtime=manifest.browser_runtime;
  if(runtime?.package!=='onnxruntime-web'||runtime.version!=='1.30.0'||runtime.assets?.length!==3)
    throw Error('Incompatible AI runtime manifest');
  const names=['ort.webgpu.min.mjs','ort-wasm-simd-threaded.asyncify.mjs','ort-wasm-simd-threaded.asyncify.wasm'];
  if(!names.every(name=>runtime.assets.filter(asset=>asset.filename===name).length===1))
    throw Error('AI runtime asset list is incompatible');
  return runtime.assets.map(asset=>{
    if(!Number.isSafeInteger(asset.bytes)||asset.bytes<1||asset.bytes>32*1024*1024
        || !/^[a-f0-9]{64}$/.test(asset.sha256))throw Error('Incompatible AI runtime asset');
    return {...asset,key:new URL(`/__diaxus_ai__/runtime/${asset.sha256}`,location.href).href};
  });
}

async function verifyRuntime(bytes,asset) {
  if(bytes.byteLength!==asset.bytes)throw Error(`AI runtime size mismatch: ${asset.filename}`);
  const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),v=>v.toString(16).padStart(2,'0')).join('');
  if(hash!==asset.sha256)throw Error(`AI runtime checksum mismatch: ${asset.filename}; explicitly remove its cache and reacquire`);
}

// Separate explicit action: the UI must show URLs/license and total byte size.
export async function acquireAiRuntime(manifestJson,check) {
  const assets=runtimeSpec(manifestJson);
  if(acquiringRuntime)throw Error('AI runtime acquisition is already active');
  acquiringRuntime=true;
  let cache;
  const published=[];
  try{
    cancelled(check);cache=await caches.open(RUNTIME_CACHE);
    for(const asset of assets){
      cancelled(check);
      const previous=await cache.match(asset.key);
      if(previous){await verifyRuntime(await previous.arrayBuffer(),asset);continue;}
      const controller=new AbortController();
      const poll=setInterval(()=>{try{if(check?.())controller.abort();}catch{controller.abort();}},50);
      const deadline=setTimeout(()=>controller.abort(),60000);
      const chunks=[];let total=0,reader;
      try{
        const response=await fetch(asset.url,{credentials:'omit',redirect:'error',signal:controller.signal});
        if(!response.ok)throw Error(`AI runtime download failed (${response.status}): ${asset.filename}`);
        // Bound decoded body bytes: Content-Length may describe HTTP compression.
        reader=response.body.getReader();
        while(true){cancelled(check);const {done,value}=await reader.read();if(done)break;
          total+=value.byteLength;if(total>asset.bytes)throw Error(`AI runtime download exceeds declared bytes: ${asset.filename}`);chunks.push(value);}
      }finally{clearInterval(poll);clearTimeout(deadline);if(reader){await reader.cancel();reader.releaseLock();}}
      const bytes=new Uint8Array(total);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength;}
      await verifyRuntime(bytes,asset);cancelled(check);
      await cache.put(asset.key,new Response(bytes));published.push(asset.key);cancelled(check);
    }
    return 'Verified optional AI runtime cache; runtime is not loaded';
  }catch(error){
    if(cache)for(const key of published)await cache.delete(key);
    cancelled(check);
    if(error?.name==='QuotaExceededError')throw Error('AI runtime cache quota exceeded; free browser storage and explicitly retry');
    if(error?.name==='AbortError'||error instanceof TypeError)throw Error('AI runtime download failed, timed out or was interrupted; check the connection and explicitly retry');
    throw error;
  }finally{acquiringRuntime=false;}
}

export async function loadAiRuntime(manifestJson) {
  // No fetch/install fallback: both the model and all runtime assets must have
  // passed their separate explicit acquisition actions, including for offline use.
  await readAiModel(manifestJson);
  if(loadedRuntime)return loadedRuntime;
  if(!runtimeLoadPromise)runtimeLoadPromise=loadVerifiedRuntime(manifestJson).catch(error=>{runtimeLoadPromise=undefined;throw error;});
  return runtimeLoadPromise;
}

async function loadVerifiedRuntime(manifestJson) {
  const assets=runtimeSpec(manifestJson),cache=await caches.open(RUNTIME_CACHE),data=new Map();
  for(const asset of assets){
    const response=await cache.match(asset.key);
    if(!response)throw Error('AI runtime is not cached; explicitly acquire optional runtime assets');
    const bytes=await response.arrayBuffer();await verifyRuntime(bytes,asset);data.set(asset.filename,bytes);
  }
  const urls=[];
  try{
    const blob=name=>{const url=URL.createObjectURL(new Blob([data.get(name)],{type:'text/javascript'}));urls.push(url);return url;};
    const nativeModule=blob('ort-wasm-simd-threaded.asyncify.mjs');
    const ort=await import(blob('ort.webgpu.min.mjs'));
    ort.env.wasm.numThreads=1;
    ort.env.wasm.wasmBinary=data.get('ort-wasm-simd-threaded.asyncify.wasm');
    ort.env.wasm.wasmPaths={mjs:nativeModule};
    // Keep verified module URLs live for lazy WASM initialization. Removing the
    // persistent cache does not claim to unload an active compiled runtime.
    loadedRuntime=ort;
    return ort;
  }catch(error){for(const url of urls)URL.revokeObjectURL(url);throw error;}
}

export async function removeAiRuntime(manifestJson) {
  runtimeSpec(manifestJson);
  if(acquiringRuntime)throw Error('Cancel/finish AI runtime acquisition before removing its cache');
  return caches.delete(RUNTIME_CACHE);
}
