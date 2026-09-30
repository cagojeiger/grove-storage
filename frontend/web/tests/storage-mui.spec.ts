import { expect, test } from "@playwright/test";
import { root, storageMock } from "./storage-fixture";
import { maintenanceMock } from "./maintenance-fixture";

for (const mode of ["light", "dark"]) {
  for (const width of [390, 768, 1280]) {
    test(`storage uses semantic MUI typography at ${width}px in ${mode}`, async ({
      page,
    }) => {
      await storageMock(page);
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`${root}/home-archive`);
      await page.getByLabel("Theme").selectOption(mode);
      const settings = page.getByRole("region", { name: "Storage settings" });
      const usage = page.getByRole("region", { name: "Storage usage" });
      for (const region of [settings, usage]) {
        const label = region.locator("dt").first();
        await expect(label).toHaveCSS("font-family", /^Inter,/);
        await expect(label).toHaveCSS("font-size", "14px");
        await expect(label).toHaveCSS("line-height", "20.02px");
        await expect(region.locator("dd").first()).toHaveCSS(
          "font-size",
          "16px",
        );
        await expect(region.getByRole("heading", { level: 2 })).toHaveCSS(
          "font-size",
          "20px",
        );
      }
      await expect(settings.locator("pre")).toHaveCSS(
        "font-family",
        /monospace/,
      );
      await expect(settings.locator("pre")).toHaveText("{}");
      const labels = await settings.locator("dt").allTextContents();
      expect(labels.at(-1)).toBe("Metadata");
      await page.screenshot({
        path: `test-results/storage-mui-detail-${width}-${mode}.png`,
        fullPage: true,
      });
      await page
        .getByRole("link", { name: "Storage", exact: true })
        .last()
        .click();
      const table = page.getByRole("table", { name: "Storage", exact: true });
      await expect(
        table.getByRole("link", { name: "home-archive" }),
      ).toBeVisible();
      expect(
        await table.evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
      const row = table.locator("tbody tr").first();
      await expect(
        row.getByText(width < 600 ? "/ 1 TiB" : "1 TiB", { exact: true }),
      ).toBeVisible();
      await page.screenshot({
        path: `test-results/storage-mui-list-${width}-${mode}.png`,
        fullPage: true,
      });
    });
  }

  test(`desktop Activity tabs retain the MUI button type scale in ${mode}`, async ({
    page,
  }) => {
    await maintenanceMock(page);
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto("/api/admin/console/#activity");
    await page.getByLabel("Theme").selectOption(mode);
    for (const tab of await page.getByRole("tab").all()) {
      await expect(tab).toHaveCSS("font-family", /^Inter,/);
      await expect(tab).toHaveCSS("font-size", "14px");
    }
  });
}
