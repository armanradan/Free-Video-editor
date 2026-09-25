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
const instrumented = source.replace(marker, `${marker}
var diaxusPeakHeapBytes = 0;
setInterval(() => {
  const size = ffmpeg?.HEAP8?.byteLength ?? 0;
  if (size > diaxusPeakHeapBytes) {
    diaxusPeakHeapBytes = size;
    self.postMessage({ type: "LOG", data: { type: "stdout", message: "DIAXUS_WASM_HEAP_PEAK=" + size } });
  }
}, 25);
`);
await fs.writeFile("assets/ffmpeg-worker.js", instrumented);
