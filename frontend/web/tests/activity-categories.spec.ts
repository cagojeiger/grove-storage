import { expect, test } from "@playwright/test";
import { activityFilters } from "../src/features/activity/filters";
import { context, invocation, maintenanceMock, security } from "./maintenance-fixture";

test("category filters are validated and limited to operation history", () => {
  expect(activityFilters("activity/invocations?category=authentication").valid).toBe(true);
  expect(activityFilters("activity/invocations?category=invalid").valid).toBe(false);
  expect(activityFilters("activity/security?category=authentication").params.has("category")).toBe(false);
});

test("operation categories use server filtering and reset pagination", async ({ page }) => {
  await maintenanceMock(page);
  const requests: URL[] = [];
  await page.route("**/identity/v1/history/invocations?*", route => {
    const url = new URL(route.request().url());
    requests.push(url);
    return route.fulfill({ json: {
      items: [{ ...invocation, operation: url.searchParams.get("category") === "authentication" ? "identity.session.logout" : "storage.test",
        context: { ...context, id: url.searchParams.has("before") ? "1" : context.id } }],
      next_before: url.searchParams.has("before") ? null : context.id,
    } });
  });
  await page.goto("/api/admin/console/#activity/invocations");
  await expect(page.getByRole("tab", { name: "Operation history", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("button", { name: "Test storage", exact: true })).toBeVisible();
  expect(requests.at(-1)?.searchParams.get("category")).toBe("management");
  await page.getByRole("button", { name: "Go to next page", exact: true }).click();
  await expect.poll(() => requests.at(-1)?.searchParams.get("before")).toBe(context.id);
  await page.getByRole("combobox", { name: "Category", exact: true }).click();
  await page.getByRole("option", { name: "Authentication & sessions", exact: true }).click();
  await expect(page.getByRole("grid").getByRole("button", { name: "Sign out", exact: true })).toBeVisible();
  expect(requests.at(-1)?.searchParams.has("before")).toBe(false);
  await page.reload();
  await expect(page.getByRole("combobox", { name: "Category", exact: true })).toContainText("Authentication & sessions");
  await page.getByRole("tab", { name: "Security events", exact: true }).click();
  await expect(page.getByRole("button", { name: "Permission denied", exact: true })).toBeVisible();
  await expect(page).not.toHaveURL(/category=/);
});

test("related authentication records are not hidden by the management default", async ({ page }) => {
  await maintenanceMock(page);
  const requests: URL[] = [];
  await page.route("**/identity/v1/history/invocations?*", route => {
    const url = new URL(route.request().url());
    requests.push(url);
    return route.fulfill({ json: { items: [{ ...invocation, operation: "identity.session.logout" }], next_before: null } });
  });
  await page.goto(`/api/admin/console/#activity/invocations?category=authentication&account_id=11111111-1111-4111-8111-111111111111`);
  await page.getByRole("grid").getByRole("button", { name: "Sign out", exact: true }).click();
  await page.getByRole("dialog").getByRole("link", { name: "View related history", exact: true }).click();
  await expect(page.getByRole("combobox", { name: "Category", exact: true })).toContainText("All operations");
  expect(requests.at(-1)?.searchParams.get("category")).toBe("all");
  expect(requests.at(-1)?.searchParams.get("operation_id")).toBe(context.request_id);
  expect(requests.at(-1)?.searchParams.get("account_id")).toBe("11111111-1111-4111-8111-111111111111");
});

for (const width of [390, 1440]) test(`activity categories and session end at ${width}px`, async ({ page }) => {
  await page.setViewportSize({ width, height: 960 });
  await maintenanceMock(page);
  await page.route("**/identity/v1/history/security?*", route => route.fulfill({ json: {
    items: [{ ...security, event_type: "session_ended", reason_code: "user_requested", operation: "identity.session.logout",
      stage: "execution", auth_method: "session", is_sampled: false }], next_before: null,
  } }));
  await page.goto("/api/admin/console/#activity/invocations");
  await expect(page.getByRole("combobox", { name: "Category", exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: `test-results/operation-categories-${width}.png`, fullPage: true });
  await page.getByRole("tab", { name: "Security events", exact: true }).click();
  await page.getByRole("button", { name: "Signed out", exact: true }).click();
  await expect(page.getByRole("dialog").getByText("user_requested", { exact: true })).toBeVisible();
  await page.screenshot({ path: `test-results/security-session-end-${width}.png`, fullPage: true });
});

test("invalid categories do not send history requests", async ({ page }) => {
  await maintenanceMock(page);
  let requests = 0;
  page.on("request", request => { if (request.url().includes("/history/")) requests++; });
  await page.goto("/api/admin/console/#activity/invocations?category=invalid");
  await expect(page.getByRole("alert")).toHaveText("Select a valid activity category.");
  expect(requests).toBe(0);
});
