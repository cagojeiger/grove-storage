import { expect, test } from "@playwright/test";
import { readFile, readdir } from "node:fs/promises";
import { maintenanceMock } from "./maintenance-fixture";
import { owner } from "./access-fixture";
import { session } from "./command-fixture";

test("shared theme has no component style overrides or legacy page CSS", async () => {
  const theme = await readFile("src/design/ConsoleTheme.tsx", "utf8");
  const css = await readFile("src/design/theme.css", "utf8");
  expect(theme).not.toContain("styleOverrides");
  expect(css).not.toMatch(
    /\.Mui|font-size|font-family|\.storage|\.account|\.usage|\.activity/,
  );
  const files = await readdir("src", { recursive: true });
  expect(files.filter((path) => path.endsWith(".css")).sort()).toEqual([
    "design/theme.css",
    "features/overview/connections.css",
  ]);
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
      await page.getByLabel("Theme").selectOption(mode);
      for (const [route, title] of [
        ["activity", "Activity"],
        ["settings", "My account"],
        ["settings/security", "Security"],
      ]) {
        await page.goto(`/api/admin/console/#${route}`);
        const heading = page.getByRole("heading", {
          name: title,
          level: 1,
          exact: true,
        });
        await expect(heading).toHaveCSS("font-size", "24px");
        await expect(heading).toHaveCSS("font-weight", "400");
        await expect(heading).toHaveCSS("font-family", /^-apple-system,/);
        await expect(
          page.getByRole("navigation", { name: "Breadcrumb" }),
        ).toContainText(title);
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
      await expect(current).toHaveCSS("font-size", "16px");
      await expect(
        page.getByRole("button", { name: "Change password" }),
      ).toHaveCSS("border-radius", "4px");
    });
  }
}
