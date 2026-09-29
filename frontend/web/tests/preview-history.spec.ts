import { expect, test } from "@playwright/test";
import type { Audit, Invocation, Security } from "../src/features/activity/model";

type Page<T> = { items: T[]; next_before: string | null };

const root = "http://127.0.0.1:5180";
const identity = `${root}/api/admin/identity/v1`;

test("preview records actions without logging secrets and shows them in Activity", async ({ page, request }) => {
  const password = "a private phrase for preview";
  const login = await request.post(`${identity}/session`, { data: { username: "owner", password } });
  expect(login.ok()).toBeTruthy();
  const session = await login.json() as { user_id: string };
  const id = `history-${Date.now()}`;
  const command = (name: string, input: object) => request.post(`${root}/api/admin/console-commands/v1`, {
    data: { command: name, input },
  });
  expect((await command("client.create", { id, storage_id: "home-archive" })).ok()).toBeTruthy();
  const credential = await command("credential.create", { client_id: id });
  const secret = (await credential.json() as { result: { secret_key: string } }).result.secret_key;
  expect((await command("client.show", { id })).ok()).toBeTruthy();
  expect((await command("storage.test", { id: "home-archive" })).status()).toBe(503);
  const audit = await (await request.get(`${identity}/history/audit?account_id=${session.user_id}`)).json() as Page<Audit>;
  expect(audit.items.some((row: { action: string; resource_id: string }) => row.action === "client.create" && row.resource_id === id)).toBeTruthy();
  expect(audit.items.some((row: { action: string }) => row.action === "client.show" || row.action === "storage.test")).toBeFalsy();
  const first = await (await request.get(`${identity}/history/invocations?limit=1`)).json() as Page<Invocation>;
  expect(first.items[0]).toMatchObject({ operation: "storage.test", outcome: "failed", error_code: "unavailable" });
  expect(first.next_before).not.toBeNull();
  const next = await (await request.get(`${identity}/history/invocations?limit=1&before=${first.next_before}`)).json() as Page<Invocation>;
  expect(next.items[0].context.id).not.toBe(first.items[0].context.id);
  const filtered = await (await request.get(`${identity}/history/audit?credential_id=00000000-0000-0000-0000-000000000000`)).json() as Page<Audit>;
  expect(filtered.items).toEqual([]);
  const security = await (await request.get(`${identity}/history/security`)).json() as Page<Security>;
  expect(security.items.some((row: { event_type: string }) => row.event_type === "login.succeeded")).toBeTruthy();
  for (const stream of [audit, first, next, security]) {
    expect(JSON.stringify(stream)).not.toContain(password);
    expect(JSON.stringify(stream)).not.toContain(secret);
  }
  await page.goto(`${root}/api/admin/console/#activity`);
  await expect(page.getByRole("button").filter({ hasText: `client: ${id}` }).first()).toBeVisible();
  await expect(page.getByRole("complementary", { name: "Workspace sidebar" }).getByRole("button", { name: "Account menu" })).toBeVisible();
  expect((await command("client.delete", { id })).ok()).toBeTruthy();
});

test("preview audits identify each S3 key and omit repeated deletions", async ({ request }) => {
  await request.post(`${identity}/session`, { data: { username: "owner", password: "a private phrase for preview" } });
  const id = `keys-${Date.now()}`;
  const command = (command: string, input: object) => request.post(`${root}/api/admin/console-commands/v1`, { data: { command, input } });
  expect((await command("client.create", { id, storage_id: "home-archive" })).ok()).toBeTruthy();
  try {
    const keys: string[] = [];
    for (let i = 0; i < 2; i++) {
      const result = await (await command("credential.create", { client_id: id })).json() as { result: { access_key_id: string } };
      keys.push(result.result.access_key_id);
    }
    const requests: string[] = [];
    for (let i = 0; i < 2; i++) {
      const response = await command("credential.delete", { client_id: id, access_key_id: keys[0] });
      expect(response.ok()).toBeTruthy();
      requests.push((await response.json() as { request_id: string }).request_id);
    }
    const audit = await (await request.get(`${identity}/history/audit`)).json() as Page<Audit>;
    for (const key of keys) {
      expect(audit.items).toEqual(expect.arrayContaining([
        expect.objectContaining({ action: "credential.create", resource_type: "s3_credential", resource_id: key, metadata: { client_id: id } }),
      ]));
    }
    expect(audit.items.filter(row => requests.includes(row.context.request_id))).toEqual([
      expect.objectContaining({ action: "credential.delete", resource_type: "s3_credential", resource_id: keys[0], metadata: { client_id: id } }),
    ]);
    const invocations = await (await request.get(`${identity}/history/invocations`)).json() as Page<Invocation>;
    expect(invocations.items.filter(row => requests.includes(row.context.request_id))).toHaveLength(2);
  } finally {
    await command("client.delete", { id });
  }
});
