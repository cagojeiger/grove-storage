import { expect } from "@playwright/test";

export async function loginWithPassword(page, username, password) {
  await page.getByLabel("Username").fill(username);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByLabel("Username").waitFor({ state: "hidden" });
  await page.getByRole("heading", { name: "Overview", exact: true }).waitFor();
  await expect(page).toHaveURL(/\/api\/admin\/console\/#$/);
}
