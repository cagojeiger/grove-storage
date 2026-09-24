import { expect, test } from "@playwright/test";
import { failure, intercept, session } from "./command-fixture";
import { example, root, storageMock } from "./storage-fixture";

test("Viewer can inspect storages but has no write controls", async ({ page }) => {
  await storageMock(page);
  await page.route("**/identity/v1/session", (route) => route.fulfill({ json: { ...session, role: "viewer" } }));
  await page.goto(root);
  await expect(page.getByText("Viewer · 읽기")).toBeVisible();
  await expect(page.getByRole("button", { name: "등록", exact: true })).toHaveCount(0);
  await page.getByRole("link", { name: new RegExp(example.id) }).click();
  await expect(page.getByRole("region", { name: "저장소 설정" })).toBeVisible();
  await expect(page.getByRole("button", { name: "저장소 수정" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "저장소 삭제" })).toHaveCount(0);
});

test("demotion while editing refreshes role and discards the form", async ({ page }) => {
  await storageMock(page);
  await page.goto(`${root}/${example.id}`);
  await page.getByRole("button", { name: "저장소 수정" }).click();
  await page.getByLabel("Secret key (재입력)").fill("discard-me");
  await page.route("**/identity/v1/session", (route) => route.fulfill({ json: { ...session, role: "viewer" } }));
  await intercept(page, "storage.replace", (route) => route.fulfill({ status: 403, json: failure(403) }));
  await page.getByRole("button", { name: "저장", exact: true }).click();
  await expect(page.getByText("Viewer · 읽기")).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "저장소 수정" })).toHaveCount(0);
});
