import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { createFetchJsonClient } from './home-api-client.js';

describe('home API client', () => {
  it('serializes JSON requests and attaches DataMax browser-scope headers', async () => {
    const calls = [];
    const fetchJson = createFetchJsonClient({
      fetchImpl: async (url, options) => {
        calls.push({ url, options });
        return new Response(JSON.stringify({ ok: true }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        });
      },
      readSecretBindingIdsHeader: () => 'bind-a,bind-b',
      readLocalThreadId: () => 'thread-001',
    });

    const payload = await fetchJson('/api/v3/example', {
      method: 'POST',
      body: { query: 'hello' },
    });

    assert.deepEqual(payload, { ok: true });
    assert.equal(calls[0].url, '/api/v3/example');
    assert.equal(calls[0].options.cache, 'no-store');
    assert.equal(calls[0].options.credentials, 'include');
    assert.equal(calls[0].options.headers.Accept, 'application/json');
    assert.equal(calls[0].options.headers['Content-Type'], 'application/json');
    assert.equal(calls[0].options.headers['X-AI-Data-Platform-Secret-Binding-Ids'], 'bind-a,bind-b');
    assert.equal(calls[0].options.headers['X-AI-Data-Platform-Local-Thread-Id'], 'thread-001');
    assert.equal(calls[0].options.body, JSON.stringify({ query: 'hello' }));
  });

  it('keeps FormData bodies untouched and does not force JSON content type', async () => {
    const form = new FormData();
    form.append('file', 'payload');
    let captured;
    const fetchJson = createFetchJsonClient({
      fetchImpl: async (_url, options) => {
        captured = options;
        return new Response('accepted', {
          status: 200,
          headers: { 'content-type': 'text/plain' },
        });
      },
      readLocalThreadId: () => '',
      readSecretBindingIdsHeader: () => '',
    });

    const payload = await fetchJson('/api/v3/upload', {
      method: 'POST',
      body: form,
    });

    assert.equal(payload, 'accepted');
    assert.equal(captured.body, form);
    assert.equal(captured.headers['Content-Type'], undefined);
  });

  it('builds structured API errors from JSON and text failures', async () => {
    const jsonClient = createFetchJsonClient({
      fetchImpl: async () => new Response(JSON.stringify({
        code: 'bad_request',
        payload: { message: 'nested failure' },
      }), {
        status: 400,
        headers: { 'content-type': 'application/json' },
      }),
      readLocalThreadId: () => '',
      readSecretBindingIdsHeader: () => '',
    });

    await assert.rejects(
      () => jsonClient('/api/v3/fail'),
      (error) => {
        assert.equal(error.name, 'ApiError');
        assert.equal(error.status, 400);
        assert.equal(error.code, 'bad_request');
        assert.equal(error.message, 'nested failure');
        return true;
      },
    );

    const textClient = createFetchJsonClient({
      fetchImpl: async () => new Response('plain failure', { status: 502 }),
      readLocalThreadId: () => '',
      readSecretBindingIdsHeader: () => '',
    });

    await assert.rejects(
      () => textClient('/api/v3/text-fail'),
      (error) => {
        assert.equal(error.name, 'ApiError');
        assert.equal(error.status, 502);
        assert.equal(error.message, 'plain failure');
        return true;
      },
    );
  });

  it('maps abort failures to the existing timeout ApiError message', async () => {
    const abortError = new Error('aborted');
    abortError.name = 'AbortError';
    const fetchJson = createFetchJsonClient({
      fetchImpl: async () => {
        throw abortError;
      },
      readLocalThreadId: () => '',
      readSecretBindingIdsHeader: () => '',
    });

    await assert.rejects(
      () => fetchJson('/api/v3/slow', { timeoutMs: 1200 }),
      (error) => {
        assert.equal(error.name, 'ApiError');
        assert.equal(error.status, 408);
        assert.equal(error.code, '');
        assert.equal(error.payload?.error, 'request_timeout');
        assert.match(error.message, /请求超时（2 秒）/);
        return true;
      },
    );
  });
});
