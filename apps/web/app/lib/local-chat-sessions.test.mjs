import test from 'node:test';
import assert from 'node:assert/strict';
import {
  isLocalChatSessionOptionId,
  localChatSessionOptionId,
  localThreadIdFromSessionOptionId,
  normalizeLocalChatSessions,
  shouldPersistLocalChatSession,
  upsertLocalChatSession,
} from './local-chat-sessions.js';

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
