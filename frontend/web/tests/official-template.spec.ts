import { expect, test } from "@playwright/test";
import { accessMock, owner } from "./access-fixture";
import { storageMock } from "./storage-fixture";
import { clientMock } from "./client-fixture";
import { maintenanceMock } from "./maintenance-fixture";
import { session } from "./command-fixture";

for (const width of [320, 768, 1440]) {
  for (const mode of ["light", "dark"]) {
    for (const kind of [
      "storage",
      "client",
      "account",
      "activity",
      "profile",
    ] as const) {
      test(`official template ${kind} ${width}px ${mode}`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        if (kind === "storage") await storageMock(page);
        if (kind === "client") await clientMock(page);
        if (kind === "account") await accessMock(page);
        if (kind === "activity" || kind === "profile")
          await maintenanceMock(page);
        await page.route("**/identity/v1/me", (route) =>
          route.fulfill({ json: owner }),
        );
        await page.route("**/identity/v1/session", (route) =>
          route.fulfill({
            json: { ...session, credential_id: null, user_id: owner.id },
          }),
        );
        const route = {
          storage: "storages",
          client: "clients",
          account: "accounts",
          activity: "activity",
          profile: "settings/security",
        }[kind];
        await page.goto(`/api/admin/console/#${route}`);
        await page.getByRole("button", { name: "Theme", exact: true }).click();
        await page
          .getByRole("menuitem", { name: new RegExp(`^${mode}$`, "i") })
          .click();
        await expect(page.locator("html")).toHaveAttribute("data-theme", mode);
        const heading = page
          .getByRole("main")
          .getByRole("heading", { level: 1 });
        await expect(heading).toHaveCSS("font-size", "20px");
        await expect(heading).toHaveCSS("font-weight", "600");
        await expect(heading).toHaveCSS("font-family", /^-apple-system,/);
        if (kind !== "profile")
          await expect(page.getByRole("grid")).toBeVisible();
        await expect
          .poll(() =>
            page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          )
          .toBe(true);
        await page.screenshot({
          path: `../../output/console-official-template-20261004/${kind}-${width}-${mode}.png`,
          fullPage: true,
          animations: "disabled",
        });
        if (kind === "profile") {
          await expect(page.getByLabel("Current password")).toHaveCSS(
            "font-family",
            /^-apple-system,/,
          );
          await expect(page.getByLabel("Current password")).toHaveCSS(
            "font-size",
            "14px",
          );
        } else if (kind === "activity") {
          await page
            .getByRole("button", { name: "storage.create", exact: true })
            .click();
          await expect(
            page.getByRole("dialog", { name: "Event details" }),
          ).toBeVisible();
          await page
            .getByRole("button", { name: "Close", exact: true })
            .click();
        } else {
          const action = {
            storage: "Add storage",
            client: "Create client",
            account: "Create account",
          }[kind];
          await page.getByRole("button", { name: action, exact: true }).click();
          const dialog = page.getByRole("dialog", { name: action });
          await expect(dialog).toBeVisible();
          await expect(
            dialog.getByRole("button", { name: "Cancel", exact: true }),
          ).toBeInViewport();
          expect(
            await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth),
          ).toBe(true);
          const fonts = await dialog
            .locator('input:not([type="checkbox"]):not([type="radio"]), button, label')
            .evaluateAll((nodes) =>
              nodes
                .filter((el) => el.getClientRects().length)
                .map((el) => getComputedStyle(el).fontFamily),
            );
          expect(new Set(fonts).size).toBe(1);
          expect(fonts[0]).toContain("-apple-system");
          await page.screenshot({
            path: `../../output/console-official-template-20261004/${kind}-dialog-${width}-${mode}.png`,
            animations: "disabled",
          });
          await dialog
            .getByRole("button", { name: "Cancel", exact: true })
            .click();
        }
      });
    }
  }
}
