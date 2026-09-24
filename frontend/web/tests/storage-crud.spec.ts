import { test, expect } from "@playwright/test";
import { example, fillS3, root, storageMock } from "./storage-fixture";

test("S3 registration sends all options with CSRF and removes the secret", async ({
  page,
}) => {
  const { writes } = await storageMock(page, []);
  await page.goto(root);
  await page.getByRole("button", { name: "등록", exact: true }).click();
  await fillS3(page);
  const sent = page.waitForRequest((req) => req.method() === "POST");
  await page.getByRole("button", { name: "저장", exact: true }).click();
  expect((await sent).headers()["x-filegate-csrf"]).toBe("1");
  await expect(
    page.getByRole("heading", { name: "new-s3", exact: true }),
  ).toBeVisible();
  expect(writes).toEqual([
    {
      method: "POST",
      body: {
        id: "new-s3",
        kind: "s3",
        endpoint: "https://s3.example.com",
        public_endpoint: "https://public.example.com",
        region: "ap-northeast-2",
        bucket: "files",
        access_key: "test-access",
        secret_key: "ephemeral-provider-secret",
        capacity_bytes: 1.5 * 1024 ** 4,
        force_path_style: true,
        force_relay: true,
      },
    },
  ]);
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).not.toContain("ephemeral-provider-secret");
  await page.getByRole("button", { name: "저장소 수정" }).click();
  await expect(page.getByLabel("Secret key (재입력)")).toHaveValue("");
  await page.getByLabel("Secret key (재입력)").fill("replacement-secret");
  await page.getByLabel("등록 용량", { exact: true }).fill("1000");
  await page.getByRole("button", { name: "저장", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(writes[1].method).toBe("PUT");
  expect(writes[1].body).toMatchObject({
    endpoint: "https://s3.example.com",
    capacity_bytes: 1000,
    secret_key: "replacement-secret",
  });
  expect(writes[1].body).not.toHaveProperty("id");
});

test("kind switch clears secret; fs creation, reload, history and deletion", async ({
  page,
}) => {
  const { writes } = await storageMock(page, []);
  await page.goto(root);
  await page.getByRole("button", { name: "등록", exact: true }).click();
  await fillS3(page, "list");
  await page.getByLabel("종류", { exact: true }).selectOption("fs");
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveCount(0);
  await page.getByLabel("종류", { exact: true }).selectOption("s3");
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveValue("");
  await page.getByLabel("종류", { exact: true }).selectOption("fs");
  await page.getByLabel("루트 경로").fill("/data/objects");
  await page.getByRole("button", { name: "저장", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "list", exact: true }),
  ).toBeVisible();
  expect(writes[0].body).toEqual({
    id: "list",
    kind: "fs",
    root_path: "/data/objects",
    capacity_bytes: 1.5 * 1024 ** 4,
  });
  await page.reload();
  await expect(page.getByText("/data/objects", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "저장소 삭제" }).click();
  const confirm = page.getByRole("button", { name: "삭제 확인" });
  await expect(confirm).toBeDisabled();
  await page.getByLabel("삭제할 저장소 ID").fill("wrong");
  await expect(confirm).toBeDisabled();
  await page.getByLabel("삭제할 저장소 ID").fill("list");
  await confirm.click();
  await expect(page.getByText("등록된 저장소가 없습니다.")).toBeVisible();
  expect(writes[1].method).toBe("DELETE");
});

test("search, cancel and keyboard focus do not mutate registry", async ({
  page,
}) => {
  const { writes } = await storageMock(page);
  await page.goto(root);
  await page.getByRole("button", { name: "등록", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "등록", exact: true }),
  ).toBeFocused();
  await page.getByRole("searchbox", { name: "저장소 검색" }).fill("missing");
  await expect(page.getByText("검색 결과가 없습니다.")).toBeVisible();
  await page.getByRole("searchbox").fill("HOME");
  await page.getByRole("link", { name: new RegExp(example.id) }).click();
  await expect(page.getByRole("heading", { name: example.id })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("searchbox")).toBeVisible();
  expect(writes).toEqual([]);
});
