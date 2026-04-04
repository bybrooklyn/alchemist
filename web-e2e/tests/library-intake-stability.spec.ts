import { expect, test } from "@playwright/test";
import {
  fulfillEmpty,
  fulfillJson,
  mockEngineStatus,
  mockSettingsBundle,
} from "./helpers";

test.use({ storageState: undefined });

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
  await mockSettingsBundle(page);
  await page.route("**/api/library/profiles", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles/presets", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/settings/watch-dirs**", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/scan/status", async (route) => {
    await fulfillJson(route, 200, {
      is_running: false,
      files_found: 0,
      current_folder: null,
    });
  });
});

test("manual scan success is surfaced from Library & Intake", async ({ page }) => {
  let scanStartCalls = 0;

  await page.route("**/api/scan/start", async (route) => {
    scanStartCalls += 1;
    await fulfillEmpty(route, 202);
  });

  await page.goto("/settings?tab=watch");
  await page.getByRole("button", { name: /scan now/i }).click();

  await expect.poll(() => scanStartCalls).toBe(1);
  await expect(page.getByRole("button", { name: "Scanning..." })).toBeVisible();
  await expect(page.getByText("Library scan started.", { exact: true })).toBeVisible();
});

test("manual scan failures are surfaced from Library & Intake", async ({ page }) => {
  await page.route("**/api/scan/start", async (route) => {
    await fulfillJson(route, 503, { message: "Scanner unavailable" });
  });

  await page.goto("/settings?tab=watch");
  await page.getByRole("button", { name: /scan now/i }).click();

  await expect(page.getByText("Scanner unavailable").first()).toBeVisible();
});
