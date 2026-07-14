import test from 'node:test';
import assert from 'node:assert/strict';
import { isExternalObservabilityPath, proxyPlatformApiRequest } from './platform-api.js';

test('isExternalObservabilityPath protects conversation list and lazy timeline detail', () => {
  assert.equal(isExternalObservabilityPath(['external', 'conversation-tests']), true);
  assert.equal(
    isExternalObservabilityPath([
      'external',
      'conversation-tests',
      '0f02e551-c95b-4a45-b809-d5364bcefd88',
      'timeline',
    ]),
    true,
  );
  assert.equal(
    isExternalObservabilityPath([
      'external',
      'integrations',
      'generic-chat-main',
      'disable',
    ]),
    true,
  );
  assert.equal(
    isExternalObservabilityPath([
      'external',
      'integrations',
      'generic-chat-main',
      'retry',
    ]),
    true,
  );
  assert.equal(
    isExternalObservabilityPath([
      'external',
      'integrations',
      'generic-chat-main',
      'rotate-secret',
    ]),
    true,
  );
  assert.equal(
    isExternalObservabilityPath([
      'external',
      'integrations',
      'generic-chat-main',
      'rotate-token',
    ]),
    true,
  );
  assert.equal(
    isExternalObservabilityPath([
      'external',
      'integrations',
      'generic-chat-main',
      'reply-dispatch',
    ]),
    true,
  );
  assert.equal(isExternalObservabilityPath(['external', 'integrations', 'channels']), true);
  assert.equal(isExternalObservabilityPath(['external', 'integrations']), false);
});

test('proxyPlatformApiRequest forwards conditional ETag requests and returns 304', async () => {
  const originalFetch = globalThis.fetch;
  let capturedHeaders;
  globalThis.fetch = async (_url, options) => {
    capturedHeaders = options.headers;
    return new Response(null, {
      status: 304,
      headers: { etag: '"semantic-fingerprint"' },
    });
  };

  try {
    const request = new Request('http://localhost/api/v3/datasets/dataset-1/understanding', {
      headers: { 'if-none-match': '"semantic-fingerprint"' },
    });
    const response = await proxyPlatformApiRequest(request, ['datasets', 'dataset-1', 'understanding']);

    assert.equal(capturedHeaders.get('if-none-match'), '"semantic-fingerprint"');
    assert.equal(response.status, 304);
    assert.equal(response.headers.get('etag'), '"semantic-fingerprint"');
  } finally {
    globalThis.fetch = originalFetch;
  }
});
