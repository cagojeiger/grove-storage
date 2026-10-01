import assert from "node:assert/strict";
import { expect } from "@playwright/test";

export async function transferChecks(page, { endpoint, origin, minio }, token) {
  async function api(path, body) {
    const response = await fetch(`${endpoint}/api/v1/files${path}`, {
      method: "POST",
      headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(15000),
    });
    assert(response.ok, `Native API returned ${response.status}`);
    return response.json();
  }
  const content = Buffer.alloc(64 * 1024, 0x47);
  const created = await api("", { declared_size: content.length });
  assert.equal(new URL(created.put_url).origin, minio.endpoint);
  const uploaded = await fetch(created.put_url, {
    method: "PUT", body: content, signal: AbortSignal.timeout(15000),
  });
  assert.equal(uploaded.status, 200);
  await api(`/${created.file_id}/commit`, {});
  const read = await api(`/${created.file_id}/read`, {});
  assert.equal(new URL(read.get_url).origin, minio.endpoint);
  const downloaded = await fetch(read.get_url, { signal: AbortSignal.timeout(15000) });
  assert.equal(downloaded.status, 200);
  assert.deepEqual(Buffer.from(await downloaded.arrayBuffer()), content);

  await page.goto(`${origin}/api/admin/console/#clients/console-client`);
  const fields = page.getByRole("region", { name: "Client details" });
  await expect(fields.getByText("Stored files", { exact: true }).locator("..").getByText("1", { exact: true })).toBeVisible();
  await expect(fields.getByText("Stored data", { exact: true }).locator("..").getByText("64 KiB", { exact: true })).toBeVisible();
  await page.goto(`${origin}/api/admin/console/#`);
  await expect(page.getByRole("table", { name: "Storage usage" }).getByRole("row").filter({ hasText: "console-live" })).toContainText("64 KiB");
  console.log("PASS actual presigned MinIO PUT/GET byte equality, commit and Console usage (1 file, 64 KiB)");
}
