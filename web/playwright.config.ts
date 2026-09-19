import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  use: { ...devices["Desktop Chrome"] },
  webServer: {
    command: "pnpm dev --port 5173 --strictPort",
    port: 5173,
    reuseExistingServer: true,
    timeout: 30_000,
  },
});
