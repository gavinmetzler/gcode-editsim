// The shared render-buffer fixture (tests/golden/render_buffer_v2.bin).

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** The fixture as a standalone ArrayBuffer, as the IPC layer delivers it. */
export function fixture(): ArrayBuffer {
  const path = fileURLToPath(new URL("../../tests/golden/render_buffer_v2.bin", import.meta.url));
  const bytes = readFileSync(path);
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}
