import assert from "node:assert/strict";
import { expect } from "@playwright/test";
import { loginWithPassword } from "./live-auth.mjs";

export async function maintenanceChecks(browser, page, origin, ownerPassword) {
  await page
    .getByRole("button", { name: "Test connection", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText(
    "Bucket access verified",
  );
  await page.goto(`${origin}/api/admin/console/#activity`);
  await page
    .getByRole("button", { name: /storage.create/ })
    .first()
    .click();
  await expect(page.getByRole("dialog")).toContainText("Token ID");
  await expect(page.getByRole("dialog")).toContainText("Request ID");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page
    .getByRole("link", { name: "Command history", exact: true })
    .click();
  await page
    .getByRole("button", { name: /storage.test/ })
    .first()
    .click();
  await expect(page.getByRole("dialog")).toContainText("Duration (ms)");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page
    .getByRole("link", { name: "Security events", exact: true })
    .click();
  await expect(page.locator(".event-row").first()).toBeVisible();

  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  try {
    const other = await context.newPage();
    await other.goto(`${origin}/api/admin/console/#settings`);
    await loginWithPassword(other, "owner", ownerPassword);
    await expect(
      other.getByRole("heading", { name: "My sessions", exact: true }),
    ).toBeVisible();
    const otherId = await other.evaluate(
      async () =>
        (await (await fetch("/api/admin/identity/v1/session")).json())
          .session_id,
    );
    await page.goto(`${origin}/api/admin/console/#settings`);
    await page
      .getByRole("button", { name: `Revoke session ${otherId}`, exact: true })
      .click();
    await page
      .getByRole("button", { name: "Confirm revoke", exact: true })
      .click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    assert.equal(
      await other.evaluate(
        async () => (await fetch("/api/admin/identity/v1/session")).status,
      ),
      401,
    );
    assert.equal(
      await page.evaluate(
        async () => (await fetch("/api/admin/identity/v1/session")).status,
      ),
      200,
    );
    await page
      .getByRole("button", { name: "Revoke current session", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Confirm revoke", exact: true })
      .click();
    await expect(page.getByLabel("Password", { exact: true })).toBeVisible();
    await loginWithPassword(page, "owner", ownerPassword);
    await expect(
      page.getByRole("heading", { name: "My sessions", exact: true }),
    ).toBeVisible();
  } finally {
    await context.close();
  }
  await page.goto(`${origin}/api/admin/console/#storages/console-live`);
  console.log(
    "PASS real Activity audit/invocation/security views and own-session revocation with independent cookies",
  );
}
