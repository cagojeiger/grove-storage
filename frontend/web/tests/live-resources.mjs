import { expect } from "@playwright/test";

export async function resourceChecks(page, { origin }) {
  await page.goto(`${origin}/api/admin/console/#`);
  await expect(page.locator(".connection-paths path").first()).toBeAttached();
  await page
    .getByRole("link", { name: "Open client console-client", exact: true })
    .focus();
  await expect(page.getByRole("status")).toContainText(
    "console-client → console-live",
  );
  await expect(
    page.locator(".connection-item.selected .connection-title"),
  ).toContainText("console-live");
  await page
    .getByRole("link", { name: "View all clients", exact: true })
    .click();
  await expect(
    page.getByRole("table", { name: "Clients", exact: true }).locator("tbody tr").filter({ hasText: "console-client" }),
  ).toContainText("console-live");
  await page.getByLabel("Rows per page").selectOption("50");
  await page
    .locator("table[aria-label=Clients] tbody a")
    .filter({ hasText: "console-client" })
    .click();
  await expect(
    page.getByRole("heading", { name: "console-client", exact: true }),
  ).toBeVisible();
  await page
    .locator("main")
    .getByRole("link", { name: "Clients", exact: true })
    .click();
  await expect(page.getByLabel("Rows per page")).toHaveValue("50");
  console.log(
    "PASS real Client-to-Storage overview, assigned storage and list return state",
  );
}
