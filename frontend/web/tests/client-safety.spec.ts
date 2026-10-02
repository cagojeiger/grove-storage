import { test, expect } from "@playwright/test";
import { clientMock, root } from "./client-fixture";
import { envelope, failure, intercept, session } from "./command-fixture";

test("Reader sees clients but never requests service keys or write controls", async ({
  page,
}) => {
  const { calls } = await clientMock(page, "reader");
  await page.goto(root + "/notegate");
  await expect(page.getByRole("heading", { name: "notegate" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Delete client|Create credential/ }),
  ).toHaveCount(0);
  expect(calls.some((c) => /^(credential|client-key)\./.test(c.command))).toBe(
    false,
  );
});

test("conflicting deletion retains the client and presents the reference guard", async ({
  page,
}) => {
  await clientMock(page);
  await intercept(page, "client.delete", (route) =>
    route.fulfill({ status: 409, json: failure(409) }),
  );
  await page.goto(root + "/notegate");
  await page.getByRole("button", { name: "Delete client" }).click();
  await page.getByLabel("Client ID to delete").fill("notegate");
  await page.getByRole("button", { name: "Confirm delete" }).click();
  await expect(page.getByRole("alert")).toContainText("pending cleanup");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.getByRole("heading", { name: "notegate" })).toBeVisible();
});

for (const mode of ["lost", "malformed", "unavailable"] as const)
  test(`${mode} issuance never retries or allows repeated submission`, async ({
    page,
  }) => {
    await clientMock(page);
    let attempts = 0;
    await intercept(page, "credential.create", async (route) => {
      attempts++;
      if (mode === "lost") await route.abort();
      else if (mode === "malformed")
        await route.fulfill({
          json: envelope("credential.create", { access_key_id: "broken" }),
        });
      else await route.fulfill({ status: 503, json: failure(503) });
    });
    await page.goto(root + "/notegate");
    await page.getByRole("tab", { name: "S3 credentials", exact: true }).click();
    await page.getByRole("button", { name: "Create credential", exact: true }).click();
    await page.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("outcome is unknown");
    await expect(
      page.getByRole("button", { name: "Confirm", exact: true }),
    ).toBeDisabled();
    expect(attempts).toBe(1);
  });

for (const code of [401, 403])
  test(`key write ${code} removes sensitive controls`, async ({ page }) => {
    await clientMock(page);
    await page.goto(root + "/notegate");
    await page.getByRole("tab", { name: "S3 credentials", exact: true }).click();
    await page.getByRole("button", { name: "Create credential" }).click();
    if (code === 403)
      await page.route("**/identity/v1/session", (route) =>
        route.fulfill({ json: { ...session, role: "reader" } }),
      );
    await intercept(page, "credential.create", (route) =>
      route.fulfill({ status: code, json: failure(code) }),
    );
    await page.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    if (code === 401)
      await expect(page.getByLabel("Password")).toBeVisible();
    else await expect(page.getByRole("button", { name: "Account menu", exact: true })).toContainText("reader");
    await expect(page.getByRole("button", { name: "Create credential" })).toHaveCount(0);
  });
