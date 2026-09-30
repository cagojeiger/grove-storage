import { test, expect } from "@playwright/test";
import { envelope, intercept } from "./command-fixture";

const root = "http://127.0.0.1:5180/api/admin/console/";

test("built console loads with restrictive browser headers", async ({
  page,
}) => {
  const errors: string[] = [];
  const assets: string[] = [];
  page.on("request", (request) => assets.push(request.url()));
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (
      message.type() === "error" &&
      /Content Security Policy|Refused to apply|Refused to load/.test(
        message.text(),
      )
    )
      errors.push(message.text());
  });
  const response = await page.goto(root);
  const headers = response?.headers();
  expect(headers?.["content-security-policy"]).not.toContain("unsafe-inline");
  expect(headers?.["x-frame-options"]).toBe("DENY");
  expect(headers?.["x-content-type-options"]).toBe("nosniff");
  expect(headers?.["referrer-policy"]).toBe("no-referrer");
  expect(headers?.["cache-control"]).toBe("no-store");
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password").fill("invalid-preview-password");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Overview", exact: true }),
  ).toHaveCount(0);
  await page.getByLabel("Password").fill("a private phrase for preview");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Overview", exact: true }),
  ).toBeVisible();
  expect(assets.some((url) => url.includes("mui-x-grid"))).toBe(false);
  expect(assets.some((url) => url.includes("mui-x-charts"))).toBe(false);
  await page.getByRole("link", { name: "Storage", exact: true }).click();
  await expect(
    page.getByRole("grid", { name: "Storage", exact: true }),
  ).toBeVisible();
  expect(assets.some((url) => url.includes("mui-x-grid"))).toBe(true);
  await page.getByRole("button", { name: "Register", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Register storage", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("link", { name: "home-archive", exact: true }).click();
  await page
    .getByRole("button", { name: "Edit metadata", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.locator(".MuiDialog-paper")).toHaveCSS(
    "background-color",
    "rgb(255, 255, 255)",
  );
  await page.keyboard.press("Escape");
  await page.getByRole("link", { name: "Clients", exact: true }).click();
  await expect(
    page.getByRole("grid", { name: "Clients", exact: true }),
  ).toBeVisible();
  const nonce = await page
    .locator('meta[name="csp-nonce"]')
    .getAttribute("content");
  const gridStyles = await page
    .locator("style")
    .evaluateAll((styles) =>
      styles
        .filter((style) => style.textContent?.includes("MuiDataGrid"))
        .map((style) => style.nonce),
    );
  expect(gridStyles.length).toBeGreaterThan(0);
  expect(gridStyles.every((value) => value === nonce)).toBe(true);
  await intercept(page, "usage.history", (r) =>
    r.fulfill({
      json: envelope("usage.history", [
        {
          day: "2026-09-24",
          storage_id: "home-archive",
          client_id: "notegate",
          active_files: 1,
          active_bytes: 1024,
        },
        {
          day: "2026-09-25",
          storage_id: "home-archive",
          client_id: "notegate",
          active_files: 2,
          active_bytes: 2048,
        },
      ]),
    }),
  );
  await page.goto(`${root}#usage`);
  await expect(
    page
      .getByLabel("Active data by day", { exact: true })
      .locator(".MuiLineChart-line"),
  ).toHaveAttribute("d", /^M/);
  expect(assets.some((url) => url.includes("mui-x-charts"))).toBe(true);
  expect(errors).toEqual([]);
});

test("built MUI styles use a fresh nonce without web font downloads", async ({
  page,
  request,
}) => {
  const response = await page.goto(root);
  const nonce = await page
    .locator('meta[name="csp-nonce"]')
    .getAttribute("content");
  expect(nonce).toMatch(/^[A-Za-z0-9+/]{32}$/);
  expect(response?.headers()["content-security-policy"]).toContain(
    `style-src-elem 'self' 'nonce-${nonce}'`,
  );
  expect(response?.headers()["content-security-policy"]).toContain(
    "style-src-attr 'none'",
  );
  await expect(page.locator("style[data-emotion]").first()).toBeAttached();
  const nonces = await page
    .locator("style[data-emotion]")
    .evaluateAll((styles) =>
      styles.map((style) => (style as HTMLStyleElement).nonce),
    );
  expect(nonces.length).toBeGreaterThan(0);
  expect(nonces.every((value) => value === nonce)).toBe(true);
  await page.evaluate(() => document.fonts.ready);
  await expect(page.locator("body")).toHaveCSS(
    "font-family",
    /^-apple-system, (?:BlinkMacSystemFont|"?system-ui"?), "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif$/,
  );
  const fonts = await page.evaluate(() =>
    performance
      .getEntriesByType("resource")
      .filter((entry) => /\.woff2?$/.test(entry.name))
      .map((entry) => entry.name),
  );
  expect(fonts).toEqual([]);
  const second = await request.get(root);
  expect(second.headers()["content-security-policy"]).not.toContain(
    `nonce-${nonce}`,
  );
});

test("built console rejects styles without its nonce", async ({ page }) => {
  await page.goto(root);
  await page.evaluate(() => {
    document.addEventListener(
      "securitypolicyviolation",
      (event) => {
        document.documentElement.dataset.blockedDirective =
          event.effectiveDirective;
      },
      { once: true },
    );
    const style = document.createElement("style");
    style.textContent = "body { --injected-style: yes; }";
    document.head.append(style);
  });
  await expect(page.locator("html")).toHaveAttribute(
    "data-blocked-directive",
    "style-src-elem",
  );
  expect(
    await page.evaluate(() =>
      getComputedStyle(document.body).getPropertyValue("--injected-style"),
    ),
  ).toBe("");
});

test("built console blocks injected inline scripts", async ({ page }) => {
  await page.goto(root);
  await page.evaluate(() => {
    document.addEventListener(
      "securitypolicyviolation",
      (event) => {
        document.documentElement.dataset.blockedDirective =
          event.effectiveDirective;
      },
      { once: true },
    );
    const script = document.createElement("script");
    script.textContent =
      'document.documentElement.dataset.injected = "executed"';
    document.head.append(script);
  });
  await expect(page.locator("html")).toHaveAttribute(
    "data-blocked-directive",
    /script-src/,
  );
  await expect(page.locator("html")).not.toHaveAttribute("data-injected");
});

test("built console blocks external connections before transmission", async ({
  page,
}) => {
  const external: string[] = [];
  page.on("request", (request) => {
    if (request.url().startsWith("https://outside.invalid/"))
      external.push(request.url());
  });
  await page.goto(root);
  await page.evaluate(async () => {
    document.addEventListener(
      "securitypolicyviolation",
      (event) => {
        document.documentElement.dataset.blockedDirective =
          event.effectiveDirective;
      },
      { once: true },
    );
    try {
      await fetch("https://outside.invalid/collect", {
        method: "POST",
        body: "fixture-only",
      });
    } catch {
      // CSP must reject this request before the browser sends it.
    }
  });
  await expect(page.locator("html")).toHaveAttribute(
    "data-blocked-directive",
    "connect-src",
  );
  expect(external).toEqual([]);
});

test("built console cannot be embedded in an iframe", async ({ page }) => {
  let blocked = false;
  page.on("console", (message) => {
    if (message.text().includes("frame-ancestors")) blocked = true;
  });
  await page.route("**/frame-test", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: `<iframe src="${root}"></iframe>`,
    }),
  );
  await page.goto("http://127.0.0.1:5180/frame-test");
  await expect.poll(() => blocked).toBe(true);
  await expect(
    page
      .frameLocator("iframe")
      .getByRole("heading", { name: "Overview", exact: true }),
  ).toHaveCount(0);
});
