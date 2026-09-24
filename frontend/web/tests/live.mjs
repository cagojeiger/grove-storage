import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { storageChecks } from "./live-storages.mjs";
import { permissionChecks } from "./live-permissions.mjs";

let input = "";
for await (const chunk of process.stdin) input += chunk;
const fixture = JSON.parse(input);
const { origin, token, credentialId, database } = fixture;
const browser = await chromium.launch();
try {
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(origin + "/api/admin/console/");
  await page.getByLabel("개인 토큰").fill(token);
  await page.getByRole("button", { name: "로그인", exact: true }).click();
  await page.getByRole("heading", { name: "개요", exact: true }).waitFor();
  await page.getByText("등록된 저장소가 없습니다.").waitFor();
  await storageChecks(page, fixture);
  await permissionChecks(browser, page, origin);
  const cookie = (await context.cookies()).find(
    (cookie) => cookie.name === "__Host-grove_session",
  );
  assert(cookie?.secure && cookie.httpOnly && cookie.sameSite === "Strict");
  assert(!(await page.evaluate(() => document.cookie)).includes(cookie.value));
  assert(
    !(
      await page.evaluate(() =>
        JSON.stringify({ ...localStorage, ...sessionStorage }),
      )
    ).includes(token),
  );
  await page.reload();
  await page.getByRole("heading", { name: "개요", exact: true }).waitFor();
  const csrf = await page.evaluate(
    async () =>
      (await fetch("/api/admin/identity/v1/session", { method: "DELETE" })).status,
  );
  assert.equal(csrf, 403);
  await page.getByRole("button", { name: "로그아웃" }).click();
  await page.getByLabel("개인 토큰").waitFor();
  assert(
    !(await context.cookies()).some(
      (cookie) => cookie.name === "__Host-grove_session",
    ),
  );
  await page.reload();
  await page.getByLabel("개인 토큰").waitFor();
  console.log(
    "PASS real HTTPS login, Secure/HttpOnly cookie, reload, CSRF rejection, logout",
  );
  async function login() {
    await page.getByLabel("개인 토큰").fill(token);
    await page.getByRole("button", { name: "로그인", exact: true }).click();
    await page.getByRole("heading", { name: "console-live" }).waitFor();
  }
  await login();
  execFileSync(
    "docker",
    [
      "exec",
      database,
      "psql",
      "-U",
      "filegate",
      "-d",
      "filegate",
      "-v",
      "ON_ERROR_STOP=1",
      "-c",
      "UPDATE management.sessions SET created_at = now() - interval '1 day', expires_at = now() - interval '1 second' WHERE auth_method='token'",
    ],
    { timeout: 10000, stdio: "pipe" },
  );
  await page.getByRole("button", { name: "새로고침" }).click();
  await page.getByLabel("개인 토큰").waitFor();
  assert.equal(
    await page.getByRole("heading", { name: "console-live" }).count(),
    0,
  );
  await login();
  const revoked = await page.evaluate(async (id) => {
    const response = await fetch(`/api/admin/identity/v1/credentials/${id}`, {
      method: "DELETE", headers: { "X-Grove-CSRF": "1" },
    });
    return response.status;
  }, credentialId);
  assert.equal(revoked, 200);
  await page.getByRole("button", { name: "새로고침" }).click();
  await page.getByLabel("개인 토큰").waitFor();
  await page.getByLabel("개인 토큰").fill(token);
  await page.getByRole("button", { name: "로그인", exact: true }).click();
  await page.getByRole("alert").waitFor();
  assert.equal(await page.getByLabel("개인 토큰").inputValue(), "");
  console.log(
    "PASS real storage overview, session expiry, token revocation, and private cache removal",
  );
  assert.deepEqual(errors, []);
} finally {
  await browser.close();
}
