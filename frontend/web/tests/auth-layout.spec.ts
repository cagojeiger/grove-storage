import { expect, test } from "@playwright/test";

const root = "/api/admin/console/";
const screenshots = "../../output/console-login-template-20261004";

for (const viewport of [
  { width: 1440, height: 900 },
  { width: 768, height: 1024 },
  { width: 390, height: 844 },
  { width: 320, height: 480 },
]) {
  for (const mode of ["light", "dark"]) {
    test(`sign-in layout ${viewport.width}px ${mode}`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await page.route("**/api/admin/identity/v1/session", (route) =>
        route.fulfill({ status: 401, json: {} }),
      );
      await page.goto(root);
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${mode}$`, "i") }).click();
      const main = page.getByRole("main");
      await expect(
        main.getByRole("heading", { name: "Sign in" }),
      ).toBeVisible();
      const panel = main;
      await expect(panel).toHaveClass(/MuiCard-root/);
      await expect(panel).toHaveCSS("border-radius", "8px");
      await expect(panel).not.toHaveCSS("box-shadow", "none");
      await expect(panel).toHaveCSS(
        "background-color",
        mode === "dark" ? "rgba(5, 7, 10, 0.4)" : "rgb(255, 255, 255)",
      );
      const box = await panel.boundingBox();
      expect(box).not.toBeNull();
      expect(
        Math.abs(box!.x + box!.width / 2 - viewport.width / 2),
      ).toBeLessThan(2);
      expect(box!.width).toBeLessThanOrEqual(450);
      if (viewport.height >= 844) {
        await expect(
          main.getByRole("button", { name: "Sign in" }),
        ).toBeInViewport();
        expect(box!.y).toBeGreaterThan(150);
      }
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      const fonts = await main
        .locator("h1, input, button")
        .evaluateAll((nodes) =>
          nodes.map((node) => getComputedStyle(node).fontFamily),
        );
      expect(new Set(fonts).size).toBe(1);
      expect(fonts[0]).toContain("-apple-system");
      expect(
        await main
          .locator("img")
          .evaluate(
            (node: HTMLImageElement) => node.complete && node.naturalWidth > 0,
          ),
      ).toBe(true);
      await page.screenshot({
        path: `${screenshots}/login-${viewport.width}-${mode}.png`,
        fullPage: true,
        animations: "disabled",
      });
      await page.getByLabel("Username").fill("reader");
      await page
        .getByLabel(/^Password\s*\*?$/)
        .fill("incorrect private phrase");
      await page.getByLabel(/^Password\s*\*?$/).press("Enter");
      await expect(page.getByRole("alert")).toContainText(
        "Username or password is incorrect.",
      );
      await expect(page.getByLabel("Username")).toHaveValue("reader");
      await expect(page.getByLabel(/^Password\s*\*?$/)).toHaveValue("");
      await page
        .getByRole("button", { name: "Sign in" })
        .scrollIntoViewIfNeeded();
      await expect(
        page.getByRole("button", { name: "Sign in" }),
      ).toBeInViewport();
    });
  }
}
