import { expect, test } from "@playwright/test";
import { accessMock, owner, rawToken, root } from "./access-fixture";
import { session } from "./command-fixture";

test("role demotion during an Access mutation removes private controls", async ({
  page,
}) => {
  await accessMock(page);
  let demoted = false;
  await page.route("**/v1/session", (route) =>
    route.fulfill({ json: { ...session, role: demoted ? "reader" : "admin" } }),
  );
  await page.route("**/v1/accounts", (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    demoted = true;
    return route.fulfill({ status: 403, json: { error: "forbidden" } });
  });
  await page.goto(root);
  await page.getByRole("button", { name: "Create user", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Rejected");
  await page.getByLabel("Username", { exact: true }).fill("rejected");
  await page.getByLabel("Current password").fill("a private admin password");
  await page.getByRole("dialog").getByRole("button", { name: "Create user" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("alert")).toHaveText("Admin access required.");
  await expect(
    page.getByRole("link", { name: "Accounts", exact: true }),
  ).toHaveCount(0);
});

test("expired Access session removes account data", async ({ page }) => {
  await accessMock(page);
  await page.goto(root);
  await expect(
    page.getByRole("button", { name: /Home administrator.*Active/ }),
  ).toBeVisible();
  await page.route("**/v1/accounts?*", (route) =>
    route.fulfill({ status: 401, json: { error: "unauthenticated" } }),
  );
  await page.getByRole("button", { name: "Refresh accounts" }).click();
  await expect(page.getByLabel("Password")).toBeVisible();
  await expect(page.getByText(owner.display_name, { exact: true })).toHaveCount(
    0,
  );
});

test("pending issuance cannot double-submit and clears the secret after close", async ({
  page,
}) => {
  await accessMock(page);
  let attempts = 0;
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/accounts/*/credentials", async (route) => {
    if (route.request().method() === "GET") return route.fallback();
    attempts++;
    await gate;
    return route.fulfill({
      status: 201,
      json: {
        account_id: owner.id,
        credential_id: "key",
        expires_at: "2099-01-01T00:00:00Z",
        token: rawToken,
      },
    });
  });
  await page.goto(root);
  await page
    .getByRole("button", { name: /Home administrator.*Active/ })
    .click();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("One request");
  await page.locator("dialog form").evaluate((form) => {
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
  });
  await expect(page.getByRole("button", { name: "Saving..." })).toBeDisabled();
  await expect.poll(() => attempts).toBe(1);
  release();
  await expect(
    page.getByRole("textbox", { name: "Issued token", exact: true }),
  ).toHaveValue(rawToken);
  await page
    .getByLabel("I have saved this token. It is shown only once.")
    .check();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  expect(attempts).toBe(1);
});

test("wrong-account issued tokens are rejected as unknown", async ({
  page,
}) => {
  await accessMock(page);
  await page.route("**/accounts/*/credentials", (route) =>
    route.request().method() === "GET"
      ? route.fallback()
      : route.fulfill({
          status: 201,
          json: {
            account_id: "wrong",
            credential_id: "key",
            expires_at: "2099-01-01T00:00:00Z",
            token: rawToken,
          },
        }),
  );
  await page.goto(root);
  await page
    .getByRole("button", { name: /Home administrator.*Active/ })
    .click();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("Wrong account");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("outcome is unknown");
  await expect(
    page.getByRole("textbox", { name: "Issued token", exact: true }),
  ).toHaveCount(0);
});
