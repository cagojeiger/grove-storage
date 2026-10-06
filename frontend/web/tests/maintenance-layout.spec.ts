import { expect, test } from "@playwright/test";
import { maintenanceMock } from "./maintenance-fixture";

for (const width of [320, 390, 768, 1440])
  for (const theme of ["light", "dark"]) {
    test(`maintenance ${width}px ${theme}`, async ({ page }) => {
      await maintenanceMock(page);
      await page.setViewportSize({ width, height: 960 });
      await page.goto("/api/admin/console/#activity");
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      await expect(
        page.getByRole("button", { name: /storage.create/ }),
      ).toBeVisible();
      await screenshot("activity");
      await page.getByRole("button", { name: /storage.create/ }).click();
      await screenshot("event");
      expect(
        await page
          .getByRole("dialog")
          .evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
      await page.getByRole("button", { name: "Close", exact: true }).click();
      if (width < 900) await page.getByRole("button", { name: "Open navigation" }).click();
      await page.getByRole("link", { name: "My account", exact: true }).click();
      await page.getByRole("tab", { name: "Sessions", exact: true }).click();
      await expect(
        page.getByText("Current session", { exact: true }),
      ).toBeVisible();
      await screenshot("sessions");
      async function screenshot(view: string) {
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        await page.screenshot({
          path: `test-results/${view}-${width}-${theme}.png`,
          fullPage: true,
        });
      }
    });
  }
