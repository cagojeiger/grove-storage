import { expect, test, type Page } from "@playwright/test";
import { context, invocation, maintenanceMock } from "./maintenance-fixture";

async function history(page: Page) {
  await maintenanceMock(page);
  await page.route("**/identity/v1/history/invocations?*", route => {
    const cursor = new URL(route.request().url()).searchParams.get("before");
    return route.fulfill({ json: {
      items: Array.from({ length: 50 }, (_, index) => ({
        ...invocation,
        operation: `storage.test.${index}`,
        context: { ...context, id: String((cursor ? 50 : 100) - index) },
      })),
      next_before: cursor ? null : "51",
    } });
  });
  await page.goto("/api/admin/console/#activity/invocations");
}

async function columnMenu(page: Page, field: string) {
  const header = page.locator(`[role="columnheader"][data-field="${field}"]`);
  // Responsive column layout can move the header away from the pointer.
  await expect(async () => {
    await header.hover();
    await header.getByRole("button").click({ timeout: 1_000 });
  }).toPass({ timeout: 10_000 });
  await expect(page.getByRole("menu")).toBeVisible();
}

test("native column controls hide and restore columns across detail and refresh", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 960 });
  await history(page);
  const scroller = page.locator(".MuiDataGrid-virtualScroller");
  await expect.poll(() => scroller.evaluate(element => element.scrollWidth - element.clientWidth)).toBeLessThanOrEqual(1);
  const source = page.locator('[role="columnheader"][data-field="source"]');
  await columnMenu(page, "source");
  await page.getByRole("menuitem", { name: "Hide column", exact: true }).click();
  await expect(source).toHaveCount(0);
  await expect(page.locator('[role="columnheader"][data-field="time"]')).toBeInViewport({ ratio: 1 });
  await page.getByRole("button", { name: "storage.test.0", exact: true }).click();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Refresh activity", exact: true }).click();
  await expect(page.getByRole("button", { name: "storage.test.0", exact: true })).toBeVisible();
  await expect(source).toHaveCount(0);
  await columnMenu(page, "event");
  await page.getByRole("menuitem", { name: "Manage columns", exact: true }).click();
  await page.getByRole("checkbox", { name: "Source", exact: true }).check();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu", { name: "Operation column menu", exact: true })).toBeHidden();
  await expect(source).toBeVisible();
  await expect.poll(() => scroller.evaluate(element => element.scrollWidth - element.clientWidth)).toBeLessThanOrEqual(1);
  await columnMenu(page, "duration");
  await expect(page.getByRole("menuitem", { name: "Hide column", exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
});

test("column choices are retained independently for mobile and desktop", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await history(page);
  const source = page.locator('[role="columnheader"][data-field="source"]');
  await expect(source).toHaveCount(0);
  await columnMenu(page, "event");
  await page.getByRole("menuitem", { name: "Manage columns", exact: true }).click();
  await page.getByRole("checkbox", { name: "Source", exact: true }).check();
  await page.keyboard.press("Escape");
  await expect(source).toBeAttached();
  await page.setViewportSize({ width: 1440, height: 960 });
  await expect(source).toBeVisible();
  await columnMenu(page, "source");
  await page.getByRole("menuitem", { name: "Hide column", exact: true }).click();
  await expect(source).toHaveCount(0);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(source).toBeAttached();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test("keyboard navigation reveals offscreen rows on the second cursor page", async ({ page }) => {
  await history(page);
  await page.getByRole("button", { name: "Go to next page", exact: true }).click();
  const grid = page.getByRole("grid", { name: "Activity", exact: true });
  await expect(grid.locator('[role="row"][data-id="50"]')).toBeVisible();
  await grid.locator('[role="columnheader"][data-field="event"]').focus();
  for (let index = 0; index < 49; index++) {
    await page.keyboard.press("ArrowDown");
    const operation = grid.getByRole("button", { name: `storage.test.${index}`, exact: true });
    await expect(operation).toBeFocused();
    await expect(operation).toBeInViewport();
  }
  const selected = grid.getByRole("button", { name: "storage.test.48", exact: true });
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog", { name: "Event details" })).toContainText("storage.test.48");
  await page.keyboard.press("Escape");
  await expect(selected).toBeFocused();
  await expect(selected).toBeInViewport();
  await page.keyboard.press("Control+End");
  const last = grid.locator('[role="row"][data-id="1"] [data-field="duration"]');
  await expect(last).toBeFocused();
  await expect(last).toBeInViewport();
  await page.keyboard.press("Control+Home");
  await expect(grid.locator('[role="row"][data-id="50"] [data-field="time"]')).toBeFocused();
});
