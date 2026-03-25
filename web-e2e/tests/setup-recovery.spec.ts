import { expect, test } from "@playwright/test";
import { fulfillJson } from "./helpers";

test("setup step 5 shows retry and back recovery on scan failures", async ({ page }) => {
  let scanStartAttempts = 0;

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

  await page.route("**/api/settings/bundle", async (route) => {
    await fulfillJson(route, 200, {
      settings: {
        appearance: { active_theme_id: "helios-orange" },
        scanner: { directories: [], watch_enabled: true, extra_watch_dirs: [] },
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
    });
  });

  await page.route("**/api/fs/recommendations", async (route) => {
    await fulfillJson(route, 200, {
      recommendations: [
        {
          path: "/srv/media",
          label: "media",
          reason: "Looks like a media library",
          media_hint: "high",
        },
      ],
    });
  });

  await page.route("**/api/fs/preview", async (route) => {
    await fulfillJson(route, 200, {
      directories: [
        {
          path: "/srv/media",
          exists: true,
          readable: true,
          media_files: 5,
          sample_files: ["/srv/media/movie.mkv"],
          media_hint: "high",
          warnings: [],
        },
      ],
      total_media_files: 5,
      warnings: [],
    });
  });

  await page.route("**/api/setup/complete", async (route) => {
    await fulfillJson(route, 200, { status: "ok" });
  });

  await page.route("**/api/scan/start", async (route) => {
    scanStartAttempts += 1;
    if (scanStartAttempts < 3) {
      await fulfillJson(route, 500, { message: "forced scan start failure" });
      return;
    }
    await route.fulfill({ status: 202, body: "" });
  });

  await page.route("**/api/scan/status", async (route) => {
    await fulfillJson(route, 200, {
      is_running: false,
      files_found: 1,
      files_added: 1,
      current_folder: null,
    });
  });

  await page.goto("/setup");
  await expect(page.getByPlaceholder("admin")).toBeVisible();

  await page.getByPlaceholder("admin").fill("playwright");
  await page.getByPlaceholder("Choose a strong password").fill("playwright-password");
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByPlaceholder("/path/to/media").fill("/srv/media");
  await page.getByRole("button", { name: /^Add$/ }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "Final Review" })).toBeVisible();
  await page.getByRole("button", { name: "Complete Setup" }).click();

  await expect(page.getByText("Scan failed or became unavailable.")).toBeVisible();
  await expect(page.getByText("forced scan start failure")).toBeVisible();

  await page.getByRole("button", { name: "Back to Review" }).click();
  await expect(page.getByRole("heading", { name: "Final Review" })).toBeVisible();

  await page.getByRole("button", { name: "Complete Setup" }).click();
  await expect(page.getByText("Scan failed or became unavailable.")).toBeVisible();

  await page.getByRole("button", { name: "Retry Scan" }).click();
  await expect(page.getByRole("button", { name: "Enter Dashboard" })).toBeVisible();
  await expect(scanStartAttempts).toBe(3);
});

test("setup submits h264 as a valid output codec", async ({ page }) => {
  let submittedBody: Record<string, unknown> | null = null;

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

  await page.route("**/api/settings/bundle", async (route) => {
    await fulfillJson(route, 200, {
      settings: {
        appearance: { active_theme_id: "helios-orange" },
        scanner: { directories: [], watch_enabled: true, extra_watch_dirs: [] },
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
    });
  });

  await page.route("**/api/fs/recommendations", async (route) => {
    await fulfillJson(route, 200, { recommendations: [] });
  });

  await page.route("**/api/fs/preview", async (route) => {
    await fulfillJson(route, 200, {
      directories: [
        {
          path: "/srv/media",
          exists: true,
          readable: true,
          media_files: 1,
          sample_files: ["/srv/media/movie.mkv"],
          media_hint: "high",
          warnings: [],
        },
      ],
      total_media_files: 1,
      warnings: [],
    });
  });

  await page.route("**/api/setup/complete", async (route) => {
    submittedBody = route.request().postDataJSON() as Record<string, unknown>;
    await fulfillJson(route, 200, { status: "ok" });
  });

  await page.route("**/api/scan/start", async (route) => {
    await route.fulfill({ status: 202, body: "" });
  });

  await page.route("**/api/scan/status", async (route) => {
    await fulfillJson(route, 200, {
      is_running: false,
      files_found: 0,
      files_added: 0,
      current_folder: null,
    });
  });

  await page.goto("/setup");
  await page.getByPlaceholder("admin").fill("playwright");
  await page.getByPlaceholder("Choose a strong password").fill("playwright-password");
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByPlaceholder("/path/to/media").fill("/srv/media");
  await page.getByRole("button", { name: /^Add$/ }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "H.264" }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Complete Setup" }).click();
  await expect(page.getByRole("button", { name: "Enter Dashboard" })).toBeVisible();

  expect((submittedBody?.settings as { transcode?: { output_codec?: string } })?.transcode?.output_codec).toBe("h264");
});
