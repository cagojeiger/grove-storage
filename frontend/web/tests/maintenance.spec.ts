import { expect, test } from "@playwright/test";
import {
  audit,
  context,
  loginSession,
  maintenanceMock,
} from "./maintenance-fixture";
import { session } from "./command-fixture";

test("activity tabs, bigint cursor and details preserve actor and token attribution", async ({
  page,
}) => {
  await maintenanceMock(page);
  const cursors: (string | null)[] = [];
  await page.route("**/identity/v1/history/audit?*", (route) => {
    const before = new URL(route.request().url()).searchParams.get("before");
    cursors.push(before);
    return route.fulfill({
      json: {
        items: [
          {
            ...audit,
            context: { ...context, id: before ? "1" : context.id },
            action: before ? "client.create" : audit.action,
          },
        ],
        next_before: before ? null : context.id,
      },
    });
  });
  await page.goto("/api/admin/console/#activity");
  await page.getByRole("button", { name: /storage.create/ }).click();
  await expect(page.getByRole("dialog")).toContainText("Token ID");
  await expect(page.getByRole("dialog")).toContainText(context.request_id);
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByRole("button", { name: "Load more", exact: true }).click();
  await expect(
    page.getByRole("button", { name: /client.create/ }),
  ).toBeVisible();
  expect(cursors.filter((cursor) => cursor !== null)).toEqual([context.id]);
  expect(cursors[0]).toBeNull();
  await page
    .getByRole("link", { name: "Command history", exact: true })
    .click();
  await page.getByRole("button", { name: /storage.test/ }).click();
  await expect(page.getByRole("dialog")).toContainText("Duration (ms)");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page
    .getByRole("link", { name: "Security events", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: /permission_denied/ }),
  ).toBeVisible();
});

for (const role of ["reader", "writer"])
  test(`${role} cannot request security events from a direct link`, async ({
    page,
  }) => {
    await maintenanceMock(page, role);
    let requests = 0;
    page.on("request", (r) => {
      if (r.url().includes("/history/security")) requests++;
    });
    await page.goto("/api/admin/console/#activity/security");
    await expect(page.getByRole("alert")).toHaveText("Admin access required.");
    await expect(
      page.getByRole("link", { name: "Security events" }),
    ).toHaveCount(0);
    expect(requests).toBe(0);
    await page.getByRole("link", { name: "Audit log" }).click();
    await expect(page.getByText("MY ACTIVITY", { exact: true })).toBeVisible();
  });

test("demotion and forbidden history responses remove installation event details", async ({
  page,
}) => {
  await maintenanceMock(page);
  await page.goto("/api/admin/console/#activity/security");
  await page.getByRole("button", { name: /permission_denied/ }).click();
  await page.route("**/identity/v1/session", (r) =>
    r.fulfill({ json: { ...session, role: "reader" } }),
  );
  await page.route("**/identity/v1/history/security?*", (r) =>
    r.fulfill({ status: 403, json: {} }),
  );
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByRole("button", { name: "Refresh activity" }).click();
  await expect(page.getByText("Reader · Read-only")).toBeVisible();
  await expect(
    page.getByRole("button", { name: /permission_denied/ }),
  ).toHaveCount(0);
});

test("sessions page confirms revocation and signs out when revoking the current session", async ({
  page,
}) => {
  await maintenanceMock(page);
  const requests: string[] = [];
  await page.route("**/identity/v1/sessions/*", (route) => {
    requests.push(
      route.request().method() + " " + new URL(route.request().url()).pathname,
    );
    return route.fulfill({ json: { changed: true } });
  });
  await page.goto("/api/admin/console/#settings");
  await expect(
    page.getByRole("button", { name: "Revoke session expired", exact: true }),
  ).toBeDisabled();
  await page
    .getByRole("button", { name: "Revoke session other", exact: true })
    .click();
  expect(requests).toEqual([]);
  await page.getByRole("button", { name: "Confirm revoke" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: "Revoke current session" }).click();
  await page.getByRole("button", { name: "Confirm revoke" }).click();
  await expect(page.getByLabel("Account token")).toBeVisible();
  expect(requests).toEqual([
    "DELETE /api/admin/identity/v1/sessions/other",
    "DELETE /api/admin/identity/v1/sessions/session",
  ]);
});

test("unknown revocation is never retried and Root sessions accept a null token ID", async ({
  page,
}) => {
  await maintenanceMock(page);
  await page.route("**/identity/v1/session", (r) =>
    r.fulfill({
      json: {
        principal: "root",
        role: "root",
        session_id: "session",
        expires_at: loginSession.expires_at,
      },
    }),
  );
  await page.route("**/identity/v1/sessions?*", (r) =>
    r.fulfill({
      json: {
        items: [{ ...loginSession, credential_id: null }],
        next_before: null,
      },
    }),
  );
  let calls = 0;
  await page.route("**/identity/v1/sessions/session", (r) => {
    calls++;
    return r.abort();
  });
  await page.goto("/api/admin/console/#settings");
  await page.getByRole("button", { name: "Revoke current session" }).click();
  await page.getByRole("button", { name: "Confirm revoke" }).click();
  await expect(page.getByRole("alert")).toContainText("outcome is unknown");
  await expect(
    page.getByRole("button", { name: "Confirm revoke" }),
  ).toBeDisabled();
  expect(calls).toBe(1);
});

for (const pageName of ["activity", "settings"])
  test(`${pageName} rejects malformed data and clears private data on 401`, async ({
    page,
  }) => {
    await maintenanceMock(page);
    const url =
      pageName === "activity"
        ? "**/identity/v1/history/audit?*"
        : "**/identity/v1/sessions?*";
    await page.route(url, (r) =>
      r.fulfill({ json: { items: [{}], next_before: null } }),
    );
    await page.goto(`/api/admin/console/#${pageName}`);
    await expect(page.getByRole("alert")).toBeVisible();
    await page.route(url, (r) => r.fulfill({ status: 401, json: {} }));
    await page
      .getByRole("button", {
        name: pageName === "activity" ? "Refresh activity" : "Refresh sessions",
      })
      .click();
    await expect(page.getByLabel("Account token")).toBeVisible();
  });
