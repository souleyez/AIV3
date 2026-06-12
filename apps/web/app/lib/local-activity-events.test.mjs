import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildLocalUserStatementMemoryPayload,
  LOCAL_ACTIVITY_EVENTS_STORAGE_KEY,
  readLocalActivityEvents,
  writeLocalActivityEvents,
} from './local-activity-events.js';

function createStorage(initial = {}) {
  const store = new Map(Object.entries(initial).map(([key, value]) => [key, String(value)]));
  return {
    getItem(key) {
      return store.has(key) ? store.get(key) : null;
    },
    setItem(key, value) {
      store.set(key, String(value));
    },
    snapshot() {
      return Object.fromEntries(store.entries());
    },
  };
}

function withWindow(localStorage, callback) {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: { localStorage },
  });
  try {
    return callback(localStorage);
  } finally {
    if (previous) {
      Object.defineProperty(globalThis, 'window', previous);
    } else {
      delete globalThis.window;
    }
  }
}

test('readLocalActivityEvents returns the first cached events without changing shape', () => {
  const events = Array.from({ length: 22 }, (_, index) => ({
    id: `activity-${index}`,
    kind: index % 2 ? 'message' : 'upload',
    title: `event ${index}`,
    metadata: { index },
  }));
  withWindow(createStorage({ [LOCAL_ACTIVITY_EVENTS_STORAGE_KEY]: JSON.stringify(events) }), () => {
    const cached = readLocalActivityEvents();
    assert.equal(cached.length, 20);
    assert.equal(cached[0].id, 'activity-0');
    assert.equal(cached.at(-1).id, 'activity-19');
    assert.deepEqual(cached.at(-1).metadata, { index: 19 });
  });
});

test('readLocalActivityEvents returns empty list without usable browser storage', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  if (previous) {
    delete globalThis.window;
  }
  try {
    assert.deepEqual(readLocalActivityEvents(), []);
  } finally {
    if (previous) {
      Object.defineProperty(globalThis, 'window', previous);
    }
  }

  withWindow(createStorage({ [LOCAL_ACTIVITY_EVENTS_STORAGE_KEY]: '{bad-json' }), () => {
    assert.deepEqual(readLocalActivityEvents(), []);
  });
});

test('writeLocalActivityEvents stores only first events and tolerates storage failures', () => {
  const events = Array.from({ length: 23 }, (_, index) => ({ id: `activity-${index}`, title: `event ${index}` }));
  withWindow(createStorage(), (storage) => {
    writeLocalActivityEvents(events);
    const stored = JSON.parse(storage.snapshot()[LOCAL_ACTIVITY_EVENTS_STORAGE_KEY]);
    assert.equal(stored.length, 20);
    assert.equal(stored[0].id, 'activity-0');
    assert.equal(stored.at(-1).id, 'activity-19');
  });

  withWindow({
    setItem() {
      throw new Error('blocked');
    },
  }, () => {
    assert.doesNotThrow(() => writeLocalActivityEvents([{ id: 'activity-1' }]));
  });
});

test('buildLocalUserStatementMemoryPayload preserves local user statement memory shape', () => {
  const payload = buildLocalUserStatementMemoryPayload({
    message: { id: 'message-1', content: '  低活跃品牌报表  ' },
    localThreadId: 'local-thread-1',
    assistantRunId: 'run-1',
  });
  assert.deepEqual(payload, {
    local_thread_id: 'local-thread-1',
    role: 'user',
    item_kind: 'user_statement',
    summary: '低活跃品牌报表',
    source_message_refs: ['message-1'],
    artifact_refs: [],
    metadata: {
      source: 'browser_local_chat',
      assistant_run_id: 'run-1',
    },
  });
});

test('buildLocalUserStatementMemoryPayload skips empty summaries and keeps fallback fields stable', () => {
  assert.equal(
    buildLocalUserStatementMemoryPayload({
      message: { id: 'message-empty', content: '   ' },
      localThreadId: 'local-thread-1',
    }),
    null,
  );

  const payload = buildLocalUserStatementMemoryPayload({
    message: { content: '普通问答' },
  });
  assert.equal(payload.local_thread_id, '');
  assert.deepEqual(payload.source_message_refs, []);
  assert.equal(payload.metadata.assistant_run_id, null);
});
