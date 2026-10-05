import { expect, test } from "@playwright/test";
import { clientMock, root } from "./client-fixture";

for (const mode of ["light", "dark"]) {
  for (const width of [320, 1280]) {
    test(`official template controls and responsive dialog at ${width}px in ${mode}`, async ({
      page,
    }) => {
      await clientMock(page);
      await page.setViewportSize({ width, height: 720 });
      await page.goto(root);
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${mode}$`, "i") }).click();
      const create = page.getByRole("button", {
        name: "Create client",
        exact: true,
      });
      await expect(create).toHaveCSS("border-radius", "8px");
      await expect(create).toHaveCSS("font-size", "14px");
      await expect(create).toHaveCSS("text-transform", "none");
      await expect(create).toHaveCSS("padding-top", "6px");
      const heading = page.getByRole("heading", { level: 1, name: "Clients" });
      await expect(heading).toHaveCSS("font-size", "20px");
      await expect(heading).toHaveCSS("font-weight", "600");
      await create.click();
      const dialog = page.getByRole("dialog", { name: "Create client" });
      await expect(dialog).toBeVisible();
      await expect(dialog).toHaveCSS("margin", width < 600 ? "0px" : "32px");
      await expect(dialog.getByRole("heading")).toHaveCSS("font-size", "18px");
      await expect(
        dialog.getByRole("button", { name: "Cancel" }),
      ).toBeInViewport();
      expect(
        await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
      await dialog.getByRole("button", { name: "Cancel" }).click();
      await expect(create).toBeFocused();
    });
  }
}
