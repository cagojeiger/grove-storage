import { expect, test } from "@playwright/test";
import { accessMock, owner } from "./access-fixture";

const base = "/api/admin/console/#accounts";
const rows = Array.from({ length: 55 }, (_, i) => ({
  ...owner,
  id: `00000000-0000-0000-0000-${String(i).padStart(12, "0")}`,
  display_name: i === 0 ? "Unloaded needle" : `Account ${i}`,
  role: "writer" as const,
  is_active: i !== 1,
  deleted_at: i === 2 ? "2026-01-01T00:00:00Z" : null,
}));

test("search finds an unloaded account and survives detail navigation and reload", async ({ page }) => {
  await accessMock(page, rows);
  await page.goto(base);
  await expect(page.getByRole("link", { name: "Unloaded needle", exact: true })).toHaveCount(0);
  await page.getByRole("searchbox", { name: "Search accounts" }).fill("needle");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.getByRole("link", { name: "Unloaded needle", exact: true })).toBeVisible();
  await page.getByLabel("Account role").selectOption("writer");
  await page.getByRole("link", { name: "Unloaded needle", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Unloaded needle" })).toBeVisible();
  await page.reload();
  await page.getByRole("button", { name: "Accounts", exact: true }).click();
  await expect(page.getByRole("searchbox", { name: "Search accounts" })).toHaveValue("needle");
  await expect(page.getByLabel("Account role")).toHaveValue("writer");
  await expect(page.getByRole("link", { name: "Unloaded needle", exact: true })).toBeVisible();
});

test("previous and next cursors survive reload and reset on filter changes", async ({ page }) => {
  await accessMock(page, rows);
  await page.goto(base);
  await expect(page.getByRole("button", { name: "Previous page" })).toBeDisabled();
  await page.getByRole("button", { name: "Next page" }).click();
  await expect(page.getByRole("link", { name: "Unloaded needle", exact: true })).toBeVisible();
  await page.reload();
  await page.getByRole("button", { name: "Previous page" }).click();
  await expect(page.getByRole("link", { name: "Account 54", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Previous page" })).toBeDisabled();
  await page.getByRole("button", { name: "Next page" }).click();
  await page.getByLabel("Account status").selectOption("disabled");
  await expect(page.getByRole("row").filter({ has: page.getByRole("link", { name: "Account 1", exact: true }) }).filter({ hasText: "Disabled" })).toBeVisible();
  expect(page.url()).not.toMatch(/before=|after=/);
  await expect(page.getByRole("button", { name: "Next page" })).toBeDisabled();
  await page.getByLabel("Account status").selectOption("deleted");
  await page.getByRole("row").filter({ hasText: "Deleted" }).getByRole("link", { name: "Account 2", exact: true }).click();
  await expect(page.getByRole("button", { name: "Delete account" })).toBeDisabled();
});

test("empty search is not initial setup and roles and size reset pagination", async ({ page }) => {
  await accessMock(page, rows);
  await page.goto(`${base}?q=absent`);
  await expect(page.getByText("No matching accounts.")).toBeVisible();
  await expect(page.getByRole("link", { name: "Set up first Admin" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Create user", exact: true })).toBeEnabled();
  await page.goto(base);
  await page.getByLabel("Rows per page").selectOption("20");
  await expect(page.getByText("20 accounts on this page")).toBeVisible();
  await page.getByRole("button", { name: "Next page" }).click();
  await page.getByLabel("Account role").selectOption("reader");
  await expect(page.getByText("No matching accounts.")).toBeVisible();
  expect(page.url()).not.toMatch(/before=|after=/);
});

for (const width of [320, 768, 1440]) for (const theme of ["light", "dark"]) {
  test(`account list filters ${width}px ${theme}`, async ({ page }) => {
    await accessMock(page, rows);
    await page.setViewportSize({ width, height: 960 });
    await page.goto(base);
    await page.getByLabel("Theme").selectOption(theme);
    await expect(page.getByRole("link", { name: "Account 54", exact: true })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: `test-results/account-list-${width}-${theme}.png` });
    await page.getByRole("navigation", { name: "Account pagination" }).scrollIntoViewIfNeeded();
    await page.screenshot({ path: `test-results/account-list-footer-${width}-${theme}.png` });
  });
}
