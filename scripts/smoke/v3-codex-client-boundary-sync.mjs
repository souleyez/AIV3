#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT_DIR = path.resolve(fileURLToPath(new URL('../..', import.meta.url)));
const DEFAULT_SOURCE_DOC = 'docs/integrations/v3-codex-client-boundary-contract.md';
const DEFAULT_MIRROR_DOC = '../../codex-web/docs/v3-codex-client-boundary-contract.md';
const DEFAULT_OUTPUT_DIR = 'target/v3-codex-client-boundary-sync';

function parseArgs(argv) {
  const args = {
    sourceDoc: process.env.V3_CODEX_CLIENT_BOUNDARY_SOURCE_DOC || DEFAULT_SOURCE_DOC,
    mirrorDoc: process.env.V3_CODEX_CLIENT_BOUNDARY_MIRROR_DOC || DEFAULT_MIRROR_DOC,
    outputDir: process.env.V3_CODEX_CLIENT_BOUNDARY_SYNC_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    allowMissingMirror: parseBoolean(process.env.V3_CODEX_CLIENT_BOUNDARY_ALLOW_MISSING_MIRROR),
    selfTest: false,
    pretty: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--source-doc') {
      args.sourceDoc = requireValue(arg, next);
      index += 1;
    } else if (arg === '--mirror-doc') {
      args.mirrorDoc = requireValue(arg, next);
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--allow-missing-mirror') {
      args.allowMissingMirror = true;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  return args;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:v3-codex-client-boundary-sync
  npm run smoke:v3-codex-client-boundary-sync -- --pretty

Checks that the V3 canonical boundary document is byte-identical to the
codex-web mirror copy. This script is read-only and never syncs files itself.

Defaults:
  source: ${DEFAULT_SOURCE_DOC}
  mirror: ${DEFAULT_MIRROR_DOC}
`);
}

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y', 'on'].includes(String(value || '').trim().toLowerCase());
}

function requireValue(flag, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function resolvePath(value) {
  return path.isAbsolute(value) ? value : path.resolve(ROOT_DIR, value);
}

function sha256(text) {
  return createHash('sha256').update(text).digest('hex');
}

function firstDifferenceLine(left, right) {
  const leftLines = left.split(/\r?\n/);
  const rightLines = right.split(/\r?\n/);
  const max = Math.max(leftLines.length, rightLines.length);
  for (let index = 0; index < max; index += 1) {
    if (leftLines[index] !== rightLines[index]) {
      return {
        line: index + 1,
        source_excerpt: String(leftLines[index] || '').slice(0, 180),
        mirror_excerpt: String(rightLines[index] || '').slice(0, 180),
      };
    }
  }
  return null;
}

function sha256LooksValid(value) {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
}

function buildSummary(args, receipt) {
  const missingMirrorAllowed = receipt.mirror_bytes === null && args.allowMissingMirror === true;
  const ready = receipt.ok === true && receipt.in_sync === true;
  const pending = receipt.ok === true && missingMirrorAllowed;
  const failed = receipt.ok !== true;
  const checks = {
    terminalStateIsConsistent: [ready, pending, failed].filter(Boolean).length === 1,
    sourceDocumentRead: Number(receipt.source_bytes || 0) > 0,
    mirrorDocumentPresentOrAllowedMissing: receipt.mirror_bytes !== null || args.allowMissingMirror === true,
    inSyncOrAllowedMissing: receipt.in_sync === true || missingMirrorAllowed,
    checksumPresent: sha256LooksValid(receipt.source_sha256)
      && (receipt.mirror_sha256 === null || sha256LooksValid(receipt.mirror_sha256)),
    driftHasDifferenceWhenBothFilesExist: receipt.ok === true
      || receipt.mirror_bytes === null
      || Boolean(receipt.first_difference),
    diffExcerptBounded: !receipt.first_difference
      || (
        String(receipt.first_difference.source_excerpt || '').length <= 180
        && String(receipt.first_difference.mirror_excerpt || '').length <= 180
      ),
    readOnly: true,
  };
  return {
    ok: Object.values(checks).every(Boolean) && receipt.ok === true,
    checks,
    ready,
    pending,
    failed,
    selfTest: receipt.mode === 'self_test',
    in_sync: receipt.in_sync === true,
    allow_missing_mirror: args.allowMissingMirror === true,
    source_doc: receipt.source_doc || null,
    mirror_doc: receipt.mirror_doc || null,
    source_bytes: receipt.source_bytes ?? null,
    mirror_bytes: receipt.mirror_bytes ?? null,
    first_difference_line: receipt.first_difference?.line || null,
  };
}

async function readText(filePath, allowMissing = false) {
  try {
    return await readFile(filePath, 'utf8');
  } catch (error) {
    if (allowMissing && error.code === 'ENOENT') return null;
    throw error;
  }
}

async function writeReceipt(args, receipt) {
  await mkdir(args.outputDir, { recursive: true });
  const receiptPath = path.join(args.outputDir, 'receipt.json');
  await writeFile(receiptPath, JSON.stringify(receipt, null, args.pretty ? 2 : 0));
  return receiptPath;
}

function selfTest() {
  const left = 'alpha\nbeta\n';
  const right = 'alpha\ngamma\n';
  const diff = firstDifferenceLine(left, right);
  if (!diff || diff.line !== 2 || diff.source_excerpt !== 'beta' || diff.mirror_excerpt !== 'gamma') {
    throw new Error('firstDifferenceLine self-test failed');
  }
  const receipt = {
    mode: 'self_test',
    ok: true,
    in_sync: true,
    source_doc: 'self-test-source.md',
    mirror_doc: 'self-test-mirror.md',
    source_bytes: Buffer.byteLength(left),
    mirror_bytes: Buffer.byteLength(left),
    source_hash: sha256(left),
    mirror_hash: sha256(left),
    source_sha256: sha256(left),
    mirror_sha256: sha256(left),
    first_difference: null,
    self_test_cases: {
      firstDifferenceLineFindsChangedLine: diff.line === 2
        && diff.source_excerpt === 'beta'
        && diff.mirror_excerpt === 'gamma',
      missingMirrorCanBePending: false,
      driftReceiptFails: false,
    },
  };
  const missingMirrorReceipt = {
    ...receipt,
    mode: 'check',
    in_sync: false,
    mirror_bytes: null,
    mirror_sha256: null,
  };
  const missingMirrorSummary = buildSummary({ allowMissingMirror: true }, missingMirrorReceipt);
  const driftReceipt = {
    ...receipt,
    mode: 'check',
    ok: false,
    in_sync: false,
    first_difference: diff,
  };
  const driftSummary = buildSummary({ allowMissingMirror: false }, driftReceipt);
  receipt.self_test_cases.missingMirrorCanBePending = missingMirrorSummary.ok === true
    && missingMirrorSummary.pending === true
    && missingMirrorSummary.ready === false;
  receipt.self_test_cases.driftReceiptFails = driftSummary.ok === false
    && driftSummary.failed === true
    && driftSummary.checks.driftHasDifferenceWhenBothFilesExist === true;
  receipt.summary = buildSummary({ allowMissingMirror: false }, receipt);
  receipt.ok = receipt.summary.ok && Object.values(receipt.self_test_cases).every(Boolean);
  receipt.summary.ok = receipt.ok;
  return receipt;
}

async function checkSync(args) {
  const sourcePath = resolvePath(args.sourceDoc);
  const mirrorPath = resolvePath(args.mirrorDoc);
  const sourceText = await readText(sourcePath);
  const mirrorText = await readText(mirrorPath, args.allowMissingMirror);
  const sourceHash = sha256(sourceText);
  const mirrorHash = mirrorText === null ? null : sha256(mirrorText);
  const inSync = mirrorText !== null && sourceText === mirrorText;
  const receipt = {
    mode: 'check',
    ok: inSync || (mirrorText === null && args.allowMissingMirror),
    in_sync: inSync,
    source_doc: path.relative(ROOT_DIR, sourcePath).replaceAll('\\', '/'),
    mirror_doc: path.relative(ROOT_DIR, mirrorPath).replaceAll('\\', '/'),
    source_bytes: Buffer.byteLength(sourceText),
    mirror_bytes: mirrorText === null ? null : Buffer.byteLength(mirrorText),
    source_sha256: sourceHash,
    mirror_sha256: mirrorHash,
    first_difference: inSync || mirrorText === null ? null : firstDifferenceLine(sourceText, mirrorText),
  };
  receipt.summary = buildSummary(args, receipt);
  receipt.ok = receipt.summary.ok;
  if (!receipt.ok) {
    const detail = receipt.first_difference
      ? `first difference line ${receipt.first_difference.line}`
      : 'missing mirror';
    throw Object.assign(new Error(`V3/codex-web boundary contract drift detected: ${detail}`), {
      receipt,
    });
  }
  return receipt;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const receipt = args.selfTest ? selfTest() : await checkSync(args);
  const receiptPath = await writeReceipt(args, receipt);
  console.log(JSON.stringify({ ...receipt, receipt_path: receiptPath }, null, args.pretty ? 2 : 0));
}

main().catch(async (error) => {
  if (error.receipt) {
    const args = parseArgs(process.argv.slice(2));
    const receiptPath = await writeReceipt(args, error.receipt);
    console.error(JSON.stringify({ ...error.receipt, receipt_path: receiptPath }, null, args.pretty ? 2 : 0));
  }
  console.error(error?.message || String(error));
  process.exit(1);
});
