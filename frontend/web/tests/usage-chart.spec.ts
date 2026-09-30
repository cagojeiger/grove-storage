import { expect, test } from "@playwright/test";
import { resolve } from "node:path";
import {
  dailyUsage,
  Snapshot,
} from "../src/features/overview/usageHistoryModel";
import { envelope, intercept, session } from "./command-fixture";

const row: Snapshot = {
  day: "2026-09-24",
  storage_id: "a",
  client_id: "client",
  active_files: 2,
  active_bytes: 1024,
};

test("daily snapshots aggregate all locations with UTC gaps instead of invented zeroes", () => {
  expect(dailyUsage([])).toEqual([]);
  expect(
    dailyUsage([
      row,
      { ...row, storage_id: "b", active_files: 3 },
      { ...row, day: "2026-09-26", active_files: 0, active_bytes: 0 },
    ]),
  ).toEqual([
    { day: "2026-09-24", files: 5, bytes: 2048 },
    { day: "2026-09-25", files: null, bytes: null },
    { day: "2026-09-26", files: 0, bytes: 0 },
  ]);
  expect(
    dailyUsage([
      { ...row, day: "2024-02-28" },
      { ...row, day: "2024-03-01" },
    ]).map((r) => r.day),
  ).toEqual(["2024-02-28", "2024-02-29", "2024-03-01"]);
});

test("a single observation stays visible and missing days do not imply transfers", async ({
  page,
}) => {
  let rows = [row];
  await page.route("**/identity/v1/session", (route) =>
    route.fulfill({ json: session }),
  );
  await intercept(page, "usage.history", (route) =>
    route.fulfill({ json: envelope("usage.history", rows) }),
  );
  await page.goto("/api/admin/console/#usage");
  const chart = page.getByLabel("Active data by day", { exact: true });
  await expect(chart.locator(".MuiLineChart-mark")).toHaveCount(1);
  await expect(chart.locator(".MuiLineChart-mark")).toBeVisible();
  rows = [row, { ...row, day: "2026-09-26" }];
  await page.getByRole("button", { name: "Refresh usage history" }).click();
  await expect(chart.locator(".MuiLineChart-mark")).toHaveCount(2);
  await expect
    .poll(
      async () =>
        (await chart.locator(".MuiLineChart-line").getAttribute("d"))?.match(
          /M/g,
        )?.length,
    )
    .toBe(2);
  await page.getByRole("tab", { name: "Active data", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("tab", { name: "Active files", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
});

for (const width of [320, 768, 1440])
  for (const mode of ["light", "dark"]) {
    test(`MUI chart and pagination ${width}px ${mode}`, async ({ page }) => {
      const fonts: string[] = [];
      const errors: string[] = [];
      page.on("request", (request) => {
        if (request.resourceType() === "font") fonts.push(request.url());
      });
      page.on("pageerror", (error) => errors.push(error.message));
      await page.route("**/identity/v1/session", (r) =>
        r.fulfill({ json: { ...session, role: "reader" } }),
      );
      await intercept(page, "usage.history", (r) =>
        r.fulfill({
          json: envelope("usage.history", [
            row,
            { ...row, day: "2026-09-25", active_files: 4, active_bytes: 2048 },
            { ...row, day: "2026-09-26", active_files: 3, active_bytes: 1536 },
            ...Array.from({ length: 98 }, (_, index) => ({
              ...row,
              client_id: `client-${index}`,
            })),
          ]),
        }),
      );
      await page.setViewportSize({ width, height: 900 });
      await page.goto("/api/admin/console/#usage");
      await page.getByLabel("Theme").selectOption(mode);
      const chart = page.getByLabel("Active data by day", { exact: true });
      await expect(chart.locator("svg")).toBeVisible();
      await expect(chart.locator(".MuiLineChart-line")).toHaveAttribute(
        "d",
        /^M/,
      );
      await expect(chart.locator("text").first()).toHaveCSS(
        "font-family",
        /^-apple-system,/,
      );
      await page
        .getByRole("tab", { name: "Active files", exact: true })
        .click();
      await expect(
        page
          .getByLabel("Active files by day", { exact: true })
          .locator(".MuiLineChart-line"),
      ).toHaveAttribute("d", /^M/);
      const next = page.getByRole("button", { name: "Go to next page" });
      await next.scrollIntoViewIfNeeded();
      const bounds = await next.boundingBox();
      expect(bounds).not.toBeNull();
      expect(bounds!.x).toBeGreaterThanOrEqual(0);
      expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
      await next.click();
      await expect(
        page.getByText("51–100 of 101", { exact: true }),
      ).toBeVisible();
      await next.click();
      await expect(
        page.getByText("101–101 of 101", { exact: true }),
      ).toBeVisible();
      await expect(next).toBeDisabled();
      await expect(page.getByRole("table").locator("tbody tr")).toHaveCount(1);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      expect(errors).toEqual([]);
      expect(fonts).toEqual([]);
      await page.screenshot({
        path: resolve(
          `../../output/console-overview-rebuild-20261001/usage-${width}-${mode}.png`,
        ),
        fullPage: true,
        animations: "disabled",
      });
    });
  }
