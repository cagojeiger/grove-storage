import { expect, test } from "@playwright/test";
import { storageMock, example } from "./storage-fixture";
import { clientMock } from "./client-fixture";
import { accessMock } from "./access-fixture";
import { maintenanceMock } from "./maintenance-fixture";

for (const mode of ["light", "dark"]) {
  for (const width of [320, 1440, 1920]) {
    for (const kind of ["storages", "clients", "accounts", "activity"] as const) {
      test(`list fills viewport ${kind} ${width}px ${mode}`, async ({ page }) => {
        await page.setViewportSize({ width, height: 1000 });
        if (kind === "storages") await storageMock(page);
        if (kind === "clients") await clientMock(page);
        if (kind === "accounts") await accessMock(page);
        if (kind === "activity") await maintenanceMock(page);
        await page.goto(`/api/admin/console/#${kind}`);
        await page.getByRole("button", { name: "Theme", exact: true }).click();
        await page.getByRole("menuitem", { name: new RegExp(`^${mode}$`, "i") }).click();
        const grid = page.getByRole("grid");
        await expect(grid).toBeVisible();
        const footer = page.locator("main footer");
        await expect(footer).toBeInViewport();
        const bounds = (await grid.boundingBox())!;
        expect(bounds.height).toBeGreaterThan(width === 320 ? 350 : 550);
        const bottom = (await footer.boundingBox())!;
        expect(bottom.y + bottom.height).toBeCloseTo(976, 0);
        expect(bounds.y + bounds.height).toBeLessThan(bottom.y);
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
        await page.screenshot({
          path: `../../output/console-list-viewport-20261004/${kind}-${width}-${mode}.png`,
          animations: "disabled",
        });
        await page.setViewportSize({ width, height: 1200 });
        await expect.poll(async () => (await grid.boundingBox())!.height).toBeCloseTo(bounds.height + 200, 0);
      });
    }
  }
}

test("large lists scroll inside the grid and preserve pagination", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await storageMock(page, Array.from({ length: 41 }, (_, index) => ({
    ...example, id: `store-${String(index + 1).padStart(2, "0")}`,
  })));
  await page.goto("/api/admin/console/#storages");
  const grid = page.getByRole("grid");
  await expect(grid.getByRole("link", { name: "store-01", exact: true })).toBeVisible();
  const viewport = grid.locator(".MuiDataGrid-virtualScroller");
  expect(await viewport.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
  await viewport.hover();
  await page.mouse.wheel(0, 1200);
  await expect.poll(() => viewport.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
  await expect(page.locator("main footer")).toBeInViewport();
  await page.getByRole("button", { name: /next page/i }).click();
  await expect(page.getByText("21–40 of 41", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: /next page/i }).click();
  await expect(grid.getByRole("link", { name: "store-41", exact: true })).toBeVisible();
});

test("short mobile viewport keeps expanded filters and pagination reachable", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 568 });
  await maintenanceMock(page);
  await page.goto("/api/admin/console/#activity");
  await page.getByRole("button", { name: "Filters", exact: true }).click();
  await expect(page.getByLabel("Actor account ID")).toBeVisible();
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await page.locator("main footer").scrollIntoViewIfNeeded();
  await expect(page.locator("main footer")).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: "../../output/console-list-viewport-20261004/short-mobile.png" });
});
