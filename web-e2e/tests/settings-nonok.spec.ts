import { expect, test } from "@playwright/test";
import { expectVisibleError, fulfillJson, mockEngineStatus } from "./helpers";

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
  await page.route("**/api/settings/watch-dirs", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, []);
      return;
    }
    await fulfillJson(route, 500, { message: "forced watch add failure" });
  });

  await page.goto("/settings?tab=watch");
  await page.getByPlaceholder("Enter full directory path...").fill("/tmp/test-media");
  await page.getByRole("button", { name: /^Add$/ }).click();

  await expectVisibleError(page, "forced watch add failure");
});

test("watch folder remove failure is visible", async ({ page }) => {
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

  await page.goto("/settings?tab=watch");
  await page.getByTitle("Stop watching").click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Stop Watching" }).click();

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
  await page.locator("button.relative.inline-flex").first().click();

  await expectVisibleError(page, "forced hardware failure");
});

test("system settings save failure is visible", async ({ page }) => {
  await page.route("**/api/settings/system", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, {
        monitoring_poll_interval: 2,
        enable_telemetry: true,
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
