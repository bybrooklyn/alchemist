import { expect, test } from "@playwright/test";
import { fulfillJson, mockEngineStatus } from "./helpers";

test("dashboard uses a single stats poller and handles visibility changes", async ({ page, context }) => {
  const statsCallTimes: number[] = [];

  await mockEngineStatus(page);

  await page.route("**/api/stats", async (route) => {
    statsCallTimes.push(Date.now());
    await fulfillJson(route, 200, {
      active: 1,
      concurrent_limit: 2,
      completed: 10,
      failed: 0,
      total: 11,
    });
  });

  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, []);
  });

  await page.route("**/api/settings/system", async (route) => {
    await fulfillJson(route, 200, {
      monitoring_poll_interval: 2,
      enable_telemetry: false,
    });
  });

  await page.route("**/api/system/resources", async (route) => {
    await fulfillJson(route, 200, {
      cpu_percent: 12,
      memory_used_mb: 2048,
      memory_total_mb: 8192,
      memory_percent: 25,
      uptime_seconds: 3600,
      active_jobs: 1,
      concurrent_limit: 2,
      cpu_count: 8,
      gpu_utilization: 0,
      gpu_memory_percent: 0,
    });
  });

  await page.goto("/");
  await expect
    .poll(() => statsCallTimes.length, { timeout: 10_000 })
    .toBeGreaterThanOrEqual(1);

  await page.waitForTimeout(6_200);
  expect(statsCallTimes.length).toBeGreaterThanOrEqual(2);
  expect(statsCallTimes.length).toBeLessThanOrEqual(3);

  const helperPage = await context.newPage();
  await helperPage.goto("about:blank");
  await helperPage.bringToFront();

  const beforeHidden = statsCallTimes.length;
  await helperPage.waitForTimeout(7_000);
  const hiddenDelta = statsCallTimes.length - beforeHidden;
  expect(hiddenDelta).toBeLessThanOrEqual(1);

  await page.bringToFront();
  const beforeRefocus = statsCallTimes.length;
  await page.waitForTimeout(6_200);
  expect(statsCallTimes.length).toBeGreaterThan(beforeRefocus);

  await helperPage.close();
});
