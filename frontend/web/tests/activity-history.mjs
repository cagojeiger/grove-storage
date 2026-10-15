import assert from "node:assert/strict";
import { expect } from "@playwright/test";

export async function openInvocation(page, operationId) {
  const endpoint = "/api/admin/identity/v1/history/invocations";
  const grid = page.getByRole("grid", { name: "Activity", exact: true });
  const seen = new Set();
  let cursor = null;
  let navigate = () => page.getByRole("tab", { name: "Command history", exact: true }).click();
  for (;;) {
    const [response] = await Promise.all([
      page.waitForResponse(response => {
        const url = new URL(response.url());
        return url.pathname === endpoint && url.searchParams.get("before") === cursor;
      }),
      navigate(),
    ]);
    expect(response.ok()).toBe(true);
    const { items, next_before } = await response.json();
    const index = items.findIndex(row => row.context.request_id === operationId);
    if (index >= 0) {
      await grid.locator(`[role="row"][data-id="${items[index].context.id}"]`)
        .getByRole("button", { name: items[index].operation, exact: true }).click();
      await expect(page.getByLabel("Operation ID", { exact: true })).toHaveValue(operationId);
      return;
    }
    assert(next_before && !seen.has(next_before), "Invocation not found in cursor history");
    seen.add(next_before);
    cursor = next_before;
    navigate = () => page.getByRole("button", { name: "Go to next page", exact: true }).click();
  }
}
