import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests",
  testMatch: "*.spec.ts",
  workers: 1,
  use: { baseURL: "http://127.0.0.1:5179", headless: true },
  webServer: [
    {
      command: "npm run dev -- --port 5179",
      url: "http://127.0.0.1:5179/api/admin/console/",
      reuseExistingServer: false,
    },
    {
      command: "node scripts/preview.mjs",
      env: { GROVE_PREVIEW_PORT: "5180" },
      url: "http://127.0.0.1:5180/api/admin/console/",
      reuseExistingServer: false,
    },
  ],
});
