import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  ASSISTANT_STREAM_EMPTY_FINAL_TEXT,
  ASSISTANT_STREAM_PLACEHOLDER_TEXT,
  appendArtifactLinkText,
  assistantRunFinalMessageContent,
  assistantRunStreamMessageContent,
  assistantRunStreamArtifactLink,
  assistantRunStreamDisplayText,
  buildAssistantStreamPlaceholderMessage,
  cleanAssistantVisibleContent,
  firstGeneratedArtifactUrlFromPayload,
} from './assistant-stream-content.js';

describe('assistant stream content helpers', () => {
  it('uses assistant_run display text while ignoring delta and completed events', () => {
    assert.equal(
      assistantRunStreamDisplayText('assistant_run.progress', { display_text: '  正在检索资料  ' }),
      '正在检索资料',
    );
    assert.equal(
      assistantRunStreamDisplayText('assistant_run.tool', { data: { displayText: '正在生成报表' } }),
      '正在生成报表',
    );
    assert.equal(assistantRunStreamDisplayText('assistant_run.delta', { display_text: 'partial' }), '');
    assert.equal(assistantRunStreamDisplayText('assistant_run.completed', { display_text: 'done' }), '');
    assert.equal(assistantRunStreamDisplayText('other.event', { display_text: 'ignore' }), '');
  });

  it('finds generated artifact URLs from nested assistant payloads only', () => {
    assert.equal(
      firstGeneratedArtifactUrlFromPayload({
        card: {
          artifactLinks: [
            'https://v3.elepcloud.com/generated-artifacts/demo/index.html',
          ],
        },
      }),
      'https://v3.elepcloud.com/generated-artifacts/demo/index.html',
    );
    assert.equal(
      firstGeneratedArtifactUrlFromPayload({
        data: {
          generated_artifact_url: '/generated-artifacts/demo/index.html',
        },
      }),
      '/generated-artifacts/demo/index.html',
    );
    assert.equal(firstGeneratedArtifactUrlFromPayload({ public_url: 'https://example.com/file.html' }), '');
  });

  it('exposes artifact links only for assistant_run stream events', () => {
    const payload = { public_url: '/generated-artifacts/report/index.html' };
    assert.equal(
      assistantRunStreamArtifactLink('assistant_run.artifact_ready', payload),
      '/generated-artifacts/report/index.html',
    );
    assert.equal(assistantRunStreamArtifactLink('external.action', payload), '');
  });

  it('removes internal JSON payloads and duplicate generated artifact links', () => {
    const content = [
      '报表已生成。',
      '',
      '链接：https://v3.elepcloud.com/generated-artifacts/report/index.html',
      '链接：https://v3.elepcloud.com/generated-artifacts/report/index.html。',
      '{"assistant_run_id":"run-1","status_url":"/internal"}',
    ].join('\n');

    assert.equal(
      cleanAssistantVisibleContent(content),
      [
        '报表已生成。',
        '',
        '链接：https://v3.elepcloud.com/generated-artifacts/report/index.html',
      ].join('\n'),
    );
  });

  it('appends one generated artifact link without duplicating an existing URL', () => {
    const url = 'https://v3.elepcloud.com/generated-artifacts/report/index.html';
    assert.equal(
      appendArtifactLinkText('报表已生成。', url),
      `报表已生成。\n\n[打开生成页面](${url})`,
    );
    assert.equal(
      appendArtifactLinkText(`报表已生成。\n${url}`, url),
      `报表已生成。\n${url}`,
    );
    assert.equal(
      appendArtifactLinkText('', url),
      `页面已生成。\n\n[打开生成页面](${url})`,
    );
  });

  it('builds assistant stream message content from streamed text, status text, and artifact link', () => {
    const url = '/generated-artifacts/report/index.html';
    assert.equal(
      assistantRunStreamMessageContent({
        streamedAssistantContent: '报表已生成。',
        streamStatusText: '正在生成报表',
        streamArtifactLink: url,
      }),
      `报表已生成。\n\n[打开生成页面](${url})`,
    );
    assert.equal(
      assistantRunStreamMessageContent({
        streamStatusText: '正在检索资料',
      }),
      '正在检索资料',
    );
    assert.equal(
      assistantRunStreamMessageContent({}),
      ASSISTANT_STREAM_PLACEHOLDER_TEXT,
    );
  });

  it('builds final assistant message content with existing fallback order', () => {
    const url = '/generated-artifacts/report/index.html';
    assert.equal(
      assistantRunFinalMessageContent({
        assistantContent: '最终回答。',
        streamedAssistantContent: '流式回答。',
        streamStatusText: '处理中',
        streamArtifactLink: url,
      }),
      `最终回答。\n\n[打开生成页面](${url})`,
    );
    assert.equal(
      assistantRunFinalMessageContent({
        streamedAssistantContent: '流式回答。',
        streamStatusText: '处理中',
      }),
      '流式回答。',
    );
    assert.equal(
      assistantRunFinalMessageContent({}),
      ASSISTANT_STREAM_EMPTY_FINAL_TEXT,
    );
  });

  it('builds assistant stream placeholder messages with existing local message shape', () => {
    const message = buildAssistantStreamPlaceholderMessage({
      messageFactory: (role, content) => ({
        id: 'message-1',
        role,
        content,
        created_at: '2026-06-12T00:00:00.000Z',
      }),
    });

    assert.deepEqual(message, {
      id: 'message-1',
      role: 'assistant',
      content: ASSISTANT_STREAM_PLACEHOLDER_TEXT,
      created_at: '2026-06-12T00:00:00.000Z',
    });
  });
});
