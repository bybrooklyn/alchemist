import { expect, test, type Page } from "@playwright/test";
import { fulfillJson, mockEngineStatus } from "./helpers";

const systemSettings = {
  monitoring_poll_interval: 2,
  enable_telemetry: false,
  watch_enabled: true,
};

async function mockSystemTab(
  page: Page,
  summary: { total_checked: number; issues_found: number; last_run: string | null },
): Promise<void> {
  await page.route("**/api/settings/system", async (route) => {
    await fulfillJson(route, 200, systemSettings);
  });

  await page.route("**/api/library/health", async (route) => {
    await fulfillJson(route, 200, summary);
  });
}

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
});

test("library doctor card renders in settings", async ({ page }) => {
  await mockSystemTab(page, {
    total_checked: 0,
    issues_found: 0,
    last_run: null,
  });

  await page.goto("/settings?tab=system");

  await expect(page.getByRole("heading", { name: "Library Doctor" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Scan Library" })).toBeEnabled();
});

test("scan library button triggers POST request", async ({ page }) => {
  let scanRequested = false;

  await mockSystemTab(page, {
    total_checked: 0,
    issues_found: 0,
    last_run: null,
  });

  await page.route("**/api/library/health/scan", async (route) => {
    scanRequested = true;
    await fulfillJson(route, 202, { status: "started" });
  });

  await page.goto("/settings?tab=system");
  await page.getByRole("button", { name: "Scan Library" }).click();

  await expect.poll(() => scanRequested).toBe(true);
  await expect(page.getByText(/Library scan started/i).last()).toBeVisible();
});

test("library doctor shows last scan summary", async ({ page }) => {
  await mockSystemTab(page, {
    total_checked: 150,
    issues_found: 3,
    last_run: "2026-03-21T10:00:00Z",
  });

  await page.goto("/settings?tab=system");

  await expect(page.getByText(/150 files checked/i)).toBeVisible();
  await expect(page.getByText(/3 issues found/i)).toBeVisible();
  await expect(page.getByText(/issues/i).first()).toBeVisible();
});
