import test from 'node:test';
import assert from 'node:assert/strict';
import {
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
