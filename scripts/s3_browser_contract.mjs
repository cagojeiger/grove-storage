import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import path from 'node:path';

const require = createRequire(path.join(process.cwd(), 'package.json'));
const { chromium } = require('@playwright/test');
let input = '';
for await (const chunk of process.stdin) input += chunk;
const { origin, targets } = JSON.parse(input);
const browser = await chromium.launch();
try {
  // Only self-signed loopback fixture certificates are used in this test.
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  const page = await context.newPage();
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  const results = {};
  for (const target of targets) {
    await page.goto(origin);
    const result = await page.evaluate(async fixture => {
      const check = (condition, message) => { if (!condition) throw new Error(message); };
      const errorCode = async response => new DOMParser()
        .parseFromString(await response.text(), 'application/xml').querySelector('Code')?.textContent;
      const headers = { 'Content-Type': 'application/octet-stream', 'If-None-Match': '*' };
      let response = await fetch(fixture.put, { method: 'PUT', headers, body: fixture.body });
      check(response.status === 200 && response.headers.get('ETag'), 'PUT / exposed ETag');
      response = await fetch(fixture.put, { method: 'PUT', headers, body: fixture.body });
      check(response.status === 412 && await errorCode(response) === 'PreconditionFailed', 'conditional XML error');
      response = await fetch(fixture.get);
      check(response.status === 200 && await response.text() === fixture.body, 'GET bytes');
      response = await fetch(fixture.get, { headers: { Range: 'bytes=0-5' } });
      check(response.status === 206 && await response.text() === fixture.body.slice(0, 6), 'Range bytes');
      response = await fetch(fixture.head, { method: 'HEAD' });
      check(response.status === 200 && response.headers.get('ETag') &&
        response.headers.get('Content-Length') === String(fixture.body.length), 'HEAD headers');
      response = await fetch(fixture.invalid);
      check(response.status === 403 && await errorCode(response) === 'SignatureDoesNotMatch', 'readable signature XML error');

      const bodies = ['m'.repeat(5 * 1024 * 1024), 'last-part'];
      const parts = [];
      for (let index = 0; index < bodies.length; index++) {
        response = await fetch(fixture.parts[index], { method: 'PUT', body: bodies[index] });
        const etag = response.headers.get('ETag');
        check(response.status === 200 && etag, 'multipart PUT / exposed ETag');
        parts.push(`<Part><PartNumber>${index + 1}</PartNumber><ETag>${etag}</ETag></Part>`);
      }
      response = await fetch(fixture.complete, { method: 'POST', headers: { 'Content-Type': 'application/xml' },
        body: `<CompleteMultipartUpload>${parts.join('')}</CompleteMultipartUpload>` });
      check(response.status === 200 && new DOMParser().parseFromString(await response.text(), 'application/xml')
        .querySelector('ETag'), 'multipart Complete XML');
      response = await fetch(fixture.multipart_get);
      const bytes = new Uint8Array(await response.arrayBuffer());
      const expected = new TextEncoder().encode(bodies.join(''));
      check(response.status === 200 && bytes.length === expected.length &&
        bytes.every((byte, index) => byte === expected[index]), 'multipart downloaded bytes');
      const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
      const multipartHash = Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
      response = await fetch(fixture.abort, { method: 'DELETE' });
      check(response.status === 204, 'multipart Abort');
      response = await fetch(fixture.aborted_part, { method: 'PUT', body: 'must-not-upload' });
      check(response.status === 404 && await errorCode(response) === 'NoSuchUpload', 'aborted upload XML error');
      for (const url of [fixture.delete, fixture.multipart_delete]) {
        response = await fetch(url, { method: 'DELETE' });
        check(response.status === 204, 'DELETE');
      }
      return { single_bytes: fixture.body.length, multipart_bytes: bytes.length, multipart_sha256: multipartHash,
        conditional_create: true, range: true, exposed_etag: true, readable_errors: true, abort: true, delete: true };
    }, target);
    await page.goto(origin.replace('127.0.0.1', 'localhost'));
    const blocked = await page.evaluate(async url => {
      try {
        await fetch(url, { method: 'PUT', headers: { 'Content-Type': 'application/octet-stream' }, body: 'blocked' });
        return false;
      } catch (error) {
        return error instanceof TypeError;
      }
    }, target.blocked);
    assert.equal(blocked, true, `${target.name}: disallowed Origin must be blocked`);
    results[target.name] = { ...result, blocked_origin: true };
  }
  assert.deepEqual(pageErrors, []);
  assert.equal(results.minio.multipart_sha256, createHash('sha256').update('m'.repeat(5 * 1024 * 1024) + 'last-part').digest('hex'));
  process.stdout.write(JSON.stringify(results));
  await context.close();
} finally {
  await browser.close();
}
