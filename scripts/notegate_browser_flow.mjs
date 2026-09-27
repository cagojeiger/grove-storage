import { createRequire } from "node:module";
import { readFile, writeFile, unlink } from "node:fs/promises";
import { createHash } from "node:crypto";

const [notegate, origin, grove, output] = process.argv.slice(2);
const require = createRequire(`${notegate}/frontend/web/package.json`);
const { chromium, expect } = require("@playwright/test");
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 960 }, acceptDownloads: true });
const page = await context.newPage();
const failures = [];
const transfers = [];
const files = [];
page.on("pageerror", error => failures.push(error.message));
page.on("response", response => {
  if (response.url().startsWith(grove + "/notegate-browser/")) {
    transfers.push({ method: response.request().method(), status: response.status(),
      path: new URL(response.url()).pathname,
      part: new URL(response.url()).searchParams.get("partNumber") });
  }
});

async function uploadAndDownload(name, bytes, multipart) {
  const before = transfers.length;
  await page.getByRole("button", { name: "Create", exact: true }).click();
  const [chooser] = await Promise.all([
    page.waitForEvent("filechooser"),
    page.getByRole("button", { name: "Upload file", exact: true }).click(),
  ]);
  const source = `${output}/input-${name}`;
  await writeFile(source, bytes);
  await chooser.setFiles(source);
  await page.getByLabel("File name", { exact: true }).fill(name);
  const [completion] = await Promise.all([
    page.waitForResponse(response => response.url().startsWith(origin + "/api/v1/")
      && response.url().includes("/file-uploads/") && response.url().endsWith("/complete"), { timeout: 120000 }),
    page.getByRole("dialog").getByRole("button", { name: "Upload", exact: true }).click(),
  ]);
  if (completion.status() !== 201) throw new Error(`upload completion returned ${completion.status()}`);
  const file = page.getByRole("treeitem", { name, exact: true }).getByRole("button", { name, exact: true });
  await expect(file).toBeVisible({ timeout: 30000 });
  await file.click();
  await expect(page.getByRole("heading", { name, exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Download", exact: true }).first()).toBeVisible();
  await page.screenshot({ path: `${output}/${multipart ? "multipart" : "single"}.png`, fullPage: true });
  const [download] = await Promise.all([
    page.waitForEvent("download", { timeout: 120000 }),
    page.getByRole("button", { name: "Download", exact: true }).first().click(),
  ]);
  const path = `${output}/download-${name}`;
  await download.saveAs(path);
  const received = await readFile(path);
  if (!received.equals(bytes)) throw new Error("downloaded bytes differ");
  const requests = transfers.slice(before);
  const puts = requests.filter(t => t.method === "PUT" && t.status === 200);
  if (!puts.length || (multipart && puts.filter(t => t.part !== null).length < 2)) {
    throw new Error("expected real Grove upload parts were not observed");
  }
  files.push({ name, bytes: bytes.length, multipart,
    sha256: createHash("sha256").update(received).digest("hex"), transfers: requests });
  await unlink(source);
  await unlink(path);
  console.log(`PASS browser ${multipart ? "multipart" : "single"} upload and byte-identical download (${bytes.length} bytes)`);
}

try {
  await page.goto(origin);
  if ((await context.request.get(origin + "/api/v1/me")).status() !== 401) {
    throw new Error("anonymous browser unexpectedly authenticated");
  }
  await expect(page.getByRole("button", { name: "Continue with Google" })).toBeVisible();
  await page.screenshot({ path: `${output}/login.png`, fullPage: true });
  await page.getByRole("button", { name: "Continue with Google" }).click();
  await expect.poll(async () => (await context.cookies(origin))
    .some(cookie => cookie.name === "notegate_browser_session"), { timeout: 30000 }).toBe(true);
  await expect(page.getByRole("button", { name: "Continue with Google" })).toBeHidden({ timeout: 30000 });
  const session = (await context.cookies(origin)).find(cookie => cookie.name === "notegate_browser_session");
  if (!session?.httpOnly || session.sameSite !== "Lax") throw new Error("missing hardened browser session");
  console.log("PASS browser login via real NoteGate callback and HttpOnly session");
  await page.getByRole("button", { name: "Add space", exact: true }).first().click();
  await page.getByLabel("Space name", { exact: true }).fill("grove-browser-test");
  await page.getByRole("dialog").getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("button", { name: "grove-browser-test", exact: true }).first()).toBeVisible();
  await uploadAndDownload("grove-browser.bin", Buffer.from("Grove browser upload and download contract\n".repeat(1000)), false);
  await uploadAndDownload("grove-multipart.bin", Buffer.alloc(101 * 1024 * 1024, 0x5a), true);
  const statusBeforeReload = await page.getByRole("contentinfo").innerText();
  const usageResponse = await context.request.get(origin + "/api/v1/me/usage");
  if (usageResponse.status() !== 200) throw new Error("usage API failed");
  const usage = await usageResponse.json();
  const spaceUsage = usage.spaces.find(space => space.name === "grove-browser-test");
  if (spaceUsage?.items.used !== 2 || spaceUsage.file_bytes.used !== files.reduce((sum, file) => sum + file.bytes, 0)) {
    throw new Error("stored usage does not match uploaded files");
  }
  await page.reload();
  await expect(page.getByRole("treeitem", { name: "grove-multipart.bin", exact: true })).toBeVisible();
  await expect(page.getByRole("contentinfo")).toContainText("2 items");
  const statusAfterReload = await page.getByRole("contentinfo").innerText();
  await page.screenshot({ path: `${output}/after-reload.png`, fullPage: true });
  console.log("Usage before reload:", statusBeforeReload.replaceAll("\n", " "));
  console.log("Usage after reload:", statusAfterReload.replaceAll("\n", " "));
  if (failures.length) throw new Error(failures.join("\n"));
  await writeFile(`${output}/result.json`, JSON.stringify({ files, pageErrors: failures,
    usage: { server: spaceUsage, statusBeforeReload, statusAfterReload } }, null, 2));
} catch (error) {
  await page.screenshot({ path: `${output}/failure.png`, fullPage: true });
  console.error(await page.locator("body").innerText());
  throw error;
} finally {
  await browser.close();
}
