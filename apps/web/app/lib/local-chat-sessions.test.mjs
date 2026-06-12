import test from 'node:test';
import assert from 'node:assert/strict';
import {
  LOCAL_CHAT_MESSAGES_STORAGE_KEY,
  LOCAL_CHAT_SESSIONS_STORAGE_KEY,
  appendLocalChatMessages,
  buildLocalChatSessionSnapshot,
  createLocalMessage,
  isLocalChatSessionOptionId,
  limitLocalChatMessages,
  localChatSessionOptionId,
  localThreadIdFromSessionOptionId,
  normalizeLocalChatSessions,
  readLocalChatMessages,
  readLocalChatSessions,
  replaceLocalChatMessageContent,
  shouldPersistLocalChatSession,
  upsertLocalChatSession,
  writeLocalChatMessages,
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

test('createLocalMessage keeps the local chat message shape', () => {
  const message = createLocalMessage('assistant', '正在生成回复...', {
    nowMs: () => 1780000000000,
    random: () => 0.123456789,
    createdAt: '2026-06-12T08:00:00.000Z',
  });

  assert.deepEqual(message, {
    id: 'local-1780000000000-4fzzzx',
    role: 'assistant',
    content: '正在生成回复...',
    created_at: '2026-06-12T08:00:00.000Z',
  });
});

test('local chat message list helpers append, replace, and preserve the 40 message cap', () => {
  const messages = Array.from({ length: 41 }, (_, index) => ({
    id: `m-${index}`,
    role: 'user',
    content: `message ${index}`,
    metadata: { index },
  }));

  const limited = limitLocalChatMessages(messages);
  assert.equal(limited.length, 40);
  assert.equal(limited[0].id, 'm-1');

  const current = messages.slice(0, 39);
  const appended = appendLocalChatMessages(current, [
    { id: 'm-39', role: 'assistant', content: 'message 39' },
    { id: 'm-40', role: 'assistant', content: 'message 40', metadata: { source: 'stream' } },
  ]);
  assert.equal(appended.length, 40);
  assert.equal(appended[0].id, 'm-1');
  assert.equal(appended.at(-1).id, 'm-40');

  const noAppend = appendLocalChatMessages(current, []);
  assert.equal(noAppend, current);

  const replaced = replaceLocalChatMessageContent(appended, 'm-40', 'updated');
  assert.equal(replaced.at(-1).content, 'updated');
  assert.deepEqual(replaced.at(-1).metadata, { source: 'stream' });

  const unboundedReplace = replaceLocalChatMessageContent(messages, 'm-40', 'streaming', { limit: false });
  assert.equal(unboundedReplace.length, 41);
  assert.equal(unboundedReplace.at(-1).content, 'streaming');
});

test('buildLocalChatSessionSnapshot preserves local conversation snapshot semantics', () => {
  const snapshot = buildLocalChatSessionSnapshot({
    fallbackThreadId: 'thread-1',
    messages: [
      { id: 'm1', role: 'assistant', content: '提示' },
      { id: 'm2', role: 'user', content: '最近低活跃品牌报表' },
    ],
    fallbackStartedAt: '2026-06-12T08:09:00+08:00',
    now: '2026-06-12T08:10:00+08:00',
    assistantRunId: 'run-1',
  });

  assert.deepEqual(snapshot, {
    id: 'thread-1',
    title: '06-12 08:09 · 最近低活跃品牌报表',
    messages: [
      { id: 'm1', role: 'assistant', content: '提示' },
      { id: 'm2', role: 'user', content: '最近低活跃品牌报表' },
    ],
    startedAt: '2026-06-12T08:09:00+08:00',
    updatedAt: '2026-06-12T08:10:00+08:00',
    assistantRunId: 'run-1',
  });

  assert.equal(
    buildLocalChatSessionSnapshot({
      threadId: 'thread-explicit',
      fallbackThreadId: 'thread-fallback',
      title: '  手工标题  ',
      messages: [],
      startedAt: '2026-06-12T08:00:00+08:00',
      updatedAt: '2026-06-12T08:01:00+08:00',
    }).title,
    '手工标题',
  );

  const generatedTimeSnapshot = buildLocalChatSessionSnapshot({
    threadId: 'thread-now',
    messages: [],
    now: '2026-06-12T08:11:00+08:00',
  });
  assert.equal(generatedTimeSnapshot.startedAt, '2026-06-12T08:11:00+08:00');
  assert.equal(generatedTimeSnapshot.updatedAt, '2026-06-12T08:11:00+08:00');
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

test('readLocalChatMessages returns the latest cached messages without changing shape', () => {
  const messages = Array.from({ length: 42 }, (_, index) => ({
    id: `m-${index}`,
    role: index % 2 ? 'assistant' : 'user',
    content: `message ${index}`,
    metadata: { index },
  }));
  withWindow(createStorage({ [LOCAL_CHAT_MESSAGES_STORAGE_KEY]: JSON.stringify(messages) }), () => {
    const cached = readLocalChatMessages();
    assert.equal(cached.length, 40);
    assert.equal(cached[0].id, 'm-2');
    assert.deepEqual(cached.at(-1).metadata, { index: 41 });
  });
});

test('readLocalChatMessages returns empty list for missing or invalid storage', () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  if (previous) {
    delete globalThis.window;
  }
  try {
    assert.deepEqual(readLocalChatMessages(), []);
  } finally {
    if (previous) {
      Object.defineProperty(globalThis, 'window', previous);
    }
  }

  withWindow(createStorage({ [LOCAL_CHAT_MESSAGES_STORAGE_KEY]: '{bad-json' }), () => {
    assert.deepEqual(readLocalChatMessages(), []);
  });
});

test('writeLocalChatMessages stores only latest messages and tolerates storage failures', () => {
  const messages = Array.from({ length: 43 }, (_, index) => ({ id: `m-${index}`, role: 'user', content: `message ${index}` }));
  withWindow(createStorage(), (storage) => {
    writeLocalChatMessages(messages);
    const stored = JSON.parse(storage.snapshot()[LOCAL_CHAT_MESSAGES_STORAGE_KEY]);
    assert.equal(stored.length, 40);
    assert.equal(stored[0].id, 'm-3');
    assert.equal(stored.at(-1).id, 'm-42');
  });

  withWindow({
    setItem() {
      throw new Error('blocked');
    },
  }, () => {
    assert.doesNotThrow(() => writeLocalChatMessages([{ id: 'm-1', role: 'user', content: 'x' }]));
  });
});
