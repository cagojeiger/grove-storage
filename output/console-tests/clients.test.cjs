const { test } = require('node:test');
const { assert, consolePage, openClient, submit, heading } = require('./support.cjs');

test('client registration can issue a one-time S3 secret', async t => {
  const { ui } = await consolePage(t);
  await ui.locator('nav [data-open=clients]').click();
  await ui.locator('[data-action=new-client]').click();
  await ui.locator('#gm-id').fill('new-client');
  await ui.locator('[name=s3]').check();
  await submit(ui);
  await ui.getByRole('dialog').waitFor();
  assert.match(await ui.getByRole('dialog').innerText(), /demo-only-not-a-real-secret/);
  await ui.locator('[data-action=close]').first().click();
  await ui.locator('[data-tab=creds]').click();
  assert.match(await ui.locator('main').innerText(), /fgak_demo_0001/);
  assert.doesNotMatch(await ui.locator('main').innerText(), /demo-only-not-a-real-secret/);
});

test('Native hash validation, uniqueness and revocation', async t => {
  const { ui } = await consolePage(t);
  await openClient(ui);
  await ui.locator('[data-tab=keys]').click();
  await ui.locator('[data-action=add-key]').click();
  await ui.locator('#gm-new-hash').fill('invalid');
  await ui.locator('[data-action=save-key]').click();
  assert.match(await ui.locator('#gm-modal-error').innerText(), /64/);
  await ui.locator('#gm-new-hash').fill('sha256:' + 'a'.repeat(64));
  await ui.locator('[data-action=save-key]').click();
  assert.match(await ui.locator('#gm-modal-error').innerText(), /이미 등록/);
  const hash = 'sha256:' + 'b'.repeat(64);
  await ui.locator('#gm-new-hash').fill(hash);
  await ui.locator('[data-action=save-key]').click();
  await ui.locator('[data-action=revoke]').click();
  await ui.locator('#gm-confirm').fill(hash);
  await ui.locator('[data-action=confirm-delete]').click();
  assert.match(await ui.locator('main').innerText(), /등록된 항목 없음/);
});

test('deleting an empty client removes the storage reference', async t => {
  const { ui } = await consolePage(t);
  await openClient(ui);
  await ui.locator('[data-action=delete]').click();
  await ui.locator('#gm-confirm').fill('sandbox');
  await ui.locator('[data-action=confirm-delete]').click();
  await heading(ui, 'Clients');
  await ui.locator('nav [data-open=storages]').click();
  await ui.locator('[data-open=storages][data-id=s3-sandbox]').click();
  assert.equal(await ui.locator('[data-action=delete]').isEnabled(), true);
});

test('client with files remains protected', async t => {
  const { ui } = await consolePage(t);
  await openClient(ui, 'notegate');
  assert.equal(await ui.locator('[data-action=delete]').isDisabled(), true);
  assert.match(await ui.locator('#gm-blocked').innerText(), /보존 메타데이터/);
});
