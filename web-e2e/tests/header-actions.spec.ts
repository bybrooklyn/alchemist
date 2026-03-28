import { expect, test } from "@playwright/test";
import { createEngineStatus, fulfillJson, mockDashboardData } from "./helpers";

test.use({ storageState: undefined });

test("start, stop, cancel stop, and pause update dashboard engine controls", async ({
  page,
}) => {
  let engineStatus = createEngineStatus({
    status: "paused",
    manual_paused: true,
    scheduler_paused: false,
    draining: false,
  });
  let resumeCalls = 0;
  let pauseCalls = 0;
  let drainCalls = 0;
  let stopDrainCalls = 0;

  await mockDashboardData(page);

  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, engineStatus);
  });
  await page.route("**/api/engine/mode", async (route) => {
    await fulfillJson(route, 200, {
      mode: engineStatus.mode,
      is_manual_override: engineStatus.is_manual_override,
      concurrent_limit: engineStatus.concurrent_limit,
      cpu_count: 8,
      computed_limits: {
        background: 1,
        balanced: 4,
        throughput: 4,
      },
    });
  });
  await page.route("**/api/engine/resume", async (route) => {
    resumeCalls += 1;
    engineStatus = createEngineStatus({
      status: "running",
      manual_paused: false,
      scheduler_paused: false,
      draining: false,
    });
    await fulfillJson(route, 200, { status: "running" });
  });
  await page.route("**/api/engine/pause", async (route) => {
    pauseCalls += 1;
    engineStatus = createEngineStatus({
      status: "paused",
      manual_paused: true,
      scheduler_paused: false,
      draining: false,
    });
    await fulfillJson(route, 200, { status: "paused" });
  });
  await page.route("**/api/engine/drain", async (route) => {
    drainCalls += 1;
    engineStatus = createEngineStatus({
      status: "draining",
      manual_paused: false,
      scheduler_paused: false,
      draining: true,
    });
    await fulfillJson(route, 200, { status: "draining" });
  });
  await page.route("**/api/engine/stop-drain", async (route) => {
    stopDrainCalls += 1;
    engineStatus = createEngineStatus({
      status: "running",
      manual_paused: false,
      scheduler_paused: false,
      draining: false,
    });
    await fulfillJson(route, 200, { status: "running" });
  });

  await page.goto("/");

  await expect(page.getByText("Paused", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Start" }).click();
  await expect.poll(() => resumeCalls).toBe(1);
  await expect(page.getByText("Running", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Pause" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Stop" })).toBeVisible();

  await page.getByRole("button", { name: "Stop" }).click();
  await expect.poll(() => drainCalls).toBe(1);
  await expect(page.getByText("Draining", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Cancel Stop" })).toBeVisible();

  await page.getByRole("button", { name: "Cancel Stop" }).click();
  await expect.poll(() => stopDrainCalls).toBe(1);
  await expect(page.getByText("Running", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Pause" }).click();
  await expect.poll(() => pauseCalls).toBe(1);
  await expect(page.getByText("Paused", { exact: true })).toBeVisible();
});

test("scheduler pause note is shown when pause comes from automation", async ({ page }) => {
  await mockDashboardData(page);
  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(
      route,
      200,
      createEngineStatus({
        status: "paused",
        manual_paused: false,
        scheduler_paused: true,
      }),
    );
  });
  await page.route("**/api/engine/mode", async (route) => {
    await fulfillJson(route, 200, {
      mode: "balanced",
      is_manual_override: false,
      concurrent_limit: 2,
      cpu_count: 8,
      computed_limits: {
        background: 1,
        balanced: 4,
        throughput: 4,
      },
    });
  });

  await page.goto("/");

  await expect(page.getByText("(schedule)")).toBeVisible();
});

test("failed engine transitions surface an error toast", async ({ page }) => {
  await mockDashboardData(page);
  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, createEngineStatus());
  });
  await page.route("**/api/engine/mode", async (route) => {
    await fulfillJson(route, 200, {
      mode: "balanced",
      is_manual_override: false,
      concurrent_limit: 2,
      cpu_count: 8,
      computed_limits: {
        background: 1,
        balanced: 4,
        throughput: 4,
      },
    });
  });
  await page.route("**/api/engine/resume", async (route) => {
    await fulfillJson(route, 500, { message: "resume failed" });
  });

  await page.goto("/");
  await page.getByRole("button", { name: "Start" }).click();

  await expect(page.getByText("Failed to update engine state.").first()).toBeVisible();
  await expect(page.getByText("Paused", { exact: true })).toBeVisible();
});
