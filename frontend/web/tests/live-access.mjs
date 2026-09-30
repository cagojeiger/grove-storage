import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { expect } from "@playwright/test";
import { loginWithPassword } from "./live-auth.mjs";

async function takeToken(page) {
  const input = page.getByRole("textbox", {
    name: "Issued token",
    exact: true,
  });
  await expect(input).toBeVisible();
  const token = await input.inputValue();
  assert.match(token, /^gsm_[a-f0-9]{64}$/);
  assert(
    !(
      await page.evaluate(() =>
        JSON.stringify({ ...localStorage, ...sessionStorage }),
      )
    ).includes(token),
  );
  await page
    .getByLabel("I have saved this token. It is shown only once.")
    .check();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await expect(input).toHaveCount(0);
  return token;
}

export async function bootstrapChecks(page, origin, ownerPassword) {
  await page.goto(origin + "/api/admin/console/");
  await loginWithPassword(page, "owner", ownerPassword);
  await expect(
    page.getByRole("heading", { name: "Overview", exact: true }),
  ).toBeVisible();
  const session = await page.evaluate(async () =>
    (await fetch("/api/admin/identity/v1/session")).json(),
  );
  const issued = await page.evaluate(async (password) => {
    const response = await fetch("/api/admin/identity/v1/me/tokens", {
      method: "POST", headers: { "X-Grove-CSRF": "1", "Content-Type": "application/json" },
      body: JSON.stringify({ label: "Fixture token", expires_in_days: 1, current_password: password }),
    });
    if (response.status !== 201) throw new Error(`Token issuance returned ${response.status}`);
    return response.json();
  }, ownerPassword);
  assert.match(issued.token, /^gsm_[a-f0-9]{64}$/);
  console.log("PASS real local first Admin and password login");
  return { token: issued.token, credentialId: issued.credential_id, accountId: session.user_id };
}

export async function accessChecks(
  browser,
  page,
  origin,
  endpoint,
  database,
  currentPassword,
) {
  await page.getByRole("link", { name: "Accounts", exact: true }).click();
  await page
    .getByRole("link", { name: "Console test owner", exact: true })
    .click();
  await page.getByRole("button", { name: "Disable", exact: true }).click();
  await page.getByLabel("Confirm account name").fill("Console test owner");
  await page.getByRole("checkbox", { name: "I understand my current session will end." }).check();
  await page.getByRole("dialog").getByRole("button", { name: "Disable account", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Keep an active Admin");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("button", { name: "Accounts", exact: true }).click();
  await page.getByRole("button", { name: "Create user", exact: true }).click();
  await page.getByLabel(/^Name\s*\*?$/).fill("Recovery admin");
  await page.getByLabel(/^Username\s*\*?$/).fill("recovery-admin");
  await page.getByLabel(/^Role\s*\*?$/).selectOption("admin");
  await page.getByLabel("Your current password").fill(currentPassword);
  await page.getByRole("dialog").getByRole("button", { name: "Create user" }).click();
  await page.getByLabel("I have saved this setup link.").check();
  await page.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("heading", { name: "Recovery admin", exact: true })).toBeVisible();
  const userId = await page.locator(".detail-fields dd").first().innerText();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page.getByLabel(/^Label\s*\*?$/).fill("Recovery fixture");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  const oldToken = await takeToken(page);
  await page.getByRole("button", { name: "Accounts", exact: true }).click();
  await page.getByRole("button", { name: "Create user", exact: true }).click();
  await page.getByLabel(/^Name\s*\*?$/).fill("CLI backup");
  await page.getByLabel(/^Username\s*\*?$/).fill("cli-backup");
  await page.getByLabel(/^Role\s*\*?$/).selectOption("writer");
  await page.getByLabel("Your current password").fill(currentPassword);
  await page.getByRole("dialog").getByRole("button", { name: "Create user" }).click();
  await page.getByLabel("I have saved this setup link.").check();
  await page.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("heading", { name: "CLI backup", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page.getByLabel(/^Label\s*\*?$/).fill("CLI key");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  const automationToken = await takeToken(page);
  async function status(token) {
    const response = await fetch(endpoint + "/api/admin/commands/v1", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ protocol: 1, command: "status", input: {} }),
    });
    return response.status;
  }
  assert.equal(await status(automationToken), 200);
  await page.getByRole("button", { name: "Revoke CLI key" }).click();
  await page.getByLabel("Revoke CLI key and its sessions").check();
  await page.getByRole("button", { name: "Revoke", exact: true }).click();
  await expect(page.getByText("Revoked", { exact: true })).toBeVisible();
  assert.equal(await status(automationToken), 401);
  const recoveredPassword = "a separate private recovery phrase";
  const recovered = JSON.parse(execFileSync("python3", ["scripts/e2e-password-account.py", database, userId], {
    cwd: new URL("../../..", import.meta.url),
    env: { ...process.env, GROVE_E2E_PASSWORD: recoveredPassword, GROVE_E2E_USERNAME: "recovery-admin" },
    encoding: "utf8", timeout: 45000,
  }));
  assert.equal(recovered.account_id, userId);
  assert.equal(await status(oldToken), 401);
  const recovery = await browser.newContext({ ignoreHTTPSErrors: true });
  try {
    const other = await recovery.newPage();
    await other.goto(origin + "/api/admin/console/");
    await loginWithPassword(other, "recovery-admin", recoveredPassword);
    const issued = await other.evaluate(async (password) => {
      const response = await fetch("/api/admin/identity/v1/me/tokens", {
        method: "POST", headers: { "X-Grove-CSRF": "1", "Content-Type": "application/json" },
        body: JSON.stringify({ label: "Recovered key", expires_in_days: 1, current_password: password }),
      });
      if (response.status !== 201) throw new Error(`Recovered token issuance returned ${response.status}`);
      return response.json();
    }, recoveredPassword);
    assert.equal(await status(issued.token), 200);
  } finally {
    await recovery.close();
  }
  await page
    .getByRole("button", { name: "Delete account", exact: true })
    .click();
  await page.getByLabel("Confirm account name").fill("CLI backup");
  await page.getByRole("dialog").getByRole("button", { name: "Delete account", exact: true }).click();
  await expect(page.getByText("Deleted", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Overview", exact: true }).click();
  console.log(
    "PASS real Access CRUD, last Admin guard, User token use/revocation and targeted local recovery",
  );
}
