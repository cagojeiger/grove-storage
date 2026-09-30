import { expect, test } from "@playwright/test";
import { clientMock } from "./client-fixture";
import { storageMock } from "./storage-fixture";
import { envelope, intercept } from "./command-fixture";
import { parseMetadata } from "../src/features/metadata/model";

test("metadata parser enforces flat strings and normalized UTF-8 byte size", () => {
  expect(parseMetadata('{"description":"notes","owner":"team"}')).toEqual({ description: "notes", owner: "team" });
  for (const value of ["[]", "null", "{", '{"x":1}', '{"x":true}', '{"x":{}}', '{"x":[]}', '{"x":null}', '{"x":"\\u0000"}']) {
    expect(() => parseMetadata(value)).toThrow();
  }
  expect(() => parseMetadata(JSON.stringify({ x: "a".repeat(8183) }))).not.toThrow();
  expect(() => parseMetadata(JSON.stringify({ x: "a".repeat(8184) }))).toThrow();
  expect(() => parseMetadata(JSON.stringify({ x: "한".repeat(2728) }))).toThrow();
});

for (const resource of ["storage", "client"] as const) {
  test(`${resource} metadata replaces, reloads, clears, and never submits credentials`, async ({ page }) => {
    if (resource === "client") await clientMock(page);
    else await storageMock(page);
    const id = resource === "client" ? "notegate" : "home-archive";
    let metadata: Record<string, string> = {};
    const writes: object[] = [];
    await intercept(page, `${resource}.metadata.show`, (route) => route.fulfill({ json: envelope(`${resource}.metadata.show`, { id, metadata }) }));
    await intercept(page, `${resource}.metadata.replace`, async (route) => {
      const { input } = route.request().postDataJSON() as { input: { id: string; metadata: Record<string, string> } };
      writes.push(input);
      metadata = input.metadata;
      await route.fulfill({ json: envelope(`${resource}.metadata.replace`, { id, metadata }) });
    });
    await page.goto(`/api/admin/console/#${resource === "client" ? "clients" : "storages"}/${id}`);
    await expect(page.locator("dl > .resource-metadata")).toHaveCount(1);
    await expect(page.getByRole("heading", { name: "Metadata", exact: true })).toHaveCount(0);
    if (resource === "storage") {
      await expect(page.getByRole("region", { name: "Storage settings" }).locator(".resource-metadata")).toHaveCount(1);
    } else {
      const metadataBeforeKeys = await page.locator(".resource-metadata").evaluate((element) =>
        Boolean(element.compareDocumentPosition(document.querySelector(".service-keys")!) & Node.DOCUMENT_POSITION_FOLLOWING));
      expect(metadataBeforeKeys).toBe(true);
    }
    await page.getByRole("button", { name: "Edit metadata", exact: true }).click();
    await page.getByLabel("Metadata JSON").fill('{"description":42}');
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("string values");
    await expect(page.getByLabel("Metadata JSON")).toHaveAttribute("aria-invalid", "true");
    await expect(page.getByLabel("Metadata JSON")).toHaveAccessibleDescription(/string values/);
    expect(writes).toHaveLength(0);
    await page.getByLabel("Metadata JSON").fill('{"description":"Production files","environment":"home"}');
    await expect(page.getByLabel("Metadata JSON")).toHaveAttribute("aria-invalid", "false");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    expect(writes).toEqual([{ id, metadata: { description: "Production files", environment: "home" } }]);
    await page.reload();
    await expect(page.locator(".metadata-json")).toContainText("Production files");
    await page.getByRole("button", { name: "Edit metadata", exact: true }).click();
    await page.getByLabel("Metadata JSON").fill("{}");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.locator(".metadata-json")).toHaveText("{}");
  });
}

test("reader can view metadata without edit controls", async ({ page }) => {
  await clientMock(page, "reader");
  await page.goto("/api/admin/console/#clients/notegate");
  await expect(page.locator(".metadata-json")).toHaveText("{}");
  await expect(page.getByRole("button", { name: "Edit metadata" })).toHaveCount(0);
});

test("unknown write stops resubmission and close re-reads metadata", async ({ page }) => {
  await clientMock(page);
  let reads = 0, writes = 0;
  await intercept(page, "client.metadata.show", async (route) => {
    reads++;
    await route.fulfill({ json: envelope("client.metadata.show", { id: "notegate", metadata: writes ? { description: "applied" } : {} }) });
  });
  await intercept(page, "client.metadata.replace", async (route) => { writes++; await route.abort(); });
  await page.goto("/api/admin/console/#clients/notegate");
  await page.getByRole("button", { name: "Edit metadata" }).click();
  await page.getByLabel("Metadata JSON").fill('{"description":"applied"}');
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Close and review" }).click();
  await expect(page.locator(".metadata-json")).toContainText("applied");
  expect(writes).toBe(1);
  expect(reads).toBeGreaterThan(1);
});

for (const width of [390, 768, 1440]) {
  test(`JSON editor fits at ${width}px in light and dark mode`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await clientMock(page);
    await page.goto("/api/admin/console/#clients/notegate");
    for (const theme of ["Light", "Dark"]) {
      await page.getByLabel(/^Theme\s*\*?$/).selectOption({ label: theme });
      await page.getByRole("button", { name: "Edit metadata" }).click();
      await page.getByLabel("Metadata JSON").fill(JSON.stringify({ description: "a".repeat(500) }));
      const box = await page.getByLabel("Metadata JSON").boundingBox();
      expect(box!.x).toBeGreaterThanOrEqual(0);
      expect(box!.x + box!.width).toBeLessThanOrEqual(width);
      await expect(page.getByRole("button", { name: "Save", exact: true })).toBeInViewport();
      await page.screenshot({ path: `test-results/metadata-${width}-${theme}.png` });
      await page.getByRole("button", { name: "Cancel", exact: true }).click();
    }
  });
}
