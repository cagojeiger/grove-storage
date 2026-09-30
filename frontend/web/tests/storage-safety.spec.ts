import { test, expect } from "@playwright/test";
import { example, fillS3, root, storageMock } from "./storage-fixture";
import { failure, intercept } from "./command-fixture";

test("zero file count does not bypass delete 409; refreshed registry is shown", async ({
  page,
}) => {
  const { reads } = await storageMock(page);
  await page.goto(root);
  await page.getByRole("link", { name: new RegExp(example.id) }).click();
  await expect(page.getByText("Files: 0").first()).toBeVisible();
  await intercept(page, "storage.delete", (route) =>
      route.fulfill({ status: 409, json: { ...failure(409), message: "secret-server-error" } }),
  );
  await page.getByRole("button", { name: "Delete storage" }).click();
  await page.getByLabel("Storage ID to delete").fill(example.id);
  await page.getByRole("button", { name: "Confirm delete" }).click();
  await expect(page.getByRole("alert")).toContainText("clients or file");
  await expect(page.getByRole("alert")).not.toContainText(
    "secret-server-error",
  );
  await expect(page.getByLabel("Storage ID to delete")).toHaveValue("");
  await page.getByRole("button", { name: "Cancel" }).click();
  await page.getByRole("link", { name: "Storage", exact: true }).first().click();
  await expect(
    page.getByRole("link", { name: new RegExp(example.id) }),
  ).toBeVisible();
  expect(reads.list).toBeGreaterThan(1);
});

test("replace conflict clears secret and retains editable nonsecret settings", async ({
  page,
}) => {
  await storageMock(page);
  await page.goto(`${root}/${example.id}`);
  await page.getByRole("button", { name: "Edit storage" }).click();
  await page.getByLabel("Secret key (re-enter)").fill("never-retain");
  await intercept(page, "storage.replace", (route) =>
      route.fulfill({ status: 409, json: failure(409) }),
  );
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "address cannot be changed",
  );
  await expect(page.getByLabel("Secret key (re-enter)")).toHaveValue("");
  await expect(page.getByLabel("Endpoint", { exact: true })).toHaveValue(
    example.endpoint!,
  );
});

test("lost response blocks resubmission and refreshes instead of retrying", async ({
  page,
}) => {
  const { reads, rows } = await storageMock(page, []);
  let posts = 0;
  await intercept(page, "storage.create", (route) => {
    posts++;
    rows.set("new-s3", { ...example, id: "new-s3" });
    return route.abort("connectionreset");
  });
  await page.goto(root);
  await page.getByRole("button", { name: "Register", exact: true }).click();
  await fillS3(page);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "change outcome is unconfirmed",
  );
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveValue("");
  await expect(
    page.getByRole("button", { name: "Save", exact: true }),
  ).toBeDisabled();
  expect(posts).toBe(1);
  await page.getByRole("button", { name: "Close and review" }).click();
  await expect(page.getByRole("link", { name: /new-s3/ })).toBeVisible();
  expect(reads.list).toBeGreaterThan(1);
});

test("storage read 401 returns to login without cached detail", async ({
  page,
}) => {
  await storageMock(page);
  await page.goto(`${root}/${example.id}`);
  await expect(page.getByRole("region", { name: "Storage settings" })).toBeVisible();
  await intercept(page, "storage.show", (route) =>
    route.fulfill({ status: 401, json: failure(401) }),
  );
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByLabel("Password")).toBeVisible();
  await expect(page.getByRole("heading", { name: example.id })).toHaveCount(0);
});

for (const method of ["POST", "PUT", "DELETE"]) {
  test(`${method} 401 removes all private views and form secrets`, async ({
    page,
  }) => {
    await storageMock(page);
    await page.goto(method === "POST" ? root : `${root}/${example.id}`);
    if (method === "DELETE") {
      await page.getByRole("button", { name: "Delete storage" }).click();
      await page.getByLabel("Storage ID to delete").fill(example.id);
    } else if (method === "POST") {
      await page.getByRole("button", { name: "Register", exact: true }).click();
      await fillS3(page);
    } else {
      await page.getByRole("button", { name: "Edit storage" }).click();
      await page.getByLabel("Secret key (re-enter)").fill("secret");
    }
    const command = { POST: "storage.create", PUT: "storage.replace", DELETE: "storage.delete" }[method]!;
    await intercept(page, command, (route) =>
        route.fulfill({ status: 401, json: failure(401) }),
    );
    await page
      .getByRole("button", {
        name: method === "DELETE" ? "Confirm delete" : "Save",
        exact: true,
      })
      .click();
    await expect(page.getByLabel("Password")).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page.getByRole("heading", { name: example.id })).toHaveCount(
      0,
    );
  });
}

test("pending write clears secret, prevents double submit and disables cancel", async ({
  page,
}) => {
  await storageMock(page);
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  let posts = 0;
  await intercept(page, "storage.create", async (route) => {
    posts++;
    await pending;
    return route.fulfill({ status: 400, json: failure(400) });
  });
  await page.goto(root);
  await page.getByRole("button", { name: "Register", exact: true }).click();
  await fillS3(page);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("button", { name: "Saving..." })).toBeDisabled();
  await expect(page.getByLabel("Secret key", { exact: true })).toHaveValue("");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("heading", { name: "Register storage" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Cancel", exact: true })).toBeDisabled();
  expect(posts).toBe(1);
  release();
  await expect(page.getByRole("alert")).toContainText("storage access");
});
