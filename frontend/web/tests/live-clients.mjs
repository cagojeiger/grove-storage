import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { expect } from "@playwright/test";

export async function clientChecks(page, { database }) {
  const id = "console-ui-client";
  await page
    .getByRole("navigation")
    .getByRole("link", { name: "Clients", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Create client", exact: true })
    .click();
  await page.getByLabel(/^Client ID\s*\*?$/).fill(id);
  await page
    .getByLabel(/^Storage\s*\*?$/)
    .selectOption("console-live");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: id, exact: true }),
  ).toBeVisible();

  await expect(page.getByRole("button", { name: /Generate key|Register key/ })).toHaveCount(0);
  async function saved() {
    await page
      .getByLabel("I have saved these keys. Secrets are shown only once.")
      .check();
    await page.getByRole("button", { name: "Done", exact: true }).click();
  }
  const fileId = randomUUID();

  await page.getByRole("button", { name: "Create credential", exact: true }).click();
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByLabel(/^Secret key\s*\*?$/)).toBeVisible();
  const s3 = await page
    .getByLabel(/^Access key ID\s*\*?$/)
    .inputValue();
  const secret = await page
    .getByLabel(/^Secret key\s*\*?$/)
    .inputValue();
  assert(secret.length > 0);
  await saved();
  assert(!(await page.content()).includes(secret));
  assert(
    !(
      await page.evaluate(() =>
        JSON.stringify({ ...localStorage, ...sessionStorage }),
      )
    ).includes(secret),
  );
  for (const key of [s3]) {
    await page
      .getByRole("button", { name: `Revoke ${key}`, exact: true })
      .click();
    await page.getByLabel("Client ID to confirm").fill(id);
    await page.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: `Revoke ${key}`, exact: true }),
    ).toHaveCount(0);
  }

  const sql = (statement) =>
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
        statement,
      ],
      { timeout: 10000, stdio: "pipe" },
    );
  sql(
    `INSERT INTO files(id,client_id,declared_size) VALUES('${fileId}','${id}',1)`,
  );
  async function remove() {
    await page
      .getByRole("button", { name: "Delete client", exact: true })
      .click();
    await page.getByLabel("Client ID to delete").fill(id);
    await page.getByRole("button", { name: "Confirm delete" }).click();
  }
  await remove();
  await expect(page.getByRole("alert")).toContainText("pending cleanup");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  sql(`DELETE FROM files WHERE id='${fileId}'`);
  await remove();
  await expect(
    page.getByRole("heading", { name: "Clients", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: new RegExp(id) })).toHaveCount(0);
  console.log(
    "PASS real S3-only Clients UI create/delete, credential issuance/revocation and pending-file delete guard",
  );
  await page.goto(
    new URL("/api/admin/console/#storages/console-live", page.url()).href,
  );
}
