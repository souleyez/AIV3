import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { renderPureThirdPartyGuideHtml } from './render-pure-third-party-guide-html.mjs';

const sampleMarkdown = `# V3 纯第三方模式对接文档

文档状态：对外草案。

## 1. 对接目标

- 用户在第三方页面提问；
- V3 按权限供料。

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
  assert.match(html, /<title>V3 纯第三方模式对接文档<\/title>/);
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

test('renderPureThirdPartyGuideHtml check mode rejects stale output', () => {
  const paths = tempPaths();
  fs.writeFileSync(paths.source, sampleMarkdown);
  fs.writeFileSync(paths.output, '<!doctype html><p>stale</p>');

  assert.throws(
    () => renderPureThirdPartyGuideHtml({ ...paths, check: true }),
    /HTML guide is stale/,
  );
});
