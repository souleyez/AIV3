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
  return {
    mode: 'self_test',
    ok: true,
    source_hash: sha256(left),
    mirror_hash: sha256(left),
  };
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
