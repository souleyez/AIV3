import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  TRAFFIC_REPORT_API_URL,
  patchPublishedReport,
  patchReportApp,
  patchReportHtml,
} from './patch-sz02-traffic-report-postgres.mjs';

const APP_FIXTURE = `
  async function fetchFullDataset() {
    if ("DecompressionStream" in window) {
      const packedResponse = await fetch(\`data.json.gz?ts=\${Date.now()}\`);
      return new Response(packedResponse.body.pipeThrough(new DecompressionStream("gzip"))).json();
    }
    const response = await fetch(\`data.json?ts=\${Date.now()}\`);
    return response.json();
  }

  async function loadDataset() {
    return fetchFullDataset();
  }

      showToast("全部 48,771 条点位小时明细已加载");
`;

test('patchReportApp switches the full data load to the PostgreSQL API', () => {
  const patched = patchReportApp(APP_FIXTURE);
  assert.match(patched, new RegExp(TRAFFIC_REPORT_API_URL.replaceAll('/', '\\/')));
  assert.match(patched, /PostgreSQL 已加载/);
  assert.doesNotMatch(patched, /data\.json\.gz/);
  assert.equal(patchReportApp(patched), patched);
});

test('patchReportApp does not mistake an unrelated API URL mention for an applied patch', () => {
  const source = `// ${TRAFFIC_REPORT_API_URL}\n${APP_FIXTURE}`;
  const patched = patchReportApp(source);
  assert.doesNotMatch(patched, /data\.json\.gz/);
  assert.match(patched, /PostgreSQL 数据查询失败/);
});

test('patchReportApp refuses an unknown report build', () => {
  assert.throws(() => patchReportApp('console.log("unknown")'), /anchor count must be 1, got 0/);
});

test('patchReportApp refuses duplicate fetch anchors', () => {
  assert.throws(() => patchReportApp(`${APP_FIXTURE}\n${APP_FIXTURE}`), /anchor count must be 1, got 2/);
});

test('patchReportHtml adds an explicit PostgreSQL relation label once', () => {
  const html = `<div>${'        <p><b>平均停留</b><span>小时值均为 0，按不可用处理</span></p>'}</div>`;
  const patched = patchReportHtml(html);
  assert.match(patched, /mall_sz02\.traffic_hourly_fact/);
  assert.equal(patchReportHtml(patched), patched);
});

test('patchReportHtml requires the exact data-info row before treating the page as patched', () => {
  const html = `<!-- mall_sz02.traffic_hourly_fact -->\n<div>${'        <p><b>平均停留</b><span>小时值均为 0，按不可用处理</span></p>'}</div>`;
  const patched = patchReportHtml(html);
  assert.equal(patched.match(/<b>查询存储<\/b>/g)?.length, 1);
});

test('patchReportHtml refuses duplicate data-info anchors', () => {
  const anchor = '        <p><b>平均停留</b><span>小时值均为 0，按不可用处理</span></p>';
  assert.throws(() => patchReportHtml(`${anchor}\n${anchor}`), /anchor count must be 1, got 2/);
});

test('patchPublishedReport backs up both live files before atomically changing them', async (context) => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'sz02-report-patch-'));
  context.after(() => rm(root, { recursive: true, force: true }));
  const reportRoot = path.join(root, 'report');
  const backupRoot = path.join(root, 'backups');
  await mkdir(reportRoot, { recursive: true });
  const html = `<div>${'        <p><b>平均停留</b><span>小时值均为 0，按不可用处理</span></p>'}</div>`;
  await Promise.all([
    writeFile(path.join(reportRoot, 'app.js'), APP_FIXTURE, 'utf8'),
    writeFile(path.join(reportRoot, 'index.html'), html, 'utf8'),
  ]);

  const receipt = await patchPublishedReport({ reportRoot, backupRoot });
  assert.equal(receipt.changed, true);
  assert.match(await readFile(path.join(reportRoot, 'app.js'), 'utf8'), /PostgreSQL 已加载/);
  assert.match(await readFile(path.join(reportRoot, 'index.html'), 'utf8'), /mall_sz02\.traffic_hourly_fact/);
  assert.deepEqual((await readdir(receipt.backupPath)).sort(), ['app.js', 'index.html']);
  assert.equal(await readFile(path.join(receipt.backupPath, 'app.js'), 'utf8'), APP_FIXTURE);

  const second = await patchPublishedReport({ reportRoot, backupRoot });
  assert.equal(second.changed, false);
  assert.equal(second.backupPath, null);
});

test('patchPublishedReport restores both originals when the second live write fails', async (context) => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'sz02-report-rollback-'));
  context.after(() => rm(root, { recursive: true, force: true }));
  const reportRoot = path.join(root, 'report');
  const backupRoot = path.join(root, 'backups');
  await mkdir(reportRoot, { recursive: true });
  const html = `<div>${'        <p><b>平均停留</b><span>小时值均为 0，按不可用处理</span></p>'}</div>`;
  await Promise.all([
    writeFile(path.join(reportRoot, 'app.js'), APP_FIXTURE, 'utf8'),
    writeFile(path.join(reportRoot, 'index.html'), html, 'utf8'),
  ]);

  let writes = 0;
  const atomicWriter = async (filePath, content) => {
    writes += 1;
    if (writes === 2) throw new Error('simulated index write failure');
    await writeFile(filePath, content, 'utf8');
  };
  await assert.rejects(
    patchPublishedReport({ reportRoot, backupRoot, atomicWriter }),
    /original files restored/,
  );
  assert.equal(await readFile(path.join(reportRoot, 'app.js'), 'utf8'), APP_FIXTURE);
  assert.equal(await readFile(path.join(reportRoot, 'index.html'), 'utf8'), html);
});
