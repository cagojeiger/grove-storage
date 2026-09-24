import { test, expect } from "@playwright/test";
import { example, fillS3, root, storageMock } from "./storage-fixture";

test("zero file count does not bypass delete 409; refreshed registry is shown", async ({
  page,
}) => {
  const { reads } = await storageMock(page);
  await page.goto(root);
  await page.getByRole("link", { name: new RegExp(example.id) }).click();
  await expect(page.getByText("0 파일").first()).toBeVisible();
  await page.route("**/storages/home-archive", (route) =>
    route.request().method() === "DELETE"
      ? route.fulfill({ status: 409, json: { message: "secret-server-error" } })
      : route.fallback(),
  );
  await page.getByRole("button", { name: "저장소 삭제" }).click();
  await page.getByLabel("삭제할 저장소 ID").fill(example.id);
  await page.getByRole("button", { name: "삭제 확인" }).click();
  await expect(page.getByRole("alert")).toContainText("클라이언트 또는 파일");
  await expect(page.getByRole("alert")).not.toContainText(
    "secret-server-error",
  );
  await expect(page.getByLabel("삭제할 저장소 ID")).toHaveValue("");
  await page.getByRole("button", { name: "취소" }).click();
  await page.getByRole("link", { name: "저장소", exact: true }).first().click();
  await expect(
    page.getByRole("link", { name: new RegExp(example.id) }),
  ).toBeVisible();
  expect(reads.list).toBeGreaterThan(1);
});

test("replace conflict clears secret and retains editable nonsecret settings", async ({
  page,
}) => {
  await storageMock(page);
  await page.goto(`${root}/${example.id}`);
  await page.getByRole("button", { name: "저장소 수정" }).click();
  await page.getByLabel("Secret key (재입력)").fill("never-retain");
  await page.route("**/storages/home-archive", (route) =>
    route.request().method() === "PUT"
      ? route.fulfill({ status: 409, json: {} })
      : route.fallback(),
  );
  await page.getByRole("button", { name: "저장", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "주소를 변경할 수 없습니다",
  );
  await expect(page.getByLabel("Secret key (재입력)")).toHaveValue("");
  await expect(page.getByLabel("Endpoint", { exact: true })).toHaveValue(
    example.endpoint!,
  );
});

test("lost response blocks resubmission and refreshes instead of retrying", async ({
  page,
}) => {
  const { reads, rows } = await storageMock(page, []);
  let posts = 0;
  await page.route("**/storages", (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    posts++;
    rows.set("new-s3", { ...example, id: "new-s3" });
    return route.abort("connectionreset");
  });
  await page.goto(root);
  await page.getByRole("button", { name: "등록", exact: true }).click();
  await fillS3(page);
  await page.getByRole("button", { name: "저장", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "변경 결과를 확인하지 못했습니다",
  );
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveValue("");
  await expect(
    page.getByRole("button", { name: "저장", exact: true }),
  ).toBeDisabled();
  expect(posts).toBe(1);
  expect(reads.list).toBeGreaterThan(1);
  await page.getByRole("button", { name: "닫고 확인" }).click();
  await expect(page.getByRole("link", { name: /new-s3/ })).toBeVisible();
});

test("storage read 401 returns to login without cached detail", async ({
  page,
}) => {
  await storageMock(page);
  await page.goto(`${root}/${example.id}`);
  await expect(page.getByRole("region", { name: "저장소 설정" })).toBeVisible();
  await page.route("**/storages/home-archive", (route) =>
    route.fulfill({ status: 401, json: {} }),
  );
  await page.getByRole("button", { name: "새로고침" }).click();
  await expect(page.getByLabel("관리자 토큰")).toBeVisible();
  await expect(page.getByRole("heading", { name: example.id })).toHaveCount(0);
});

for (const method of ["POST", "PUT", "DELETE"]) {
  test(`${method} 401 removes all private views and form secrets`, async ({
    page,
  }) => {
    await storageMock(page);
    await page.goto(method === "POST" ? root : `${root}/${example.id}`);
    if (method === "DELETE") {
      await page.getByRole("button", { name: "저장소 삭제" }).click();
      await page.getByLabel("삭제할 저장소 ID").fill(example.id);
    } else if (method === "POST") {
      await page.getByRole("button", { name: "등록", exact: true }).click();
      await fillS3(page);
    } else {
      await page.getByRole("button", { name: "저장소 수정" }).click();
      await page.getByLabel("Secret key (재입력)").fill("secret");
    }
    await page.route("**/storages**", (route) =>
      route.request().method() === method
        ? route.fulfill({ status: 401, json: {} })
        : route.fallback(),
    );
    await page
      .getByRole("button", {
        name: method === "DELETE" ? "삭제 확인" : "저장",
        exact: true,
      })
      .click();
    await expect(page.getByLabel("관리자 토큰")).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page.getByRole("heading", { name: example.id })).toHaveCount(
      0,
    );
  });
}

test("pending write clears secret, prevents double submit and holds dialog", async ({
  page,
}) => {
  await storageMock(page);
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  let posts = 0;
  await page.route("**/storages", async (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    posts++;
    await pending;
    return route.fulfill({ status: 400, json: {} });
  });
  await page.goto(root);
  await page.getByRole("button", { name: "등록", exact: true }).click();
  await fillS3(page);
  await page.getByRole("button", { name: "저장", exact: true }).click();
  await expect(page.getByRole("button", { name: "저장 중..." })).toBeDisabled();
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveValue("");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(posts).toBe(1);
  release();
  await expect(page.getByRole("alert")).toContainText("접근 권한");
});
