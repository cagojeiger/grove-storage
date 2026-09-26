import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { storageChecks } from "./live-storages.mjs";
import { bootstrapChecks, accessChecks } from "./live-access.mjs";
import { permissionChecks } from "./live-permissions.mjs";
import { clientChecks } from "./live-clients.mjs";
import { maintenanceChecks } from "./live-maintenance.mjs";
import { usageChecks } from "./live-usage.mjs";
import { resourceChecks } from "./live-resources.mjs";

let input = "";
for await (const chunk of process.stdin) input += chunk;
const fixture = JSON.parse(input);
const { origin, masterToken, database, endpoint } = fixture;
const browser = await chromium.launch();
try {
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const { token, credentialId } = await bootstrapChecks(page, origin, masterToken);
  await page.getByRole("heading", { name: "Overview", exact: true }).waitFor();
  await page.getByText("No storage registered.").waitFor();
  await storageChecks(page, fixture);
  await clientChecks(page, fixture);
  await resourceChecks(page, fixture);
  await usageChecks(page, fixture);
  await maintenanceChecks(browser, page, origin, token);
  await permissionChecks(browser, page, origin);
  await accessChecks(browser, page, origin, endpoint, masterToken);
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
  await page.reload();
  await page.getByRole("heading", { name: "Overview", exact: true }).waitFor();
  const csrf = await page.evaluate(
    async () =>
      (await fetch("/api/admin/identity/v1/session", { method: "DELETE" })).status,
  );
  assert.equal(csrf, 403);
  await page.getByRole("button", { name: "Sign out" }).click();
  await page.getByLabel("Account token").waitFor();
  assert(
    !(await context.cookies()).some(
      (cookie) => cookie.name === "__Host-grove_session",
    ),
  );
  await page.reload();
  await page.getByLabel("Account token").waitFor();
  console.log(
    "PASS real HTTPS login, Secure/HttpOnly cookie, reload, CSRF rejection, logout",
  );
  async function login() {
    await page.getByLabel("Account token").fill(token);
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await page.getByRole("heading", { name: "console-live" }).waitFor();
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
      "UPDATE management.sessions SET created_at = now() - interval '1 day', expires_at = now() - interval '1 second' WHERE auth_method='token'",
    ],
    { timeout: 10000, stdio: "pipe" },
  );
  await page.getByRole("button", { name: "Refresh" }).click();
  await page.getByLabel("Account token").waitFor();
  assert.equal(
    await page.getByRole("heading", { name: "console-live" }).count(),
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
  await page.getByLabel("Account token").waitFor();
  await page.getByLabel("Account token").fill(token);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByRole("alert").waitFor();
  assert.equal(await page.getByLabel("Account token").inputValue(), "");
  console.log(
    "PASS real storage overview, session expiry, token revocation, and private cache removal",
  );
  assert.deepEqual(errors, []);
} finally {
  await browser.close();
}
