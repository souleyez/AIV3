import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  appendArtifactLinkText,
  assistantRunStreamArtifactLink,
  assistantRunStreamDisplayText,
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
});
