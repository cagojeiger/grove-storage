import { expect, test } from "@playwright/test";
import { failure, intercept, session } from "./command-fixture";
import { example, root, storageMock } from "./storage-fixture";

test("Reader can inspect storages but has no write controls", async ({ page }) => {
  await storageMock(page);
  await page.route("**/identity/v1/session", (route) => route.fulfill({ json: { ...session, role: "reader" } }));
  await page.goto(root);
  await expect(page.getByText("Reader · Read-only")).toBeVisible();
  await expect(page.getByRole("button", { name: "Register", exact: true })).toHaveCount(0);
  await page.getByRole("link", { name: new RegExp(example.id) }).click();
  await expect(page.getByRole("region", { name: "Storage settings" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Edit storage" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Delete storage" })).toHaveCount(0);
});

test("demotion while editing refreshes role and discards the form", async ({ page }) => {
  await storageMock(page);
  await page.goto(`${root}/${example.id}`);
  await page.getByRole("button", { name: "Edit storage" }).click();
  await page.getByLabel("Secret key (re-enter)").fill("discard-me");
  await page.route("**/identity/v1/session", (route) => route.fulfill({ json: { ...session, role: "reader" } }));
  await intercept(page, "storage.replace", (route) => route.fulfill({ status: 403, json: failure(403) }));
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("Reader · Read-only")).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByLabel("Secret key (re-enter)")).toHaveCount(0);
  await expect(page.getByRole("alert")).toHaveText("Write access required.");
  await expect(page.getByRole("button", { name: "Edit storage" })).toHaveCount(0);
});
