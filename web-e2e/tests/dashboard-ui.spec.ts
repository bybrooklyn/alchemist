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

test("Queue ETA panel shows aggregate estimate", async ({ page }) => {
  await page.unroute("**/api/stats/queue-eta");
  await page.route("**/api/stats/queue-eta", async (route) => {
    await fulfillJson(route, 200, {
      remaining_jobs: 3,
      est_seconds_remaining: 3600,
      sample_size: 4,
    });
  });

  await page.goto("/");

  await expect(page.getByText("Queue ETA")).toBeVisible();
  await expect(page.getByText("3 jobs remaining")).toBeVisible();
  await expect(page.getByText("About 1h 0m left")).toBeVisible();
  await expect(page.getByText("Based on 4 recent completed jobs.")).toBeVisible();
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

test("mobile dashboard prioritizes active jobs", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await page.unroute("**/api/jobs/table**");
  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, [
      {
        id: 41,
        input_path: "/media/active-now.mkv",
        output_path: "/output/active-now.mkv",
        status: "encoding",
        priority: 10,
        progress: 42,
        created_at: "2026-05-18T10:00:00Z",
        updated_at: "2026-05-18T10:05:00Z",
      },
    ]);
  });

  await page.goto("/");

  const activeNow = page.getByRole("region", { name: "Active Now" });
  await expect(activeNow).toBeVisible();
  await expect(activeNow.getByText("active-now.mkv")).toBeVisible();
  await expect(activeNow.getByText("42%")).toBeVisible();
  await expect(page.getByText("Total Processed")).not.toBeVisible();
});

test("About modal opens and does not contain Al badge", async ({ page }) => {
  await page.route("**/api/system/info", async (route) => {
    await fulfillJson(route, 200, {
      version: "0.3.0",
      os_version: "macos aarch64",
      is_docker: false,
      telemetry_enabled: false,
      ffmpeg_version: "N-12345",
      cpu_count: 8,
      total_memory_gb: 16,
    });
  });
  await page.route("**/api/system/update", async (route) => {
    await fulfillJson(route, 200, {
      current_version: "0.3.0",
      channel: "stable",
      latest_version: "0.3.1",
      update_available: true,
      release_url: "https://github.com/bybrooklyn/alchemist/releases/tag/v0.3.1",
      install_type: "direct_binary",
      can_self_update: false,
      action: "guided",
      guidance: null,
      guidance_command: null,
      verification_status: "verified",
      verification_error: null,
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: "About" }).click();

  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Alchemist" })).toBeVisible();
  await expect(page.getByText("v0.3.0").first()).toBeVisible();
  await expect(page.getByText("v0.3.1", { exact: false }).first()).toBeVisible();
  await expect(page.getByText(/^Al$/)).toHaveCount(0);
});

test("System Status modal exits cleanly by close button and Escape", async ({ page }) => {
  await page.goto("/");

  const engineStatus = page.getByText("Engine Status", { exact: true });
  const dialog = page.getByRole("dialog", { name: "System Status" });

  await engineStatus.click();
  await expect(dialog).toBeVisible();
  await page.getByRole("button", { name: "Close system status dialog" }).click();
  await expect(dialog).toHaveCount(0);

  await engineStatus.click();
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});

test("Settings page does not show Setup & Runtime Controls banner", async ({ page }) => {
  await page.goto("/settings");

  await expect(page.getByText("Setup & Runtime Controls")).not.toBeVisible();
});
