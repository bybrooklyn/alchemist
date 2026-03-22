import { expect, test } from "@playwright/test";
import { fulfillJson, mockEngineStatus } from "./helpers";

interface JobsRequestSnapshot {
  archived: string | null;
  sortBy: string | null;
  sortDesc: string | null;
  status: string | null;
}

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
});

test("skipped tab shows only skipped jobs", async ({ page }) => {
  const requests: JobsRequestSnapshot[] = [];

  await page.route("**/api/jobs/table**", async (route) => {
    const url = new URL(route.request().url());
    requests.push({
      archived: url.searchParams.get("archived"),
      sortBy: url.searchParams.get("sort_by"),
      sortDesc: url.searchParams.get("sort_desc"),
      status: url.searchParams.get("status"),
    });
    await fulfillJson(route, 200, []);
  });

  await page.goto("/jobs");
  const skippedTab = page.getByRole("button", { name: /^Skipped$/i });
  await skippedTab.click();

  await expect(skippedTab).toHaveClass(/bg-helios-surface-soft/);
  await expect.poll(() => requests.some((request) => request.status === "skipped")).toBe(true);
  await expect(page.getByText("No jobs found")).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("archived tab shows archived jobs", async ({ page }) => {
  const requests: JobsRequestSnapshot[] = [];

  await page.route("**/api/jobs/table**", async (route) => {
    const url = new URL(route.request().url());
    requests.push({
      archived: url.searchParams.get("archived"),
      sortBy: url.searchParams.get("sort_by"),
      sortDesc: url.searchParams.get("sort_desc"),
      status: url.searchParams.get("status"),
    });
    await fulfillJson(route, 200, []);
  });

  await page.goto("/jobs");
  await page.getByRole("button", { name: /^Archived$/i }).click();

  await expect.poll(() => requests.some((request) => request.archived === "true")).toBe(true);
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("sort controls change the request params", async ({ page }) => {
  const requests: JobsRequestSnapshot[] = [];

  await page.route("**/api/jobs/table**", async (route) => {
    const url = new URL(route.request().url());
    requests.push({
      archived: url.searchParams.get("archived"),
      sortBy: url.searchParams.get("sort_by"),
      sortDesc: url.searchParams.get("sort_desc"),
      status: url.searchParams.get("status"),
    });
    await fulfillJson(route, 200, []);
  });

  await page.goto("/jobs");

  const sortSelect = page.getByRole("combobox").first();
  await expect(sortSelect).toBeVisible();
  await sortSelect.selectOption("input_path");

  await expect.poll(() =>
    requests.some((request) => request.sortBy === "input_path" && request.sortDesc === "true")
  ).toBe(true);

  await page.getByRole("button", { name: "Sort descending" }).click();

  await expect.poll(() =>
    requests.some((request) => request.sortBy === "input_path" && request.sortDesc === "false")
  ).toBe(true);
});

test("default sort is last updated descending", async ({ page }) => {
  const requests: JobsRequestSnapshot[] = [];

  await page.route("**/api/jobs/table**", async (route) => {
    const url = new URL(route.request().url());
    requests.push({
      archived: url.searchParams.get("archived"),
      sortBy: url.searchParams.get("sort_by"),
      sortDesc: url.searchParams.get("sort_desc"),
      status: url.searchParams.get("status"),
    });
    await fulfillJson(route, 200, []);
  });

  await page.goto("/jobs");

  await expect.poll(() => requests.length).toBeGreaterThan(0);
  expect(requests[0]).toMatchObject({
    sortBy: "updated_at",
    sortDesc: "true",
  });
});
