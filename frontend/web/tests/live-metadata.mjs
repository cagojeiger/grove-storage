import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { expect } from "@playwright/test";

export async function metadataChecks(page, { origin, database }) {
  const sql = (query) =>
    execFileSync(
      "docker",
      [
        "exec",
        database,
        "psql",
        "-U",
        "grove",
        "-d",
        "grove",
        "-At",
        "-v",
        "ON_ERROR_STOP=1",
        "-c",
        query,
      ],
      { encoding: "utf8", timeout: 10000 },
    ).trim();
  for (const [table, id] of [
    ["storages", "console-live"],
    ["clients", "console-client"],
  ]) {
    const before = sql(
      `SELECT to_jsonb(r) - 'metadata' - 'updated_at' FROM ${table} r WHERE id='${id}'`,
    );
    await page.goto(`${origin}/api/admin/console/#${table}/${id}`);
    await expect(page.getByRole("region", { name: "Metadata", exact: true })).toContainText("No metadata.");
    for (const metadata of [
      { description: "Console metadata test", environment: "home" },
      {},
    ]) {
      await page
        .getByRole("button", { name: "Edit metadata", exact: true })
        .click();
      await page
        .getByLabel("Metadata JSON")
        .fill(JSON.stringify(metadata, null, 2));
      await page.getByRole("button", { name: "Save", exact: true }).click();
      await expect(page.getByRole("dialog")).toHaveCount(0);
      await page.reload();
      if (Object.keys(metadata).length) {
        for (const [key, value] of Object.entries(metadata)) await expect(page.getByText(`${key}: ${value}`, { exact: true })).toBeVisible();
      } else await expect(page.getByRole("region", { name: "Metadata", exact: true })).toContainText("No metadata.");
      assert.deepEqual(
        JSON.parse(sql(`SELECT metadata FROM ${table} WHERE id='${id}'`)),
        metadata,
      );
    }
    assert.equal(
      sql(
        `SELECT to_jsonb(r) - 'metadata' - 'updated_at' FROM ${table} r WHERE id='${id}'`,
      ),
      before,
    );
    const action = `${table === "storages" ? "storage" : "client"}.metadata.replace`;
    assert.equal(
      sql(
        `SELECT count(*) FROM management.audit_events WHERE action='${action}' AND resource_id='${id}' AND surface='console' AND metadata='{}'`,
      ),
      "2",
    );
  }
  console.log(
    "PASS real HTTPS metadata save/reload/clear, PostgreSQL persistence, unchanged settings and payload-free audit",
  );
}
