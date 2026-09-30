import { mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { expect } from "@playwright/test";

export async function captureConsole(page, origin, directory) {
  const output = resolve(directory);
  await mkdir(output, { recursive: true });
  for (const mode of ["light", "dark"]) {
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.getByLabel("Theme").selectOption(mode);
    await expect(page.locator("html")).toHaveAttribute("data-theme", mode);
    for (const [name, route, title] of [
      ["overview", "", "Overview"],
      ["storage", "storages/console-live", "console-live"],
      ["client", "clients/console-client", "console-client"],
      ["accounts", "accounts", "Accounts"],
      ["activity", "activity", "Activity"],
    ]) {
      await page.goto(`${origin}/api/admin/console/#${route}`);
      await expect(
        page.getByRole("heading", { name: title, exact: true }),
      ).toBeVisible();
      if (name === "overview") {
        await expect(
          page.getByRole("link", { name: "Open storage console-live" }),
        ).toBeVisible();
        await expect(
          page.getByRole("table", { name: "Storage usage" }),
        ).toContainText("64 KiB");
        await expect(
          page.locator(".connection-paths path").first(),
        ).toHaveAttribute("d", /^M/);
      } else if (name === "storage") {
        await page
          .getByRole("button", { name: "Test connection", exact: true })
          .click();
        await expect(page.getByRole("status")).toContainText(
          "Bucket access verified",
        );
      } else if (name === "client") {
        await expect(
          page.getByLabel("Saved metadata", { exact: true }),
        ).toHaveText("{}");
        await expect(page.getByText("No credentials issued.")).toBeVisible();
      } else {
        await expect(
          page.getByRole("table").locator("tbody tr").first(),
        ).toBeVisible();
      }
      await expect(page.getByRole("alert")).toHaveCount(0);
      await page.screenshot({
        path: join(output, `${name}-${mode}.png`),
        fullPage: true,
        animations: "disabled",
      });
    }
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(`${origin}/api/admin/console/#accounts`);
    await expect(page.getByRole("table", { name: "Accounts" })).toContainText(
      "Console test owner",
    );
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.screenshot({
      path: join(output, `accounts-mobile-${mode}.png`),
      fullPage: true,
      animations: "disabled",
    });
  }
  console.log(`PASS real API screenshots saved to ${output}`);
}
