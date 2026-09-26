import { expect, test } from "@playwright/test";
import { storageMock } from "./storage-fixture";
import { envelope, intercept } from "./command-fixture";

test("built Overview draws unselected branches on first load, reload and return", async ({ page }) => {
  await storageMock(page);
  await intercept(page, "client.list", r => r.fulfill({ json: envelope("client.list", ["notegate"]) }));
  await intercept(page, "usage.clients", r => r.fulfill({ json: envelope("usage.clients", []) }));
  await intercept(page, "client.show", r => r.fulfill({ json: envelope("client.show", { id: "notegate", storage_id: "home-archive" }) }));
  await page.goto("http://127.0.0.1:5180/api/admin/console/#");
  await expect(page.getByRole("heading", { name: "Connections", exact: true })).toBeVisible();
  await expect(page.locator(".connection-paths path")).toHaveCount(2);
  await expect(page.locator(".connection-paths path.selected")).toHaveCount(0);
  await page.reload();
  await expect(page.locator(".connection-paths path")).toHaveCount(2);
  await page.getByRole("link", { name: "View all clients" }).click();
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Overview", exact: true }).click();
  await expect(page.locator(".connection-paths path")).toHaveCount(2);
  for (const theme of ["light", "dark"]) {
    await page.getByLabel("Theme").selectOption(theme);
    await expect(page.locator(".connection-paths path").first()).toHaveCSS("stroke-width", "2px");
    await expect(page.locator(".connection-paths path").first()).toHaveCSS("stroke", theme === "light" ? "rgb(130, 149, 139)" : "rgb(113, 135, 121)");
    await page.screenshot({ path: `test-results/overview-built-unselected-${theme}.png`, fullPage: true });
  }
});
