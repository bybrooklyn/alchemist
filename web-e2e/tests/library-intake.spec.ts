import { expect, test } from "@playwright/test";
import {
  createSettingsBundle,
  fulfillEmpty,
  fulfillJson,
  mockEngineStatus,
  mockSettingsBundle,
} from "./helpers";

test.use({ storageState: undefined });

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
  await page.route("**/api/library/profiles", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles/presets", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/profiles", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/scan/status", async (route) => {
    await fulfillJson(route, 200, {
      is_running: false,
      files_found: 0,
      current_folder: null,
    });
  });
});

test("library intake shows unified folder list with no Library Directories or Watch Folders headings", async ({
  page,
}) => {
  await mockSettingsBundle(page, {
    scanner: {
      directories: ["/media/movies"],
      watch_enabled: true,
      extra_watch_dirs: [],
    },
  });
  await page.route("**/api/settings/watch-dirs**", async (route) => {
    await fulfillJson(route, 200, [
      {
        id: 1,
        path: "/media/tv",
        is_recursive: true,
        profile_id: null,
      },
    ]);
  });

  await page.goto("/settings?tab=watch");

  await expect(page.getByText("/media/movies")).toBeVisible();
  await expect(page.getByText("/media/tv")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Library Directories" })).not.toBeVisible();
  await expect(page.getByRole("heading", { name: "Watch Folders" })).not.toBeVisible();
  await expect(page.getByText(/watch subdirectories recursively/i)).not.toBeVisible();
});

test("Scan Now button is present on Library & Intake page", async ({ page }) => {
  await mockSettingsBundle(page);
  await page.route("**/api/settings/watch-dirs**", async (route) => {
    await fulfillJson(route, 200, []);
  });

  await page.goto("/settings?tab=watch");

  await expect(page.getByRole("button", { name: /scan now/i })).toBeVisible();
});

test("adding a folder via text input calls bundle and watch-dirs APIs", async ({ page }) => {
  let bundlePutCalled = false;
  let watchDirsPostCalled = false;

  await page.route("**/api/settings/bundle", async (route) => {
    if (route.request().method() === "PUT") {
      bundlePutCalled = true;
      await fulfillJson(route, 200, {});
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
  await page.route("**/api/settings/watch-dirs**", async (route) => {
    if (route.request().method() === "POST") {
      watchDirsPostCalled = true;
      await fulfillJson(route, 201, {
        id: 99,
        path: "/media/new",
        is_recursive: true,
        profile_id: null,
      });
      return;
    }

    await fulfillJson(route, 200, []);
  });

  await page.goto("/settings?tab=watch");
  await page.getByPlaceholder(/path/i).fill("/media/new");
  await page.getByRole("button", { name: "Add" }).click();

  await expect.poll(() => bundlePutCalled).toBe(true);
  await expect.poll(() => watchDirsPostCalled).toBe(true);
});

test("deleting a folder calls bundle PUT and watch-dirs DELETE", async ({ page }) => {
  let bundlePutCalled = false;
  let watchDirDeleteCalled = false;

  await page.route("**/api/settings/bundle", async (route) => {
    if (route.request().method() === "PUT") {
      bundlePutCalled = true;
      await fulfillJson(route, 200, {});
      return;
    }

    await fulfillJson(
      route,
      200,
      createSettingsBundle({
        scanner: {
          directories: ["/media/movies"],
          watch_enabled: true,
          extra_watch_dirs: [],
        },
      }),
    );
  });
  await page.route("**/api/settings/watch-dirs**", async (route) => {
    if (route.request().method() === "DELETE") {
      watchDirDeleteCalled = true;
      await fulfillEmpty(route, 204);
      return;
    }

    await fulfillJson(route, 200, [
      {
        id: 1,
        path: "/media/movies",
        is_recursive: true,
        profile_id: null,
      },
    ]);
  });

  await page.goto("/settings?tab=watch");
  await expect(page.getByText("/media/movies")).toBeVisible();

  await page.getByRole("button", { name: "Remove /media/movies" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Remove" }).click();

  await expect.poll(() => bundlePutCalled).toBe(true);
  await expect.poll(() => watchDirDeleteCalled).toBe(true);
});
