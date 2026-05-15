#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const defaultSourcePath = path.join(repoRoot, 'docs/integrations/pure-third-party-integration-guide.zh-CN.md');
const defaultOutputPath = path.join(repoRoot, 'docs/integrations/pure-third-party-integration-guide.zh-CN.html');

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

function inlineMarkdown(value) {
  let html = escapeHtml(value);
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
  html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, label, href) => {
    return `<a href="${escapeHtml(href)}">${label}</a>`;
  });
  return html;
}

function slugFor(index) {
  return `section-${String(index).padStart(2, '0')}`;
}

function renderTable(lines) {
  const rows = lines
    .map((line) => line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((cell) => cell.trim()))
    .filter((cells) => !cells.every((cell) => /^:?-{3,}:?$/.test(cell)));
  const [head, ...body] = rows;
  return [
    '<div class="table-shell"><table>',
    '<thead><tr>',
    ...head.map((cell) => `<th>${inlineMarkdown(cell)}</th>`),
    '</tr></thead>',
    '<tbody>',
    ...body.map((row) => `<tr>${row.map((cell) => `<td>${inlineMarkdown(cell)}</td>`).join('')}</tr>`),
    '</tbody></table></div>',
  ].join('');
}

function renderArchitectureDiagram() {
  const steps = [
    ['外部用户', '输入问题或操作请求'],
    ['第三方聊天页面/门户', '提交标准化消息事件'],
    ['V3 对外接入网关', '校验连接、幂等、通道策略'],
    ['第三方用户/权限系统', '解析身份、部门、组、角色'],
    ['第三方文档库', '按版本和 ACL 获取可见证据'],
    ['V3 助手运行时', '权限过滤后供料给模型'],
    ['第三方产物/业务系统', '执行已确认动作并回传结果'],
  ];
  return `<div class="flow-board" aria-label="纯第三方模式总体流程">
    ${steps
      .map(
        ([title, desc], index) => `
          <div class="flow-node">
            <span class="flow-index">${index + 1}</span>
            <strong>${title}</strong>
            <small>${desc}</small>
          </div>
        `,
      )
      .join('<span class="flow-arrow">→</span>')}
  </div>`;
}

function renderCodeBlock(language, code) {
  if (language === 'mermaid') {
    return renderArchitectureDiagram();
  }
  const label = language || 'text';
  return `<figure class="code-card">
    <figcaption><span>${escapeHtml(label)}</span><button type="button" class="copy-code">复制</button></figcaption>
    <pre><code>${escapeHtml(code.replace(/\n$/, ''))}</code></pre>
  </figure>`;
}

function markdownToHtml(markdown) {
  const lines = markdown.replace(/\r\n/g, '\n').split('\n');
  const html = [];
  const nav = [];
  let index = 0;
  let sectionIndex = 0;
  let inList = false;
  let inOrderedList = false;

  function closeList() {
    if (inList) {
      html.push('</ul>');
      inList = false;
    }
    if (inOrderedList) {
      html.push('</ol>');
      inOrderedList = false;
    }
  }

  while (index < lines.length) {
    const line = lines[index];
    const trimmed = line.trim();

    if (!trimmed) {
      closeList();
      index += 1;
      continue;
    }

    if (trimmed.startsWith('```')) {
      closeList();
      const language = trimmed.slice(3).trim();
      index += 1;
      const codeLines = [];
      while (index < lines.length && !lines[index].trim().startsWith('```')) {
        codeLines.push(lines[index]);
        index += 1;
      }
      index += 1;
      html.push(renderCodeBlock(language, codeLines.join('\n')));
      continue;
    }

    if (/^\|.*\|$/.test(trimmed) && index + 1 < lines.length && /^\|\s*:?-{3,}/.test(lines[index + 1].trim())) {
      closeList();
      const tableLines = [trimmed];
      index += 1;
      while (index < lines.length && /^\|.*\|$/.test(lines[index].trim())) {
        tableLines.push(lines[index].trim());
        index += 1;
      }
      html.push(renderTable(tableLines));
      continue;
    }

    const heading = /^(#{1,6})\s+(.+)$/.exec(trimmed);
    if (heading) {
      closeList();
      const level = heading[1].length;
      const title = heading[2].trim();
      if (level === 1) {
        html.push(`<h1>${inlineMarkdown(title)}</h1>`);
      } else {
        sectionIndex += 1;
        const id = slugFor(sectionIndex);
        if (level === 2) {
          nav.push({ id, title });
        }
        html.push(`<h${level} id="${id}">${inlineMarkdown(title)}</h${level}>`);
      }
      index += 1;
      continue;
    }

    const unordered = /^-\s+(.+)$/.exec(trimmed);
    if (unordered) {
      if (inOrderedList) {
        html.push('</ol>');
        inOrderedList = false;
      }
      if (!inList) {
        html.push('<ul>');
        inList = true;
      }
      html.push(`<li>${inlineMarkdown(unordered[1])}</li>`);
      index += 1;
      continue;
    }

    const ordered = /^\d+\.\s+(.+)$/.exec(trimmed);
    if (ordered) {
      if (inList) {
        html.push('</ul>');
        inList = false;
      }
      if (!inOrderedList) {
        html.push('<ol>');
        inOrderedList = true;
      }
      html.push(`<li>${inlineMarkdown(ordered[1])}</li>`);
      index += 1;
      continue;
    }

    closeList();
    const paragraph = [trimmed];
    index += 1;
    while (
      index < lines.length &&
      lines[index].trim() &&
      !lines[index].trim().startsWith('```') &&
      !/^(#{1,6})\s+/.test(lines[index].trim()) &&
      !/^-\s+/.test(lines[index].trim()) &&
      !/^\d+\.\s+/.test(lines[index].trim()) &&
      !/^\|.*\|$/.test(lines[index].trim())
    ) {
      paragraph.push(lines[index].trim());
      index += 1;
    }
    html.push(`<p>${inlineMarkdown(paragraph.join(' '))}</p>`);
  }

  closeList();
  return { body: html.join('\n'), nav };
}

function renderDocument({ body, nav }) {
  const navHtml = nav
    .map(({ id, title }) => `<a href="#${id}">${escapeHtml(title.replace(/^\d+\.\s*/, ''))}</a>`)
    .join('\n');

  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>V3 纯第三方模式对接文档</title>
  <style>
    :root {
      color-scheme: light;
      --ink: #17211f;
      --muted: #66726e;
      --line: #d8e2de;
      --paper: #fbfcf8;
      --panel: #ffffff;
      --deep: #0c2f33;
      --teal: #0f766e;
      --green: #1f9d6e;
      --amber: #d9912b;
      --coral: #c84d3c;
      --code: #0e1b1d;
      --shadow: 0 18px 50px rgba(20, 42, 41, 0.12);
    }

    * { box-sizing: border-box; }
    html { scroll-behavior: smooth; }
    body {
      margin: 0;
      color: var(--ink);
      background:
        linear-gradient(90deg, rgba(15, 118, 110, 0.08) 1px, transparent 1px),
        linear-gradient(180deg, rgba(15, 118, 110, 0.06) 1px, transparent 1px),
        var(--paper);
      background-size: 36px 36px;
      font-family: "Noto Serif SC", "Source Han Serif SC", "Songti SC", Georgia, serif;
      font-size: 17px;
      line-height: 1.78;
    }

    a { color: var(--teal); text-decoration-thickness: 0.08em; text-underline-offset: 0.18em; }
    code {
      border: 1px solid rgba(15, 118, 110, 0.22);
      border-radius: 6px;
      padding: 0.08rem 0.34rem;
      background: rgba(15, 118, 110, 0.08);
      font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
      font-size: 0.9em;
    }

    .shell {
      display: grid;
      grid-template-columns: minmax(220px, 300px) minmax(0, 1fr);
      min-height: 100vh;
    }

    aside {
      position: sticky;
      top: 0;
      height: 100vh;
      padding: 28px 22px;
      overflow: auto;
      background: rgba(255, 255, 255, 0.78);
      border-right: 1px solid var(--line);
      backdrop-filter: blur(18px);
    }

    .brand {
      display: grid;
      gap: 8px;
      margin-bottom: 26px;
      padding-bottom: 22px;
      border-bottom: 1px solid var(--line);
    }
    .brand span {
      display: inline-flex;
      width: fit-content;
      padding: 3px 9px;
      border-radius: 999px;
      color: #fff;
      background: var(--deep);
      font: 700 12px/1.5 "Cascadia Code", monospace;
      letter-spacing: 0;
    }
    .brand strong {
      font-size: 22px;
      line-height: 1.2;
    }
    .brand small { color: var(--muted); }

    nav {
      display: grid;
      gap: 2px;
    }
    nav a {
      display: block;
      padding: 8px 10px;
      border-radius: 8px;
      color: var(--muted);
      font-size: 14px;
      line-height: 1.35;
      text-decoration: none;
    }
    nav a:hover {
      color: var(--ink);
      background: rgba(15, 118, 110, 0.09);
    }

    main {
      min-width: 0;
      padding: 38px clamp(22px, 5vw, 68px) 72px;
    }
    .hero {
      display: grid;
      gap: 24px;
      margin: 0 auto 38px;
      max-width: 1120px;
      padding: 42px clamp(24px, 4vw, 54px);
      color: #f5fffb;
      background: linear-gradient(135deg, #0c2f33 0%, #124b4c 48%, #22513c 100%);
      box-shadow: var(--shadow);
      border: 1px solid rgba(255, 255, 255, 0.12);
    }
    .hero h1 {
      margin: 0;
      max-width: 780px;
      font-size: clamp(36px, 7vw, 76px);
      line-height: 1.02;
      letter-spacing: 0;
    }
    .hero p {
      margin: 0;
      max-width: 820px;
      color: rgba(245, 255, 251, 0.82);
      font-size: 19px;
    }
    .hero-metrics {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 12px;
    }
    .metric {
      border: 1px solid rgba(255,255,255,0.18);
      padding: 15px;
      min-height: 96px;
      background: rgba(255,255,255,0.08);
    }
    .metric strong {
      display: block;
      font-size: 27px;
      line-height: 1.15;
    }
    .metric span {
      color: rgba(245,255,251,0.74);
      font-size: 13px;
    }

    article {
      max-width: 1120px;
      margin: 0 auto;
      padding: 32px clamp(20px, 4vw, 56px);
      background: rgba(255,255,255,0.9);
      border: 1px solid var(--line);
      box-shadow: var(--shadow);
    }
    article > h1 { display: none; }
    h2 {
      margin: 54px 0 14px;
      padding-top: 12px;
      border-top: 3px solid var(--deep);
      font-size: clamp(25px, 3.6vw, 38px);
      line-height: 1.2;
      letter-spacing: 0;
    }
    h3 {
      margin: 34px 0 10px;
      color: var(--deep);
      font-size: 23px;
      line-height: 1.3;
    }
    h4, h5, h6 {
      margin: 24px 0 8px;
      color: var(--deep);
    }
    p { margin: 0 0 16px; }
    ul, ol {
      margin: 0 0 18px;
      padding-left: 1.3rem;
    }
    li { margin: 6px 0; }
    li::marker { color: var(--teal); font-weight: 700; }

    .table-shell {
      width: 100%;
      overflow-x: auto;
      margin: 18px 0 26px;
      border: 1px solid var(--line);
      background: var(--panel);
    }
    table {
      width: 100%;
      border-collapse: collapse;
      min-width: 680px;
      font-size: 15px;
      line-height: 1.55;
    }
    th, td {
      padding: 12px 14px;
      border-bottom: 1px solid var(--line);
      vertical-align: top;
      text-align: left;
    }
    th {
      color: #fff;
      background: var(--deep);
      font-weight: 700;
    }
    tr:nth-child(even) td { background: rgba(15, 118, 110, 0.045); }

    .code-card {
      margin: 20px 0 28px;
      overflow: hidden;
      border: 1px solid #23393a;
      background: var(--code);
      color: #e8fffb;
    }
    .code-card figcaption {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 12px;
      padding: 9px 12px;
      border-bottom: 1px solid rgba(255,255,255,0.12);
      color: #b9d9d4;
      font: 700 12px/1.4 "Cascadia Code", monospace;
    }
    .copy-code {
      min-height: 32px;
      border: 1px solid rgba(255,255,255,0.18);
      border-radius: 8px;
      padding: 0 10px;
      color: #e8fffb;
      background: rgba(255,255,255,0.08);
      cursor: pointer;
      font: inherit;
    }
    pre {
      margin: 0;
      padding: 18px;
      overflow-x: auto;
      white-space: pre;
      font: 14px/1.65 "Cascadia Code", "SFMono-Regular", Consolas, monospace;
    }
    pre code {
      border: 0;
      padding: 0;
      background: transparent;
      color: inherit;
      font-size: inherit;
    }

    .flow-board {
      display: grid;
      grid-template-columns: repeat(7, minmax(120px, 1fr));
      align-items: stretch;
      gap: 8px;
      margin: 22px 0 32px;
      overflow-x: auto;
      padding: 4px 0 14px;
    }
    .flow-arrow {
      display: none;
    }
    .flow-node {
      position: relative;
      min-height: 142px;
      padding: 18px 14px;
      border: 1px solid var(--line);
      border-top: 5px solid var(--teal);
      background: #fff;
    }
    .flow-node:nth-child(4n) { border-top-color: var(--amber); }
    .flow-node:nth-child(4n + 2) { border-top-color: var(--green); }
    .flow-node:nth-child(4n + 3) { border-top-color: var(--coral); }
    .flow-index {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 28px;
      height: 28px;
      margin-bottom: 12px;
      color: #fff;
      background: var(--deep);
      border-radius: 50%;
      font: 700 13px/1 "Cascadia Code", monospace;
    }
    .flow-node strong {
      display: block;
      line-height: 1.25;
      margin-bottom: 8px;
    }
    .flow-node small {
      color: var(--muted);
      font-size: 13px;
      line-height: 1.45;
    }

    .doc-footer {
      max-width: 1120px;
      margin: 22px auto 0;
      color: var(--muted);
      font-size: 14px;
    }

    @media (max-width: 920px) {
      .shell { display: block; }
      aside {
        position: static;
        height: auto;
        border-right: 0;
        border-bottom: 1px solid var(--line);
      }
      nav {
        grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
      }
      main { padding: 20px 14px 46px; }
      .hero { padding: 30px 20px; }
      .hero-metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); }
      article { padding: 24px 18px; }
      .flow-board { grid-template-columns: repeat(7, 180px); }
    }

    @media (max-width: 560px) {
      body { font-size: 16px; }
      .hero h1 { font-size: 34px; }
      .hero-metrics { grid-template-columns: 1fr; }
      table { min-width: 620px; }
    }

    @media print {
      body { background: #fff; }
      .shell { display: block; }
      aside, .copy-code { display: none; }
      main { padding: 0; }
      .hero, article { box-shadow: none; }
      article { border: 0; }
    }
  </style>
</head>
<body>
  <div class="shell">
    <aside>
      <div class="brand">
        <span>V3 / PURE THIRD PARTY</span>
        <strong>纯第三方模式</strong>
        <small>自建页面、文档库、用户权限、产物与业务动作的对接说明。</small>
      </div>
      <nav aria-label="文档目录">
        ${navHtml}
      </nav>
    </aside>
    <main>
      <section class="hero">
        <h1>V3 纯第三方模式对接文档</h1>
        <p>把原来的长 Markdown 拆成可浏览的交付页面：左侧目录快速定位，正文保留完整接口细节，代码块可复制，表格和流程更适合发给第三方技术团队评审。</p>
        <div class="hero-metrics">
          <div class="metric"><strong>4</strong><span>核心闭环：问答、权限、产物、事务</span></div>
          <div class="metric"><strong>6</strong><span>第三方接口面：聊天、文档、用户、ACL、产物、动作</span></div>
          <div class="metric"><strong>0</strong><span>无需整库搬迁，支持按需读取与增量索引</span></div>
          <div class="metric"><strong>V3</strong><span>统一控制权限、供料、风控和审计</span></div>
        </div>
      </section>
      <article>
        ${body}
      </article>
      <p class="doc-footer">Generated from <code>docs/integrations/pure-third-party-integration-guide.zh-CN.md</code>. Keep Markdown as the editable source and HTML as the human-facing review surface.</p>
    </main>
  </div>
  <script>
    document.querySelectorAll('.copy-code').forEach((button) => {
      button.addEventListener('click', async () => {
        const code = button.closest('.code-card').querySelector('code').innerText;
        try {
          await navigator.clipboard.writeText(code);
          button.textContent = '已复制';
          setTimeout(() => { button.textContent = '复制'; }, 1200);
        } catch {
          button.textContent = '复制失败';
          setTimeout(() => { button.textContent = '复制'; }, 1200);
        }
      });
    });
  </script>
</body>
</html>
`;
}

function renderPureThirdPartyGuideHtml({
  source = defaultSourcePath,
  output = defaultOutputPath,
  check = false,
} = {}) {
  const markdown = fs.readFileSync(source, 'utf8');
  const rendered = renderDocument(markdownToHtml(markdown));
  if (check) {
    const current = fs.existsSync(output) ? fs.readFileSync(output, 'utf8') : '';
    if (current !== rendered) {
      throw new Error(
        `HTML guide is stale: ${path.relative(repoRoot, output)}. Run npm run build:pure-third-party-guide-html.`,
      );
    }
    return { outputPath: output, bytes: Buffer.byteLength(rendered), checked: true };
  }
  fs.writeFileSync(output, rendered);
  return { outputPath: output, bytes: Buffer.byteLength(rendered), checked: false };
}

function main(argv = process.argv.slice(2)) {
  const check = argv.includes('--check');
  const result = renderPureThirdPartyGuideHtml({ check });
  const relativeOutput = path.relative(repoRoot, result.outputPath);
  console.log(check ? `HTML guide is up to date: ${relativeOutput}` : `Rendered ${relativeOutput}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}

export {
  markdownToHtml,
  renderDocument,
  renderPureThirdPartyGuideHtml,
};
