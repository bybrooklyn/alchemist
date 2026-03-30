import { expect, test } from "@playwright/test";
import { fulfillJson, mockDashboardData, mockSetupBootstrap } from "./helpers";

test("setup completes successfully, seeds the first scan, and lands on a paused dashboard", async ({
  page,
}) => {
  let scanStatusCalls = 0;

  await mockSetupBootstrap(page, {
    recommendations: [
      {
        path: "/srv/media",
        label: "media",
        reason: "Looks like a media library",
        media_hint: "high",
      },
    ],
  });
  await mockDashboardData(page, {
    bundle: {
      scanner: {
        directories: ["/srv/media"],
        watch_enabled: true,
        extra_watch_dirs: [],
      },
    },
  });

  await page.route("**/api/settings/preferences", async (route) => {
    await fulfillJson(route, 200, { status: "ok" });
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
    const body = route.request().postDataJSON() as {
      username: string;
      password: string;
      settings: {
        scanner: {
          directories: string[];
        };
      };
    };
    expect(body.username).toBe("playwright");
    expect(body.settings.scanner.directories).toContain("/srv/media");
    await fulfillJson(route, 200, { status: "ok" });
  });
  await page.route("**/api/scan/start", async (route) => {
    await route.fulfill({ status: 202, body: "" });
  });
  await page.route("**/api/scan/status", async (route) => {
    scanStatusCalls += 1;
    if (scanStatusCalls === 1) {
      await fulfillJson(route, 200, {
        is_running: true,
        files_found: 5,
        files_added: 2,
        current_folder: "/srv/media",
      });
      return;
    }

    await fulfillJson(route, 200, {
      is_running: false,
      files_found: 5,
      files_added: 5,
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
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "Final Review" })).toBeVisible();
  await page.getByRole("button", { name: "Complete Setup" }).click();

  await page.waitForURL((url) => !url.pathname.includes("/setup"));
  await expect(page.getByRole("button", { name: "Enter Dashboard" })).toHaveCount(0);
  await expect(page.getByText("Paused", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Start" })).toBeVisible();
});
