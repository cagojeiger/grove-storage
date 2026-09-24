import assert from "node:assert/strict";
import { expect } from "@playwright/test";

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

export async function bootstrapChecks(page, origin, masterToken) {
  await page.goto(origin + "/api/admin/console/");
  await page
    .getByRole("link", { name: "Initial setup / recovery", exact: true })
    .click();
  await page.getByLabel("Master token").fill(masterToken);
  await page.getByRole("button", { name: "Verify master token" }).click();
  await page.getByLabel("Admin name").fill("Console test owner");
  await page.getByRole("button", { name: "Create Admin", exact: true }).click();
  const token = await takeToken(page);
  await page.getByLabel("Personal token").fill(token);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Overview", exact: true }),
  ).toBeVisible();
  const session = await page.evaluate(async () =>
    (await fetch("/api/admin/identity/v1/session")).json(),
  );
  console.log(
    "PASS real browser master bootstrap, one-time token and separate User login",
  );
  return { token, credentialId: session.credential_id };
}

export async function accessChecks(
  browser,
  page,
  origin,
  endpoint,
  masterToken,
) {
  await page.getByRole("link", { name: "Access", exact: true }).click();
  await page
    .getByRole("button", { name: /Console test owner.*Active/ })
    .click();
  await page.getByRole("button", { name: "Disable", exact: true }).click();
  await page.getByLabel("Confirm account name").fill("Console test owner");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Keep an active Admin");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("button", { name: "Users", exact: true }).click();
  await page.getByRole("button", { name: "Create user", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Recovery admin");
  await page.getByLabel("Role", { exact: true }).selectOption("admin");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await page.getByRole("button", { name: /Recovery admin.*Active/ }).click();
  const userId = await page.locator(".detail-fields dd").first().innerText();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("Recovery fixture");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  const oldToken = await takeToken(page);
  await page.getByRole("button", { name: "Users", exact: true }).click();
  await page.getByRole("button", { name: "Create user", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("CLI backup");
  await page.getByLabel("Role", { exact: true }).selectOption("operator");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await page.getByRole("button", { name: /CLI backup.*Active/ }).click();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("CLI key");
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
  const recovery = await browser.newContext({ ignoreHTTPSErrors: true });
  try {
    const other = await recovery.newPage();
    await other.goto(origin + "/api/admin/console/#setup");
    await other.getByLabel("Master token").fill(masterToken);
    await other.getByRole("button", { name: "Verify master token" }).click();
    await other.getByLabel("Admin user ID").fill(userId);
    await other
      .getByLabel("Replace this Admin's tokens and revoke its sessions")
      .check();
    await other.getByRole("button", { name: "Recover access" }).click();
    const newToken = await takeToken(other);
    assert.equal(await status(oldToken), 401);
    assert.equal(await status(newToken), 200);
  } finally {
    await recovery.close();
  }
  await page
    .getByRole("button", { name: "Delete account", exact: true })
    .click();
  await page.getByLabel("Confirm account name").fill("CLI backup");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByText("Deleted", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Overview", exact: true }).click();
  console.log(
    "PASS real Access CRUD, last Admin guard, User token use/revocation and targeted master recovery",
  );
}
