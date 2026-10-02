import { expect, test } from "@playwright/test";
import { clientMock } from "./client-fixture";
import { storageMock } from "./storage-fixture";

test("storage tabs preserve capacity, list context and edit return", async ({
  page,
}) => {
  await storageMock(page);
  await page.goto(
    "/api/admin/console/#storages/home-archive?q=home&size=50&sort=desc",
  );
  await expect(
    page.getByRole("progressbar", { name: "Accounted capacity" }),
  ).toBeVisible();
  await expect(
    page.getByLabel("Saved metadata", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Test connection", exact: true }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "Configuration", exact: true }).click();
  await expect(
    page.getByRole("progressbar", { name: "Accounted capacity" }),
  ).toHaveCount(0);
  await expect(page.getByLabel("Storage properties")).toContainText("Configured capacity");
  await expect(
    page.getByLabel("Storage properties").locator("dd"),
  ).toHaveCount(9);
  await expect(page.getByLabel("Saved metadata", { exact: true })).toHaveCount(
    0,
  );
  await page.reload();
  await expect(
    page.getByRole("tab", { name: "Configuration", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await page.getByRole("button", { name: "Edit storage", exact: true }).click();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByRole("tab", { name: "Configuration", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await page.getByRole("link", { name: "Storage", exact: true }).last().click();
  await expect(page).toHaveURL(/#storages\?q=home&size=50&sort=desc$/);
  await expect(page.getByLabel("Search storage")).toHaveValue("home");
  await page
    .getByRole("grid")
    .getByRole("link", { name: "home-archive", exact: true })
    .click();
  await expect(
    page.getByRole("tab", { name: "Overview", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
});

test("client credentials load only when selected and support keyboard navigation", async ({
  page,
}) => {
  const { calls } = await clientMock(page);
  await page.goto("/api/admin/console/#clients/notegate?tab=unknown");
  const overview = page.getByRole("tab", { name: "Overview", exact: true });
  await expect(overview).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByLabel("Saved metadata", { exact: true }),
  ).toBeVisible();
  expect(
    calls.filter((call) => call.command === "credential.list"),
  ).toHaveLength(0);
  await overview.focus();
  await page.keyboard.press("ArrowDown");
  const credentials = page.getByRole("tab", {
    name: "S3 credentials",
    exact: true,
  });
  await expect(credentials).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("table", { name: "Issued S3 credentials" }),
  ).toBeVisible();
  expect(calls.some((call) => call.command === "credential.list")).toBe(true);
  await page.reload();
  await expect(credentials).toHaveAttribute("aria-selected", "true");
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
    page.getByRole("tab", { name: "S3 credentials", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Create credential", exact: true }),
  ).toHaveCount(0);
  expect(
    calls.filter((call) => call.command === "credential.list"),
  ).toHaveLength(0);
});
