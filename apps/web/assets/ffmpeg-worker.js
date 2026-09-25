// node_modules/@ffmpeg/ffmpeg/dist/esm/const.js
var CORE_VERSION = "0.12.9";
var CORE_URL = `https://unpkg.com/@ffmpeg/core@${CORE_VERSION}/dist/umd/ffmpeg-core.js`;
var FFMessageType;
(function(FFMessageType2) {
  FFMessageType2["LOAD"] = "LOAD";
  FFMessageType2["EXEC"] = "EXEC";
  FFMessageType2["FFPROBE"] = "FFPROBE";
  FFMessageType2["WRITE_FILE"] = "WRITE_FILE";
  FFMessageType2["READ_FILE"] = "READ_FILE";
  FFMessageType2["DELETE_FILE"] = "DELETE_FILE";
  FFMessageType2["RENAME"] = "RENAME";
  FFMessageType2["CREATE_DIR"] = "CREATE_DIR";
  FFMessageType2["LIST_DIR"] = "LIST_DIR";
  FFMessageType2["DELETE_DIR"] = "DELETE_DIR";
  FFMessageType2["ERROR"] = "ERROR";
  FFMessageType2["DOWNLOAD"] = "DOWNLOAD";
  FFMessageType2["PROGRESS"] = "PROGRESS";
  FFMessageType2["LOG"] = "LOG";
  FFMessageType2["MOUNT"] = "MOUNT";
  FFMessageType2["UNMOUNT"] = "UNMOUNT";
})(FFMessageType || (FFMessageType = {}));

// node_modules/@ffmpeg/ffmpeg/dist/esm/errors.js
var ERROR_UNKNOWN_MESSAGE_TYPE = new Error("unknown message type");
var ERROR_NOT_LOADED = new Error("ffmpeg is not loaded, call `await ffmpeg.load()` first");
var ERROR_TERMINATED = new Error("called FFmpeg.terminate()");
var ERROR_IMPORT_FAILURE = new Error("failed to import ffmpeg-core.js");

// node_modules/@ffmpeg/ffmpeg/dist/esm/worker.js
var ffmpeg;

var diaxusPeakHeapBytes = 0;
var diaxusDeviceId = 0;
setInterval(() => {
  const size = ffmpeg?.HEAP8?.byteLength ?? 0;
  if (size > diaxusPeakHeapBytes) {
    diaxusPeakHeapBytes = size;
    self.postMessage({ type: "LOG", data: { type: "stdout", message: "DIAXUS_WASM_HEAP_PEAK=" + size } });
  }
}, 25);
var load = async ({ coreURL: _coreURL, wasmURL: _wasmURL, workerURL: _workerURL }) => {
  const first = !ffmpeg;
  try {
    if (!_coreURL)
      _coreURL = CORE_URL;
    importScripts(_coreURL);
  } catch {
    if (!_coreURL || _coreURL === CORE_URL)
      _coreURL = CORE_URL.replace("/umd/", "/esm/");
    self.createFFmpegCore = (await import(
      /* @vite-ignore */
      _coreURL
    )).default;
    if (!self.createFFmpegCore) {
      throw ERROR_IMPORT_FAILURE;
    }
  }
  const coreURL = _coreURL;
  const wasmURL = _wasmURL ? _wasmURL : _coreURL.replace(/.js$/g, ".wasm");
  const workerURL = _workerURL ? _workerURL : _coreURL.replace(/.js$/g, ".worker.js");
  ffmpeg = await self.createFFmpegCore({
    // Fix `Overload resolution failed.` when using multi-threaded ffmpeg-core.
    // Encoded wasmURL and workerURL in the URL as a hack to fix locateFile issue.
    mainScriptUrlOrBlob: `${coreURL}#${btoa(JSON.stringify({ wasmURL, workerURL }))}`
  });
  ffmpeg.setLogger((data) => self.postMessage({ type: FFMessageType.LOG, data }));
  ffmpeg.setProgress((data) => self.postMessage({
    type: FFMessageType.PROGRESS,
    data
  }));
  return first;
};
var exec = ({ args, timeout = -1 }) => {
  ffmpeg.setTimeout(timeout);
  ffmpeg.exec(...args);
  const ret = ffmpeg.ret;
  ffmpeg.reset();
  for (const access of self.__diaxusAccessHandles ?? []) {
    try { access.flush(); } finally { access.close(); }
  }
  self.__diaxusAccessHandles = [];
  if (self.__diaxusOutputMetrics) {
    self.postMessage({ type: "LOG", data: { type: "stdout", message: "DIAXUS_OPFS_OUTPUT=" + JSON.stringify(self.__diaxusOutputMetrics()) } });
  }
  if (self.__diaxusRingMetrics) {
    self.postMessage({ type: "LOG", data: { type: "stdout", message: "DIAXUS_RGBA_RING=" + JSON.stringify(self.__diaxusRingMetrics()) } });
  }
  return ret;
};
var ffprobe = ({ args, timeout = -1 }) => {
  ffmpeg.setTimeout(timeout);
  ffmpeg.ffprobe(...args);
  const ret = ffmpeg.ret;
  ffmpeg.reset();
  return ret;
};
var writeFile = ({ path, data }) => {
  ffmpeg.FS.writeFile(path, data);
  return true;
};
var readFile = ({ path, encoding }) => ffmpeg.FS.readFile(path, { encoding });
var deleteFile = ({ path }) => {
  ffmpeg.FS.unlink(path);
  return true;
};
var rename = ({ oldPath, newPath }) => {
  ffmpeg.FS.rename(oldPath, newPath);
  return true;
};
var createDir = ({ path }) => {
  ffmpeg.FS.mkdir(path);
  return true;
};
var listDir = ({ path }) => {
  const names = ffmpeg.FS.readdir(path);
  const nodes = [];
  for (const name of names) {
    const stat = ffmpeg.FS.stat(`${path}/${name}`);
    const isDir = ffmpeg.FS.isDir(stat.mode);
    nodes.push({ name, isDir });
  }
  return nodes;
};
var deleteDir = ({ path }) => {
  ffmpeg.FS.rmdir(path);
  return true;
};
var mount = async ({ fsType, options, mountPoint }) => {
  if (fsType === "DIAXUS_RGBA_RING") {
    const FS = ffmpeg.FS;
    const sharedBuffer = options?.buffer;
    if (!(sharedBuffer instanceof SharedArrayBuffer) || !self.crossOriginIsolated) {
      throw Error("FFmpeg raw-frame ring requires cross-origin-isolated shared memory.");
    }
    const control = new Int32Array(sharedBuffer, 0, 16);
    const data = new Uint8Array(sharedBuffer, 64);
    if (data.byteLength !== 4 * 1048576) throw Error("FFmpeg raw-frame ring layout mismatch.");
    const device = FS.makedev(91, ++diaxusDeviceId);
    let nextSlot = 0, slotOffset = 0, consumedBytes = 0, consumerWaits = 0;
    FS.registerDevice(device, {
      open(stream) { stream.seekable = false; },
      close() {},
      read(stream, target, offset, length) {
        let filled = 0;
        while (filled < length) {
          if (Atomics.load(control, 0) === 2) throw Error("FFmpeg raw-frame ring aborted.");
          const stateIndex = 1 + nextSlot;
          if (Atomics.load(control, stateIndex) !== 1) {
            if (Atomics.load(control, 0) === 1) break;
            consumerWaits++;
            Atomics.wait(control, stateIndex, 0, 1000);
            continue;
          }
          const slotLength = Atomics.load(control, 5 + nextSlot);
          if (slotLength <= 0 || slotLength > 1048576 || slotOffset >= slotLength) {
            throw Error("FFmpeg raw-frame ring slot length is invalid.");
          }
          const amount = Math.min(length - filled, slotLength - slotOffset);
          const start = nextSlot * 1048576 + slotOffset;
          target.set(data.subarray(start, start + amount), offset + filled);
          filled += amount;
          consumedBytes += amount;
          slotOffset += amount;
          if (slotOffset === slotLength) {
            slotOffset = 0;
            Atomics.store(control, stateIndex, 0);
            Atomics.notify(control, stateIndex);
            nextSlot = (nextSlot + 1) % 4;
          }
        }
        return filled;
      },
      llseek() { throw new FS.ErrnoError(29); },
    });
    FS.mkdev(mountPoint, 0o444, device);
    self.__diaxusRingMetrics = () => ({ consumedBytes, consumerWaits });
    return true;
  }
  if (fsType === "DIAXUS_OPFS") {
    const FS = ffmpeg.FS;
    const { fileHandle, mode } = options;
    if (!fileHandle?.createSyncAccessHandle || !["read", "write"].includes(mode)) {
      throw Error("OPFS synchronous file handles are unavailable for FFmpeg streaming.");
    }
    const access = await fileHandle.createSyncAccessHandle();
    if (mode === "write") access.truncate(0);
    const device = FS.makedev(90, ++diaxusDeviceId);
    let writes = 0, maxWriteBytes = 0, writtenBytes = 0;
    FS.registerDevice(device, {
      open(stream) { stream.seekable = true; },
      close() { if (mode === "write") access.flush(); },
      read(stream, buffer, offset, length, position) {
        return access.read(buffer.subarray(offset, offset + length), { at: position ?? stream.position });
      },
      write(stream, buffer, offset, length, position) {
        if (mode !== "write") throw Error("OPFS input device is not writable.");
        const count = access.write(buffer.subarray(offset, offset + length), { at: position ?? stream.position });
        writes++; writtenBytes += count; maxWriteBytes = Math.max(maxWriteBytes, count);
        return count;
      },
      llseek(stream, offset, whence) {
        const base = whence === 0 ? 0 : whence === 1 ? stream.position : access.getSize();
        const next = base + offset;
        if (!Number.isSafeInteger(next) || next < 0) throw Error("Invalid OPFS seek offset.");
        return next;
      },
    });
    FS.mkdev(mountPoint, 0o666, device);
    if (mode === "write") {
      self.__diaxusOutputMetrics = () => ({ writes, maxWriteBytes, writtenBytes });
    }
    (self.__diaxusAccessHandles ??= []).push(access);
    return true;
  }

  const str = fsType;
  const fs = ffmpeg.FS.filesystems[str];
  if (!fs)
    return false;
  ffmpeg.FS.mount(fs, options, mountPoint);
  return true;
};
var unmount = ({ mountPoint }) => {
  ffmpeg.FS.unmount(mountPoint);
  return true;
};
self.onmessage = async ({ data: { id, type, data: _data } }) => {
  const trans = [];
  let data;
  try {
    if (type !== FFMessageType.LOAD && !ffmpeg)
      throw ERROR_NOT_LOADED;
    switch (type) {
      case FFMessageType.LOAD:
        data = await load(_data);
        break;
      case FFMessageType.EXEC:
        data = exec(_data);
        break;
      case FFMessageType.FFPROBE:
        data = ffprobe(_data);
        break;
      case FFMessageType.WRITE_FILE:
        data = writeFile(_data);
        break;
      case FFMessageType.READ_FILE:
        data = readFile(_data);
        break;
      case FFMessageType.DELETE_FILE:
        data = deleteFile(_data);
        break;
      case FFMessageType.RENAME:
        data = rename(_data);
        break;
      case FFMessageType.CREATE_DIR:
        data = createDir(_data);
        break;
      case FFMessageType.LIST_DIR:
        data = listDir(_data);
        break;
      case FFMessageType.DELETE_DIR:
        data = deleteDir(_data);
        break;
      case FFMessageType.MOUNT:
        data = await mount(_data);
        break;
      case FFMessageType.UNMOUNT:
        data = unmount(_data);
        break;
      default:
        throw ERROR_UNKNOWN_MESSAGE_TYPE;
    }
  } catch (e) {
    self.postMessage({
      id,
      type: FFMessageType.ERROR,
      data: e.toString()
    });
    return;
  }
  if (data instanceof Uint8Array) {
    trans.push(data.buffer);
  }
  self.postMessage({ id, type, data }, trans);
};
