import { expect, test } from "@playwright/test";

const root = "http://127.0.0.1:5180/api/admin/console/";

test("password user edits own display name without changing username or role", async ({ page }) => {
  await page.goto(root);
  await page.evaluate(async () => {
    await fetch("/api/admin/identity/v1/session", { method: "DELETE", headers: { "X-Grove-CSRF": "1" } });
  });
  await page.reload();
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password").fill("a private phrase for preview");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.locator('summary[aria-label="Account menu"]').click();
  await page.getByRole("link", { name: "My account" }).click();
  await expect(page.getByRole("heading", { name: "Home administrator" })).toBeVisible();
  await expect(page.getByText("owner", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Edit my name" }).click();
  await page.getByRole("dialog").getByLabel("Name").fill("Profile display");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Profile display" })).toBeVisible();
  await expect(page.getByText("owner", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Edit my name" }).click();
  await page.getByRole("dialog").getByLabel("Name").fill("Home administrator");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Home administrator" })).toBeVisible();
});
