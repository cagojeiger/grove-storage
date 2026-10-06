import { expect, test } from "@playwright/test";
import { accessMock, owner } from "./access-fixture";

test("mobile account status remains readable without truncated chips", async ({
  page,
}) => {
  await accessMock(page, [{ ...owner, password_ready: false }]);
  await page.setViewportSize({ width: 320, height: 844 });
  await page.goto("/api/admin/console/#accounts");
  const status = page.getByRole("grid").getByText("Pending setup", { exact: true });
  await expect(status).toBeVisible();
  expect(await status.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(
    true,
  );
  await expect(status.locator(".MuiChip-root")).toHaveCount(0);
});

test("account details preserve list context, reload and browser history", async ({
  page,
}) => {
  await accessMock(page);
  let tokenReads = 0;
  await page.route("**/accounts/*/credentials*", (route) => {
    tokenReads++;
    return route.fallback();
  });
  await page.goto(
    `/api/admin/console/#accounts/${owner.id}?q=Home&role=admin&limit=20`,
  );
  await expect(page.getByRole("heading", { name: owner.display_name })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Delete account", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "Management API tokens" }),
  ).toBeVisible();
  await expect.poll(() => tokenReads).toBeGreaterThan(0);
  await page.reload();
  await expect(
    page.getByRole("region", { name: "Management API tokens" }),
  ).toBeVisible();
  await page.getByRole("link", { name: "View account actions" }).click();
  await expect(page).toHaveURL(new RegExp(`#activity\\?account_id=${owner.id}`));
  await page.goBack();
  await expect(
    page.getByRole("region", { name: "Management API tokens" }),
  ).toBeVisible();
  await page
    .getByRole("main")
    .getByRole("link", { name: "Accounts", exact: true })
    .click();
  await expect(page.getByLabel("Search accounts")).toHaveValue("Home");
  await expect(page.getByLabel("Account role")).toHaveValue("admin");
  await expect(page.getByLabel("Page size")).toHaveValue("20");
  expect(page.url()).not.toContain("tab=");
});

test("legacy account tab links still expose keyboard-accessible actions", async ({
  page,
}) => {
  await accessMock(page);
  await page.goto(`/api/admin/console/#accounts/${owner.id}?tab=unknown`);
  await expect(page.getByRole("heading", { name: owner.display_name })).toBeVisible();
  const edit = page.getByRole("button", { name: "Edit name", exact: true });
  await edit.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(edit).toBeFocused();
});

for (const theme of ["light", "dark"]) {
  for (const width of [320, 768, 1440]) {
    test(`account workspace sections at ${width}px in ${theme}`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 900 });
      await accessMock(page);
      await page.goto(`/api/admin/console/#accounts/${owner.id}`);
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      for (const section of ["Edit name", "Issue token", "Delete account"]) {
        const action = page.getByRole("button", { name: section, exact: true });
        await action.scrollIntoViewIfNeeded();
        await expect(action).toBeInViewport();
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        await page.screenshot({
          path: `test-results/account-workspace-${section.replace(" ", "-")}-${width}-${theme}.png`,
          fullPage: true,
        });
      }
    });
  }
}
