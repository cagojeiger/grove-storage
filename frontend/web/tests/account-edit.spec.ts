import { expect, test } from "@playwright/test";
import { accessMock, owner, otherUser } from "./access-fixture";

const detail = `/api/admin/console/#accounts/${owner.id}`;
const acknowledgement = "I understand my current session will end.";

test("rename trims the label while preserving account identity", async ({ page }) => {
  const mock = await accessMock(page);
  await page.goto(detail);
  await page.getByRole("button", { name: "Edit name", exact: true }).click();
  await expect(page.getByLabel("Name", { exact: true })).toHaveValue(owner.display_name);
  const confirm = page.getByRole("button", { name: "Confirm", exact: true });
  await expect(confirm).toBeDisabled();
  await page.getByLabel("Name", { exact: true }).fill("   ");
  await expect(confirm).toBeDisabled();
  await page.getByLabel("Name", { exact: true }).fill("  New administrator  ");
  await confirm.click();
  await expect(page.getByRole("heading", { name: "New administrator", exact: true })).toBeVisible();
  expect(mock.writes.at(-1)?.body).toEqual({ operation: "name", display_name: "New administrator" });
  await expect(page).toHaveURL(new RegExp(owner.id));
  await page.reload();
  await expect(page.getByRole("heading", { name: "New administrator", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Accounts", exact: true }).click();
  await expect(page.getByRole("button", { name: /New administrator.*Active/ })).toBeVisible();
});

test("self demotion requires acknowledgement and refreshes permissions", async ({ page }) => {
  await accessMock(page, [owner, { ...otherUser, role: "admin" }]);
  await page.goto(detail);
  await page.getByRole("button", { name: "Change role", exact: true }).click();
  await page.getByLabel("Role", { exact: true }).selectOption("reader");
  const confirm = page.getByRole("button", { name: "Confirm", exact: true });
  await expect(confirm).toBeDisabled();
  await page.getByRole("checkbox", { name: "I understand I will lose access to Accounts." }).check();
  await confirm.click();
  await expect(page.getByRole("alert")).toHaveText("Admin access required.");
  await expect(page.getByRole("link", { name: "Accounts", exact: true })).toHaveCount(0);
});

for (const action of ["Disable", "Delete account"]) {
  test(`self ${action} requires confirmation and ends the session`, async ({ page }) => {
    await accessMock(page, [owner, { ...otherUser, role: "admin" }]);
    await page.goto(detail);
    await page.getByRole("button", { name: action, exact: true }).click();
    await page.getByLabel("Confirm account name").fill(owner.display_name);
    const confirm = page.getByRole("button", { name: "Confirm", exact: true });
    await expect(confirm).toBeDisabled();
    await page.getByRole("checkbox", { name: acknowledgement }).check();
    await confirm.click();
    await expect(page.getByLabel("Account token")).toBeVisible();
    await expect(page.getByRole("heading", { name: owner.display_name })).toHaveCount(0);
  });
}

test("other account changes do not claim to end the current session", async ({ page }) => {
  await accessMock(page);
  await page.goto(`/api/admin/console/#accounts/${otherUser.id}`);
  await page.getByRole("button", { name: "Disable", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: acknowledgement })).toHaveCount(0);
  await page.getByLabel("Confirm account name").fill(otherUser.display_name);
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await page.getByRole("button", { name: "Enable", exact: true }).click();
  await expect(page.getByText("Unexpired, unrevoked tokens become usable again. Previous sessions remain revoked.")).toBeVisible();
});

test("unknown rename outcome blocks resubmission", async ({ page }) => {
  const mock = await accessMock(page);
  let writes = 0;
  await page.route(`**/v1/accounts/${owner.id}`, route => {
    if (route.request().method() === "GET") return route.fallback();
    writes++;
    return route.abort("failed");
  });
  await page.goto(detail);
  await page.getByRole("button", { name: "Edit name", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Uncertain");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("outcome is unknown");
  await expect(page.getByRole("button", { name: "Confirm", exact: true })).toBeDisabled();
  expect(writes).toBe(1);
  expect(mock.accounts[0].display_name).toBe(owner.display_name);
});

test("deleted accounts and Root have no usable rename action", async ({ page }) => {
  await accessMock(page, [owner, { ...otherUser, deleted_at: "2026-09-27T00:00:00Z", is_active: false }]);
  await page.goto(`/api/admin/console/#accounts/${otherUser.id}`);
  await expect(page.getByRole("button", { name: "Edit name", exact: true })).toBeDisabled();
  await page.goto("/api/admin/console/#accounts/root");
  await expect(page.getByRole("heading", { name: "Root", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Edit name", exact: true })).toHaveCount(0);
});

for (const width of [320, 768, 1440]) for (const theme of ["light", "dark"]) {
  test(`account confirmation ${width}px ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 });
    await accessMock(page);
    await page.goto(detail);
    await page.getByLabel("Theme").selectOption(theme);
    await page.getByRole("button", { name: "Delete account", exact: true }).click();
    await expect(page.getByRole("checkbox", { name: acknowledgement })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    expect(await page.locator("dialog").evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    await page.screenshot({ path: `test-results/account-confirm-${width}-${theme}.png`, fullPage: true });
  });
}
