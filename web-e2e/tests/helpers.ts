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
    await fulfillJson(route, 200, { status: "ok" });
  });
}

export async function expectVisibleError(page: Page, message: string): Promise<void> {
  await expect(page.getByText(message).first()).toBeVisible();
}
