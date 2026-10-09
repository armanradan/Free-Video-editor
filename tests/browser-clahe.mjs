// Supply the supported Browser tool's tab-scoped CDP and file chooser.
// Calls the real serialized job transport, not a mock or second GPU device.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { expectedTimeline, assertTimeline } from './color-timing-reference.mjs';

export async function runBrowserClahe({send, navigate, selectFile, click, directory,
  contexts=['worker','main'], baseUrl='http://127.0.0.1:8084/', profile='mp4-h264-aac'}) {
  assert.ok(['mp4-h264-aac','webm-vp8-opus'].includes(profile));
  fs.mkdirSync(directory,{recursive:true});
  const evidence={status:'running',profile,contexts:[],cases:[],recovery:[]};
  const save=()=>fs.writeFileSync(path.join(directory,'evidence.json'),JSON.stringify(evidence,null,2));
  const run=(program,args,binary=false)=>{
    const result=spawnSync(program,args,{encoding:binary?null:'utf8',timeout:120000,maxBuffer:64*1024*1024});
    assert.equal(result.status,0,result.stderr?.toString());return result.stdout;
  };
  const evaluate=async expression=>{
    const value=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});
    assert.ok(!value.exceptionDetails,JSON.stringify(value.exceptionDetails));return value.result.value;
  };
  const wait=async expression=>{
    const deadline=Date.now()+45000;
    do {const value=await evaluate(expression);if(value)return value;await new Promise(r=>setTimeout(r,50));}while(Date.now()<deadline);
    throw Error(`Timeout: ${expression}`);
  };
  const source=path.resolve('fixtures/m35-vfr-offset.mp4');
  const streams=JSON.parse(run('ffprobe',['-v','error','-show_streams','-of','json',source])).streams;
  const sourcePts=JSON.parse(run('ffprobe',['-v','error','-select_streams','v:0','-show_frames',
    '-show_entries','frame=best_effort_timestamp_time','-of','json',source])).frames.map(f=>Number(f.best_effort_timestamp_time));
  const audio=streams.find(s=>s.codec_type==='audio');
  const packets=JSON.parse(run('ffprobe',['-v','error','-select_streams','a:0','-show_packets',
    '-show_entries','packet=pts','-of','json',source])).packets;
  const [n,d]=audio.time_base.split('/').map(BigInt);
  const origin={n:BigInt(packets[0].pts)*n,d};
  const decode=file=>run('ffmpeg',['-v','error','-i',file,'-map','0:v:0','-fps_mode','passthrough','-f','rawvideo','-pix_fmt','rgb24','-'],true);
  try {
    for(const context of contexts) {
      const url=new URL(baseUrl);url.searchParams.set('verify','full');
      if(context==='main')url.searchParams.set('execution','main');
      await navigate(url.href);
      await wait("!!document.querySelector('details.regression button')");
      await click('details.regression summary');await click('details.regression button');
      const m1=await wait("/^(PASS|FAILED):/.test(document.querySelector('#status')?.textContent)&&document.querySelector('#status').textContent");
      assert.match(m1,/^PASS: 30\/30/);
      assert.match(m1,context==='worker'?/Execution: dedicated worker/:/Execution: main-thread/);
      assert.match(m1,/Cleanup: 0 application-owned live frames/);
      await selectFile(source);
      await wait("!document.querySelector('#convert').disabled && /cached frames=1/.test(document.querySelector('#source-preview-status')?.textContent)");
      const environment=await evaluate("({userAgent:navigator.userAgent,gpu:document.querySelector('#selected-gpu')?.textContent,execution:document.querySelector('#execution-context')?.textContent})");
      evidence.contexts.push({context,m1,environment});save();
      // Resolve the already loaded runtime modules; do not initialize another wasm/device.
      await evaluate(`(async()=>{const resources=performance.getEntriesByType('resource').map(x=>x.name);
        const transportUrl=resources.find(x=>x.endsWith('/src/worker-host.js'));
        const wasmUrl=resources.find(x=>x.endsWith('/wasm/converter-web.js'));
        if(!transportUrl||!wasmUrl)throw Error('Loaded runtime modules not found');
        window.__claheTest={transport:await import(transportUrl),runtime:await import(wasmUrl),progress:[]};return true;})()`);
      const outputs=new Map();
      for(const [tag,strength,fps] of [['off',null,'original'],['zero',0,'original'],
        ['enabled',50,'original'],['drop',50,'15'],['duplicate',50,'60'],['repeat',50,'original']]) {
        const policy=strength===null?'0/50':`1/${strength}`;
        const command=`percent:50~bps=8000000~${fps}~10/80/60~${policy}`;
        const result=await evaluate(`(async()=>{const t=window.__claheTest;t.progress=[];
          const result=await t.transport.dispatchJob(document.querySelector('#source-file').files[0],
          'convert',${JSON.stringify(profile)},'no-preference',${JSON.stringify(command)},s=>t.progress.push(s),t.runtime.execute_job);
          const bytes=new Uint8Array(await (await fetch(result.downloadUrl)).arrayBuffer());
          let binary='';for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));
          return {result,bytes:btoa(binary),progress:t.progress};})()`);
        const file=path.join(directory,`${context}-${tag}.${profile.startsWith('mp4')?'mp4':'webm'}`);fs.writeFileSync(file,Buffer.from(result.bytes,'base64'));
        const summary=result.result.summary;
        assert.match(summary,/leases live\/peak=0\//);
        assert.match(summary,/Cleanup: 0 application-held frame references, 0 samples/);
        assert.match(summary,/conversion pixel readbacks=0/);
        if(strength)assert.match(summary,new RegExp(`CLAHE: ${sourcePts.length} recorded source frames; ${sourcePts.length*4} nominal compute passes`));
        else assert.ok(!summary.includes('CLAHE:'),'off/zero must bypass scratch/stages');
        const actual=JSON.parse(run('ffprobe',['-v','error','-select_streams','v:0','-show_frames',
          '-show_entries','frame=best_effort_timestamp_time','-of','json',file])).frames.map(f=>Number(f.best_effort_timestamp_time));
        const timeline=assertTimeline(actual,expectedTimeline(streams,sourcePts,fps,origin));
        const outputStreams=JSON.parse(run('ffprobe',['-v','error','-show_streams','-of','json',file])).streams;
        const video=outputStreams.find(s=>s.codec_type==='video'),outAudio=outputStreams.find(s=>s.codec_type==='audio');
        assert.deepEqual([video.width,video.height],[160,90]);assert.equal(outAudio.codec_name,profile.startsWith('mp4')?'aac':'opus');
        run('ffmpeg',['-v','error','-i',file,'-f','null','-']);
        const pcm=run('ffmpeg',['-v','error','-i',file,'-map','0:a:0','-f','f32le','-'],true);
        let peak=0;for(let i=0;i<pcm.length;i+=4)peak=Math.max(peak,Math.abs(pcm.readFloatLE(i)));assert.ok(peak>.01);
        evidence.cases.push({context,tag,strength,fps,file,result:result.result,timeline,video,audio:outAudio,audioPeak:peak});save();
        outputs.set(tag,decode(file));
      }
      assert.deepEqual(outputs.get('off'),outputs.get('zero'),'zero strength identity');
      assert.deepEqual(outputs.get('enabled'),outputs.get('repeat'),'new job history must be deterministic');
      assert.notDeepEqual(outputs.get('off'),outputs.get('enabled'),'CLAHE must change actual encoded pixels');
      for(const failure of ['cancel','codec-once','device-loss-once']) {
        const failed=await evaluate(`(async()=>{const t=window.__claheTest;
          const url=new URL(location.href);url.searchParams.set('failure',${JSON.stringify(failure)});history.replaceState(null,'',url);
          const progress=[];let cancelled=false;
          try {await t.transport.dispatchJob(document.querySelector('#source-file').files[0],'convert',
            ${JSON.stringify(profile)},'no-preference','percent:50~bps=8000000~original~10/80/60~1/50',s=>{
              progress.push(s);if(${JSON.stringify(failure)}==='cancel'&&!cancelled&&s.startsWith('Converting ')){
                cancelled=true;t.transport.cancelRemote();t.runtime.cancel_local();}},t.runtime.execute_job);
            return {unexpectedSuccess:true,progress};
          }catch(e){return {error:String(e),progress,cancelled};}})()`);
        assert.ok(!failed.unexpectedSuccess,`${failure} must fail`);
        assert.match(failed.error,failure==='cancel'?/CANCELLED/:failure==='codec-once'?/INJECTED: codec failure/:/INJECTED: WebGPU device loss after 4 completed frames/);
        assert.match(failed.error,/Cleanup: 0 application-held frame references, 0 samples/);
        if(failure==='cancel')assert.ok(failed.cancelled,'cancel only after actual processed-frame progress');
        const retry=await evaluate(`(async()=>{const t=window.__claheTest;const url=new URL(location.href);
          url.searchParams.delete('failure');history.replaceState(null,'',url);
          return await t.transport.dispatchJob(document.querySelector('#source-file').files[0],'convert',
            ${JSON.stringify(profile)},'no-preference','percent:50~bps=8000000~original~10/80/60~1/50',()=>{},t.runtime.execute_job);})()`);
        assert.equal(retry.inputFrameCount,sourcePts.length);assert.equal(retry.frameCount,sourcePts.length);
        assert.match(retry.summary,/full re-decode PASS/);
        assert.match(retry.summary,/Cleanup: 0 application-held frame references, 0 samples/);
        assert.match(retry.summary,/leases live\/peak=0\//);
        assert.match(retry.summary,new RegExp(`CLAHE: ${sourcePts.length} recorded source frames`));
        if(failure==='device-loss-once')assert.match(retry.summary,/device generation 2/);
        evidence.recovery.push({context,failure,failed,retry});save();
      }
    }
    evidence.status='passed';return evidence;
  } catch(error) {evidence.status='failed';evidence.error=String(error.stack);throw error;}
  finally {save();}
}
