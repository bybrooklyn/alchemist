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
  await expect(page.getByRole("heading", { name: "Alchemist Setup" })).toBeVisible();

  await page.getByPlaceholder("admin").fill("playwright");
  await page.getByPlaceholder("••••••••").fill("playwright-password");
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Next" }).click();

  await expect(page.getByRole("heading", { name: "Final Review" })).toBeVisible();
  await page.getByRole("button", { name: "Build Engine" }).click();

  await expect(page.getByText("Scan failed or became unavailable.")).toBeVisible();
  await expect(page.getByText("forced scan start failure")).toBeVisible();

  await page.getByRole("button", { name: "Back to Review" }).click();
  await expect(page.getByRole("heading", { name: "Final Review" })).toBeVisible();

  await page.getByRole("button", { name: "Build Engine" }).click();
  await expect(page.getByText("Scan failed or became unavailable.")).toBeVisible();

  await page.getByRole("button", { name: "Retry Scan" }).click();
  await expect(page.getByRole("button", { name: "Enter Dashboard" })).toBeVisible();
  await expect(scanStartAttempts).toBe(3);
});
