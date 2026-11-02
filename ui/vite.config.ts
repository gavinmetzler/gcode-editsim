// Vite builds the UI into dist/, which the Tauri shell will serve; Vitest
// runs the tests in Node.
import { execFileSync } from "node:child_process";

import { defineConfig } from "vitest/config";

/** Which commit the UI was built from, for the diagnostics report (M5.2):
 *  CI's commit, else git's, else "unknown" (a build outside git). */
function uiBuild(): string {
  const ci = process.env.GITHUB_SHA;
  if (ci) return ci.slice(0, 7);
  try {
    return execFileSync("git", ["rev-parse", "--short", "HEAD"], { encoding: "utf8" }).trim() || "unknown";
  } catch {
    return "unknown";
  }
}

export default defineConfig({
  // Relative asset paths, so the build works from the app's own origin.
  base: "./",
  define: { __UI_BUILD__: JSON.stringify(uiBuild()) },
  build: {
    target: "es2022",
    // three.js alone is about 550 kB; the app loads it from disk, not a
    // network, so one bundle is fine.
    chunkSizeWarningLimit: 1024,
  },
  // The bundled sample program lives in tests/golden/.
  server: {
    fs: { allow: [".."] },
  },
  test: {
    include: ["tests/**/*.test.ts"],
    environment: "node",
  },
});
