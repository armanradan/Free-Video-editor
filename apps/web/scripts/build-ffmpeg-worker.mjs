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
  throw Error("FFmpeg mount handler layout changed; inspect OPFS device injection before rebuilding.");
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
  return ret;
};`);
await fs.writeFile("assets/ffmpeg-worker.js", instrumented);
