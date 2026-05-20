import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  renderFullThirdPartyGuideHtml,
  renderPureThirdPartyGuideHtml,
} from './render-pure-third-party-guide-html.mjs';

const sampleMarkdown = `# V3 纯第三方简单版接口文档

文档状态：对外草案。

## 1. 对接目标

- 用户在第三方页面提问；
- V3 按本轮文档范围供料。

| 接口面 | 用途 |
| --- | --- |
| 文档接口 | 按需读取 |

\`\`\`mermaid
flowchart LR
  User --> V3
\`\`\`

\`\`\`http
POST /v1/external/channels/{connection_id}/events
\`\`\`
`;

function tempPaths() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-guide-html-'));
  return {
    dir,
    source: path.join(dir, 'guide.md'),
    output: path.join(dir, 'guide.html'),
  };
}

test('renderPureThirdPartyGuideHtml renders a review-friendly HTML document', () => {
  const paths = tempPaths();
  fs.writeFileSync(paths.source, sampleMarkdown);

  const result = renderPureThirdPartyGuideHtml(paths);

  assert.equal(result.checked, false);
  assert.ok(result.bytes > 0);
  const html = fs.readFileSync(paths.output, 'utf8');
  assert.match(html, /<title>V3 纯第三方简单版接口文档<\/title>/);
  assert.match(html, /<nav aria-label="文档目录">/);
  assert.match(html, /flow-board/);
  assert.match(html, /table-shell/);
  assert.match(html, /copy-code/);
  assert.match(html, /POST \/v1\/external\/channels\/\{connection_id\}\/events/);
});

test('renderPureThirdPartyGuideHtml check mode accepts current output', () => {
  const paths = tempPaths();
  fs.writeFileSync(paths.source, sampleMarkdown);
  renderPureThirdPartyGuideHtml(paths);

  const result = renderPureThirdPartyGuideHtml({ ...paths, check: true });

  assert.equal(result.checked, true);
  assert.ok(result.bytes > 0);
});

test('renderPureThirdPartyGuideHtml publishes explicit public HTML and Markdown copies', () => {
  const paths = tempPaths();
  const publicHtmlOutput = path.join(paths.dir, 'public', 'external-integrations', 'guide.html');
  const publicMarkdownOutput = path.join(paths.dir, 'public', 'external-integrations', 'guide.md');
  fs.writeFileSync(paths.source, sampleMarkdown);

  const result = renderPureThirdPartyGuideHtml({
    ...paths,
    publicHtmlOutput,
    publicMarkdownOutput,
  });

  assert.equal(result.publicHtmlOutputPath, publicHtmlOutput);
  assert.equal(result.publicMarkdownOutputPath, publicMarkdownOutput);
  assert.equal(fs.readFileSync(publicHtmlOutput, 'utf8'), fs.readFileSync(paths.output, 'utf8'));
  assert.equal(fs.readFileSync(publicMarkdownOutput, 'utf8'), sampleMarkdown);
  assert.equal(
    renderPureThirdPartyGuideHtml({
      ...paths,
      publicHtmlOutput,
      publicMarkdownOutput,
      check: true,
    }).checked,
    true,
  );
});

test('renderFullThirdPartyGuideHtml renders complete integration docs without pure flow substitution', () => {
  const paths = tempPaths();
  const markdown = sampleMarkdown.replace('V3 纯第三方简单版接口文档', 'V3 第三方接入说明书');
  fs.writeFileSync(paths.source, markdown);

  renderFullThirdPartyGuideHtml(paths);

  const html = fs.readFileSync(paths.output, 'utf8');
  assert.match(html, /<title>V3 第三方完整对接文档<\/title>/);
  assert.match(html, /V3 \/ THIRD PARTY API/);
  assert.match(html, /<span>mermaid<\/span>/);
  assert.doesNotMatch(html, /<div class="flow-board"/);
});

test('renderPureThirdPartyGuideHtml check mode rejects stale output', () => {
  const paths = tempPaths();
  fs.writeFileSync(paths.source, sampleMarkdown);
  fs.writeFileSync(paths.output, '<!doctype html><p>stale</p>');

  assert.throws(
    () => renderPureThirdPartyGuideHtml({ ...paths, check: true }),
    /HTML guide is stale/,
  );
});
