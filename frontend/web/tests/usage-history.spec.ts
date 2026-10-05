import { expect, Page, test } from "@playwright/test";
import {
  commandUrl,
  envelope,
  failure,
  intercept,
  session,
} from "./command-fixture";

const snapshot = {
  day: "2026-09-24",
  storage_id: "retired-storage",
  client_id: "notegate",
  active_files: 1240,
  active_bytes: 1024 ** 3,
};
async function mock(page: Page, rows: unknown = [snapshot]) {
  await page.route("**/identity/v1/session", (r) =>
    r.fulfill({ json: { ...session, role: "reader" } }),
  );
  const inputs: { days: number }[] = [];
  await page.route(commandUrl, (r) => {
    const body = r.request().postDataJSON() as {
      command: string;
      input: { days: number };
    };
    inputs.push(body.input);
    return r.fulfill({ json: envelope(body.command, rows) });
  });
  return inputs;
}

test("Reader can load usage snapshots, including retired resource IDs, and change days", async ({
  page,
}) => {
  const inputs = await mock(page, [
    snapshot,
    { ...snapshot, day: "2026-09-25", active_bytes: 2 * 1024 ** 3 },
  ]);
  await page.goto("/api/admin/console/#usage");
  await expect(
    page.getByRole("gridcell", { name: "1,240", exact: true }),
  ).toHaveCount(2);
  await expect(page.getByRole("row").nth(1)).toContainText("2026-09-25");
  await expect(page.getByRole("row").nth(2)).toContainText("retired-storage");
  expect(inputs.every((i) => i.days === 90)).toBe(true);
  await page.getByLabel(/^Days\s*\*?$/).fill("7");
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect.poll(() => inputs.at(-1)?.days).toBe(7);
  const count = inputs.length;
  for (const value of ["0", "3651", "1.5"]) {
    await page.getByLabel(/^Days\s*\*?$/).fill(value);
    await page.getByRole("button", { name: "Apply", exact: true }).click();
    expect(inputs.length).toBe(count);
  }
  await page.getByLabel(/^Days\s*\*?$/).fill("3650");
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect.poll(() => inputs.at(-1)?.days).toBe(3650);
});

test("empty history is not fabricated as zero usage; failures hide stale rows and 401 clears the session", async ({
  page,
}) => {
  await mock(page, []);
  await page.goto("/api/admin/console/#usage");
  await expect(
    page.getByText("No usage snapshots recorded."),
  ).toBeVisible();
  await intercept(page, "usage.history", (r) =>
    r.fulfill({ json: envelope("usage.history", [snapshot]) }),
  );
  await page.getByRole("button", { name: "Refresh usage history" }).click();
  await expect(
    page.getByRole("gridcell", { name: snapshot.client_id }),
  ).toBeVisible();
  await intercept(page, "usage.history", (r) =>
    r.fulfill({ status: 503, json: failure(503) }),
  );
  await page.getByRole("button", { name: "Refresh usage history" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("grid")).toHaveCount(0);
  await intercept(page, "usage.history", (r) =>
    r.fulfill({ status: 401, json: failure(401) }),
  );
  await page.getByRole("button", { name: "Refresh usage history" }).click();
  await expect(page.getByLabel("Password")).toBeVisible();
});

for (const invalid of [
  { day: "2026-02-30" },
  { active_files: -1 },
  { active_bytes: 0.5 },
  { active_bytes: Number.MAX_SAFE_INTEGER + 1 },
]) {
  test(`malformed usage is rejected: ${JSON.stringify(invalid)}`, async ({
    page,
  }) => {
    await mock(page, [{ ...snapshot, ...invalid }]);
    await page.goto("/api/admin/console/#usage");
    await expect(page.getByRole("alert")).toBeVisible();
    await expect(page.getByRole("grid")).toHaveCount(0);
  });
}

test("large histories use bounded local pages without extra API requests", async ({
  page,
}) => {
  const inputs = await mock(
    page,
    Array.from({ length: 101 }, (_, i) => ({
      ...snapshot,
      client_id: `client-${i}`,
    })),
  );
  await page.goto("/api/admin/console/#usage");
  await page.getByRole("combobox", { name: /Rows per page/ }).click();
  await page.getByRole("option", { name: "50", exact: true }).click();
  await expect(page.getByText("1–50 of 101", { exact: true })).toBeVisible();
  expect(await page.getByRole("row").count()).toBeLessThanOrEqual(51);
  const count = inputs.length;
  await page.getByRole("button", { name: "Go to next page" }).click();
  await expect(page.getByText("51–100 of 101", { exact: true })).toBeVisible();
  expect(await page.getByRole("row").count()).toBeLessThanOrEqual(51);
  await page.getByRole("button", { name: "Go to next page" }).click();
  await expect(page.getByRole("row")).toHaveCount(2);
  expect(inputs.length).toBe(count);
});

for (const width of [320, 768, 1440])
  for (const theme of ["light", "dark"]) {
    test(`usage history ${width}px ${theme}`, async ({ page }) => {
      await mock(page);
      await page.setViewportSize({ width, height: 960 });
      await page.goto("/api/admin/console/#usage");
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      if (width < 900) {
        await page.getByRole("grid", { name: "Daily snapshots" }).locator(".MuiDataGrid-virtualScroller").evaluate(element => { element.scrollLeft = 300; });
      }
      await expect(
        page.getByRole("gridcell", { name: snapshot.client_id }),
      ).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: `test-results/usage-history-${width}-${theme}.png`,
        fullPage: true,
      });
    });
  }
