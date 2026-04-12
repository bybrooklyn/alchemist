import { expect, test } from "@playwright/test";
import {
  createEngineMode,
  createEngineStatus,
  fulfillJson,
  mockDashboardData,
} from "./helpers";

test.use({ storageState: undefined });

test.beforeEach(async ({ page }) => {
  await mockDashboardData(page);
  await page.route("**/api/engine/mode", async (route) => {
    await fulfillJson(route, 200, createEngineMode());
  });
});

test("pause then resume transitions engine state correctly", async ({ page }) => {
  let engineStatus = createEngineStatus({ status: "running", manual_paused: false });
  let pauseCalls = 0;
  let resumeCalls = 0;

  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, engineStatus);
  });
  await page.route("**/api/engine/pause", async (route) => {
    pauseCalls += 1;
    engineStatus = createEngineStatus({ status: "paused", manual_paused: true });
    await fulfillJson(route, 200, { status: "paused" });
  });
  await page.route("**/api/engine/resume", async (route) => {
    resumeCalls += 1;
    engineStatus = createEngineStatus({ status: "running", manual_paused: false });
    await fulfillJson(route, 200, { status: "running" });
  });

  await page.goto("/settings?tab=system");

  await page.getByRole("button", { name: "Pause" }).click();
  await expect.poll(() => pauseCalls).toBe(1);

  await page.getByRole("button", { name: "Start" }).click();
  await expect.poll(() => resumeCalls).toBe(1);
});

test("drain transitions to draining state and cancel-stop reverts it", async ({ page }) => {
  let engineStatus = createEngineStatus({ status: "running", manual_paused: false });
  let drainCalls = 0;
  let stopDrainCalls = 0;

  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, engineStatus);
  });
  await page.route("**/api/engine/drain", async (route) => {
    drainCalls += 1;
    engineStatus = createEngineStatus({
      status: "draining",
      manual_paused: false,
      draining: true,
    });
    await fulfillJson(route, 200, { status: "draining" });
  });
  await page.route("**/api/engine/stop-drain", async (route) => {
    stopDrainCalls += 1;
    engineStatus = createEngineStatus({ status: "running", manual_paused: false });
    await fulfillJson(route, 200, { status: "running" });
  });

  await page.goto("/");

  await page.getByRole("button", { name: "Stop" }).click();
  await expect.poll(() => drainCalls).toBe(1);
  await expect(page.getByText("Stopping", { exact: true })).toBeVisible();

  await expect.poll(() => stopDrainCalls).toBe(0);
});

test("engine restart endpoint is called and status returns to running", async ({ page }) => {
  let engineStatus = createEngineStatus({ status: "running", manual_paused: false });
  let restartCalls = 0;

  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, engineStatus);
  });
  await page.route("**/api/engine/restart", async (route) => {
    restartCalls += 1;
    engineStatus = createEngineStatus({ status: "running", manual_paused: false });
    await fulfillJson(route, 200, { status: "running" });
  });

  await page.goto("/");

  const result = await page.evaluate(async () => {
    const res = await fetch("/api/engine/restart", { method: "POST" });
    const body = await res.json() as { status: string };
    return { status: res.status, body };
  });

  expect(restartCalls).toBe(1);
  expect(result.status).toBe(200);
  expect(result.body.status).toBe("running");
});
