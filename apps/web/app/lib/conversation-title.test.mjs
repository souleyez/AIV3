import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildCurrentConversationTitle,
  buildDefaultConversationTitle,
  compactConversationSummary,
  formatConversationTitleTime,
} from './conversation-title.js';

test('compactConversationSummary normalizes whitespace and falls back to new conversation', () => {
  assert.equal(compactConversationSummary('  邓工   是谁\n'), '邓工 是谁');
  assert.equal(compactConversationSummary(''), '新对话');
  assert.equal(compactConversationSummary(null), '新对话');
});

test('compactConversationSummary truncates long prompts with ellipsis', () => {
  assert.equal(compactConversationSummary('abcdefghijklmnopqrstuvwxyz', 8), 'abcdefgh...');
});

test('formatConversationTitleTime formats stable zh-CN month day and time', () => {
  assert.equal(formatConversationTitleTime(new Date('2026-06-12T08:09:00+08:00')), '06-12 08:09');
  assert.equal(formatConversationTitleTime(new Date('2026-06-12T00:09:00Z')), '06-12 08:09');
});

test('formatConversationTitleTime tolerates invalid dates', () => {
  assert.match(formatConversationTitleTime('not-a-date'), /^\d{2}-\d{2} \d{2}:\d{2}$/);
});

test('buildDefaultConversationTitle combines timestamp and compact prompt', () => {
  assert.equal(
    buildDefaultConversationTitle('最近低活跃品牌报表', new Date('2026-06-12T08:09:00+08:00')),
    '06-12 08:09 · 最近低活跃品牌报表',
  );
});

test('buildCurrentConversationTitle preserves selected session and draft title precedence', () => {
  assert.equal(
    buildCurrentConversationTitle({
      selectedSession: { title: '后端会话标题' },
      draftSessionTitle: '草稿标题',
      input: '输入标题',
      startedAt: new Date('2026-06-12T08:09:00+08:00'),
    }),
    '后端会话标题',
  );

  assert.equal(
    buildCurrentConversationTitle({
      selectedSession: { title: '' },
      draftSessionTitle: '草稿标题',
    }),
    '当前对话',
  );

  assert.equal(
    buildCurrentConversationTitle({
      draftSessionTitle: '  草稿标题  ',
      input: '输入标题',
    }),
    '草稿标题',
  );
});

test('buildCurrentConversationTitle falls back to input, first user message, and new conversation title', () => {
  const startedAt = new Date('2026-06-12T08:09:00+08:00');
  assert.equal(
    buildCurrentConversationTitle({
      input: '输入标题',
      messages: [{ role: 'user', content: '首条用户消息' }],
      startedAt,
    }),
    '06-12 08:09 · 输入标题',
  );

  assert.equal(
    buildCurrentConversationTitle({
      messages: [
        { role: 'assistant', content: '提示' },
        { role: 'user', content: '首条用户消息' },
      ],
      startedAt,
    }),
    '06-12 08:09 · 首条用户消息',
  );

  assert.equal(
    buildCurrentConversationTitle({ startedAt }),
    '06-12 08:09 · 新对话',
  );
});
