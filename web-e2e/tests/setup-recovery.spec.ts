import { expect, test } from "@playwright/test";
import { fulfillJson } from "./helpers";

test("setup shows a persistent inline alert and disables telemetry", async ({ page }) => {
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

  await page.goto("/setup");
  await page.getByRole("button", { name: "Get Started" }).click();

  await expect(page.getByLabel("Anonymous Usage Telemetry")).toBeDisabled();
  await expect(page.getByText("Temporarily unavailable while Alembic stabilizes. Telemetry stays off for now.")).toBeVisible();

  await page.getByPlaceholder("admin").fill("playwright");
  await page.getByPlaceholder("Choose a strong password").fill("playwright-password");
  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Library Selection" })).toBeVisible();
  await page.getByRole("button", { name: "Next" }).click();

  const alert = page.getByRole("alert").first();
  await expect(alert).toBeVisible();
  await expect(alert).toContainText("Select at least one server folder before continuing.");
});

test("setup completes directly without an intermediate scan step", async ({ page }) => {
  let scanStartCalls = 0;

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
    scanStartCalls += 1;
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
  await page.getByRole("button", { name: "Get Started" }).click();
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

  await page.waitForURL((url) => !url.pathname.includes("/setup"));
  await expect(page.getByRole("button", { name: "Enter Dashboard" })).toHaveCount(0);
  await expect(page.getByText("Scan failed or became unavailable.")).toHaveCount(0);
  expect(scanStartCalls).toBe(0);
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
  await page.route("**/api/settings/preferences", async (route) => {
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
  await page.getByRole("button", { name: "Get Started" }).click();
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
  await page.waitForURL((url) => !url.pathname.includes("/setup"));

  expect((submittedBody?.settings as { transcode?: { output_codec?: string } })?.transcode?.output_codec).toBe("h264");
});
