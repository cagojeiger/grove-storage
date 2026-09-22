const assert = require('node:assert/strict');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('playwright');

async function consolePage(t, width = 1280) {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width, height: 1000 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  t.after(async () => {
    await browser.close();
    assert.deepEqual(errors, [], 'browser runtime errors');
  });
  const source = await readFile(path.join(__dirname, '../filegate-console-management.html'), 'utf8');
  // Match the preview isolation without depending on a Codex-installed renderer.
  await page.setContent('<iframe style="width:100%;border:0;height:2000px" sandbox="allow-scripts allow-forms allow-downloads"></iframe>');
  await page.locator('iframe').evaluate((iframe, source) => { iframe.srcdoc = source; }, source);
  const ui = page.frameLocator('iframe');
  await ui.locator('[data-action=new-storage]').waitFor();
  return { page, ui };
}

async function storageForm(ui, id = 'test-storage') {
  await ui.locator('[data-action=new-storage]').click();
  for (const [name, value] of Object.entries({
    id, endpoint: 'https://s3.example.com', bucket: 'demo-bucket',
    access: 'demo-access', secret: 'demo-secret',
  })) await ui.locator(`#gm-${name}`).fill(value);
}

async function submit(ui) {
  await ui.locator('#gm-form [type=submit]').click();
}

async function heading(ui, expected) {
  await ui.getByRole('heading', { level: 1, name: expected, exact: true }).waitFor();
}

async function openClient(ui, id = 'sandbox') {
  await ui.locator('nav [data-open=clients]').click();
  await ui.locator(`[data-open=clients][data-id="${id}"]`).click();
}

module.exports = { assert, consolePage, storageForm, submit, heading, openClient };
