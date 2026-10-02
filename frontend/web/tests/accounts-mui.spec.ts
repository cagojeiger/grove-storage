import { expect, test } from "@playwright/test";
import { accessMock, owner, rawSetupToken } from "./access-fixture";

for (const mode of ["light", "dark"]) {
  for (const width of [320, 390, 768, 1280]) {
    test(`Accounts use stock MUI at ${width}px in ${mode}`, async ({
      page,
    }) => {
      const name = "Administrator-with-a-long-unbroken-name-for-layout";
      await accessMock(page, [{ ...owner, display_name: name }]);
      await page.setViewportSize({ width, height: 900 });
      await page.goto("/api/admin/console/#accounts");
      await page.getByLabel("Theme").selectOption(mode);
      const heading = page.getByRole("heading", {
        name: "Accounts",
        exact: true,
      });
      await expect(heading).toHaveCSS("font-size", "24px");
      await expect(heading).toHaveCSS("font-weight", "400");
      await expect(heading).toHaveCSS("font-family", /^-apple-system,/);
      await expect(
        page
          .getByRole("main")
          .locator("[class*='account-toolbar'], .pagination, .overview"),
      ).toHaveCount(0);
      await expect(page.getByRole("link", { name, exact: true })).toBeVisible();
      await expect(page.getByRole("button", { name: "Create account" })).toHaveCSS(
        "background-color",
        mode === "dark" ? "rgb(156, 219, 121)" : "rgb(23, 107, 61)",
      );
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: `test-results/accounts-mui-list-${width}-${mode}.png`,
        animations: "disabled",
        fullPage: true,
      });
      await page.getByRole("link", { name, exact: true }).click();
      const details = page.locator('dl[aria-label="Account details"]');
      await expect(details.locator("dt")).toHaveCount(4);
      await expect(details.locator("dt").first()).toHaveCSS(
        "font-size",
        "14px",
      );
      await expect(details.locator("dd").first()).toHaveCSS(
        "font-size",
        "16px",
      );
      await page.getByRole("tab", { name: "API tokens", exact: true }).click();
      await page
        .getByRole("button", { name: "Issue token", exact: true })
        .click();
      await page
        .getByLabel(/^Label/)
        .fill("CLI-with-a-long-token-label-for-layout");
      await page.getByRole("button", { name: "Issue", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "API token created" });
      await expect(
        dialog.getByRole("button", { name: "Done", exact: true }),
      ).toBeDisabled();
      await dialog.getByRole("checkbox").check();
      await dialog.getByRole("button", { name: "Done", exact: true }).click();
      const tokens = page.getByRole("list", { name: "Management API tokens" });
      await expect(tokens.getByRole("listitem")).toHaveCount(1);
      await expect(
        tokens.getByRole("button", { name: /^Revoke CLI/ }),
      ).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: `test-results/accounts-mui-detail-${width}-${mode}.png`,
        animations: "disabled",
        fullPage: true,
      });
    });
  }

  for (const kind of ["token", "setup link"]) {
    test(`${kind} confirmation stays visible on short mobile in ${mode}`, async ({
      page,
    }) => {
      await accessMock(page, [{ ...owner, password_ready: kind !== "setup link" }]);
      await page.setViewportSize({ width: 320, height: 480 });
      await page.goto(`/api/admin/console/#accounts/${owner.id}`);
      await page.getByLabel("Theme").selectOption(mode);
      if (kind === "token") {
        await page.getByRole("tab", { name: "API tokens", exact: true }).click();
        await page
          .getByRole("button", { name: "Issue token", exact: true })
          .click();
        await page.getByLabel(/^Label/).fill("CLI");
        await page.getByRole("button", { name: "Issue", exact: true }).click();
      } else {
        await page.route("**/accounts/*/password-setup", (route) =>
          route.fulfill({
            json: {
              account_id: owner.id,
              username: "owner",
              token: rawSetupToken,
              expires_at: "2099-01-01T00:00:00Z",
            },
          }),
        );
        await page.getByRole("tab", { name: "Security", exact: true }).click();
        await page
          .getByRole("button", { name: "Issue setup link", exact: true })
          .click();
        await expect(page.getByLabel(/^Username/)).toHaveValue("owner");
        await page
          .getByLabel("Current password")
          .fill("a private admin password");
        await page
          .getByRole("button", { name: "Issue link", exact: true })
          .click();
      }
      const dialog = page.getByRole("dialog", { name: kind === "token" ? "API token created" : `Save ${kind}` });
      const done = dialog.getByRole("button", { name: "Done", exact: true });
      const saved = dialog.getByRole("checkbox");
      await expect(done).toBeInViewport();
      await saved.scrollIntoViewIfNeeded();
      await expect(saved).toBeInViewport();
      await expect(done).toBeDisabled();
      await expect(
        dialog.getByRole("button", { name: "Close", exact: true }),
      ).toHaveCount(0);
      await page.keyboard.press("Escape");
      await expect(dialog).toBeVisible();
      expect(
        await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
      await expect(dialog.locator("..")).toHaveCSS("opacity", "1");
      await page.screenshot({
        path: `test-results/accounts-mui-${kind.replace(" ", "-")}-${mode}.png`,
        animations: "disabled",
      });
      await saved.check();
      await done.click();
      await expect(dialog).toHaveCount(0);
    });
  }
}
