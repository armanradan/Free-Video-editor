// This module is copied beside the wasm-bindgen snippets, not separately bundled.
// It is both the window-side transport and the dedicated module worker entry.
const inWorker = typeof document === "undefined";
const messageOf = error => error?.message ?? String(error);

if (inWorker) {
  let runtime;
  let active = null;
  self.onmessage = async ({ data }) => {
    const { id, operation } = data;
    if (operation === "cancel") {
      if (active === id) runtime?.cancel_local();
      return;
    }
    if (active !== null) {
      self.postMessage({ id, type: "error", error: "Media worker is already running a command" });
      return;
    }
    active = id;
    try {
      if (operation === "init") {
        if (!self.isSecureContext) throw Error("worker requires a secure context");
        for (const name of ["VideoDecoder", "VideoEncoder", "VideoFrame", "AudioDecoder", "AudioEncoder", "OffscreenCanvas"]) {
          if (typeof self[name] !== "function") throw Error(`worker ${name} is unavailable`);
        }
        if (!navigator.gpu) throw Error("worker WebGPU is unavailable");
        // No extra probe device: Rust initializes the actual processing device/surface.
        runtime = await import(data.wasm);
        await runtime.default();
        await Promise.all([import(data.m1), import(data.pipeline)]);
        if (!self.__DIAXUS_M1__ || !self.__DIAXUS_MEDIA_WEB__) throw Error("worker codec scripts did not load");
        await runtime.initialize_worker(data.canvas);
        self.postMessage({ id, type: "result", result: {} });
      } else {
        if (!runtime) throw Error("media worker was not initialized");
        const result = await runtime.execute_job(data.file, operation, data.profile, data.acceleration,
          status => self.postMessage({ id, type: "progress", status }));
        self.postMessage({ id, type: "result", result });
      }
    } catch (error) {
      self.postMessage({ id, type: "error", error: messageOf(error) });
    } finally {
      active = null;
    }
  };
}

let assets;
let worker;
let initialization;
let fallbackReason;
let startupDurationMs;
let nextId = 0;
let cancelGeneration = 0;
let runningId = null;
let downloadUrl;
let tail = Promise.resolve();
const pending = new Map();

export function setupRuntime(m1, pipeline, wasm) {
  assets = { m1: new URL(m1, document.baseURI).href, pipeline: new URL(pipeline, document.baseURI).href, wasm };
}

function displayExecution(mode, reason = "") {
  const label = document.getElementById("execution-context");
  if (label) label.textContent = mode === "worker"
    ? "Execution: dedicated worker — WebCodecs + wgpu + OffscreenCanvas; frame handles stay in the worker."
    : `Execution: main-thread compatibility fallback — ${reason}`;
  const mainCanvas = document.getElementById("export-canvas");
  const workerPreview = document.getElementById("worker-preview");
  if (mainCanvas) mainCanvas.hidden = mode === "worker";
  if (workerPreview) workerPreview.hidden = mode !== "worker";
}

function stopWorker(error) {
  worker?.terminate();
  worker = null;
  initialization = null;
  for (const entry of pending.values()) entry.reject(error);
  pending.clear();
  runningId = null;
}

function request(operation, fields = {}, status = () => {}, transfer = []) {
  const id = ++nextId;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject, status });
    if (operation !== "init") runningId = id;
    try { worker.postMessage({ id, operation, ...fields }, transfer); }
    catch (error) { pending.delete(id); runningId = null; reject(error); }
  });
}

async function initialize() {
  if (fallbackReason) return false;
  if (initialization) return initialization;
  initialization = (async () => {
    const startupStarted = performance.now();
    let timer;
    try {
      if (!assets) throw Error("media runtime asset configuration is unavailable");
      if (new URL(location.href).searchParams.get("execution") === "main") throw Error("main-thread mode explicitly requested (?execution=main)");
      if (typeof Worker !== "function") throw Error("DedicatedWorker is unavailable");
      if (!HTMLCanvasElement.prototype.transferControlToOffscreen) throw Error("canvas transfer to OffscreenCanvas is unavailable");
      const preview = document.getElementById("worker-preview");
      if (!preview) throw Error("worker preview host is not mounted");
      const canvas = document.createElement("canvas");
      canvas.width = 160;
      canvas.height = 90;
      canvas.setAttribute("aria-label", "Worker wgpu output");
      preview.replaceChildren(canvas);
      const offscreen = canvas.transferControlToOffscreen();
      worker = new Worker(new URL(import.meta.url), { type: "module", name: "diaxus-media" });
      worker.onmessage = ({ data }) => {
        const entry = pending.get(data.id);
        if (!entry) return; // Ignore stale messages from retired jobs.
        if (data.type === "progress") { entry.status(data.status); return; }
        pending.delete(data.id);
        if (runningId === data.id) runningId = null;
        if (data.type === "result") entry.resolve(data.result);
        else entry.reject(Error(data.error));
      };
      worker.onerror = event => {
        event.preventDefault();
        stopWorker(Error(`Media worker failed: ${event.message || "runtime error"}. Retry to initialize a new worker.`));
      };
      worker.onmessageerror = () => stopWorker(Error("Media worker message could not be decoded. Retry the job."));
      await Promise.race([
        request("init", { ...assets, canvas: offscreen }, undefined, [offscreen]),
        new Promise((_, reject) => { timer = setTimeout(() => reject(Error("worker startup exceeded 30 seconds")), 30_000); }),
      ]);
      displayExecution("worker");
      startupDurationMs = performance.now() - startupStarted;
      return true;
    } catch (error) {
      fallbackReason = messageOf(error);
      startupDurationMs = performance.now() - startupStarted;
      stopWorker(error);
      displayExecution("main", fallbackReason);
      return false;
    } finally {
      clearTimeout(timer);
    }
  })();
  return initialization;
}

export function cancelRemote() {
  cancelGeneration++;
  if (worker && runningId !== null) worker.postMessage({ operation: "cancel", id: runningId });
}

export function dispatchJob(file, operation, profile, acceleration, status, local) {
  const generation = cancelGeneration;
  // Serialize profile probes and jobs: no configure/cancel race on a shared device.
  const task = tail.then(async () => {
    const reusedExecutionContext = Boolean(initialization);
    const useWorker = await initialize();
    const cancelled = () => generation !== cancelGeneration;
    if (cancelled()) throw Error("CANCELLED: stopped before job startup");
    if (operation === "convert" && downloadUrl) {
      URL.revokeObjectURL(downloadUrl);
      downloadUrl = null;
    }
    const report = value => { if (!cancelled()) status(value); };
    let result;
    if (useWorker) {
      displayExecution("worker");
      result = await request(operation, { file, profile, acceleration }, report);
    } else {
      displayExecution("main", fallbackReason);
      await Promise.all([import(assets.m1), import(assets.pipeline)]);
      if (cancelled()) throw Error("CANCELLED: stopped before codec startup");
      result = await local(file, operation, profile, acceleration, report);
    }
    if (cancelled()) throw Error("CANCELLED: completed work discarded after cancellation");
    if (result.blob) {
      downloadUrl = URL.createObjectURL(result.blob);
      delete result.blob;
      result.downloadUrl = downloadUrl;
    }
    if (result.summary) {
      result.summary += `\nExecution: ${useWorker ? "dedicated worker" : `main-thread fallback (${fallbackReason})`}.`;
      result.summary += `\nExecution-context startup probe: ${(startupDurationMs ?? 0).toFixed(1)} ms once; reused for this command=${reusedExecutionContext}.`;
    }
    return result;
  });
  tail = task.catch(() => {});
  return task;
}
