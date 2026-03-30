import { expect, test } from "@playwright/test";
import {
  createSettingsBundle,
  expectVisibleError,
  fulfillJson,
  mockEngineStatus,
} from "./helpers";

const transcodeSettings = {
  concurrent_jobs: 2,
  size_reduction_threshold: 0.3,
  min_bpp_threshold: 0.1,
  min_file_size_mb: 100,
  output_codec: "av1",
  quality_profile: "balanced",
  threads: 0,
  allow_fallback: true,
  hdr_mode: "preserve",
  tonemap_algorithm: "hable",
  tonemap_peak: 100,
  tonemap_desat: 0.2,
};

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
});

test("appearance preference save failure is visible", async ({ page }) => {
  await page.route("**/api/ui/preferences", async (route) => {
    await fulfillJson(route, 500, { message: "forced appearance failure" });
  });

  await page.goto("/settings?tab=appearance");
  await page.getByRole("button", { name: /Sunset/i }).first().click();

  await expectVisibleError(page, "Unable to save theme preference to server.");
});

test("file settings save failure is visible", async ({ page }) => {
  await page.route("**/api/settings/files", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, {
        delete_source: false,
        output_extension: "mkv",
        output_suffix: "-alchemist",
        replace_strategy: "keep",
        output_root: null,
      });
      return;
    }
    await fulfillJson(route, 500, { message: "forced files failure" });
  });

  await page.goto("/settings?tab=files");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await expectVisibleError(page, "forced files failure");
  await expect(page.getByText("File settings saved.")).toHaveCount(0);
});

test("file settings output root round-trips through save payload", async ({ page }) => {
  let savedBody: Record<string, unknown> | null = null;

  await page.route("**/api/settings/files", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, {
        delete_source: false,
        output_extension: "mkv",
        output_suffix: "-alchemist",
        replace_strategy: "replace",
        output_root: "/encoded",
      });
      return;
    }

    savedBody = route.request().postDataJSON() as Record<string, unknown>;
    await fulfillJson(route, 200, savedBody);
  });

  await page.goto("/settings?tab=files");
  const outputRootInput = page.getByPlaceholder("Optional mirrored output directory");
  await expect(outputRootInput).toHaveValue("/encoded");
  await outputRootInput.fill("/encoded/mirror");
  await page.getByRole("button", { name: "Save Settings" }).click();

  expect(savedBody).toMatchObject({
    output_root: "/encoded/mirror",
    replace_strategy: "replace",
  });
});

test("schedule add failure is visible", async ({ page }) => {
  await page.route("**/api/settings/schedule", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, []);
      return;
    }
    await fulfillJson(route, 500, { message: "forced schedule failure" });
  });

  await page.goto("/settings?tab=schedule");
  await page.getByRole("button", { name: "Add Schedule" }).click();
  await page.getByRole("button", { name: "Save Schedule" }).click();

  await expectVisibleError(page, "forced schedule failure");
});

test("schedule delete failure is visible", async ({ page }) => {
  await page.route("**/api/settings/schedule", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, [
        {
          id: 7,
          start_time: "00:00",
          end_time: "08:00",
          days_of_week: "[1,2,3,4,5]",
          enabled: true,
        },
      ]);
      return;
    }
    await route.continue();
  });

  await page.route("**/api/settings/schedule/7", async (route) => {
    await fulfillJson(route, 500, { message: "forced schedule delete failure" });
  });

  await page.goto("/settings?tab=schedule");
  await page.getByLabel("Delete schedule 00:00-08:00").click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Remove" }).click();

  await expectVisibleError(page, "forced schedule delete failure");
});

test("notification add failure is visible", async ({ page }) => {
  await page.route("**/api/settings/notifications", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, []);
      return;
    }
    await fulfillJson(route, 500, { message: "forced notifications add failure" });
  });

  await page.goto("/settings?tab=notifications");
  await page.getByRole("button", { name: "Add Target" }).click();
  await page.getByPlaceholder("My Discord").fill("Playwright Target");
  await page.getByPlaceholder("https://discord.com/api/webhooks/...").fill("https://example.invalid/webhook");
  await page.getByRole("button", { name: "Save Target" }).click();

  await expectVisibleError(page, "forced notifications add failure");
});

test("notification test send failure is visible", async ({ page }) => {
  await page.route("**/api/settings/notifications", async (route) => {
    await fulfillJson(route, 200, [
      {
        id: 10,
        name: "Discord",
        target_type: "discord",
        endpoint_url: "https://example.invalid/webhook",
        auth_token: null,
        events: "[\"completed\",\"failed\"]",
        enabled: true,
      },
    ]);
  });

  await page.route("**/api/settings/notifications/test", async (route) => {
    await fulfillJson(route, 500, { message: "forced notifications test failure" });
  });

  await page.goto("/settings?tab=notifications");
  await page.getByTitle("Test Notification").click();

  await expectVisibleError(page, "forced notifications test failure");
});

test("watch folder add failure is visible", async ({ page }) => {
  await page.route("**/api/settings/bundle", async (route) => {
    if (route.request().method() === "PUT") {
      await fulfillJson(route, 200, { status: "ok" });
      return;
    }

    await fulfillJson(
      route,
      200,
      createSettingsBundle({
        scanner: {
          directories: [],
          watch_enabled: true,
          extra_watch_dirs: [],
        },
      }),
    );
  });
  await page.route("**/api/settings/watch-dirs", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, []);
      return;
    }
    await fulfillJson(route, 500, { message: "forced watch add failure" });
  });
  await page.route("**/api/profiles/presets", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles", async (route) => {
    await fulfillJson(route, 200, []);
  });

  await page.goto("/settings?tab=watch");
  await page.getByPlaceholder("/path/to/media").fill("/tmp/test-media");
  await page.getByRole("button", { name: /^Add$/ }).click();

  await expectVisibleError(page, "forced watch add failure");
});

test("watch folder add submits recursive mode by default", async ({ page }) => {
  let savedBody: Record<string, unknown> | null = null;

  await page.route("**/api/settings/bundle", async (route) => {
    if (route.request().method() === "PUT") {
      await fulfillJson(route, 200, { status: "ok" });
      return;
    }

    await fulfillJson(
      route,
      200,
      createSettingsBundle({
        scanner: {
          directories: [],
          watch_enabled: true,
          extra_watch_dirs: [],
        },
      }),
    );
  });
  await page.route("**/api/settings/watch-dirs", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, []);
      return;
    }

    savedBody = route.request().postDataJSON() as Record<string, unknown>;
    await fulfillJson(route, 200, {
      id: 1,
      path: savedBody.path,
      is_recursive: savedBody.is_recursive,
    });
  });
  await page.route("**/api/profiles/presets", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles", async (route) => {
    await fulfillJson(route, 200, []);
  });

  await page.goto("/settings?tab=watch");
  await expect(page.getByText("Watch subdirectories recursively")).toHaveCount(0);
  await page.getByPlaceholder("/path/to/media").fill("/tmp/test-media");
  await page.getByRole("button", { name: /^Add$/ }).click();

  await expect.poll(() => savedBody).not.toBeNull();
  expect(savedBody).toMatchObject({
    path: "/tmp/test-media",
    is_recursive: true,
  });
});

test("watch folder remove failure is visible", async ({ page }) => {
  await page.route("**/api/settings/bundle", async (route) => {
    if (route.request().method() === "PUT") {
      await fulfillJson(route, 200, { status: "ok" });
      return;
    }

    await fulfillJson(
      route,
      200,
      createSettingsBundle({
        scanner: {
          directories: ["/tmp/test-media"],
          watch_enabled: true,
          extra_watch_dirs: [],
        },
      }),
    );
  });
  await page.route("**/api/settings/watch-dirs", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, [
        { id: 5, path: "/tmp/test-media", is_recursive: true },
      ]);
      return;
    }
    await route.continue();
  });

  await page.route("**/api/settings/watch-dirs/5", async (route) => {
    await fulfillJson(route, 500, { message: "forced watch delete failure" });
  });
  await page.route("**/api/profiles/presets", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles", async (route) => {
    await fulfillJson(route, 200, []);
  });

  await page.goto("/settings?tab=watch");
  await page.getByRole("button", { name: "Remove /tmp/test-media" }).click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Remove" }).click();

  await expectVisibleError(page, "forced watch delete failure");
});

test("transcode settings save failure is visible", async ({ page }) => {
  await page.route("**/api/settings/transcode", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, transcodeSettings);
      return;
    }
    await fulfillJson(route, 500, { message: "forced transcode failure" });
  });

  await page.goto("/settings?tab=transcode");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await expectVisibleError(page, "forced transcode failure");
});

test("hardware update failure is visible", async ({ page }) => {
  await page.route("**/api/system/hardware", async (route) => {
    await fulfillJson(route, 200, {
      vendor: "Cpu",
      device_path: null,
      supported_codecs: ["h264", "hevc"],
    });
  });

  await page.route("**/api/settings/hardware", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, {
        allow_cpu_fallback: true,
        allow_cpu_encoding: true,
        cpu_preset: "medium",
        preferred_vendor: null,
      });
      return;
    }
    await fulfillJson(route, 500, { message: "forced hardware failure" });
  });

  await page.goto("/settings?tab=hardware");
  await page.getByLabel("Allow CPU Encoding").uncheck({ force: true });

  await expectVisibleError(page, "forced hardware failure");
});

test("hardware settings save the device path on blur", async ({ page }) => {
  let savedBody: Record<string, unknown> | null = null;

  await page.route("**/api/system/hardware", async (route) => {
    await fulfillJson(route, 200, {
      vendor: "Intel",
      device_path: null,
      supported_codecs: ["h264", "hevc"],
      backends: [],
      detection_notes: [],
    });
  });

  await page.route("**/api/system/hardware/probe-log", async (route) => {
    await fulfillJson(route, 200, { entries: [] });
  });

  await page.route("**/api/settings/hardware", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, {
        allow_cpu_fallback: true,
        allow_cpu_encoding: true,
        cpu_preset: "medium",
        preferred_vendor: null,
        device_path: null,
      });
      return;
    }

    savedBody = route.request().postDataJSON() as Record<string, unknown>;
    await fulfillJson(route, 200, { status: "ok" });
  });

  await page.goto("/settings?tab=hardware");
  const devicePath = page.getByLabel("Explicit Device Path");
  await devicePath.fill("/dev/dri/renderD129");
  await devicePath.blur();

  expect(savedBody).toMatchObject({
    device_path: "/dev/dri/renderD129",
  });
});

test("hardware immediate-save changes keep the current device-path draft", async ({ page }) => {
  const savedBodies: Array<Record<string, unknown>> = [];

  await page.route("**/api/system/hardware", async (route) => {
    await fulfillJson(route, 200, {
      vendor: "Intel",
      device_path: null,
      supported_codecs: ["h264", "hevc"],
      backends: [],
      detection_notes: [],
    });
  });

  await page.route("**/api/system/hardware/probe-log", async (route) => {
    await fulfillJson(route, 200, { entries: [] });
  });

  await page.route("**/api/settings/hardware", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, {
        allow_cpu_fallback: true,
        allow_cpu_encoding: true,
        cpu_preset: "medium",
        preferred_vendor: null,
        device_path: null,
      });
      return;
    }

    savedBodies.push(route.request().postDataJSON() as Record<string, unknown>);
    await fulfillJson(route, 200, { status: "ok" });
  });

  await page.goto("/settings?tab=hardware");
  await page.getByLabel("Explicit Device Path").fill("/dev/dri/renderD129");
  await page.getByLabel("Preferred Vendor").selectOption("intel");

  expect(savedBodies.at(-1)).toMatchObject({
    preferred_vendor: "intel",
    device_path: "/dev/dri/renderD129",
  });
});

test("runtime telemetry is disabled in the UI while Alembic stabilizes", async ({ page }) => {
  await page.route("**/api/settings/system", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, {
        monitoring_poll_interval: 2,
        enable_telemetry: true,
        watch_enabled: true,
      });
      return;
    }
    await fulfillJson(route, 200, "Settings updated");
  });

  await page.goto("/settings?tab=system");

  await expect(page.getByText("Temporarily unavailable while Alembic stabilizes. Telemetry stays off for now.")).toBeVisible();
  await expect(page.getByLabel("Anonymous Telemetry")).toBeDisabled();
  await expect(page.getByLabel("Anonymous Telemetry")).not.toBeChecked();
});

test("system settings save failure is visible", async ({ page }) => {
  await page.route("**/api/settings/system", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, {
        monitoring_poll_interval: 2,
        enable_telemetry: false,
        watch_enabled: true,
      });
      return;
    }
    await fulfillJson(route, 500, { message: "forced system failure" });
  });

  await page.goto("/settings?tab=system");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await expectVisibleError(page, "forced system failure");
  await expect(page.getByText("Settings saved successfully.")).toHaveCount(0);
});
