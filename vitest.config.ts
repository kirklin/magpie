import { defineConfig } from "vitest/config";

// Separate from vite.config.ts (which carries Tauri dev-server settings) so the
// test runner stays isolated from the app build. Pure-function and store tests
// run in the node environment; no DOM needed yet.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    // Pin the timezone so date-grouping tests (which compare local
    // toDateString() values) are deterministic across machines/CI.
    env: { TZ: "UTC" },
  },
});
