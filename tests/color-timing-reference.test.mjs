import { test } from 'node:test';
import assert from 'node:assert/strict';
import { expectedTimeline, assertTimeline, colorReference16, errors16 } from './color-timing-reference.mjs';

test('exact ticks avoid a spurious CFR tail and preserve nonzero A/V origins', () => {
  const video = { codec_type: 'video', time_base: '1/30', start_pts: 0, duration_ts: 2, duration: '.066667' };
  assert.deepEqual(expectedTimeline([video], [0, 1/30], '30'), [0, 1/30]);
  const streams = [{ ...video, start_pts: 15, duration_ts: 3 },
    { codec_type: 'audio', time_base: '1/48000', start_pts: 19200 }];
  assertTimeline(expectedTimeline(streams, [.5, .566667], 'original'), [.1, .166667], 1e-12);
  const grid = expectedTimeline(streams, [], '15');
  assert.equal(grid.length, 1);
  assert.ok(Math.abs(grid[0] - 2/15) < 1e-12);
  assert.throws(() => assertTimeline([0, .066667], [0, 1/30]), /frame 1/);
});

test('16-bit reference preserves sub-8-bit precision and clamps after the combined chain', () => {
  const source = Buffer.alloc(6);
  [10001,30003,60007].forEach((v, i) => source.writeUInt16LE(v, i * 2));
  const identity = colorReference16(source, { brightness:0, contrast:100, saturation:100 });
  assert.ok(identity.equals(source));
  const adjusted = colorReference16(source, { brightness:10, contrast:80, saturation:60 });
  assert.ok([0,2,4].some(c => adjusted.readUInt16LE(c) % 257 !== 0));
  assert.equal(errors16(adjusted,adjusted).maximum16,0);
  assert.ok(colorReference16(source,{ brightness:100,contrast:200,saturation:0 }).every(v=>v===255));
});
