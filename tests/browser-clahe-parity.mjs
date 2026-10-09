// Developer-only comparison of decoded-source paused preview against the real
// conversion stage before lossy encoding. Browser connection is supplied by
// the supported tab-scoped API; this module opens no browser socket.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {errors,geometryColorReference} from './browser-color-numeric.mjs';

export async function runClaheParity({send,navigate,selectFile,directory,context='main',selectedHeight=null,fixture='fixtures/m35-vfr-offset.mp4'}) {
  fs.mkdirSync(directory,{recursive:true});
  const evidence={status:'running',context,selectedHeight,cases:[],gate:{maximum:2,domain:'pre-encoder RGBA8'}};
  const evaluate=async expression=>{
    const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});
    assert.ok(!r.exceptionDetails,JSON.stringify(r.exceptionDetails));return r.result.value;
  };
  const dispatch=(operation,profile,settings)=>evaluate(`(async()=>{const t=window.__parityTest;
    return await t.host.dispatchJob(document.querySelector('#source-file').files[0],${JSON.stringify(operation)},
      ${JSON.stringify(profile)},'no-preference',${JSON.stringify(settings)},()=>{},t.wasm.execute_job);})()`);
  try {
    await navigate(`http://127.0.0.1:8084/?verify=full&diagnostic-parity=${selectedHeight?'0,14':'0,7,14,24,35'}${context==='main'?'&execution=main':''}`);
    await selectFile(path.resolve(fixture));
    const deadline=Date.now()+45000;
    while(!await evaluate("!document.querySelector('#convert').disabled && /cached frames=1/.test(document.querySelector('#source-preview-status')?.textContent)")) {
      assert.ok(Date.now()<deadline,'source probe timeout');await new Promise(r=>setTimeout(r,50));
    }
    await new Promise(r=>setTimeout(r,200));
    evidence.environment=await evaluate("({userAgent:navigator.userAgent,gpu:document.querySelector('#selected-gpu')?.textContent,execution:document.querySelector('#execution-context')?.textContent})");
    await evaluate(`(async()=>{const urls=performance.getEntriesByType('resource').map(x=>x.name);
      window.__parityTest={host:await import(urls.find(x=>x.endsWith('/src/worker-host.js'))),
        wasm:await import(urls.find(x=>x.endsWith('/wasm/converter-web.js')))};return true;})()`);
    for(const [strength,color] of (selectedHeight?[[80,'-10/120/140']]:[[25,'0/100/100'],[50,'10/80/60'],[80,'-10/120/140']])) {
      for(const fps of (selectedHeight?['original']:['original','15','60'])) {
        const resize=selectedHeight?`exact:${selectedHeight*16/9}:${selectedHeight}:1`:'percent:50';
        const settings=`${resize}~bps=8000000~${fps}~${color}~1/${strength}`;
        const conversion=await dispatch('convert','mp4-h264-aac',settings);
        assert.match(conversion.summary,/full re-decode PASS/);
        assert.match(conversion.summary,/Cleanup: 0 application-held frame references, 0 samples/);
        assert.match(conversion.summary,/leases live\/peak=0\//);
        assert.match(conversion.summary,/CLAHE: 36 recorded source frames/);
        assert.equal(conversion.frameCount,{original:36,'15':30,'60':119}[fps]);
        assert.match(conversion.summary,selectedHeight?/2 explicit test-only pre-encoder pixel readbacks/:/5 explicit test-only pre-encoder pixel readbacks/);
        assert.equal(conversion.parityFrames.length,selectedHeight?2:5);
        const start=conversion.parityFrames[0].sourceTimestamp;
        const result={strength,color,fps,conversionSummary:conversion.summary,frames:[]};
        evidence.cases.push(result);
        for(const frame of conversion.parityFrames) {
          // Seek just inside this frame, not on a floating-point boundary that
          // could select its predecessor. Exact decoded PTS is asserted below.
          const position=(frame.sourceTimestamp-start+10)/1e6;
          const preview=await dispatch('preview',`parity-source@${position}`,settings);
          assert.equal(preview.parity.sourceTimestamp,frame.sourceTimestamp,'decoded source PTS differs');
          let actual=Buffer.from(preview.parity.data,'base64');
          let expected=Buffer.from(frame.data,'base64');
          if(selectedHeight){
            assert.deepEqual([frame.width,frame.height],[selectedHeight*16/9,selectedHeight]);
            assert.deepEqual([preview.parity.width,preview.parity.height],[640,360]);
            assert.match(preview.summary,new RegExp(`statistics/adjustments ${frame.width}×${frame.height}, display 640×360`));
            expected=geometryColorReference({data:expected,width:frame.width,height:frame.height},640,360,{brightness:0,contrast:100,saturation:100});
            actual=Buffer.from(actual.filter((_,i)=>i%4!==3));
          }else assert.deepEqual([preview.parity.width,preview.parity.height],[frame.width,frame.height]);
          const delta=errors(actual,expected);
          result.frames.push({index:frame.index,sourceTimestamp:frame.sourceTimestamp,position,delta,previewSummary:preview.summary});
          assert.ok(delta.maximum<=2,`preview/export pre-encoder error ${JSON.stringify(delta)}`);
          if(selectedHeight){
            await dispatch('preview',`parity-source@${position}`,`${resize}~bps=8000000~${fps}~0/100/100~1/0`);
            const restored=await dispatch('preview',`parity-source@${position}`,settings);
            assert.match(restored.summary,/history reused/);
            assert.equal(restored.parity.data,preview.parity.data,'large-preview Before restoration changed pixels');
            result.frames.at(-1).beforeRestoration='identical; history reused';
          }
        }
      }
    }
    evidence.status='passed';return evidence;
  } catch(error) {evidence.status='failed';evidence.error=String(error.stack);throw error;}
  finally {fs.writeFileSync(path.join(directory,'evidence.json'),JSON.stringify(evidence,null,2));}
}
