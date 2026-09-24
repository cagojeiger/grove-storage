import { ApiError, identity, send } from "./http";

export type Role = "reader" | "writer" | "admin";
export type Account = {
  id: string;
  kind: "user";
  display_name: string;
  role: Role;
  is_active: boolean;
  deleted_at: string | null;
};
export type Credential = {
  id: string;
  account_id: string;
  label: string;
  token_prefix: string;
  created_at: string;
  expires_at: string;
  revoked_at: string | null;
};
export type Issued = {
  token: string;
  credential_id: string;
  expires_at: string;
  user_id?: string;
  account_id?: string;
};
export type Page<T> = { items: T[]; next_before: string | null };
export type MasterSession = {
  principal: "master";
  scope: "setup_recovery";
  initialized: boolean;
};
export const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const nullable = (v: unknown) => v === null || typeof v === "string";
export const isAccount = (v: unknown): v is Account =>
  isObject(v) &&
  ["id", "display_name"].every((k) => typeof v[k] === "string") &&
  v.kind === "user" &&
  ["reader", "writer", "admin"].includes(String(v.role)) &&
  typeof v.is_active === "boolean" &&
  nullable(v.deleted_at);
export const isCredential = (v: unknown): v is Credential =>
  isObject(v) &&
  [
    "id",
    "account_id",
    "label",
    "token_prefix",
    "created_at",
    "expires_at",
  ].every((k) => typeof v[k] === "string") &&
  nullable(v.revoked_at);
export const isIssued = (v: unknown): v is Issued =>
  isObject(v) &&
  typeof v.token === "string" &&
  /^gsm_[a-f0-9]{64}$/.test(v.token) &&
  typeof v.credential_id === "string" &&
  typeof v.expires_at === "string" &&
  Number.isFinite(Date.parse(v.expires_at)) &&
  (typeof v.user_id === "string" || typeof v.account_id === "string");
export const isMaster = (v: unknown): v is MasterSession =>
  isObject(v) &&
  v.principal === "master" &&
  v.scope === "setup_recovery" &&
  typeof v.initialized === "boolean";
export const isChanged = (v: unknown): v is { changed: boolean } =>
  isObject(v) && typeof v.changed === "boolean";
export const isCreated = (v: unknown): v is { account_id: string } =>
  isObject(v) && typeof v.account_id === "string";

// Identity errors have a different wire contract from resource commands.
export async function identityRequest<T>(
  path: string,
  valid: (v: unknown) => v is T,
  options: RequestInit = {},
): Promise<T> {
  const response = await send(identity + path, options);
  if (response.status === 401 || response.status === 403)
    throw new ApiError(response.status, null, "not_applied");
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new ApiError(502, null, "unknown");
  }
  if (!response.ok) {
    const codes: Record<string, number> = {
      conflict: 409,
      invalid_input: 400,
      request_rejected: 400,
      not_found: 404,
      unavailable: 503,
      outcome_unknown: 503,
      rate_limited: 429,
    };
    if (
      isObject(body) &&
      typeof body.error === "string" &&
      codes[body.error] === response.status
    )
      throw new ApiError(
        response.status,
        Number(response.headers.get("Retry-After")) || null,
        body.error === "outcome_unknown" ? "unknown" : "not_applied",
      );
    throw new ApiError(502, null, "unknown");
  }
  if (!valid(body)) throw new ApiError(502, null, "unknown");
  return body;
}

export function identityPage<T>(
  path: string,
  item: (v: unknown) => v is T,
  before: string | null,
  signal: AbortSignal,
) {
  return identityRequest(
    path +
      "?limit=50" +
      (before ? `&before=${encodeURIComponent(before)}` : ""),
    (v): v is Page<T> =>
      isObject(v) &&
      Array.isArray(v.items) &&
      v.items.every(item) &&
      nullable(v.next_before),
    { signal },
  );
}
export function identityMessage(error: unknown): string {
  if (!(error instanceof ApiError) || error.outcome !== "not_applied")
    return "The change outcome is unknown. Close and refresh before making another change. A token may have been issued; review the token list before issuing again.";
  if (error.status === 409)
    return "Change blocked. Keep an active Admin, check the account state and token limit, then refresh.";
  if (error.status === 404)
    return "The account, token, or master configuration is unavailable.";
  if (error.status === 400) return "Check the name, role and expiry.";
  return error.message;
}

export function field(data: FormData, name: string): string {
  const value = data.get(name);
  return typeof value === "string" ? value : "";
}
