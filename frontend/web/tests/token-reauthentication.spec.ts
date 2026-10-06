import { expect, test } from "@playwright/test";
import { accessMock, owner, rawToken, root } from "./access-fixture";

test("admin token issuance requires a password and a rejected password can be retried", async ({ page }) => {
  await accessMock(page);
  const submitted: Record<string, unknown>[] = [];
  await page.route("**/accounts/*/credentials", async (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    const body = route.request().postDataJSON() as Record<string, unknown>;
    submitted.push(body);
    expect(route.request().headers()["x-grove-csrf"]).toBe("1");
    if (body.current_password !== "the acting admin password")
      return route.fulfill({ status: 401, json: { error: "unauthenticated" } });
    return route.fulfill({ status: 201, json: {
      account_id: owner.id, credential_id: "key", token: rawToken,
      expires_at: "2099-01-01T00:00:00Z",
    } });
  });
  await page.goto(root);
  await page.getByRole("link", { name: owner.display_name, exact: true }).click();
  await page.getByRole("button", { name: "Issue token", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel(/^Label\s*\*?$/).fill("Automation");
  await dialog.getByRole("button", { name: "Issue", exact: true }).click();
  expect(submitted).toHaveLength(0);
  await expect(dialog.getByLabel("Current password")).toHaveAttribute("required", "");
  await dialog.getByLabel("Current password").fill("wrong password");
  await dialog.getByRole("button", { name: "Issue", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText("Current password is incorrect");
  await expect(dialog.getByLabel("Current password")).toHaveValue("");
  await dialog.getByLabel(/^Label\s*\*?$/).fill("Automation");
  await dialog.getByLabel("Current password").fill("the acting admin password");
  await dialog.getByRole("button", { name: "Issue", exact: true }).click();
  await expect(page.getByLabel("Issued token", { exact: true })).toHaveValue(rawToken);
  expect(submitted.map((body) => body.current_password)).toEqual([
    "wrong password", "the acting admin password",
  ]);
  const stored = await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }));
  expect(stored).not.toContain("the acting admin password");
  expect(stored).not.toContain(rawToken);
});
