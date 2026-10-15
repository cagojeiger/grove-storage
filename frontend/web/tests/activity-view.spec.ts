import { expect, test } from "@playwright/test";
import { duration } from "../src/design/format";
import { actor, activityName, authMethodName, eventLabel, eventName, invocationStatus, recordingStatus, sourceName, stageName, targetName, validator, type Invocation } from "../src/features/activity/model";
import { audit, context, invocation, maintenanceMock, security } from "./maintenance-fixture";
import { openInvocation } from "./activity-history.mjs";

const commands = ([
  { ...invocation, operation: "storage.test", outcome: "succeeded", duration_ms: 12, target_type: "storage", target_id: "backup-s3", stage: "commit", auth_method: "session" },
  { ...invocation, operation: "client.create", outcome: "denied", error_code: "forbidden", duration_ms: 8, target_type: "client", target_id: "notegate", stage: "authorization", auth_method: "session" },
  { ...invocation, operation: "storage.delete", outcome: "unknown", error_code: "outcome_unknown", duration_ms: 1240, target_type: "storage", target_id: "home-archive", stage: "commit", auth_method: "api_token", context: { ...context, surface: "cli" } },
  { ...invocation, operation: "storage.replace", outcome: "failed", error_code: "unavailable", duration_ms: 750, target_type: "storage", target_id: "backup-s3", stage: "execution", auth_method: "api_token", context: { ...context, surface: "mcp" } },
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

test("history labels preserve missing context and recording uncertainty", () => {
  expect(stageName(null)).toBe("Not recorded");
  expect(authMethodName(undefined)).toBe("Not recorded");
  expect(stageName("browser_admission")).toBe("Browser admission");
  expect(stageName("future_stage")).toBe("future_stage");
  expect(authMethodName("constructor")).toBe("constructor");
  expect(targetName(invocation as Invocation)).toBe("Not recorded");
  expect(recordingStatus(true).label).toBe("Representative");
  expect(recordingStatus(false).label).toBe("Individual");
  expect(recordingStatus(null).label).toBe("Unknown");
  expect(recordingStatus(undefined).label).toBe("Unknown");
});

test("activity labels leave raw codes and unknown operations unchanged", () => {
  expect(eventLabel(audit)).toBe("Create storage");
  expect(eventLabel(security)).toBe("Permission denied");
  expect(eventName(security)).toBe("permission_denied");
  expect(activityName("identity.credential.issue")).toBe("Issue API token");
  expect(activityName("credential.create")).toBe("Create S3 credential");
  expect(activityName("future.operation")).toBe("future.operation");
  expect(activityName("constructor")).toBe("constructor");
});

test("security operation context does not change the event discriminator", () => {
  for (const operation of ["identity.session.login", null]) {
    const row = { ...security, operation, stage: "authentication", auth_method: "password", is_sampled: true };
    expect(validator("security")(row)).toBe(true);
    expect(eventName(row)).toBe("permission_denied");
  }
  expect(validator("security")({ ...security, is_sampled: "true" })).toBe(false);
  expect(validator("invocations")({ ...invocation, target_id: {} })).toBe(false);
});

test("security records with operation context remain selectable", async ({ page }) => {
  await maintenanceMock(page);
  await page.route("**/identity/v1/history/security?*", route => route.fulfill({
    json: { items: [{ ...security, operation: "identity.session.login", stage: "authentication", auth_method: "password", is_sampled: true }], next_before: null },
  }));
  await page.goto("/api/admin/console/#activity/security");
  await page.getByRole("button", { name: "Permission denied", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Event details" })).toContainText("permission_denied");
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
  await grid.getByRole("row").filter({ hasText: "Delete storage" }).getByText("Unknown", { exact: true }).click();
  const details = page.getByRole("dialog", { name: "Event details" });
  await expect(details.getByText("Unknown", { exact: true })).toBeVisible();
  await expect(details).toContainText("outcome_unknown");
  await expect(details).toContainText("1.2 s");
  await expect(details).toContainText("home-archive");
  await expect(details).toContainText("Commit confirmation");
  await expect(details).toContainText("API token");
  await expect(details.getByText("Succeeded", { exact: true })).toHaveCount(0);
});

test("command targets and stages are visible and absent legacy fields are not inferred", async ({ page }) => {
  await commandHistory(page);
  const grid = page.getByRole("grid", { name: "Activity", exact: true });
  await expect(grid.getByRole("columnheader", { name: "Target", exact: true })).toBeVisible();
  await expect(grid).toContainText("storage: home-archive");
  await expect(grid).toContainText("Authorization");
  await page.route("**/identity/v1/history/invocations?*", route => route.fulfill({ json: { items: [invocation], next_before: null } }));
  await page.getByRole("button", { name: "Refresh activity", exact: true }).click();
  await grid.getByRole("button", { name: "Test storage", exact: true }).click();
  const details = page.getByRole("dialog", { name: "Event details" });
  await expect(details.getByText("Not recorded", { exact: true })).toHaveCount(4);
  await expect(details.getByText("Password", { exact: true })).toHaveCount(0);
});

const securityRows = [
  { ...security, event_type: "authentication_failed", reason_code: "unauthenticated", operation: "identity.session.login", stage: "authentication", auth_method: "password", is_sampled: true,
    context: { ...context, id: "101", actor_kind: "anonymous", actor_id: null } },
  { ...security, operation: "identity.account.role", stage: "authorization", auth_method: "session", is_sampled: false, context: { ...context, id: "102" } },
  { ...security, operation: null, stage: null, auth_method: null, is_sampled: null, context: { ...context, id: "103" } },
];

async function securityHistory(page: Parameters<typeof maintenanceMock>[0]) {
  await maintenanceMock(page);
  await page.route("**/identity/v1/history/security?*", route => route.fulfill({ json: { items: securityRows, next_before: null } }));
  await page.goto("/api/admin/console/#activity/security");
}

test("security recording modes preserve representative and unknown context", async ({ page }) => {
  await securityHistory(page);
  const grid = page.getByRole("grid", { name: "Activity", exact: true });
  for (const label of ["Representative", "Individual", "Unknown", "Authentication", "Authorization"])
    await expect(grid.getByText(label, { exact: true })).toBeVisible();
  await grid.getByRole("button", { name: "Authentication failed", exact: true }).click();
  const details = page.getByRole("dialog", { name: "Event details" });
  await expect(details).toContainText("identity.session.login");
  await expect(details).toContainText("Password");
  await expect(details.getByRole("alert")).toContainText("Other anonymous attempts may not be stored");
  await expect(details).toContainText("Anonymous");
  await page.keyboard.press("Escape");
  await grid.getByRole("row").filter({ hasText: "Unknown" }).getByRole("button", { name: "Permission denied", exact: true }).click();
  await expect(details.getByText("Unknown", { exact: true })).toBeVisible();
  await expect(details.getByText("Not recorded", { exact: true }).filter({ visible: true })).toHaveCount(3);
  await expect(details.getByRole("alert")).toHaveCount(0);
});

for (const width of [320, 1440]) for (const theme of ["light", "dark"]) {
  test(`security context view and detail ${width}px ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 });
    await securityHistory(page);
    await page.getByRole("button", { name: "Theme", exact: true }).click();
    await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
    const grid = page.getByRole("grid", { name: "Activity", exact: true });
    await expect(grid.getByText("Representative", { exact: true })).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: `test-results/security-context-${width}-${theme}.png`, animations: "disabled" });
    await grid.getByRole("button", { name: "Authentication failed", exact: true }).click();
    const details = page.getByRole("dialog", { name: "Event details" });
    await expect(details).toContainText("Password");
    expect(await details.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    await details.screenshot({ path: `test-results/security-context-detail-${width}-${theme}.png`, animations: "disabled" });
  });
}

test("operation ID matches the backend history value and can be copied", async ({ page, context: browserContext }) => {
  await maintenanceMock(page);
  await browserContext.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/api/admin/console/#activity");
  await page.getByRole("button", { name: "Create storage", exact: true }).click();
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
  await page.getByRole("button", { name: "Create storage", exact: true }).click();
  await page.getByRole("button", { name: "Copy operation ID", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Copy failed.");
  await expect(page.getByRole("status")).not.toContainText("copied");
});

test("audit shows its resource without inventing command duration or outcome", async ({ page }) => {
  await maintenanceMock(page);
  await page.goto("/api/admin/console/#activity");
  await page.getByRole("button", { name: "Create storage", exact: true }).click();
  const details = page.getByRole("dialog", { name: "Event details" });
  await expect(details).toContainText(audit.resource_id);
  await expect(details.locator("dt").filter({ hasText: /^Duration$/ })).toHaveCount(0);
  await expect(details.locator("dt").filter({ hasText: /^Outcome$/ })).toHaveCount(0);
  await expect(details.getByText("Metadata", { exact: true })).toBeVisible();
});

test("technical identifiers stay collapsed while the summary and correlation remain visible", async ({ page }) => {
  await commandHistory(page);
  await page.getByRole("button", { name: "Test storage", exact: true }).click();
  const details = page.getByRole("dialog", { name: "Event details" });
  await expect(details.getByText("12 ms", { exact: true })).toBeVisible();
  await expect(details.getByText("Session ID", { exact: true })).toBeHidden();
  await expect(details.getByText("Event code", { exact: true })).toBeHidden();
  await expect(details.getByLabel("Operation ID", { exact: true })).toBeVisible();
  await expect(details.getByRole("link", { name: "View related history", exact: true })).toBeVisible();
  await details.getByRole("button", { name: "Technical details", exact: true }).click();
  await expect(details.getByText("Session ID", { exact: true })).toBeVisible();
  await expect(details.getByText("storage.test", { exact: true })).toBeVisible();
});

for (const width of [320, 390]) test(`mobile command outcomes include elapsed time at ${width}px`, async ({ page }) => {
  await page.setViewportSize({ width, height: 844 });
  await commandHistory(page);
  const grid = page.getByRole("grid", { name: "Activity", exact: true });
  await expect(grid.getByText("1.2 s", { exact: true })).toBeInViewport();
  await expect(grid.getByText("Unknown", { exact: true })).toBeInViewport();
  await expect(grid.getByText("750 ms", { exact: true })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
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
  const operation = grid.getByRole("button", { name: "Test storage", exact: true });
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

test("audit correlation follows cursor pages beyond the initial viewport", async ({ page }) => {
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
    await grid.getByRole("button", { name: "Delete storage", exact: true }).click();
    const details = page.getByRole("dialog", { name: "Event details" });
    await expect(details.getByLabel("Operation ID", { exact: true })).toHaveValue(context.request_id);
    await expect(details.getByText("Unknown", { exact: true })).toBeVisible();
    expect(await details.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    await details.screenshot({ path: `test-results/activity-detail-${width}-${theme}.png`, animations: "disabled" });
  });
}
