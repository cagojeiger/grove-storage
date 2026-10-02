import { expect, test } from "@playwright/test";
import { accessMock, owner } from "./access-fixture";
import { maintenanceMock } from "./maintenance-fixture";

for (const width of [320, 768, 1440]) {
  test(`replacement navigation and account workspace at ${width}px`, async ({
    page,
  }) => {
    await accessMock(page);
    await page.setViewportSize({ width, height: 720 });
    await page.goto(`/api/admin/console/#accounts/${owner.id}`);
    const tabs = page.getByRole("tablist", { name: "Account sections" });
    if (width >= 1200)
      await expect(tabs).toHaveAttribute("aria-orientation", "vertical");
    else await expect(tabs).not.toHaveAttribute("aria-orientation", "vertical");
    await page.getByRole("tab", { name: "Overview", exact: true }).focus();
    await page.keyboard.press(width >= 1200 ? "ArrowDown" : "ArrowRight");
    await page.keyboard.press("Enter");
    await expect(
      page.getByRole("tabpanel", { name: "API tokens" }),
    ).toBeVisible();
    if (width < 900)
      await page.getByRole("button", { name: "Open navigation" }).click();
    const sidebar = page.getByRole("complementary", {
      name: "Workspace sidebar",
    });
    const accountMenu = sidebar.getByRole("button", { name: "Account menu" });
    await expect(accountMenu).toBeInViewport();
    const bounds = await accountMenu.boundingBox();
    expect(bounds!.height).toBeLessThan(100);
    expect(bounds!.y).toBeGreaterThan(600);
    await expect(
      sidebar.getByRole("link", { name: "Activity" }),
    ).toBeInViewport();
    await sidebar.getByRole("link", { name: "Accounts", exact: true }).click();
    await expect(page.getByRole("grid", { name: "Accounts" })).toBeVisible();
    await expect(page.getByRole("contentinfo")).toContainText("Grove Storage");
    await expect(page.getByRole("contentinfo").getByRole("button")).toHaveCount(
      0,
    );
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  });

  test(`event inspection uses an accessible side panel at ${width}px`, async ({
    page,
  }) => {
    await maintenanceMock(page);
    await page.setViewportSize({ width, height: 720 });
    await page.goto("/api/admin/console/#activity");
    const event = page.getByRole("button", { name: /storage.create/ });
    await event.click();
    const details = page.getByRole("dialog", { name: "Event details" });
    await expect(details.getByLabel("Event properties")).toContainText(
      "storage.create",
    );
    await expect(
      details.getByRole("button", { name: "Close", exact: true }),
    ).toBeInViewport();
    await expect(details).toHaveCSS("right", "0px");
    await page.keyboard.press("Escape");
    await expect(details).toHaveCount(0);
    await expect(event).toBeFocused();
  });
}
