import { expect, test } from "@playwright/test";
import { clientMock } from "./client-fixture";

for (const mode of ["missing", "denied", "available"] as const) {
  test(`S3 credential copy handles ${mode} clipboard`, async ({
    page,
    context,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    if (mode === "available") {
      await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    } else {
      await page.addInitScript((mode) => {
        Object.defineProperty(navigator, "clipboard", {
          value:
            mode === "missing"
              ? undefined
              : {
                  writeText: () =>
                    Promise.reject(
                      new DOMException("Denied", "NotAllowedError"),
                    ),
                },
        });
      }, mode);
    }
    await clientMock(page);
    await page.goto("/api/admin/console/#clients/notegate");
    await page
      .getByRole("button", { name: "Create credential", exact: true })
      .click();
    await page.getByRole("button", { name: "Create", exact: true }).click();
    await page
      .getByRole("button", { name: "Copy secret key", exact: true })
      .click();
    if (mode === "available") {
      await expect(page.getByRole("status")).toHaveText("Copied.");
      expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
        "one-time-provider-independent-secret",
      );
    } else {
      await expect(page.getByRole("status")).toHaveText(
        mode === "missing"
          ? "Copy unavailable. Select the value manually."
          : "Copy failed. Select the value manually.",
      );
      await expect(page.getByLabel("Secret key", { exact: true })).toHaveValue(
        "one-time-provider-independent-secret",
      );
    }
    expect(errors).toEqual([]);
    await expect(
      page.getByRole("button", { name: "Done", exact: true }),
    ).toBeDisabled();
  });
}
