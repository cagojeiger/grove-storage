import { test, expect } from "@playwright/test";
import { example, root, storageMock } from "./storage-fixture";

for (const width of [320, 390, 768, 1024, 1440]) {
  for (const theme of ["light", "dark"]) {
    test(`storages ${width}px ${theme}: list, detail and editor`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 960 });
      const long = {
        ...example,
        id: "home-archive-with-a-long-identifier-for-responsive-layout",
        endpoint:
          "https://storage.home.example.com/long-endpoint-for-the-storage",
      };
      await storageMock(page, [
        example,
        long,
        {
          ...example,
          id: "local-files",
          kind: "fs",
          root_path: "/mnt/storage/objects",
          capacity_bytes: 2 * 1024 ** 4,
        },
      ]);
      await page.goto(root);
      await page.getByLabel("화면 테마").selectOption(theme);
      await expect(
        page.getByRole("link", { name: new RegExp(long.id) }),
      ).toBeVisible();
      await screenshot("list");
      await page.getByRole("link", { name: new RegExp(long.id) }).click();
      await expect(
        page.getByRole("heading", { name: long.id, level: 1 }),
      ).toBeVisible();
      await expect(
        page.getByRole("region", { name: "저장소 설정" }),
      ).toBeVisible();
      await screenshot("detail");
      await page.getByRole("button", { name: "저장소 수정" }).click();
      await expect(page.getByLabel("Secret key (재입력)")).toBeVisible();
      await screenshot("editor");
      await page
        .getByLabel("등록 용량", { exact: true })
        .scrollIntoViewIfNeeded();
      await page
        .getByRole("button", { name: "저장", exact: true })
        .scrollIntoViewIfNeeded();
      await expect(
        page.getByRole("button", { name: "저장", exact: true }),
      ).toBeInViewport();
      await screenshot("editor-bottom");
      expect(
        await page
          .locator("dialog")
          .evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
      await page.getByRole("button", { name: "취소" }).click();
      await expect(
        page.getByRole("button", { name: "저장소 수정" }),
      ).toBeFocused();
      async function screenshot(view: string) {
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        await page.screenshot({
          path: `test-results/storage-${view}-${width}-${theme}.png`,
          fullPage: !view.startsWith("editor"),
        });
      }
    });
  }
}
