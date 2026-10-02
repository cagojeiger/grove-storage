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
      await page.getByLabel("Theme").selectOption(theme);
      await expect(
        page.getByRole("link", { name: new RegExp(long.id) }),
      ).toBeVisible();
      await screenshot("list");
      await page.getByRole("link", { name: new RegExp(long.id) }).click();
      await expect(
        page.getByRole("heading", { name: long.id, level: 1 }),
      ).toBeVisible();
      await page.getByRole("tab", { name: "Configuration", exact: true }).click();
      await expect(
        page.getByRole("region", { name: "Storage settings" }),
      ).toBeVisible();
      await screenshot("detail");
      await page.getByRole("button", { name: "Edit storage" }).click();
      await expect(page.getByLabel("Secret key (re-enter)")).toBeVisible();
      await screenshot("editor");
      await page
        .getByLabel(/^Configured capacity\s*\*?$/)
        .scrollIntoViewIfNeeded();
      await page
        .getByRole("button", { name: "Save", exact: true })
        .scrollIntoViewIfNeeded();
      await expect(
        page.getByRole("button", { name: "Save", exact: true }),
      ).toBeInViewport();
      await screenshot("editor-bottom");
      expect(
        await page
          .getByRole("main")
          .evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
      await page.getByRole("button", { name: "Cancel" }).click();
      await expect(
        page.getByRole("button", { name: "Edit storage" }),
      ).toBeVisible();
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
