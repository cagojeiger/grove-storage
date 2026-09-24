import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { expect } from "@playwright/test";

export async function storageChecks(
  page,
  { objects, otherObjects, endpoint, minio },
) {
  async function command(command, input) {
    return page.evaluate(
      async ({ command, input }) => {
        const response = await fetch("/api/admin/console-commands/v1", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Grove-CSRF": "1",
          },
          body: JSON.stringify({ protocol: 1, command, input }),
        });
        return {
          status: response.status,
          body: (await response.json()).result,
        };
      },
      { command, input },
    );
  }
  async function createFs(id) {
    await page.getByRole("button", { name: "Register", exact: true }).click();
    await page.getByLabel("Storage ID", { exact: true }).fill(id);
    await page.getByLabel("Type", { exact: true }).selectOption("fs");
    await page.getByLabel("Root path").fill(objects);
    await page.getByLabel("Registered capacity", { exact: true }).fill("1");
    await page.getByLabel("Capacity unit").selectOption("GiB");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: id, exact: true }),
    ).toBeVisible();
  }
  async function deleteStorage(id) {
    await page.getByRole("button", { name: "Delete storage" }).click();
    await page.getByLabel("Storage ID to delete").fill(id);
    await page.getByRole("button", { name: "Confirm delete" }).click();
  }
  async function list() {
    await page
      .getByRole("navigation")
      .getByRole("link", { name: "Storage", exact: true })
      .click();
  }
  await list();
  await createFs("console-live");
  await page.getByRole("button", { name: "Edit storage" }).click();
  await page.getByLabel("Registered capacity", { exact: true }).fill("2147483648");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  assert.equal(
    (await command("storage.show", { id: "console-live" })).body.capacity_bytes,
    2147483648,
  );

  // A second actor adds a client after the page observed an empty storage.
  assert.equal(
    (
      await command("client.create", {
        id: "console-client",
        storage_id: "console-live",
      })
    ).status,
    200,
  );
  await deleteStorage("console-live");
  await expect(page.getByRole("alert")).toContainText("clients or file");
  await page.getByRole("button", { name: "Cancel" }).click();
  const raw = "console-isolated-native-key";
  const key_hash = `sha256:${createHash("sha256").update(raw).digest("hex")}`;
  assert.equal(
    (await command("client-key.register", { client_id: "console-client", key_hash })).status,
    200,
  );
  const allocated = await fetch(`${endpoint}/api/v1/files`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${raw}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ declared_size: 0 }),
  });
  assert.equal(allocated.status, 201);
  await page.getByRole("button", { name: "Edit storage" }).click();
  await page.getByLabel("Root path").fill(otherObjects);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "address cannot be changed",
  );
  assert.equal(
    (await command("storage.show", { id: "console-live" })).body.root_path,
    objects,
  );
  await page.getByRole("button", { name: "Cancel" }).click();
  await list();
  await createFs("console-removable");
  await deleteStorage("console-removable");
  await expect(
    page.getByRole("heading", { name: "Storage", exact: true }),
  ).toBeVisible();
  assert.equal((await command("storage.show", { id: "console-removable" })).status, 404);
  console.log(
    "PASS real filesystem UI create/replace/delete, concurrent client delete 409, pending-file address replace 409",
  );

  if (minio) {
    await page.getByRole("button", { name: "Register", exact: true }).click();
    await page.getByLabel("Storage ID", { exact: true }).fill("console-minio");
    await page.getByLabel("Endpoint", { exact: true }).fill(minio.endpoint);
    await page.getByLabel("Public endpoint (optional)").fill(minio.endpoint);
    await page.getByLabel("Region", { exact: true }).fill(minio.region);
    await page.getByLabel("Bucket", { exact: true }).fill(minio.bucket);
    await page.getByLabel("Access key", { exact: true }).fill(minio.access_key);
    await page.getByLabel("Secret key", { exact: true }).fill(minio.secret_key);
    await page.getByLabel("Path-style", { exact: true }).check();
    await page.getByLabel("Use relay", { exact: true }).check();
    await page
      .getByLabel("Registered capacity", { exact: true })
      .fill(String(minio.capacity_bytes));
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "console-minio", exact: true }),
    ).toBeVisible();
    const saved = (await command("storage.show", { id: "console-minio" })).body;
    assert.equal(saved.force_relay, true);
    assert.equal(saved.force_path_style, true);
    assert.equal(saved.public_endpoint, minio.endpoint);
    assert(!JSON.stringify(saved).includes(minio.secret_key));
    await page.getByRole("button", { name: "Edit storage" }).click();
    await expect(page.getByLabel("Secret key (re-enter)")).toHaveValue("");
    await page.getByLabel("Secret key (re-enter)").fill(minio.secret_key);
    await page.getByLabel("Registered capacity", { exact: true }).fill("2147483648");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    assert.equal(
      (await command("storage.show", { id: "console-minio" })).body.capacity_bytes,
      2147483648,
    );
    assert(
      !(
        await page.evaluate(() =>
          JSON.stringify({ ...localStorage, ...sessionStorage }),
        )
      ).includes(minio.secret_key),
    );
    await deleteStorage("console-minio");
    await expect(
      page.getByRole("heading", { name: "Storage", exact: true }),
    ).toBeVisible();
    assert.equal((await command("storage.show", { id: "console-minio" })).status, 404);
    console.log(
      "PASS real MinIO UI registration, all S3 options, secret re-entry, replacement and deletion",
    );
  }
  await page
    .getByRole("navigation")
    .getByRole("link", { name: "Overview", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "console-live", exact: true }),
  ).toBeVisible();
}
