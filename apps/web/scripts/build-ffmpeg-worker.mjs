// Pin instrumentation to the installed @ffmpeg/ffmpeg worker source. This
// measures allocated WASM linear memory, not whole-process resident memory.
import { build } from "esbuild";
import fs from "node:fs/promises";

const result = await build({
  entryPoints: ["node_modules/@ffmpeg/ffmpeg/dist/esm/worker.js"],
  bundle: true,
  format: "esm",
  platform: "browser",
  write: false,
});
const source = result.outputFiles[0].text;
const marker = "var ffmpeg;\n";
if (!source.includes(marker)) throw Error("FFmpeg worker layout changed; inspect metrics injection before rebuilding.");
const mountMarker = "var mount = ({ fsType, options, mountPoint }) => {";
const mountCall = "data = mount(_data);";
const execEnd = "  ffmpeg.reset();\n  return ret;\n};";
if (!source.includes(mountMarker) || !source.includes(mountCall) || !source.includes(execEnd)) {
  throw Error("FFmpeg mount handler layout changed; inspect device injection before rebuilding.");
}
const instrumented = source.replace(marker, `${marker}
var diaxusPeakHeapBytes = 0;
var diaxusDeviceId = 0;
setInterval(() => {
  const size = ffmpeg?.HEAP8?.byteLength ?? 0;
  if (size > diaxusPeakHeapBytes) {
    diaxusPeakHeapBytes = size;
    self.postMessage({ type: "LOG", data: { type: "stdout", message: "DIAXUS_WASM_HEAP_PEAK=" + size } });
  }
}, 25);
`).replace(mountMarker, `var mount = async ({ fsType, options, mountPoint }) => {
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
`).replace(mountCall, "data = await mount(_data);").replace(execEnd, `  ffmpeg.reset();
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
};`);
await fs.writeFile("assets/ffmpeg-worker.js", instrumented);
