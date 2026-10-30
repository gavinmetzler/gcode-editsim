// Vite builds the UI into dist/, which the Tauri shell will serve; Vitest
// runs the tests in Node.
import { defineConfig } from "vitest/config";

export default defineConfig({
  // Relative asset paths, so the build works from the app's own origin.
  base: "./",
  build: {
    target: "es2022",
  },
  test: {
    include: ["tests/**/*.test.ts"],
    environment: "node",
  },
});
