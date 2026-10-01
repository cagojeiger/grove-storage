import { expect, test } from "@playwright/test";
import { storageMock, example } from "./storage-fixture";
import { envelope, intercept } from "./command-fixture";

test("leaving a pending storage save refreshes data without redirecting back", async ({ page }) => {
  const { rows, reads } = await storageMock(page);
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  let writes = 0;
  await intercept(page, "storage.replace", async route => {
    writes++;
    await gate;
    const saved = { ...example, capacity_bytes: 2 * 1024 ** 4 };
    rows.set(example.id, saved);
    await route.fulfill({ json: envelope("storage.replace", saved) });
  });
  await page.goto("/api/admin/console/#storages/home-archive/edit");
  await page.getByLabel(/^Secret key \(re-enter\)/).fill("test-only-secret");
  await page.getByLabel("Configured capacity", { exact: true }).fill("2");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("button", { name: "Saving...", exact: true })).toBeDisabled();
  await page.getByRole("link", { name: "Storage", exact: true }).click();
  await expect(page.getByRole("grid", { name: "Storage", exact: true })).toBeVisible();
  release();
  await expect.poll(() => reads.list).toBeGreaterThan(1);
  await expect(page.getByRole("grid").getByText("0 B / 2 TiB")).toBeVisible();
  await expect(page).toHaveURL(/#storages$/);
  expect(writes).toBe(1);
});
