import { mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { expect } from "@playwright/test";

export async function captureConsole(page, origin, directory) {
  const output = resolve(directory);
  await mkdir(output, { recursive: true });
  for (const mode of ["light", "dark"]) {
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${mode}$`, "i") }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", mode);
    for (const [name, route, title] of [
      ["overview", "", "Overview"],
      ["usage", "usage", "Usage history"],
      ["storage", "storages/console-live", "console-live"],
      ["client", "clients/console-client", "console-client"],
      ["accounts", "accounts", "Accounts"],
      ["activity", "activity", "Activity"],
      ["profile", "settings", "My account"],
      ["security", "settings/security", "My account"],
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
      } else if (name === "usage") {
        await expect(
          page
            .getByLabel("Stored data by day", { exact: true })
            .locator(".MuiLineChart-mark")
            .first(),
        ).toBeVisible();
        await expect(
          page.getByRole("region", { name: "Daily snapshots" }),
        ).toContainText("1 GiB");
      } else if (name === "storage") {
        await page
          .getByRole("button", { name: "Test connection", exact: true })
          .click();
        await expect(page.getByRole("status")).toContainText(
          "Bucket access verified",
        );
      } else if (name === "client") {
        await expect(page.getByLabel("Saved metadata", { exact: true })).toHaveText("No metadata.");
        await expect(page.getByText("No credentials issued.")).toBeVisible();
      } else if (name === "profile") {
        await expect(page.getByRole("textbox", { name: "Display name", exact: true })).toHaveValue("Console test owner");
        await page.getByRole("tab", { name: "API tokens", exact: true }).click();
        await expect(
          page.getByRole("list", { name: "Management API tokens", exact: true }),
        ).toBeAttached();
        await page.getByRole("tab", { name: "Sessions", exact: true }).click();
        await expect(page.getByRole("heading", { name: "My sessions", exact: true })).toBeVisible();
        await expect(
          page
            .getByRole("list", { name: "My sessions", exact: true })
            .getByRole("listitem")
            .first(),
        ).toBeVisible();
        await page.getByRole("tab", { name: "Profile", exact: true }).click();
      } else if (name === "security") {
        await expect(page.getByLabel("Current password")).toBeVisible();
      } else {
        await expect(
          page.getByRole("grid").getByRole("row").nth(1),
        ).toBeVisible();
      }
      await expect(page.getByRole("alert")).toHaveCount(0);
      await expect(
        page.getByRole("status").filter({ hasText: /Loading/ }),
      ).toHaveCount(0);
      await page.evaluate(() => window.scrollTo(0, 0));
      await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
      await page.screenshot({
        path: join(output, `${name}-${mode}.png`),
        fullPage: true,
        animations: "disabled",
      });
      if (name === "accounts") {
        await page
          .getByRole("link", { name: "Console test owner", exact: true })
          .click();
        await expect(
          page.getByRole("heading", {
            name: "Console test owner",
            exact: true,
            level: 1,
          }),
        ).toBeVisible();
        await expect(
          page.getByRole("button", { name: "Edit name", exact: true }),
        ).toBeVisible();
        await page.screenshot({
          path: join(output, `account-detail-${mode}.png`),
          fullPage: true,
          animations: "disabled",
        });
      }
    }
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(`${origin}/api/admin/console/#accounts`);
    await expect(page.getByRole("grid", { name: "Accounts" })).toContainText(
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
