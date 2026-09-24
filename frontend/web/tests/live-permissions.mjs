import assert from "node:assert/strict";
import { expect } from "@playwright/test";

export async function permissionChecks(browser, admin, origin) {
  async function identity(method, path, body) {
    return admin.evaluate(async ({ method, path, body }) => {
      const response = await fetch(`/api/admin/identity/v1${path}`, {
        method, headers: { "Content-Type": "application/json", "X-Grove-CSRF": "1" },
        body: body ? JSON.stringify(body) : undefined,
      });
      if (!response.ok) throw new Error(`Fixture identity request: ${response.status}`);
      return response.json();
    }, { method, path, body });
  }
  const user = await identity("POST", "/accounts", { kind: "user", role: "writer", display_name: "Console writer" });
  const credential = await identity("POST", `/accounts/${user.account_id}/credentials`, { label: "browser-test", expires_in_days: 1 });
  const automationKey = await identity("POST", `/accounts/${user.account_id}/credentials`, { label: "automation", expires_in_days: 1 });
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  try {
    const page = await context.newPage();
    await page.goto(`${origin}/api/admin/console/#storages/console-live`);
    await page.getByLabel("Personal token").fill(credential.token);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page.getByRole("button", { name: "Edit storage" }).click();
    await page.getByLabel("Secret key (re-enter)").fill("not-probed-after-demotion");
    await identity("PATCH", `/accounts/${user.account_id}`, { operation: "role", role: "reader" });
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByText("Reader · Read-only")).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Edit storage" })).toHaveCount(0);
    await page.reload();
    await expect(page.getByRole("region", { name: "Storage settings" })).toBeVisible();
    const blocked = await page.evaluate(async () => {
      const response = await fetch("/api/admin/console-commands/v1", {
        method: "POST", headers: { "Content-Type": "application/json", "X-Grove-CSRF": "1" },
        body: JSON.stringify({ protocol: 1, command: "storage.delete", input: { id: "console-live" } }),
      });
      return response.status;
    });
    assert.equal(blocked, 403);
    await page.getByRole("button", { name: "Sign out" }).click();
    await page.getByLabel("Personal token").fill(automationKey.token);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await expect(page.getByText("Reader · Read-only")).toBeVisible();
    assert.equal((await context.cookies()).filter((c) => c.name === "__Host-grove_session").length, 1);
    const audit = await identity("GET", "/history/audit?limit=100");
    const events = audit.items.filter((e) => e.action === "storage.create");
    assert(events.length > 0 && events.every((e) => e.context.surface === "console"));
    assert(!JSON.stringify(audit).includes(credential.token));
    console.log("PASS real role demotion, Reader read-only UI/server, named User token login, console audit");
  } finally { await context.close(); }
}
