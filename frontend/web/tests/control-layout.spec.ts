import { expect, test } from "@playwright/test";
import { accessMock, owner } from "./access-fixture";
import { clientMock } from "./client-fixture";
import { maintenanceMock } from "./maintenance-fixture";
import { storageMock } from "./storage-fixture";

const lists = [
  { route: "storages", minimumRowHeight: 44, setup: storageMock },
  { route: "clients", minimumRowHeight: 44, setup: clientMock },
  { route: "accounts", minimumRowHeight: 36, setup: accessMock },
  { route: "activity", minimumRowHeight: 36, setup: maintenanceMock },
] as const;

for (const theme of ["light", "dark"]) {
  for (const width of [390, 1280]) {
    for (const list of lists) {
      test(`${list.route} rows retain spacing at ${width}px in ${theme}`, async ({ page }) => {
        await page.setViewportSize({ width, height: 720 });
        await list.setup(page);
        await page.goto(`/api/admin/console/#${list.route}`);
        await page.getByRole("button", { name: "Theme", exact: true }).click();
        await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
        const row = page.getByRole("gridcell").first().locator('xpath=ancestor::*[@role="row"]');
        await expect(row).toBeVisible();
        const bounds = await row.boundingBox();
        // The template uses compact rows; resource rows also contain usage data.
        expect(bounds!.height).toBeGreaterThanOrEqual(list.minimumRowHeight);
        await expect(row).toHaveCSS("font-family", /^-apple-system,/);
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
        if (width === 1280 && theme === "light") {
          await page.screenshot({ path: `test-results/review-${list.route}.png`, animations: "disabled" });
        }
      });
    }

    test(`storage page actions remain reachable at ${width}px in ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 720 });
      await storageMock(page);
      await page.goto("/api/admin/console/#storages");
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      await page.getByRole("button", { name: "Add storage", exact: true }).click();
      const save = page.getByRole("button", { name: "Save", exact: true });
      const cancel = page.getByRole("button", { name: "Cancel", exact: true });
      await expect(page.getByRole("dialog", { name: "Add storage" })).toBeVisible();
      await save.scrollIntoViewIfNeeded();
      await expect(save).toBeInViewport();
      await expect(cancel).toBeInViewport();
      await page.screenshot({ path: `test-results/review-register-${width}-${theme}.png`, animations: "disabled" });
      await page.getByLabel(/^Configured capacity\s*\*?$/).scrollIntoViewIfNeeded();
      await save.scrollIntoViewIfNeeded();
      await expect(save).toBeInViewport();
      const after = await save.boundingBox();
      expect(after!.y).toBeGreaterThanOrEqual(0);
      expect(after!.y + after!.height).toBeLessThanOrEqual(720);
      await save.click();
      await expect(page.getByLabel(/^Storage ID\s*\*?$/)).toBeFocused();
      await expect(page.getByRole("heading", { name: "Add storage" })).toBeVisible();
      await cancel.click();
      await expect(page.getByRole("heading", { name: "Storage", exact: true })).toBeVisible();
    });
  }
}

test("account deletion and confirmation retain the template error color", async ({ page }) => {
  await accessMock(page);
  await page.goto(`/api/admin/console/#accounts/${owner.id}`);
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  await page.getByRole("menuitem", { name: /^light$/i }).click();
  const remove = page.getByRole("button", { name: "Delete account", exact: true });
  await expect(remove).toHaveCSS("color", "rgb(194, 10, 10)");
  await remove.click();
  await page.getByLabel("Account name to confirm").fill(owner.display_name);
  await page.getByRole("checkbox", { name: "I understand this changes my access." }).check();
  await expect(page.getByRole("dialog").getByRole("button", { name: "Save", exact: true })).toHaveCSS("background-color", "rgb(194, 10, 10)");
});
