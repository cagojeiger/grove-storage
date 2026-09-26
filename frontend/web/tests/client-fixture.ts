import { Page } from "@playwright/test";
import { commandUrl, envelope, failure, session } from "./command-fixture";
import { example } from "./storage-fixture";

export const root = "/api/admin/console/#clients";
export const nativeHash = "sha256:" + "a".repeat(64);
export const s3Id = "existing12345678";
export async function clientMock(page: Page, role = "admin") {
  const clients = new Map([
    ["notegate", { id: "notegate", storage_id: example.id }],
  ]);
  const native = new Set([nativeHash]),
    s3 = new Set([s3Id]);
  const calls: { command: string; input: Record<string, string> }[] = [];
  await page.route("**/identity/v1/session", (route) =>
    route.fulfill({ json: { ...session, role } }),
  );
  await page.route(commandUrl, async (route) => {
    const { command, input } = route.request().postDataJSON() as {
      command: string;
      input: Record<string, string>;
    };
    calls.push({ command, input });
    let result: unknown;
    switch (command) {
      case "client.metadata.show":
        result = { id: input.id, metadata: {} };
        break;
      case "client.list":
        result = [...clients.keys()];
        break;
      case "client.show":
        result = clients.get(input.id);
        break;
      case "client.create": {
        result = { id: input.id, storage_id: input.storage_id };
        clients.set(input.id, result as { id: string; storage_id: string });
        break;
      }
      case "client.delete":
        clients.delete(input.id);
        result = { resource: "client", id: input.id };
        break;
      case "storage.list":
        result = [example];
        break;
      case "usage.clients":
        result = [...clients.values()].map((c) => ({
          client_id: c.id,
          storage_id: c.storage_id,
          active_files: 0,
          active_bytes: 0,
        }));
        break;
      case "client-key.list":
        result = [...native];
        break;
      case "credential.list":
        result = [...s3];
        break;
      case "client-key.register":
        native.add(input.key_hash);
        result = input;
        break;
      case "credential.create":
        s3.add("issued12345678");
        result = {
          access_key_id: "issued12345678",
          secret_key: "one-time-provider-independent-secret",
        };
        break;
      case "client-key.delete":
        native.delete(input.key_hash);
        result = {
          resource: "client-key",
          id: input.key_hash,
          client_id: input.client_id,
        };
        break;
      case "credential.delete":
        s3.delete(input.access_key_id);
        result = {
          resource: "credential",
          id: input.access_key_id,
          client_id: input.client_id,
        };
        break;
      default:
        return route.fulfill({ status: 400, json: failure(400) });
    }
    await route.fulfill({ json: envelope(command, result) });
  });
  return { calls, clients, native, s3 };
}
