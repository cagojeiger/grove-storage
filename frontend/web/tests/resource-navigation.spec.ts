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
