import { test, expect, Page } from "@playwright/test";
import { commandUrl, envelope, failure, intercept, session } from "./command-fixture";

const root = "/api/admin/console/";

test.describe("English default", () => {
  test.use({ locale: "de-DE" });
  test("document, sign-in, labels and numbers stay English", async ({ page }) => {
    await mock(page, false);
    await page.goto(root);
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await page.getByLabel("Account token").fill("fixture-token");
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await expect(page.getByText("Active files: 1,240", { exact: false })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Overview", exact: true })).toBeVisible();
    expect(await page.locator("body").innerText()).not.toMatch(/[\uAC00-\uD7A3]/);
  });
});

async function mock(page: Page, signedIn = true) {
  let loggedIn = signedIn;
  await page.route("**/readyz", (route) =>
    route.fulfill({ json: { status: "ready" } }),
  );
  await page.route("**/api/admin/identity/v1/session", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith("/session")) {
      if (route.request().method() === "POST") loggedIn = true;
      if (route.request().method() === "DELETE") {
        loggedIn = false;
        await route.fulfill({ status: 204 });
        return;
      }
      await route.fulfill({
        status: loggedIn ? 200 : 401,
        json: loggedIn ? session : {},
      });
    }
  });
  await page.route(commandUrl, async (route) => {
    const { command } = route.request().postDataJSON() as { command: string };
    if (command === "client.list")
      await route.fulfill({ json: envelope(command, ["notegate"]) });
    else if (command === "usage.clients")
      await route.fulfill({ json: envelope(command, [{ client_id: "notegate", storage_id: "home-storage-long-identifier", active_files: 1240, active_bytes: 1024 ** 3 * 128 }]) });
    else
      await route.fulfill({
        json: envelope(command, [
          {
            storage_id: "home-storage-long-identifier",
            kind: "s3",
            capacity_bytes: 1024 ** 4,
            active_bytes: 1024 ** 3 * 128,
            reserved_bytes: 1024 ** 2 * 40,
            purge_pending_bytes: 0,
            remaining_bytes: 1024 ** 3 * 896,
            active_files: 1240,
            reserved_files: 0,
            purge_pending_files: 0,
          },
        ]),
      });
  });
}

test("login clears token; logout removes overview", async ({ page }) => {
  await mock(page, false);
  await page.goto(root);
  await page.getByLabel("Account token").fill("test-token");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Overview", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).not.toContain("test-token");
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(page.getByLabel("Account token")).toHaveValue("");
  await expect(page.getByText("home-storage-long-identifier")).toHaveCount(0);
});

for (const status of [307, 308]) {
  test(`login rejects ${status} without forwarding its token`, async ({ page }) => {
    await mock(page, false);
    const forwarded: string[] = [];
    page.on("request", (request) => {
      if (new URL(request.url()).pathname === "/redirect-target")
        forwarded.push(request.postData() ?? "");
    });
    await page.route("**/api/admin/identity/v1/session", async (route) => {
      if (route.request().method() === "POST") {
        await route.fulfill({ status, headers: { Location: "/redirect-target" } });
      } else {
        await route.fallback();
      }
    });
    await page.goto(root);
    await page.getByLabel("Account token").fill("fixture-secret-not-for-redirect");
    await page.getByRole("button", { name: "Sign in", exact: true }).click();
    await expect(
      page.getByRole("alert").or(page.getByRole("heading", { name: "Overview", exact: true })),
    ).toBeVisible();
    expect(forwarded).toEqual([]);
    await expect(page.getByRole("alert")).toContainText("Unable to connect to the server");
    await expect(page.getByLabel("Account token")).toHaveValue("");
  });
}

test("429 clears input and honors Retry-After", async ({ page }) => {
  await mock(page, false);
  await page.goto(root);
  await expect(page.getByLabel("Account token")).toBeVisible();
  await page.route("**/session", (route) =>
    route.request().method() === "POST"
      ? route.fulfill({
          status: 429,
          headers: { "Retry-After": "2" },
          json: {},
        })
      : route.fallback(),
  );
  await page.getByLabel("Account token").fill("do-not-persist");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Too many sign-in attempts");
  await expect(page.getByLabel("Account token")).toHaveValue("");
  await expect(
    page.getByRole("button", { name: /Retry in/ }),
  ).toBeDisabled();
});

test("overview 401 removes cached private data", async ({ page }) => {
  await mock(page);
  await page.goto(root);
  await expect(page.getByText("home-storage-long-identifier")).toBeVisible();
  await intercept(page, "usage.storages", (route) =>
    route.fulfill({ status: 401, json: failure(401) }),
  );
  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(page.getByLabel("Account token")).toBeVisible();
  await expect(page.getByText("home-storage-long-identifier")).toHaveCount(0);
});

test("empty, API failure, retry, and logout failure", async ({ page }) => {
  await mock(page);
  await intercept(page, "usage.storages", (route) => route.fulfill({ json: envelope("usage.storages", []) }));
  await page.goto(root);
  await expect(page.getByText("No storage registered.")).toBeVisible();
  await intercept(page, "usage.storages", (route) =>
    route.fulfill({ status: 500, json: failure(500) }),
  );
  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await intercept(page, "usage.storages", (route) => route.fulfill({ json: envelope("usage.storages", []) }));
  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(page.getByText("No storage registered.")).toBeVisible();
  await page.route("**/session", (route) =>
    route.fulfill({ status: 500, json: {} }),
  );
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Overview", exact: true }),
  ).toBeVisible();
});

for (const width of [320, 390, 768, 1024, 1440]) {
  for (const theme of ["light", "dark"])
    test(`${width}px ${theme} layout`, async ({ page }) => {
      await page.setViewportSize({ width, height: 960 });
      await mock(page);
      await page.goto(root);
      await page.getByLabel("Theme").selectOption(theme);
      await expect(
        page.getByText("home-storage-long-identifier"),
      ).toBeVisible();
      await expect(page.getByText("Admin · Management")).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      expect(
        await page
          .locator(".brand img")
          .evaluate((img: HTMLImageElement) => img.naturalWidth > 0),
      ).toBe(true);
      await page.screenshot({
        path: `test-results/overview-${width}-${theme}.png`,
        fullPage: true,
      });
    });
}

test("system theme follows OS and explicit selection persists", async ({
  page,
}) => {
  await mock(page);
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto(root);
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.getByLabel("Theme").selectOption("dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});

for (const theme of ["light", "dark"] as const) {
  test(`${theme} select options have an explicit matching surface`, async ({ page }) => {
    await mock(page);
    await page.emulateMedia({ colorScheme: theme === "dark" ? "light" : "dark" });
    await page.goto(root);
    await page.getByLabel("Theme").selectOption(theme);
    const colors = theme === "dark"
      ? { text: "rgb(240, 240, 243)", surface: "rgb(32, 33, 38)" }
      : { text: "rgb(35, 37, 43)", surface: "rgb(255, 255, 255)" };
    await expect(page.getByLabel("Theme")).toHaveCSS("color-scheme", theme);
    for (const option of await page.getByLabel("Theme").locator("option").all()) {
      await expect(option).toHaveCSS("color", colors.text);
      await expect(option).toHaveCSS("background-color", colors.surface);
    }
  });
}
