import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { session } from "./command-fixture";

const exported = process.env.GROVE_TEST_OPENAPI;
const documents = exported ? JSON.parse(readFileSync(exported, "utf8")) as Record<string, object> : null;
const base = "/api/admin/console/#api";

async function identity(page: Page) {
  await page.route("**/api/admin/identity/v1/session", route => route.fulfill({ json: { ...session, role: "reader" } }));
}

test("documentation failure is explicit and can be retried", async ({ page }) => {
  await identity(page);
  await page.route("**/api/docs/*.json", route => route.fulfill({ status: 503, json: { error: "unavailable" } }));
  await page.goto(base);
  await expect(page.getByRole("heading", { name: "API docs", exact: true })).toBeVisible();
  await expect(page.getByRole("alert").first()).toContainText("The request could not be completed");
  await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
});

test.describe("Rust-generated Swagger documents", () => {
  test.skip(!documents, "Set GROVE_TEST_OPENAPI to the filegate openapi export");

  for (const built of [false, true]) {
    test(`all API surfaces render read-only without leaking styles or credentials (${built ? "production CSP" : "development"})`, async ({ page }) => {
      await identity(page);
      const violations: string[] = [];
      const externalRequests: string[] = [];
      const writes: string[] = [];
      page.on("console", msg => { if (msg.text().includes("Content Security Policy")) violations.push(msg.text()); });
      page.on("request", request => {
        const url = new URL(request.url());
        if (url.host !== "127.0.0.1:5179" && url.host !== "127.0.0.1:5180") externalRequests.push(url.href);
        if (request.method() !== "GET") writes.push(url.href);
      });
      await page.route("**/api/docs/*.json", async route => {
        expect(route.request().headers().authorization).toBeUndefined();
        expect(route.request().headers().cookie).toBeUndefined();
        const surface = new URL(route.request().url()).pathname.split("/").at(-1)?.split(".")[0] ?? "";
        await route.fulfill({ json: documents?.[surface] });
      });
      const url = built ? `http://127.0.0.1:5180${base}` : base;
      await page.goto(url);
      const font = await page.getByRole("heading", { name: "API docs", exact: true }).evaluate(element => getComputedStyle(element).fontFamily);
      await expect(page.locator(".swagger-ui .info .title")).toContainText("S3-compatible API");
      await expect(page.locator(".swagger-ui .opblock")).toHaveCount(5);
      await page.locator(".swagger-ui .opblock-summary-control").first().click();
      await expect(page.locator(".swagger-ui .opblock-body").first()).toBeVisible();
      await expect(page.getByRole("button", { name: "Try it out" })).toHaveCount(0);
      await expect(page.getByRole("button", { name: "Authorize", exact: true })).toHaveCount(0);
      await page.getByRole("tab", { name: "Management API", exact: true }).click();
      await expect(page.locator(".swagger-ui .info .title")).toContainText("Management API");
      await expect(page.locator(".swagger-ui .opblock")).toHaveCount(1);
      await page.locator(".swagger-ui .opblock-summary-control").click();
      await expect(page.locator(".swagger-ui .opblock-body")).toContainText('"command": "status"');
      await page.getByRole("tab", { name: "Native compatibility", exact: true }).click();
      await expect(page.locator(".swagger-ui .info .title")).toContainText("Native compatibility API");
      await expect(page.locator(".swagger-ui .opblock")).toHaveCount(6);
      expect(await page.getByRole("heading", { name: "API docs", exact: true }).evaluate(element => getComputedStyle(element).fontFamily)).toBe(font);
      expect(await page.locator(".swagger-ui .info .title").evaluate(element => getComputedStyle(element).fontFamily)).toBe(font);
      await page.setViewportSize({ width: 390, height: 844 });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      expect(violations).toEqual([]);
      expect(externalRequests).toEqual([]);
      expect(writes).toEqual([]);
      await page.screenshot({ path: `/tmp/grove-swagger-${built ? "production" : "development"}-mobile.png`, fullPage: true, animations: "disabled" });
      await page.setViewportSize({ width: 1440, height: 1000 });
      await page.getByRole("tab", { name: "S3 API", exact: true }).click();
      await expect(page.getByRole("tab", { name: "S3 API", exact: true })).toHaveAttribute("aria-selected", "true");
      await expect(page.locator(".swagger-ui .info .title")).toContainText("S3-compatible API");
      await page.screenshot({ path: `/tmp/grove-swagger-${built ? "production" : "development"}.png`, fullPage: true, animations: "disabled" });
    });
  }
});
