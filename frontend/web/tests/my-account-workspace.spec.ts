import { expect, test } from "@playwright/test";
import { maintenanceMock, audit, context } from "./maintenance-fixture";
import { session } from "./command-fixture";
import { owner } from "./access-fixture";

test("personal workspace loads private lists only in their respective tabs", async ({
  page,
}) => {
  await maintenanceMock(page);
  await page.route("**/identity/v1/session", (route) =>
    route.fulfill({ json: { ...session, credential_id: null } }),
  );
  await page.route("**/identity/v1/me", (route) =>
    route.fulfill({ json: owner }),
  );
  await page.route("**/identity/v1/me/tokens?*", (route) =>
    route.fulfill({ json: { items: [], next_before: null } }),
  );
  const lists: string[] = [];
  page.on("request", (request) => {
    if (/\/me\/(sessions|tokens)\?/.test(request.url()))
      lists.push(request.url());
  });
  await page.goto("/api/admin/console/#settings");
  await expect(
    page.getByRole("tab", { name: "Profile", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByRole("tab", { name: "Security", exact: true }),
  ).toBeVisible();
  expect(lists).toEqual([]);
  await expect(
    page.getByRole("button", { name: "Refresh sessions" }),
  ).toHaveCount(0);
  await page.getByRole("tab", { name: "API tokens", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Issue token", exact: true }),
  ).toBeVisible();
  expect(lists.some((url) => url.includes("/tokens?"))).toBe(true);
  expect(lists.some((url) => url.includes("/sessions?"))).toBe(false);
  await page.getByRole("tab", { name: "Sessions", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Revoke current session", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Refresh sessions" }),
  ).toBeEnabled();
  await page.reload();
  await expect(
    page.getByRole("tab", { name: "Sessions", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
});

test("token sessions cannot reveal personal API token management through a tab URL", async ({
  page,
}) => {
  await maintenanceMock(page);
  await page.goto("/api/admin/console/#settings?tab=tokens");
  await expect(page.getByRole("alert")).toHaveText("Password sign-in required.");
  await expect(
    page.getByRole("tab", { name: "Profile", exact: true }),
  ).toHaveAttribute("aria-selected", "false");
  await expect(
    page.getByRole("tab", { name: "API tokens", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("tab", { name: "Security", exact: true }),
  ).toHaveCount(0);
});

test("activity retains full attribution in event details", async ({
  page,
}) => {
  await maintenanceMock(page);
  const id = "12345678-1234-1234-1234-123456789abc";
  await page.route("**/identity/v1/history/audit?*", (route) =>
    route.fulfill({
      json: {
        items: [{ ...audit, context: { ...context, actor_id: id } }],
        next_before: null,
      },
    }),
  );
  await page.goto("/api/admin/console/#activity");
  await expect(page.getByRole("grid", { name: "Activity" }).getByRole("gridcell", { name: id, exact: true })).toHaveCount(1);
  await page
    .getByRole("button", { name: "storage.create", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toContainText(id);
});
