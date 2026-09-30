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
    page
      .locator('[data-side="storage"][data-selected="true"]')
      .getByRole("link"),
  ).toContainText("console-live");
  await page
    .getByRole("link", { name: "View all clients", exact: true })
    .click();
  await expect(
    page
      .getByRole("grid", { name: "Clients", exact: true })
      .getByRole("row")
      .filter({ hasText: "console-client" }),
  ).toContainText("console-live");
  await page.getByRole("combobox", { name: "Rows per page:" }).click();
  await page.getByRole("option", { name: "50", exact: true }).click();
  await page
    .getByRole("grid", { name: "Clients", exact: true })
    .getByRole("link")
    .filter({ hasText: "console-client" })
    .click();
  await expect(
    page.getByRole("heading", { name: "console-client", exact: true }),
  ).toBeVisible();
  await page
    .locator("main")
    .getByRole("link", { name: "Clients", exact: true })
    .click();
  await expect(
    page.getByRole("combobox", { name: "Rows per page:" }),
  ).toHaveText("50");
  console.log(
    "PASS real Client-to-Storage overview, assigned storage and list return state",
  );
}
