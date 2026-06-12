import assert from 'node:assert/strict';
import test from 'node:test';

import {
  SUPPRESSED_UI_NOTICE_TEXT,
  buildUiNoticeDescriptor,
  buildUiNoticeLocalMessage,
} from './ui-notice-message.js';

test('buildUiNoticeDescriptor suppresses empty and background-send notices', () => {
  assert.equal(buildUiNoticeDescriptor('warning', ''), null);
  assert.equal(buildUiNoticeDescriptor('warning', '   '), null);
  assert.equal(buildUiNoticeDescriptor('warning', SUPPRESSED_UI_NOTICE_TEXT), null);
});

test('buildUiNoticeDescriptor preserves warning content and stable whitespace key', () => {
  const descriptor = buildUiNoticeDescriptor('warning', '  请先选择一个数据集\n再设置默认报表。 ');

  assert.deepEqual(descriptor, {
    displayContent: '请先选择一个数据集\n再设置默认报表。',
    stableKey: 'ui-notice:warning:请先选择一个数据集 再设置默认报表。',
    tone: 'warning',
  });
});

test('buildUiNoticeDescriptor prefixes error notices consistently', () => {
  const descriptor = buildUiNoticeDescriptor('error', '请求失败');

  assert.deepEqual(descriptor, {
    displayContent: '提示：请求失败',
    stableKey: 'ui-notice:error:提示：请求失败',
    tone: 'error',
  });
});

test('buildUiNoticeLocalMessage creates assistant metadata message', () => {
  const descriptor = buildUiNoticeDescriptor('warning', '请先选择一个数据集');
  const message = buildUiNoticeLocalMessage(descriptor, {
    messageFactory(role, content) {
      return {
        id: 'local-fixed',
        role,
        content,
        created_at: '2026-06-12T08:00:00.000Z',
      };
    },
  });

  assert.deepEqual(message, {
    id: 'local-fixed',
    role: 'assistant',
    content: '请先选择一个数据集',
    created_at: '2026-06-12T08:00:00.000Z',
    metadata: {
      source: 'ui_notice',
      tone: 'warning',
      key: 'ui-notice:warning:请先选择一个数据集',
    },
  });
});

test('buildUiNoticeLocalMessage ignores invalid descriptors', () => {
  assert.equal(buildUiNoticeLocalMessage(null), null);
  assert.equal(buildUiNoticeLocalMessage({ stableKey: '', displayContent: 'x' }), null);
});
