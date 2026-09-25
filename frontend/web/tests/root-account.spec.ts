import { test, expect } from "@playwright/test";
import { accessMock } from "./access-fixture";

for (const principal of ["user", "root"] as const) {
  test(`${principal} can inspect the protected Root without editing it`, async ({ page }) => {
    const { writes } = await accessMock(page);
    if (principal === "root") await page.route("**/v1/session", r => r.fulfill({ json: { principal: "root", role: "root", session_id: "root-session", expires_at: "2099-01-01T00:00:00Z" } }));
    await page.goto("/api/admin/console/#accounts");
    await expect(page.getByRole("heading", { name: "Accounts", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Root Config Protected Configured" }).click();
    await expect(page.getByRole("region", { name: "Root account" })).toBeVisible();
    await expect(page.getByText("Managed by server configuration")).toBeVisible();
    await expect(page.getByRole("button", { name: /Change role|Delete account|Disable|Issue token/ })).toHaveCount(0);
    expect(writes).toHaveLength(0);
  });
}
