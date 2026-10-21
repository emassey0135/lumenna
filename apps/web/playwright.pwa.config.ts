import { defineConfig, devices } from "@playwright/test";

// The installed web app, which only a production build is: its service worker, opening with
// no network, and its manifest. `npm run test:pwa` builds first; the dev server registers no
// service worker.
export default defineConfig({
  testDir: "pwa",
  timeout: 60_000,
  use: { baseURL: "http://localhost:4173", trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npx vite preview",
    url: "http://localhost:4173",
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
