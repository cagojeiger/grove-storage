import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { expect } from "@playwright/test";

export async function storageChecks(
  page,
  { objects, otherObjects, endpoint, minio },
) {
  async function admin(method, path, body) {
    return page.evaluate(
      async ({ method, path, body }) => {
        const response = await fetch(`/api/admin/v1/${path}`, {
          method,
          headers: {
            "Content-Type": "application/json",
            "X-FileGate-CSRF": "1",
          },
          body: body ? JSON.stringify(body) : undefined,
        });
        return {
          status: response.status,
          body: response.status === 204 ? null : await response.json(),
        };
      },
      { method, path, body },
    );
  }
  async function createFs(id) {
    await page.getByRole("button", { name: "등록", exact: true }).click();
    await page.getByLabel("저장소 ID", { exact: true }).fill(id);
    await page.getByLabel("종류", { exact: true }).selectOption("fs");
    await page.getByLabel("루트 경로").fill(objects);
    await page.getByLabel("등록 용량", { exact: true }).fill("1");
    await page.getByLabel("용량 단위").selectOption("GiB");
    await page.getByRole("button", { name: "저장", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: id, exact: true }),
    ).toBeVisible();
  }
  async function deleteStorage(id) {
    await page.getByRole("button", { name: "저장소 삭제" }).click();
    await page.getByLabel("삭제할 저장소 ID").fill(id);
    await page.getByRole("button", { name: "삭제 확인" }).click();
  }
  async function list() {
    await page
      .getByRole("navigation")
      .getByRole("link", { name: "저장소", exact: true })
      .click();
  }
  await list();
  await createFs("console-live");
  await page.getByRole("button", { name: "저장소 수정" }).click();
  await page.getByLabel("등록 용량", { exact: true }).fill("2147483648");
  await page.getByRole("button", { name: "저장", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  assert.equal(
    (await admin("GET", "storages/console-live")).body.capacity_bytes,
    2147483648,
  );

  // A second actor adds a client after the page observed an empty storage.
  assert.equal(
    (
      await admin("POST", "clients", {
        id: "console-client",
        storage_id: "console-live",
      })
    ).status,
    201,
  );
  await deleteStorage("console-live");
  await expect(page.getByRole("alert")).toContainText("클라이언트 또는 파일");
  await page.getByRole("button", { name: "취소" }).click();
  const raw = "console-isolated-native-key";
  const key_hash = `sha256:${createHash("sha256").update(raw).digest("hex")}`;
  assert.equal(
    (await admin("POST", "clients/console-client/keys", { key_hash })).status,
    201,
  );
  const allocated = await fetch(`${endpoint}/api/v1/files`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${raw}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ declared_size: 0 }),
  });
  assert.equal(allocated.status, 201);
  await page.getByRole("button", { name: "저장소 수정" }).click();
  await page.getByLabel("루트 경로").fill(otherObjects);
  await page.getByRole("button", { name: "저장", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "주소를 변경할 수 없습니다",
  );
  assert.equal(
    (await admin("GET", "storages/console-live")).body.root_path,
    objects,
  );
  await page.getByRole("button", { name: "취소" }).click();
  await list();
  await createFs("console-removable");
  await deleteStorage("console-removable");
  await expect(
    page.getByRole("heading", { name: "저장소", exact: true }),
  ).toBeVisible();
  assert.equal((await admin("GET", "storages/console-removable")).status, 404);
  console.log(
    "PASS real filesystem UI create/replace/delete, concurrent client delete 409, pending-file address replace 409",
  );

  if (minio) {
    await page.getByRole("button", { name: "등록", exact: true }).click();
    await page.getByLabel("저장소 ID", { exact: true }).fill("console-minio");
    await page.getByLabel("Endpoint", { exact: true }).fill(minio.endpoint);
    await page.getByLabel("Public endpoint (선택)").fill(minio.endpoint);
    await page.getByLabel("리전", { exact: true }).fill(minio.region);
    await page.getByLabel("버킷", { exact: true }).fill(minio.bucket);
    await page.getByLabel("Access key", { exact: true }).fill(minio.access_key);
    await page.getByLabel("Secret key", { exact: true }).fill(minio.secret_key);
    await page.getByLabel("Path-style", { exact: true }).check();
    await page.getByLabel("릴레이 사용", { exact: true }).check();
    await page
      .getByLabel("등록 용량", { exact: true })
      .fill(String(minio.capacity_bytes));
    await page.getByRole("button", { name: "저장", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "console-minio", exact: true }),
    ).toBeVisible();
    const saved = (await admin("GET", "storages/console-minio")).body;
    assert.equal(saved.force_relay, true);
    assert.equal(saved.force_path_style, true);
    assert.equal(saved.public_endpoint, minio.endpoint);
    assert(!JSON.stringify(saved).includes(minio.secret_key));
    await page.getByRole("button", { name: "저장소 수정" }).click();
    await expect(page.getByLabel("Secret key (재입력)")).toHaveValue("");
    await page.getByLabel("Secret key (재입력)").fill(minio.secret_key);
    await page.getByLabel("등록 용량", { exact: true }).fill("2147483648");
    await page.getByRole("button", { name: "저장", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    assert.equal(
      (await admin("GET", "storages/console-minio")).body.capacity_bytes,
      2147483648,
    );
    assert(
      !(
        await page.evaluate(() =>
          JSON.stringify({ ...localStorage, ...sessionStorage }),
        )
      ).includes(minio.secret_key),
    );
    await deleteStorage("console-minio");
    await expect(
      page.getByRole("heading", { name: "저장소", exact: true }),
    ).toBeVisible();
    assert.equal((await admin("GET", "storages/console-minio")).status, 404);
    console.log(
      "PASS real MinIO UI registration, all S3 options, secret re-entry, replacement and deletion",
    );
  }
  await page
    .getByRole("navigation")
    .getByRole("link", { name: "개요", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "console-live", exact: true }),
  ).toBeVisible();
}
