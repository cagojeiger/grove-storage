import { expect, test } from "@playwright/test";
import { clientMock, root } from "./client-fixture";
import { context, maintenanceMock } from "./maintenance-fixture";

for (const theme of ["light", "dark"]) {
  for (const width of [390, 1280]) {
    test(`client storage label stays above empty and selected values at ${width}px in ${theme}`, async ({ page }) => {
      await clientMock(page);
      await page.setViewportSize({ width, height: 844 });
      await page.goto(root);
      await page.getByLabel("Theme").selectOption(theme);
      await page.getByRole("button", { name: "Create client" }).click();
      const storage = page.getByRole("combobox", { name: /^Storage/ });
      const label = page.getByRole("dialog").locator("label").filter({ hasText: /^Storage/ });
      await expect(storage).toHaveValue("");
      await expect(label).toHaveAttribute("data-shrink", "true");
      await page.getByRole("dialog").screenshot({
        path: `test-results/client-empty-${width}-${theme}.png`,
        animations: "disabled",
      });
      const labelBox = await label.boundingBox();
      const fieldBox = await storage.boundingBox();
      expect(labelBox!.y + labelBox!.height).toBeLessThan(fieldBox!.y + fieldBox!.height / 2);
      await storage.selectOption("home-archive");
      await page.getByLabel(/^Client ID/).click();
      await expect(label).toHaveAttribute("data-shrink", "true");
      await expect(storage).toHaveValue("home-archive");
    });
  }

  for (const stream of ["audit", "invocations", "security"]) {
    test(`mobile ${stream} exposes result and actor without horizontal scrolling in ${theme}`, async ({ page }) => {
      await maintenanceMock(page);
      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(`/api/admin/console/#activity${stream === "audit" ? "" : `/${stream}`}`);
      await page.getByLabel("Theme").selectOption(theme);
      for (const tab of await page.getByRole("tab").all()) {
        await expect(tab).toBeInViewport({ ratio: 1 });
      }
      const table = page.getByRole("table", { name: "Activity" });
      const event = table.getByRole("button");
      await expect(event).toBeVisible();
      expect(await table.evaluate(el => el.getBoundingClientRect().right <= innerWidth)).toBe(true);
      await expect(table.locator("tbody td").filter({ hasText: context.actor_id })).toBeInViewport();
      await event.click();
      const dialog = page.getByRole("dialog", { name: "Event details" });
      await expect(dialog.getByText(context.request_id, { exact: true })).toBeVisible();
      await expect(dialog.getByText(context.credential_id, { exact: true })).toBeVisible();
      const actorLabel = dialog.locator("dt").filter({ hasText: /^Actor$/ });
      await expect(actorLabel).toHaveCSS("font-size", "13px");
    });
  }
}
