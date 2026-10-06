import { expect, test } from "@playwright/test";
import { readFile, readdir } from "node:fs/promises";
import { maintenanceMock } from "./maintenance-fixture";
import { owner } from "./access-fixture";
import { session } from "./command-fixture";

test("shared theme has no component style overrides or legacy page CSS", async () => {
  const theme = await readFile("src/console/Theme.tsx", "utf8");
  expect(theme).not.toContain("styleOverrides");
  const files = await readdir("src", { recursive: true });
  expect(files.filter((path) => path.endsWith(".css"))).toEqual([]);
  expect(files).not.toContain("design/Fields.tsx");
  expect(files).not.toContain("design/Dialog.tsx");
  const dependencies = JSON.parse(await readFile("package.json", "utf8")) as {
    dependencies: Record<string, string>;
  };
  expect(
    Object.keys(dependencies.dependencies)
      .filter((name) => /@mui\/x-/.test(name))
      .sort(),
  ).toEqual(["@mui/x-charts", "@mui/x-data-grid"]);
});

for (const mode of ["light", "dark"]) {
  for (const width of [320, 768, 1280]) {
    test(`management typography and layout ${width}px ${mode}`, async ({
      page,
    }) => {
      await maintenanceMock(page);
      await page.route("**/identity/v1/session", (route) =>
        route.fulfill({ json: { ...session, credential_id: null } }),
      );
      await page.route("**/identity/v1/me", (route) =>
        route.fulfill({ json: owner }),
      );
      await page.route("**/identity/v1/me/tokens?*", (route) =>
        route.fulfill({ json: { items: [], next_before: null } }),
      );
      await page.setViewportSize({ width, height: 900 });
      await page.goto("/api/admin/console/#activity");
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${mode}$`, "i") }).click();
      for (const [route, title] of [
        ["activity", "Activity"],
        ["settings", "My account"],
        ["settings/security", "My account"],
      ]) {
        await page.goto(`/api/admin/console/#${route}`);
        const heading = page.getByRole("heading", {
          name: title,
          level: 1,
          exact: true,
        });
        await expect(heading).toHaveCSS("font-size", "20px");
        await expect(heading).toHaveCSS("font-weight", "600");
        await expect(heading).toHaveCSS("font-family", /^-apple-system,/);
        const breadcrumb = page.getByRole("navigation", { name: "Console path" });
        if (width >= 900) await expect(breadcrumb).toContainText(title);
        else await expect(breadcrumb).toHaveCount(0);
        await expect(page.getByRole("main")).not.toHaveClass(
          /overview|settings|activity/,
        );
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        await page.screenshot({
          path: `test-results/management-mui-${route.replace("/", "-")}-${width}-${mode}.png`,
          animations: "disabled",
          fullPage: true,
        });
      }
      const current = page.getByLabel("Current password");
      await expect(current).toHaveCSS("font-family", /^-apple-system,/);
      await expect(current).toHaveCSS("font-size", "14px");
      await expect(
        page.getByRole("button", { name: "Change password" }),
      ).toHaveCSS("border-radius", "8px");
    });
  }
}
