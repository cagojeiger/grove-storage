import { Page } from "@playwright/test";
import { Storage } from "../src/features/storages/model";

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
  const writes: { method: string; body: Record<string, unknown> | null }[] = [];
  const reads = { list: 0 };
  await page.route("**/readyz", (route) =>
    route.fulfill({ json: { status: "ready" } }),
  );
  await page.route("**/api/admin/v1/**", async (route) => {
    const req = route.request();
    const path = new URL(req.url()).pathname;
    if (path.endsWith("/session")) {
      await route.fulfill({
        json: { principal: "admin", credential_id: "test" },
      });
      return;
    }
    if (path.endsWith("/usage")) {
      await route.fulfill({
        json: [...rows.values()].map((row) => ({
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
        })),
      });
      return;
    }
    if (path.endsWith("/clients")) {
      await route.fulfill({ json: [] });
      return;
    }
    const suffix = path.split("/storages")[1];
    const id = suffix ? decodeURIComponent(suffix.slice(1)) : "";
    if (req.method() === "GET") {
      if (!id) reads.list++;
      await route.fulfill({
        status: id && !rows.has(id) ? 404 : 200,
        json: id ? (rows.get(id) ?? {}) : [...rows.values()],
      });
      return;
    }
    const body = req.postDataJSON() as Record<string, unknown> | null;
    writes.push({ method: req.method(), body });
    if (req.method() === "DELETE") {
      rows.delete(id);
      await route.fulfill({ status: 204 });
      return;
    }
    const saved = {
      ...example,
      ...body,
      id: id || String(body?.id),
    } as Storage & { secret_key?: string };
    delete saved.secret_key;
    rows.set(saved.id, saved);
    await route.fulfill({
      status: req.method() === "POST" ? 201 : 200,
      json: saved,
    });
  });
  return { rows, writes, reads };
}

export async function fillS3(page: Page, id = "new-s3") {
  await page.getByLabel("저장소 ID", { exact: true }).fill(id);
  await page
    .getByLabel("Endpoint", { exact: true })
    .fill("https://s3.example.com");
  await page
    .getByLabel("Public endpoint (선택)")
    .fill("https://public.example.com");
  await page.getByLabel("리전", { exact: true }).fill("ap-northeast-2");
  await page.getByLabel("버킷", { exact: true }).fill("files");
  await page.getByLabel("Access key", { exact: true }).fill("test-access");
  await page
    .getByLabel("Secret key", { exact: true })
    .fill("ephemeral-provider-secret");
  await page.getByLabel("Path-style", { exact: true }).check();
  await page.getByLabel("릴레이 사용", { exact: true }).check();
  await page.getByLabel("등록 용량", { exact: true }).fill("1.5");
  await page.getByLabel("용량 단위").selectOption("TiB");
}
