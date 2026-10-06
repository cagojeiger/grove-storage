import { expect, test, type Page } from "@playwright/test";
import { resolve } from "node:path";
import { accessMock, owner } from "./access-fixture";
import { session } from "./command-fixture";

async function fixture(page: Page, password = true) {
  await accessMock(page);
  await page.route("**/api/admin/identity/v1/me", (route) =>
    route.fulfill({ json: owner }),
  );
  let signedIn = true;
  let signouts = 0;
  await page.route("**/api/admin/identity/v1/session", (route) => {
    if (route.request().method() === "DELETE") {
      signedIn = false;
      signouts++;
      return route.fulfill({ status: 204 });
    }
    return signedIn
      ? route.fulfill({
          json: {
            ...session,
            user_id: owner.id,
            credential_id: password ? null : "credential",
          },
        })
      : route.fulfill({ status: 401, json: { error: "unauthenticated" } });
  });
  return { signouts: () => signouts };
}

for (const width of [320, 768, 1440])
  for (const theme of ["light", "dark"]) {
    test(`sidebar opens my account directly at ${width}px in ${theme}`, async ({
      page,
    }) => {
      await fixture(page);
      await page.setViewportSize({ width, height: 720 });
      await page.goto("/api/admin/console/#accounts");
      await expect(page.getByRole("grid", { name: "Accounts" })).toContainText(
        owner.display_name,
      );
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      if (width < 900) {
        await expect(
          page.getByRole("link", { name: "My account", exact: true }),
        ).toHaveCount(0);
        await page.getByRole("button", { name: "Open navigation" }).click();
      }
      const sidebar = page.getByRole("complementary", {
        name: "Workspace sidebar",
      });
      const trigger = sidebar.getByRole("link", { name: "My account", exact: true });
      await expect(trigger).toBeVisible();
      await expect(
        sidebar.getByText(owner.display_name, { exact: true }),
      ).toBeVisible();
      await expect(sidebar.getByText("admin", { exact: true })).toBeVisible();
      await expect(
        page
          .getByRole("contentinfo")
          .getByRole("link", { name: "My account", exact: true }),
      ).toHaveCount(0);
      const bounds = await trigger.boundingBox();
      expect(bounds!.y).toBeGreaterThan(600);
      expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(720);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      if (width === 1440) {
        await page.mouse.wheel(0, 1200);
        await expect
          .poll(async () => (await trigger.boundingBox())!.y)
          .toBe(bounds!.y);
        await page.mouse.wheel(0, -1200);
        await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
      }
      await page.screenshot({
        path: resolve(
          "../../output/console-sidebar-20261002",
          `layout-${width}-${theme}.png`,
        ),
        animations: "disabled",
      });
      await trigger.focus();
      await page.keyboard.press("Enter");
      await expect(
        page.getByRole("heading", { name: "My account", exact: true }),
      ).toBeVisible();
      await page.screenshot({
        path: resolve(
          "../../output/console-sidebar-20261002",
          `accounts-${width}-${theme}.png`,
        ),
        animations: "disabled",
      });
      await expect(page.getByRole("button", { name: "Account menu" })).toHaveCount(0);
      await page.getByRole("tab", { name: "Security", exact: true }).click();
      await expect(
        page.getByRole("heading", { name: "My account", exact: true }),
      ).toBeVisible();
      await expect(page.getByRole("menu")).toHaveCount(0);
      if (width < 900) {
        await expect(sidebar).toHaveCount(0);
      }
    });
  }

test("mobile drawer closes from its close button and same-page navigation", async ({
  page,
}) => {
  await fixture(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/api/admin/console/#accounts");
  const open = page.getByRole("button", { name: "Open navigation" });
  await open.click();
  await page.getByRole("button", { name: "Close navigation" }).click();
  await expect(open).toBeFocused();
  await open.click();
  await page.getByRole("link", { name: "Accounts", exact: true }).click();
  await expect(
    page.getByRole("navigation", { name: "Main navigation" }),
  ).toHaveCount(0);
});

test("token session hides password actions and signing out is sent once", async ({
  page,
}) => {
  const state = await fixture(page, false);
  await page.goto("/api/admin/console/#accounts");
  await page.getByRole("link", { name: "My account", exact: true }).click();
  await expect(
    page.getByRole("tab", { name: "Security", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Sign in", exact: true }),
  ).toBeVisible();
  expect(state.signouts()).toBe(1);
  await expect(
    page.getByRole("complementary", { name: "Workspace sidebar" }),
  ).toHaveCount(0);
});
