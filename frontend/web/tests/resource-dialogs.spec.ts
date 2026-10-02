import { expect, test } from "@playwright/test";
import { resolve } from "node:path";
import { storageMock } from "./storage-fixture";
import { clientMock } from "./client-fixture";

for (const theme of ["light", "dark"]) {
  for (const width of [320, 1440]) {
    test(`resource details and stock dialogs at ${width}px in ${theme}`, async ({
      page,
    }) => {
      const fonts: string[] = [];
      page.on("request", (request) => {
        if (request.resourceType() === "font") fonts.push(request.url());
      });
      await page.setViewportSize({ width, height: width === 320 ? 480 : 900 });
      await storageMock(page);
      await page.goto("/api/admin/console/#storages/home-archive");
      await page.getByLabel("Theme").selectOption(theme);
      await page.getByRole("tab", { name: "Configuration", exact: true }).click();
      await expect(
        page.getByRole("list", { name: "Storage properties" }),
      ).toBeVisible();
      await page.getByRole("tab", { name: "Overview", exact: true }).click();
      await expect(
        page.getByLabel("Saved metadata", { exact: true }),
      ).toHaveText("{}");
      await page.screenshot({
        path: resolve(
          `../../output/console-resource-details-20261001/storage-${width}-${theme}.png`,
        ),
        fullPage: true,
      });
      await page
        .getByRole("button", { name: "Edit storage", exact: true })
        .click();
      const editor = page.getByRole("main");
      await expect(editor).toBeVisible();
      const save = editor.getByRole("button", { name: "Save", exact: true });
      await save.scrollIntoViewIfNeeded();
      await expect(save).toBeInViewport();
      await editor
        .getByLabel("Configured capacity", { exact: true })
        .scrollIntoViewIfNeeded();
      await editor.getByLabel("Capacity unit").selectOption("TiB");
      await editor.getByLabel("Capacity unit").scrollIntoViewIfNeeded();
      await expect(editor.getByLabel("Capacity unit")).toBeInViewport();
      await expect(save).toBeInViewport();
      await expect(save).toHaveCSS("font-family", /^-apple-system,/);
      await expect(editor.locator("..")).toHaveCSS("opacity", "1");
      await page.screenshot({
        path: resolve(
          `../../output/console-resource-details-20261001/editor-${width}-${theme}.png`,
        ),
      });
      await editor.getByRole("button", { name: "Cancel", exact: true }).click();
      await expect(page.getByRole("heading", { name: "Edit storage", exact: true })).toHaveCount(0);
      await expect(
        page.getByRole("heading", { name: "home-archive", exact: true }),
      ).toBeVisible();

      await clientMock(page);
      await page.goto("/api/admin/console/#clients");
      const create = page.getByRole("button", {
        name: "Create client",
        exact: true,
      });
      await create.click();
      await page
        .getByRole("dialog")
        .getByRole("button", { name: "Cancel", exact: true })
        .click();
      await expect(create).toBeFocused();
      await page
        .getByRole("grid")
        .getByRole("link", { name: "notegate", exact: true })
        .click();
      await page.getByRole("tab", { name: "S3 credentials", exact: true }).click();
      await expect(
        page.getByRole("table", { name: "Issued S3 credentials" }),
      ).toBeVisible();
      await page.getByRole("tab", { name: "Overview", exact: true }).click();
      await page
        .getByRole("button", { name: "Edit metadata", exact: true })
        .click();
      const metadata = page.getByRole("dialog", {
        name: "Edit metadata",
        exact: true,
      });
      await expect(
        metadata.getByRole("button", { name: "Save", exact: true }),
      ).toBeInViewport();
      await metadata
        .getByRole("button", { name: "Cancel", exact: true })
        .click();
      await expect(
        page.getByRole("button", { name: "Edit metadata", exact: true }),
      ).toBeFocused();
      await page.screenshot({
        path: resolve(
          `../../output/console-resource-details-20261001/client-${width}-${theme}.png`,
        ),
        fullPage: true,
      });
      await page.getByRole("tab", { name: "S3 credentials", exact: true }).click();
      await page
        .getByRole("button", { name: "Create credential", exact: true })
        .click();
      await page.getByRole("button", { name: "Confirm", exact: true }).click();
      const issued = page.getByRole("dialog", {
        name: "Create S3 credential",
        exact: true,
      });
      const done = issued.getByRole("button", { name: "Done", exact: true });
      await expect(done).toBeDisabled();
      await expect(done).toBeInViewport();
      await page.keyboard.press("Escape");
      await expect(issued).toBeVisible();
      for (let i = 0; i < 8; i++) {
        await page.keyboard.press("Tab");
        expect(
          await issued.evaluate((element) =>
            element.contains(document.activeElement),
          ),
        ).toBe(true);
      }
      await issued.getByRole("checkbox").check();
      await done.click();
      await expect(issued).toHaveCount(0);
      await expect(page.getByLabel("Secret key", { exact: true })).toHaveCount(
        0,
      );
      expect(fonts).toEqual([]);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
    });
  }
}
