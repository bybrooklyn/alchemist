import { expect, test, type Page } from "@playwright/test";
import { fulfillJson, mockEngineStatus } from "./helpers";

async function patchBrokenRechartsChunk(page: Page): Promise<void> {
  await page.route("**/_astro/BarChart*.js", async (route) => {
    const response = await route.fetch();
    const original = await response.text();
    const patched = original.replace(
      "const re={any:Lr(),array:Lr(),element:Lr(),object:Lr(),oneOfType:()=>Lr()},",
      "const re={any:Lr(),array:Lr(),arrayOf:()=>Lr(),bool:Lr(),element:Lr(),func:Lr(),node:Lr(),number:Lr(),object:Lr(),oneOf:()=>Lr(),oneOfType:()=>Lr(),shape:()=>Lr(),string:Lr()},",
    );

    await route.fulfill({
      response,
      body: patched,
      contentType: "application/javascript",
    });
  });
}

async function mockStatsPage(
  page: Page,
  savingsSummary: {
    total_input_bytes: number;
    total_output_bytes: number;
    total_bytes_saved: number;
    savings_percent: number;
    job_count: number;
    savings_by_codec: Array<{ codec: string; bytes_saved: number; job_count: number }>;
    savings_over_time: Array<{ date: string; bytes_saved: number }>;
  },
): Promise<void> {
  await page.route("**/api/stats/**", async (route) => {
    const pathname = new URL(route.request().url()).pathname;

    if (pathname.endsWith("/api/stats/savings")) {
      await fulfillJson(route, 200, savingsSummary);
      return;
    }

    if (pathname.endsWith("/api/stats/aggregated")) {
      await fulfillJson(route, 200, {
        total_input_bytes: savingsSummary.total_input_bytes,
        total_output_bytes: savingsSummary.total_output_bytes,
        total_savings_bytes: savingsSummary.total_bytes_saved,
        total_time_seconds: 7200,
        total_jobs: savingsSummary.job_count,
        avg_vmaf: 94.2,
      });
      return;
    }

    if (pathname.endsWith("/api/stats/daily")) {
      await fulfillJson(
        route,
        200,
        savingsSummary.savings_over_time.map((entry) => ({
          date: entry.date,
          jobs_completed: Math.max(1, savingsSummary.job_count),
          bytes_saved: entry.bytes_saved,
          total_input_bytes: savingsSummary.total_input_bytes,
          total_output_bytes: savingsSummary.total_output_bytes,
        })),
      );
      return;
    }

    if (pathname.endsWith("/api/stats/detailed")) {
      await fulfillJson(route, 200, []);
      return;
    }

    await route.continue();
  });
}

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
  await patchBrokenRechartsChunk(page);
});

test("savings overview renders on stats page", async ({ page }) => {
  await mockStatsPage(page, {
    total_input_bytes: 2_000_000_000_000,
    total_output_bytes: 1_200_000_000_000,
    total_bytes_saved: 800_000_000_000,
    savings_percent: 40,
    job_count: 24,
    savings_by_codec: [
      { codec: "AV1", bytes_saved: 500_000_000_000, job_count: 16 },
      { codec: "HEVC", bytes_saved: 300_000_000_000, job_count: 8 },
    ],
    savings_over_time: [
      { date: "2026-03-20", bytes_saved: 200_000_000_000 },
      { date: "2026-03-21", bytes_saved: 300_000_000_000 },
    ],
  });

  await page.goto("/stats");

  await expect(page.getByText("Total saved")).toBeVisible();
  await expect(page.getByText(/saved across/i)).toBeVisible();
  await expect(page.getByText("Unable to load storage savings.")).toHaveCount(0);
});

test("savings overview shows empty state when no data", async ({ page }) => {
  await mockStatsPage(page, {
    total_input_bytes: 0,
    total_output_bytes: 0,
    total_bytes_saved: 0,
    savings_percent: 0,
    job_count: 0,
    savings_by_codec: [],
    savings_over_time: [],
  });

  await page.goto("/stats");

  await expect(page.getByText(/No transcoding data yet/i).first()).toBeVisible();
});
