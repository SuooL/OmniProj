import { defineConfig } from "@playwright/test";

// Browser-level R0 gates run against the Vite dev build with a mocked Tauri transport (see
// e2e/support/harness.ts) — never the user's real ~/.omniproj store. Uses the bundled Chromium
// (not the Desktop Chrome channel) so CI needs only `playwright install chromium`.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: "line",
  use: {
    baseURL: "http://localhost:5199",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { browserName: "chromium", viewport: { width: 1280, height: 800 } },
    },
  ],
  webServer: {
    command: "npm run dev -- --port 5199 --strictPort",
    port: 5199,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
