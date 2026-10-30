// Vite builds the UI into dist/, which the Tauri shell will serve; Vitest
// runs the tests in Node.
import { defineConfig } from "vitest/config";

export default defineConfig({
  // Relative asset paths, so the build works from the app's own origin.
  base: "./",
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
