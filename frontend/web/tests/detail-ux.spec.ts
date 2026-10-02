import { expect, test } from "@playwright/test";
import { resolve } from "node:path";
import { accessMock, owner, rawToken } from "./access-fixture";
import { commandUrl, envelope, failure } from "./command-fixture";
import { example, storageMock } from "./storage-fixture";

for (const mode of ["light", "dark"]) {
  for (const width of [390, 768, 1440]) {
    test(`token details keep one-time safeguards at ${width}px in ${mode}`, async ({ page, context }) => {
      await accessMock(page);
      await context.grantPermissions(["clipboard-read", "clipboard-write"]);
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`/api/admin/console/#accounts/${owner.id}`);
      await page.getByLabel("Theme").selectOption(mode);
      await page.getByRole("tab", { name: "API tokens", exact: true }).click();
      await page.getByRole("button", { name: "Issue token", exact: true }).click();
      await page.getByLabel(/^Label/).fill("Deployment automation");
      await page.getByRole("button", { name: "Issue", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: "API token created" });
      const token = dialog.getByLabel("Issued token", { exact: true });
      const done = dialog.getByRole("button", { name: "Done", exact: true });
      await expect(token).toHaveAttribute("type", "password");
      await expect(done).toBeDisabled();
      await expect(dialog.getByRole("heading", { level: 2 })).toHaveCount(1);
      await expect(dialog.getByRole("heading", { name: "Token issued" })).toHaveCount(0);
      await expect(dialog.getByRole("button", { name: "Connection examples" })).toHaveAttribute("aria-expanded", "false");
      await expect(dialog.getByText(owner.display_name, { exact: true })).toBeVisible();
      await dialog.getByRole("button", { name: "Show token" }).click();
      await expect(token).toHaveAttribute("type", "text");
      await expect(token).toHaveValue(rawToken);
      await dialog.getByRole("button", { name: "Hide token" }).click();
      await dialog.getByRole("button", { name: "Copy token" }).click();
      await expect(dialog.getByRole("status")).toHaveText("Copied");
      expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(rawToken);
      await page.screenshot({
        path: resolve(`../../output/console-detail-ux-20261001/token-${width}-${mode}.png`),
        animations: "disabled",
      });
      await dialog.getByRole("button", { name: "Connection examples" }).click();
      await expect(dialog.getByText(/gscli --endpoint/)).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(dialog).toBeVisible();
      expect(await dialog.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
      await dialog.getByRole("checkbox").check();
      await done.click();
      await expect(token).toHaveCount(0);
      await page.screenshot({
        path: resolve(`../../output/console-detail-ux-20261001/account-${width}-${mode}.png`),
        fullPage: true,
        animations: "disabled",
      });
    });
  }
}

for (const mode of ["light", "dark"]) {
  test(`expanded token examples keep actions reachable on a short phone in ${mode}`, async ({ page }) => {
    await accessMock(page);
    await page.setViewportSize({ width: 320, height: 480 });
    await page.goto(`/api/admin/console/#accounts/${owner.id}?tab=tokens`);
    await page.getByLabel("Theme").selectOption(mode);
    await page.getByRole("button", { name: "Issue token", exact: true }).click();
    await page.getByLabel(/^Label/).fill("Phone automation");
    await page.getByRole("button", { name: "Issue", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "API token created" });
    await dialog.getByRole("button", { name: "Connection examples" }).click();
    await expect(dialog.getByText(/gscli --endpoint/)).toBeVisible();
    const done = dialog.getByRole("button", { name: "Done", exact: true });
    await expect(done).toBeDisabled();
    await expect(done).toBeInViewport();
    expect(await dialog.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    await page.keyboard.press("Escape");
    await expect(dialog).toBeVisible();
    await dialog.getByRole("checkbox").check();
    await expect(done).toBeInViewport();
    await page.screenshot({
      path: resolve(`../../output/console-mui-simplify-20261002/token-short-${mode}.png`),
      animations: "disabled",
    });
    await done.click();
    await expect(dialog).toHaveCount(0);
  });
}

for (const scenario of ["normal", "zero", "over", "unavailable"] as const) {
  test(`capacity visualization handles ${scenario} without misleading percentages`, async ({ page }) => {
    const capacity = scenario === "zero" ? 0 : 1024;
    await storageMock(page, [{ ...example, capacity_bytes: capacity }]);
    await page.route(commandUrl, async route => {
      const { command } = route.request().postDataJSON() as { command: string };
      if (command !== "usage.storages") return route.fallback();
      if (scenario === "unavailable") return route.fulfill({ status: 503, json: failure(503) });
      return route.fulfill({ json: envelope(command, [{
        storage_id: example.id, kind: "s3", capacity_bytes: capacity,
        active_bytes: scenario === "zero" ? 0 : 512,
        reserved_bytes: scenario === "zero" ? 0 : 256,
        purge_pending_bytes: scenario === "over" ? 768 : 0,
        active_files: 1, reserved_files: 1, purge_pending_files: 0,
        remaining_bytes: scenario === "over" ? -512 : scenario === "zero" ? 0 : 256,
      }]) });
    });
    await page.goto("/api/admin/console/#storages");
    const grid = page.getByRole("grid", { name: "Storage", exact: true });
    const progress = grid.getByRole("progressbar");
    if (scenario === "zero" || scenario === "unavailable") {
      await expect(grid.getByText(scenario === "zero" ? "0 B remaining" : "Usage unavailable")).toBeVisible();
      await expect(progress).toHaveCount(0);
    } else {
      await expect(progress).toHaveAttribute("aria-valuetext", `${scenario === "over" ? 150 : 75}% of configured capacity`);
      await expect(progress).toHaveAttribute("aria-valuenow", scenario === "over" ? "100" : "75");
      if (scenario === "over") await expect(grid.getByText("Over capacity by 512 B")).toBeVisible();
    }
  });
}
