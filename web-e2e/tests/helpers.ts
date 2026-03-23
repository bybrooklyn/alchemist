import { expect, type Page, type Route } from "@playwright/test";

export async function fulfillJson(route: Route, status: number, body: unknown): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });
}

export async function mockEngineStatus(page: Page): Promise<void> {
  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, {
      status: "paused",
      manual_paused: true,
      scheduler_paused: false,
      draining: false,
      mode: "balanced",
      concurrent_limit: 2,
      is_manual_override: false,
    });
  });
  await page.route("**/api/engine/mode", async (route) => {
    await fulfillJson(route, 200, {
      mode: "balanced",
      is_manual_override: false,
      concurrent_limit: 2,
      cpu_count: 8,
      computed_limits: {
        background: 1,
        balanced: 4,
        throughput: 4,
      },
    });
  });
}

export async function expectVisibleError(page: Page, message: string): Promise<void> {
  await expect(page.getByText(message).first()).toBeVisible();
}
