import { expect, test } from "@playwright/test";
import { accessMock, root } from "./access-fixture";
import { clientMock } from "./client-fixture";

for (const theme of ["light", "dark"]) {
  test(`account rows inherit the shared font in ${theme} mode`, async ({
    page,
  }) => {
    await accessMock(page);
    await page.goto(root);
    await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
    const rows = page
      .getByRole("grid", { name: "Accounts", exact: true })
      .getByRole("row").filter({ has: page.getByRole("link") });
    await expect(rows).toHaveCount(2);
    for (const row of await rows.all()) {
      await expect(row).toHaveCSS("font-family", /^-apple-system,/);
      await expect(row.locator("a")).toHaveCSS(
        "font-family",
        /^-apple-system,/,
      );
    }
    await expect(
      page.getByRole("button", { name: "Create account", exact: true }),
    ).toHaveCSS("font-family", /^-apple-system,/);
  });

  test(`metadata uses the shared system font in ${theme} mode`, async ({ page }) => {
    await clientMock(page);
    await page.goto("/api/admin/console/#clients/notegate");
    await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
    await expect(page.getByLabel("Saved metadata", { exact: true })).toHaveCSS(
      "font-family",
      /^-apple-system,/,
    );
    await page
      .getByRole("button", { name: "Edit metadata", exact: true })
      .click();
    await expect(page.getByLabel("Metadata JSON")).toHaveCSS(
      "font-family",
      /^-apple-system,/,
    );
    await expect(
      page.getByRole("button", { name: "Save", exact: true }),
    ).toHaveCSS("font-family", /^-apple-system,/);
  });
}
