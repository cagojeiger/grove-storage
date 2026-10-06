import { expect, test } from "@playwright/test";
import { accessMock, owner } from "./access-fixture";
import { session } from "./command-fixture";

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
  await page.getByRole("link", { name: "My account", exact: true }).click();
  await expect(page.getByLabel("Display name")).toHaveValue("Home administrator");
  await expect(page.getByText("owner", { exact: true })).toBeVisible();
  const save = page.getByRole("button", { name: "Save changes", exact: true });
  await expect(save).toBeDisabled();
  await page.getByLabel("Display name").fill("   ");
  await expect(save).toBeDisabled();
  await page.getByLabel("Display name").fill("  Profile display  ");
  await save.click();
  await expect(page.getByLabel("Display name")).toHaveValue("Profile display");
  await expect(save).toBeDisabled();
  await expect(page.getByRole("link", { name: "My account", exact: true })).toContainText("Profile display");
  await expect(page.getByText("owner", { exact: true })).toBeVisible();
  await page.getByLabel("Display name").fill("Home administrator");
  await save.click();
  await expect(page.getByLabel("Display name")).toHaveValue("Home administrator");
  await expect(page.getByLabel("Account ID", { exact: true })).toBeHidden();
  await page.getByRole("button", { name: "Technical details" }).click();
  await expect(page.getByLabel("Account ID", { exact: true })).toBeVisible();
});

test("uncertain profile save keeps the draft and prevents resubmission", async ({ page }) => {
  await accessMock(page);
  await page.route("**/identity/v1/session", (route) =>
    route.fulfill({ json: { ...session, user_id: owner.id, credential_id: null } }),
  );
  let writes = 0;
  await page.route("**/identity/v1/me", (route) => {
    if (route.request().method() === "GET") return route.fulfill({ json: owner });
    writes++;
    return route.abort("failed");
  });
  await page.goto("/api/admin/console/#settings");
  await page.getByLabel("Display name").fill("Uncertain name");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByRole("alert")).toContainText("outcome is unknown");
  await expect(page.getByLabel("Display name")).toHaveValue("Uncertain name");
  await expect(page.getByRole("button", { name: "Save changes" })).toBeDisabled();
  expect(writes).toBe(1);
});

test("technical details are collapsed and account ID can be copied", async ({ page, context }) => {
  await accessMock(page);
  await page.route("**/identity/v1/me", (route) => route.fulfill({ json: owner }));
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/api/admin/console/#settings");
  await expect(page.getByLabel("Account ID", { exact: true })).toBeHidden();
  await page.getByRole("button", { name: "Technical details" }).click();
  await expect(page.getByLabel("Account ID", { exact: true })).toHaveValue(owner.id);
  await page.getByRole("button", { name: "Copy account ID" }).click();
  await expect(page.getByRole("status")).toHaveText("Account ID copied.");
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(owner.id);
});
