import assert from "node:assert/strict";
import test from "node:test";
import { statistics, orderForRound, routes, expectedTransfers } from "./native-m5-release-bench.mjs";

test("summaries use medians without changing the recorded sample order", () => {
  const samples = [500, 100, 110];
  assert.deepEqual(statistics(samples), { min: 100, median: 110, max: 500, samples: 3 });
  assert.deepEqual(samples, [500, 100, 110]);
  assert.equal(statistics([40, 20, 10, 30]).median, 25);
  for (const invalid of [[], [NaN], [-1], [Infinity]]) assert.throws(() => statistics(invalid));
});

test("balanced rounds visit every route once and rotate the first route", () => {
  const first = new Set();
  for (let round = 0; round < routes.length; round++) {
    const order = orderForRound(round);
    assert.deepEqual([...order].sort(), [...routes].sort());
    first.add(order[0]);
  }
  assert.equal(first.size, routes.length);
});

test("transfer accounting distinguishes codec NV12 from processor RGBA", () => {
  const spec = { width: 320, height: 180, frames: 36 };
  assert.deepEqual(expectedTransfers(spec, "nvidia"), {
    explicit_cpu_to_gpu_bytes: 0, explicit_gpu_to_cpu_bytes: 0, codec_gpu_to_cpu_bytes: 0, codec_cpu_to_gpu_bytes: 0,
  });
  assert.deepEqual(expectedTransfers(spec, "wgpu-nvidia"), {
    explicit_cpu_to_gpu_bytes: 8_294_400, explicit_gpu_to_cpu_bytes: 2_073_600,
    codec_gpu_to_cpu_bytes: 3_110_400, codec_cpu_to_gpu_bytes: 777_600,
  });
});
