import { ApiError, message } from "../api/http";
import { isObject } from "../api/identity";

export type SetupLink = {
  account_id: string;
  username: string;
  expires_at: string;
  token: string;
};
export function isSetupLink(value: unknown): value is SetupLink {
  return (
    isObject(value) &&
    typeof value.account_id === "string" &&
    typeof value.username === "string" &&
    typeof value.expires_at === "string" &&
    Number.isFinite(Date.parse(value.expires_at)) &&
    typeof value.token === "string" &&
    /^gsps_[0-9a-f]{64}$/.test(value.token)
  );
}
export type BrowserSession = {
  id: string;
  credential_id: string | null;
  created_at: string;
  expires_at: string;
  revoked_at: string | null;
};
export function isBrowserSession(value: unknown): value is BrowserSession {
  return (
    isObject(value) &&
    typeof value.id === "string" &&
    (value.credential_id === null || typeof value.credential_id === "string") &&
    ["created_at", "expires_at"].every(
      (key) =>
        typeof value[key] === "string" &&
        Number.isFinite(Date.parse(value[key])),
    ) &&
    (value.revoked_at === null || typeof value.revoked_at === "string")
  );
}
export function reauthenticationError(error: unknown) {
  if (
    !(error instanceof ApiError) ||
    ![400, 401, 403, 404, 409, 429].includes(error.status)
  )
    return "The result is unknown. Close and refresh before making another change.";
  if (error.status === 401)
    return "Current password is incorrect or the session expired.";
  if (error.status === 409)
    return "The change conflicts with the current account state. Refresh and review.";
  if (error.status === 400)
    return "Check the username, password, name and expiry.";
  return message(error);
}
