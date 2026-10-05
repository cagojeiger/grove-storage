import { expect, test } from "@playwright/test";
import { owner, accessMock } from "./access-fixture";
import { maintenanceMock } from "./maintenance-fixture";
import { session } from "./command-fixture";

for (const width of [320, 768, 1440]) {
  for (const mode of ["light", "dark"]) {
    test(`account navigation and visual states ${width}px ${mode}`, async ({ page }) => {
      await accessMock(page);
      await maintenanceMock(page);
      await page.route("**/identity/v1/me", (route) => route.fulfill({ json: owner }));
      await page.route("**/identity/v1/session", (route) => route.fulfill({
        json: { ...session, user_id: owner.id, credential_id: null },
      }));
      await page.setViewportSize({ width, height: 900 });
      await page.goto("/api/admin/console/#settings");
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${mode}$`, "i") }).click();
      const heading = page.getByRole("heading", { level: 1 });
      const initial = await heading.evaluate((el) => ({
        color: getComputedStyle(el).color,
        top: el.getBoundingClientRect().top,
      }));
      const fieldWidth = (await page.getByLabel("Display name").boundingBox())!.width;
      const save = page.getByRole("button", { name: "Save changes", exact: true });
      await expect(save).toBeDisabled();
      await expect(save).toHaveCSS("background-image", "none");
      await expect(save).toHaveCSS("box-shadow", "none");
      const disabledColor = await save.evaluate((el) => getComputedStyle(el).color);
      await page.getByLabel("Display name").fill("Updated preview name");
      await expect(save).toBeEnabled();
      await expect(save).not.toHaveCSS("color", disabledColor);
      await page.getByLabel("Display name").fill(owner.display_name);
      await expect(save).toBeDisabled();
      await expect(save).toHaveCSS("background-image", "none");

      for (const name of ["Security", "API tokens", "Sessions", "Profile"]) {
        const tab = page.getByRole("tab", { name, exact: true });
        await tab.click();
        await expect(tab).toHaveAttribute("aria-selected", "true");
        await expect(heading).toHaveText("My account");
        await expect(heading).toHaveCSS("color", initial.color);
        expect((await heading.boundingBox())!.y).toBe(initial.top);
        await expect(page.getByRole("navigation", { name: "Page path" })).toHaveCount(0);
        await tab.hover();
        await page.mouse.move(width - 1, 0);
        await expect(tab).toHaveAttribute("aria-selected", "true");
        if (name === "Security") {
          expect((await page.getByLabel("Current password").boundingBox())!.width).toBe(fieldWidth);
          await page.screenshot({
            path: `../../output/console-account-consistency-20261004/security-${width}-${mode}.png`,
            animations: "disabled",
          });
        }
      }
      await page.screenshot({
        path: `../../output/console-account-consistency-20261004/profile-${width}-${mode}.png`,
        animations: "disabled",
      });
      if (width < 900) await page.getByRole("button", { name: "Open navigation" }).click();
      const account = page.getByRole("link", { name: "My account", exact: true });
      await expect(account).toHaveCSS("text-decoration-line", "none");
      expect(await account.evaluate((el) => getComputedStyle(el, "::before").content)).toBe("none");
      await expect(page.getByRole("button", { name: "Account options" })).toHaveCount(0);
      await expect(page.getByRole("button", { name: "Sign out", exact: true })).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    });
  }
}
