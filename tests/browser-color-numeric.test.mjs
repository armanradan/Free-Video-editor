import assert from 'node:assert/strict';
import { test } from 'node:test';
import { colorReference, errors, geometryColorReference } from './browser-color-numeric.mjs';

test('reference operates on encoded RGB and applies saturation before contrast/brightness', () => {
  assert.deepEqual([...colorReference(Buffer.from([51,102,153,255]),
    {brightness:10,contrast:150,saturation:50})], [71,109,148]);
  assert.deepEqual([...colorReference(Buffer.from([255,0,0,255]),
    {brightness:0,contrast:100,saturation:0})], [54,54,54]);
});

test('reference clips only the final result and ignores opaque alpha', () => {
  const frame = Buffer.from([0,0,0,255,255,255,255,255]);
  assert.deepEqual([...colorReference(frame,{brightness:100,contrast:100,saturation:100})], [255,255,255,255,255,255]);
  assert.deepEqual([...colorReference(frame,{brightness:-100,contrast:100,saturation:100})], [0,0,0,0,0,0]);
});

test('lossy statistics retain an outlier without confusing it with percentile error', () => {
  const actual = Buffer.alloc(100); actual[99] = 255;
  assert.deepEqual(errors(actual,Buffer.alloc(100)), {maximum:255,mae:2.55,p95:0,channels:100});
  assert.throws(()=>errors(Buffer.alloc(1),Buffer.alloc(2)), /pixel geometry differs/);
});

test('geometry oracle rotates then reflects asymmetric corner pixels', () => {
  const image = {width:3,height:2,data:Buffer.from([10,0,0,255,20,0,0,255,30,0,0,255,40,0,0,255,50,0,0,255,60,0,0,255])};
  const neutral = {brightness:0,contrast:100,saturation:100};
  const reds = (rotation, flip, width, height) => [...geometryColorReference(image,width,height,neutral,rotation,flip)].filter((_,i)=>i%3===0);
  assert.deepEqual(reds(90,false,2,3), [40,10,50,20,60,30]);
  assert.deepEqual(reds(90,true,2,3), [10,40,20,50,30,60]);
  assert.deepEqual(reds(180,false,3,2), [60,50,40,30,20,10]);
  assert.deepEqual(reds(270,false,2,3), [30,60,20,50,10,40]);
});

test('geometry oracle interpolates before adjustment/clamping and clamps edge sampling', () => {
  const image = {width:2,height:1,data:Buffer.from([0,0,0,255,255,255,255,255])};
  assert.deepEqual([...geometryColorReference(image,1,1,{brightness:25,contrast:200,saturation:100})], [191,191,191]);
  assert.deepEqual([...geometryColorReference(image,4,1,{brightness:0,contrast:100,saturation:100})],
    [0,0,0,64,64,64,191,191,191,255,255,255]);
});
