import { expect, test } from "@playwright/test";
import { storageMock, root } from "./storage-fixture";
import { commandUrl, envelope, failure, intercept, session } from "./command-fixture";

test("cached detail cannot start a probe while its saved settings are refreshing", async ({ page }) => {
  await storageMock(page);
  await intercept(page, "storage.test", (route) => route.fulfill({ json: envelope("storage.test", { id: "home-archive", state: "ok" }) }));
  await page.goto(`${root}/home-archive`);
  await expect(page.getByRole("button", { name: "Test connection", exact: true })).toBeEnabled();
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Storage", exact: true }).click();
  let release: () => void = () => {};
  const held = new Promise<void>((resolve) => { release = resolve; });
  await intercept(page, "storage.show", async (route) => { await held; await route.fallback(); });
  await page.getByRole("link", { name: /home-archive/ }).click();
  await expect(page.getByRole("button", { name: "Test connection", exact: true })).toBeDisabled();
  release();
  await page.getByRole("button", { name: "Test connection", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Bucket access verified");
});

test("Reader explicitly tests saved storage, with no retries or persistence", async ({ page }) => {
  const { writes } = await storageMock(page);
  await page.route("**/api/admin/identity/v1/session", (r) => r.fulfill({ json: { ...session, role: "reader" } }));
  let calls = 0;
  await page.route(commandUrl, async (route) => {
    const { command, input } = route.request().postDataJSON() as { command: string; input: { id: string } };
    if (command !== "storage.test") return route.fallback();
    calls++;
    expect(input).toEqual({ id: "home-archive" });
    await route.fulfill({ json: envelope(command, { id: input.id, state: "ok" }) });
  });
  await page.goto(`${root}/home-archive`);
  await expect(page.getByRole("button", { name: "Test connection", exact: true })).toBeVisible();
  expect(calls).toBe(0);
  await page.getByRole("button", { name: "Test connection", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Bucket access verified");
  await expect(page.getByRole("status")).toContainText("Upload, download and public URL not tested");
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByRole("region", { name: "Storage connection" })).toContainText("Not checked");
  expect(calls).toBe(1);
  expect(writes).toEqual([]);
  expect(await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }))).not.toContain("state");
});

for (const outcome of ["failed", "changed", "malformed", "unauthorized"]) {
  test(`connection ${outcome} never shows success or retries`, async ({ page }) => {
    await storageMock(page);
    let calls = 0;
    await page.route(commandUrl, async (route) => {
      const { command } = route.request().postDataJSON() as { command: string };
      if (command !== "storage.test") return route.fallback();
      calls++;
      const status = outcome === "failed" ? 503 : outcome === "changed" ? 409 : outcome === "unauthorized" ? 401 : 200;
      await route.fulfill({ status, json: status === 200 ? envelope(command, { id: "another-storage", state: "ok" }) : failure(status) });
    });
    await page.goto(`${root}/home-archive`);
    await page.getByRole("button", { name: "Test connection", exact: true }).click();
    if (outcome === "unauthorized") await expect(page.getByRole("heading", { name: "Sign in", exact: true })).toBeVisible();
    else await expect(page.getByRole("alert")).toBeVisible();
    await expect(page.getByText("Bucket access verified", { exact: false })).toHaveCount(0);
    expect(calls).toBe(1);
  });
}

test("pending test blocks duplicate clicks and clears the previous success", async ({ page }) => {
  await storageMock(page);
  let release: () => void = () => {};
  const held = new Promise<void>((resolve) => { release = resolve; });
  let calls = 0;
  await page.route(commandUrl, async (route) => {
    const { command, input } = route.request().postDataJSON() as { command: string; input: { id: string } };
    if (command !== "storage.test") return route.fallback();
    calls++;
    if (calls === 2) await held;
    await route.fulfill({ json: envelope(command, { id: input.id, state: "ok" }) });
  });
  await page.goto(`${root}/home-archive`);
  await page.getByRole("button", { name: "Test connection", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Bucket access verified");
  await page.getByRole("button", { name: "Test connection", exact: true }).click();
  await expect(page.getByRole("button", { name: "Testing...", exact: true })).toBeDisabled();
  await expect(page.getByRole("status")).toHaveText("Checking bucket access...");
  release();
  await expect(page.getByRole("status")).toContainText("Bucket access verified");
  expect(calls).toBe(2);
});
