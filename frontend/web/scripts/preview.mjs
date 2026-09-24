import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
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
    "local-files",
    {
      id: "local-files",
      kind: "fs",
      force_relay: false,
      root_path: "/mnt/grove/objects",
      endpoint: null,
      public_endpoint: null,
      region: null,
      bucket: null,
      force_path_style: false,
      access_key: null,
      capacity_bytes: 2199023255552,
    },
  ],
]);
let signedIn = true;

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
      : storage.id === "local-files"
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
        : storage.id === "local-files"
          ? 32
          : 0,
    reserved_files: 0,
    purge_pending_files: 0,
  };
}

async function response(req, res) {
  const path = new URL(req.url, "http://127.0.0.1").pathname;
  const method = req.method;
  if (path === "/readyz") return json(res, 200, { status: "ready" });
  if (path === "/api/admin/v1/session") {
    if (method === "POST") signedIn = true;
    if (method === "DELETE") {
      signedIn = false;
      res.writeHead(204, { "Cache-Control": "no-store" });
      return res.end();
    }
    return json(
      res,
      signedIn ? 200 : 401,
      signedIn ? { principal: "preview", credential_id: "preview" } : {},
    );
  }
  if (path.startsWith("/api/admin/v1/")) {
    if (!signedIn) return json(res, 401, {});
    if (path === "/api/admin/v1/clients" && method === "GET")
      return json(res, 200, [{ id: "notegate" }]);
    if (path === "/api/admin/v1/usage" && method === "GET")
      return json(res, 200, [...storages.values()].map(usage));
    const match = /^\/api\/admin\/v1\/storages(?:\/([^/]+))?$/.exec(path);
    if (!match) return json(res, 404, {});
    const id = match[1] ? decodeURIComponent(match[1]) : null;
    if (method === "GET")
      return json(
        res,
        id && !storages.has(id) ? 404 : 200,
        id ? (storages.get(id) ?? {}) : [...storages.values()],
      );
    if (method === "DELETE") {
      if (!storages.has(id)) return json(res, 404, {});
      if (id === "home-archive") return json(res, 409, {});
      storages.delete(id);
      res.writeHead(204, { "Cache-Control": "no-store" });
      return res.end();
    }
    if (method !== "POST" && method !== "PUT") return json(res, 405, {});
    let raw = "";
    for await (const chunk of req) {
      raw += chunk;
      if (raw.length > 65536) return json(res, 413, {});
    }
    const body = JSON.parse(raw);
    const storageId = id ?? body.id;
    if (method === "POST" && storages.has(storageId)) return json(res, 409, {});
    if (method === "PUT" && !storages.has(storageId)) return json(res, 404, {});
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
    return json(res, method === "POST" ? 201 : 200, saved);
  }
  if (!path.startsWith(base)) return json(res, 404, {});
  const relative = path.slice(base.length) || "index.html";
  const file = resolve(dist, relative);
  if (!file.startsWith(dist + sep)) return json(res, 404, {});
  try {
    const body = await readFile(file);
    const type = file.endsWith(".js")
      ? "text/javascript"
      : file.endsWith(".css")
        ? "text/css"
        : file.endsWith(".png")
          ? "image/png"
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
