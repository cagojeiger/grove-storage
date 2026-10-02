import { expect, test } from "@playwright/test";
import { clientMock, root } from "./client-fixture";

for (const theme of ["light", "dark"]) {
  for (const width of [320, 390, 768, 1280]) {
    test(`client list and detail use MUI typography at ${width}px in ${theme}`, async ({
      page,
    }) => {
      const { clients, s3 } = await clientMock(page);
      const id = "client-with-a-long-identifier-for-responsive-layout";
      clients.set(id, {
        id,
        storage_id: "storage-with-a-long-identifier-for-responsive-layout",
      });
      s3.add("a".repeat(64));
      await page.setViewportSize({ width, height: 900 });
      await page.goto(root);
      await page.getByLabel("Theme").selectOption(theme);
      const table = page.getByRole("grid", { name: "Clients", exact: true });
      await expect(
        table.getByRole("link", { name: id, exact: true }),
      ).toBeVisible();
      expect(
        await table.evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      const row = table
        .getByRole("row")
        .filter({ has: page.getByRole("link", { name: id, exact: true }) });
      if (width >= 600)
        await expect(
          row.getByRole("gridcell", { name: "0", exact: true }),
        ).toBeVisible();
      await expect(
        row.getByRole("gridcell", { name: "0 B", exact: true }),
      ).toBeVisible();
      if (width >= 900)
        await expect(
          row
            .getByText("storage-with-a-long-identifier-for-responsive-layout", {
              exact: true,
            })
            .filter({ visible: true }),
        ).toBeVisible();
      await page.screenshot({
        path: `test-results/clients-mui-list-${width}-${theme}.png`,
        fullPage: true,
      });
      await table.getByRole("link", { name: id, exact: true }).click();
      await expect(
        page.getByRole("heading", { name: id, level: 1 }),
      ).toBeVisible();
      const details = page.getByRole("region", { name: "Client details" });
      const properties = details.getByLabel("Client properties");
      await expect(properties.locator("dd")).toHaveCount(3);
      for (const name of ["Client ID", "S3 bucket", "Storage"]) {
        const label = properties.getByText(name, { exact: true });
        await expect(label).toHaveCSS("font-family", /^-apple-system,/);
        await expect(label).toHaveCSS("font-size", "14px");
      }
      await expect(properties.getByRole("link")).toHaveCSS("font-size", "14px");
      await expect(details.getByText("0 B", { exact: true })).toHaveCSS(
        "font-size",
        "24px",
      );
      await page.getByRole("tab", { name: "S3 credentials", exact: true }).click();
      const keys = page.getByRole("region", { name: "S3 credentials" });
      await expect(keys.locator("code")).toHaveCount(2);
      await expect(keys.locator("code").last()).toHaveCSS(
        "font-family",
        /monospace/,
      );
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: `test-results/clients-mui-detail-${width}-${theme}.png`,
        fullPage: true,
      });
    });
  }

  test(`one-time credential actions stay visible on a short screen in ${theme}`, async ({
    page,
  }) => {
    await clientMock(page);
    await page.setViewportSize({ width: 320, height: 480 });
    await page.goto(`${root}/notegate`);
    await page.getByLabel("Theme").selectOption(theme);
    await page.getByRole("tab", { name: "S3 credentials", exact: true }).click();
    await page
      .getByRole("button", { name: "Create credential", exact: true })
      .click();
    await page.getByRole("button", { name: "Confirm", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Create S3 credential" });
    const done = dialog.getByRole("button", { name: "Done", exact: true });
    const saved = dialog.getByRole("checkbox");
    await expect(done).toBeInViewport();
    await expect(saved).toBeInViewport();
    await expect(done).toBeDisabled();
    await expect(
      dialog.getByRole("button", { name: "Done", exact: true }),
    ).toBeDisabled();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByLabel("Secret key", { exact: true })).toHaveCSS(
      "font-family",
      /monospace/,
    );
    await saved.check();
    await done.click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByLabel("Secret key", { exact: true })).toHaveCount(0);
  });
}
