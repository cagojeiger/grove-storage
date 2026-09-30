import assert from "node:assert/strict";
import { chromium, expect } from "@playwright/test";
import { execFileSync, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { storageChecks } from "./live-storages.mjs";
import { bootstrapChecks, accessChecks } from "./live-access.mjs";
import { permissionChecks } from "./live-permissions.mjs";
import { clientChecks } from "./live-clients.mjs";
import { maintenanceChecks } from "./live-maintenance.mjs";
import { usageChecks } from "./live-usage.mjs";
import { resourceChecks } from "./live-resources.mjs";
import { metadataChecks } from "./live-metadata.mjs";
import { loginWithPassword } from "./live-auth.mjs";

let input = "";
for await (const chunk of process.stdin) input += chunk;
const fixture = JSON.parse(input);
const { origin, ownerPassword, ownerId, database, endpoint } = fixture;
const browser = await chromium.launch();
try {
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const { token, credentialId, accountId } = await bootstrapChecks(page, origin, ownerPassword);
  assert.equal(accountId, ownerId);
  await page.getByRole("heading", { name: "Overview", exact: true }).waitFor();
  await page.getByText("No storage registered.").waitFor();
  await storageChecks(page, fixture);
  await clientChecks(page, fixture);
  await resourceChecks(page, fixture);
  await metadataChecks(page, fixture);
  await usageChecks(page, fixture);
  await maintenanceChecks(browser, page, origin, ownerPassword);
  await permissionChecks(browser, page, origin, endpoint, ownerPassword);
  const cookie = (await context.cookies()).find(
    (cookie) => cookie.name === "__Host-grove_session",
  );
  assert(cookie?.secure && cookie.httpOnly && cookie.sameSite === "Strict");
  assert(!(await page.evaluate(() => document.cookie)).includes(cookie.value));
  assert(
    !(
      await page.evaluate(() =>
        JSON.stringify({ ...localStorage, ...sessionStorage }),
      )
    ).includes(token),
  );
  await page.goto(`${origin}/api/admin/console/#`);
  await page.reload();
  await page.getByRole("heading", { name: "Overview", exact: true }).waitFor();
  const csrf = await page.evaluate(
    async () =>
      (await fetch("/api/admin/identity/v1/session", { method: "DELETE" })).status,
  );
  assert.equal(csrf, 403);
  await page.locator('button[aria-label="Account menu"]').click();
  await page.getByRole("button", { name: "Sign out" }).click();
  await page.getByLabel("Password", { exact: true }).waitFor();
  assert(
    !(await context.cookies()).some(
      (cookie) => cookie.name === "__Host-grove_session",
    ),
  );
  await page.reload();
  await page.getByLabel("Password", { exact: true }).waitFor();
  console.log(
    "PASS real HTTPS login, Secure/HttpOnly cookie, reload, CSRF rejection, logout",
  );
  async function login() {
    await loginWithPassword(page, "owner", ownerPassword);
    await page.getByRole("button", { name: "Select storage console-live", exact: true }).waitFor();
  }
  await login();
  execFileSync(
    "docker",
    [
      "exec",
      database,
      "psql",
      "-U",
      "filegate",
      "-d",
      "filegate",
      "-v",
      "ON_ERROR_STOP=1",
      "-c",
      "UPDATE management.sessions SET created_at = now() - interval '1 day', expires_at = now() - interval '1 second' WHERE auth_method='password'",
    ],
    { timeout: 10000, stdio: "pipe" },
  );
  await page.getByRole("button", { name: "Refresh" }).click();
  await page.getByLabel("Password", { exact: true }).waitFor();
  assert.equal(
    await page.getByRole("button", { name: "Select storage console-live", exact: true }).count(),
    0,
  );
  await login();
  const revoked = await page.evaluate(async (id) => {
    const response = await fetch(`/api/admin/identity/v1/credentials/${id}`, {
      method: "DELETE", headers: { "X-Grove-CSRF": "1" },
    });
    return response.status;
  }, credentialId);
  assert.equal(revoked, 200);
  await page.getByRole("button", { name: "Refresh" }).click();
  await page.getByRole("heading", { name: "Overview", exact: true }).waitFor();
  const rejected = await fetch(`${endpoint}/api/admin/commands/v1`, {
    method: "POST", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ protocol: 1, command: "status", input: {} }),
  });
  assert.equal(rejected.status, 401);
  const password = "an isolated browser integration passphrase";
  const recovered = JSON.parse(execFileSync("python3", ["scripts/e2e-password-account.py", database, accountId], {
    cwd: new URL("../../..", import.meta.url),
    env: { ...process.env, GROVE_E2E_PASSWORD: password },
    encoding: "utf8", timeout: 45000,
  }));
  assert.equal(recovered.account_id, accountId);
  await page.getByRole("button", { name: "Refresh" }).click();
  await page.getByLabel("Password", { exact: true }).waitFor();
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByRole("heading", { name: "Overview", exact: true }).waitFor();
  await page.goto(`${origin}/api/admin/console/#settings/security`);
  const replacement = "a different private browser integration passphrase";
  await page.getByLabel("Current password").fill(password);
  await page.getByLabel("New password", { exact: true }).fill(replacement);
  await page.getByLabel("Confirm new password").fill(replacement);
  await page.getByRole("button", { name: "Change password" }).click();
  await page.getByRole("button", { name: "Sign in", exact: true }).waitFor();
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password", { exact: true }).fill(replacement);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByRole("heading", { name: "Security", exact: true }).waitFor();
  await page.getByRole("main").getByRole("link", { name: "My account", exact: true }).click();
  await page.getByRole("heading", { name: "My account", exact: true }).waitFor();
  await page.getByRole("button", { name: "Edit my name" }).click();
  await page.getByRole("dialog").getByLabel("Name").fill("Console owner updated");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await page.getByRole("heading", { name: "Console owner updated", exact: true }).waitFor();
  await page.getByRole("button", { name: "Edit my name" }).click();
  await page.getByRole("dialog").getByLabel("Name").fill("Console test owner");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await page.getByRole("heading", { name: "Console test owner", exact: true }).waitFor();
  console.log("PASS real local recovery, password login, password change and re-login");
  await page.getByRole("link", { name: "Accounts", exact: true }).click();
  await page.getByRole("button", { name: "Create user", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Setup recipient");
  await page.getByLabel("Username", { exact: true }).fill("recipient");
  await page.getByLabel("Role", { exact: true }).selectOption("reader");
  await page.getByLabel("Current password").fill(replacement);
  await page.getByRole("dialog").getByRole("button", { name: "Create user" }).click();
  const setupLink = await page.getByRole("textbox", { name: "Setup link", exact: true }).inputValue();
  assert.match(setupLink, /#set-password\/gsps_[a-f0-9]{64}$/);
  await page.getByLabel("I have saved this setup link.").check();
  await page.getByRole("button", { name: "Done" }).click();
  await page.getByRole("heading", { name: "Setup recipient", exact: true }).waitFor();
  const recipientContext = await browser.newContext({ ignoreHTTPSErrors: true });
  try {
    const recipient = await recipientContext.newPage();
    await recipient.goto(setupLink);
    await expect(recipient).not.toHaveURL(/gsps_/);
    await recipient.getByLabel("Username").waitFor();
    assert.equal(await recipient.getByLabel("Username").inputValue(), "recipient");
    const recipientPassword = "a separate private phrase for recipient";
    await recipient.getByLabel("New password").fill(recipientPassword);
    await recipient.getByLabel("Confirm password").fill(recipientPassword);
    await recipient.getByRole("button", { name: "Set password" }).click();
    await recipient.getByText("Password set. Sign in with your username and password.").waitFor();
    await recipient.getByRole("link", { name: "Sign in" }).click();
    await recipient.getByLabel("Username").fill("recipient");
    await recipient.getByLabel("Password", { exact: true }).fill(recipientPassword);
    await recipient.getByRole("button", { name: "Sign in", exact: true }).click();
    await recipient.getByText("Reader · Read-only").waitFor();
    await expect(recipient.getByRole("link", { name: "Accounts", exact: true })).toHaveCount(0);
    await recipient.locator('button[aria-label="Account menu"]').click();
    await recipient.getByRole("link", { name: "My account" }).click();
    await recipient.getByRole("region", { name: "My API tokens" }).getByRole("button", { name: "Issue token" }).click();
    await recipient.getByLabel("Label", { exact: true }).fill("Reader CLI");
    await recipient.getByLabel("Expires in days").fill("1");
    await recipient.getByRole("dialog").getByLabel("Current password").fill(recipientPassword);
    await recipient.getByRole("button", { name: "Issue", exact: true }).click();
    const readerToken = await recipient.getByRole("textbox", { name: "Issued token" }).inputValue();
    assert.match(readerToken, /^gsm_[a-f0-9]{64}$/);
    await recipient.getByLabel("I have saved this token. It is shown only once.").check();
    await recipient.getByRole("button", { name: "Done" }).click();
    async function readerStatus() {
      return (await fetch(`${endpoint}/api/admin/commands/v1`, {
        method: "POST",
        headers: { Authorization: `Bearer ${readerToken}`, "Content-Type": "application/json" },
        body: JSON.stringify({ protocol: 1, command: "status", input: {} }),
      })).status;
    }
    function readerCli() {
      return spawnSync(fileURLToPath(new URL("../../../target/debug/gscli", import.meta.url)),
        ["--output", "json", "status"], {
          env: { ...process.env, GROVE_ENDPOINT: endpoint, GROVE_TOKEN: readerToken },
          encoding: "utf8", timeout: 10000,
        });
    }
    async function readerMcp() {
      return fetch(`${endpoint}/api/admin/mcp`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${readerToken}`,
          Accept: "application/json, text/event-stream",
          "Content-Type": "application/json",
          "Mcp-Method": "tools/call",
          "Mcp-Name": "status",
          "Mcp-Protocol-Version": "2026-07-28",
        },
        body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: {
          name: "status", arguments: {}, _meta: {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {},
          },
        } }),
      });
    }
    assert.equal(await readerStatus(), 200);
    const cli = readerCli();
    assert.equal(cli.status, 0, cli.stderr);
    assert.equal(JSON.parse(cli.stdout).ok, true);
    const mcp = await readerMcp();
    assert.equal(mcp.status, 200);
    assert.equal((await mcp.json()).result.structuredContent.result.registry.storage_count, 1);
    const identityDenied = await fetch(`${endpoint}/api/admin/identity/v1/accounts`, {
      headers: { Authorization: `Bearer ${readerToken}` },
    });
    assert.equal(identityDenied.status, 401);
    await recipient.getByRole("button", { name: "Revoke Reader CLI" }).click();
    await recipient.getByRole("checkbox", { name: "Revoke Reader CLI" }).check();
    await recipient.getByRole("button", { name: "Revoke", exact: true }).click();
    await recipient.getByRole("region", { name: "My API tokens" }).getByText("Revoked").waitFor();
    assert.equal(await readerStatus(), 401);
    assert.notEqual(readerCli().status, 0);
    assert.equal((await readerMcp()).status, 401);
    console.log("PASS real Reader personal token, CLI/MCP/Resource parity and immediate revocation");
  } finally {
    await recipientContext.close();
  }
  console.log("PASS real one-time account setup link, recipient password and Reader login");
  await accessChecks(browser, page, origin, endpoint, database, replacement);
  console.log(
    "PASS real storage overview, session expiry, token revocation, and private cache removal",
  );
  assert.deepEqual(errors, []);
} finally {
  await browser.close();
}
