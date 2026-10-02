import { expect, test } from "@playwright/test";

test("sample storage with advertised files cannot be deleted without a client reference", async ({
  request,
}) => {
  const origin = "http://127.0.0.1:5180";
  const login = await request.post(`${origin}/api/admin/identity/v1/session`, {
    data: { username: "owner", password: "a private phrase for preview" },
    headers: { "X-Grove-CSRF": "1" },
  });
  expect(login.ok()).toBe(true);
  const command = (name: string, input: object) =>
    request.post(`${origin}/api/admin/console-commands/v1`, {
      data: { protocol: 1, command: name, input },
      headers: { "X-Grove-CSRF": "1" },
    });
  const before = await command("storage.show", { id: "backup-s3" });
  expect(before.status()).toBe(200);
  const removed = await command("storage.delete", { id: "backup-s3" });
  expect(removed.status()).toBe(409);
  expect(await removed.json()).toMatchObject({
    error: { code: "conflict", outcome: "not_applied" },
  });
  expect((await command("storage.show", { id: "backup-s3" })).status()).toBe(
    200,
  );
});
