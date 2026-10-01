import { expect, test } from "@playwright/test";
import { accessMock, root } from "./access-fixture";

for (const theme of ["light", "dark"]) {
  test(`MUI dialog keeps actions visible while its content scrolls in ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 400 });
    await accessMock(page);
    await page.goto(root);
    await page.getByLabel("Theme").selectOption(theme);
    await page.getByRole("button", { name: "Create account", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Create account", exact: true });
    await expect(dialog).toBeVisible();
    const ids = await dialog.locator("[id]").evaluateAll(elements => elements.map(element => element.id));
    expect(new Set(ids).size).toBe(ids.length);
    const cancel = dialog.getByRole("button", { name: "Cancel" });
    const submit = dialog.getByRole("button", { name: "Create account" });
    await expect(cancel).toBeInViewport();
    await expect(submit).toBeInViewport();
    await dialog.getByLabel("Your current password").scrollIntoViewIfNeeded();
    await expect(cancel).toBeInViewport();
    await expect(submit).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await cancel.click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Create account", exact: true })).toBeFocused();
  });
}
