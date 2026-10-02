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
      await page
        .getByRole("tab", { name: "Configuration", exact: true })
        .click();
      const settings = page.getByRole("region", { name: "Storage settings" });
      const usage = page.getByRole("region", { name: "Storage usage" });
      const properties = settings.getByLabel("Storage properties");
      await expect(properties.locator("dd")).toHaveCount(9);
      for (const label of [properties.getByText("Type", { exact: true })]) {
        await expect(label).toHaveCSS("font-family", /^-apple-system,/);
        await expect(label).toHaveCSS("font-size", "14px");
      }
      await expect(properties.getByText("S3", { exact: true })).toHaveCSS(
        "font-size",
        "14px",
      );
      await expect(settings.getByRole("heading", { level: 2 })).toHaveCSS(
        "font-size",
        "20px",
      );
      await page.screenshot({
        path: `test-results/storage-mui-config-${width}-${mode}.png`,
        fullPage: true,
      });
      await page.getByRole("tab", { name: "Overview", exact: true }).click();
      await expect(usage.getByText("Stored data", { exact: true })).toHaveCSS(
        "font-size",
        "14px",
      );
      await expect(usage.getByText("Stored data", { exact: true })).toHaveCSS(
        "font-family",
        /^-apple-system,/,
      );
      await expect(usage.getByText("0 B", { exact: true }).first()).toHaveCSS(
        "font-size",
        "24px",
      );
      await expect(
        page.getByLabel("Saved metadata", { exact: true }),
      ).toHaveCSS("font-family", /monospace/);
      await expect(
        page.getByLabel("Saved metadata", { exact: true }),
      ).toHaveText("{}");
      expect(
        await page
          .getByRole("region", { name: "Metadata", exact: true })
          .evaluate((el) =>
            Boolean(
              el.compareDocumentPosition(
                document.querySelector('[aria-label="Storage usage"]')!,
              ) & Node.DOCUMENT_POSITION_FOLLOWING,
            ),
          ),
      ).toBe(true);
      await page.screenshot({
        path: `test-results/storage-mui-detail-${width}-${mode}.png`,
        fullPage: true,
      });
      await page
        .getByRole("link", { name: "Storage", exact: true })
        .last()
        .click();
      const table = page.getByRole("grid", { name: "Storage", exact: true });
      await expect(
        table.getByRole("link", { name: "home-archive" }),
      ).toBeVisible();
      expect(
        await table.evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
      if (width >= 600)
        await expect(
          table.getByRole("gridcell").filter({ hasText: /\/ 1 TiB/ }),
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
      await expect(tab).toHaveCSS("font-family", /^-apple-system,/);
      await expect(tab).toHaveCSS("font-size", "14px");
    }
  });
}
