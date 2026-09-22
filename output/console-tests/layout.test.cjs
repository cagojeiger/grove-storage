const { test } = require('node:test');
const { assert, consolePage, storageForm, submit, heading } = require('./support.cjs');

for (const width of [320, 768, 1280]) {
  test(`light/dark forms and navigation fit at ${width}px`, async t => {
    const { ui } = await consolePage(t, width);
    await storageForm(ui);
    await ui.locator('summary').click();
    for (const theme of ['light', 'dark']) {
      await ui.locator(`[data-theme=${theme}]`).click();
      assert.equal(await ui.locator('#fg-management').evaluate(e => e.style.colorScheme), theme);
      assert.equal(await ui.locator('#fg-management').evaluate(e => e.scrollWidth > e.clientWidth), false);
      const formOverflow = await ui.locator('#gm-form').evaluate(form => {
        const bounds = form.getBoundingClientRect();
        return [...form.querySelectorAll('input,select,button')].some(e => {
          const rect = e.getBoundingClientRect();
          return rect.width && (rect.left < bounds.left - 1 || rect.right > bounds.right + 1);
        });
      });
      assert.equal(formOverflow, false, 'form controls must remain in bounds');
    }
  });
}

test('logout/login clears token field and returns to overview', async t => {
  const { ui } = await consolePage(t);
  await ui.locator('[data-action=logout]').click();
  await ui.locator('#gm-token').fill('demo-token-not-real');
  await submit(ui);
  await heading(ui, 'Overview');
  await ui.locator('[data-action=logout]').click();
  assert.equal(await ui.locator('#gm-token').inputValue(), '');
});

test('dialog Escape closes and restores background interaction', async t => {
  const { page, ui } = await consolePage(t);
  await ui.locator('nav [data-open=clients]').click();
  await ui.locator('[data-open=clients][data-id=sandbox]').click();
  await ui.locator('[data-action=delete]').click();
  assert.equal(await ui.locator('#gm-view').evaluate(e => e.inert), true);
  await page.keyboard.press('Escape');
  assert.equal(await ui.getByRole('dialog').count(), 0);
  assert.equal(await ui.locator('#gm-view').evaluate(e => e.inert), false);
});
