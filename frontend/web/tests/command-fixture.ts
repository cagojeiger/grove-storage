import { Page, Route } from "@playwright/test";

export const commandUrl = "**/api/admin/console-commands/v1";
export const session = { principal: "user", user_id: "user", credential_id: "credential", session_id: "session", role: "admin" };
export function envelope(command: string, result: unknown) {
  return { protocol: 1, request_id: "12345678-1234-1234-1234-123456789abc", command, result };
}
export function failure(status: number) {
  const codes: Record<number, string> = { 400: "invalid_input", 401: "unauthorized", 403: "forbidden", 404: "not_found", 409: "conflict", 500: "internal", 503: "unavailable" };
  return { protocol: 1, request_id: "12345678-1234-1234-1234-123456789abc", error: { code: codes[status], outcome: status >= 500 ? "unknown" : "not_applied" } };
}
export async function intercept(page: Page, command: string, handler: (route: Route) => Promise<void>) {
  await page.route(commandUrl, (route) =>
    (route.request().postDataJSON() as {command: string}).command === command ? handler(route) : route.fallback());
}
