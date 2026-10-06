import { execFileSync } from "node:child_process";
import { expect } from "@playwright/test";

export async function usageChecks(page, { database, origin }) {
  // Seed observations in the disposable DB; snapshot production is tested separately.
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
      "INSERT INTO usage_snapshot(day,storage_id,client_id,active_bytes,active_files) VALUES(current_date-1,'console-live','console-client',1073741824,1240),(current_date-180,'retired-storage','retired-client',1024,2)",
    ],
    { timeout: 10000, stdio: "pipe" },
  );
  await page.goto(`${origin}/api/admin/console/#`);
  await page.getByRole("link", { name: "Usage history", exact: true }).click();
  await expect(
    page.getByRole("gridcell", { name: "1,240", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("gridcell", { name: "1 GiB", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("gridcell", { name: "retired-client", exact: true }),
  ).toHaveCount(0);
  await page.getByLabel(/^Days\s*\*?$/).fill("365");
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect(
    page.getByRole("gridcell", { name: "retired-client", exact: true }),
  ).toBeVisible();
  await page.goto(`${origin}/api/admin/console/#storages/console-live`);
  console.log(
    "PASS real usage snapshot UI, date range and retained history for deleted resources",
  );
}
