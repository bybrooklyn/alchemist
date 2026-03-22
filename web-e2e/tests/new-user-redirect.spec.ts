import { expect, test, type Page } from "@playwright/test";
import { fulfillJson } from "./helpers";

function bundleResponse(directories: string[]) {
  return {
    settings: {
      appearance: { active_theme_id: "helios-orange" },
      scanner: { directories, watch_enabled: true, extra_watch_dirs: [] },
      transcode: {
        concurrent_jobs: 2,
        size_reduction_threshold: 0.3,
        min_bpp_threshold: 0.1,
        min_file_size_mb: 100,
        output_codec: "av1",
        quality_profile: "balanced",
        allow_fallback: true,
        subtitle_mode: "copy",
      },
      hardware: {
        allow_cpu_encoding: true,
        allow_cpu_fallback: true,
        preferred_vendor: null,
        cpu_preset: "medium",
        device_path: null,
      },
      files: {
        delete_source: false,
        output_extension: "mkv",
        output_suffix: "-alchemist",
        replace_strategy: "keep",
        output_root: null,
      },
      quality: {
        enable_vmaf: false,
        min_vmaf_score: 90,
        revert_on_low_quality: true,
      },
      notifications: { enabled: false, targets: [] },
      schedule: { windows: [] },
      system: { enable_telemetry: false, monitoring_poll_interval: 2 },
    },
  };
}

async function mockHomePage(
  page: Page,
  options: { directories: string[]; setupCompleteValue?: string },
): Promise<void> {
  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, { status: "running" });
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
    await fulfillJson(route, 200, bundleResponse(options.directories));
  });

  await page.route("**/api/settings/preferences/setup_complete", async (route) => {
    if (options.setupCompleteValue == null) {
      await fulfillJson(route, 404, { message: "not found" });
      return;
    }

    await fulfillJson(route, 200, {
      key: "setup_complete",
      value: options.setupCompleteValue,
    });
  });

  await page.route("**/api/setup/status", async (route) => {
    await fulfillJson(route, 200, {
      setup_required: true,
      enable_telemetry: false,
    });
  });

  await page.route("**/api/system/hardware", async (route) => {
    await fulfillJson(route, 200, {
      vendor: "Cpu",
      device_path: null,
      supported_codecs: ["h264", "hevc", "av1"],
    });
  });

  await page.route("**/api/fs/recommendations", async (route) => {
    await fulfillJson(route, 200, { recommendations: [] });
  });
}

test("redirects to setup when no directories configured", async ({ page }) => {
  await mockHomePage(page, { directories: [] });

  await page.goto("/");

  await expect(page).toHaveURL(/\/setup$/);
  await expect(page.getByRole("heading", { name: "Alchemist Setup" })).toBeVisible();
});

test("does not redirect when directories are configured", async ({ page }) => {
  await mockHomePage(page, { directories: ["/media/movies"] });

  await page.goto("/");

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();
});

test("does not redirect when setup is already complete", async ({ page }) => {
  await mockHomePage(page, { directories: [], setupCompleteValue: "true" });

  await page.goto("/");

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();
});
