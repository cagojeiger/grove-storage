import assert from "node:assert/strict";
import { expect } from "@playwright/test";
import { loginWithPassword } from "./live-auth.mjs";

export async function permissionChecks(browser, admin, origin, endpoint, ownerPassword) {
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
  const writerPassword = "a separate private writer phrase";
  const user = await identity("POST", "/accounts", { kind: "user_with_password_setup", role: "writer",
    display_name: "Console writer", username: "console-writer", current_password: ownerPassword });
  const setupStatus = await admin.evaluate(async ({ token, password }) => {
    const response = await fetch("/api/admin/identity/v1/password-setup", {
      method: "POST", headers: { "Content-Type": "application/json", "X-Grove-CSRF": "1" },
      body: JSON.stringify({ token, password }),
    });
    return response.status;
  }, { token: user.token, password: writerPassword });
  assert.equal(setupStatus, 204);
  const credential = await identity("POST", `/accounts/${user.account_id}/credentials`, { label: "browser-test", expires_in_days: 1 });
  const automationKey = await identity("POST", `/accounts/${user.account_id}/credentials`, { label: "automation", expires_in_days: 1 });
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  try {
    const page = await context.newPage();
    await page.goto(`${origin}/api/admin/console/#storages/console-live`);
    await loginWithPassword(page, "console-writer", writerPassword);
    await page.getByRole("button", { name: "Edit storage" }).click();
    await page.getByLabel("Secret key (re-enter)").fill("not-probed-after-demotion");
    await identity("PATCH", `/accounts/${user.account_id}`, { operation: "role", role: "reader" });
    const status = await admin.request.post(`${endpoint}/api/admin/commands/v1`, {
      headers: { Authorization: `Bearer ${automationKey.token}` },
      data: { protocol: 1, command: "status", input: {} },
    });
    assert.equal(status.status(), 200);
    const forbidden = await admin.request.post(`${endpoint}/api/admin/commands/v1`, {
      headers: { Authorization: `Bearer ${automationKey.token}` },
      data: { protocol: 1, command: "client.create", input: { id: "blocked", storage_id: "console-live" } },
    });
    assert.equal(forbidden.status(), 403);
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByRole("button", { name: "Account menu", exact: true })).toContainText("reader");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Edit storage" })).toHaveCount(0);
    await page.reload();
    await expect(page.getByRole("alert")).toHaveText("Write access required.");
    await expect(page.getByLabel("Secret key (re-enter)")).toHaveCount(0);
    await page.getByRole("navigation").getByRole("link", { name: "Storage", exact: true }).click();
    await page.getByRole("link", { name: "console-live", exact: true }).click();
    await page.getByRole("tab", { name: "Configuration", exact: true }).click();
    await expect(page.getByRole("region", { name: "Storage settings" })).toBeVisible();
    const blocked = await page.evaluate(async () => {
      const response = await fetch("/api/admin/console-commands/v1", {
        method: "POST", headers: { "Content-Type": "application/json", "X-Grove-CSRF": "1" },
        body: JSON.stringify({ protocol: 1, command: "storage.delete", input: { id: "console-live" } }),
      });
      return response.status;
    });
    assert.equal(blocked, 403);
    await page.locator('button[aria-label="Account menu"]').click();
    await page.getByRole("menuitem", { name: "Sign out" }).click();
    await loginWithPassword(page, "console-writer", writerPassword);
    await expect(page.getByRole("button", { name: "Account menu", exact: true })).toContainText("reader");
    assert.equal((await context.cookies()).filter((c) => c.name === "__Host-grove_session").length, 1);
    const audit = await identity("GET", "/history/audit?limit=100");
    const events = audit.items.filter((e) => e.action === "storage.create");
    assert(events.length > 0 && events.every((e) => e.context.surface === "console"));
    assert(!JSON.stringify(audit).includes(credential.token));
    await page.goto(`${origin}/api/admin/console/#activity`);
    await expect(page.getByRole("heading", { name: "My activity", exact: true })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Security events", exact: true })).toHaveCount(0);
    const scoped = await page.evaluate(async () => (await (await fetch("/api/admin/identity/v1/history/audit?limit=100")).json()));
    assert(scoped.items.every((event) => event.context.actor_id === user.account_id || event.context.owner_user_id === user.account_id));
    const calls = await page.evaluate(async () => (await (await fetch("/api/admin/identity/v1/history/invocations?limit=100")).json()));
    assert(calls.items.length > 0);
    assert(calls.items.every((event) => event.context.actor_id === user.account_id || event.context.owner_user_id === user.account_id));
    assert.equal(await page.evaluate(async () => (await fetch("/api/admin/identity/v1/history/security")).status), 403);
    await page.goto(`${origin}/api/admin/console/#settings`);
    await page.getByRole("tab", { name: "Sessions", exact: true }).click();
    await expect(page.getByRole("button", { name: "Revoke current session", exact: true })).toBeVisible();
    console.log("PASS real role demotion, Reader read-only UI/server, password login and console audit");
  } finally { await context.close(); }
}
