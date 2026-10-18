import { defineConfig, devices } from "@playwright/test";

// Screenshots of every screen for a visual review, not tests: `npm run screens`, into
// screens/out, in light and dark, at a desktop and a phone width. Build the core first.
export default defineConfig({
  testDir: "screens",
  timeout: 240_000,
  use: { baseURL: "http://localhost:5173" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npm run dev",
    url: "http://localhost:5173",
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
