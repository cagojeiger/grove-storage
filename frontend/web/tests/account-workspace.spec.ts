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

test("account tabs preserve list context, reload and browser history", async ({
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
  await expect(
    page.getByRole("tab", { name: "Overview", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tabpanel", { name: "Overview" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Delete account", exact: true }),
  ).toHaveCount(0);
  expect(tokenReads).toBe(0);
  await page.getByRole("tab", { name: "API tokens", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Management API tokens" }),
  ).toBeVisible();
  expect(tokenReads).toBeGreaterThan(0);
  await page.reload();
  await expect(
    page.getByRole("tab", { name: "API tokens", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await page.getByRole("tab", { name: "Security", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Delete account", exact: true }),
  ).toBeVisible();
  await page.goBack();
  await expect(
    page.getByRole("tabpanel", { name: "API tokens" }),
  ).toBeVisible();
  await page
    .getByRole("main")
    .getByRole("link", { name: "Accounts", exact: true })
    .click();
  await expect(page.getByLabel("Search accounts")).toHaveValue("Home");
  await expect(page.getByLabel("Account role")).toHaveValue("admin");
  await expect(page.getByLabel("Rows per page")).toHaveValue("20");
  expect(page.url()).not.toContain("tab=");
});

test("unknown account tabs use overview and keyboard navigation reaches Security", async ({
  page,
}) => {
  await accessMock(page);
  await page.goto(`/api/admin/console/#accounts/${owner.id}?tab=unknown`);
  const overview = page.getByRole("tab", { name: "Overview", exact: true });
  await expect(overview).toHaveAttribute("aria-selected", "true");
  await overview.focus();
  await expect(page.getByRole("tablist", { name: "Account sections" })).toHaveAttribute("aria-orientation", "vertical");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tabpanel", { name: "Security" })).toBeVisible();
});

for (const theme of ["light", "dark"]) {
  for (const width of [320, 768, 1440]) {
    test(`account workspace sections at ${width}px in ${theme}`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 900 });
      await accessMock(page);
      await page.goto(`/api/admin/console/#accounts/${owner.id}`);
      await page.getByLabel("Theme").selectOption(theme);
      for (const section of ["Overview", "API tokens", "Security"]) {
        await page.getByRole("tab", { name: section, exact: true }).click();
        const panel = page.getByRole("tabpanel", {
          name: section,
          exact: true,
        });
        await expect(panel).toBeVisible();
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
