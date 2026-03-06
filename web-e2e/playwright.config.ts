import { defineConfig } from "@playwright/test";
import { CONFIG_PATH, DB_PATH } from "./testConfig";

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
    baseURL: "http://127.0.0.1:3000",
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
    command: "sh -c 'mkdir -p .runtime/media && cargo run --manifest-path ../Cargo.toml -- --reset-auth'",
    url: "http://127.0.0.1:3000/api/health",
    reuseExistingServer: false,
    timeout: 120_000,
    env: {
      ALCHEMIST_CONFIG_PATH: CONFIG_PATH,
      ALCHEMIST_DB_PATH: DB_PATH,
      ALCHEMIST_CONFIG_MUTABLE: "true",
      RUST_LOG: "warn",
    },
  },
});
