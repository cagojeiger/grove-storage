import { randomBytes } from "node:crypto";

// Disposable in-memory preview only; production uses the shared command service.
export function previewClients(storages) {
  const clients = new Map([
    ["notegate", { id: "notegate", storage_id: "home-archive" }],
  ]);
  const native = new Map(),
    s3 = new Map();
  return {
    references: (id) => [...clients.values()].some((c) => c.storage_id === id),
    handle(command, input, success, failure) {
      if (command === "client.list") {
        success([...clients.keys()]);
        return true;
      }
      if (command === "usage.clients") {
        success(
          [...clients.values()].map((c) => ({
            client_id: c.id,
            storage_id: c.storage_id,
            active_files: c.id === "notegate" ? 1240 : 0,
            active_bytes: c.id === "notegate" ? 137438953472 : 0,
          })),
        );
        return true;
      }
      if (
        !["client.", "client-key.", "credential."].some((prefix) =>
          command.startsWith(prefix),
        )
      )
        return false;
      const id = input.id ?? input.client_id;
      if (command === "client.create") {
        if (!storages.has(input.storage_id)) failure(404, "not_found");
        else if (clients.has(id)) failure(409, "conflict");
        else {
          const client = { id, storage_id: input.storage_id };
          clients.set(id, client);
          success(client);
        }
        return true;
      }
      if (!clients.has(id)) {
        failure(404, "not_found");
        return true;
      }
      switch (command) {
        case "client.show":
          success(clients.get(id));
          break;
        case "client.delete":
          if (id === "notegate") failure(409, "conflict");
          else {
            clients.delete(id);
            native.delete(id);
            s3.delete(id);
            success({ resource: "client", id });
          }
          break;
        case "client-key.list":
          success([...(native.get(id) ?? [])]);
          break;
        case "credential.list":
          success([...(s3.get(id) ?? [])]);
          break;
        case "client-key.register": {
          const keys = native.get(id) ?? new Set();
          keys.add(input.key_hash);
          native.set(id, keys);
          success({ client_id: id, key_hash: input.key_hash });
          break;
        }
        case "credential.create": {
          const access_key_id = randomBytes(12).toString("hex"),
            keys = s3.get(id) ?? new Set();
          keys.add(access_key_id);
          s3.set(id, keys);
          success({
            access_key_id,
            secret_key: randomBytes(32).toString("hex"),
          });
          break;
        }
        case "client-key.delete":
          native.get(id)?.delete(input.key_hash);
          success({
            resource: "client-key",
            id: input.key_hash,
            client_id: id,
          });
          break;
        case "credential.delete":
          s3.get(id)?.delete(input.access_key_id);
          success({
            resource: "credential",
            id: input.access_key_id,
            client_id: id,
          });
          break;
        default:
          failure(400, "unknown_command");
      }
      return true;
    },
  };
}
