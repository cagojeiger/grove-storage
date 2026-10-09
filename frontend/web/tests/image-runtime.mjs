import assert from "node:assert/strict";
import { chromium, expect } from "@playwright/test";
import { loginWithPassword } from "./live-auth.mjs";

let input = "";
for await (const chunk of process.stdin) input += chunk;
const { origin, password, reportDir } = JSON.parse(input);
const browser = await chromium.launch();
try {
  const context = await browser.newContext({ ignoreHTTPSErrors: true, viewport: { width: 1440, height: 1000 } });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("console", message => {
    if (message.type() === "error" && /Content Security Policy|Refused to (apply|load)/.test(message.text())) errors.push(message.text());
  });
  const root = `${origin}/api/admin/console/`;
  const first = await page.goto(`${origin}/`);
  await expect(page).toHaveURL(root);
  const csp = first.headers()["content-security-policy"];
  assert(!csp.includes("unsafe-inline"));
  assert.equal(first.headers()["cache-control"], "no-store");
  const nonce = await page.locator('meta[name="csp-nonce"]').getAttribute("content");
  assert(nonce && nonce !== "__GROVE_CSP_NONCE__" && csp.includes(`nonce-${nonce}`));
  await expect(page.locator("style[data-emotion]").first()).toBeAttached();
  await page.screenshot({ path: `${reportDir}/sign-in.png` });
  await loginWithPassword(page, "owner", password);
  const cookie = (await context.cookies()).find(value => value.name === "__Host-grove_session");
  assert(cookie?.secure && cookie.httpOnly && cookie.sameSite === "Strict");
  for (const name of ["Storage", "Clients", "Accounts", "Activity"]) {
    await page.getByRole("link", { name, exact: true }).click();
    await expect(page.getByRole("heading", { name, exact: true, level: 1 })).toBeVisible();
  }
  await page.getByRole("link", { name: "API docs", exact: true }).click();
  await expect(page.locator(".swagger-ui")).toBeVisible();
  await expect(page.locator(".swagger-ui .opblock").first()).toBeVisible();
  await page.getByRole("tab", { name: "Management API", exact: true }).click();
  await expect(page.locator(".swagger-ui .opblock").first()).toBeVisible();
  await page.screenshot({ path: `${reportDir}/api-docs.png` });
  await page.getByRole("link", { name: "My account", exact: true }).click();
  await expect(page.getByRole("tab", { name: "Profile", exact: true })).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: `${reportDir}/my-account-mobile.png` });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await expect(page.getByRole("button", { name: "Sign in", exact: true })).toBeVisible();
  const second = await page.reload();
  assert.notEqual(second.headers()["content-security-policy"], csp);
  assert.equal((await context.request.get(`${origin}/test-client/object.html`)).status(), 404);
  assert.deepEqual(errors, []);
  console.log("PASS packaged MUI console: root redirect, CSP nonce, login/logout, secure cookie, all sections, Swagger, desktop/mobile");
} finally {
  await browser.close();
}
