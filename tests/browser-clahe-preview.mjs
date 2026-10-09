// Experimental paused-preview gate through the supported tab-scoped Browser API.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';

export async function runClahePreview({send,navigate,selectFile,directory,context='main',lifecycle=false,deviceLoss=false}) {
  fs.mkdirSync(directory,{recursive:true});
  const evidence={status:'running',context,cases:[]};
  const evaluate=async expression=>{
    const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});
    assert.ok(!r.exceptionDetails,JSON.stringify(r.exceptionDetails));return r.result.value;
  };
  try {
    await navigate(`http://127.0.0.1:8084/?verify=full&diagnostic-preview=1${context==='main'?'&execution=main':''}`);
    await selectFile(path.resolve('fixtures/m35-vfr-offset.mp4'));
    const deadline=Date.now()+45000;
    while(!await evaluate("!document.querySelector('#convert').disabled && /cached frames=1/.test(document.querySelector('#source-preview-status')?.textContent)")) {
      assert.ok(Date.now()<deadline,'source probe timeout');await new Promise(r=>setTimeout(r,50));
    }
    // Let the ordinary UI's trailing source-selection coalescer settle before
    // issuing developer-only settings outside that UI owner.
    await new Promise(r=>setTimeout(r,200));
    await evaluate(`(async()=>{const urls=performance.getEntriesByType('resource').map(x=>x.name);
      window.__historyTest={host:await import(urls.find(x=>x.endsWith('/src/worker-host.js'))),
      wasm:await import(urls.find(x=>x.endsWith('/wasm/converter-web.js')))};return true;})()`);
    const preview=async(position,strength=50,streaming=false,resize=50,color='10/80/60')=>evaluate(`(async()=>{const t=window.__historyTest;
      return await t.host.dispatchJob(document.querySelector('#source-file').files[0],'preview',
      ${JSON.stringify(`history-test@${position}${streaming?'@1':''}`)},'no-preference',
      ${JSON.stringify(`percent:${resize}~recommended~original~${color}~1/${strength}`)},()=>{},t.wasm.execute_job);})()`);
    const snapshot=async()=>{
      const image=await evaluate(`(async()=>{let pixels,width,height;
        if(document.querySelector('#worker-preview').hidden){const d=window.__DIAXUS_PREVIEW_DIAGNOSTIC__;({pixels,width,height}=d);}
        else {const f=new VideoFrame(document.querySelector('#worker-preview canvas'),{timestamp:0});
          try{width=f.displayWidth;height=f.displayHeight;pixels=new Uint8Array(f.allocationSize({format:'RGBA'}));await f.copyTo(pixels,{format:'RGBA'});}finally{f.close();}}
        let binary='';for(let i=0;i<pixels.length;i+=8192)binary+=String.fromCharCode(...pixels.subarray(i,i+8192));
        return {width,height,data:btoa(binary)};})()`);
      assert.deepEqual([image.width,image.height],[160,90]);
      const bytes=Buffer.from(image.data,'base64');assert.ok(bytes.some((x,i)=>i%4!==3&&x>20));
      return createHash('sha256').update(bytes).digest('hex');
    };
    let initial;
    for(const position of [0,.4,.8,.4,.4]) {
      const result=await preview(position);
      assert.match(result.summary,/CLAHE paused preroll:/);
      assert.match(result.summary,/cached frames=1/);
      const hash=await snapshot();
      if(position===.4){if(initial)assert.equal(hash,initial,'seek-back/repeat pixels must be deterministic');else initial=hash;}
      evidence.cases.push({position,result,hash});
    }
    assert.match(evidence.cases.at(-1).result.summary,/history reused/);
    const count=(result,field)=>Number(result.summary.match(new RegExp(`${field}=(\\d+)`))[1]);
    const cached=evidence.cases.at(-1).result;
    for(const [strength,color] of [[20,'10/80/60'],[80,'20/120/80'],[0,'0/100/100'],[50,'10/80/60']]) {
      const result=await preview(.4,strength,false,50,color);
      if(strength)assert.match(result.summary,/history reused/);
      assert.equal(count(result,'cache loads'),count(cached,'cache loads'));
      assert.equal(count(result,'preroll opens'),count(cached,'preroll opens'));
      const hash=await snapshot();
      if(strength===50)assert.equal(hash,initial,'restore after Strength/sliders/Before must preserve original mapping');
      evidence.cases.push({position:.4,strength,color,result,hash});
    }
    for(const resize of [75,50]) {
      const result=await preview(.4,50,false,resize);
      assert.match(result.summary,/history rebuilt/);evidence.cases.push({resize,result});
    }
    await evaluate("window.__historyTest.host.dispatchJob(null,'clear-preview','','','original',()=>{},window.__historyTest.wasm.execute_job)");
    const cleared=await preview(.4);assert.match(cleared.summary,/history rebuilt/);
    evidence.cases.push({cleared:true,result:cleared});
    const playing = await preview(.4,50,true);
    assert.match(playing.summary,/CLAHE paused preroll:/);
    assert.equal(await snapshot(),initial,'sampled playback must retain exact preroll parity');
    evidence.cases.push({sampledPlayback:true,result:playing});
    const retry=await preview(.4);assert.match(retry.summary,/CLAHE paused preroll:/);
    evidence.retry=retry;
    if(lifecycle){
      const replaced=await evaluate(`(async()=>{const t=window.__historyTest;const original=document.querySelector('#source-file').files[0];
        const replacement=new File([original],'replacement-fixture.mp4',{type:original.type,lastModified:original.lastModified+1});
        return await t.host.dispatchJob(replacement,'preview','history-test@0.4','no-preference',
          'percent:50~recommended~original~10/80/60~1/50',()=>{},t.wasm.execute_job);})()`);
      assert.match(replaced.summary,/history rebuilt/);
      assert.equal(await snapshot(),initial,'replacement of identical fixture bytes must not inherit different pixels');
      const sourceRetry=await preview(.4);assert.match(sourceRetry.summary,/history rebuilt/);
      const rapid=await evaluate(`(async()=>{const t=window.__historyTest;const file=document.querySelector('#source-file').files[0];
        return await Promise.all([20,80,35,65,15,50].map(strength=>t.host.dispatchJob(file,'preview',
          'history-test@0.4','no-preference','percent:50~recommended~original~10/80/60~1/'+strength,()=>{},t.wasm.execute_job)));})()`);
      assert.equal(rapid.length,6);assert.ok(rapid.every(r=>/history reused/.test(r.summary)));
      assert.equal(await snapshot(),initial,'latest queued settings must win');
      await evaluate("window.__historyTest.host.dispatchJob(null,'clear-preview','','','original',()=>{},window.__historyTest.wasm.execute_job)");
      const cancelled=await evaluate(`(async()=>{const t=window.__historyTest;const file=document.querySelector('#source-file').files[0];let prepared=false;
        const command=status=>t.host.dispatchJob(file,'preview','history-test@0.4','no-preference',
          'percent:50~recommended~original~10/80/60~1/50',status,t.wasm.execute_job);
        const active=command(status=>{if(!prepared&&status.includes('first GPU frame prepared')){
          prepared=true;t.host.cancelRemote();t.wasm.cancel_local();}});
        const stale=command(()=>{});
        const results=await Promise.allSettled([active,stale]);
        return {prepared,results:results.map(r=>({status:r.status,error:String(r.reason)}))};})()`);
      assert.ok(cancelled.prepared,'cancellation must happen after a real GPU-processed frame');
      assert.ok(cancelled.results.every(r=>r.status==='rejected'&&/CANCELLED/.test(r.error)));
      const recovered=await preview(.4);assert.match(recovered.summary,/history rebuilt/);
      assert.equal(await snapshot(),initial,'cancel/retry must reproduce the original preview');
      evidence.lifecycle={replacement:replaced.summary,sourceRetry:sourceRetry.summary,rapid:rapid.map(r=>r.summary),cancelled,recovered:recovered.summary};
    }
    if(deviceLoss){
      await evaluate("window.__historyTest.host.dispatchJob(null,'clear-preview','','','original',()=>{},window.__historyTest.wasm.execute_job)");
      await evaluate("(()=>{const url=new URL(location.href);url.searchParams.set('failure','device-loss-once');history.replaceState(null,'',url);return true;})()");
      let lossError;
      try { await preview(.4); } catch(error) { lossError=String(error); }
      assert.match(lossError ?? '',/device loss/);
      const lossRetry=await preview(.4);
      assert.match(lossRetry.summary,/history rebuilt/);
      assert.match(lossRetry.summary,/device generation 2/);
      assert.equal(await snapshot(),initial,'new-device preview must reproduce pixels');
      evidence.deviceLoss={injection:'actual device.destroy after one prepared GPU frame',error:lossError,retry:lossRetry.summary};
    }
    const conversion=await evaluate(`(async()=>{const t=window.__historyTest;return await t.host.dispatchJob(
      document.querySelector('#source-file').files[0],'convert','mp4-h264-aac','no-preference',
      'percent:50~bps=8000000~original~10/80/60~1/50',()=>{},t.wasm.execute_job);})()`);
    assert.equal(conversion.frameCount,36);assert.match(conversion.summary,/full re-decode PASS/);
    assert.match(conversion.summary,/leases live\/peak=0\//);
    assert.match(conversion.summary,/Cleanup: 0 application-held frame references, 0 samples/);
    assert.match(conversion.summary,/CLAHE: 36 recorded source frames/);
    evidence.conversion=conversion;evidence.status='passed';return evidence;
  }catch(error){evidence.status='failed';evidence.error=String(error.stack);throw error;}
  finally{fs.writeFileSync(path.join(directory,'evidence.json'),JSON.stringify(evidence,null,2));}
}
