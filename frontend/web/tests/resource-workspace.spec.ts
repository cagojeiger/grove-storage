import { expect, test } from "@playwright/test";
import { clientMock } from "./client-fixture";
import { storageMock } from "./storage-fixture";

test("storage details preserve capacity, list context and edit return", async ({
  page,
}) => {
  await storageMock(page);
  await page.goto(
    "/api/admin/console/#storages/home-archive?q=home&size=50&sort=desc",
  );
  await expect(
    page.getByRole("progressbar", { name: "Configured capacity usage" }),
  ).toBeVisible();
  await expect(
    page.getByLabel("Saved metadata", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Test connection", exact: true }),
  ).toBeVisible();
  const properties = page.getByRole("region", { name: "Storage settings" }).locator("dl").first();
  await expect(properties).toContainText("Configured capacity");
  await expect(
    properties.locator("dd"),
  ).toHaveCount(9);
  await page.reload();
  await expect(
    page.getByLabel("Saved metadata", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Edit storage", exact: true }).click();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Edit storage", exact: true }),
  ).toBeFocused();
  await page.getByRole("link", { name: "Storage", exact: true }).last().click();
  await expect(page).toHaveURL(/#storages\?q=home&size=50&sort=desc$/);
  await expect(page.getByLabel("Search storage")).toHaveValue("home");
  await page
    .getByRole("grid")
    .getByRole("link", { name: "home-archive", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "Usage", exact: true }),
  ).toBeVisible();
});

test("client credentials survive legacy deep links and support keyboard actions", async ({
  page,
}) => {
  const { calls } = await clientMock(page);
  await page.goto("/api/admin/console/#clients/notegate?tab=unknown");
  await expect(
    page.getByLabel("Saved metadata", { exact: true }),
  ).toBeVisible();
  const credentials = page.getByRole("button", {
    name: "Create credential",
    exact: true,
  });
  await credentials.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("dialog", { name: "Create S3 credential" }),
  ).toBeVisible();
  expect(calls.some((call) => call.command === "credential.list")).toBe(true);
  await page.keyboard.press("Escape");
  await expect(credentials).toBeFocused();
  await page.reload();
  await expect(page.getByRole("region", { name: "S3 credentials" })).toBeVisible();
});

test("reader credential deep links do not expose or request keys", async ({
  page,
}) => {
  const { calls } = await clientMock(page, "reader");
  await page.goto("/api/admin/console/#clients/notegate?tab=credentials");
  await expect(
    page.getByLabel("Saved metadata", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "S3 credentials", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Create credential", exact: true }),
  ).toHaveCount(0);
  expect(
    calls.filter((call) => call.command === "credential.list"),
  ).toHaveLength(0);
});
