import { expect, test, type Page } from "@playwright/test";
import { accessMock, owner, rawToken } from "./access-fixture";
import { clientMock } from "./client-fixture";
import { envelope, failure, intercept, session } from "./command-fixture";
import { example, fillS3, storageMock } from "./storage-fixture";
import { maintenanceMock } from "./maintenance-fixture";

const root = "/api/admin/console/";
async function account(page: Page) {
  await page.goto(`${root}#accounts/${owner.id}`);
  await expect(
    page.getByRole("heading", { name: owner.display_name, exact: true }),
  ).toBeVisible();
}

test("fresh storage form preserves the S3 contract", async ({ page }) => {
  const fixture = await storageMock(page, []);
  await page.goto(`${root}#storages`);
  await page.getByRole("button", { name: "Add storage" }).click();
  await fillS3(page);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "new-s3", exact: true }),
  ).toBeVisible();
  expect(fixture.writes[0]).toMatchObject({
    command: "storage.create",
    input: {
      id: "new-s3",
      spec: {
        kind: "s3",
        force_relay: true,
        force_path_style: true,
        capacity_bytes: 1.5 * 1024 ** 4,
      },
    },
  });
  await expect(page.getByText("ephemeral-provider-secret")).toHaveCount(0);
});

test("unknown storage write is not retried and close refreshes registry", async ({
  page,
}) => {
  const fixture = await storageMock(page, []);
  let writes = 0;
  await intercept(page, "storage.create", (route) => {
    writes++;
    fixture.rows.set("new-s3", { ...example, id: "new-s3" });
    return route.abort();
  });
  await page.goto(`${root}#storages`);
  await page.getByRole("button", { name: "Add storage" }).click();
  await fillS3(page);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Save", exact: true }),
  ).toBeDisabled();
  await expect(page.getByLabel(/^Secret key/)).toHaveValue("");
  await page.getByRole("button", { name: "Close and review" }).click();
  await expect(
    page.getByRole("link", { name: "new-s3", exact: true }),
  ).toBeVisible();
  expect(writes).toBe(1);
});

test("delete conflict preserves registration and does not expose server errors", async ({
  page,
}) => {
  await storageMock(page);
  await intercept(page, "storage.delete", (route) =>
    route.fulfill({
      status: 409,
      json: { ...failure(409), message: "private upstream message" },
    }),
  );
  await page.goto(`${root}#storages/${example.id}`);
  await page.getByRole("button", { name: "Delete storage" }).click();
  await expect(
    page.getByRole("button", { name: "Confirm delete" }),
  ).toBeDisabled();
  await page.getByLabel("Storage ID to delete").fill(example.id);
  await page.getByRole("button", { name: "Confirm delete" }).click();
  await expect(page.getByRole("alert")).toContainText("clients or file");
  await expect(page.getByRole("alert")).not.toContainText("private upstream");
});

test("reader and demoted writer cannot use resource mutation forms", async ({
  page,
}) => {
  await storageMock(page);
  let role = "writer";
  await page.route("**/identity/v1/session", (route) =>
    route.fulfill({ json: { ...session, role } }),
  );
  await intercept(page, "storage.replace", (route) => {
    role = "reader";
    return route.fulfill({ status: 403, json: failure(403) });
  });
  await page.goto(`${root}#storages/${example.id}`);
  await page.getByRole("button", { name: "Edit storage" }).click();
  await page.getByLabel("Secret key (re-enter)").fill("ephemeral");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Edit storage" })).toHaveCount(
    0,
  );
  await expect(
    page.getByRole("button", { name: "Delete storage" }),
  ).toHaveCount(0);
});

test("expired session clears resource details", async ({ page }) => {
  await storageMock(page);
  await page.goto(`${root}#storages/${example.id}`);
  await expect(
    page.getByRole("heading", { name: example.id, exact: true }),
  ).toBeVisible();
  await intercept(page, "storage.show", (route) =>
    route.fulfill({ status: 401, json: failure(401) }),
  );
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByLabel("Password")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: example.id, exact: true }),
  ).toHaveCount(0);
});

test("metadata precedes usage and persists string labels", async ({ page }) => {
  await storageMock(page);
  let metadata: Record<string, string> = {};
  await intercept(page, "storage.metadata.show", (route) =>
    route.fulfill({
      json: envelope("storage.metadata.show", { id: example.id, metadata }),
    }),
  );
  await intercept(page, "storage.metadata.replace", (route) => {
    metadata = (
      route.request().postDataJSON() as {
        input: { metadata: Record<string, string> };
      }
    ).input.metadata;
    return route.fulfill({
      json: envelope("storage.metadata.replace", { id: example.id, metadata }),
    });
  });
  await page.goto(`${root}#storages/${example.id}`);
  await page.getByRole("button", { name: "Edit metadata" }).click();
  await page.getByLabel("Metadata JSON").fill('{"team":"home"}');
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("team: home", { exact: true })).toBeVisible();
  const metadataBox = await page
    .getByRole("region", { name: "Metadata", exact: true })
    .boundingBox();
  const usage = await page
    .getByRole("region", { name: "Usage", exact: true })
    .boundingBox();
  expect(metadataBox!.y).toBeLessThan(usage!.y);
});

test("client creation and S3 credential one-time display work", async ({
  page,
}) => {
  const fixture = await clientMock(page);
  await page.goto(`${root}#clients`);
  await page
    .getByRole("button", { name: "Create client", exact: true })
    .click();
  await page
    .getByRole("dialog")
    .getByLabel(/^Client ID/)
    .fill("new-client");
  await page
    .getByRole("dialog")
    .getByLabel(/^Storage/)
    .selectOption(example.id);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "new-client", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Create credential" }).click();
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveValue(
    "one-time-provider-independent-secret",
  );
  await expect(page.getByRole("button", { name: "Done" })).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByLabel("I have saved this credential.").check();
  await page.getByRole("button", { name: "Done" }).click();
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveCount(0);
  expect(fixture.clients.has("new-client")).toBe(true);
});

test("account token is validated, shown once and revoked", async ({ page }) => {
  const fixture = await accessMock(page);
  await account(page);
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page
    .getByRole("dialog")
    .getByLabel(/^Label/)
    .fill("CLI");
  await page.getByLabel("Current password").fill("fixture administrator password");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  await expect(page.getByLabel("Issued token", { exact: true })).toHaveValue(
    rawToken,
  );
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).not.toContain(rawToken);
  await page
    .getByLabel("I have saved this token. It is shown only once.")
    .check();
  await page.getByRole("button", { name: "Done" }).click();
  await page.getByRole("button", { name: "Revoke CLI", exact: true }).click();
  await page.getByLabel(/^Confirmation/).fill("CLI");
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(page.getByText("Revoked", { exact: true })).toBeVisible();
  expect(fixture.tokens[0].revoked_at).not.toBeNull();
});

test("last admin stays protected and self-demotion needs confirmation", async ({
  page,
}) => {
  await accessMock(page);
  await account(page);
  await page.getByRole("button", { name: "Change role" }).click();
  await page
    .getByRole("dialog")
    .getByLabel("Role", { exact: true })
    .selectOption("reader");
  await expect(
    page.getByRole("button", { name: "Save", exact: true }),
  ).toBeDisabled();
  await page.getByLabel("I understand this changes my access.").check();
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Keep an active Admin");
});

test("wrong-account token response fails closed", async ({ page }) => {
  await accessMock(page);
  await page.route("**/accounts/*/credentials", (route) =>
    route.request().method() === "GET"
      ? route.fallback()
      : route.fulfill({
          json: {
            token: rawToken,
            credential_id: "token",
            expires_at: "2099-01-01T00:00:00Z",
            account_id: "wrong",
          },
        }),
  );
  await account(page);
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  await page
    .getByRole("dialog")
    .getByLabel(/^Label/)
    .fill("CLI");
  await page.getByLabel("Current password").fill("fixture administrator password");
  await page.getByRole("button", { name: "Issue", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("The result is unknown");
  await expect(
    page.getByRole("button", { name: "Issue", exact: true }),
  ).toBeDisabled();
  await expect(page.getByLabel("Issued token", { exact: true })).toHaveCount(0);
});

test("activity is read-only and event details use a drawer", async ({
  page,
}) => {
  await maintenanceMock(page);
  await page.goto(`${root}#activity`);
  await page
    .getByRole("button", { name: "storage.create", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Event details" }),
  ).toBeVisible();
  await expect(
    page.getByRole("dialog").getByText("home-archive", { exact: false }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByRole("tab", { name: "Security events" }).click();
  await expect(
    page.getByRole("button", { name: "permission_denied" }),
  ).toBeVisible();
});

for (const width of [320, 768, 1440])
  for (const theme of ["light", "dark"] as const) {
    test(`fresh console responsive shell ${width} ${theme}`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 900 });
      await accessMock(page);
      await account(page);
      await page.getByRole("button", { name: "Theme", exact: true }).click();
      await page.getByRole("menuitem", { name: new RegExp(`^${theme}$`, "i") }).click();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      await page
        .getByRole("button", { name: "Issue token", exact: true })
        .click();
      await expect(page.getByRole("dialog")).toBeVisible();
      expect(
        await page
          .getByRole("dialog")
          .evaluate((element) => element.scrollWidth <= element.clientWidth),
      ).toBe(true);
      await page.getByRole("button", { name: "Cancel", exact: true }).click();
      if (width < 900) {
        await page.getByRole("button", { name: "Open navigation" }).click();
        await expect(
          page.getByRole("navigation", { name: "Main navigation" }),
        ).toBeVisible();
        await page.keyboard.press("Escape");
        await expect(
          page.getByRole("navigation", { name: "Main navigation" }),
        ).toBeHidden();
      }
      await page.screenshot({
        path: `test-results/fresh-${width}-${theme}.png`,
        fullPage: true,
      });
    });
  }
