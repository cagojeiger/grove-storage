import { expect, test } from "@playwright/test";
import { accessMock, owner, otherUser } from "./access-fixture";
import { audit, context, maintenanceMock } from "./maintenance-fixture";

const tokenId = "44444444-4444-4444-4444-444444444444";
const filtered = `/api/admin/console/#activity?account_id=${owner.id}&credential_id=${tokenId}`;

test("activity uses a compact filter toolbar and a tab underline", async ({ page }) => {
  await maintenanceMock(page);
  await page.goto("/api/admin/console/#activity");
  await expect(page.getByLabel("Actor account ID")).toBeHidden();
  const tab = page.getByRole("tab", { name: "Audit log" });
  await expect(tab).toHaveCSS("border-left-width", "0px");
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
  await expect(page.getByRole("button", { name: "Filters" })).toHaveCount(0);
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
  await page.getByRole("button", { name: "Load more", exact: true }).click();
  await expect(page.getByRole("button", { name: /client.create/ })).toBeVisible();
  expect(requests.at(-1)?.searchParams.get("before")).toBe(context.id);
  await page.getByRole("tab", { name: "Command history", exact: true }).click();
  await expect(page.getByRole("button", { name: /storage.test/ })).toBeVisible();
  await page.reload();
  await expect(page.getByLabel("Used token ID")).toHaveValue(tokenId);
  await page.getByRole("tab", { name: "Security events", exact: true }).click();
  await expect(page.getByRole("button", { name: /permission_denied/ })).toBeVisible();
  expect(requests.every(url => url.searchParams.get("account_id") === owner.id && url.searchParams.get("credential_id") === tokenId)).toBe(true);
  await page.getByRole("tab", { name: "Audit log", exact: true }).click();
  await expect(page.getByRole("button", { name: /storage.create/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /client.create/ })).toHaveCount(0);
  await page.getByLabel("Actor account ID").fill(otherUser.id);
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect.poll(() => requests.at(-1)?.searchParams.get("account_id")).toBe(otherUser.id);
  expect(requests.at(-1)?.searchParams.has("before")).toBe(false);
  await page.getByRole("link", { name: "Clear filters" }).click();
  await expect(page.getByLabel("Actor account ID")).toHaveValue("");
  await expect.poll(() => requests.at(-1)?.searchParams.has("account_id")).toBe(false);
});

test("invalid deep-link filters make no request and can be cleared", async ({ page }) => {
  await maintenanceMock(page);
  let requests = 0;
  page.on("request", request => { if (request.url().includes("/history/")) requests++; });
  await page.goto("/api/admin/console/#activity?account_id=invalid");
  await expect(page.getByRole("alert")).toHaveText("Enter a valid account or token ID.");
  expect(requests).toBe(0);
  await page.getByRole("link", { name: "Clear filters" }).click();
  await expect(page.getByRole("button", { name: /storage.create/ })).toBeVisible();
});

test("changing filter removes an open event detail", async ({ page }) => {
  await maintenanceMock(page);
  await page.goto(filtered);
  await page.getByRole("button", { name: /storage.create/ }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.evaluate(id => { location.hash = `activity?account_id=${id}`; }, otherUser.id);
  await expect(page.getByRole("dialog")).toHaveCount(0);
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
    await page.getByLabel("Theme").selectOption(theme);
    await expect(page.getByLabel("Actor account ID")).toHaveValue(owner.id);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: `test-results/activity-filters-${width}-${theme}.png`, fullPage: true });
  });
}
