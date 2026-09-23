import { test, expect, Page } from "@playwright/test";

const root = "/api/admin/console/";
async function mock(page: Page, signedIn = true) {
  let loggedIn = signedIn;
  await page.route("**/readyz", (route) =>
    route.fulfill({ json: { status: "ready" } }),
  );
  await page.route("**/api/admin/v1/**", async (route) => {
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
        json: loggedIn ? { principal: "admin", credential_id: "test" } : {},
      });
    } else if (path.endsWith("/clients"))
      await route.fulfill({ json: [{ id: "notegate" }] });
    else
      await route.fulfill({
        json: [
          {
            storage_id: "home-storage-long-identifier",
            kind: "s3",
            capacity_bytes: 1024 ** 4,
            active_bytes: 1024 ** 3 * 128,
            reserved_bytes: 1024 ** 2 * 40,
            purge_pending_bytes: 0,
            remaining_bytes: 1024 ** 3 * 896,
            active_files: 1240,
          },
        ],
      });
  });
}

test("login clears token; logout removes overview", async ({ page }) => {
  await mock(page, false);
  await page.goto(root);
  await page.getByLabel("관리자 토큰").fill("test-token");
  await page.getByRole("button", { name: "로그인", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "개요", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    ),
  ).not.toContain("test-token");
  await page.getByRole("button", { name: "로그아웃" }).click();
  await expect(page.getByLabel("관리자 토큰")).toHaveValue("");
  await expect(page.getByText("home-storage-long-identifier")).toHaveCount(0);
});

test("429 clears input and honors Retry-After", async ({ page }) => {
  await mock(page, false);
  await page.goto(root);
  await expect(page.getByLabel("관리자 토큰")).toBeVisible();
  await page.route("**/session", (route) =>
    route.request().method() === "POST"
      ? route.fulfill({
          status: 429,
          headers: { "Retry-After": "2" },
          json: {},
        })
      : route.fallback(),
  );
  await page.getByLabel("관리자 토큰").fill("do-not-persist");
  await page.getByRole("button", { name: "로그인", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("요청이 많습니다");
  await expect(page.getByLabel("관리자 토큰")).toHaveValue("");
  await expect(
    page.getByRole("button", { name: /초 후 재시도/ }),
  ).toBeDisabled();
});

test("overview 401 removes cached private data", async ({ page }) => {
  await mock(page);
  await page.goto(root);
  await expect(page.getByText("home-storage-long-identifier")).toBeVisible();
  await page.route("**/usage", (route) =>
    route.fulfill({ status: 401, json: {} }),
  );
  await page.getByRole("button", { name: "새로고침" }).click();
  await expect(page.getByLabel("관리자 토큰")).toBeVisible();
  await expect(page.getByText("home-storage-long-identifier")).toHaveCount(0);
});

test("empty, API failure, retry, and logout failure", async ({ page }) => {
  await mock(page);
  await page.route("**/usage", (route) => route.fulfill({ json: [] }));
  await page.goto(root);
  await expect(page.getByText("등록된 저장소가 없습니다.")).toBeVisible();
  await page.route("**/usage", (route) =>
    route.fulfill({ status: 500, json: {} }),
  );
  await page.getByRole("button", { name: "새로고침" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await page.route("**/usage", (route) => route.fulfill({ json: [] }));
  await page.getByRole("button", { name: "새로고침" }).click();
  await expect(page.getByText("등록된 저장소가 없습니다.")).toBeVisible();
  await page.route("**/session", (route) =>
    route.fulfill({ status: 500, json: {} }),
  );
  await page.getByRole("button", { name: "로그아웃" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "개요", exact: true }),
  ).toBeVisible();
});

for (const width of [320, 390, 768, 1024, 1440]) {
  for (const theme of ["light", "dark"])
    test(`${width}px ${theme} layout`, async ({ page }) => {
      await page.setViewportSize({ width, height: 960 });
      await mock(page);
      await page.goto(root);
      await page.getByLabel("화면 테마").selectOption(theme);
      await expect(
        page.getByText("home-storage-long-identifier"),
      ).toBeVisible();
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
  await page.getByLabel("화면 테마").selectOption("dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});
