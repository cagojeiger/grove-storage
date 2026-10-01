import { expect, test } from "@playwright/test";
import { command, commandPath } from "../src/api/commands";
import { ApiError } from "../src/api/http";
import { uncertain } from "../src/features/storages/model";
import { envelope, failure } from "./command-fixture";

test("command transport uses cookie and server-owned surface, with no mutation retry", async () => {
  const original = globalThis.fetch;
  let calls = 0;
  try {
    globalThis.fetch = (path, options) => {
      calls++;
      expect(path).toBe(commandPath);
      expect(options?.credentials).toBe("same-origin");
      expect(options?.redirect).toBe("error");
      expect(new Headers(options?.headers).get("X-Grove-CSRF")).toBe("1");
      expect(JSON.parse(options?.body as string)).toEqual({ protocol: 1, command: "storage.delete", input: { id: "test" } });
      return Promise.resolve(Response.json(envelope("storage.delete", { resource: "storage", id: "test" })));
    };
    await command("storage.delete", { id: "test" });
    expect(calls).toBe(1);
  } finally { globalThis.fetch = original; }
});

test("explicit outcomes and untrusted replies keep mutation uncertainty", async () => {
  const original = globalThis.fetch;
  try {
    for (const [status, body, unknown] of [
      [503, { ...failure(503), error: { code: "unavailable", outcome: "not_applied" } }, false],
      [503, failure(503), true],
      [500, { ...failure(500), error: { code: "internal", outcome: "applied" } }, true],
      [409, {}, true],
      [200, envelope("wrong.command", {}), true],
      [200, { ...envelope("storage.delete", {}), protocol: 2 }, true],
      [200, envelope("storage.delete", { resource: "storage", id: "another-target" }), true],
      [200, envelope("storage.delete", null), true],
    ] as const) {
      globalThis.fetch = () => Promise.resolve(Response.json(body, { status }));
      let caught: unknown;
      try { await command("storage.delete", { id: "test" }); }
      catch (error) { caught = error; }
      expect(caught).toBeInstanceOf(ApiError);
      expect(uncertain(caught)).toBe(unknown);
    }
  } finally { globalThis.fetch = original; }
});
