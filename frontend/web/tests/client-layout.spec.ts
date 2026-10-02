import { test, expect } from "@playwright/test";
import { clientMock, root } from "./client-fixture";

for (const width of [320, 390, 768, 1024, 1440])
  for (const theme of ["light", "dark"]) {
    test(`clients ${width}px ${theme}`, async ({ page }) => {
      await clientMock(page);
      await page.setViewportSize({ width, height: 960 });
      await page.goto(root);
      await page.getByLabel("Theme").selectOption(theme);
      await page.getByRole("link", { name: /notegate/ }).click();
      await page.getByRole("tab", { name: "S3 credentials", exact: true }).click();
      await expect(page.getByRole("region", { name: "S3 credentials" })).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: `test-results/clients-${width}-${theme}.png`,
        fullPage: true,
      });
      await page.getByRole("button", { name: "Create credential" }).click();
      await page.getByRole("button", { name: "Confirm", exact: true }).click();
      await expect(
        page.getByLabel(/^Secret key\s*\*?$/),
      ).toBeVisible();
      expect(
        await page
          .getByRole("dialog")
          .evaluate((e) => e.scrollWidth <= e.clientWidth),
      ).toBe(true);
    });
  }
