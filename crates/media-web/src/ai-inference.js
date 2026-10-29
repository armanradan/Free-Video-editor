// Concrete ORT boundary; runtime/model bytes come only from the verified cache.
// No model IO, device creation, CPU tensor fallback or pixel readback here.
const deviceStates = new WeakMap();
function deviceState(device) {
  let state = deviceStates.get(device);
  if (!state) {
    state = {lost: null};
    deviceStates.set(device, state);
    // One observer per device, independent of session lifetime. Do not replace
    // wgpu's loss callback or retain each released session until device loss.
    device.lost.then(info => {state.lost = `${info.reason}: ${info.message}`;});
  }
  return state;
}
function checkDevice(owner) {
  if (owner.deviceState.lost !== null) {
    owner.poisoned = true;
    throw Error(`AI processing device lost: ${owner.deviceState.lost}`);
  }
}
export async function createAiSession(ort, model, device) {
  const state = deviceState(device);
  ort.env.webgpu.device = device;
  const session = await ort.InferenceSession.create(model, {
    executionProviders: [{name: 'webgpu', device}],
    preferredOutputLocation: 'gpu-buffer',
    extra: {session: {disable_cpu_ep_fallback: '1'}},
  });
  const owner = {ort, session, device, deviceState: state, busy: false, poisoned: false, active: null};
  try { checkDevice(owner); }
  catch (error) { await session.release(); throw error; }
  return owner;
}

export function runAiTile(owner, inputBuffer, outputBuffer, width, height) {
  if (owner.busy || owner.poisoned || !owner.session) throw Error('AI session is busy or retired; create a fresh session');
  const active = runTileInner(owner, inputBuffer, outputBuffer, width, height);
  owner.active = active;
  active.then(() => {owner.active = null;}, () => {owner.active = null;});
  return active;
}

async function runTileInner(owner, inputBuffer, outputBuffer, width, height) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width > 196 || height > 196)
    throw Error('AI tile exceeds the validated 128-interior/34-halo envelope');
  owner.busy = true;
  let input, output, result;
  try {
    checkDevice(owner);
    input = owner.ort.Tensor.fromGpuBuffer(inputBuffer, {dataType: 'float32', dims: [1, 3, height, width]});
    output = owner.ort.Tensor.fromGpuBuffer(outputBuffer, {dataType: 'float32', dims: [1, 3, height * 4, width * 4]});
    result = (await owner.session.run({input}, {output})).output;
    if (result.gpuBuffer !== outputBuffer || result.location !== 'gpu-buffer' ||
        result.dims.join() !== [1, 3, height * 4, width * 4].join()) throw Error('ORT changed output buffer identity/geometry');
    // The next tile must not overwrite buffers still consumed by GPU work.
    await owner.device.queue.onSubmittedWorkDone();
    checkDevice(owner);
  } catch (error) {
    owner.poisoned = true;
    throw error;
  } finally {
    result?.dispose();
    if (output && output !== result) output.dispose();
    input?.dispose();
    owner.busy = false;
  }
}

export async function finishAiWork(owner) {
  await owner.device.queue.onSubmittedWorkDone();
  checkDevice(owner);
}

export async function releaseAiSession(owner) {
  owner.poisoned = true;
  // Also handle a dropped Rust future: its JavaScript inference promise keeps
  // running, so buffer/session destruction must wait for that promise to settle.
  if (owner.active) await owner.active.catch(() => {});
  try { await owner.device.queue.onSubmittedWorkDone(); }
  finally {
    const session = owner.session;
    owner.session = null;
    if (session) await session.release();
  }
}
