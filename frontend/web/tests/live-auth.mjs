export async function loginWithPassword(page, username, password) {
  await page.getByLabel("Username").fill(username);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByLabel("Username").waitFor({ state: "hidden" });
  await page.getByRole("main").waitFor();
}
