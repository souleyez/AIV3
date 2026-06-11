#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_OUTPUT_DIR = 'target/document-fingerprint-inventory-smoke';
const DEFAULT_DATABASE_URL_ENV = 'PLATFORM_DATABASE_URL';
const DEFAULT_DATASET_LIMIT = 20;

function parseArgs(argv) {
  const args = {
    envFile: process.env.DOCUMENT_FINGERPRINT_INVENTORY_ENV_FILE || '',
    databaseUrlEnv: process.env.DOCUMENT_FINGERPRINT_INVENTORY_DATABASE_URL_ENV || DEFAULT_DATABASE_URL_ENV,
    datasetLimit: Number(process.env.DOCUMENT_FINGERPRINT_INVENTORY_DATASET_LIMIT || DEFAULT_DATASET_LIMIT),
    outputDir: process.env.DOCUMENT_FINGERPRINT_INVENTORY_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    psqlBin: process.env.DOCUMENT_FINGERPRINT_INVENTORY_PSQL_BIN || 'psql',
    selfTest: false,
    pretty: false,
    jsonStdout: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--env-file') {
      args.envFile = requireValue(arg, next);
      index += 1;
    } else if (arg === '--database-url-env') {
      args.databaseUrlEnv = requireValue(arg, next);
      index += 1;
    } else if (arg === '--dataset-limit') {
      args.datasetLimit = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--psql-bin') {
      args.psqlBin = requireValue(arg, next);
      index += 1;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--json-stdout') {
      args.jsonStdout = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (!Number.isInteger(args.datasetLimit) || args.datasetLimit < 1 || args.datasetLimit > 100) {
    throw new Error('--dataset-limit must be an integer from 1 to 100');
  }
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(args.databaseUrlEnv)) {
    throw new Error('--database-url-env must be an environment variable name');
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
  node scripts/smoke/document-fingerprint-inventory.mjs \\
    --env-file /etc/aiv3/aiv3.env \\
    --dataset-limit 20

This read-only smoke queries aggregate document fingerprint and dedup state
only. It does not read document text, titles, object locators, hash values, or
source rows. It never deletes files or modifies database rows.

Use --self-test for deterministic offline validation without psql or DataMax.
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
  const timestamp = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 17);
  const monotonic = process.hrtime.bigint().toString(36).slice(-6);
  return `${timestamp}-${process.pid.toString(36)}${monotonic}`;
}

function inventorySql(datasetLimit) {
  return `
with active_documents as (
  select
    tenant_id,
    dataset_id,
    id,
    nullif(content_sha256, '') as content_sha256,
    content_size_bytes,
    canonical_document_id,
    coalesce(nullif(dedup_state, ''), 'unknown') as dedup_state,
    nullif(object_key, '') as object_locator,
    coalesce(nullif(content_type, ''), 'unknown') as content_type
  from documents
  where lifecycle <> 'deleted'
),
dedup_state_counts as (
  select coalesce(jsonb_object_agg(dedup_state, count order by dedup_state), '{}'::jsonb) as value
  from (
    select dedup_state, count(*)::bigint as count
    from active_documents
    group by dedup_state
  ) grouped
),
content_type_counts as (
  select coalesce(jsonb_agg(jsonb_build_object('content_type', content_type, 'count', count) order by count desc, content_type), '[]'::jsonb) as value
  from (
    select content_type, count(*)::bigint as count
    from active_documents
    group by content_type
    order by count(*) desc, content_type
    limit 20
  ) grouped
),
content_hash_groups as (
  select tenant_id, content_sha256, count(*)::bigint as document_count
  from active_documents
  where content_sha256 is not null
  group by tenant_id, content_sha256
),
duplicate_summary as (
  select
    count(*) filter (where document_count > 1)::bigint as repeated_hash_group_count,
    coalesce(sum(document_count - 1) filter (where document_count > 1), 0)::bigint as duplicate_document_candidate_count,
    count(*)::bigint as distinct_hash_group_count
  from content_hash_groups
),
fingerprint_table_summary as (
  select
    count(*)::bigint as row_count,
    count(distinct canonical_document_id)::bigint as canonical_document_count
  from document_content_fingerprints
),
object_locator_classification as (
  select jsonb_build_object(
    'empty_locator_count', count(*) filter (where object_locator is null),
    'remote_locator_count', count(*) filter (
      where lower(object_locator) like 'http://%'
         or lower(object_locator) like 'https://%'
         or lower(object_locator) like 's3://%'
         or lower(object_locator) like 'cos://%'
         or lower(object_locator) like 'oss://%'
         or lower(object_locator) like 'gs://%'
    ),
    'local_locator_candidate_count', count(*) filter (
      where object_locator is not null
        and not (
          lower(object_locator) like 'http://%'
          or lower(object_locator) like 'https://%'
          or lower(object_locator) like 's3://%'
          or lower(object_locator) like 'cos://%'
          or lower(object_locator) like 'oss://%'
          or lower(object_locator) like 'gs://%'
        )
    ),
    'filesystem_checked', false
  ) as value
  from active_documents
),
dataset_rows as (
  select
    dataset_id,
    count(*)::bigint as document_count,
    count(*) filter (where object_locator is not null)::bigint as locator_present_count,
    count(*) filter (where content_sha256 is not null)::bigint as fingerprinted_document_count,
    count(*) filter (where dedup_state = 'canonical')::bigint as canonical_state_count,
    count(*) filter (where dedup_state = 'duplicate')::bigint as duplicate_state_count,
    count(*) filter (where dedup_state = 'unknown')::bigint as unknown_state_count,
    count(distinct content_type)::bigint as content_type_count,
    count(distinct content_sha256) filter (where content_sha256 is not null)::bigint as distinct_hash_count
  from active_documents
  where dataset_id is not null
  group by dataset_id
),
dataset_limited as (
  select *
  from dataset_rows
  order by document_count desc, dataset_id
  limit ${datasetLimit}
)
select jsonb_build_object(
  'schema', 'datamax.document_fingerprint_inventory.v1',
  'generated_at', now(),
  'redaction', jsonb_build_object(
    'hash_values_included', false,
    'document_titles_included', false,
    'object_locators_included', false,
    'raw_document_text_included', false,
    'credential_values_included', false
  ),
  'overall', jsonb_build_object(
    'document_count', (select count(*) from active_documents),
    'dataset_count', (select count(distinct dataset_id) from active_documents where dataset_id is not null),
    'locator_present_count', (select count(*) from active_documents where object_locator is not null),
    'fingerprinted_document_count', (select count(*) from active_documents where content_sha256 is not null),
    'canonical_document_reference_count', (select count(*) from active_documents where canonical_document_id is not null)
  ),
  'dedup_state_counts', (select value from dedup_state_counts),
  'content_type_counts_top', (select value from content_type_counts),
  'content_hash_groups', (select row_to_json(duplicate_summary)::jsonb from duplicate_summary),
  'fingerprint_table', (select row_to_json(fingerprint_table_summary)::jsonb from fingerprint_table_summary),
  'object_locator_classification', (select value from object_locator_classification),
  'dataset_limit', ${datasetLimit},
  'datasets', coalesce((
    select jsonb_agg(
      jsonb_build_object(
        'dataset_id', dataset_id,
        'document_count', document_count,
        'locator_present_count', locator_present_count,
        'fingerprinted_document_count', fingerprinted_document_count,
        'canonical_state_count', canonical_state_count,
        'duplicate_state_count', duplicate_state_count,
        'unknown_state_count', unknown_state_count,
        'content_type_count', content_type_count,
        'distinct_hash_count', distinct_hash_count
      )
      order by document_count desc, dataset_id
    )
    from dataset_limited
  ), '[]'::jsonb)
)::text as report;
`;
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

function sanitizeText(value) {
  return String(value || '')
    .replace(/postgres(?:ql)?:\/\/[^\s"']+/gi, 'postgres://[REDACTED]')
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/gi, 'Bearer [REDACTED]')
    .replace(/sk-[A-Za-z0-9_-]{8,}/g, 'sk-[REDACTED]');
}

function parsePsqlJson(stdout) {
  const candidates = String(stdout || '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.startsWith('{') && line.endsWith('}'));
  if (!candidates.length) {
    throw new Error('psql stdout did not contain a JSON object');
  }
  return JSON.parse(candidates[candidates.length - 1]);
}

function validateReport(report) {
  assert.equal(report.schema, 'datamax.document_fingerprint_inventory.v1');
  assert.equal(report.redaction.hash_values_included, false);
  assert.equal(report.redaction.document_titles_included, false);
  assert.equal(report.redaction.object_locators_included, false);
  assert.equal(report.redaction.raw_document_text_included, false);
  assert.equal(report.redaction.credential_values_included, false);
  assert.equal(report.object_locator_classification.filesystem_checked, false);
  assert.ok(Number(report.overall.document_count) >= 0);
  assert.ok(Array.isArray(report.datasets));

  const serialized = JSON.stringify(report);
  assert.equal(/[a-f0-9]{64}/i.test(serialized), false, 'hash values must not be included');
  assert.equal(/Bearer\s+[A-Za-z0-9]/i.test(serialized), false, 'bearer values must not be included');
  assert.equal(/postgres(?:ql)?:\/\//i.test(serialized), false, 'database URLs must not be included');
  assert.equal(/https?:\/\//i.test(serialized), false, 'raw URLs must not be included');
  return true;
}

function fixtureReport() {
  return {
    schema: 'datamax.document_fingerprint_inventory.v1',
    generated_at: '2026-06-12T00:00:00Z',
    redaction: {
      hash_values_included: false,
      document_titles_included: false,
      object_locators_included: false,
      raw_document_text_included: false,
      credential_values_included: false,
    },
    overall: {
      document_count: 8,
      dataset_count: 2,
      locator_present_count: 7,
      fingerprinted_document_count: 5,
      canonical_document_reference_count: 4,
    },
    dedup_state_counts: {
      canonical: 3,
      duplicate: 2,
      unknown: 3,
    },
    content_type_counts_top: [
      { content_type: 'application/pdf', count: 4 },
      { content_type: 'text/markdown', count: 4 },
    ],
    content_hash_groups: {
      repeated_hash_group_count: 2,
      duplicate_document_candidate_count: 3,
      distinct_hash_group_count: 4,
    },
    fingerprint_table: {
      row_count: 4,
      canonical_document_count: 3,
    },
    object_locator_classification: {
      empty_locator_count: 1,
      remote_locator_count: 2,
      local_locator_candidate_count: 5,
      filesystem_checked: false,
    },
    dataset_limit: 20,
    datasets: [
      {
        dataset_id: '00000000-0000-0000-0000-000000000001',
        document_count: 5,
        locator_present_count: 5,
        fingerprinted_document_count: 4,
        canonical_state_count: 2,
        duplicate_state_count: 1,
        unknown_state_count: 2,
        content_type_count: 2,
        distinct_hash_count: 3,
      },
    ],
  };
}

async function writeReport(outputDir, runId, report, pretty) {
  await mkdir(outputDir, { recursive: true });
  const reportPath = join(outputDir, `${runId}.json`);
  await writeFile(reportPath, JSON.stringify(report, null, pretty ? 2 : 0), 'utf8');
  return reportPath;
}

async function runSelfTest(args) {
  const report = fixtureReport();
  validateReport(report);
  const runId = `${makeRunId()}-self-test`;
  const reportPath = await writeReport(args.outputDir, runId, {
    selfTest: true,
    ok: true,
    report,
  }, args.pretty);
  return {
    runId,
    selfTest: true,
    ok: true,
    checks: {
      redactionContract: true,
      aggregateOnlyShape: true,
      noFilesystemCheck: true,
    },
    reportPath,
  };
}

async function runLive(args) {
  const envFromFile = await loadEnvFile(args.envFile);
  const databaseUrl = envFromFile[args.databaseUrlEnv] || process.env[args.databaseUrlEnv] || '';
  if (!databaseUrl) {
    throw new Error(`${args.databaseUrlEnv} is required via --env-file or environment`);
  }
  const runId = makeRunId();
  const outputDir = join(args.outputDir, runId);
  await mkdir(outputDir, { recursive: true });
  const result = await spawnCapture(args.psqlBin, [
    '-X',
    '-A',
    '-t',
    '-q',
    '-v',
    'ON_ERROR_STOP=1',
    databaseUrl,
    '-c',
    inventorySql(args.datasetLimit),
  ], {
    cwd: process.cwd(),
    env: { ...process.env, ...envFromFile },
  });
  const stdoutPath = join(outputDir, 'psql.stdout.txt');
  const stderrPath = join(outputDir, 'psql.stderr.txt');
  await writeFile(stdoutPath, sanitizeText(result.stdout), 'utf8');
  await writeFile(stderrPath, sanitizeText(result.stderr), 'utf8');
  if (result.exitCode !== 0) {
    throw new Error(`psql exited with ${result.exitCode}; stderr=${sanitizeText(result.stderr).slice(0, 240)}`);
  }
  const report = parsePsqlJson(result.stdout);
  validateReport(report);
  const receipt = {
    runId,
    ok: true,
    datasetLimit: args.datasetLimit,
    report,
    safety: {
      readOnlySelectOnly: true,
      filesystemChecked: false,
      deletesEnabled: false,
      writesEnabled: false,
      rawLocatorsIncluded: false,
      hashValuesIncluded: false,
    },
    stdoutPath,
    stderrPath,
    generatedAt: new Date().toISOString(),
  };
  const reportPath = join(outputDir, 'report.json');
  await writeFile(reportPath, JSON.stringify(receipt, null, args.pretty ? 2 : 0), 'utf8');
  return {
    runId,
    ok: true,
    reportPath,
    overall: report.overall,
    dedupStateCounts: report.dedup_state_counts,
    contentHashGroups: report.content_hash_groups,
    objectLocatorClassification: report.object_locator_classification,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = args.selfTest ? await runSelfTest(args) : await runLive(args);
  console.log(JSON.stringify(result, null, args.pretty || args.jsonStdout ? 2 : 0));
}

main().catch((error) => {
  console.error(error?.message || error);
  process.exit(1);
});
