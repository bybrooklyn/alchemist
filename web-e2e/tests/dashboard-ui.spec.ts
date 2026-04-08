import { expect, test } from "@playwright/test";
import { fulfillJson, mockDashboardData, mockEngineStatus } from "./helpers";

test.use({ storageState: undefined });

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
  await mockDashboardData(page);
  await page.route("**/api/stats/daily", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/settings/watch-dirs**", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles/presets", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles", async (route) => {
    await fulfillJson(route, 200, []);
  });
});

test("Last 7 Days panel is shown on dashboard", async ({ page }) => {
  await page.unroute("**/api/stats/daily");
  await page.route("**/api/stats/daily", async (route) => {
    await fulfillJson(route, 200, [
      {
        date: "2026-03-22",
        jobs_completed: 5,
        bytes_saved: 1_073_741_824,
      },
      {
        date: "2026-03-23",
        jobs_completed: 3,
        bytes_saved: 536_870_912,
      },
    ]);
  });

  await page.goto("/");

  await expect(page.getByText("Last 7 Days")).toBeVisible();
  await expect(page.getByText("Space recovered")).toBeVisible();
  await expect(page.getByText("Jobs completed")).toBeVisible();
});

test("ENGINE PAUSED banner is shown when engine is paused and mentions auto-analysis", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.getByText("ENGINE PAUSED")).toBeVisible();
  await expect(page.getByText(/Analysis runs automatically/i)).toBeVisible();
  await expect(page.getByText(/won't start encoding until/i)).not.toBeVisible();
});

test("ENGINE PAUSED banner is hidden when engine is running", async ({ page }) => {
  await page.unroute("**/api/engine/status");
  await page.unroute("**/api/engine/mode");
  await mockEngineStatus(page, {
    status: "running",
    manual_paused: false,
  });

  await page.goto("/");

  await expect(page.getByText("ENGINE PAUSED")).not.toBeVisible();
});

test("About modal opens and does not contain Al badge", async ({ page }) => {
  await page.route("**/api/system/info", async (route) => {
    await fulfillJson(route, 200, {
      version: "0.3.0",
      os_version: "macos aarch64",
      is_docker: false,
      telemetry_enabled: false,
      ffmpeg_version: "N-12345",
    });
  });
  await page.route("**/api/system/update", async (route) => {
    await fulfillJson(route, 200, {
      current_version: "0.3.0",
      latest_version: "0.3.1",
      update_available: true,
      release_url: "https://github.com/bybrooklyn/alchemist/releases/tag/v0.3.1",
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: "About" }).click();

  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Alchemist" })).toBeVisible();
  await expect(page.getByText("v0.3.0")).toBeVisible();
  await expect(page.getByText("v0.3.1")).toBeVisible();
  await expect(page.getByRole("link", { name: "Download Update" })).toBeVisible();
  await expect(page.getByText(/^Al$/)).toHaveCount(0);
});

test("Settings page does not show Setup & Runtime Controls banner", async ({ page }) => {
  await page.goto("/settings");

  await expect(page.getByText("Setup & Runtime Controls")).not.toBeVisible();
});
