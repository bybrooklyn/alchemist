import { defineConfig } from "@playwright/test";
import { BASE_URL, CONFIG_PATH, DB_PATH, PORT } from "./testConfig";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },
  reporter: "list",
  globalSetup: "./global-setup.ts",
  use: {
    baseURL: BASE_URL,
    headless: true,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "auth",
      testIgnore: /setup-recovery\.spec\.ts/,
      use: {
        storageState: ".runtime/auth-state.json",
      },
    },
    {
      name: "setup",
      testMatch: /setup-recovery\.spec\.ts/,
      use: {
        storageState: undefined,
      },
    },
  ],
  webServer: {
    command: "sh -c 'mkdir -p .runtime/media && cd .. && cargo run --no-default-features -- --reset-auth'",
    url: `${BASE_URL}/api/health`,
    reuseExistingServer: false,
    timeout: 120_000,
    env: {
      ALCHEMIST_CONFIG_PATH: CONFIG_PATH,
      ALCHEMIST_DB_PATH: DB_PATH,
      ALCHEMIST_CONFIG_MUTABLE: "true",
      ALCHEMIST_SERVER_PORT: String(PORT),
      RUST_LOG: "warn",
    },
  },
});
