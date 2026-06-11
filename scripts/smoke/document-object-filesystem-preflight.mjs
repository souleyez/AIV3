#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';

const DEFAULT_OUTPUT_DIR = 'target/document-object-filesystem-preflight-smoke';
const DEFAULT_DATABASE_URL_ENV = 'PLATFORM_DATABASE_URL';
const DEFAULT_PROBE_LIMIT = 200;

function parseArgs(argv) {
  const args = {
    envFile: process.env.DOCUMENT_OBJECT_FILESYSTEM_PREFLIGHT_ENV_FILE || '',
    databaseUrlEnv: process.env.DOCUMENT_OBJECT_FILESYSTEM_PREFLIGHT_DATABASE_URL_ENV || DEFAULT_DATABASE_URL_ENV,
    localObjectRootEnv: process.env.DOCUMENT_OBJECT_FILESYSTEM_PREFLIGHT_ROOT_ENV || 'PLATFORM_LOCAL_OBJECT_ROOT',
    probeLimit: Number(process.env.DOCUMENT_OBJECT_FILESYSTEM_PREFLIGHT_PROBE_LIMIT || DEFAULT_PROBE_LIMIT),
    outputDir: process.env.DOCUMENT_OBJECT_FILESYSTEM_PREFLIGHT_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    psqlBin: process.env.DOCUMENT_OBJECT_FILESYSTEM_PREFLIGHT_PSQL_BIN || 'psql',
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
    } else if (arg === '--local-object-root-env') {
      args.localObjectRootEnv = requireValue(arg, next);
      index += 1;
    } else if (arg === '--probe-limit') {
      args.probeLimit = Number(requireValue(arg, next));
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

  if (!Number.isInteger(args.probeLimit) || args.probeLimit < 1 || args.probeLimit > 1000) {
    throw new Error('--probe-limit must be an integer from 1 to 1000');
  }
  for (const [name, value] of [
    ['--database-url-env', args.databaseUrlEnv],
    ['--local-object-root-env', args.localObjectRootEnv],
  ]) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(value)) {
      throw new Error(`${name} must be an environment variable name`);
    }
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
  node scripts/smoke/document-object-filesystem-preflight.mjs \\
    --env-file /etc/aiv3/aiv3.env \\
    --probe-limit 200

This read-only smoke checks aggregate filesystem reachability for a bounded
sample of local object candidates that are missing content fingerprints. It
only calls stat(), never reads file content, never prints object paths, never
downloads remote objects, never deletes files, and never writes database rows.

Use --self-test for deterministic offline validation without psql or DataMax.
`);
}

async function loadEnvFile(filePath) {
  if (!filePath) {
    return {};
  }
  const text = await readFile(filePath, 'utf8');
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

function candidateSql(probeLimit) {
  return `
with active_documents as (
  select
    dataset_id,
    id,
    nullif(content_sha256, '') as content_sha256,
    nullif(object_key, '') as object_locator,
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
candidate_totals as (
  select
    count(*)::bigint as document_count,
    count(*) filter (where locator_kind = 'local_candidate' and content_sha256 is null)::bigint as local_missing_fingerprint_count,
    count(*) filter (where locator_kind = 'remote' and content_sha256 is null)::bigint as remote_missing_fingerprint_count,
    count(*) filter (where locator_kind = 'missing' and content_sha256 is null)::bigint as missing_locator_fingerprint_count
  from active_documents
),
probe_rows as (
  select object_locator
  from active_documents
  where locator_kind = 'local_candidate'
    and content_sha256 is null
  order by dataset_id nulls last, id
  limit ${probeLimit}
)
select jsonb_build_object(
  'schema', 'datamax.document_object_filesystem_preflight.input.v1',
  'totals', (select row_to_json(candidate_totals)::jsonb from candidate_totals),
  'probe_limit', ${probeLimit},
  'candidates', coalesce((
    select jsonb_agg(jsonb_build_object('object_locator', object_locator))
    from probe_rows
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

function looksRemote(value) {
  const lower = String(value || '').trim().toLowerCase();
  return ['http://', 'https://', 's3://', 'cos://', 'oss://', 'gs://'].some((prefix) =>
    lower.startsWith(prefix),
  );
}

function isInsideOrSame(parent, child) {
  const relative = path.relative(parent, child);
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function resolveCandidate(rawLocator, root) {
  const trimmed = String(rawLocator || '').trim();
  if (!trimmed) {
    return { status: 'missing_locator', resolution: 'none' };
  }
  if (looksRemote(trimmed)) {
    return { status: 'remote_locator_skipped', resolution: 'remote' };
  }
  const raw = trimmed.replace(/^file:\/\//i, '').trim();
  if (!raw) {
    return { status: 'missing_locator', resolution: 'none' };
  }

  const direct = path.resolve(raw);
  if (path.isAbsolute(raw)) {
    return { status: 'probe_ready', resolution: 'absolute_direct', path: direct };
  }

  if (!root) {
    return { status: 'local_root_unset', resolution: 'relative_without_root' };
  }

  const rootResolved = path.resolve(root);
  const rooted = path.resolve(rootResolved, raw);
  if (!isInsideOrSame(rootResolved, rooted)) {
    return { status: 'path_traversal_blocked', resolution: 'relative_outside_root' };
  }
  return { status: 'probe_ready', resolution: 'relative_under_root', path: rooted };
}

function increment(map, key, amount = 1) {
  map[key] = (map[key] || 0) + amount;
}

async function inspectCandidate(rawLocator, root, statFn = stat) {
  const resolved = resolveCandidate(rawLocator, root);
  if (resolved.status !== 'probe_ready') {
    return {
      status: resolved.status,
      resolution: resolved.resolution,
      sizeBytes: 0,
    };
  }

  try {
    const info = await statFn(resolved.path);
    if (!info.isFile()) {
      return { status: 'not_file', resolution: resolved.resolution, sizeBytes: 0 };
    }
    return { status: 'file_found', resolution: resolved.resolution, sizeBytes: Number(info.size || 0) };
  } catch (error) {
    if (error?.code === 'ENOENT' || error?.code === 'ENOTDIR') {
      return { status: 'file_missing', resolution: resolved.resolution, sizeBytes: 0 };
    }
    if (error?.code === 'EACCES' || error?.code === 'EPERM') {
      return { status: 'inaccessible', resolution: resolved.resolution, sizeBytes: 0 };
    }
    return { status: 'stat_error', resolution: resolved.resolution, sizeBytes: 0 };
  }
}

async function buildPreflightReport(input, options = {}) {
  const root = String(options.localObjectRoot || '').trim();
  const candidates = Array.isArray(input.candidates) ? input.candidates : [];
  const statusCounts = {};
  const resolutionCounts = {};
  let foundFileTotalBytes = 0;
  let foundFileMaxBytes = 0;

  for (const candidate of candidates) {
    const result = await inspectCandidate(candidate.object_locator, root, options.statFn || stat);
    increment(statusCounts, result.status);
    increment(resolutionCounts, result.resolution);
    if (result.status === 'file_found') {
      foundFileTotalBytes += result.sizeBytes;
      foundFileMaxBytes = Math.max(foundFileMaxBytes, result.sizeBytes);
    }
  }

  return {
    schema: 'datamax.document_object_filesystem_preflight.v1',
    generated_at: new Date().toISOString(),
    mode: 'read_only_stat_preflight',
    redaction: {
      object_locators_included: false,
      filesystem_paths_included: false,
      hash_values_included: false,
      document_titles_included: false,
      raw_document_text_included: false,
      credential_values_included: false,
    },
    execution_policy: {
      stat_only: true,
      file_content_read: false,
      filesystem_mutation_enabled: false,
      remote_fetch_enabled: false,
      database_writes_enabled: false,
      object_deletes_enabled: false,
      path_values_included: false,
      impact_is_aggregate_only: true,
    },
    root_summary: {
      root_configured: Boolean(root),
      root_value_included: false,
    },
    input_summary: {
      document_count: Number(input.totals?.document_count || 0),
      local_missing_fingerprint_count: Number(input.totals?.local_missing_fingerprint_count || 0),
      remote_missing_fingerprint_count: Number(input.totals?.remote_missing_fingerprint_count || 0),
      missing_locator_fingerprint_count: Number(input.totals?.missing_locator_fingerprint_count || 0),
      probe_limit: Number(input.probe_limit || candidates.length),
      probed_count: candidates.length,
    },
    status_counts: statusCounts,
    resolution_counts: resolutionCounts,
    found_file_size_summary: {
      file_found_count: Number(statusCounts.file_found || 0),
      total_bytes: foundFileTotalBytes,
      max_bytes: foundFileMaxBytes,
    },
    cleanup_policy: {
      real_cleanup_allowed_by_this_report: false,
      next_required_step: 'operator_reviewed_manifest_before_any_cleanup',
    },
  };
}

function validateReport(report) {
  assert.equal(report.schema, 'datamax.document_object_filesystem_preflight.v1');
  assert.equal(report.mode, 'read_only_stat_preflight');
  assert.equal(report.redaction.object_locators_included, false);
  assert.equal(report.redaction.filesystem_paths_included, false);
  assert.equal(report.redaction.hash_values_included, false);
  assert.equal(report.redaction.document_titles_included, false);
  assert.equal(report.execution_policy.stat_only, true);
  assert.equal(report.execution_policy.file_content_read, false);
  assert.equal(report.execution_policy.filesystem_mutation_enabled, false);
  assert.equal(report.execution_policy.remote_fetch_enabled, false);
  assert.equal(report.execution_policy.database_writes_enabled, false);
  assert.equal(report.execution_policy.object_deletes_enabled, false);
  assert.equal(report.execution_policy.path_values_included, false);
  assert.equal(report.cleanup_policy.real_cleanup_allowed_by_this_report, false);
  assert.ok(Number(report.input_summary.probed_count) >= 0);

  const serialized = JSON.stringify(report);
  assert.equal(/[a-f0-9]{64}/i.test(serialized), false, 'hash values must not be included');
  assert.equal(/Bearer\s+[A-Za-z0-9]/i.test(serialized), false, 'bearer values must not be included');
  assert.equal(/postgres(?:ql)?:\/\//i.test(serialized), false, 'database URLs must not be included');
  assert.equal(/https?:\/\//i.test(serialized), false, 'raw URLs must not be included');
  return true;
}

async function writeReport(outputDir, runId, report, pretty) {
  await mkdir(outputDir, { recursive: true });
  const reportPath = path.join(outputDir, `${runId}.json`);
  await writeFile(reportPath, JSON.stringify(report, null, pretty ? 2 : 0), 'utf8');
  return reportPath;
}

async function runSelfTest(args) {
  const fakeRoot = path.resolve('/datamax-object-root');
  const foundPath = path.resolve(fakeRoot, 'documents/found.pdf');
  const input = {
    schema: 'datamax.document_object_filesystem_preflight.input.v1',
    totals: {
      document_count: 6,
      local_missing_fingerprint_count: 5,
      remote_missing_fingerprint_count: 1,
      missing_locator_fingerprint_count: 0,
    },
    probe_limit: 5,
    candidates: [
      { object_locator: 'documents/found.pdf' },
      { object_locator: 'documents/missing.pdf' },
      { object_locator: '../escape.pdf' },
      { object_locator: 'file://documents/missing-file-uri.pdf' },
      { object_locator: '' },
    ],
  };
  const statFn = async (candidatePath) => {
    if (path.resolve(candidatePath) === foundPath) {
      return { isFile: () => true, size: 1234 };
    }
    const error = new Error('not found');
    error.code = 'ENOENT';
    throw error;
  };
  const report = await buildPreflightReport(input, {
    localObjectRoot: fakeRoot,
    statFn,
  });
  validateReport(report);
  assert.equal(report.status_counts.file_found, 1);
  assert.equal(report.status_counts.file_missing, 2);
  assert.equal(report.status_counts.path_traversal_blocked, 1);
  assert.equal(report.status_counts.missing_locator, 1);
  assert.equal(report.found_file_size_summary.total_bytes, 1234);

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
      statOnly: true,
      noPathValues: true,
      traversalBlocked: true,
      cleanupDisabled: true,
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
  const localObjectRoot = envFromFile[args.localObjectRootEnv] || process.env[args.localObjectRootEnv] || '';
  const runId = makeRunId();
  const outputDir = path.join(args.outputDir, runId);
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
    candidateSql(args.probeLimit),
  ], {
    cwd: process.cwd(),
    env: { ...process.env, ...envFromFile },
  });
  const stdoutPath = path.join(outputDir, 'psql.stdout.redacted.txt');
  const stderrPath = path.join(outputDir, 'psql.stderr.txt');
  await writeFile(
    stdoutPath,
    '[REDACTED: candidate object locator rowset intentionally not persisted]\n',
    'utf8',
  );
  await writeFile(stderrPath, sanitizeText(result.stderr), 'utf8');
  if (result.exitCode !== 0) {
    throw new Error(`psql exited with ${result.exitCode}; stderr=${sanitizeText(result.stderr).slice(0, 240)}`);
  }
  const input = parsePsqlJson(result.stdout);
  const report = await buildPreflightReport(input, { localObjectRoot });
  validateReport(report);
  const receipt = {
    runId,
    ok: true,
    probeLimit: args.probeLimit,
    report,
    safety: {
      readOnlySelectOnly: true,
      statOnly: true,
      fileContentRead: false,
      filesystemMutationEnabled: false,
      remoteFetchEnabled: false,
      databaseWritesEnabled: false,
      objectDeletesEnabled: false,
      rawLocatorsIncluded: false,
      pathValuesIncluded: false,
    },
    stdoutPath,
    stderrPath,
    generatedAt: new Date().toISOString(),
  };
  const reportPath = path.join(outputDir, 'report.json');
  await writeFile(reportPath, JSON.stringify(receipt, null, args.pretty ? 2 : 0), 'utf8');
  return {
    runId,
    ok: true,
    reportPath,
    inputSummary: report.input_summary,
    rootSummary: report.root_summary,
    statusCounts: report.status_counts,
    resolutionCounts: report.resolution_counts,
    foundFileSizeSummary: report.found_file_size_summary,
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
