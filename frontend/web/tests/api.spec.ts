import { test, expect } from '@playwright/test';
import { ApiError, request } from '../src/api/http';
import { bytes } from '../src/features/overview/Overview';

test('byte formatting includes zero and over-capacity values', () => {
  expect(bytes(0)).toBe('0 B');
  expect(bytes(1024)).toBe('1 KiB');
  expect(bytes(1024 ** 4)).toBe('1 TiB');
  expect(bytes(-1024)).toBe('-1 KiB');
});

test('transport uses same-origin cookies, CSRF, no-store and handles 204', async () => {
  const original = globalThis.fetch;
  const controller = new AbortController();
  try {
    globalThis.fetch = async (_url, options) => {
      expect(options?.credentials).toBe('same-origin');
      expect(options?.cache).toBe('no-store');
      expect(new Headers(options?.headers).get('X-FileGate-CSRF')).toBe('1');
      expect(new Headers(options?.headers).has('Authorization')).toBe(false);
      controller.abort();
      expect(options?.signal?.aborted).toBe(true);
      return new Response(null, { status: 204 });
    };
    expect(await request('/api/admin/v1/session', { method: 'DELETE', signal: controller.signal })).toBeUndefined();
  } finally { globalThis.fetch = original; }
});

test('transport exposes retry delay but never echoes response secrets', async () => {
  const original = globalThis.fetch;
  try {
    globalThis.fetch = async () => new Response('secret-from-server', { status: 429, headers: { 'Retry-After': '60' } });
    try { await request('/api/admin/v1/session'); throw new Error('expected failure'); }
    catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect((error as ApiError).retryAfter).toBe(60);
      expect((error as Error).message).not.toContain('secret-from-server');
    }
  } finally { globalThis.fetch = original; }
});
