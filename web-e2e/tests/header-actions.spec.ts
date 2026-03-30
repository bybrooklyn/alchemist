import { expect, test } from "@playwright/test";
import {
  createEngineMode,
  createEngineStatus,
  fulfillJson,
  mockDashboardData,
} from "./helpers";

test.use({ storageState: undefined });

test("engine cycle: Start transitions to Running, Stop transitions to Stopping", async ({
  page,
}) => {
  let engineStatus = createEngineStatus({
    status: "paused",
    manual_paused: true,
    draining: false,
  });
  let resumeCalls = 0;
  let drainCalls = 0;

  await mockDashboardData(page);

  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, engineStatus);
  });
  await page.route("**/api/engine/mode", async (route) => {
    await fulfillJson(route, 200, createEngineMode());
  });
  await page.route("**/api/engine/resume", async (route) => {
    resumeCalls += 1;
    engineStatus = createEngineStatus({
      status: "running",
      manual_paused: false,
      draining: false,
    });
    await fulfillJson(route, 200, { status: "running" });
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

  await page.goto("/");

  await expect(page.getByText("Paused", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Start" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Pause" })).not.toBeVisible();
  await expect(page.getByRole("button", { name: "Cancel Stop" })).not.toBeVisible();

  await page.getByRole("button", { name: "Start" }).click();
  await expect.poll(() => resumeCalls).toBe(1);
  await expect(page.getByText("Running", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Stop" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Pause" })).not.toBeVisible();

  await page.getByRole("button", { name: "Stop" }).click();
  await expect.poll(() => drainCalls).toBe(1);
  await expect(page.getByText("Stopping", { exact: true })).toBeVisible();
  await expect(page.getByText("Draining", { exact: true })).not.toBeVisible();
  await expect(page.getByRole("button", { name: "Cancel Stop" })).not.toBeVisible();
  await expect(page.getByRole("button", { name: /Stopping/i })).toBeDisabled();
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
    await fulfillJson(route, 200, createEngineMode());
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
    await fulfillJson(route, 200, createEngineMode());
  });
  await page.route("**/api/engine/resume", async (route) => {
    await fulfillJson(route, 500, { message: "resume failed" });
  });

  await page.goto("/");
  await page.getByRole("button", { name: "Start" }).click();

  await expect(page.getByText("Failed to update engine state.").first()).toBeVisible();
  await expect(page.getByText("Paused", { exact: true })).toBeVisible();
});
