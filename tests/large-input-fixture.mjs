import fs from "node:fs";
import path from "node:path";

const FREE_BOX_BYTES = 257 * 1024 * 1024;

export function ensureLargeInputFixture() {
  const source = path.resolve("fixtures/m2-h264-aac.mp4");
  const directory = path.resolve("tmp/m36-streaming");
  const output = path.join(directory, "m2-h264-aac-plus-free-box.mp4");
  const expectedSize = fs.statSync(source).size + FREE_BOX_BYTES;
  if (fs.existsSync(output) && fs.statSync(output).size === expectedSize) return output;

  fs.mkdirSync(directory, { recursive: true });
  fs.copyFileSync(source, output);
  const file = fs.openSync(output, "r+");
  try {
    const sourceSize = fs.fstatSync(file).size;
    const header = Buffer.alloc(8);
    header.writeUInt32BE(FREE_BOX_BYTES, 0);
    header.write("free", 4, "ascii");
    fs.writeSync(file, header, 0, header.length, sourceSize);
    // Extending rather than materializing the zero-filled payload keeps this
    // deterministic validation fixture sparse on filesystems that support it.
    fs.ftruncateSync(file, expectedSize);
  } finally {
    fs.closeSync(file);
  }
  return output;
}
