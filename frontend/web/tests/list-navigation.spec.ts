import { expect, test } from "@playwright/test";
import { accessMock, owner } from "./access-fixture";
import { audit, context, maintenanceMock } from "./maintenance-fixture";
import { example, storageMock } from "./storage-fixture";

const base = "/api/admin/console/";

test("account search follows browser back and forward", async ({ page }) => {
  await accessMock(page);
  await page.goto(`${base}#accounts`);
  const search = page.getByRole("searchbox", { name: "Search accounts" });
  await search.fill("absent");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.getByText("No matching accounts.")).toBeVisible();
  await page.goBack();
  await expect(search).toHaveValue("");
  await expect(
    page.getByRole("link", { name: owner.display_name, exact: true }),
  ).toBeVisible();
  await page.goForward();
  await expect(search).toHaveValue("absent");
  await expect(page.getByText("No matching accounts.")).toBeVisible();
});

test("activity uses one cursor pagination control and keeps filters", async ({
  page,
}) => {
  await maintenanceMock(page);
  const requests: URL[] = [];
  await page.route("**/identity/v1/history/audit?*", (route) => {
    const url = new URL(route.request().url());
    requests.push(url);
    const second = Boolean(url.searchParams.get("before"));
    return route.fulfill({
      json: {
        items: Array.from({ length: second ? 1 : 50 }, (_, index) => ({
          ...audit,
          action: second ? "client.create" : `storage.create.${index}`,
          context: { ...context, id: second ? "1" : String(100 - index) },
        })),
        next_before: second ? null : "51",
      },
    });
  });
  await page.goto(`${base}#activity?account_id=${owner.id}`);
  const next = page.getByRole("button", { name: "Go to next page" });
  const previous = page.getByRole("button", { name: "Go to previous page" });
  await expect(
    page.getByRole("button", { name: "storage.create.0", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Load more" })).toHaveCount(0);
  await expect(previous).toBeDisabled();
  const initialRequests = requests.length;
  await next.click();
  await expect(
    page.getByRole("button", { name: "client.create", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "storage.create.0", exact: true }),
  ).toHaveCount(0);
  await expect(next).toBeDisabled();
  await expect(page.getByText("51–51 of 51", { exact: true })).toBeVisible();
  await previous.click();
  await expect(
    page.getByRole("button", { name: "storage.create.0", exact: true }),
  ).toBeVisible();
  await expect(next).toBeEnabled();
  expect(
    requests
      .slice(initialRequests)
      .map((url) => url.searchParams.get("before")),
  ).toEqual(["51", null]);
  expect(
    requests.every(
      (url) =>
        url.searchParams.get("limit") === "50" &&
        url.searchParams.get("account_id") === owner.id,
    ),
  ).toBe(true);
});

test("activity next-page failure retries the same cursor", async ({ page }) => {
  await maintenanceMock(page);
  let failed = false;
  const cursors: (string | null)[] = [];
  await page.route("**/identity/v1/history/audit?*", (route) => {
    const cursor = new URL(route.request().url()).searchParams.get("before");
    cursors.push(cursor);
    if (cursor && !failed) {
      failed = true;
      return route.fulfill({ status: 503, json: { error: "unavailable" } });
    }
    return route.fulfill({
      json: {
        items: [
          {
            ...audit,
            action: cursor ? "client.create" : "storage.create",
            context: { ...context, id: cursor ? "1" : context.id },
          },
        ],
        next_before: cursor ? null : context.id,
      },
    });
  });
  await page.goto(`${base}#activity`);
  await page.getByRole("button", { name: "Go to next page" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "storage.create", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "client.create", exact: true }),
  ).toBeVisible();
  expect(cursors[0]).toBeNull();
  expect(cursors.filter((cursor) => cursor !== null)).toEqual([
    context.id,
    context.id,
  ]);
});

for (const width of [390, 1280]) {
  for (const theme of ["light", "dark"]) {
    test(`small account list fits ${width}px ${theme}`, async ({ page }) => {
      await accessMock(page, [owner]);
      await page.setViewportSize({ width, height: 720 });
      await page.goto(`${base}#accounts`);
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      await expect(
        page.getByRole("link", { name: owner.display_name, exact: true }),
      ).toBeVisible();
      const grid = await page
        .locator(".MuiDataGrid-root")
        .boundingBox();
      const pagination = await page
        .getByRole("navigation", { name: "Account pagination" })
        .boundingBox();
      expect(grid!.height).toBeGreaterThanOrEqual(200);
      expect(pagination!.y - (grid!.y + grid!.height)).toBeCloseTo(16, 0);
      expect(pagination!.y + pagination!.height).toBeLessThanOrEqual(720);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: `../../output/console-list-ux-20261002/accounts-${width}-${theme}.png`,
      });
    });
  }
}

test("storage detail has one path and one title", async ({ page }) => {
  await storageMock(page);
  await page.goto(`${base}#storages/${example.id}`);
  const path = page.getByRole("navigation", { name: "Page path" });
  await expect(path).toHaveCount(1);
  await expect(
    path.getByRole("link", { name: "Storage", exact: true }),
  ).toBeVisible();
  await expect(
    path.getByRole("heading", { name: example.id, exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("main").getByText(example.id, { exact: true }),
  ).toHaveCount(1);
  await expect(page.getByRole("banner").getByRole("navigation")).toHaveCount(0);
  await path.getByRole("link", { name: "Storage", exact: true }).click();
  await expect(
    page.getByRole("grid", { name: "Storage", exact: true }),
  ).toBeVisible();
  const grid = await page
    .locator(".MuiDataGrid-root")
    .boundingBox();
  expect(grid!.height).toBeGreaterThanOrEqual(200);
  const footer = await page.locator("main footer").boundingBox();
  expect(footer!.y - (grid!.y + grid!.height)).toBeCloseTo(16, 0);
});
