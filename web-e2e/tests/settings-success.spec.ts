import { expect, test } from "@playwright/test";
import {
  EngineModeResponse,
  EngineStatusResponse,
  LibraryProfileFixture,
  NotificationTargetFixture,
  ScheduleWindowFixture,
  WatchDirFixture,
  createSettingsBundle,
  fulfillJson,
  mockEngineStatus,
} from "./helpers";

const profilePresets: LibraryProfileFixture[] = [
  {
    id: 1,
    name: "Space Saver",
    preset: "space_saver",
    codec: "av1",
    quality_profile: "speed",
    hdr_mode: "tonemap",
    audio_mode: "aac",
    crf_override: null,
    notes: "Optimized for aggressive size reduction.",
    builtin: true,
  },
  {
    id: 2,
    name: "Balanced",
    preset: "balanced",
    codec: "av1",
    quality_profile: "balanced",
    hdr_mode: "preserve",
    audio_mode: "copy",
    crf_override: null,
    notes: "Balanced compression and playback quality.",
    builtin: true,
  },
];

test.use({ storageState: undefined });

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
});

test("hardware settings save, device-path blur commit, and rollback on failure", async ({
  page,
}) => {
  let currentSettings = {
    allow_cpu_fallback: true,
    allow_cpu_encoding: true,
    cpu_preset: "medium",
    preferred_vendor: null as string | null,
    device_path: null as string | null,
  };
  const savedBodies: Array<Record<string, unknown>> = [];
  let failNextSave = false;

  await page.route("**/api/system/hardware", async (route) => {
    await fulfillJson(route, 200, {
      vendor: "Cpu",
      device_path: null,
      supported_codecs: ["h264", "hevc", "av1"],
      backends: [],
      detection_notes: [],
    });
  });
  await page.route("**/api/settings/hardware", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, currentSettings);
      return;
    }

    const body = route.request().postDataJSON() as Record<string, unknown>;
    savedBodies.push(body);
    if (failNextSave) {
      failNextSave = false;
      await fulfillJson(route, 500, { message: "forced hardware failure" });
      return;
    }

    currentSettings = {
      allow_cpu_fallback: Boolean(body.allow_cpu_fallback),
      allow_cpu_encoding: Boolean(body.allow_cpu_encoding),
      cpu_preset: String(body.cpu_preset),
      preferred_vendor:
        typeof body.preferred_vendor === "string" ? body.preferred_vendor : null,
      device_path: typeof body.device_path === "string" ? body.device_path : null,
    };
    await fulfillJson(route, 200, currentSettings);
  });
  await page.route("**/api/system/hardware/probe-log", async (route) => {
    await fulfillJson(route, 200, { entries: [] });
  });

  await page.goto("/settings?tab=hardware");

  await page.getByLabel("Preferred Vendor").selectOption("amd");
  await expect(page.getByText("Hardware settings saved.").first()).toBeVisible();
  expect(savedBodies.at(-1)).toMatchObject({ preferred_vendor: "amd" });

  const devicePathInput = page.getByLabel("Explicit Device Path");
  await devicePathInput.fill("/dev/dri/renderD129");
  await page.getByText("Transcoding Hardware").click();
  await expect(page.getByText("Hardware settings saved.").first()).toBeVisible();
  expect(savedBodies.at(-1)).toMatchObject({
    preferred_vendor: "amd",
    device_path: "/dev/dri/renderD129",
  });

  failNextSave = true;
  await page.getByLabel("Preferred Vendor").selectOption("cpu");
  await expect(page.getByText("forced hardware failure").first()).toBeVisible();
  await expect(page.locator("#hardware-preferred-vendor")).toHaveValue("amd");
});

test("notification targets can be added, tested, and removed", async ({ page }) => {
  let targets: NotificationTargetFixture[] = [];
  let testPayload: Record<string, unknown> | null = null;

  await page.route("**/api/settings/notifications", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, targets);
      return;
    }

    const body = route.request().postDataJSON() as Record<string, unknown>;
    const nextTarget: NotificationTargetFixture = {
      id: 11,
      name: String(body.name),
      target_type: body.target_type as NotificationTargetFixture["target_type"],
      endpoint_url: String(body.endpoint_url),
      auth_token: (body.auth_token as string | null) ?? null,
      events: JSON.stringify(body.events),
      enabled: Boolean(body.enabled),
    };
    targets = [nextTarget];
    await fulfillJson(route, 200, nextTarget);
  });
  await page.route("**/api/settings/notifications/test", async (route) => {
    testPayload = route.request().postDataJSON() as Record<string, unknown>;
    await fulfillJson(route, 200, { status: "sent" });
  });
  await page.route("**/api/settings/notifications/11", async (route) => {
    targets = [];
    await fulfillJson(route, 200, { status: "deleted" });
  });

  await page.goto("/settings?tab=notifications");
  await page.getByRole("button", { name: "Add Target" }).click();
  await page.getByPlaceholder("My Discord").fill("Playwright Target");
  await page
    .getByPlaceholder("https://discord.com/api/webhooks/...")
    .fill("https://example.invalid/webhook");
  await page.getByRole("button", { name: "Save Target" }).click();

  await expect(page.getByText("Playwright Target")).toBeVisible();
  await expect(page.getByText("Target added.").first()).toBeVisible();

  await page.getByTitle("Test Notification").click();
  await expect(page.getByText("Test notification sent.").first()).toBeVisible();
  expect(testPayload).toMatchObject({
    name: "Playwright Target",
    target_type: "discord_webhook",
  });

  await page.getByLabel("Delete notification target Playwright Target").click();
  await page.getByRole("dialog").getByRole("button", { name: "Remove" }).click();

  await expect(page.getByText("Playwright Target")).toHaveCount(0);
  await expect(page.getByText("Target removed.").first()).toBeVisible();
});

test("schedule windows can be added and removed", async ({ page }) => {
  let windows: ScheduleWindowFixture[] = [];

  await page.route("**/api/settings/schedule", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, windows);
      return;
    }

    windows = [
      {
        id: 9,
        start_time: "00:00",
        end_time: "08:00",
        days_of_week: "[0,1,2,3,4,5,6]",
        enabled: true,
      },
    ];
    await fulfillJson(route, 200, windows[0]);
  });
  await page.route("**/api/settings/schedule/9", async (route) => {
    windows = [];
    await fulfillJson(route, 200, { status: "deleted" });
  });

  await page.goto("/settings?tab=schedule");
  await page.getByRole("button", { name: "Add Schedule" }).click();
  await page.getByRole("button", { name: "Save Schedule" }).click();

  await expect(page.getByText("00:00 - 08:00")).toBeVisible();
  await expect(page.getByText("Schedule added.").first()).toBeVisible();

  await page.getByLabel("Delete schedule 00:00-08:00").click();
  await page.getByRole("dialog").getByRole("button", { name: "Remove" }).click();

  await expect(page.getByText("00:00 - 08:00")).toHaveCount(0);
  await expect(page.getByText("Schedule removed.").first()).toBeVisible();
  await expect(page.getByText("No schedules active. Processing is allowed 24/7.")).toBeVisible();
});

test("watch folders can be added and removed", async ({ page }) => {
  let dirs: WatchDirFixture[] = [];

  await page.route("**/api/settings/bundle", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, createSettingsBundle());
      return;
    }
    await fulfillJson(route, 200, { status: "ok" });
  });
  await page.route("**/api/settings/watch-dirs", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, dirs);
      return;
    }

    dirs = [
      {
        id: 1,
        path: "/tmp/test-media",
        is_recursive: true,
        profile_id: null,
      },
    ];
    await fulfillJson(route, 200, dirs[0]);
  });
  await page.route("**/api/settings/watch-dirs/1", async (route) => {
    dirs = [];
    await fulfillJson(route, 200, { status: "deleted" });
  });
  await page.route("**/api/profiles/presets", async (route) => {
    await fulfillJson(route, 200, profilePresets);
  });
  await page.route("**/api/profiles", async (route) => {
    await fulfillJson(route, 200, profilePresets);
  });

  await page.goto("/settings?tab=watch");
  await page.getByPlaceholder("/path/to/media").fill("/tmp/test-media");
  await page.getByRole("button", { name: /^Add$/ }).click();

  await expect(page.getByText("/tmp/test-media")).toBeVisible();
  await expect(page.getByText("Folder added.").first()).toBeVisible();

  await page.getByRole("button", { name: "Remove /tmp/test-media" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Remove" })
    .click();

  await expect(page.getByText("/tmp/test-media")).toHaveCount(0);
  await expect(page.getByText("Folder removed.").first()).toBeVisible();
});

test("system engine mode changes refresh runtime controls", async ({ page }) => {
  const systemSettings = {
    monitoring_poll_interval: 2,
    enable_telemetry: false,
    watch_enabled: true,
  };
  let mode: EngineModeResponse = {
    mode: "balanced",
    is_manual_override: false,
    concurrent_limit: 2,
    cpu_count: 8,
    computed_limits: {
      background: 1,
      balanced: 2,
      throughput: 4,
    },
  };
  let status: EngineStatusResponse = {
    status: "paused",
    manual_paused: true,
    scheduler_paused: false,
    draining: false,
    mode: "balanced",
    concurrent_limit: 2,
    is_manual_override: false,
  };

  await page.route("**/api/settings/system", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, systemSettings);
      return;
    }
    await fulfillJson(route, 200, systemSettings);
  });
  await page.route("**/api/engine/mode", async (route) => {
    if (route.request().method() === "GET") {
      await fulfillJson(route, 200, mode);
      return;
    }

    mode = { ...mode, mode: "throughput", is_manual_override: true, concurrent_limit: 4 };
    status = { ...status, mode: "throughput", is_manual_override: true, concurrent_limit: 4 };
    await fulfillJson(route, 200, {
      mode: "throughput",
      concurrent_limit: 4,
      is_manual_override: true,
    });
  });
  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, status);
  });

  await page.goto("/settings?tab=system");
  await page.getByRole("button", { name: "throughput" }).click();

  await expect(page.getByText("Mode set to throughput.").first()).toBeVisible();
  await expect(page.getByText(/Manual override active/i)).toBeVisible();
});
