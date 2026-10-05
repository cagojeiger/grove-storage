import { expect, test } from "@playwright/test";
import { accessMock, owner, root } from "./access-fixture";

for (const width of [320, 390, 768, 1024, 1440])
  for (const theme of ["light", "dark"]) {
    test(`Access ${width}px ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 960 });
      await accessMock(page, [
        {
          ...owner,
          display_name: "Home-administrator-with-a-long-name-for-layout",
        },
      ]);
      await page.goto(root);
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      await page
        .getByRole("link", { name: /^Home-administrator/ })
        .click();
      await expect(
        page.getByRole("region", { name: "Management API tokens" }),
      ).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: `test-results/access-${width}-${theme}.png`,
        fullPage: true,
      });
      await page
        .getByRole("button", { name: "Issue token", exact: true })
        .click();
      await expect(page.getByLabel("Expires in days")).toBeVisible();
      expect(
        await page
          .getByRole("dialog")
          .evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
    });
  }
