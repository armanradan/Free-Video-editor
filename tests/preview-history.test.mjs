import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

// Exercise the actual browser-boundary function with instrumented codec owners;
// real decoder/GPU execution is a separate browser gate.
const source=fs.readFileSync(new URL('../apps/web/assets/m2.js',import.meta.url),'utf8');
const body=source.slice(source.indexOf('async function previewHistory('),source.indexOf('\nconst OUTPUT_PROFILES'));
function harness(timestamps, reject=false) {
  const counts={samples:0,frames:0,outputs:0,peakFrames:0,disposed:0,returned:0,processed:[]};
  const sample=pts=>{
    counts.samples++;
    return {timestamp:pts/1e6,close(){counts.samples--;},toVideoFrame(){
      counts.frames++;counts.peakFrames=Math.max(counts.frames,counts.peakFrames);
      return {timestamp:pts,duration:0,close(){counts.frames--;}};
    }};
  };
  const context=vm.createContext({
    previewCache:{frame:{timestamp:200000}},previewStream:null,previewHistoryOpens:0,
    MediabunnyInputAdapter:{open:async()=>({videoStart:0,track:{},input:{dispose(){counts.disposed++;}}})},
    VideoSampleSink:class {async *samples(){try{for(const pts of timestamps)yield sample(pts);}finally{counts.returned++;}}},
    fail(message){throw Error(message);},
  });
  vm.runInContext(`${body};this.run=previewHistory;`,context);
  const process=async frame=>{
    counts.processed.push(frame.timestamp);
    if(reject)throw Error('GPU rejected frame');
    counts.outputs++;return {close(){counts.outputs--;}};
  };
  return {counts,run:()=>context.run({},process,100000)};
}
test('VFR preroll includes the exact window boundary but excludes current/future frames',async()=>{
  const h=harness([99000,100000,130000,199999,200000,220000]);
  assert.equal((await h.run()).processed,3);
  assert.deepEqual(h.counts.processed,[100000,130000,199999]);
  assert.equal(h.counts.peakFrames,1);
  for(const key of ['samples','frames','outputs'])assert.equal(h.counts[key],0);
  assert.equal(h.counts.disposed,1);assert.equal(h.counts.returned,1);
});
test('preroll releases sample/frame/input when GPU processing rejects',async()=>{
  const h=harness([150000],true);
  await assert.rejects(h.run(),/GPU rejected/);
  for(const key of ['samples','frames','outputs'])assert.equal(h.counts[key],0);
  assert.equal(h.counts.disposed,1);assert.equal(h.counts.returned,1);
});
test('extreme sample rates fail explicitly instead of retaining or dropping unbounded history',async()=>{
  const h=harness(Array.from({length:65},(_,i)=>100000+i));
  await assert.rejects(h.run(),/exceeds 64/);
  assert.equal(h.counts.processed.length,64);assert.equal(h.counts.peakFrames,1);
  for(const key of ['samples','frames','outputs'])assert.equal(h.counts[key],0);
  assert.equal(h.counts.disposed,1);assert.equal(h.counts.returned,1);
});
