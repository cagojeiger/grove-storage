import { test, expect } from "@playwright/test";
import { clientMock, root, s3Id } from "./client-fixture";

test("client creation and confirmed deletion use the shared resource contract", async ({ page }) => {
  const { calls } = await clientMock(page);
  await page.goto(root);
  await page.getByRole("button", { name: "Create client" }).click();
  await page.getByLabel(/^Client ID\s*\*?$/).fill("new-client");
  await page.getByLabel(/^Storage\s*\*?$/).selectOption("home-archive");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("heading", { name: "new-client", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Delete client", exact: true }).click();
  await expect(page.getByRole("button", { name: "Confirm delete" })).toBeDisabled();
  await page.getByLabel("Client ID to delete").fill("new-client");
  await page.getByRole("button", { name: "Confirm delete" }).click();
  await expect(page.getByRole("heading", { name: "Clients", exact: true })).toBeVisible();
  expect(calls.filter((c) => ["client.create", "client.delete"].includes(c.command))).toEqual([
    { command: "client.create", input: { id: "new-client", storage_id: "home-archive" } },
    { command: "client.delete", input: { id: "new-client" } },
  ]);
});

test("console exposes only S3 credentials and never requests Native keys", async ({ page }) => {
  const { calls } = await clientMock(page);
  await page.goto(root + "/notegate");
  await expect(page.getByRole("heading", { name: "S3 Credentials" })).toBeVisible();
  await expect(page.getByRole("button", { name: /Generate key|Register key/ })).toHaveCount(0);
  await expect(page.getByText("Native keys", { exact: true })).toHaveCount(0);
  expect(calls.some((call) => call.command.startsWith("client-key."))).toBe(false);
});

test("S3 issuance and revocation keep the secret out of lists and storage", async ({ page }) => {
  const { calls } = await clientMock(page);
  await page.goto(root + "/notegate");
  await page.getByRole("button", { name: "Create credential", exact: true }).click();
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByLabel(/^Secret key\s*\*?$/)).toHaveValue("one-time-provider-independent-secret");
  await expect(page.getByRole("button", { name: "Close", exact: true })).toBeDisabled();
  await page.getByLabel("I have saved these keys. Secrets are shown only once.").check();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await page.getByRole("button", { name: `Revoke ${s3Id}`, exact: true }).click();
  await page.getByLabel("Client ID to confirm").fill("notegate");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(calls).toContainEqual({ command: "credential.delete", input: { client_id: "notegate", access_key_id: s3Id } });
  expect(await page.content()).not.toContain("one-time-provider-independent-secret");
  expect(await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }))).not.toContain("one-time-provider-independent-secret");
});
