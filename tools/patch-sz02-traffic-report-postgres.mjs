#!/usr/bin/env node

import { cp, mkdir, mkdtemp, readFile, rename, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

export const DEFAULT_REPORT_ROOT = '/srv/aiv3/shared/objects/generated-artifacts/database-static-pages/sz02-july-traffic-20260716/traffic-dashboard';
export const DEFAULT_BACKUP_ROOT = '/srv/aiv3/deploy-backups';
export const TRAFFIC_REPORT_API_URL = '/api/v3/public/malls/SZ02/traffic/report-data';

const FETCH_FUNCTION_PATTERN = /  async function fetchFullDataset\(\) \{[\s\S]*?\n  \}\n\n  async function loadDataset/;
const LEGACY_TOAST = '      showToast("全部 48,771 条点位小时明细已加载");';
const POSTGRES_TOAST = '      showToast(`PostgreSQL 已加载 ${numberFormat.format(fullDataset.source.rowCount)} 条点位小时明细`);';
const DATA_INFO_ANCHOR = '        <p><b>平均停留</b><span>小时值均为 0，按不可用处理</span></p>';
const DATA_INFO_POSTGRES = '        <p><b>查询存储</b><span>PostgreSQL · mall_sz02.traffic_hourly_fact</span></p>';

function occurrenceCount(source, value) {
  return source.split(value).length - 1;
}

function postgresFetchFunction() {
  return `  async function fetchFullDataset() {
    const response = await fetch(\`${TRAFFIC_REPORT_API_URL}?ts=\${Date.now()}\`, {
      cache: "no-store",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(\`PostgreSQL 数据查询失败：HTTP \${response.status}\`);
    }
    return response.json();
  }

  async function loadDataset`;
}

export function patchReportApp(source) {
  if (typeof source !== 'string' || !source.trim()) {
    throw new Error('app.js is empty');
  }

  const fetchMatches = source.match(new RegExp(FETCH_FUNCTION_PATTERN.source, 'g')) ?? [];
  if (fetchMatches.length !== 1) {
    throw new Error(`app.js fetchFullDataset anchor count must be 1, got ${fetchMatches.length}`);
  }
  const [fetchMatch] = fetchMatches;
  const postgresFunction = postgresFetchFunction();
  let patched = fetchMatch === postgresFunction
    ? source
    : source.replace(FETCH_FUNCTION_PATTERN, postgresFunction);

  const postgresToastCount = occurrenceCount(patched, POSTGRES_TOAST);
  const legacyToastCount = occurrenceCount(patched, LEGACY_TOAST);
  if (postgresToastCount === 0) {
    if (legacyToastCount !== 1) {
      throw new Error(`app.js full-dataset toast anchor count must be 1, got ${legacyToastCount}`);
    }
    patched = patched.replace(LEGACY_TOAST, POSTGRES_TOAST);
  } else if (postgresToastCount !== 1 || legacyToastCount !== 0) {
    throw new Error('app.js contains ambiguous PostgreSQL/legacy toast anchors');
  }

  return patched;
}

export function patchReportHtml(source) {
  if (typeof source !== 'string' || !source.trim()) {
    throw new Error('index.html is empty');
  }
  const postgresInfoCount = occurrenceCount(source, DATA_INFO_POSTGRES);
  const anchorCount = occurrenceCount(source, DATA_INFO_ANCHOR);
  if (postgresInfoCount === 1 && anchorCount === 1) {
    return source;
  }
  if (postgresInfoCount !== 0) {
    throw new Error(`index.html PostgreSQL data-info row count must be 0 or 1, got ${postgresInfoCount}`);
  }
  if (anchorCount !== 1) {
    throw new Error(`index.html data-info anchor count must be 1, got ${anchorCount}`);
  }
  return source.replace(DATA_INFO_ANCHOR, `${DATA_INFO_ANCHOR}\n${DATA_INFO_POSTGRES}`);
}

function parseArgs(argv) {
  const options = {
    reportRoot: process.env.SZ02_TRAFFIC_REPORT_ROOT || DEFAULT_REPORT_ROOT,
    backupRoot: process.env.AIV3_DEPLOY_BACKUP_ROOT || DEFAULT_BACKUP_ROOT,
    dryRun: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--dry-run') {
      options.dryRun = true;
    } else if (argument === '--report-root') {
      options.reportRoot = argv[++index];
    } else if (argument === '--backup-root') {
      options.backupRoot = argv[++index];
    } else {
      throw new Error(`unknown argument: ${argument}`);
    }
  }
  if (!options.reportRoot || !options.backupRoot) {
    throw new Error('report and backup roots are required');
  }
  return options;
}

function timestamp() {
  return new Date().toISOString().replaceAll('-', '').replaceAll(':', '').replace(/\.\d{3}Z$/, 'Z');
}

async function atomicWrite(filePath, content) {
  const temporaryPath = `${filePath}.postgres-${process.pid}.tmp`;
  await writeFile(temporaryPath, content, 'utf8');
  await rename(temporaryPath, filePath);
}

export async function patchPublishedReport({
  reportRoot,
  backupRoot,
  dryRun = false,
  atomicWriter = atomicWrite,
}) {
  const appPath = path.join(reportRoot, 'app.js');
  const htmlPath = path.join(reportRoot, 'index.html');
  const [appSource, htmlSource] = await Promise.all([
    readFile(appPath, 'utf8'),
    readFile(htmlPath, 'utf8'),
  ]);
  const patchedApp = patchReportApp(appSource);
  const patchedHtml = patchReportHtml(htmlSource);
  const appChanged = patchedApp !== appSource;
  const htmlChanged = patchedHtml !== htmlSource;

  if (!appChanged && !htmlChanged) {
    return { changed: false, dryRun, reportRoot, backupPath: null };
  }
  if (dryRun) {
    return { changed: true, dryRun: true, reportRoot, backupPath: null };
  }

  await mkdir(backupRoot, { recursive: true });
  const backupPath = await mkdtemp(
    path.join(backupRoot, `sz02-traffic-report-postgres-${timestamp()}-`),
  );
  await Promise.all([
    cp(appPath, path.join(backupPath, 'app.js'), { errorOnExist: true, force: false }),
    cp(htmlPath, path.join(backupPath, 'index.html'), { errorOnExist: true, force: false }),
  ]);
  try {
    if (appChanged) await atomicWriter(appPath, patchedApp);
    if (htmlChanged) await atomicWriter(htmlPath, patchedHtml);
  } catch (error) {
    const rollbackErrors = [];
    for (const [filePath, originalSource] of [
      [appPath, appSource],
      [htmlPath, htmlSource],
    ]) {
      try {
        await atomicWriter(filePath, originalSource);
      } catch (rollbackError) {
        rollbackErrors.push(`${path.basename(filePath)}: ${rollbackError.message}`);
      }
    }
    const suffix = rollbackErrors.length
      ? `; rollback also failed (${rollbackErrors.join(', ')})`
      : '; original files restored';
    throw new Error(`live report update failed: ${error.message}${suffix}`);
  }

  return { changed: true, dryRun: false, reportRoot, backupPath };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const result = await patchPublishedReport(options);
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

if (import.meta.url === `file://${process.argv[1]?.replaceAll('\\', '/')}`) {
  main().catch((error) => {
    process.stderr.write(`sz02 report patch failed: ${error.message}\n`);
    process.exitCode = 1;
  });
}
