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

for (const theme of ["light", "dark"]) {
  test(`folded Overview controls retain the shared font in ${theme}`, async ({
    page,
  }) => {
    await fixture(page, 7);
    await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
    const more = page.getByRole("link", { name: "View all clients (+2)" });
    await expect(more).toHaveCSS(
      "font-family",
      /^-apple-system,/,
    );
    await more.click();
    const row = page
      .getByRole("grid", { name: "Clients", exact: true })
      .getByRole("link", { name: "client-6", exact: true });
    await expect(row).toHaveCSS("font-family", /^-apple-system,/);
  });
}

for (const count of [0, 1, 6, 7]) {
  test(`Overview ${count} resources folds both sides only after six`, async ({
    page,
  }) => {
    await fixture(page, count);
    await expect(
      page.getByRole("heading", { name: "Connections", exact: true }),
    ).toBeVisible();
    await expect(page.getByLabel("Usage summary").getByRole("region")).toHaveCount(4);
    await expect(page.getByLabel("Usage summary").locator("section > p:first-child")).toHaveText([
      "Clients",
      "Storage",
      "Stored files",
      "Stored data",
    ]);
    await expect(
      page
        .getByRole("region", { name: "Storage", exact: true })
        .getByText(String(count), { exact: true }),
    ).toBeVisible();
    await expect(page.locator('[data-side="client"]')).toHaveCount(
      count > 6 ? 5 : count,
    );
    await expect(page.locator('[data-side="storage"]')).toHaveCount(
      count > 6 ? 5 : count,
    );
    await expect(page.locator(".connection-paths g")).toHaveCount(
      (count > 6 ? 5 : count) * 2,
    );
    await expect(
      page.getByRole("link", { name: /^View all .*\(\+\d+\)/ }),
    ).toHaveCount(count > 6 ? 2 : 0);
    if (count > 6) {
      await expect(
        page.getByRole("link", { name: "View all clients (+2)" }),
      ).toBeVisible();
      await page.getByRole("link", { name: "View all storage (+2)" }).click();
      await page.getByRole("searchbox", { name: "Search storage" }).fill("store-6");
      await page.getByRole("grid", { name: "Storage", exact: true }).getByRole("link", { name: "store-6", exact: true }).click();
      await expect(page.getByRole("heading", { name: "store-6", exact: true })).toBeVisible();
    }
  });
}

test("hidden clients remain searchable and paged with bounded authoritative reads", async ({ page }) => {
  const reads = await fixture(page, 45);
  await expect(page.getByRole("link", { name: "View all clients (+40)" })).toBeVisible();
  await expect.poll(() => new Set(reads).size).toBe(5);
  await page.getByRole("link", { name: "View all clients (+40)" }).click();
  await expect(page.getByText("1–20 of 45", { exact: true })).toBeVisible();
  await expect.poll(() => new Set(reads).size).toBe(20);
  await page.getByRole("button", { name: "Go to next page" }).click();
  await expect(page.getByText("21–40 of 45", { exact: true })).toBeVisible();
  await expect.poll(() => new Set(reads).size).toBe(40);
  await page.getByRole("searchbox", { name: "Search clients" }).fill("client-44");
  await expect(page.getByText("1–1 of 1", { exact: true })).toBeVisible();
  await expect.poll(() => new Set(reads).size).toBe(41);
  await page.getByRole("grid", { name: "Clients", exact: true }).getByRole("link", { name: "client-44", exact: true }).click();
  await expect(page.getByRole("heading", { name: "client-44", exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "Client details" })).toContainText("store-44");
  await page.goBack();
  await expect(page.getByRole("searchbox", { name: "Search clients" })).toHaveValue("client-44");
  expect(new Set(reads).size).toBe(41);
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
    page.getByRole("region", { name: "Stored files", exact: true }),
  ).toContainText("16");
  await expect(
    page.getByRole("region", { name: "Stored data", exact: true }),
  ).toContainText("8 GiB");
  await page
    .getByRole("link", { name: "Open client client-0", exact: true })
    .focus();
  await expect(page.locator(".connection-paths path.selected")).toHaveCount(2);
  await intercept(page, "client.show", (r) =>
    r.fulfill({ status: 404, json: failure(404) }),
  );
  await intercept(page, "usage.clients", (r) =>
    r.fulfill({ status: 500, json: failure(500) }),
  );
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    page.getByRole("status", { name: "Selected connection" }),
  ).toHaveText("Storage assignment unavailable");
  await expect(page.locator(".connection-paths path.selected")).toHaveCount(1);
  await expect(
    page.locator('[data-side="client"]').first(),
  ).toContainText("Usage unavailable");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(page.locator(".connection-paths animate")).toHaveCount(0);
});
