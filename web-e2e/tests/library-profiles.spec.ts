import { expect, test, type Page } from "@playwright/test";
import { fulfillJson, mockEngineStatus } from "./helpers";

const watchDirs = [
  {
    id: 1,
    path: "/media/movies",
    is_recursive: true,
    profile_id: 3,
  },
];

const profilePresets = [
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
    name: "Quality First",
    preset: "quality_first",
    codec: "hevc",
    quality_profile: "quality",
    hdr_mode: "preserve",
    audio_mode: "copy",
    crf_override: null,
    notes: "Prioritizes fidelity over maximum compression.",
    builtin: true,
  },
  {
    id: 3,
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
  {
    id: 4,
    name: "Streaming",
    preset: "streaming",
    codec: "h264",
    quality_profile: "balanced",
    hdr_mode: "tonemap",
    audio_mode: "aac_stereo",
    crf_override: null,
    notes: "Maximizes compatibility for streaming clients.",
    builtin: true,
  },
];

async function mockWatchFolders(page: Page): Promise<void> {
  await page.route("**/api/settings/bundle", async (route) => {
    await fulfillJson(route, 200, {
      settings: {
        scanner: {
          directories: ["/media/movies"],
          watch_enabled: true,
          extra_watch_dirs: [],
        },
      },
    });
  });

  await page.route("**/api/settings/watch-dirs", async (route) => {
    await fulfillJson(route, 200, watchDirs);
  });

  await page.route("**/api/profiles/presets", async (route) => {
    await fulfillJson(route, 200, profilePresets);
  });

  await page.route("**/api/profiles", async (route) => {
    await fulfillJson(route, 200, profilePresets);
  });
}

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
});

test("watch folders show profile selector", async ({ page }) => {
  if (watchDirs.length === 0) {
    test.skip(true, "requires at least one watch folder to be configured");
  }

  await mockWatchFolders(page);
  await page.goto("/settings?tab=watch");

  await expect(page.getByRole("combobox").first()).toBeVisible();
});

test("profile selector shows preset options", async ({ page }) => {
  if (watchDirs.length === 0) {
    test.skip(true, "requires at least one watch folder to be configured");
  }

  await mockWatchFolders(page);
  await page.goto("/settings?tab=watch");

  const profileSelect = page.getByRole("combobox").first();
  await profileSelect.focus();

  const optionTexts = await profileSelect.locator("option").allTextContents();
  expect(
    optionTexts.some((option) =>
      ["Space Saver", "Quality First", "Balanced", "Streaming", "No profile"].some((label) =>
        option.includes(label),
      ),
    ),
  ).toBe(true);
});

test("customize button opens profile modal", async ({ page }) => {
  if (watchDirs.length === 0) {
    test.skip(true, "requires at least one watch folder to be configured");
  }

  await mockWatchFolders(page);
  await page.goto("/settings?tab=watch");

  await page.getByTitle("Customize profile").first().click();

  await expect(page.getByRole("heading", { name: "Customize Profile" })).toBeVisible();
  await expect(page.getByText("Name").first()).toBeVisible();
  await expect(page.getByText("Codec").first()).toBeVisible();
});
