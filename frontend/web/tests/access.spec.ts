import { expect, test } from "@playwright/test";
import { accessMock, otherUser, owner, rawToken, root } from "./access-fixture";
import { session } from "./command-fixture";

test("create a User without Agent or owner fields", async ({ page }) => {
  const { writes } = await accessMock(page);
  await page.goto(root);
  await expect(
    page.getByRole("link", { name: "Agents", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Create user", exact: true }).click();
  await expect(page.getByLabel("Owner", { exact: true })).toHaveCount(0);
  await expect(page.getByLabel("Role", { exact: true }).locator("option")).toHaveText([
    "Reader", "Writer", "Admin",
  ]);
  await page.getByLabel("Name", { exact: true }).fill("Writer");
  await page.getByLabel("Role", { exact: true }).selectOption("writer");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(writes.map((w) => w.body)).toEqual([
    { kind: "user", display_name: "Writer", role: "writer" },
  ]);
});

test("issue one-time token, discard it and revoke with confirmation", async ({
  page,
}) => {
  const { writes } = await accessMock(page);
  await page.goto(root);
  await page
    .getByRole("button", { name: /Home administrator.*Active/ })
    .click();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("CLI automation");
  await page.getByLabel("Expires in days").fill("7");
  const sent = page.waitForRequest((r) => r.method() === "POST");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  expect((await sent).headers()["x-grove-csrf"]).toBe("1");
  await expect(
    page.getByRole("textbox", { name: "Issued token", exact: true }),
  ).toHaveValue(rawToken);
  await expect(
    page.getByRole("button", { name: "Done", exact: true }),
  ).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).not.toContain(rawToken);
  await page
    .getByLabel("I have saved this token. It is shown only once.")
    .check();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Issued token", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Revoke CLI automation" }).click();
  await expect(
    page.getByRole("button", { name: "Revoke", exact: true }),
  ).toBeDisabled();
  await page.getByLabel("Revoke CLI automation and its sessions").check();
  await page.getByRole("button", { name: "Revoke", exact: true }).click();
  await expect(page.getByText("Revoked", { exact: true })).toBeVisible();
  expect(writes).toHaveLength(2);
});

test("last Admin conflict remains visible; destructive change requires name", async ({
  page,
}) => {
  await accessMock(page);
  await page.goto(root);
  await page
    .getByRole("button", { name: /Home administrator.*Active/ })
    .click();
  await page
    .getByRole("button", { name: "Delete account", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Confirm", exact: true }),
  ).toBeDisabled();
  await page.getByLabel("Confirm account name").fill(owner.display_name);
  await page.getByRole("checkbox", { name: "I understand my current session will end." }).check();
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Keep an active Admin");
});

for (const failure of ["lost", "unknown", "malformed"])
  test(`token ${failure} outcome blocks duplicate issuance`, async ({
    page,
  }) => {
    await accessMock(page);
    let attempts = 0;
    await page.route("**/accounts/*/credentials", async (route) => {
      if (route.request().method() === "GET") return route.fallback();
      attempts++;
      if (failure === "lost") return route.abort();
      return route.fulfill({
        status: failure === "unknown" ? 503 : 201,
        json:
          failure === "unknown"
            ? { error: "outcome_unknown" }
            : { token: rawToken },
      });
    });
    await page.goto(root);
    await page
      .getByRole("button", { name: /Home administrator.*Active/ })
      .click();
    await page
      .getByRole("button", { name: "Issue token", exact: true })
      .click();
    await page.getByLabel("Label", { exact: true }).fill("Uncertain");
    await page.getByRole("button", { name: "Issue", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("outcome is unknown");
    await expect(
      page.getByRole("button", { name: "Issue", exact: true }),
    ).toBeDisabled();
    expect(attempts).toBe(1);
  });

test("Reader cannot discover Access or request accounts", async ({ page }) => {
  const { writes } = await accessMock(page);
  await page.route("**/v1/session", (route) =>
    route.fulfill({ json: { ...session, role: "reader" } }),
  );
  let reads = 0;
  await page.route("**/v1/accounts?*", (route) => {
    reads++;
    return route.abort();
  });
  await page.goto(root);
  await expect(
    page.getByRole("link", { name: "Accounts", exact: true }),
  ).toHaveCount(0);
  await expect(page.getByRole("alert")).toHaveText("Admin access required.");
  expect(reads).toBe(0);
  expect(writes).toHaveLength(0);
});

test("account paging can return to earlier Users", async ({ page }) => {
  await accessMock(page);
  await page.route("**/v1/accounts?*", (route) =>
    route.fulfill({
      json: new URL(route.request().url()).searchParams.has("before")
        ? { items: [owner], next_before: null, previous_after: owner.id, initialized: true }
        : { items: [otherUser], next_before: otherUser.id, previous_after: null, initialized: true },
    }),
  );
  await page.goto(root);
  await page.getByRole("button", { name: "Next page" }).click();
  await expect(
    page.getByRole("button", { name: /Home administrator.*Active/ }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Previous page" }).click();
  await expect(
    page.getByRole("button", { name: /Writer.*Active/ }),
  ).toBeVisible();
});
