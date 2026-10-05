import { expect, test } from "@playwright/test";
import { accessMock, otherUser, owner } from "./access-fixture";

const root = "/api/admin/console/";
const token = `gsps_${"b".repeat(64)}`;
const issued = { account_id: otherUser.id, username: "writer", expires_at: "2099-01-01T00:00:00Z", token };

test("Reader creation validates usernames and preserves non-secret fields on retry", async ({ page }) => {
  await accessMock(page, [owner]);
  const requests: Record<string, unknown>[] = [];
  await page.route("**/api/admin/identity/v1/accounts", async (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    const body = route.request().postDataJSON() as Record<string, unknown>;
    requests.push(body);
    await route.fulfill(requests.length === 1
      ? { status: 401, json: { error: "unauthenticated" } }
      : { status: 201, json: { ...issued, username: "reader.check" } });
  });
  await page.goto(`${root}#accounts`);
  await page.getByRole("button", { name: "Create account", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Create account" });
  await dialog.getByLabel("Display name").fill("Reader test");
  await dialog.getByLabel("Username").fill("reader@invalid");
  await dialog.getByLabel("Administrator password").fill("wrong-password");
  await dialog.getByRole("button", { name: "Create account", exact: true }).click();
  expect(requests).toHaveLength(0);
  await dialog.getByLabel("Username").fill("Reader.Check");
  await dialog.getByRole("button", { name: "Create account", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText("Current password is incorrect");
  await expect(dialog.getByLabel("Display name")).toHaveValue("Reader test");
  await expect(dialog.getByLabel("Username")).toHaveValue("Reader.Check");
  await expect(dialog.getByLabel("Role")).toHaveValue("reader");
  await expect(dialog.getByLabel("Administrator password")).toHaveValue("");
  await dialog.getByLabel("Administrator password").fill("private-admin-password");
  await dialog.getByRole("button", { name: "Create account", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Save setup link" })).toBeVisible();
  expect(requests).toHaveLength(2);
  expect(requests[1]).toEqual({
    kind: "user_with_password_setup", display_name: "Reader test",
    username: "reader.check", role: "reader", current_password: "private-admin-password",
  });
});

test("Admin issues a one-time setup link without persisting its secret", async ({ page }) => {
  await accessMock(page, [owner, { ...otherUser, password_ready: false }]);
  let body: unknown;
  await page.route(`**/api/admin/identity/v1/accounts/${otherUser.id}/password-setup`, async (route) => {
    body = route.request().postDataJSON();
    await route.fulfill({ json: issued });
  });
  await page.goto(`${root}#accounts/${otherUser.id}`);
  await page.getByRole("button", { name: "Issue setup link" }).click();
  await expect(page.getByLabel("Username")).toHaveValue("writer");
  await expect(page.getByLabel("Username")).toHaveAttribute("readonly", "");
  await page.getByLabel("Current password").fill("private-admin-password");
  await page.getByRole("button", { name: "Issue link" }).click();
  await expect.poll(() => body).toEqual({ username: "writer", current_password: "private-admin-password" });
  const link = page.getByRole("textbox", { name: "Setup link", exact: true });
  await expect(link).toHaveValue(new RegExp(`#set-password/${token}$`));
  expect(await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }))).not.toContain(token);
  await expect(page.getByRole("button", { name: "Done" })).toBeDisabled();
  await page.getByLabel("I have saved this setup link.").check();
  await page.getByRole("button", { name: "Done" }).click();
  await expect(link).toHaveCount(0);
});

for (const account of [
  owner,
  { ...otherUser, password_ready: false, is_active: false },
  { ...otherUser, password_ready: false, deleted_at: "2026-10-01T00:00:00Z" },
]) {
  test(`setup is unavailable for ${account.password_ready ? "initialized" : account.deleted_at ? "deleted" : "disabled"} accounts`, async ({ page }) => {
    const { writes } = await accessMock(page, account.id === owner.id ? [account] : [owner, account]);
    await page.goto(`${root}#accounts/${account.id}`);
    await expect(page.getByRole("button", { name: "Issue setup link" })).toHaveCount(0);
    await expect(page.getByRole("dialog")).toHaveCount(0);
    expect(writes).toEqual([]);
  });
}

test("recipient consumes setup link and returns to password sign-in", async ({ page }) => {
  let completions = 0;
  await page.route("**/api/admin/identity/v1/session", (route) => route.fulfill({ status: 401, json: {} }));
  await page.route("**/api/admin/identity/v1/password-setup/inspect", (route) => route.fulfill({ json: { username: "writer", expires_at: issued.expires_at } }));
  await page.route("**/api/admin/identity/v1/password-setup", async (route) => {
    const body = route.request().postDataJSON() as { token: string; password: string };
    expect(body).toEqual({ token, password: "a unique private phrase for setup" });
    completions++;
    await route.fulfill({ status: 204 });
  });
  await page.goto(`${root}#set-password/${token}`);
  await expect(page).toHaveURL(/#set-password\?link=1$/);
  await expect(page.getByRole("heading", { name: "Set password" })).toBeVisible();
  await expect(page.getByLabel("Username")).toHaveValue("writer");
  await page.getByLabel("New password").fill("a unique private phrase for setup");
  await page.getByLabel("Confirm password").fill("another private phrase");
  await page.getByRole("button", { name: "Set password" }).click();
  await expect(page.getByRole("alert")).toContainText("do not match");
  expect(completions).toBe(0);
  await page.getByLabel("Confirm password").fill("a unique private phrase for setup");
  await page.getByRole("button", { name: "Set password" }).click();
  await expect(page.getByText("Password set. Sign in with your username and password.")).toBeVisible();
  expect(completions).toBe(1);
  expect(await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }))).not.toContain(token);
  await page.getByRole("link", { name: "Sign in" }).click();
  await expect(page.getByLabel("Password")).toBeVisible();
});

for (const width of [320, 1440]) {
  for (const theme of ["light", "dark"])
    test(`setup form ${width}px ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 900 });
      await page.route("**/api/admin/identity/v1/session", (route) => route.fulfill({ status: 401, json: {} }));
      await page.route("**/api/admin/identity/v1/password-setup/inspect", (route) => route.fulfill({ json: { username: "writer", expires_at: issued.expires_at } }));
      await page.goto(`${root}#set-password/${token}`);
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      await expect(page.getByLabel("New password")).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: `test-results/password-setup-${width}-${theme}.png`, fullPage: true });
    });
}
