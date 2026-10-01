import { expect, test } from "@playwright/test";
import { accessMock, owner } from "./access-fixture";
import { clientMock } from "./client-fixture";
import { maintenanceMock } from "./maintenance-fixture";
import { storageMock } from "./storage-fixture";

const lists = [
  { route: "storages", selector: '[role="gridcell"]', setup: storageMock },
  { route: "clients", selector: '[role="gridcell"]', setup: clientMock },
  { route: "accounts", selector: "tbody td", setup: accessMock },
  { route: "activity", selector: "tbody td", setup: maintenanceMock },
] as const;

for (const theme of ["light", "dark"]) {
  for (const width of [390, 1280]) {
    for (const list of lists) {
      test(`${list.route} rows retain spacing at ${width}px in ${theme}`, async ({ page }) => {
        await page.setViewportSize({ width, height: 720 });
        await list.setup(page);
        await page.goto(`/api/admin/console/#${list.route}`);
        await page.getByLabel("Theme").selectOption(theme);
        const row = page.locator(list.selector).first();
        await expect(row).toBeVisible();
        if (list.route === "accounts" || list.route === "activity") {
          await expect(row).toHaveCSS("padding-top", "6px");
          await expect(row).toHaveCSS("padding-bottom", "6px");
          await expect(row).toHaveCSS("border-bottom-width", "1px");
        } else {
          const bounds = await row.boundingBox();
          expect(bounds!.height).toBeGreaterThanOrEqual(44);
        }
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
      await page.getByLabel("Theme").selectOption(theme);
      await page.getByRole("button", { name: "Add storage", exact: true }).click();
      const save = page.getByRole("button", { name: "Save", exact: true });
      const cancel = page.getByRole("button", { name: "Cancel", exact: true });
      await expect(page.getByRole("dialog")).toHaveCount(0);
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

test("account danger controls and confirmation share error color", async ({ page }) => {
  await accessMock(page);
  await page.goto(`/api/admin/console/#accounts/${owner.id}`);
  await page.getByLabel("Theme").selectOption("light");
  const remove = page.getByRole("button", { name: "Delete account", exact: true });
  await expect(remove).toHaveCSS("color", "rgb(211, 47, 47)");
  await expect(page.getByRole("button", { name: "Disable", exact: true })).toHaveCSS("color", "rgb(211, 47, 47)");
  await remove.click();
  await page.getByLabel("Confirm account name").fill(owner.display_name);
  await page.getByRole("checkbox", { name: "I understand my current session will end." }).check();
  await expect(page.getByRole("dialog").getByRole("button", { name: "Delete account", exact: true })).toHaveCSS("background-color", "rgb(211, 47, 47)");
});
