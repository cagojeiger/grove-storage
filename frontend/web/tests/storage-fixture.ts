import { Page } from "@playwright/test";
import { Storage } from "../src/features/storages/model";
import { commandUrl, envelope, failure, session } from "./command-fixture";

export const root = "/api/admin/console/#storages";
export const example: Storage = {
  id: "home-archive",
  kind: "s3",
  endpoint: "https://s3.home.example.com",
  public_endpoint: "https://files.example.com",
  region: "us-east-1",
  bucket: "archive",
  access_key: "fixture-access-key",
  force_path_style: true,
  force_relay: false,
  root_path: null,
  capacity_bytes: 1024 ** 4,
};

export async function storageMock(page: Page, initial = [example]) {
  const rows = new Map(initial.map((row) => [row.id, row]));
  const writes: { command: string; input: Record<string, unknown> }[] = [];
  const reads = { list: 0 };
  await page.route("**/readyz", (route) =>
    route.fulfill({ json: { status: "ready" } }),
  );
  await page.route("**/api/admin/identity/v1/session", (route) => route.fulfill({ json: session }));
  await page.route(commandUrl, async (route) => {
    const req = route.request();
    const { command, input } = req.postDataJSON() as { command: string; input: { id?: string; spec?: Record<string, unknown> } };
    if (command === "usage.storages") {
      await route.fulfill({
        json: envelope(command, [...rows.values()].map((row) => ({
          storage_id: row.id,
          kind: row.kind,
          capacity_bytes: row.capacity_bytes,
          active_bytes: 0,
          reserved_bytes: 0,
          purge_pending_bytes: 0,
          active_files: 0,
          reserved_files: 0,
          purge_pending_files: 0,
          remaining_bytes: row.capacity_bytes,
        }))),
      });
      return;
    }
    if (command === "client.list") {
      await route.fulfill({ json: envelope(command, []) });
      return;
    }
    const id = input.id ?? "";
    if (command === "storage.metadata.show") {
      await route.fulfill({ json: envelope(command, { id, metadata: {} }) });
      return;
    }
    if (["storage.list", "storage.show"].includes(command)) {
      if (!id) reads.list++;
      await route.fulfill({
        status: id && !rows.has(id) ? 404 : 200,
        json: id && !rows.has(id) ? failure(404) : envelope(command, id ? rows.get(id) : [...rows.values()]),
      });
      return;
    }
    writes.push({ command, input });
    if (command === "storage.delete") {
      rows.delete(id);
      await route.fulfill({ json: envelope(command, { resource: "storage", id }) });
      return;
    }
    const saved = {
      ...example,
      ...input.spec,
      id,
    } as Storage & { secret_key?: string };
    delete saved.secret_key;
    rows.set(saved.id, saved);
    await route.fulfill({
      json: envelope(command, saved),
    });
  });
  return { rows, writes, reads };
}

export async function fillS3(page: Page, id = "new-s3") {
  await page.getByLabel(/^Storage ID\s*\*?$/).fill(id);
  await page
    .getByLabel(/^Endpoint\s*\*?$/)
    .fill("https://s3.example.com");
  await page
    .getByLabel("Public endpoint (optional)")
    .fill("https://public.example.com");
  await page.getByLabel(/^Region\s*\*?$/).fill("ap-northeast-2");
  await page.getByLabel(/^Bucket\s*\*?$/).fill("files");
  await page.getByLabel(/^Access key\s*\*?$/).fill("test-access");
  await page
    .getByLabel(/^Secret key\s*\*?$/)
    .fill("ephemeral-provider-secret");
  await page.getByLabel(/^Path-style\s*\*?$/).check();
  await page.getByLabel(/^Use relay\s*\*?$/).check();
  await page.getByLabel(/^Registered capacity\s*\*?$/).fill("1.5");
  await page.getByLabel("Capacity unit").selectOption("TiB");
}
