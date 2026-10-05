import assert from "node:assert/strict";
import { test } from "node:test";
import { previewIdentity } from "./preview-identity.mjs";

test("Reader creation normalizes usernames and checks canonical duplicates", async () => {
  const api = previewIdentity(
    (res, status, body) => Object.assign(res, { status, body }),
    { record() {} },
  );
  async function post(path, body) {
    const res = {};
    await api.handle({
      method: "POST",
      async *[Symbol.asyncIterator]() { yield JSON.stringify(body); },
    }, res, new URL(`http://localhost/api/admin/identity/v1${path}`));
    return res;
  }
  const password = "a private phrase for preview";
  assert.equal((await post("/session", { username: "owner", password })).status, 200);
  const input = {
    kind: "user_with_password_setup", display_name: "Reader test",
    role: "reader", username: " Reader.Check ", current_password: password,
  };
  assert.equal((await post("/accounts", { ...input, current_password: "wrong" })).status, 401);
  const created = await post("/accounts", input);
  assert.equal(created.status, 201);
  assert.equal(created.body.username, "reader.check");
  assert.match(created.body.token, /^gsps_[0-9a-f]{64}$/);
  assert.ok(Number.isFinite(Date.parse(created.body.expires_at)));
  assert.equal((await post("/accounts", { ...input, username: "READER.CHECK" })).status, 409);
  assert.equal((await post("/accounts", { ...input, username: "OWNER" })).status, 409);
  for (const username of ["_reader", "re@der", "ab"]) {
    assert.equal((await post("/accounts", { ...input, username })).status, 400);
  }
  const setup = { token: created.body.token, password: "a private phrase for reader" };
  assert.equal((await post("/password-setup", setup)).status, 204);
  assert.equal((await post("/password-setup", setup)).status, 404);
  const signedIn = await post("/session", {
    username: "reader.check", password: setup.password,
  });
  assert.equal(signedIn.status, 200);
  assert.equal(signedIn.body.role, "reader");
  assert.equal((await post("/accounts", input)).status, 403);
});
