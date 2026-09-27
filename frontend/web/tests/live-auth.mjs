import assert from "node:assert/strict";

export async function loginWithPassword(page, username, password) {
  await page.getByLabel("Username").fill(username);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByRole("button", { name: "Sign out" }).waitFor();
}

// Existing token sessions remain a migration oracle without exposing a token form.
export async function loginWithToken(page, token) {
  const origin = new URL(page.url()).origin;
  const response = await page.request.post(`${origin}/api/admin/identity/v1/session`, {
    data: { token },
    headers: { Origin: origin, "X-Grove-CSRF": "1" },
  });
  assert.equal(response.status(), 200, await response.text());
  await page.reload();
}
