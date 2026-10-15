import { expect, test } from "@playwright/test";
import { accessMock, owner, otherUser } from "./access-fixture";
import { audit, context, invocation, maintenanceMock, security } from "./maintenance-fixture";

const tokenId = "44444444-4444-4444-4444-444444444444";
const filtered = `/api/admin/console/#activity?account_id=${owner.id}&credential_id=${tokenId}`;

test("activity uses a compact filter toolbar and a tab underline", async ({ page }) => {
  await maintenanceMock(page);
  await page.goto("/api/admin/console/#activity");
  await expect(page.getByLabel("Actor account ID")).toBeHidden();
  const tab = page.getByRole("tab", { name: "Audit log" });
  await expect(tab).toHaveCSS("border-left-color", "rgba(0, 0, 0, 0)");
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".MuiTabs-indicator")).toBeVisible();
  await page.getByRole("button", { name: "Filters" }).click();
  await expect(page.getByLabel("Actor account ID")).toBeVisible();
  await page.getByRole("button", { name: "Filters" }).click();
  await expect(page.getByLabel("Actor account ID")).toBeHidden();
});

test("activity distinguishes no history from no filtered results", async ({ page }) => {
  await maintenanceMock(page);
  await page.route("**/identity/v1/history/*", route => route.fulfill({ json: { items: [], next_before: null } }));
  await page.goto("/api/admin/console/#activity");
  await expect(page.getByText("No activity recorded yet.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Filters" })).toBeVisible();
  await page.goto(filtered);
  await expect(page.getByText("No activity matches these filters.")).toBeVisible();
  await expect(page.getByRole("link", { name: "Clear filters" })).toBeVisible();
});

test("account and revoked token links select their actor filters", async ({ page }) => {
  await accessMock(page);
  await maintenanceMock(page);
  await page.route("**/accounts/*/credentials?*", route => route.fulfill({ json: {
    items: [{ id: tokenId, account_id: owner.id, label: "Old token", token_prefix: "gsm_old",
      created_at: "2026-09-01T00:00:00Z", expires_at: "2026-09-20T00:00:00Z", revoked_at: "2026-09-10T00:00:00Z" }],
    next_before: null,
  } }));
  await page.goto(`/api/admin/console/#accounts/${owner.id}`);
  await page.getByRole("link", { name: "View account actions" }).click();
  await expect(page.getByLabel("Actor account ID")).toHaveValue(owner.id);
  await expect(page.getByLabel("Used token ID")).toHaveValue("");
  await page.goBack();
  await page.getByRole("link", { name: "View token actions" }).click();
  await expect(page.getByLabel("Actor account ID")).toHaveValue(owner.id);
  await expect(page.getByLabel("Used token ID")).toHaveValue(tokenId);
});

test("filters survive pagination, tabs, reload and reset without stale rows", async ({ page }) => {
  await maintenanceMock(page);
  const requests: URL[] = [];
  await page.route("**/identity/v1/history/*", route => {
    const url = new URL(route.request().url());
    requests.push(url);
    if (!url.pathname.endsWith("audit")) return route.fallback();
    const before = url.searchParams.get("before");
    return route.fulfill({ json: {
      items: [{ ...audit, action: before ? "client.create" : "storage.create",
        context: { ...context, actor_id: owner.id, credential_id: tokenId, id: before ? "1" : context.id } }],
      next_before: before ? null : context.id,
    } });
  });
  await page.goto(filtered);
  await page.getByRole("button", { name: "Go to next page", exact: true }).click();
  await expect(page.getByRole("button", { name: /Create client/ })).toBeVisible();
  expect(requests.at(-1)?.searchParams.get("before")).toBe(context.id);
  await page.getByRole("tab", { name: "Command history", exact: true }).click();
  await expect(page.getByRole("button", { name: /Test storage/ })).toBeVisible();
  await page.reload();
  await expect(page.getByLabel("Used token ID")).toHaveValue(tokenId);
  await page.getByRole("tab", { name: "Security events", exact: true }).click();
  await expect(page.getByRole("button", { name: /Permission denied/ })).toBeVisible();
  expect(requests.every(url => url.searchParams.get("account_id") === owner.id && url.searchParams.get("credential_id") === tokenId)).toBe(true);
  await page.getByRole("tab", { name: "Audit log", exact: true }).click();
  await expect(page.getByRole("button", { name: /Create storage/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Create client/ })).toHaveCount(0);
  await page.getByLabel("Actor account ID").fill(otherUser.id);
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect.poll(() => requests.at(-1)?.searchParams.get("account_id")).toBe(otherUser.id);
  expect(requests.at(-1)?.searchParams.has("before")).toBe(false);
  await page.getByRole("link", { name: "Clear filters" }).click();
  await expect(page.getByLabel("Actor account ID")).toBeHidden();
  await page.getByRole("button", { name: "Filters", exact: true }).click();
  await expect(page.getByLabel("Actor account ID")).toHaveValue("");
  await expect.poll(() => requests.at(-1)?.searchParams.has("account_id")).toBe(false);
});

test("invalid deep-link filters make no request and can be cleared", async ({ page }) => {
  await maintenanceMock(page);
  let requests = 0;
  page.on("request", request => { if (request.url().includes("/history/")) requests++; });
  await page.goto("/api/admin/console/#activity?account_id=invalid");
  await expect(page.getByRole("alert")).toHaveText("Enter a valid operation, account or token ID.");
  expect(requests).toBe(0);
  await page.getByRole("link", { name: "Clear filters" }).click();
  await expect(page.getByRole("button", { name: /Create storage/ })).toBeVisible();
});

test("changing filter removes an open event detail", async ({ page }) => {
  await maintenanceMock(page);
  await page.goto(filtered);
  await page.getByRole("button", { name: /Create storage/ }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.evaluate(id => { location.hash = `activity?account_id=${id}`; }, otherUser.id);
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("related history narrows every tab by operation without discarding actor filters", async ({ page }) => {
  await maintenanceMock(page);
  const requests: URL[] = [];
  await page.route("**/identity/v1/history/*", route => {
    const url = new URL(route.request().url());
    requests.push(url);
    const row = url.pathname.endsWith("audit") ? audit : url.pathname.endsWith("invocations") ? invocation : security;
    const rows = [context.request_id, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"].map((operation, index) => ({
      ...row, context: { ...context, id: String(100 - index), request_id: operation, actor_id: owner.id, credential_id: tokenId },
    }));
    const operation = url.searchParams.get("operation_id");
    return route.fulfill({ json: { items: rows.filter(row => !operation || row.context.request_id === operation), next_before: null } });
  });
  await page.goto(filtered);
  await page.getByRole("button", { name: "Create storage", exact: true }).first().click();
  await page.getByRole("dialog").getByRole("link", { name: "View related history", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByLabel("Operation ID", { exact: true })).toHaveValue(context.request_id);
  for (const [tab, label] of [["Audit log", "Create storage"], ["Command history", "Test storage"], ["Security events", "Permission denied"]]) {
    await page.getByRole("tab", { name: tab, exact: true }).click();
    const button = page.getByRole("grid", { name: "Activity", exact: true }).getByRole("button", { name: label, exact: true });
    await expect(button).toHaveCount(1);
    const url = requests.at(-1)!;
    expect(url.searchParams.get("operation_id")).toBe(context.request_id);
    expect(url.searchParams.get("account_id")).toBe(owner.id);
    expect(url.searchParams.get("credential_id")).toBe(tokenId);
    expect(url.searchParams.has("before")).toBe(false);
  }
  await page.reload();
  await expect(page.getByLabel("Operation ID", { exact: true })).toHaveValue(context.request_id);
  await page.getByRole("link", { name: "Clear filters", exact: true }).click();
  await expect(page.getByRole("button", { name: "Permission denied", exact: true })).toHaveCount(2);
  expect(requests.at(-1)!.searchParams.has("operation_id")).toBe(false);
});

test("invalid operation ID blocks requests and can be corrected", async ({ page }) => {
  await maintenanceMock(page);
  const requests: URL[] = [];
  page.on("request", request => { if (request.url().includes("/history/")) requests.push(new URL(request.url())); });
  await page.goto("/api/admin/console/#activity?operation_id=invalid");
  await expect(page.getByRole("alert")).toHaveText("Enter a valid operation, account or token ID.");
  expect(requests).toHaveLength(0);
  await page.getByLabel("Operation ID", { exact: true }).fill(context.request_id);
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.getByRole("button", { name: "Create storage", exact: true })).toBeVisible();
  expect(requests.at(-1)!.searchParams.get("operation_id")).toBe(context.request_id);
});

for (const role of ["reader", "writer"]) test(`${role} filtered security stays forbidden`, async ({ page }) => {
  await maintenanceMock(page, role);
  let requests = 0;
  page.on("request", request => { if (request.url().includes("/history/security")) requests++; });
  await page.goto(filtered.replace("#activity?", "#activity/security?"));
  await expect(page.getByRole("alert")).toHaveText("Admin access required.");
  expect(requests).toBe(0);
});

for (const width of [320, 768, 1440]) for (const theme of ["light", "dark"]) {
  test(`activity filters ${width}px ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 });
    await maintenanceMock(page);
    await page.goto(filtered);
    await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
    await expect(page.getByLabel("Actor account ID")).toHaveValue(owner.id);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: `test-results/activity-filters-${width}-${theme}.png`, fullPage: true });
  });
}
