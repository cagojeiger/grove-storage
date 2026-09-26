import { expect, test, Page } from "@playwright/test";
import {
  foldConnections,
  clientTotals,
  sumClients,
} from "../src/features/overview/connectionsModel";
import { example, storageMock } from "./storage-fixture";
import { envelope, failure, intercept } from "./command-fixture";

test("folding preserves every row once and pins selection at the six/seven boundary", () => {
  for (const count of [0, 1, 6, 7, 20, 100]) {
    const rows = Array.from({ length: count }, (_, i) => `client-${i}`);
    const folded = foldConnections(rows, (id) => id, rows.at(-1));
    expect(folded.visible).toHaveLength(count > 6 ? 5 : count);
    expect(folded.hidden).toHaveLength(count > 6 ? count - 5 : 0);
    expect(new Set([...folded.visible, ...folded.hidden]).size).toBe(count);
    if (count) expect(folded.visible).toContain(rows.at(-1));
    expect(
      foldConnections([...rows].reverse(), (id) => id, rows.at(-1)),
    ).toEqual(folded);
  }
  expect(foldConnections(["x"], (id) => id, "deleted")).toEqual({
    visible: ["x"],
    hidden: [],
  });
});

test("folded client usage sums all locations without inventing an assignment", () => {
  const totals = clientTotals([
    { client_id: "a", storage_id: "old", active_files: 3, active_bytes: 10 },
    { client_id: "a", storage_id: "new", active_files: 2, active_bytes: 20 },
    { client_id: "b", storage_id: "old", active_files: 7, active_bytes: 40 },
  ]);
  expect(sumClients(["a", "b", "empty"], totals)).toEqual({
    files: 12,
    bytes: 70,
  });
  expect(sumClients(["a"], undefined)).toBeUndefined();
  expect(sumClients(["empty"], totals)).toEqual({ files: 0, bytes: 0 });
});

async function fixture(page: Page, count: number) {
  await storageMock(
    page,
    Array.from({ length: count }, (_, i) => ({ ...example, id: `store-${i}` })),
  );
  const ids = Array.from({ length: count }, (_, i) => `client-${i}`);
  await intercept(page, "client.list", (r) =>
    r.fulfill({ json: envelope("client.list", ids) }),
  );
  await intercept(page, "usage.clients", (r) =>
    r.fulfill({
      json: envelope(
        "usage.clients",
        ids.map((id) => ({
          client_id: id,
          storage_id: "old-location",
          active_files: 2,
          active_bytes: 1024 ** 3,
        })),
      ),
    }),
  );
  const reads: string[] = [];
  await intercept(page, "client.show", (r) => {
    const { input } = r.request().postDataJSON() as { input: { id: string } };
    reads.push(input.id);
    return r.fulfill({
      json: envelope("client.show", {
        id: input.id,
        storage_id: `store-${count - 1}`,
      }),
    });
  });
  await page.goto("/api/admin/console/#");
  return reads;
}

for (const count of [0, 1, 6, 7]) {
  test(`Overview ${count} resources folds both sides only after six`, async ({
    page,
  }) => {
    await fixture(page, count);
    await expect(
      page.getByRole("heading", { name: "Connections", exact: true }),
    ).toBeVisible();
    await expect(page.locator('[data-side="client"]')).toHaveCount(
      count > 6 ? 6 : count,
    );
    await expect(page.locator('[data-side="storage"]')).toHaveCount(
      count > 6 ? 6 : count,
    );
    await expect(page.locator(".connection-paths path")).toHaveCount(
      (count > 6 ? 6 : count) * 2,
    );
    await expect(page.locator(".connection-more")).toHaveCount(
      count > 6 ? 2 : 0,
    );
    if (count > 6) {
      await expect(
        page.getByRole("button", { name: "Show 2 more clients" }),
      ).toContainText("4 files · 2 GiB");
      await page.getByRole("button", { name: "Show 2 more storage" }).click();
      const dialog = page.getByRole("dialog");
      await dialog.getByLabel("Search hidden storage").fill("store-6");
      await dialog
        .getByRole("button", { name: "Show storage store-6 on map" })
        .click();
      await expect(dialog).toHaveCount(0);
      await expect(
        page.getByRole("button", { name: "Select storage store-6" }),
      ).toHaveAttribute("aria-pressed", "true");
      await expect(page.locator(".connection-paths path.selected")).toHaveCount(
        1,
      );
    }
  });
}

test("hidden clients are searched, paged and grouped from bounded authoritative reads", async ({
  page,
}) => {
  const reads = await fixture(page, 45);
  await expect(
    page.getByRole("button", { name: "Show 40 more clients" }),
  ).toContainText("80 files · 40 GiB");
  expect(reads).toHaveLength(0);
  await page.getByRole("button", { name: "Show 40 more clients" }).click();
  const dialog = page.getByRole("dialog");
  await expect(
    dialog.getByRole("heading", { name: "store-44 (20)" }),
  ).toBeVisible();
  expect(new Set(reads).size).toBe(20);
  await expect(dialog.getByText("1–20 of 40")).toBeVisible();
  await dialog.getByRole("button", { name: "Next page" }).click();
  await expect(dialog.getByText("21–40 of 40")).toBeVisible();
  await dialog.getByLabel("Search hidden clients").fill("client-44");
  await expect(dialog.getByText("1–1 of 1")).toBeVisible();
  await dialog
    .getByRole("button", { name: "Show client client-44 on map" })
    .click();
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Select client client-44", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".connection-selection")).toHaveText(
    "client-44 → store-44",
  );
  await expect(page.locator(".connection-paths path.selected")).toHaveCount(2);
  await expect(page.locator(".connection-title")).toHaveCount(5);
  await expect(
    page.getByRole("button", { name: "Show 40 more clients" }),
  ).toContainText("80 files · 40 GiB");
  await page.getByRole("button", { name: "Show 40 more clients" }).click();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Show 40 more clients" }),
  ).toBeFocused();
});

test("refresh totals include hidden storage and assignment failures clear highlighted routes", async ({
  page,
}) => {
  await fixture(page, 8);
  await intercept(page, "usage.storages", (r) =>
    r.fulfill({
      json: envelope(
        "usage.storages",
        Array.from({ length: 8 }, (_, i) => ({
          storage_id: `store-${i}`,
          kind: "s3",
          capacity_bytes: 1024 ** 4,
          active_files: 2,
          active_bytes: 1024 ** 3,
          reserved_files: 0,
          purge_pending_files: 0,
          reserved_bytes: 0,
          purge_pending_bytes: 0,
          remaining_bytes: 1024 ** 4 - 1024 ** 3,
        })),
      ),
    }),
  );
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    page
      .locator(".metrics div")
      .filter({ has: page.getByText("Total files", { exact: true }) }),
  ).toContainText("16");
  await expect(
    page
      .locator(".metrics div")
      .filter({ has: page.getByText("Total stored", { exact: true }) }),
  ).toContainText("8 GiB");
  await page
    .getByRole("button", { name: "Select client client-0", exact: true })
    .click();
  await expect(page.locator(".connection-paths path.selected")).toHaveCount(2);
  await intercept(page, "client.show", (r) =>
    r.fulfill({ status: 404, json: failure(404) }),
  );
  await intercept(page, "usage.clients", (r) =>
    r.fulfill({ status: 500, json: failure(500) }),
  );
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.locator(".connection-selection")).toHaveText(
    "Storage assignment unavailable",
  );
  await expect(page.locator(".connection-paths path.selected")).toHaveCount(1);
  await expect(
    page.getByRole("button", { name: "Show 3 more clients" }),
  ).toContainText("Usage unavailable");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(page.locator(".connection-paths path").first()).toHaveCSS(
    "transition-duration",
    "0s",
  );
});
