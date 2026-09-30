import { expect, test } from "@playwright/test";
import { example, storageMock } from "./storage-fixture";
import { session } from "./command-fixture";
import { maintenanceMock } from "./maintenance-fixture";

for (const width of [390, 768]) {
  test(`mobile navigation closes after resource and account navigation at ${width}px`, async ({ page }) => {
    await maintenanceMock(page);
    await page.setViewportSize({ width, height: 844 });
    await page.goto("/api/admin/console/#activity");
    const open = page.getByRole("button", { name: "Open navigation" });
    await expect(page.getByRole("navigation", { name: "Main navigation" })).toHaveCount(0);
    await open.click();
    await page.getByRole("button", { name: "Close navigation" }).click();
    await expect(open).toBeFocused();
    await open.click();
    await page.getByRole("link", { name: "Activity", exact: true }).click();
    await expect(page.getByRole("navigation", { name: "Main navigation" })).toHaveCount(0);
    await open.click();
    await page.getByRole("button", { name: "Account menu", exact: true }).click();
    await page.getByRole("link", { name: "My account", exact: true }).click();
    await expect(page.getByRole("heading", { name: "My account", exact: true })).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page.getByRole("contentinfo")).toHaveCount(0);
  });
}

test("storage edit deep link survives reload and cancellation retains the resource", async ({ page }) => {
  const { writes } = await storageMock(page);
  await page.goto(`/api/admin/console/#storages/${example.id}/edit`);
  await expect(page.getByRole("heading", { name: "Edit storage" })).toBeVisible();
  await expect(page.getByLabel("Storage ID", { exact: true })).toHaveValue(example.id);
  await page.reload();
  await expect(page.getByLabel("Secret key (re-enter)")).toHaveValue("");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.getByRole("heading", { name: example.id, exact: true })).toBeVisible();
  expect(writes).toEqual([]);
});

for (const route of ["storages/create/new", `storages/${example.id}/edit`]) {
  test(`reader cannot bypass write controls at ${route}`, async ({ page }) => {
    const { writes } = await storageMock(page);
    await page.route("**/identity/v1/session", r => r.fulfill({ json: { ...session, role: "reader" } }));
    await page.goto(`/api/admin/console/#${route}`);
    await expect(page.getByRole("alert")).toHaveText("Write access required.");
    await expect(page.getByRole("button", { name: "Save", exact: true })).toHaveCount(0);
    await expect(page.locator('input[type="password"]')).toHaveCount(0);
    expect(writes).toEqual([]);
  });
}

for (const id of ["create", "edit"]) test(`storage named ${id} is not mistaken for an editor route`, async ({ page }) => {
  await storageMock(page, [{ ...example, id }]);
  await page.goto(`/api/admin/console/#storages/${id}`);
  await expect(page.getByRole("heading", { name: id, exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Edit storage", exact: true }).click();
  await expect(page.getByLabel("Storage ID", { exact: true })).toHaveValue(id);
});
