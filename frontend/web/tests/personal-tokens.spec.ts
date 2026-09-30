import { expect, test } from "@playwright/test";

const root = "http://127.0.0.1:5180/api/admin/console/";
const password = "a private phrase for preview";

async function signIn(page: import("@playwright/test").Page) {
  await page.goto(root);
  await page.evaluate(async () => {
    await fetch("/api/admin/identity/v1/session", { method: "DELETE", headers: { "X-Grove-CSRF": "1" } });
  });
  await page.reload();
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.locator('button[aria-label="Account menu"]').click();
  await page.getByRole("link", { name: "My account" }).click();
  await expect(page.getByRole("heading", { name: "My API tokens" })).toBeVisible();
}

test("preview password session lasts eight hours", async ({ page }) => {
  await signIn(page);
  const session = await page.evaluate(async () => {
    const response = await fetch("/api/admin/identity/v1/me/sessions?limit=50");
    const body = (await response.json()) as { items: Array<{ created_at: string; expires_at: string }> };
    return body.items[0];
  });
  expect(Date.parse(session.expires_at) - Date.parse(session.created_at)).toBe(8 * 60 * 60 * 1000);
});

test("password user manages only own API tokens", async ({ page }) => {
  await signIn(page);
  await page.getByRole("region", { name: "My API tokens" }).getByRole("button", { name: "Issue token" }).click();
  await page.getByLabel("Label", { exact: true }).fill("Laptop CLI");
  await page.getByLabel("Expires in days").fill("7");
  await page.getByRole("dialog").getByLabel("Current password").fill("wrong password");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Current password is incorrect");
  await expect(page.getByRole("heading", { name: "My account", includeHidden: true })).toBeAttached();
  await page.getByLabel("Label", { exact: true }).fill("Laptop CLI");
  await page.getByLabel("Expires in days").fill("7");
  await page.getByRole("dialog").getByLabel("Current password").fill(password);
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  const token = await page.getByRole("textbox", { name: "Issued token" }).inputValue();
  expect(token).toMatch(/^gsm_[a-f0-9]{64}$/);
  expect(await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }))).not.toContain(token);
  await page.getByLabel("I have saved this token. It is shown only once.").check();
  await page.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("region", { name: "My API tokens" }).getByText("Laptop CLI")).toBeVisible();
  await page.getByRole("button", { name: "Revoke Laptop CLI" }).click();
  await page.getByRole("checkbox", { name: "Revoke Laptop CLI" }).check();
  await page.getByRole("button", { name: "Revoke", exact: true }).click();
  await expect(page.getByRole("region", { name: "My API tokens" }).getByText("Revoked")).toBeVisible();
});

for (const width of [320, 1440]) for (const theme of ["light", "dark"])
  test(`personal tokens ${width}px ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await signIn(page);
    await page.getByLabel("Theme").selectOption(theme);
    await page.getByRole("region", { name: "My API tokens" }).getByRole("button", { name: "Issue token" }).click();
    await expect(page.getByLabel("Expires in days")).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: `test-results/personal-tokens-${width}-${theme}.png`, fullPage: true });
  });
