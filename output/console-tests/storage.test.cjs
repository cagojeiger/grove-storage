const { test } = require('node:test');
const { assert, consolePage, storageForm, submit, heading } = require('./support.cjs');

test('S3 registration, advanced options, and secret re-entry on edit', async t => {
  const { ui } = await consolePage(t);
  await storageForm(ui);
  await ui.locator('summary').click();
  await ui.locator('#gm-public').fill('https://public.example.com');
  await ui.locator('[name=path]').check();
  await submit(ui);
  await heading(ui, 'test-storage');
  assert.match(await ui.locator('main').innerText(), /public.example.com/);
  assert.doesNotMatch(await ui.locator('main').innerText(), /demo-secret/);
  await ui.locator('[data-action=edit-storage]').click();
  assert.equal(await ui.locator('#gm-id').getAttribute('readonly'), '');
  assert.equal(await ui.locator('#gm-secret').inputValue(), '');
  await ui.locator('#gm-capacity').fill('250');
  await ui.locator('#gm-secret').fill('replacement-demo-secret');
  await submit(ui);
  await heading(ui, 'test-storage');
  assert.match(await ui.locator('main').innerText(), /250 GB/);
});

test('duplicate ID and non-HTTP endpoint are rejected', async t => {
  const { ui } = await consolePage(t);
  await storageForm(ui, 'r2-primary');
  await submit(ui);
  assert.match(await ui.locator('#gm-error').innerText(), /이미 등록/);
  await ui.locator('#gm-id').fill('new-storage');
  await ui.locator('#gm-endpoint').fill('ftp://s3.example.com');
  await submit(ui);
  assert.match(await ui.locator('#gm-error').innerText(), /HTTP/);
});

test('files and client references block storage deletion', async t => {
  const { ui } = await consolePage(t);
  await ui.locator('[data-open=storages][data-id=r2-primary]').click();
  assert.equal(await ui.locator('[data-action=delete]').isDisabled(), true);
  const reason = await ui.locator('#gm-blocked').innerText();
  assert.match(reason, /파일/);
  assert.match(reason, /물리 삭제 대기/);
  assert.match(reason, /연결된 클라이언트/);
});

test('empty storage deletion requires exact name confirmation', async t => {
  const { ui } = await consolePage(t);
  await storageForm(ui);
  await submit(ui);
  await heading(ui, 'test-storage');
  await ui.locator('[data-action=delete]').click();
  await ui.locator('#gm-confirm').fill('wrong-name');
  assert.equal(await ui.locator('[data-action=confirm-delete]').isDisabled(), true);
  await ui.locator('#gm-confirm').fill('test-storage');
  await ui.locator('[data-action=confirm-delete]').click();
  await heading(ui, 'Storages');
  assert.equal(await ui.locator('[data-id=test-storage]').count(), 0);
});
