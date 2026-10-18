import assert from "node:assert/strict";
import { expect } from "@playwright/test";
import { loginWithPassword } from "./live-auth.mjs";
import { openInvocation } from "./activity-history.mjs";

export async function maintenanceChecks(browser, page, origin, ownerPassword) {
  await page
    .getByRole("button", { name: "Test connection", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText(
    "Bucket access verified",
  );
  await page.goto(`${origin}/api/admin/console/#activity`);
  await page
    .getByRole("button", { name: /Create storage/ })
    .first()
    .click();
  await page.getByRole("button", { name: "Technical details", exact: true }).click();
  await expect(page.getByRole("dialog").getByText("Token ID", { exact: true })).toBeVisible();
  await expect(page.getByLabel("Operation ID", { exact: true })).toHaveValue(
    /^[0-9a-f-]{36}$/,
  );
  const operationId = await page.getByLabel("Operation ID", { exact: true }).inputValue();
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page
    .getByRole("tab", { name: "Operation history", exact: true })
    .click();
  await page
    .getByRole("button", { name: /Test storage/ })
    .first()
    .click();
  await expect(page.getByRole("dialog")).toContainText("Duration");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.goto(`${origin}/api/admin/console/#activity`);
  await expect(page.getByRole("tab", { name: "Audit log", exact: true })).toHaveAttribute("aria-selected", "true");
  await openInvocation(page, operationId);
  await expect(page.getByRole("dialog")).toContainText("Succeeded");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page
    .getByRole("tab", { name: "Security events", exact: true })
    .click();
  await expect(page.getByRole("grid", { name: "Activity", exact: true }).getByRole("row").nth(1)).toBeVisible();

  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  try {
    const other = await context.newPage();
    await other.goto(`${origin}/api/admin/console/#settings`);
    await loginWithPassword(other, "owner", ownerPassword);
    await other.getByRole("link", { name: "My account", exact: true }).click();
    await other.getByRole("tab", { name: "Sessions", exact: true }).click();
    await expect(
      other.getByRole("heading", { name: "My sessions", exact: true }),
    ).toBeVisible();
    const otherId = await other.evaluate(
      async () =>
        (await (await fetch("/api/admin/identity/v1/session")).json())
          .session_id,
    );
    await page.goto(`${origin}/api/admin/console/#settings`);
    await page.getByRole("tab", { name: "Sessions", exact: true }).click();
    await page
      .getByRole("button", { name: `Revoke session ${otherId}`, exact: true })
      .click();
    await page.getByLabel("Confirmation").fill("REVOKE");
    await page.getByRole("button", { name: "Confirm", exact: true }).click();
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
    await page.getByLabel("Confirmation").fill("REVOKE");
    await page.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(page.getByLabel(/^Password\s*\*?$/)).toBeVisible();
    await loginWithPassword(page, "owner", ownerPassword);
    await page.getByRole("link", { name: "My account", exact: true }).click();
    await page.getByRole("tab", { name: "Sessions", exact: true }).click();
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
