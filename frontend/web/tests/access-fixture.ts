import { Page } from "@playwright/test";
import { Account, Credential } from "../src/api/identity";
import { session } from "./command-fixture";

export const owner: Account = {
  id: "11111111-1111-1111-1111-111111111111",
  kind: "user",
  display_name: "Home administrator",
  role: "admin",
  is_active: true,
  deleted_at: null,
};
export const otherUser: Account = {
  ...owner,
  id: "22222222-2222-2222-2222-222222222222",
  kind: "user",
  display_name: "Writer",
  role: "writer",
};
export const rawToken = "gsm_" + "a".repeat(64);
export const root = "/api/admin/console/#access/users";
export async function accessMock(
  page: Page,
  rows: Account[] = [owner, otherUser],
) {
  const accounts = rows.map((row) => ({ ...row }));
  const tokens: Credential[] = [];
  const writes: {
    method: string;
    path: string;
    body: Record<string, unknown>;
  }[] = [];
  await page.route("**/api/admin/identity/v1/**", async (route) => {
    const req = route.request(),
      path = new URL(req.url()).pathname.split("/v1")[1];
    const body = (req.postData() ? req.postDataJSON() : {}) as Record<
      string,
      unknown
    >;
    const method = req.method();
    if (path === "/session")
      return route.fulfill({ json: { ...session, user_id: owner.id } });
    if (method !== "GET") writes.push({ path, method, body });
    if (path === "/accounts" && method === "GET")
      return route.fulfill({ json: { items: accounts, next_before: null } });
    if (path === "/accounts" && method === "POST") {
      const row: Account = {
        ...owner,
        id: "33333333-3333-3333-3333-333333333333",
        ...body,
      };
      accounts.push(row);
      return route.fulfill({ status: 201, json: { account_id: row.id } });
    }
    if (path.endsWith("/credentials")) {
      if (method === "GET")
        return route.fulfill({ json: { items: tokens, next_before: null } });
      const account_id = path.split("/")[2];
      const token: Credential = {
        id: "44444444-4444-4444-4444-444444444444",
        account_id,
        label: String(body.label),
        token_prefix: rawToken.slice(0, 12),
        created_at: "2026-09-24T00:00:00Z",
        expires_at: "2099-01-01T00:00:00Z",
        revoked_at: null,
      };
      tokens.push(token);
      return route.fulfill({
        status: 201,
        json: {
          account_id,
          credential_id: token.id,
          expires_at: token.expires_at,
          token: rawToken,
        },
      });
    }
    if (path.startsWith("/credentials/")) {
      const token = tokens.find((row) => path.endsWith(row.id));
      if (token) token.revoked_at = "2026-09-24T00:00:00Z";
    } else {
      const row = accounts.find((row) => path.endsWith(row.id));
      if (row?.role === "admin")
        return route.fulfill({ status: 409, json: { error: "conflict" } });
      if (row && method === "DELETE") row.deleted_at = "2026-09-24T00:00:00Z";
      if (row && body.operation === "active")
        row.is_active = Boolean(body.is_active);
      if (row && body.operation === "role")
        row.role = body.role as Account["role"];
    }
    return route.fulfill({ json: { changed: true } });
  });
  return { writes, accounts, tokens };
}
