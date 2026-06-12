import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { createFetchSseJsonClient, parseSseEventBlock } from './home-api-client.js';

function sseResponse(chunks, init = {}) {
  const encoder = new TextEncoder();
  const stream = new ReadableStream({
    start(controller) {
      chunks.forEach((chunk) => controller.enqueue(encoder.encode(chunk)));
      controller.close();
    },
  });
  return new Response(stream, {
    status: 200,
    headers: { 'content-type': 'text/event-stream' },
    ...init,
  });
}

describe('home SSE API client', () => {
  it('parses SSE event blocks with comments, default event name, and multi-line data', () => {
    assert.deepEqual(parseSseEventBlock(': keep-alive\ndata: one\ndata: two'), {
      event: 'message',
      data: 'one\ntwo',
    });
    assert.deepEqual(parseSseEventBlock('event: assistant_run.delta\ndata: {"delta":"A"}'), {
      event: 'assistant_run.delta',
      data: '{"delta":"A"}',
    });
  });

  it('streams JSON payloads, delta callbacks, and completed response payloads across chunks', async () => {
    const calls = [];
    const deltas = [];
    const events = [];
    const fetchSseJson = createFetchSseJsonClient({
      fetchImpl: async (url, options) => {
        calls.push({ url, options });
        return sseResponse([
          'event: assistant_run.started\ndata: {"display_text":"开始"}\n\n',
          'event: assistant_run.delta\ndata: {"delta":"Hel',
          'lo"}\n\n',
          'event: assistant_run.completed\ndata: {"response":{"answer":"done"}}\n\n',
        ]);
      },
      readSecretBindingIdsHeader: () => 'bind-1',
      readLocalThreadId: () => 'thread-1',
    });

    const completed = await fetchSseJson('/api/v3/assistant-runs/stream', {
      method: 'POST',
      body: { prompt: 'hello' },
    }, {
      onEvent: (eventName, payload) => events.push({ eventName, payload }),
      onDelta: (delta, payload, eventName) => deltas.push({ delta, payload, eventName }),
    });

    assert.deepEqual(completed, { answer: 'done' });
    assert.equal(calls[0].url, '/api/v3/assistant-runs/stream');
    assert.equal(calls[0].options.cache, 'no-store');
    assert.equal(calls[0].options.credentials, 'include');
    assert.equal(calls[0].options.headers.Accept, 'text/event-stream');
    assert.equal(calls[0].options.headers['Content-Type'], 'application/json');
    assert.equal(calls[0].options.headers['X-AI-Data-Platform-Secret-Binding-Ids'], 'bind-1');
    assert.equal(calls[0].options.headers['X-AI-Data-Platform-Local-Thread-Id'], 'thread-1');
    assert.equal(calls[0].options.body, JSON.stringify({ prompt: 'hello' }));
    assert.deepEqual(deltas.map((item) => item.delta), ['Hello']);
    assert.deepEqual(events.map((item) => item.eventName), [
      'assistant_run.started',
      'assistant_run.delta',
      'assistant_run.completed',
    ]);
  });

  it('keeps plain text SSE data readable and returns raw completed payloads', async () => {
    const eventPayloads = [];
    const fetchSseJson = createFetchSseJsonClient({
      fetchImpl: async () => sseResponse([
        'event: assistant_run.delta\ndata: plain text\n\n',
        'event: assistant_run.completed\ndata: final text\n\n',
      ]),
      readSecretBindingIdsHeader: () => '',
      readLocalThreadId: () => '',
    });

    const completed = await fetchSseJson('/stream', {}, {
      onEvent: (_eventName, payload) => eventPayloads.push(payload),
    });

    assert.deepEqual(eventPayloads, ['plain text', 'final text']);
    assert.equal(completed, 'final text');
  });

  it('builds ApiError for non-ok responses and terminal error events', async () => {
    const nonOk = createFetchSseJsonClient({
      fetchImpl: async () => new Response(JSON.stringify({ message: 'denied', code: 'denied' }), {
        status: 403,
        headers: { 'content-type': 'application/json' },
      }),
      readSecretBindingIdsHeader: () => '',
      readLocalThreadId: () => '',
    });
    await assert.rejects(
      () => nonOk('/stream'),
      (error) => {
        assert.equal(error.name, 'ApiError');
        assert.equal(error.status, 403);
        assert.equal(error.code, 'denied');
        assert.equal(error.message, 'denied');
        return true;
      },
    );

    const terminal = createFetchSseJsonClient({
      fetchImpl: async () => sseResponse([
        'event: error\ndata: {"status":409,"error":{"code":"stream_failed","message":"stream failed"}}\n\n',
      ]),
      readSecretBindingIdsHeader: () => '',
      readLocalThreadId: () => '',
    });
    await assert.rejects(
      () => terminal('/stream'),
      (error) => {
        assert.equal(error.name, 'ApiError');
        assert.equal(error.status, 409);
        assert.equal(error.code, 'stream_failed');
        assert.equal(error.message, 'stream failed');
        return true;
      },
    );
  });

  it('keeps the existing unsupported streaming response message', async () => {
    const fetchSseJson = createFetchSseJsonClient({
      fetchImpl: async () => new Response(null, { status: 200 }),
      readSecretBindingIdsHeader: () => '',
      readLocalThreadId: () => '',
    });

    await assert.rejects(
      () => fetchSseJson('/stream'),
      /当前浏览器不支持流式响应读取。/,
    );
  });
});
