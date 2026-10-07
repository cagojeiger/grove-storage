import { Page } from "@playwright/test";
import { session } from "./command-fixture";

export const context = {
  id: "9007199254740993",
  created_at: "2026-09-25T00:00:00Z",
  actor_kind: "account",
  actor_id: "user",
  credential_id: "credential",
  session_id: "session",
  request_id: "12345678-1234-1234-1234-123456789abc",
  surface: "console",
};
export const audit = {
  context,
  action: "storage.create",
  resource_type: "storage",
  resource_id: "home-archive",
  metadata: { after: { kind: "s3" } },
};
export const invocation = {
  context,
  operation: "storage.test",
  outcome: "succeeded",
  error_code: null,
  duration_ms: 12,
};
export const security = {
  context,
  event_type: "permission_denied",
  reason_code: "forbidden",
};
export const loginSession = {
  id: "session",
  credential_id: "credential",
  created_at: "2026-09-25T00:00:00Z",
  expires_at: "2099-09-25T00:00:00Z",
  revoked_at: null,
};
export async function maintenanceMock(page: Page, role = "admin") {
  await page.route("**/api/admin/identity/v1/session", (route) =>
    route.fulfill({ json: { ...session, role } }),
  );
  await page.route("**/api/admin/identity/v1/me/sessions?*", (route) =>
    route.fulfill({
      json: {
        items: [
          loginSession,
          { ...loginSession, id: "other" },
          {
            ...loginSession,
            id: "expired",
            expires_at: "2000-01-01T00:00:00Z",
          },
        ],
        next_before: null,
      },
    }),
  );
  await page.route("**/api/admin/identity/v1/history/*", (route) => {
    const path = new URL(route.request().url()).pathname;
    return route.fulfill({
      json: {
        items: [
          path.endsWith("audit")
            ? audit
            : path.endsWith("invocations")
              ? invocation
              : security,
        ],
        next_before: null,
      },
    });
  });
}
