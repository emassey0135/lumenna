import { defineConfig, devices } from "@playwright/test";

// The web client in a real browser. Each test gets a fresh context, so a fresh, empty
// store in OPFS. Build the core first: `npm run core` (or `npm run core:dev`).
export default defineConfig({
  testDir: "tests",
  fullyParallel: true,
  timeout: 60_000,
  use: {
    baseURL: "http://localhost:5173",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npm run dev",
    url: "http://localhost:5173",
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
