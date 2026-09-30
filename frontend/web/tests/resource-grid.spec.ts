import { expect, test } from "@playwright/test";
import { resolve } from "node:path";
import { example, storageMock } from "./storage-fixture";
import { envelope, intercept } from "./command-fixture";

test("Storage grid retains paging, sort and search across detail and deletion", async ({
  page,
}) => {
  await storageMock(
    page,
    Array.from({ length: 41 }, (_, i) => ({
      ...example,
      id: `store-${String(i + 1).padStart(2, "0")}`,
    })),
  );
  await page.goto("/api/admin/console/#storages");
  const grid = page.getByRole("grid", { name: "Storage", exact: true });
  await expect(
    grid.getByRole("link", { name: "store-01", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: /last page/i }).click();
  await grid.getByRole("link", { name: "store-41", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "store-41", exact: true }),
  ).toBeVisible();
  await page.reload();
  await page
    .locator("main")
    .getByRole("link", { name: "Storage", exact: true })
    .click();
  await expect(page.getByText("41–41 of 41", { exact: true })).toBeVisible();
  await grid.getByRole("link", { name: "store-41", exact: true }).click();
  await page
    .getByRole("button", { name: "Delete storage", exact: true })
    .click();
  await page.getByLabel("Storage ID to delete").fill("store-41");
  await page.getByRole("button", { name: "Confirm delete" }).click();
  await expect(page.getByText("21–40 of 40", { exact: true })).toBeVisible();
  await grid.getByRole("columnheader", { name: /^Storage/ }).click();
  await expect(
    grid.getByRole("link", { name: "store-40", exact: true }),
  ).toBeVisible();
  await expect(page).toHaveURL(/sort=desc/);
  await page.getByLabel("Search storage").fill("store-3");
  await expect(page.getByText("1–10 of 10", { exact: true })).toBeVisible();
  await grid.getByRole("link", { name: "store-39", exact: true }).click();
  await page
    .locator("main")
    .getByRole("link", { name: "Storage", exact: true })
    .click();
  await expect(page.getByLabel("Search storage")).toHaveValue("store-3");
  await expect(
    grid.getByRole("columnheader", { name: /^Storage/ }),
  ).toHaveAttribute("aria-sort", "descending");
  await page.getByLabel("Search storage").fill("missing");
  await expect(grid).toContainText("No matching storage.");
  await expect(page.getByRole("button", { name: /next page/i })).toBeDisabled();
});

test("Client grid resolves only the visible page and preserves return state", async ({
  page,
}) => {
  await storageMock(page);
  const ids = Array.from(
    { length: 45 },
    (_, i) => `client-${String(i + 1).padStart(2, "0")}`,
  );
  await intercept(page, "client.list", (r) =>
    r.fulfill({ json: envelope("client.list", ids) }),
  );
  await intercept(page, "usage.clients", (r) =>
    r.fulfill({ json: envelope("usage.clients", []) }),
  );
  const requested = new Set<string>();
  await intercept(page, "client.show", (r) => {
    const { input } = r.request().postDataJSON() as { input: { id: string } };
    requested.add(input.id);
    return r.fulfill({
      json: envelope("client.show", {
        id: input.id,
        storage_id: "home-archive",
      }),
    });
  });
  await page.goto("/api/admin/console/#clients");
  const grid = page.getByRole("grid", { name: "Clients", exact: true });
  await expect(
    grid.getByRole("row").filter({ hasText: "client-01" }),
  ).toContainText("home-archive");
  expect(requested.size).toBe(20);
  expect([...requested].every((id) => ids.slice(0, 20).includes(id))).toBe(
    true,
  );
  await page.getByRole("button", { name: /next page/i }).click();
  await grid.getByRole("link", { name: "client-21", exact: true }).click();
  await expect(page.getByText("S3 bucket", { exact: true })).toBeVisible();
  await page
    .locator("main")
    .getByRole("link", { name: "Clients", exact: true })
    .click();
  await expect(page.getByText("21–40 of 45", { exact: true })).toBeVisible();
  await page.getByRole("combobox", { name: "Rows per page:" }).click();
  await page.getByRole("option", { name: "50", exact: true }).click();
  await expect(page.getByText("1–45 of 45", { exact: true })).toBeVisible();
  await page.getByLabel("Search clients").fill("missing");
  await expect(grid).toContainText("No matching clients.");
});

for (const mode of ["light", "dark"]) {
  for (const width of [390, 1440]) {
    test(`new shell and grid use system fonts at ${width}px in ${mode}`, async ({
      page,
    }) => {
      await storageMock(page);
      const fonts: string[] = [];
      page.on("request", (request) => {
        if (request.resourceType() === "font") fonts.push(request.url());
      });
      await page.setViewportSize({ width, height: 900 });
      await page.goto("/api/admin/console/#storages");
      await page.getByLabel("Theme").selectOption(mode);
      const grid = page.getByRole("grid", { name: "Storage", exact: true });
      await expect(grid.getByRole("link", { name: example.id })).toBeVisible();
      for (const item of [
        grid,
        grid.getByRole("columnheader").first(),
        grid.getByRole("gridcell").first(),
        page.getByRole("button", { name: "Register", exact: true }),
      ]) {
        await expect(item).toHaveCSS("font-family", /^-apple-system,/);
      }
      expect(fonts).toEqual([]);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: resolve(
          "../../output/console-rebuild-20261001",
          `storage-${width}-${mode}.png`,
        ),
        fullPage: true,
      });
      if (width < 900)
        await page.getByRole("button", { name: "Open navigation" }).click();
      await expect(
        page.getByRole("navigation", { name: "Main navigation" }),
      ).toBeVisible();
      await page
        .getByRole("button", { name: "Account menu", exact: true })
        .click();
      await expect(
        page.getByRole("menuitem", { name: "My account", exact: true }),
      ).toBeVisible();
    });
  }
}
