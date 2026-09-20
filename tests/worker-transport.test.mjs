import assert from "node:assert/strict";
import { test } from "node:test";

let serial = 0;
async function fixture({ initError, crash = false, hold = false, mode = "worker", verify = false } = {}) {
  const nodes = new Map(["execution-context", "export-canvas", "worker-preview"].map(id => [id, {
    hidden: false, textContent: "", replaceChildren() {},
  }]));
  const sent = [];
  const workers = [];
  globalThis.document = {
    baseURI: "https://test.invalid/",
    getElementById: id => nodes.get(id),
    createElement: () => ({ setAttribute() {}, transferControlToOffscreen: () => ({ offscreen: true }) }),
  };
  const query = new URLSearchParams();
  if (mode === "main") query.set("execution", "main");
  if (verify) query.set("verify", "full");
  globalThis.location = { href: `https://test.invalid/?${query}` };
  globalThis.HTMLCanvasElement = class { transferControlToOffscreen() {} };
  globalThis.Worker = class {
    constructor() { workers.push(this); }
    terminate() { this.terminated = true; }
    postMessage(message, transfer) {
      sent.push({ message, transfer });
      queueMicrotask(() => {
        if (message.operation === "init") {
          this.reply(message.id, initError ? "error" : "result", initError ?? {});
        } else if (message.operation === "cancel") {
          this.reply(message.id, "error", "CANCELLED: test job drained");
        } else if (crash) {
          crash = false;
          this.onerror({ preventDefault() {}, message: "injected runtime failure" });
        } else if (!hold) {
          this.onmessage({ data: { id: message.id, type: "progress", status: "processing" } });
          this.reply(message.id, "result", { summary: "PASS: test", gpu: "test adapter" });
        }
      });
    }
    reply(id, type, value) {
      this.onmessage({ data: { id, type, ...(type === "error" ? { error: value } : { result: value }) } });
    }
  };
  const module = await import(`../crates/media-web/src/worker-host.js?case=${serial++}`);
  const script = "data:text/javascript,export default 0";
  module.setupRuntime(script, script, script);
  return { module, sent, workers, nodes };
}

const until = async predicate => {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 1));
  }
  throw Error("test transport did not reach expected state");
};

test("worker transports commands and metadata; queued jobs are serialized", async () => {
  const { module, sent, workers, nodes } = await fixture({ hold: true, verify: true });
  const statuses = [];
  const local = () => { throw Error("unexpected fallback"); };
  const first = module.dispatchJob(null, "m1", "", "", value => statuses.push(value), local);
  const second = module.dispatchJob(null, "m1", "", "", () => {}, local);
  await until(() => sent.length === 2);
  assert.deepEqual(sent.map(item => item.message.operation), ["init", "m1"]);
  assert.equal(sent[1].message.verify, true);
  assert.equal(sent[0].transfer.length, 1);
  const firstId = sent[1].message.id;
  workers[0].reply(firstId, "result", { summary: "PASS" });
  assert.match((await first).summary, /dedicated worker/);
  await until(() => sent.length === 3);
  workers[0].onmessage({ data: { id: firstId, type: "progress", status: "stale" } });
  assert.deepEqual(statuses, []);
  workers[0].reply(sent[2].message.id, "result", { summary: "PASS" });
  await second;
  assert.match(nodes.get("execution-context").textContent, /dedicated worker/);
  assert.equal(nodes.get("export-canvas").hidden, true);
});

test("cancellation drains current command, rejects queued stale commands, permits restart", async () => {
  const { module, sent, workers } = await fixture({ hold: true });
  const run = () => module.dispatchJob(null, "m1", "", "", () => {}, () => assert.fail("fallback"));
  const first = run();
  const queued = run();
  const rejected = [assert.rejects(first, /CANCELLED/), assert.rejects(queued, /CANCELLED/)];
  await until(() => sent.length === 2);
  module.cancelRemote();
  await Promise.all(rejected);
  const next = run();
  await until(() => sent.filter(item => item.message.operation === "m1").length === 2);
  workers[0].reply(sent.at(-1).message.id, "result", { summary: "PASS" });
  await next;
  assert.equal(workers.length, 1, "successful cancellation should retain the device context");
});

test("startup failure gives exact visible fallback and preserves profile", async () => {
  const { module, workers, nodes } = await fixture({ initError: "worker WebGPU is unavailable", verify: true });
  let calls = 0;
  const result = await module.dispatchJob(null, "convert", "mp4-h264-aac", "prefer-hardware", () => {}, async (_, operation, profile, acceleration, verify) => {
    calls++;
    assert.equal(operation, "convert");
    assert.equal(profile, "mp4-h264-aac");
    assert.equal(acceleration, "prefer-hardware");
    assert.equal(verify, true);
    return { summary: "PASS" };
  });
  assert.equal(calls, 1);
  assert.equal(workers[0].terminated, true);
  assert.match(result.summary, /main-thread fallback \(worker WebGPU is unavailable\)/);
  assert.match(nodes.get("execution-context").textContent, /worker WebGPU is unavailable/);
});

test("runtime crash rejects job without silently rerunning it; retry creates new worker", async () => {
  const { module, workers } = await fixture({ crash: true });
  const run = () => module.dispatchJob(null, "m1", "", "", () => {}, () => assert.fail("must not fallback mid-job"));
  await assert.rejects(run(), /injected runtime failure/);
  assert.equal(workers[0].terminated, true);
  assert.match((await run()).summary, /dedicated worker/);
  assert.equal(workers.length, 2);
});

test("explicit main-thread mode bypasses worker creation", async () => {
  const { module, workers } = await fixture({ mode: "main" });
  const result = await module.dispatchJob(null, "m1", "", "", () => {}, async () => ({ summary: "PASS" }));
  assert.equal(workers.length, 0);
  assert.match(result.summary, /explicitly requested/);
});
