import { strict as assert } from "node:assert";
import { test } from "node:test";
import { previewHistory } from "./preview-history.mjs";

test("preview category filtering precedes pagination without widening actor scope", () => {
  const history = previewHistory();
  const user = { user_id: "reader", role: "reader" };
  for (const operation of ["storage.test", "identity.session.logout", "storage.create", "identity.session.current"]) {
    history.record("invocations", user, { operation });
  }
  history.record("invocations", { user_id: "other" }, { operation: "storage.delete" });
  const management = history.page("invocations", user, new URLSearchParams({ category: "management", limit: "1" }));
  assert.equal(management.items[0].operation, "storage.create");
  const next = history.page("invocations", user, new URLSearchParams({ category: "management", limit: "1", before: management.next_before }));
  assert.equal(next.items[0].operation, "storage.test");
  assert.equal(next.next_before, null);
  assert.deepEqual(history.page("invocations", user, new URLSearchParams({ category: "authentication" })).items.map(row => row.operation),
    ["identity.session.current", "identity.session.logout"]);
  assert.equal(history.page("invocations", user, new URLSearchParams()).items.length, 4);
  assert.equal(history.page("invocations", user, new URLSearchParams({ category: "all" })).items.length, 4);
});
