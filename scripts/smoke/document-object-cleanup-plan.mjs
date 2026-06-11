#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_OUTPUT_DIR = 'target/document-object-cleanup-plan-smoke';
const DEFAULT_DATABASE_URL_ENV = 'PLATFORM_DATABASE_URL';
const DEFAULT_DATASET_LIMIT = 20;

function parseArgs(argv) {
  const args = {
    envFile: process.env.DOCUMENT_OBJECT_CLEANUP_PLAN_ENV_FILE || '',
    databaseUrlEnv: process.env.DOCUMENT_OBJECT_CLEANUP_PLAN_DATABASE_URL_ENV || DEFAULT_DATABASE_URL_ENV,
    datasetLimit: Number(process.env.DOCUMENT_OBJECT_CLEANUP_PLAN_DATASET_LIMIT || DEFAULT_DATASET_LIMIT),
    outputDir: process.env.DOCUMENT_OBJECT_CLEANUP_PLAN_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    psqlBin: process.env.DOCUMENT_OBJECT_CLEANUP_PLAN_PSQL_BIN || 'psql',
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
  node scripts/smoke/document-object-cleanup-plan.mjs \\
    --env-file /etc/aiv3/aiv3.env \\
    --dataset-limit 20

This read-only smoke builds an aggregate dry-run object cleanup plan. It does
not read object paths, document text, titles, hash values, or source rows. It
does not probe the filesystem, download remote objects, delete files, or modify
database rows.

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

function cleanupPlanSql(datasetLimit) {
  return `
with active_documents as (
  select
    tenant_id,
    dataset_id,
    id,
    nullif(content_sha256, '') as content_sha256,
    canonical_document_id,
    coalesce(nullif(dedup_state, ''), 'unknown') as dedup_state,
    nullif(object_key, '') as object_locator,
    coalesce(nullif(content_type, ''), 'unknown') as content_type,
    case
      when nullif(object_key, '') is null then 'missing'
      when lower(object_key) like 'http://%'
        or lower(object_key) like 'https://%'
        or lower(object_key) like 's3://%'
        or lower(object_key) like 'cos://%'
        or lower(object_key) like 'oss://%'
        or lower(object_key) like 'gs://%'
        then 'remote'
      else 'local_candidate'
    end as locator_kind
  from documents
  where lifecycle <> 'deleted'
),
hash_grouped as (
  select
    active_documents.*,
    count(*) filter (where content_sha256 is not null) over (partition by tenant_id, content_sha256) as hash_group_size
  from active_documents
),
classified as (
  select
    *,
    case
      when locator_kind = 'missing' then 'blocked_missing_locator'
      when locator_kind = 'remote' then 'blocked_remote_locator'
      when content_sha256 is null then 'blocked_missing_fingerprint'
      when dedup_state = 'canonical' then 'blocked_canonical_document'
      when dedup_state = 'duplicate' and canonical_document_id is not null then 'review_object_cleanup_candidate'
      when hash_group_size > 1 then 'review_index_mapping_candidate'
      when dedup_state = 'unknown' then 'no_cleanup_unique_or_unlinked'
      else 'blocked_unclassified'
    end as cleanup_class
  from hash_grouped
),
cleanup_class_counts as (
  select coalesce(jsonb_object_agg(cleanup_class, count order by cleanup_class), '{}'::jsonb) as value
  from (
    select cleanup_class, count(*)::bigint as count
    from classified
    group by cleanup_class
  ) grouped
),
locator_kind_counts as (
  select coalesce(jsonb_object_agg(locator_kind, count order by locator_kind), '{}'::jsonb) as value
  from (
    select locator_kind, count(*)::bigint as count
    from classified
    group by locator_kind
  ) grouped
),
dataset_rows as (
  select
    dataset_id,
    count(*)::bigint as document_count,
    count(*) filter (where cleanup_class = 'review_object_cleanup_candidate')::bigint as review_object_cleanup_candidate_count,
    count(*) filter (where cleanup_class = 'review_index_mapping_candidate')::bigint as review_index_mapping_candidate_count,
    count(*) filter (where cleanup_class like 'blocked_%')::bigint as blocked_count,
    count(*) filter (where cleanup_class = 'blocked_missing_fingerprint')::bigint as blocked_missing_fingerprint_count,
    count(*) filter (where cleanup_class = 'blocked_remote_locator')::bigint as blocked_remote_locator_count,
    count(*) filter (where cleanup_class = 'blocked_missing_locator')::bigint as blocked_missing_locator_count,
    count(*) filter (where cleanup_class = 'no_cleanup_unique_or_unlinked')::bigint as no_cleanup_unique_or_unlinked_count,
    count(distinct content_type)::bigint as content_type_count
  from classified
  where dataset_id is not null
  group by dataset_id
),
dataset_limited as (
  select *
  from dataset_rows
  order by
    review_object_cleanup_candidate_count desc,
    review_index_mapping_candidate_count desc,
    blocked_count desc,
    document_count desc,
    dataset_id
  limit ${datasetLimit}
)
select jsonb_build_object(
  'schema', 'datamax.document_object_cleanup_plan.v1',
  'generated_at', now(),
  'mode', 'dry_run_read_only',
  'redaction', jsonb_build_object(
    'hash_values_included', false,
    'document_titles_included', false,
    'object_locators_included', false,
    'raw_document_text_included', false,
    'credential_values_included', false
  ),
  'execution_policy', jsonb_build_object(
    'manual_approval_required', true,
    'filesystem_checked', false,
    'remote_fetch_enabled', false,
    'database_writes_enabled', false,
    'object_deletes_enabled', false,
    'object_paths_included', false,
    'impact_is_aggregate_only', true
  ),
  'overall', jsonb_build_object(
    'document_count', (select count(*) from classified),
    'dataset_count', (select count(distinct dataset_id) from classified where dataset_id is not null),
    'review_object_cleanup_candidate_count', (select count(*) from classified where cleanup_class = 'review_object_cleanup_candidate'),
    'review_index_mapping_candidate_count', (select count(*) from classified where cleanup_class = 'review_index_mapping_candidate'),
    'blocked_count', (select count(*) from classified where cleanup_class like 'blocked_%'),
    'no_cleanup_count', (select count(*) from classified where cleanup_class = 'no_cleanup_unique_or_unlinked')
  ),
  'cleanup_class_counts', (select value from cleanup_class_counts),
  'locator_kind_counts', (select value from locator_kind_counts),
  'dataset_limit', ${datasetLimit},
  'impact_by_dataset', coalesce((
    select jsonb_agg(
      jsonb_build_object(
        'dataset_id', dataset_id,
        'document_count', document_count,
        'review_object_cleanup_candidate_count', review_object_cleanup_candidate_count,
        'review_index_mapping_candidate_count', review_index_mapping_candidate_count,
        'blocked_count', blocked_count,
        'blocked_missing_fingerprint_count', blocked_missing_fingerprint_count,
        'blocked_remote_locator_count', blocked_remote_locator_count,
        'blocked_missing_locator_count', blocked_missing_locator_count,
        'no_cleanup_unique_or_unlinked_count', no_cleanup_unique_or_unlinked_count,
        'content_type_count', content_type_count
      )
      order by
        review_object_cleanup_candidate_count desc,
        review_index_mapping_candidate_count desc,
        blocked_count desc,
        document_count desc,
        dataset_id
    )
    from dataset_limited
  ), '[]'::jsonb),
  'rollback_plan', jsonb_build_object(
    'required_before_real_cleanup', jsonb_build_array(
      'operator_approval_id',
      'full_redacted_manifest_with_document_ids_and_object_locator_hashes',
      'object_backup_or_retention_window',
      'database_snapshot_or_reversible_mapping_plan',
      'post_cleanup_verification_commands'
    ),
    'restore_strategy', jsonb_build_array(
      'disable_cleanup_job',
      'restore_objects_from_backup_or_retention_window',
      'restore_document_object_mappings_from_manifest',
      'rerun_fingerprint_inventory_and_retrieval_smokes'
    ),
    'real_cleanup_allowed_by_this_report', false
  ),
  'recommended_next_actions', jsonb_build_array(
    'review aggregate candidates only; do not delete from this report',
    'add filesystem preflight if local object reachability must be proven',
    'produce operator-reviewed manifest before any real cleanup'
  )
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
  assert.equal(report.schema, 'datamax.document_object_cleanup_plan.v1');
  assert.equal(report.mode, 'dry_run_read_only');
  assert.equal(report.redaction.hash_values_included, false);
  assert.equal(report.redaction.document_titles_included, false);
  assert.equal(report.redaction.object_locators_included, false);
  assert.equal(report.redaction.raw_document_text_included, false);
  assert.equal(report.redaction.credential_values_included, false);
  assert.equal(report.execution_policy.manual_approval_required, true);
  assert.equal(report.execution_policy.filesystem_checked, false);
  assert.equal(report.execution_policy.remote_fetch_enabled, false);
  assert.equal(report.execution_policy.database_writes_enabled, false);
  assert.equal(report.execution_policy.object_deletes_enabled, false);
  assert.equal(report.execution_policy.impact_is_aggregate_only, true);
  assert.equal(report.rollback_plan.real_cleanup_allowed_by_this_report, false);
  assert.ok(Array.isArray(report.impact_by_dataset));
  assert.ok(Number(report.overall.document_count) >= 0);

  const serialized = JSON.stringify(report);
  assert.equal(/[a-f0-9]{64}/i.test(serialized), false, 'hash values must not be included');
  assert.equal(/Bearer\s+[A-Za-z0-9]/i.test(serialized), false, 'bearer values must not be included');
  assert.equal(/postgres(?:ql)?:\/\//i.test(serialized), false, 'database URLs must not be included');
  assert.equal(/https?:\/\//i.test(serialized), false, 'raw URLs must not be included');
  return true;
}

function fixtureReport() {
  return {
    schema: 'datamax.document_object_cleanup_plan.v1',
    generated_at: '2026-06-12T00:00:00Z',
    mode: 'dry_run_read_only',
    redaction: {
      hash_values_included: false,
      document_titles_included: false,
      object_locators_included: false,
      raw_document_text_included: false,
      credential_values_included: false,
    },
    execution_policy: {
      manual_approval_required: true,
      filesystem_checked: false,
      remote_fetch_enabled: false,
      database_writes_enabled: false,
      object_deletes_enabled: false,
      object_paths_included: false,
      impact_is_aggregate_only: true,
    },
    overall: {
      document_count: 9,
      dataset_count: 2,
      review_object_cleanup_candidate_count: 2,
      review_index_mapping_candidate_count: 1,
      blocked_count: 4,
      no_cleanup_count: 2,
    },
    cleanup_class_counts: {
      blocked_missing_fingerprint: 3,
      blocked_remote_locator: 1,
      no_cleanup_unique_or_unlinked: 2,
      review_index_mapping_candidate: 1,
      review_object_cleanup_candidate: 2,
    },
    locator_kind_counts: {
      local_candidate: 8,
      remote: 1,
    },
    dataset_limit: 20,
    impact_by_dataset: [
      {
        dataset_id: '00000000-0000-0000-0000-000000000001',
        document_count: 6,
        review_object_cleanup_candidate_count: 2,
        review_index_mapping_candidate_count: 1,
        blocked_count: 2,
        blocked_missing_fingerprint_count: 2,
        blocked_remote_locator_count: 0,
        blocked_missing_locator_count: 0,
        no_cleanup_unique_or_unlinked_count: 1,
        content_type_count: 2,
      },
    ],
    rollback_plan: {
      required_before_real_cleanup: [
        'operator_approval_id',
        'full_redacted_manifest_with_document_ids_and_object_locator_hashes',
        'object_backup_or_retention_window',
        'database_snapshot_or_reversible_mapping_plan',
        'post_cleanup_verification_commands',
      ],
      restore_strategy: [
        'disable_cleanup_job',
        'restore_objects_from_backup_or_retention_window',
        'restore_document_object_mappings_from_manifest',
        'rerun_fingerprint_inventory_and_retrieval_smokes',
      ],
      real_cleanup_allowed_by_this_report: false,
    },
    recommended_next_actions: [
      'review aggregate candidates only; do not delete from this report',
      'add filesystem preflight if local object reachability must be proven',
      'produce operator-reviewed manifest before any real cleanup',
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
      aggregateOnlyImpact: true,
      cleanupDisabled: true,
      rollbackPlanPresent: true,
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
    cleanupPlanSql(args.datasetLimit),
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
      remoteFetchEnabled: false,
      deletesEnabled: false,
      writesEnabled: false,
      rawLocatorsIncluded: false,
      hashValuesIncluded: false,
      manualApprovalRequired: true,
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
    cleanupClassCounts: report.cleanup_class_counts,
    locatorKindCounts: report.locator_kind_counts,
    rollbackRequired: report.rollback_plan.required_before_real_cleanup,
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
