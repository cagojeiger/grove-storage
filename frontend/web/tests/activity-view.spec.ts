import { expect, test } from "@playwright/test";
import { duration } from "../src/design/format";
import { actor, invocationStatus, sourceName, validator, type Invocation } from "../src/features/activity/model";
import { audit, context, invocation, maintenanceMock } from "./maintenance-fixture";
import { openInvocation } from "./activity-history.mjs";

const commands = ([
  { ...invocation, operation: "storage.test", outcome: "succeeded", duration_ms: 12 },
  { ...invocation, operation: "client.create", outcome: "denied", error_code: "forbidden", duration_ms: 8 },
  { ...invocation, operation: "storage.delete", outcome: "unknown", error_code: "outcome_unknown", duration_ms: 1240, context: { ...context, surface: "cli" } },
  { ...invocation, operation: "storage.replace", outcome: "failed", error_code: "unavailable", duration_ms: 750, context: { ...context, surface: "mcp" } },
] satisfies Invocation[]).map((row, index) => ({ ...row, context: { ...row.context, id: String(BigInt(context.id) + BigInt(index)) } }));

test("activity models preserve outcome uncertainty and validate elapsed time", () => {
  for (const row of commands) {
    expect(validator("invocations")(row)).toBe(true);
    expect(invocationStatus(row.outcome).label).not.toBe("");
  }
  expect(invocationStatus("unknown")).toEqual({ label: "Unknown", color: "warning" });
  for (const value of [-1, 0.5, NaN, Number.MAX_SAFE_INTEGER + 1, "12"])
    expect(validator("invocations")({ ...invocation, duration_ms: value })).toBe(false);
  for (const outcome of ["success", "pending", "toString", null, {}])
    expect(validator("invocations")({ ...invocation, outcome })).toBe(false);
  expect(actor({ ...context, actor_kind: "anonymous", actor_id: null })).toBe("Anonymous");
  expect(sourceName("mcp")).toBe("MCP");
  expect(sourceName("constructor")).toBe("constructor");
  expect(duration(0)).toBe("0 ms");
  expect(duration(999)).toBe("999 ms");
  expect(duration(1240)).toBe("1.2 s");
});

async function commandHistory(page: Parameters<typeof maintenanceMock>[0]) {
  await maintenanceMock(page);
  await page.route("**/identity/v1/history/invocations?*", route => route.fulfill({
    json: { items: commands, next_before: null },
  }));
  await page.goto("/api/admin/console/#activity/invocations");
}

test("command outcomes, source and elapsed time are visible without opening details", async ({ page }) => {
  await commandHistory(page);
  const grid = page.getByRole("grid", { name: "Activity", exact: true });
  for (const label of ["Succeeded", "Denied", "Unknown", "Failed", "12 ms", "1.2 s", "CLI", "MCP"])
    await expect(grid.getByText(label, { exact: true })).toBeVisible();
  await expect(grid.getByRole("columnheader", { name: "Duration", exact: true })).toBeVisible();
  await grid.getByRole("row").filter({ hasText: "storage.delete" }).getByText("Unknown", { exact: true }).click();
  const details = page.getByRole("dialog", { name: "Event details" });
  await expect(details.getByText("Unknown", { exact: true })).toBeVisible();
  await expect(details).toContainText("outcome_unknown");
  await expect(details).toContainText("1.2 s");
  await expect(details.getByText("Succeeded", { exact: true })).toHaveCount(0);
});

test("operation ID matches the backend history value and can be copied", async ({ page, context: browserContext }) => {
  await maintenanceMock(page);
  await browserContext.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/api/admin/console/#activity");
  await page.getByRole("button", { name: "storage.create", exact: true }).click();
  const id = page.getByLabel("Operation ID", { exact: true });
  await expect(id).toHaveValue(context.request_id);
  await expect(id).toHaveAttribute("readonly", "");
  await page.getByRole("button", { name: "Copy operation ID", exact: true }).click();
  await expect(page.getByRole("status")).toHaveText("Operation ID copied.");
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(context.request_id);
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("clipboard rejection does not report a successful copy", async ({ page }) => {
  await maintenanceMock(page);
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: () => Promise.reject(new Error("denied")) } });
  });
  await page.goto("/api/admin/console/#activity");
  await page.getByRole("button", { name: "storage.create", exact: true }).click();
  await page.getByRole("button", { name: "Copy operation ID", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Copy failed.");
  await expect(page.getByRole("status")).not.toContainText("copied");
});

test("audit shows its resource without inventing command duration or outcome", async ({ page }) => {
  await maintenanceMock(page);
  await page.goto("/api/admin/console/#activity");
  await page.getByRole("button", { name: "storage.create", exact: true }).click();
  const details = page.getByRole("dialog", { name: "Event details" });
  await expect(details).toContainText(audit.resource_id);
  await expect(details.locator("dt").filter({ hasText: /^Duration$/ })).toHaveCount(0);
  await expect(details.locator("dt").filter({ hasText: /^Outcome$/ })).toHaveCount(0);
  await expect(details.getByText("Metadata", { exact: true })).toBeVisible();
});

test("inspection preserves resized columns and keyboard focus", async ({ page }) => {
  await commandHistory(page);
  const grid = page.getByRole("grid", { name: "Activity", exact: true });
  const header = grid.locator('[role="columnheader"][data-field="event"]');
  const separator = header.locator(".MuiDataGrid-columnSeparator");
  const originalWidth = (await header.boundingBox())!.width;
  const bounds = await separator.boundingBox();
  expect(bounds).not.toBeNull();
  await page.mouse.move(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
  await page.mouse.down();
  await page.mouse.move(bounds!.x + bounds!.width / 2 + 60, bounds!.y + bounds!.height / 2, { steps: 5 });
  await page.mouse.up();
  const width = (await header.boundingBox())!.width;
  expect(width).toBeGreaterThan(originalWidth);
  const operation = grid.getByRole("button", { name: "storage.test", exact: true });
  // MUI briefly suppresses clicks after resizing; inspect the tooltip before activating.
  await operation.hover();
  await expect(page.getByRole("tooltip", { name: "storage.test", exact: true })).toBeVisible();
  await operation.focus();
  await operation.press("Enter");
  await expect(page.getByRole("dialog", { name: "Event details" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(operation).toBeFocused();
  await expect.poll(async () => (await header.boundingBox())!.width).toBe(width);
});

test("audit correlation follows cursor pages and reveals a virtualized invocation", async ({ page }) => {
  await maintenanceMock(page);
  const cursors: (string | null)[] = [];
  await page.route("**/identity/v1/history/invocations?*", route => {
    const cursor = new URL(route.request().url()).searchParams.get("before");
    cursors.push(cursor);
    return route.fulfill({ json: {
      items: Array.from({ length: 50 }, (_, index) => ({
        ...invocation,
        context: { ...context,
          id: String((cursor ? 50 : 100) - index),
          request_id: cursor && index === 48 ? context.request_id : "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        },
      })),
      next_before: cursor ? null : "51",
    } });
  });
  await page.goto("/api/admin/console/#activity");
  await openInvocation(page, context.request_id);
  await expect(page.getByRole("dialog", { name: "Event details" })).toContainText("Succeeded");
  expect(cursors[0]).toBeNull();
  expect(cursors.filter(cursor => cursor !== null)).toEqual(["51"]);
});

for (const width of [320, 1440]) for (const theme of ["light", "dark"]) {
  test(`activity view and detail ${width}px ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 });
    await commandHistory(page);
    await page.getByRole("button", { name: "Theme", exact: true }).click();
    await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
    const grid = page.getByRole("grid", { name: "Activity", exact: true });
    await expect(grid.getByText("Unknown", { exact: true })).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: `test-results/activity-view-${width}-${theme}.png`, animations: "disabled" });
    await grid.getByRole("button", { name: "storage.delete", exact: true }).click();
    const details = page.getByRole("dialog", { name: "Event details" });
    await expect(details.getByLabel("Operation ID", { exact: true })).toHaveValue(context.request_id);
    await expect(details.getByText("Unknown", { exact: true })).toBeVisible();
    expect(await details.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    await details.screenshot({ path: `test-results/activity-detail-${width}-${theme}.png`, animations: "disabled" });
  });
}
