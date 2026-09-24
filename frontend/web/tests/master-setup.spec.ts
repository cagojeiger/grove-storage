import { expect, test } from "@playwright/test";
import { owner, rawToken } from "./access-fixture";

for (const initialized of [false, true])
  test(`master ${initialized ? "recovery" : "bootstrap"} stays separate from User login`, async ({
    page,
  }) => {
    let setupSession = false;
    const writes: { path: string; body: unknown }[] = [];
    await page.route("**/api/admin/identity/v1/**", (route) => {
      const req = route.request(),
        path = new URL(req.url()).pathname.split("/v1")[1];
      if (path === "/session")
        return route.fulfill({
          status: 401,
          json: { error: "unauthenticated" },
        });
      if (req.method() === "POST") {
        writes.push({ path, body: req.postDataJSON() as unknown });
        if (path === "/master/session") {
          setupSession = true;
          return route.fulfill({ json: { principal: "master" } });
        }
        setupSession = false;
        return route.fulfill({
          status: 201,
          json: {
            user_id: owner.id,
            credential_id: "key",
            expires_at: "2099-01-01T00:00:00Z",
            token: rawToken,
          },
        });
      }
      return route.fulfill({
        status: setupSession ? 200 : 401,
        json: setupSession
          ? { principal: "master", scope: "setup_recovery", initialized }
          : { error: "unauthenticated" },
      });
    });
    await page.goto("/api/admin/console/");
    await page
      .getByRole("link", { name: "Initial setup / recovery", exact: true })
      .click();
    await page.getByLabel("Master token").fill("gsmt_test-secret");
    await page.getByRole("button", { name: "Verify master token" }).click();
    await expect(page.getByLabel("Master token")).toHaveCount(0);
    if (initialized) {
      await page.getByLabel("Admin user ID").fill(owner.id);
      await expect(
        page.getByRole("button", { name: "Recover access" }),
      ).toBeDisabled();
      await page
        .getByLabel("Replace this Admin's tokens and revoke its sessions")
        .check();
      await page.getByRole("button", { name: "Recover access" }).click();
    } else {
      await page.getByLabel("Admin name").fill("Owner");
      await page.getByRole("button", { name: "Create Admin" }).click();
    }
    await expect(
      page.getByRole("textbox", { name: "Issued token", exact: true }),
    ).toHaveValue(rawToken);
    expect(
      await page.evaluate(() =>
        JSON.stringify({ ...localStorage, ...sessionStorage }),
      ),
    ).not.toContain(rawToken);
    await page
      .getByLabel("I have saved this token. It is shown only once.")
      .check();
    await page.getByRole("button", { name: "Done", exact: true }).click();
    await expect(page.getByLabel("Personal token")).toBeVisible();
    expect(writes).toEqual([
      { path: "/master/session", body: { token: "gsmt_test-secret" } },
      {
        path: initialized ? "/master/recover" : "/master/bootstrap",
        body: initialized
          ? { user_id: owner.id, confirm: true }
          : { display_name: "Owner" },
      },
    ]);
  });

test("unknown bootstrap result never retries issuance", async ({ page }) => {
  let attempts = 0;
  await page.route("**/api/admin/identity/v1/**", (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith("/master/session"))
      return route.fulfill({
        json: {
          principal: "master",
          scope: "setup_recovery",
          initialized: false,
        },
      });
    if (path.endsWith("/bootstrap")) {
      attempts++;
      return route.fulfill({ status: 503, json: { error: "outcome_unknown" } });
    }
    return route.fulfill({ status: 401, json: { error: "unauthenticated" } });
  });
  await page.goto("/api/admin/console/#setup");
  await page.getByLabel("Admin name").fill("Owner");
  await page.getByRole("button", { name: "Create Admin" }).click();
  await expect(page.getByRole("alert")).toContainText("outcome is unknown");
  await expect(
    page.getByRole("button", { name: "Create Admin" }),
  ).toBeDisabled();
  expect(attempts).toBe(1);
});
