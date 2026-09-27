import { randomBytes, randomUUID } from "node:crypto";

// Loopback sample data only. The real API owns authentication and authorization.
export function previewIdentity(json) {
  const owner = randomUUID();
  const accounts = new Map([
    [
      owner,
      {
        id: owner,
        kind: "user",
        display_name: "Home administrator",
        role: "admin",
        is_active: true,
        deleted_at: null,
      },
    ],
  ]);
  const credentials = new Map();
  let current = null;
  let master = false;
  let signedInAt = new Date().toISOString();
  function issue(
    account,
    label,
    days = 90,
    token = "gsm_" + randomBytes(32).toString("hex"),
  ) {
    const id = randomUUID();
    const expires_at = new Date(Date.now() + days * 86400000).toISOString();
    credentials.set(id, {
      id,
      account_id: account,
      label,
      token_prefix: token.slice(0, 12),
      token,
      created_at: new Date().toISOString(),
      expires_at,
      revoked_at: null,
    });
    return { account_id: account, credential_id: id, expires_at, token };
  }
  issue(owner, "Preview sign-in", 90, "qwer1234");
  function session() {
    if (current === "root") return { principal: "root", role: "root", session_id: "preview-root", expires_at: new Date(Date.now()+1800000).toISOString() };
    const key = credentials.get(current);
    const user = accounts.get(key?.account_id);
    if (
      !user?.is_active ||
      user.deleted_at ||
      key.revoked_at ||
      Date.parse(key.expires_at) <= Date.now()
    )
      return null;
    return {
      principal: "user",
      role: user.role,
      user_id: user.id,
      credential_id: key.id,
      session_id: key.id,
    };
  }
  return {
    session,
    async handle(req, res, url) {
      const prefix = "/api/admin/identity/v1";
      if (!url.pathname.startsWith(prefix + "/")) return false;
      const path = url.pathname.slice(prefix.length),
        method = req.method;
      let raw = "";
      for await (const chunk of req) {
        raw += chunk;
        if (raw.length > 65536) {
          json(res, 413, {});
          return true;
        }
      }
      const body = raw ? JSON.parse(raw) : {};
      const fail = (status, error) =>
        json(res, status, { error, request_id: randomUUID() });
      const page = (rows) => {
        const ordered = rows
          .sort((a, b) => b.id.localeCompare(a.id))
          .filter(
            (row) =>
              !url.searchParams.get("before") ||
              row.id < url.searchParams.get("before"),
          );
        const items = ordered.slice(0, 50);
        return {
          items,
          next_before: items.length === 50 ? items.at(-1).id : null,
        };
      };
      if (path === "/session") {
        if (method === "POST") {
          signedInAt = new Date().toISOString();
          if (body.token === "qwer1234") { current = "root"; json(res, 200, session()); return true; }
          const key = [...credentials.values()].find(
            (key) => key.token === body.token,
          );
          if (accounts.get(key?.account_id)?.kind !== "user") {
            fail(401, "unauthenticated");
            return true;
          }
          current = key.id;
        }
        if (method === "DELETE") {
          current = null;
          json(res, 204, null);
          return true;
        }
        const value = session();
        value ? json(res, 200, value) : fail(401, "unauthenticated");
        return true;
      }
      if (path.startsWith("/master/")) {
        if (path === "/master/session" && method === "DELETE") {
          master = false;
          json(res, 204, null);
          return true;
        }
        if (path === "/master/session" && method === "POST")
          master = body.token === "qwer1234";
        if (!master) {
          fail(401, "unauthenticated");
          return true;
        }
        if (path === "/master/session")
          json(res, 200, {
            principal: "master",
            scope: "setup_recovery",
            initialized: true,
          });
        else if (path === "/master/recover") {
          const user = accounts.get(body.user_id);
          if (
            !body.confirm ||
            user?.kind !== "user" ||
            user.role !== "admin" ||
            !user.is_active ||
            user.deleted_at
          )
            fail(409, "conflict");
          else {
            for (const key of credentials.values())
              if (key.account_id === user.id)
                key.revoked_at = new Date().toISOString();
            const key = issue(user.id, "master-issued");
            master = false;
            json(res, 201, { ...key, user_id: user.id });
          }
        } else fail(409, "conflict");
        return true;
      }
      if (!session()) {
        fail(401, "unauthenticated");
        return true;
      }
      if (path === "/sessions" && method === "GET") {
        const value = session();
        json(res, 200, { items: [{ id: value.session_id, credential_id: value.credential_id ?? null, created_at: signedInAt, expires_at: new Date(Date.parse(signedInAt)+1800000).toISOString(), revoked_at: null }], next_before: null });
        return true;
      }
      if (path.startsWith("/sessions/") && method === "DELETE") {
        const id = decodeURIComponent(path.slice("/sessions/".length));
        if (id !== session().session_id) fail(403, "forbidden");
        else { current = null; json(res, 200, { changed: true }); }
        return true;
      }
      if (path.startsWith("/history/") && method === "GET") {
        if (path === "/history/security" && !["admin", "root"].includes(session().role)) fail(403, "forbidden");
        else json(res, 200, { items: [], next_before: null });
        return true;
      }
      if (!["admin", "root"].includes(session().role)) {
        fail(403, "forbidden");
        return true;
      }
      if (path === "/root" && method === "GET") json(res, 200, { id: "root", configured: true, protected: true, source: "config" });
      else if (path === "/accounts" && method === "GET")
        json(res, 200, page([...accounts.values()]));
      else if (path === "/accounts" && method === "POST") {
        if (
          !body.display_name?.trim() ||
          body.kind !== "user" ||
          !["reader", "writer", "admin"].includes(body.role)
        )
          fail(400, "invalid_input");
        else {
          const id = randomUUID();
          accounts.set(id, {
            id,
            ...body,
            is_active: true,
            deleted_at: null,
          });
          json(res, 201, { account_id: id });
        }
      } else {
        const parts = path.split("/");
        const account = accounts.get(parts[2]);
        if (parts[1] === "credentials" && method === "DELETE") {
          const key = credentials.get(parts[2]);
          if (!key) fail(404, "not_found");
          else {
            key.revoked_at = new Date().toISOString();
            json(res, 200, { changed: true });
          }
        } else if (!account) fail(404, "not_found");
        else if (parts[1] === "accounts" && parts.length === 3 && method === "GET")
          json(res, 200, account);
        else if (parts[3] === "credentials") {
          if (method === "GET")
            json(
              res,
              200,
              page(
                [...credentials.values()]
                  .filter((key) => key.account_id === account.id)
                  .map(({ token: _token, ...key }) => key),
              ),
            );
          else if (!account.is_active || account.deleted_at)
            fail(409, "conflict");
          else
            json(res, 201, issue(account.id, body.label, body.expires_in_days));
        } else {
          const retiring =
            method === "DELETE" ||
            (body.operation === "role" && body.role !== "admin") ||
            (body.operation === "active" && !body.is_active);
          if (
            retiring &&
            account.kind === "user" &&
            account.role === "admin" &&
            account.is_active &&
            [...accounts.values()].filter(
              (a) =>
                a.kind === "user" &&
                a.role === "admin" &&
                a.is_active &&
                !a.deleted_at,
            ).length === 1
          )
            fail(409, "conflict");
          else {
            if (method === "DELETE") {
              account.deleted_at = new Date().toISOString();
              account.is_active = false;
              for (const key of credentials.values())
                if (key.account_id === account.id)
                  key.revoked_at = new Date().toISOString();
            } else if (body.operation === "role") account.role = body.role;
            else if (body.operation === "active") {
              account.is_active = body.is_active;
              if (
                !account.is_active &&
                credentials.get(current)?.account_id === account.id
              )
                current = null;
            }
            json(res, 200, { changed: true });
          }
        }
      }
      return true;
    },
  };
}
