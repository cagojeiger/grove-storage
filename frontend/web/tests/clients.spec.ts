import { test, expect } from "@playwright/test";
import { createHash } from "node:crypto";
import { clientMock, root, s3Id } from "./client-fixture";

test("client creation and confirmed deletion use the shared resource contract", async ({
  page,
}) => {
  const { calls } = await clientMock(page);
  await page.goto(root);
  await page.getByRole("button", { name: "Create client" }).click();
  await page.getByLabel("Client ID", { exact: true }).fill("new-client");
  await page
    .getByLabel("Storage", { exact: true })
    .selectOption("home-archive");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "new-client", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Delete client", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Confirm delete" }),
  ).toBeDisabled();
  await page.getByLabel("Client ID to delete").fill("new-client");
  await page.getByRole("button", { name: "Confirm delete" }).click();
  await expect(
    page.getByRole("heading", { name: "Clients", exact: true }),
  ).toBeVisible();
  expect(
    calls.filter((c) => ["client.create", "client.delete"].includes(c.command)),
  ).toEqual([
    {
      command: "client.create",
      input: { id: "new-client", storage_id: "home-archive" },
    },
    { command: "client.delete", input: { id: "new-client" } },
  ]);
});

test("Native registration transmits only SHA-256 and generation shows the raw key once", async ({
  page,
}) => {
  const { calls } = await clientMock(page);
  await page.goto(root + "/notegate");
  await page.getByRole("button", { name: "Register key", exact: true }).click();
  await page.getByLabel("Existing Native key").fill("existing-raw-key");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(
    calls.find((c) => c.command === "client-key.register")?.input.key_hash,
  ).toBe(
    "sha256:" + createHash("sha256").update("existing-raw-key").digest("hex"),
  );
  expect(JSON.stringify(calls)).not.toContain("existing-raw-key");
  await page.getByRole("button", { name: "Generate key", exact: true }).click();
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  const key = page.getByLabel("Native API key", { exact: true });
  await expect(key).toBeVisible();
  const raw = await key.inputValue();
  expect(raw).toMatch(/^gsk_[a-f0-9]{64}$/);
  expect(
    calls.filter((c) => c.command === "client-key.register").at(-1)?.input
      .key_hash,
  ).toBe("sha256:" + createHash("sha256").update(raw).digest("hex"));
  await expect(
    page.getByRole("button", { name: "Close", exact: true }),
  ).toBeDisabled();
  await page
    .getByLabel("I have saved these keys. Secrets are shown only once.")
    .check();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await expect(key).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).not.toContain(raw);
});

test("S3 issuance and revocation keep the secret out of lists and storage", async ({
  page,
}) => {
  const { calls } = await clientMock(page);
  await page.goto(root + "/notegate");
  await page.getByRole("button", { name: "Issue key", exact: true }).click();
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveValue(
    "one-time-provider-independent-secret",
  );
  await page
    .getByLabel("I have saved these keys. Secrets are shown only once.")
    .check();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await page
    .getByRole("button", { name: `Revoke ${s3Id}`, exact: true })
    .click();
  await page.getByLabel("Client ID to confirm").fill("notegate");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(calls).toContainEqual({
    command: "credential.delete",
    input: { client_id: "notegate", access_key_id: s3Id },
  });
  expect(await page.content()).not.toContain(
    "one-time-provider-independent-secret",
  );
});
