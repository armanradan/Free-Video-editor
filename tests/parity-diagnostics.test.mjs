import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

const source=fs.readFileSync(new URL('../apps/web/assets/m2.js',import.meta.url),'utf8');
const body=source.slice(source.indexOf('function parityFrameIndices('),source.indexOf('async function previewRender('));
function harness(value='') {
  const context=vm.createContext({__DIAXUS_PARITY_FRAMES__:value,Uint8Array,btoa:binary=>Buffer.from(binary,'binary').toString('base64'),fail:message=>{throw Error(message);}});
  vm.runInContext(`${body};this.indices=parityFrameIndices;this.snapshot=paritySnapshot;`,context);
  return context;
}
test('diagnostic selection is opt-in and bounded; malformed indices reject',()=>{
  assert.equal(harness().indices().size,0);
  assert.deepEqual(Array.from(harness('0,7,35').indices()),[0,7,35]);
  for(const value of ['-1','1.5','1,','0,1,2,3,4,5,6,7,8','1000001'])assert.throws(()=>harness(value).indices());
});
test('diagnostic snapshot rejects oversized storage before reading pixels',async()=>{
  let reads=0;
  const frame={displayWidth:1921,displayHeight:1080,copyTo(){reads++;}};
  await assert.rejects(harness().snapshot(frame,12),/1920/);
  frame.displayWidth=1920;frame.allocationSize=()=>1920*1080*4+1;
  await assert.rejects(harness().snapshot(frame,12),/byte budget/);
  assert.equal(reads,0);
});
test('diagnostic snapshot preserves source PTS and owned RGBA bytes without closing its borrowed frame',async()=>{
  const frame={displayWidth:1,displayHeight:1,allocationSize:()=>4,copyTo:async bytes=>bytes.set([7,23,42,255]),close(){throw Error('borrowed frame closed');}};
  const result=await harness().snapshot(frame,1272000,7);
  assert.equal(result.sourceTimestamp,1272000);assert.equal(result.index,7);
  assert.deepEqual([...Buffer.from(result.data,'base64')],[7,23,42,255]);
});
