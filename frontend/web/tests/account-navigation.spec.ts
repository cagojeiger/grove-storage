import { expect, test } from "@playwright/test";
import { accessMock, owner, otherUser } from "./access-fixture";
import { session } from "./command-fixture";

const base = "/api/admin/console/#accounts";

test("account refresh tracks list loading instead of an inactive detail query", async ({
  page,
}) => {
  await accessMock(page);
  await page.goto(base);
  await expect(
    page.getByRole("link", { name: owner.display_name, exact: true }),
  ).toBeVisible();
  let release: () => void = () => {};
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/v1/accounts?*", async (route) => {
    await pending;
    await route.fallback();
  });
  const refresh = page.getByRole("button", { name: "Refresh accounts" });
  await refresh.click();
  await expect(refresh).toBeDisabled();
  release();
  await expect(refresh).toBeEnabled();
});

test("details survive reload and history without reading the account list", async ({
  page,
}) => {
  await accessMock(page);
  let lists = 0;
  await page.route("**/v1/accounts?*", (route) => {
    lists++;
    return route.fallback();
  });
  await page.goto(`${base}/${owner.id}`);
  await expect(
    page.getByRole("heading", { name: owner.display_name }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("heading", { name: owner.display_name }),
  ).toBeVisible();
  expect(lists).toBe(0);
  await page
    .getByRole("main")
    .getByRole("link", { name: "Accounts", exact: true })
    .click();
  await expect(
    page.getByRole("link", { name: "Writer", exact: true }),
  ).toBeVisible();
  await page.getByRole("link", { name: "Writer", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(otherUser.id));
  await page.goBack();
  await expect(
    page.getByRole("link", { name: "Writer", exact: true }),
  ).toBeVisible();
  await page.goForward();
  await expect(
    page.getByRole("heading", { name: "Writer", exact: true }),
  ).toBeVisible();
});

test("creation opens the returned account even outside the loaded page", async ({
  page,
}) => {
  await accessMock(page);
  await page.route("**/v1/accounts?*", (route) =>
    route.fulfill({
      json: {
        items: [owner],
        next_before: owner.id,
        previous_after: null,
        initialized: true,
      },
    }),
  );
  await page.goto(base);
  await page.getByRole("button", { name: "Create account", exact: true }).click();
  await page.getByLabel(/^Display name\s*\*?$/).fill("New account");
  await page.getByLabel(/^Username\s*\*?$/).fill("new.account");
  await page
    .getByLabel("Your current password")
    .fill("a private admin password");
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Create account" })
    .click();
  await page.getByLabel("I have saved this setup link.").check();
  await page.getByRole("button", { name: "Done" }).click();
  await expect(page).toHaveURL(
    /#accounts\/33333333-3333-3333-3333-333333333333\?/,
  );
  await expect(
    page.getByRole("heading", { name: "New account" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await expect(page.getByLabel(/^Label\s*\*?$/)).toBeVisible();
});

test("missing accounts show a route back and mismatched responses show no tokens", async ({
  page,
}) => {
  await accessMock(page);
  await page.goto(`${base}/00000000-0000-0000-0000-000000000000`);
  await expect(page.getByRole("alert")).toContainText("Account unavailable");
  await page.getByRole("link", { name: "Back to accounts" }).click();
  await expect(
    page.getByRole("button", { name: "Create account", exact: true }),
  ).toBeVisible();
  await page.route(`**/v1/accounts/${owner.id}`, (route) =>
    route.fulfill({ json: otherUser }),
  );
  await page.goto(`${base}/${owner.id}`);
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Issue token", exact: true }),
  ).toHaveCount(0);
});

test("forbidden detail refresh removes admin controls", async ({ page }) => {
  await accessMock(page);
  await page.goto(`${base}/${owner.id}`);
  await expect(
    page.getByRole("heading", { name: owner.display_name }),
  ).toBeVisible();
  await page.route("**/v1/session", (route) =>
    route.fulfill({ json: { ...session, role: "reader" } }),
  );
  await page.route(`**/v1/accounts/${owner.id}`, (route) =>
    route.fulfill({ status: 403, json: { error: "forbidden" } }),
  );
  await page.getByRole("button", { name: "Refresh accounts" }).click();
  await expect(page.getByRole("alert")).toHaveText("Admin access required.");
  await expect(
    page.getByRole("button", { name: "Issue token", exact: true }),
  ).toHaveCount(0);
});
