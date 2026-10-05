import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { chromium, expect } from "@playwright/test";
import { loginWithPassword } from "./live-auth.mjs";

let input = "";
for await (const chunk of process.stdin) input += chunk;
const { origin, ownerPassword } = JSON.parse(input);
const root = `${origin}/api/admin/console/`;
const output = new URL("../../../output/console-real-auth-20261004/", import.meta.url);
await mkdir(output, { recursive: true });
const browser = await chromium.launch();
const errors = [];
try {
  // Only the disposable fixture's self-signed certificate is accepted here.
  const contexts = await Promise.all([0, 1, 2].map(() =>
    browser.newContext({ ignoreHTTPSErrors: true, viewport: { width: 1440, height: 1000 } })));
  const pages = await Promise.all(contexts.map(context => context.newPage()));
  for (const page of pages) page.on("pageerror", error => errors.push(error.message));
  const [admin, reader, otherReader] = pages;
  await admin.goto(root);
  await loginWithPassword(admin, "owner", ownerPassword);
  await admin.getByRole("link", { name: "Accounts", exact: true }).click();
  await admin.getByRole("button", { name: "Create account", exact: true }).click();
  const dialog = admin.getByRole("dialog", { name: "Create account" });
  await expect(dialog.getByRole("heading", { name: "New account", exact: true })).toBeVisible();
  await expect(dialog.getByRole("heading", { name: "Administrator verification" })).toBeVisible();
  await dialog.getByLabel("Display name").fill("Reader verification");
  await dialog.getByLabel("Username").fill("Reader.Review");
  await expect(dialog.getByLabel("Role")).toHaveValue("reader");
  await admin.screenshot({ path: new URL("create-account.png", output).pathname, animations: "disabled" });
  await admin.setViewportSize({ width: 390, height: 844 });
  await expect.poll(() => admin.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await admin.screenshot({ path: new URL("create-account-mobile.png", output).pathname, animations: "disabled" });
  await admin.setViewportSize({ width: 1440, height: 1000 });
  await dialog.getByLabel("Administrator password").fill("an incorrect administrator password");
  await dialog.getByRole("button", { name: "Create account", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText("Current password is incorrect");
  await expect(dialog.getByLabel("Username")).toHaveValue("Reader.Review");
  await expect(dialog.getByLabel("Administrator password")).toHaveValue("");
  await dialog.getByLabel("Administrator password").fill(ownerPassword);
  await dialog.getByRole("button", { name: "Create account", exact: true }).click();
  const setupLink = await admin.getByRole("textbox", { name: "Setup link", exact: true }).inputValue();
  assert.match(setupLink, /#set-password\/gsps_[a-f0-9]{64}$/);
  await admin.getByLabel("I have saved this setup link.").check();
  await admin.getByRole("button", { name: "Done", exact: true }).click();
  await expect(admin.getByRole("heading", { name: "Reader verification", exact: true })).toBeVisible();

  await reader.goto(setupLink);
  await expect(reader.getByLabel("Username")).toHaveValue("reader.review");
  await expect(reader).not.toHaveURL(/gsps_/);
  const password = "an isolated reader browser passphrase";
  await reader.getByLabel(/^New password\s*\*?$/).fill(password);
  await reader.getByLabel("Confirm password").fill(password);
  await reader.getByRole("button", { name: "Set password", exact: true }).click();
  await expect(reader.getByText("Password set. Sign in with your username and password.")).toBeVisible();
  await reader.getByRole("link", { name: "Sign in", exact: true }).click();
  await loginWithPassword(reader, "reader.review", password);
  await otherReader.goto(root);
  await loginWithPassword(otherReader, "reader.review", password);

  const cookies = await Promise.all(contexts.map(context => context.cookies()));
  const sessions = cookies.map(rows => rows.find(cookie => cookie.name === "__Host-grove_session"));
  assert(sessions.every(cookie => cookie?.secure && cookie.httpOnly && cookie.sameSite === "Strict"));
  assert.equal(new Set(sessions.map(cookie => cookie.value)).size, 3);
  for (let index = 0; index < pages.length; index++) {
    const visible = await pages[index].evaluate(() =>
      JSON.stringify({ cookie: document.cookie, local: { ...localStorage }, session: { ...sessionStorage } }));
    assert(!visible.includes(sessions[index].value));
    assert(!visible.includes(password) && !visible.includes(ownerPassword));
  }
  await expect(reader.getByRole("link", { name: "Accounts", exact: true })).toHaveCount(0);
  assert.equal(await reader.evaluate(async () =>
    (await fetch("/api/admin/identity/v1/accounts")).status), 403);
  assert.equal(await reader.evaluate(async () =>
    (await fetch("/api/admin/identity/v1/session", { method: "DELETE" })).status), 403);
  await reader.getByRole("link", { name: "My account", exact: true }).click();
  await reader.getByRole("tab", { name: "Sessions", exact: true }).click();
  await expect(reader.getByRole("list", { name: "My sessions" }).getByRole("listitem")).toHaveCount(2);
  await reader.screenshot({ path: new URL("reader-sessions.png", output).pathname, animations: "disabled" });
  console.log("PASS real HTTPS: account creation/retry, recipient setup, independent cookies, Reader denial, CSRF, two browser sessions");

  await reader.getByRole("tab", { name: "Security", exact: true }).click();
  const replacement = "a replacement reader browser passphrase";
  await reader.getByLabel("Current password").fill(password);
  await reader.getByLabel(/^New password\s*\*?$/).fill(replacement);
  await reader.getByLabel("Confirm new password").fill(replacement);
  await reader.getByRole("button", { name: "Change password", exact: true }).click();
  await expect(reader.getByRole("button", { name: "Sign in", exact: true })).toBeVisible();
  await otherReader.reload();
  await expect(otherReader.getByRole("button", { name: "Sign in", exact: true })).toBeVisible();
  await admin.reload();
  await expect(admin.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Accounts", exact: true })).toBeVisible();
  await loginWithPassword(reader, "reader.review", replacement);
  await reader.getByRole("button", { name: "Sign out", exact: true }).click();
  await expect(reader.getByRole("button", { name: "Sign in", exact: true })).toBeVisible();
  assert(!(await contexts[1].cookies()).some(cookie => cookie.name === "__Host-grove_session"));
  assert.deepEqual(errors, []);
  console.log("PASS real HTTPS: password change signs out both Reader browsers, Admin remains signed in, re-login and logout");
} finally {
  await browser.close();
}
