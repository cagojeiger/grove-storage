import { randomBytes, randomUUID } from "node:crypto";

// Loopback sample data only. The real API owns authentication and authorization.
export function previewIdentity(json, history) {
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
        username: "owner",
        password_ready: true,
      },
    ],
  ]);
  const credentials = new Map();
  const passwords = new Map([[owner, { username: "owner", password: "a private phrase for preview" }]]);
  const setups = new Map();
  let current = null;
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
  issue(owner, "Preview CLI token");
  function session() {
    if (current && Date.now() >= Date.parse(signedInAt) + 8 * 60 * 60 * 1000) current = null;
    const user = accounts.get(current?.account_id);
    if (
      !user?.is_active ||
      user.deleted_at
    )
      return null;
    return {
      principal: "user",
      role: user.role,
      user_id: user.id,
      credential_id: null,
      session_id: current.session_id,
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
          const match = [...passwords].find(([id, login]) =>
            login.username === body.username && login.password === body.password &&
            accounts.get(id)?.is_active && !accounts.get(id)?.deleted_at);
          if (!match) {
            history.record("security", null, { event_type: "login.failed", reason_code: "unauthenticated" });
            fail(401, "unauthenticated");
            return true;
          }
          current = { account_id: match[0], session_id: randomUUID() };
          signedInAt = new Date().toISOString();
          history.record("security", session(), { event_type: "login.succeeded", reason_code: "password" });
        }
        if (method === "DELETE") {
          if (session()) history.record("security", session(), { event_type: "logout", reason_code: "user_requested" });
          current = null;
          json(res, 204, null);
          return true;
        }
        const value = session();
        value ? json(res, 200, value) : fail(401, "unauthenticated");
        return true;
      }
      if (path === "/password-setup/inspect" || path === "/password-setup") {
        const entry = [...setups].find(([id, setup]) => setup.token === body.token &&
          Date.parse(setup.expires_at) > Date.now() && accounts.get(id)?.is_active &&
          !accounts.get(id)?.deleted_at);
        if (!entry) fail(404, "not_found");
        else if (path === "/password-setup/inspect")
          json(res, 200, { username: entry[1].username, expires_at: entry[1].expires_at });
        else if (typeof body.password !== "string" || [...body.password].length < 15 || [...body.password].length > 128)
          fail(400, "invalid_input");
        else {
          passwords.set(entry[0], { username: entry[1].username, password: body.password });
          accounts.get(entry[0]).password_ready = true;
          setups.delete(entry[0]);
          json(res, 204, null);
        }
        return true;
      }
      if (!session()) {
        fail(401, "unauthenticated");
        return true;
      }
      if (path === "/me" && method === "GET") {
        json(res, 200, accounts.get(current.account_id));
        return true;
      }
      if (path === "/me" && method === "PATCH") {
        if (!body.display_name?.trim() || body.display_name.trim().length > 80) fail(400, "invalid_input");
        else {
          const account = accounts.get(current.account_id);
          const changed = account.display_name !== body.display_name.trim();
          account.display_name = body.display_name.trim();
          if (changed) history.record("audit", session(), { action: "account.name", resource_type: "account", resource_id: account.id, metadata: {} });
          json(res, 200, { changed });
        }
        return true;
      }
      if (path === "/me/password" && method === "POST") {
        const login = passwords.get(current.account_id);
        if (body.current_password !== login?.password ||
            typeof body.new_password !== "string" ||
            [...body.new_password].length < 15 ||
            [...body.new_password].length > 128) {
          fail(body.current_password === login?.password ? 400 : 401, "invalid_input");
        } else {
          passwords.set(current.account_id, { ...login, password: body.new_password });
          current = null;
          json(res, 204, null);
        }
        return true;
      }
      if (path === "/me/tokens" && method === "GET") {
        const before = url.searchParams.get("before");
        const limit = Number(url.searchParams.get("limit") ?? 50);
        if (!Number.isInteger(limit) || limit < 1 || limit > 100) fail(400, "invalid_input");
        else {
          const rows = [...credentials.values()]
            .filter(token => token.account_id === current.account_id && (!before || token.id < before))
            .sort((a, b) => b.id.localeCompare(a.id));
          const items = rows.slice(0, limit).map(({ token, ...item }) => item);
          json(res, 200, { items, next_before: items.length === limit ? items.at(-1).id : null });
        }
        return true;
      }
      if (path === "/me/tokens" && method === "POST") {
        const login = passwords.get(current.account_id);
        if (body.current_password !== login?.password) fail(401, "unauthenticated");
        else if (!body.label?.trim() || body.label.trim().length > 80 ||
                 !Number.isInteger(body.expires_in_days) || body.expires_in_days < 1 || body.expires_in_days > 90)
          fail(400, "invalid_input");
        else json(res, 201, issue(current.account_id, body.label.trim(), body.expires_in_days));
        return true;
      }
      if (path.startsWith("/me/tokens/") && method === "DELETE") {
        const token = credentials.get(path.slice("/me/tokens/".length));
        if (!token || token.account_id !== current.account_id) fail(404, "not_found");
        else { const changed = !token.revoked_at; token.revoked_at = new Date().toISOString(); json(res, 200, { changed }); }
        return true;
      }
      if (path === "/me/sessions" && method === "GET") {
        const value = session();
        json(res, 200, { items: [{ id: value.session_id, credential_id: value.credential_id ?? null, created_at: signedInAt, expires_at: new Date(Date.parse(signedInAt)+8*60*60*1000).toISOString(), revoked_at: null }], next_before: null });
        return true;
      }
      if (path.startsWith("/me/sessions/") && method === "DELETE") {
        const id = decodeURIComponent(path.slice("/me/sessions/".length));
        if (id !== session().session_id) fail(403, "forbidden");
        else { current = null; json(res, 200, { changed: true }); }
        return true;
      }
      if (path.startsWith("/history/") && method === "GET") {
        if (path === "/history/security" && session().role !== "admin") fail(403, "forbidden");
        else {
          const result = history.page(path.slice("/history/".length), session(), url.searchParams);
          result ? json(res, 200, result) : fail(404, "not_found");
        }
        return true;
      }
      if (session().role !== "admin") {
        fail(403, "forbidden");
        return true;
      }
      if (path === "/accounts" && method === "GET") {
        const params = url.searchParams;
        const q = (params.get("q") ?? "").trim().toLowerCase();
        const role = params.get("role"), status = params.get("status") ?? "all";
        const before = params.get("before"), after = params.get("after");
        const limit = Number(params.get("limit") ?? 50);
        if (q.length > 80 || (before && after) || !Number.isInteger(limit) || limit < 1 || limit > 100 ||
            (role && !["admin", "writer", "reader"].includes(role)) ||
            !["all", "current", "active", "disabled", "deleted"].includes(status)) {
          fail(400, "invalid_input");
          return true;
        }
        const rows = [...accounts.values()].filter(row =>
          (!q || row.display_name.toLowerCase().includes(q) || row.id.includes(q)) &&
          (!role || row.role === role) &&
          (status === "all" || (status === "deleted" ? Boolean(row.deleted_at) : !row.deleted_at &&
            (status === "current" || (status === "active" ? row.is_active : !row.is_active)))) &&
          (!before || row.id < before) && (!after || row.id > after),
        ).sort((a, b) => a.id.localeCompare(b.id) * (after ? 1 : -1));
        const more = rows.length > limit;
        const items = rows.slice(0, limit);
        if (after) items.reverse();
        json(res, 200, { items, initialized: accounts.size > 0,
          next_before: after || more ? items.at(-1)?.id ?? after : null,
          previous_after: before || (after && more) ? items[0]?.id ?? before : null,
        });
      }
      else if (path === "/accounts" && method === "POST") {
        const username = typeof body.username === "string" ? body.username.trim().toLowerCase() : "";
        if (
          !body.display_name?.trim() ||
          !["user", "user_with_password_setup"].includes(body.kind) ||
          !["reader", "writer", "admin"].includes(body.role)
        )
          fail(400, "invalid_input");
        else if (body.kind === "user_with_password_setup" && body.current_password !== passwords.get(current.account_id)?.password)
          fail(401, "unauthenticated");
        else if (body.kind === "user_with_password_setup" && (
          [...passwords.values()].some(login => login.username === username) ||
          [...setups.values()].some(setup => setup.username === username)))
          fail(409, "conflict");
        else if (body.kind === "user_with_password_setup" &&
          !/^[a-z0-9][a-z0-9._-]{2,63}$/.test(username))
          fail(400, "invalid_input");
        else {
          const id = randomUUID();
          accounts.set(id, {
            id,
            kind: "user",
            display_name: body.display_name,
            role: body.role,
            is_active: true,
            deleted_at: null,
            username: body.kind === "user_with_password_setup" ? username : null,
            password_ready: false,
          });
          if (body.kind === "user_with_password_setup") {
            const setup = { username,
              token: "gsps_" + randomBytes(32).toString("hex"),
              expires_at: new Date(Date.now() + 86400000).toISOString() };
            setups.set(id, setup);
            json(res, 201, { account_id: id, ...setup });
          } else json(res, 201, { account_id: id });
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
        else if (parts[3] === "password-setup" && method === "POST") {
          const username = typeof body.username === "string" ? body.username.trim().toLowerCase() : "";
          const reserved = [...setups].some(([id, setup]) => id !== account.id && setup.username === username);
          if (body.current_password !== passwords.get(current.account_id)?.password) fail(401, "unauthenticated");
          else if (!account.is_active || account.deleted_at || passwords.has(account.id) ||
                   reserved || [...passwords.values()].some((login) => login.username === username)) fail(409, "conflict");
          else if (!/^[a-z0-9][a-z0-9._-]{2,63}$/.test(username) ||
                   (setups.has(account.id) && setups.get(account.id).username !== username)) fail(400, "invalid_input");
          else {
            const setup = {
              username,
              token: "gsps_" + randomBytes(32).toString("hex"),
              expires_at: new Date(Date.now() + 86400000).toISOString(),
            };
            setups.set(account.id, setup);
            account.username = username;
            json(res, 200, { account_id: account.id, ...setup });
          }
        }
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
          else if (typeof body.current_password !== "string")
            fail(400, "invalid_input");
          else if (body.current_password !== passwords.get(current.account_id)?.password)
            fail(401, "unauthenticated");
          else
            json(res, 201, issue(account.id, body.label, body.expires_in_days));
        } else {
          if (account.deleted_at) { fail(404, "not_found"); return true; }
          if (method === "PATCH" && body.operation === "name") {
            const name = typeof body.display_name === "string" ? body.display_name.trim() : "";
            if (!name || Array.from(name).length > 80) fail(400, "invalid_input");
            else {
              const changed = account.display_name !== name;
              account.display_name = name;
              json(res, 200, { changed });
            }
            return true;
          }
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
              setups.delete(account.id);
              for (const key of credentials.values())
                if (key.account_id === account.id)
                  key.revoked_at = new Date().toISOString();
            } else if (body.operation === "role") account.role = body.role;
            else if (body.operation === "active") {
              account.is_active = body.is_active;
              if (
                !account.is_active &&
                current?.account_id === account.id
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
