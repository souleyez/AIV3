#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';

const DEFAULT_OUTPUT_DIR = 'target/p2-summary-only-dry-run-smoke';
const DEFAULT_BIN_DIR = 'target/release';
const DEFAULT_KIND = 'procedure_steps,table_structure';
const DEFAULT_LIMIT = 5;

function parseArgs(argv) {
  const args = {
    datasetId: process.env.P2_SUMMARY_ONLY_DATASET_ID || '',
    documentId: process.env.P2_SUMMARY_ONLY_DOCUMENT_ID || '',
    limit: Number(process.env.P2_SUMMARY_ONLY_LIMIT || DEFAULT_LIMIT),
    kind: process.env.P2_SUMMARY_ONLY_KIND || DEFAULT_KIND,
    binDir: process.env.P2_SUMMARY_ONLY_BIN_DIR || DEFAULT_BIN_DIR,
    envFile: process.env.P2_SUMMARY_ONLY_ENV_FILE || '',
    outputDir: process.env.P2_SUMMARY_ONLY_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    selfTest: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--dataset-id') {
      args.datasetId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--document-id') {
      args.documentId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--limit') {
      args.limit = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--kind') {
      args.kind = requireValue(arg, next);
      index += 1;
    } else if (arg === '--bin-dir') {
      args.binDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--env-file') {
      args.envFile = requireValue(arg, next);
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (!args.selfTest && !args.datasetId) {
    throw new Error('--dataset-id or P2_SUMMARY_ONLY_DATASET_ID is required');
  }
  if (!Number.isInteger(args.limit) || args.limit < 1 || args.limit > 20) {
    throw new Error('--limit must be an integer from 1 to 20');
  }
  if (!args.kind.trim()) {
    throw new Error('--kind must not be empty');
  }
  return args;
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`Usage:
  node scripts/smoke/p2-summary-only-dry-run.mjs \\
    --dataset-id <dataset-uuid> \\
    --limit 5 \\
    --env-file /etc/aiv3/aiv3.env

Optional:
  --document-id <document-uuid>      scope all three commands to one document
  --kind procedure_steps,table_structure
  --bin-dir target/release
  --output-dir target/p2-summary-only-dry-run-smoke
  --self-test                       validate command construction and parser fixtures only

This smoke always passes --dry-run --summary-only --pretty to the backfill
binaries. It has no option to run --confirm-real-run.
`);
}

async function loadEnvFile(path) {
  if (!path) {
    return {};
  }
  const text = await readFile(path, 'utf8');
  const env = {};
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) {
      continue;
    }
    const normalized = line.startsWith('export ') ? line.slice('export '.length).trim() : line;
    const separator = normalized.indexOf('=');
    if (separator <= 0) {
      continue;
    }
    const key = normalized.slice(0, separator).trim();
    let value = normalized.slice(separator + 1).trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
      continue;
    }
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1);
    }
    env[key] = value;
  }
  return env;
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

function commandSpecs(args) {
  const scope = ['--dataset-id', args.datasetId];
  if (args.documentId) {
    scope.push('--document-id', args.documentId);
  } else {
    scope.push('--limit', String(args.limit));
  }
  return [
    {
      key: 'fingerprint',
      binary: 'document-fingerprint-backfill',
      args: [...scope, '--dry-run', '--summary-only', '--pretty'],
    },
    {
      key: 'fact_index',
      binary: 'fact-index-backfill',
      args: [...scope, '--dry-run', '--summary-only', '--pretty'],
    },
    {
      key: 'enrichment',
      binary: 'document-enrichment-backfill',
      args: [...scope, '--kind', args.kind, '--dry-run', '--summary-only', '--pretty'],
    },
  ];
}

function assertSafeCommandSpec(spec) {
  if (spec.args.includes('--confirm-real-run')) {
    throw new Error(`${spec.key} unexpectedly includes --confirm-real-run`);
  }
  for (const required of ['--dry-run', '--summary-only', '--pretty']) {
    if (!spec.args.includes(required)) {
      throw new Error(`${spec.key} missing required ${required}`);
    }
  }
}

async function runCommand(spec, args, runEnv, outputDir) {
  assertSafeCommandSpec(spec);
  const binaryPath = resolve(args.binDir, spec.binary);
  const startedAt = Date.now();
  const result = await spawnCapture(binaryPath, spec.args, {
    cwd: process.cwd(),
    env: { ...process.env, ...runEnv },
  });
  const stdoutPath = join(outputDir, `${spec.key}.stdout.txt`);
  const stderrPath = join(outputDir, `${spec.key}.stderr.txt`);
  await writeFile(stdoutPath, sanitizeText(result.stdout), 'utf8');
  await writeFile(stderrPath, sanitizeText(result.stderr), 'utf8');
  const parsed = result.exitCode === 0 ? extractLastJson(`${result.stdout}\n${result.stderr}`) : null;
  return {
    key: spec.key,
    binary: spec.binary,
    args: spec.args,
    exitCode: result.exitCode,
    durationMs: Date.now() - startedAt,
    stdoutPath,
    stderrPath,
    parsed,
    parsedOk: Boolean(parsed),
    stdoutBytes: Buffer.byteLength(result.stdout),
    stderrBytes: Buffer.byteLength(result.stderr),
    error: result.error ? String(result.error.message || result.error) : null,
  };
}

function spawnCapture(command, args, options) {
  return new Promise((resolvePromise) => {
    const child = spawn(command, args, {
      ...options,
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    let error = null;
    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString('utf8');
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString('utf8');
    });
    child.on('error', (event) => {
      error = event;
    });
    child.on('close', (exitCode) => {
      resolvePromise({
        exitCode: typeof exitCode === 'number' ? exitCode : 1,
        stdout,
        stderr,
        error,
      });
    });
  });
}

function extractLastJson(text) {
  const trimmed = text.trim();
  const starts = [];
  for (let index = 0; index < trimmed.length; index += 1) {
    if (trimmed[index] === '{') {
      starts.push(index);
    }
  }
  for (let index = starts.length - 1; index >= 0; index -= 1) {
    const candidate = trimmed.slice(starts[index]).trim();
    try {
      return JSON.parse(candidate);
    } catch {
      // Try the next earlier JSON object start.
    }
  }
  return null;
}

function sanitizeText(value) {
  return String(value || '')
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/gi, 'Bearer [REDACTED]')
    .replace(/(postgres(?:ql)?:\/\/[^:\s]+:)[^@\s]+@/gi, '$1[REDACTED]@')
    .replace(/(DATABASE_URL\s*=\s*)\S+/gi, '$1[REDACTED]')
    .replace(/(API[_-]?KEY\s*=\s*)\S+/gi, '$1[REDACTED]')
    .replace(/sk-[A-Za-z0-9_-]{8,}/g, 'sk-[REDACTED]');
}

function summarizeParsed(reports) {
  const byKey = Object.fromEntries(reports.map((item) => [item.key, item.parsed || {}]));
  return {
    fingerprint: {
      candidate_count: byKey.fingerprint.candidate_count ?? null,
      would_record_count: byKey.fingerprint.would_record_count ?? null,
      recorded_count: byKey.fingerprint.recorded_count ?? null,
      duplicate_count: byKey.fingerprint.duplicate_count ?? null,
      skipped_count: byKey.fingerprint.skipped_count ?? null,
      summary_only: byKey.fingerprint.summary_only ?? null,
      dry_run: byKey.fingerprint.dry_run ?? null,
    },
    fact_index: {
      document_count: byKey.fact_index.document_count ?? null,
      derived_fact_count: byKey.fact_index.derived_fact_count ?? null,
      inserted_fact_count: byKey.fact_index.inserted_fact_count ?? null,
      snapshot_updated: byKey.fact_index.snapshot_updated ?? null,
      fact_type_counts: byKey.fact_index.fact_type_counts ?? null,
      summary_only: byKey.fact_index.summary_only ?? null,
      dry_run: byKey.fact_index.dry_run ?? null,
    },
    enrichment: {
      document_count: byKey.enrichment.document_count ?? null,
      missing_fingerprint_count: byKey.enrichment.missing_fingerprint_count ?? null,
      would_enqueue_count: byKey.enrichment.would_enqueue_count ?? null,
      enqueued_count: byKey.enrichment.enqueued_count ?? null,
      enqueue_count_by_kind: byKey.enrichment.enqueue_count_by_kind ?? null,
      summary_only: byKey.enrichment.summary_only ?? null,
      dry_run: byKey.enrichment.dry_run ?? null,
    },
  };
}

function validateNonMutating(parsedSummary) {
  return [
    parsedSummary.fingerprint.recorded_count === 0,
    parsedSummary.fact_index.inserted_fact_count === 0,
    parsedSummary.fact_index.snapshot_updated === false,
    parsedSummary.enrichment.enqueued_count === 0,
    parsedSummary.fingerprint.summary_only === true,
    parsedSummary.fact_index.summary_only === true,
    parsedSummary.enrichment.summary_only === true,
    parsedSummary.fingerprint.dry_run === true,
    parsedSummary.fact_index.dry_run === true,
    parsedSummary.enrichment.dry_run === true,
  ].every(Boolean);
}

async function runSelfTest(args) {
  const runId = `${makeRunId()}-self-test`;
  const fixtureArgs = {
    ...args,
    datasetId: '00000000-0000-0000-0000-000000000002',
    documentId: '',
    limit: DEFAULT_LIMIT,
    kind: DEFAULT_KIND,
  };
  const specs = commandSpecs(fixtureArgs);
  specs.forEach(assertSafeCommandSpec);
  const textWithNotices = [
    '{"timestamp":"fixture","fields":{"message":"relation already exists, skipping"}}',
    '{',
    '  "document_count": 5,',
    '  "derived_fact_count": 281,',
    '  "inserted_fact_count": 0,',
    '  "snapshot_updated": false,',
    '  "dry_run": true,',
    '  "summary_only": true',
    '}',
  ].join('\n');
  const parsedFixture = extractLastJson(textWithNotices);
  const reports = [
    {
      key: 'fingerprint',
      parsed: {
        candidate_count: 5,
        would_record_count: 5,
        recorded_count: 0,
        duplicate_count: 5,
        skipped_count: 0,
        dry_run: true,
        summary_only: true,
      },
      exitCode: 0,
      parsedOk: true,
    },
    {
      key: 'fact_index',
      parsed: parsedFixture,
      exitCode: 0,
      parsedOk: true,
    },
    {
      key: 'enrichment',
      parsed: {
        document_count: 5,
        missing_fingerprint_count: 4,
        would_enqueue_count: 2,
        enqueued_count: 0,
        dry_run: true,
        summary_only: true,
      },
      exitCode: 0,
      parsedOk: true,
    },
  ];
  const parsedSummary = summarizeParsed(reports);
  const checks = {
    specCountIsThree: specs.length === 3,
    allSpecsAreDryRunSummaryOnly: specs.every((spec) => (
      spec.args.includes('--dry-run')
      && spec.args.includes('--summary-only')
      && spec.args.includes('--pretty')
      && !spec.args.includes('--confirm-real-run')
    )),
    limitScopeUsedWhenDocumentIdMissing: specs.every((spec) => spec.args.includes('--limit') && spec.args.includes('5')),
    fixtureJsonParsedAfterNoticeLines: parsedFixture?.derived_fact_count === 281,
    nonMutatingSummaryRecognized: validateNonMutating(parsedSummary),
    sanitizerRedactsSecrets: sanitizeText('DATABASE_URL=postgres://u:secret@example/db Bearer abc sk-1234567890')
      === 'DATABASE_URL=[REDACTED] Bearer [REDACTED] sk-[REDACTED]',
  };
  const ok = Object.values(checks).every(Boolean);
  const report = {
    runId,
    selfTest: true,
    ok,
    checks,
    commandShape: specs.map((spec) => ({
      key: spec.key,
      binary: spec.binary,
      args: spec.args,
    })),
    parsedSummary,
    generatedAt: new Date().toISOString(),
  };
  await mkdir(args.outputDir, { recursive: true });
  const reportPath = join(args.outputDir, `${runId}.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({ runId, selfTest: true, ok, checks, reportPath }, null, 2));
  if (!ok) {
    process.exitCode = 1;
  }
}

async function runLiveDryRun(args) {
  const runId = makeRunId();
  const outputDir = join(process.cwd(), args.outputDir, runId);
  await mkdir(outputDir, { recursive: true });
  const env = await loadEnvFile(args.envFile);
  const specs = commandSpecs(args);
  const commandReports = [];
  for (const spec of specs) {
    commandReports.push(await runCommand(spec, args, env, outputDir));
  }
  const parsedSummary = summarizeParsed(commandReports);
  const ok = commandReports.every((item) => item.exitCode === 0 && item.parsedOk)
    && validateNonMutating(parsedSummary);
  const report = {
    runId,
    selfTest: false,
    ok,
    datasetId: args.datasetId,
    documentId: args.documentId || null,
    limit: args.documentId ? null : args.limit,
    kind: args.kind,
    envFileLoaded: Boolean(args.envFile),
    envValuePrinted: false,
    outputDir,
    commandReports,
    parsedSummary,
    safety: {
      dryRunOnly: true,
      summaryOnly: true,
      confirmRealRunAccepted: false,
      fingerprintRecordsWritten: parsedSummary.fingerprint.recorded_count,
      factsInserted: parsedSummary.fact_index.inserted_fact_count,
      snapshotUpdated: parsedSummary.fact_index.snapshot_updated,
      enrichmentRunsEnqueued: parsedSummary.enrichment.enqueued_count,
    },
    generatedAt: new Date().toISOString(),
  };
  const reportPath = join(outputDir, 'report.json');
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({
    runId,
    ok,
    datasetId: args.datasetId,
    documentId: args.documentId || null,
    limit: args.documentId ? null : args.limit,
    parsedSummary,
    reportPath,
  }, null, 2));
  if (!ok) {
    process.exitCode = 1;
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) {
    await runSelfTest(args);
    return;
  }
  await runLiveDryRun(args);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
