import { expect, test } from "@playwright/test";
import { fulfillJson, mockEngineStatus } from "./helpers";

interface JobsRequestSnapshot {
  archived: string | null;
  search: string | null;
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
      search: url.searchParams.get("search"),
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
      search: url.searchParams.get("search"),
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
      search: url.searchParams.get("search"),
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
      search: url.searchParams.get("search"),
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

test("saved job views apply filters and persist custom views", async ({ page }) => {
  const requests: JobsRequestSnapshot[] = [];
  let persistedValue = "";

  await page.route("**/api/settings/preferences/saved_job_views", async (route) => {
    await fulfillJson(route, 200, {
      key: "saved_job_views",
      value: JSON.stringify([
        {
          id: "custom-needs-review",
          label: "Needs Review",
          activeTab: "failed",
          sortBy: "input_path",
          sortDesc: false,
          search: "failed",
        },
      ]),
    });
  });
  await page.route("**/api/settings/preferences", async (route) => {
    const body = route.request().postDataJSON() as { key: string; value: string };
    persistedValue = body.value;
    await fulfillJson(route, 200, body);
  });
  await page.route("**/api/jobs/table**", async (route) => {
    const url = new URL(route.request().url());
    requests.push({
      archived: url.searchParams.get("archived"),
      search: url.searchParams.get("search"),
      sortBy: url.searchParams.get("sort_by"),
      sortDesc: url.searchParams.get("sort_desc"),
      status: url.searchParams.get("status"),
    });
    await fulfillJson(route, 200, []);
  });

  await page.goto("/jobs");
  await page.getByRole("button", { name: "Needs Review", exact: true }).click();

  await expect.poll(() =>
    requests.some((request) =>
      request.status === "failed,cancelled" &&
      request.sortBy === "input_path" &&
      request.sortDesc === "false" &&
      request.search === "failed"
    )
  ).toBe(true);

  page.once("dialog", async (dialog) => {
    await dialog.accept(" Review Queue ");
  });
  await page.getByRole("button", { name: /Save View/i }).click();

  await expect(page.getByRole("button", { name: "Review Queue", exact: true })).toBeVisible();
  await expect.poll(() => {
    if (!persistedValue) {
      return false;
    }
    const views = JSON.parse(persistedValue) as Array<{ label?: string; search?: string }>;
    return views.some((view) => view.label === "Review Queue" && view.search === "failed");
  }).toBe(true);
});
