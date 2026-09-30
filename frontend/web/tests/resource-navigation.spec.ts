import { expect, test } from "@playwright/test";
import { example, storageMock } from "./storage-fixture";
import { envelope, failure, intercept } from "./command-fixture";
import { paginate } from "../src/app/resourceList";

test("paging is sorted, bounded and clamps after the final page disappears", () => {
  const state = { search: "", sort: "asc", size: 20, page: 3 };
  const rows = Array.from({ length: 41 }, (_, i) => `item-${i + 1}`);
  expect(paginate(rows, state, (id) => id).rows).toEqual(["item-41"]);
  expect(paginate(rows.slice(0, 40), state, (id) => id).page).toBe(2);
  expect(paginate([], state, (id: string) => id)).toMatchObject({
    page: 1,
    pages: 1,
    total: 0,
  });
  expect(
    paginate(
      rows,
      { ...state, page: 1, sort: "desc", search: "item-4" },
      (id) => id,
    ).rows,
  ).toEqual(["item-41", "item-40", "item-4"]);
});

test("Storage keeps search, order and page across detail, reload and deletion", async ({
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
  await expect(page.getByRole("table", { name: "Storage", exact: true }).locator("tbody a")).toHaveCount(20);
  await page.getByRole("button", { name: "Last page", exact: true }).click();
  await page.getByRole("link", { name: /store-41/ }).click();
  await expect(
    page.getByRole("heading", { name: "store-41", exact: true }),
  ).toBeVisible();
  await page.reload();
  await page
    .locator("main")
    .getByRole("link", { name: "Storage", exact: true })
    .click();
  await expect(
    page.getByRole("status"),
  ).toHaveText("41–41 of 41");
  await page.getByRole("link", { name: /store-41/ }).click();
  await page
    .getByRole("button", { name: "Delete storage", exact: true })
    .click();
  await page.getByLabel("Storage ID to delete").fill("store-41");
  await page.getByRole("button", { name: "Confirm delete" }).click();
  await expect(
    page.getByRole("status"),
  ).toHaveText("21–40 of 40");
  await expect(page.getByRole("table", { name: "Storage", exact: true }).locator("tbody a")).toHaveCount(20);
  await page.getByLabel(/^Sort\s*\*?$/).selectOption("desc");
  await expect(page.getByRole("table", { name: "Storage", exact: true }).locator("tbody a").first()).toContainText(
    "store-40",
  );
  await page.getByLabel("Search storage").fill("store-3");
  await expect(page.getByRole("table", { name: "Storage", exact: true }).locator("tbody a")).toHaveCount(10);
  await page.getByRole("table", { name: "Storage", exact: true }).locator("tbody a").first().click();
  await page
    .locator("main")
    .getByRole("link", { name: "Storage", exact: true })
    .click();
  await expect(page.getByLabel("Search storage")).toHaveValue("store-3");
  await expect(page.getByLabel(/^Sort\s*\*?$/)).toHaveValue("desc");
  await page.getByLabel("Search storage").fill("");
  await page.getByLabel("Rows per page").selectOption("50");
  await expect(page.getByRole("table", { name: "Storage", exact: true }).locator("tbody a")).toHaveCount(40);
});

test("Client rows resolve assigned storage only for the visible page, including empty clients", async ({
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
  await expect(page.getByRole("table", { name: "Clients", exact: true }).locator("tbody a")).toHaveCount(20);
  await expect(page.getByRole("table", { name: "Clients", exact: true }).locator("tbody tr").first()).toContainText(
    "home-archive",
  );
  expect([...requested].every((id) => ids.slice(0, 20).includes(id))).toBe(
    true,
  );
  await page.getByRole("button", { name: "Next page" }).click();
  await expect(page.getByRole("table", { name: "Clients", exact: true }).locator("tbody a").first()).toContainText("client-21");
  await page.getByRole("table", { name: "Clients", exact: true }).locator("tbody a").first().click();
  await expect(page.getByText("S3 bucket", { exact: true })).toBeVisible();
  await page
    .locator("main")
    .getByRole("link", { name: "Clients", exact: true })
    .click();
  await expect(
    page.getByRole("status"),
  ).toHaveText("21–40 of 45");
  await page.getByLabel("Search clients").fill("missing");
  await expect(page.getByText("No matching clients.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Next page" })).toBeDisabled();
});

test("Overview highlights the registered assignment outside its first five storages", async ({
  page,
}) => {
  await storageMock(
    page,
    Array.from({ length: 8 }, (_, i) => ({ ...example, id: `storage-${i}` })),
  );
  await intercept(page, "client.list", (r) =>
    r.fulfill({ json: envelope("client.list", ["notegate"]) }),
  );
  await intercept(page, "usage.clients", (r) =>
    r.fulfill({ json: envelope("usage.clients", []) }),
  );
  await intercept(page, "client.show", (r) =>
    r.fulfill({
      json: envelope("client.show", {
        id: "notegate",
        storage_id: "storage-7",
      }),
    }),
  );
  await page.goto("/api/admin/console/#");
  await expect(page.locator(".connection-title")).toHaveCount(5);
  await page.getByRole("link", { name: "Open client notegate" }).focus();
  await expect(
    page.locator(".connection-item.selected .connection-title"),
  ).toContainText("storage-7");
  await expect(page.getByRole("status")).toContainText("notegate → storage-7");
  await expect(page.locator(".connection-title")).toHaveCount(5);
  await intercept(page, "client.show", (r) =>
    r.fulfill({ status: 404, json: failure(404) }),
  );
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByRole("status")).toHaveText(
    "Storage assignment unavailable",
  );
  await expect(
    page.locator(".connection-item.selected .connection-title"),
  ).toHaveCount(0);
});
