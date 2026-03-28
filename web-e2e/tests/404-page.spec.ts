import { expect, test, type Page } from "@playwright/test";
import { fulfillJson, mockEngineStatus } from "./helpers";

async function mockDashboard(page: Page): Promise<void> {
  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, []);
  });

  await page.route("**/api/stats", async (route) => {
    await fulfillJson(route, 200, {
      active: 0,
      concurrent_limit: 1,
      completed: 0,
      failed: 0,
      total: 0,
    });
  });

  await page.route("**/api/settings/system", async (route) => {
    await fulfillJson(route, 200, {
      monitoring_poll_interval: 2,
      enable_telemetry: false,
    });
  });

  await page.route("**/api/system/resources", async (route) => {
    await fulfillJson(route, 200, {
      cpu_percent: 8,
      memory_used_mb: 1024,
      memory_total_mb: 8192,
      memory_percent: 12.5,
      uptime_seconds: 3600,
      active_jobs: 0,
      concurrent_limit: 1,
      cpu_count: 8,
      gpu_utilization: 0,
      gpu_memory_percent: 0,
    });
  });

  await page.route("**/api/settings/bundle", async (route) => {
    await fulfillJson(route, 200, {
      settings: {
        scanner: {
          directories: ["/media/movies"],
          watch_enabled: true,
          extra_watch_dirs: [],
        },
        notifications: { targets: [] },
        schedule: { windows: [] },
      },
    });
  });
}

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
});

test("unknown route shows 404 page", async ({ page }) => {
  await page.goto("/this-page-does-not-exist");

  await expect(page.getByText("404")).toBeVisible();
  await expect(page.getByText("This page doesn't exist")).toBeVisible();
  await expect(page.getByRole("link", { name: /Go to Dashboard/i })).toBeVisible();
});

test("404 page dashboard link navigates home", async ({ page }) => {
  await mockDashboard(page);
  await page.goto("/this-page-does-not-exist");

  await page.getByRole("link", { name: /Go to Dashboard/i }).click();

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("heading", { name: "Recent Activity" })).toBeVisible();
});
