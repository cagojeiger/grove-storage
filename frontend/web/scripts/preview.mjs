import { createServer } from "node:http";
import { randomBytes, randomUUID } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { previewIdentity } from "./preview-identity.mjs";
import { previewClients } from "./preview-clients.mjs";
import { previewMetadata } from "./preview-metadata.mjs";
import { previewHistory } from "./preview-history.mjs";
import { consoleHeaders } from "../security-headers.mjs";

const dist = resolve(import.meta.dirname, "../dist");
const base = "/api/admin/console/";
const storages = new Map([
  [
    "home-archive",
    {
      id: "home-archive",
      kind: "s3",
      force_relay: false,
      root_path: null,
      endpoint: "https://s3.home.example.com",
      public_endpoint: "https://files.example.com",
      region: "us-east-1",
      bucket: "archive",
      force_path_style: true,
      access_key: "sample-access-key",
      capacity_bytes: 1099511627776,
    },
  ],
  [
    "backup-s3",
    {
      id: "backup-s3",
      kind: "s3",
      force_relay: false,
      root_path: null,
      endpoint: "https://backup.example.com",
      public_endpoint: null,
      region: "us-east-1",
      bucket: "backup",
      force_path_style: false,
      access_key: "sample-backup-key",
      capacity_bytes: 2199023255552,
    },
  ],
]);
const history = previewHistory();
const identities = previewIdentity(json, history);
const clients = previewClients(storages);

function json(res, status, body) {
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Cache-Control": "no-store",
  });
  res.end(JSON.stringify(body));
}

function usage(storage) {
  const active =
    storage.id === "home-archive"
      ? 137438953472
      : storage.id === "backup-s3"
        ? 21474836480
        : 0;
  return {
    storage_id: storage.id,
    kind: storage.kind,
    capacity_bytes: storage.capacity_bytes,
    active_bytes: active,
    reserved_bytes: 0,
    purge_pending_bytes: 0,
    remaining_bytes: storage.capacity_bytes - active,
    active_files:
      storage.id === "home-archive"
        ? 1240
        : storage.id === "backup-s3"
          ? 32
          : 0,
    reserved_files: 0,
    purge_pending_files: 0,
  };
}

async function response(req, res) {
  const url = new URL(req.url, "http://127.0.0.1");
  const path = url.pathname;
  if (await identities.handle(req, res, url)) return;
  const method = req.method;
  if (path === "/readyz") return json(res, 200, { status: "ready" });
  if (path === "/api/admin/console-commands/v1" && method === "POST") {
    if (!identities.session()) return json(res, 401, {});
    let raw = "";
    for await (const chunk of req) {
      raw += chunk;
      if (raw.length > 65536) return json(res, 413, {});
    }
    const { command, input } = JSON.parse(raw);
    const envelope = { protocol: 1, request_id: randomUUID() };
    const actor = identities.session();
    const started = performance.now();
    const invocation = (outcome, error_code = null) => history.record("invocations", actor, {
      operation: command, outcome, error_code, duration_ms: Math.round(performance.now() - started),
    }, envelope.request_id);
    const success = (result, audit = null) => {
      invocation("succeeded");
      if (audit) history.record("audit", actor, { ...audit, action: command }, envelope.request_id);
      json(res, 200, { ...envelope, command, result });
    };
    const failure = (status, code) => {
      invocation("failed", code);
      json(res, status, { ...envelope, error: { code, outcome: "not_applied" } });
    };
    if (identities.session().role === "reader" && !["client.list", "client.show", "storage.list", "storage.show", "storage.test", "usage.clients", "usage.storages", "usage.history", "storage.metadata.show", "client.metadata.show"].includes(command)) return failure(403, "forbidden");
    if (command === "usage.history") return success([]);
    if (clients.handle(command, input, success, failure)) return;
    if (command === "usage.storages") return success([...storages.values()].map(usage));
    if (command === "storage.list") return success([...storages.values()]);
    const storageId = input.id;
    if (previewMetadata(command, input, storages.get(storageId), success, failure)) return;
    // Sample storage has no provider credentials. Never fabricate connectivity.
    if (command === "storage.test")
      return storages.has(storageId) ? failure(503, "unavailable") : failure(404, "not_found");
    if (command === "storage.show")
      return storages.has(storageId) ? success(storages.get(storageId)) : failure(404, "not_found");
    if (command === "storage.delete") {
      if (!storages.has(storageId)) return failure(404, "not_found");
      const counters = usage(storages.get(storageId));
      if (clients.references(storageId) || ["active_files", "reserved_files", "purge_pending_files", "active_bytes", "reserved_bytes", "purge_pending_bytes"].some((key) => counters[key] > 0)) return failure(409, "conflict");
      storages.delete(storageId);
      return success({ resource: "storage", id: storageId }, { resource_type: "storage", resource_id: storageId, metadata: {} });
    }
    if (!["storage.create", "storage.replace"].includes(command)) return failure(400, "unknown_command");
    if (command === "storage.create" && storages.has(storageId)) return failure(409, "conflict");
    if (command === "storage.replace" && !storages.has(storageId)) return failure(404, "not_found");
    const body = input.spec;
    if (body.kind !== "s3") return failure(400, "invalid_input");
    const saved = {
      id: storageId,
      kind: body.kind,
      force_relay: body.force_relay ?? false,
      root_path: body.root_path ?? null,
      endpoint: body.endpoint ?? null,
      public_endpoint: body.public_endpoint ?? body.endpoint ?? null,
      region: body.region ?? null,
      bucket: body.bucket ?? null,
      force_path_style: body.force_path_style ?? false,
      access_key: body.access_key ?? null,
      capacity_bytes: body.capacity_bytes,
    };
    storages.set(storageId, saved);
    return success(saved, { resource_type: "storage", resource_id: storageId, metadata: {} });
  }
  if (!path.startsWith(base)) return json(res, 404, {});
  const relative = path.slice(base.length) || "index.html";
  const file = resolve(dist, relative);
  if (!file.startsWith(dist + sep)) return json(res, 404, {});
  try {
    let body = await readFile(file);
    if (relative === "index.html") {
      const nonce = randomBytes(24).toString("base64");
      body = Buffer.from(body.toString().replace("__GROVE_CSP_NONCE__", nonce));
      for (const [name, value] of Object.entries(consoleHeaders(false, nonce))) res.setHeader(name, value);
    }
    const type = file.endsWith(".js")
      ? "text/javascript"
      : file.endsWith(".css")
        ? "text/css"
        : file.endsWith(".png")
          ? "image/png"
          : file.endsWith(".woff2") ? "font/woff2"
          : file.endsWith(".woff") ? "font/woff"
          : "text/html";
    res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" });
    res.end(body);
  } catch {
    json(res, 404, {});
  }
}

const server = createServer((req, res) => {
  for (const [name, value] of Object.entries(consoleHeaders())) res.setHeader(name, value);
  void response(req, res).catch(() => json(res, 500, {}));
});
server.listen(Number(process.env.GROVE_PREVIEW_PORT ?? 0), "127.0.0.1", () => {
  console.log(
    `Preview (sample data only): http://127.0.0.1:${server.address().port}${base}`,
  );
});
