import { expect, test } from "@playwright/test";
import { accessMock, root } from "./access-fixture";
import { clientMock } from "./client-fixture";

for (const theme of ["light", "dark"]) {
  test(`account rows inherit the shared font in ${theme} mode`, async ({ page }) => {
    await accessMock(page);
    await page.goto(root);
    await page.getByLabel("Theme").selectOption(theme);
    const rows = page.locator(".account-row");
    await expect(rows).toHaveCount(2);
    for (const row of await rows.all()) {
      await expect(row).toHaveCSS("font-family", /^Inter,/);
      await expect(row.locator("strong")).toHaveCSS("font-family", /^Inter,/);
    }
    await expect(page.getByRole("button", { name: "Create user", exact: true })).toHaveCSS("font-family", /^Inter,/);
  });

  test(`JSON keeps its monospace font in ${theme} mode`, async ({ page }) => {
    await clientMock(page);
    await page.goto("/api/admin/console/#clients/notegate");
    await page.getByLabel("Theme").selectOption(theme);
    await expect(page.locator(".metadata-json")).toHaveCSS("font-family", /monospace/);
    await page.getByRole("button", { name: "Edit metadata", exact: true }).click();
    await expect(page.getByLabel("Metadata JSON")).toHaveCSS("font-family", /monospace/);
    await expect(page.getByRole("button", { name: "Save", exact: true })).toHaveCSS("font-family", /^Inter,/);
  });
}
