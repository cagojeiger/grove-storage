import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
import { execFileSync, spawnSync } from "node:child_process";

let input = "";
for await (const chunk of process.stdin) input += chunk;
const { origin, token, credentialId, database, server, serverEnv, objects } =
  JSON.parse(input);
const browser = await chromium.launch();
try {
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(origin + "/api/admin/console/");
  await page.getByLabel("관리자 토큰").fill(token);
  await page.getByRole("button", { name: "로그인", exact: true }).click();
  await page.getByRole("heading", { name: "개요", exact: true }).waitFor();
  await page.getByText("등록된 저장소가 없습니다.").waitFor();
  const registered = await page.evaluate(async (root) => {
    const response = await fetch("/api/admin/v1/storages", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-FileGate-CSRF": "1" },
      body: JSON.stringify({
        id: "console-live",
        kind: "fs",
        root_path: root,
        capacity_bytes: 1073741824,
      }),
    });
    return response.status;
  }, objects);
  assert.equal(registered, 201);
  await page.getByRole("button", { name: "새로고침" }).click();
  await page.getByRole("heading", { name: "console-live" }).waitFor();
  const cookie = (await context.cookies()).find(
    (cookie) => cookie.name === "__Host-filegate_session",
  );
  assert(cookie?.secure && cookie.httpOnly && cookie.sameSite === "Strict");
  assert(!(await page.evaluate(() => document.cookie)).includes("fgss_"));
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
      (await fetch("/api/admin/v1/session", { method: "DELETE" })).status,
  );
  assert.equal(csrf, 403);
  await page.getByRole("button", { name: "로그아웃" }).click();
  await page.getByLabel("관리자 토큰").waitFor();
  assert(
    !(await context.cookies()).some(
      (cookie) => cookie.name === "__Host-filegate_session",
    ),
  );
  await page.reload();
  await page.getByLabel("관리자 토큰").waitFor();
  console.log(
    "PASS real HTTPS login, Secure/HttpOnly cookie, reload, CSRF rejection, logout",
  );
  async function login() {
    await page.getByLabel("관리자 토큰").fill(token);
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
      "UPDATE admin_sessions SET expires_at = now() - interval '1 second'",
    ],
    { timeout: 10000, stdio: "pipe" },
  );
  await page.getByRole("button", { name: "새로고침" }).click();
  await page.getByLabel("관리자 토큰").waitFor();
  assert.equal(
    await page.getByRole("heading", { name: "console-live" }).count(),
    0,
  );
  await login();
  const revoked = spawnSync(
    server,
    ["admin", "token", "revoke", credentialId, "--yes"],
    { env: serverEnv, timeout: 10000, stdio: "pipe" },
  );
  assert.equal(revoked.status, 0);
  await page.getByRole("button", { name: "새로고침" }).click();
  await page.getByLabel("관리자 토큰").waitFor();
  await page.getByLabel("관리자 토큰").fill(token);
  await page.getByRole("button", { name: "로그인", exact: true }).click();
  await page.getByRole("alert").waitFor();
  assert.equal(await page.getByLabel("관리자 토큰").inputValue(), "");
  console.log(
    "PASS real storage overview, session expiry, token revocation, and private cache removal",
  );
  assert.deepEqual(errors, []);
} finally {
  await browser.close();
}
