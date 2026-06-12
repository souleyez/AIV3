import test from 'node:test';
import assert from 'node:assert/strict';
import {
  LOCAL_CHAT_SESSIONS_STORAGE_KEY,
  isLocalChatSessionOptionId,
  localChatSessionOptionId,
  localThreadIdFromSessionOptionId,
  normalizeLocalChatSessions,
  readLocalChatSessions,
  shouldPersistLocalChatSession,
  upsertLocalChatSession,
  writeLocalChatSessions,
} from './local-chat-sessions.js';

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

test('local chat option ids round-trip safely', () => {
  const optionId = localChatSessionOptionId('thread-1');
  assert.equal(optionId, 'local-chat:thread-1');
  assert.equal(isLocalChatSessionOptionId(optionId), true);
  assert.equal(localThreadIdFromSessionOptionId(optionId), 'thread-1');
  assert.equal(localThreadIdFromSessionOptionId('backend-session-1'), '');
});

test('local chat sessions normalize, dedupe, and sort by update time', () => {
  const sessions = normalizeLocalChatSessions([
    {
      id: 'thread-old',
      title: 'Old',
      updatedAt: '2026-06-10T00:00:00.000Z',
      messages: [{ id: 'm1', role: 'user', content: '旧问题', created_at: '2026-06-10T00:00:00.000Z' }],
    },
    {
      id: 'thread-new',
      title: 'New',
      updatedAt: '2026-06-11T00:00:00.000Z',
      messages: [{ id: 'm2', role: 'user', content: '新问题', created_at: '2026-06-11T00:00:00.000Z' }],
    },
    {
      id: 'thread-old',
      title: 'Duplicate should lose',
      updatedAt: '2026-06-09T00:00:00.000Z',
      messages: [{ id: 'm3', role: 'user', content: '重复', created_at: '2026-06-09T00:00:00.000Z' }],
    },
  ]);

  assert.deepEqual(sessions.map((session) => session.id), ['thread-new', 'thread-old']);
  assert.equal(sessions[1].title, 'Old');
});

test('upsert local chat session keeps latest copy', () => {
  const sessions = upsertLocalChatSession([
    {
      id: 'thread-1',
      title: 'Before',
      updatedAt: '2026-06-10T00:00:00.000Z',
      messages: [{ id: 'm1', role: 'user', content: '旧问题', created_at: '2026-06-10T00:00:00.000Z' }],
    },
  ], {
    id: 'thread-1',
    title: 'After',
    updatedAt: '2026-06-11T00:00:00.000Z',
    messages: [{ id: 'm2', role: 'user', content: '新问题', created_at: '2026-06-11T00:00:00.000Z' }],
  });

  assert.equal(sessions.length, 1);
  assert.equal(sessions[0].title, 'After');
  assert.equal(sessions[0].messages[0].content, '新问题');
});

test('local chat sessions only persist useful conversations', () => {
  assert.equal(shouldPersistLocalChatSession({ messages: [] }), false);
  assert.equal(shouldPersistLocalChatSession({
    messages: [{ role: 'assistant', content: '提示' }],
  }), false);
  assert.equal(shouldPersistLocalChatSession({
    messages: [{ role: 'user', content: '邓工是谁' }],
  }), true);
  assert.equal(shouldPersistLocalChatSession({ messages: [], assistantRunId: 'run-1' }), true);
});

test('readLocalChatSessions returns normalized cached sessions', () => {
  const cached = JSON.stringify([
    {
      id: 'thread-1',
      title: 'Cached',
      updatedAt: '2026-06-11T00:00:00.000Z',
      messages: [{ id: 'm1', role: 'user', content: '最近低活跃品牌', created_at: '2026-06-11T00:00:00.000Z' }],
    },
    { id: '', messages: [] },
  ]);
  withWindow(createStorage({ [LOCAL_CHAT_SESSIONS_STORAGE_KEY]: cached }), () => {
    const sessions = readLocalChatSessions();
    assert.equal(sessions.length, 1);
    assert.equal(sessions[0].id, 'thread-1');
    assert.equal(sessions[0].title, 'Cached');
  });
});

test('readLocalChatSessions returns an empty list without browser storage', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  if (previous) {
    delete globalThis.window;
  }
  try {
    assert.deepEqual(readLocalChatSessions(), []);
  } finally {
    if (previous) {
      Object.defineProperty(globalThis, 'window', previous);
    }
  }
});

test('readLocalChatSessions tolerates invalid browser cache', () => {
  withWindow(createStorage({ [LOCAL_CHAT_SESSIONS_STORAGE_KEY]: '{bad-json' }), () => {
    assert.deepEqual(readLocalChatSessions(), []);
  });
});

test('writeLocalChatSessions stores normalized sessions and tolerates storage failures', () => {
  withWindow(createStorage(), (storage) => {
    writeLocalChatSessions([
      {
        id: 'thread-1',
        title: 'Stored',
        updatedAt: '2026-06-11T00:00:00.000Z',
        messages: [{ id: 'm1', role: 'user', content: '邓工是谁', created_at: '2026-06-11T00:00:00.000Z' }],
      },
      { id: '', messages: [] },
    ]);
    const stored = JSON.parse(storage.snapshot()[LOCAL_CHAT_SESSIONS_STORAGE_KEY]);
    assert.equal(stored.length, 1);
    assert.equal(stored[0].id, 'thread-1');
    assert.equal(stored[0].title, 'Stored');
  });

  withWindow({
    setItem() {
      throw new Error('blocked');
    },
  }, () => {
    assert.doesNotThrow(() => writeLocalChatSessions([{ id: 'thread-1', messages: [{ role: 'user', content: 'x' }] }]));
  });
});
