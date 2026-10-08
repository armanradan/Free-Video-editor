import assert from 'node:assert/strict';

const ticks = (value, timeBase) => {
  const [n, d] = timeBase.split('/').map(BigInt);
  return { n: BigInt(value) * n, d };
};
const subtract = (a, b) => ({ n: a.n * b.d - b.n * a.d, d: a.d * b.d });
const add = (a, b) => ({ n: a.n * b.d + b.n * a.d, d: a.d * b.d });
const seconds = value => Number(value.n) / Number(value.d);

// Independent fixture oracle: stream ticks, not FFprobe's rounded duration text
// or the application's own planned count. All accepted fixture origins are >=0.
export function expectedTimeline(streams, inputPts, fps, originOverride = null) {
  const video = streams.find(s => s.codec_type === 'video');
  const start = ticks(video.start_pts, video.time_base);
  const audio = streams.find(s => s.codec_type === 'audio');
  let origin = audio ? ticks(audio.start_pts, audio.time_base) : start;
  if (start.n * origin.d < origin.n * start.d) origin = start;
  if (originOverride) origin = originOverride;
  if (fps === 'original') return inputPts.map(pts => pts - seconds(origin));
  const [num, den = 1n] = fps.split('/').map(BigInt);
  const firstTime = subtract(start, origin);
  const endTime = subtract(add(start, ticks(video.duration_ts, video.time_base)), origin);
  assert.ok(firstTime.n >= 0n && num > 0n && den > 0n);
  const firstN = firstTime.n * num, firstD = firstTime.d * den;
  const first = (2n * firstN + firstD) / (2n * firstD);
  const endN = endTime.n * num, endD = endTime.d * den;
  const end = (endN + endD - 1n) / endD;
  assert.ok(end - first <= 10000n, 'fixture oracle is not a whole-movie allocator');
  return Array.from({ length: Number(end - first) }, (_, i) => Number(first + BigInt(i)) * Number(den) / Number(num));
}

export function assertTimeline(actual, expected, tolerance = .0011) {
  assert.equal(actual.length, expected.length, 'output frame count differs');
  let maximumError = 0;
  actual.forEach((pts, i) => {
    const error = Math.abs(pts - expected[i]);
    maximumError = Math.max(error, maximumError);
    assert.ok(error <= tolerance, `frame ${i}: ${pts} vs ${expected[i]}`);
  });
  return { frames: actual.length, maximumErrorSeconds: maximumError, toleranceSeconds: tolerance };
}

export function colorReference16(rgb48, { brightness, contrast, saturation }) {
  assert.equal(rgb48.length % 6, 0);
  const output = Buffer.alloc(rgb48.length);
  for (let i = 0; i < rgb48.length; i += 6) {
    const rgb = [0, 2, 4].map(c => rgb48.readUInt16LE(i + c) / 65535);
    const y = .2126 * rgb[0] + .7152 * rgb[1] + .0722 * rgb[2];
    rgb.forEach((x, c) => output.writeUInt16LE(Math.round(65535 * Math.max(0, Math.min(1,
      (y + saturation / 100 * (x - y) - .5) * contrast / 100 + .5 + brightness / 100))), i + c * 2));
  }
  return output;
}

export function errors16(actual, expected) {
  assert.equal(actual.length, expected.length);
  const histogram = new Uint32Array(65536);
  let sum = 0, maximum = 0;
  for (let i = 0; i < actual.length; i += 2) {
    const error = Math.abs(actual.readUInt16LE(i) - expected.readUInt16LE(i));
    histogram[error]++; sum += error; maximum = Math.max(maximum, error);
  }
  const channels = actual.length / 2;
  let p95 = 0, count = 0;
  for (; p95 < 65535; p95++) { count += histogram[p95]; if (count >= channels * .95) break; }
  return { maximum16: maximum, mae8Equivalent: sum / channels / 257, p95_8Equivalent: p95 / 257, channels };
}
