import { expect, test } from "@playwright/test";
import { expectVisibleError, fulfillJson, mockEngineStatus } from "./helpers";

const jobsTable = [
  {
    id: 1,
    input_path: "/media/sample.mkv",
    output_path: "/output/sample-av1.mkv",
    status: "completed",
    priority: 0,
    progress: 100,
    created_at: "2025-01-01T00:00:00Z",
    updated_at: "2025-01-01T00:00:00Z",
    vmaf_score: 95,
    decision_reason: "Good candidate",
  },
];

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);

  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, jobsTable);
  });
});

test("jobs batch delete failure is surfaced to the user", async ({ page }) => {
  await page.route("**/api/jobs/batch", async (route) => {
    await fulfillJson(route, 500, { message: "forced batch failure" });
  });

  await page.goto("/jobs");
  await expect(page.getByTitle("/media/sample.mkv")).toBeVisible();

  await page.locator("tbody input[type='checkbox']").first().check();
  await page.getByTitle("Delete").first().click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Delete" }).click();

  await expectVisibleError(page, "forced batch failure");
});

test("clear completed failure is surfaced to the user", async ({ page }) => {
  await page.route("**/api/jobs/clear-completed", async (route) => {
    await fulfillJson(route, 500, { message: "forced clear-completed failure" });
  });

  await page.goto("/jobs");
  await expect(page.getByTitle("/media/sample.mkv")).toBeVisible();

  await page.getByRole("button", { name: /Clear Completed/i }).click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Clear" }).click();

  await expectVisibleError(page, "forced clear-completed failure");
});

test("single job delete failure is surfaced to the user", async ({ page }) => {
  await page.route("**/api/jobs/1/delete", async (route) => {
    await fulfillJson(route, 500, { message: "forced single-job failure" });
  });

  await page.goto("/jobs");
  await expect(page.getByTitle("/media/sample.mkv")).toBeVisible();

  await page.getByTitle("Actions").first().click();
  await page.getByRole("button", { name: /^Delete$/ }).first().click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Delete" }).click();

  await expectVisibleError(page, "forced single-job failure");
});

test("log clear failure is surfaced to the user", async ({ page }) => {
  await page.route("**/api/logs/history**", async (route) => {
    await fulfillJson(route, 200, [
      {
        id: 1,
        level: "info",
        message: "hello",
        created_at: "2025-01-01T00:00:00Z",
      },
    ]);
  });

  await page.route("**/api/logs", async (route) => {
    if (route.request().method() === "DELETE") {
      await fulfillJson(route, 500, { message: "forced log clear failure" });
      return;
    }
    await route.continue();
  });

  await page.goto("/logs");
  await expect(page.getByText("Server Logs")).toBeVisible();

  await page.getByTitle("Clear Server Logs").click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Clear Logs" }).click();

  await expectVisibleError(page, "forced log clear failure");
});
