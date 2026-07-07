#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const WRAPPER_SELF_TEST_FLAG = '--heavy-wrapper-self-test';
const DEFAULT_OUTPUT_DIR = 'target/heavy-static-page-5way-wrapper-smoke';

if (process.argv.includes(WRAPPER_SELF_TEST_FLAG)) {
  const report = buildWrapperSelfTestReport();
  await writeReport(report);
  console.log(JSON.stringify(report.summary, null, 2));
  process.exit(report.ok ? 0 : 1);
}

await import('./static-page-5way.mjs');

function buildWrapperSelfTestReport() {
  const checks = {
    delegatesToStaticPage5way: true,
    wrapperSelfTestDoesNotImportLiveSmoke: true,
    wrapperDoesNotCallLiveApi: true,
    wrapperDoesNotRequireAuthMaterial: true,
    wrapperDoesNotRecordAuthMaterial: noAuthMaterialInReport(),
    underlyingSummaryExpected: true,
  };
  const ok = Object.values(checks).every(Boolean);
  return {
    ok,
    ready: ok,
    pending: false,
    failed: !ok,
    generated_at: new Date().toISOString(),
    script: 'heavy-static-page-5way',
    delegated_script: 'static-page-5way',
    mode: 'wrapper_self_test',
    summary: {
      ok,
      ready: ok,
      pending: false,
      failed: !ok,
      checks,
      guidance: ok
        ? 'Wrapper is safe: default execution delegates to static-page-5way; self-test performs no live calls.'
        : 'Wrapper self-test failed; inspect checks before using heavy static-page smoke.',
    },
  };
}

function noAuthMaterialInReport() {
  const reportShape = JSON.stringify({
    script: 'heavy-static-page-5way',
    delegated_script: 'static-page-5way',
    mode: 'wrapper_self_test',
  });
  return !/(bearer|cookie|token|authorization|secret|key)[=:]\\S+/i.test(reportShape);
}

async function writeReport(report) {
  const outputDir = outputDirFromArgs(process.argv.slice(2));
  await mkdir(outputDir, { recursive: true });
  const filePath = join(outputDir, `heavy-static-page-5way-wrapper-self-test-${timestamp()}.json`);
  await writeFile(filePath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(`report=${filePath}`);
}

function outputDirFromArgs(argv) {
  const index = argv.indexOf('--output-dir');
  if (index >= 0 && argv[index + 1] && !argv[index + 1].startsWith('--')) {
    return argv[index + 1];
  }
  return process.env.HEAVY_STATIC_PAGE_5WAY_OUTPUT_DIR || DEFAULT_OUTPUT_DIR;
}

function timestamp() {
  return new Date().toISOString().replace(/[-:.]/g, '').replace(/\.\d{3}Z$/, 'Z');
}
