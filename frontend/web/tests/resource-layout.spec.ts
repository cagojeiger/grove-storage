import { expect, test } from "@playwright/test";
import { example, storageMock } from "./storage-fixture";
import { envelope, intercept } from "./command-fixture";

for (const width of [320, 768, 1440])
  for (const theme of ["light", "dark"]) {
    test(`resource workspace ${width}px ${theme}`, async ({ page }) => {
      await storageMock(
        page,
        Array.from({ length: 7 }, (_, i) => ({
          ...example,
          id: `home-storage-long-name-${i}`,
        })),
      );
      const clients = Array.from(
        { length: 24 },
        (_, i) => `client-long-name-${i}`,
      );
      await intercept(page, "client.list", (r) =>
        r.fulfill({ json: envelope("client.list", clients) }),
      );
      await intercept(page, "usage.clients", (r) =>
        r.fulfill({ json: envelope("usage.clients", []) }),
      );
      await intercept(page, "client.show", (r) => {
        const { input } = r.request().postDataJSON() as {
          input: { id: string };
        };
        return r.fulfill({
          json: envelope("client.show", {
            id: input.id,
            storage_id: "home-storage-long-name-6",
          }),
        });
      });
      await page.setViewportSize({ width, height: 960 });
      await page.goto("/api/admin/console/#");
      await page.getByLabel("Theme").selectOption(theme);
      await page
        .getByRole("link", {
          name: "Open client client-long-name-0",
          exact: true,
        })
        .focus();
      await expect(
        page
          .locator('[data-side="storage"][data-selected="true"]')
          .getByRole("link"),
      ).toContainText("home-storage-long-name-6");
      await expect(page.locator(".connection-paths path.selected")).toHaveCount(
        2,
      );
      await page.emulateMedia({ reducedMotion: "reduce" });
      await capture("connections");
      const hub = await page.locator(".grove-hub").boundingBox();
      const left = await page
        .locator(".connection-column")
        .first()
        .boundingBox();
      const right = await page
        .locator(".connection-column")
        .last()
        .boundingBox();
      expect(hub && left && right).toBeTruthy();
      if (hub && left && right) {
        if (width === 1440) {
          expect(hub.x).toBeGreaterThan(left.x + left.width);
          expect(right.x).toBeGreaterThan(hub.x + hub.width);
        } else {
          expect(hub.y).toBeGreaterThanOrEqual(left.y + left.height);
          expect(right.y).toBeGreaterThanOrEqual(hub.y + hub.height);
        }
      }
      await page.getByRole("link", { name: "View all storage" }).click();
      await expect(
        page
          .getByRole("grid", { name: "Storage", exact: true })
          .getByRole("link"),
      ).toHaveCount(7);
      await capture("storage-list");
      if (width < 900)
        await page.getByRole("button", { name: "Open navigation" }).click();
      await page
        .getByRole("navigation", { name: "Main navigation" })
        .getByRole("link", { name: "Clients", exact: true })
        .click();
      await expect(page.getByText("1–20 of 24", { exact: true })).toBeVisible();
      await page.getByRole("button", { name: /next page/i }).click();
      await expect(
        page
          .getByRole("grid", { name: "Clients", exact: true })
          .getByRole("link"),
      ).toHaveCount(4);
      await capture("client-list");
      async function capture(view: string) {
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        await page.screenshot({
          path: `test-results/resources-${view}-${width}-${theme}.png`,
          fullPage: true,
        });
      }
    });
  }
