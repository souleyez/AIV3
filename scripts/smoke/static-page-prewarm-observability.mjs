#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import assert from 'node:assert/strict';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_QUEUE_FILTER = 'static_page_image_preview,static_page_publish,codex';
const DEFAULT_REQUIRED_STATUSES = [
  'queued',
  'running',
  'published',
  'failed',
  'skipped_existing_template',
  'waiting_for_low_load',
];

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.STATIC_PAGE_PREWARM_OBSERVABILITY_BASE_URL || DEFAULT_BASE_URL,
    cookie: process.env.STATIC_PAGE_PREWARM_OBSERVABILITY_COOKIE || '',
    bearer: process.env.STATIC_PAGE_PREWARM_OBSERVABILITY_BEARER || '',
    timeoutMs: Number(process.env.STATIC_PAGE_PREWARM_OBSERVABILITY_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    queueFilter: process.env.STATIC_PAGE_PREWARM_OBSERVABILITY_QUEUE_FILTER || DEFAULT_QUEUE_FILTER,
    requiredStatuses: parseList(
      process.env.STATIC_PAGE_PREWARM_OBSERVABILITY_REQUIRED_STATUSES,
      DEFAULT_REQUIRED_STATUSES,
    ),
    allowMissingCredentials:
      process.env.STATIC_PAGE_PREWARM_OBSERVABILITY_ALLOW_MISSING_CREDENTIALS === 'true',
    outputDir: process.env.STATIC_PAGE_PREWARM_OBSERVABILITY_OUTPUT_DIR
      || 'target/static-page-prewarm-observability-smoke',
    selfTest: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--base-url') {
      args.baseUrl = requireValue(arg, next);
      index += 1;
    } else if (arg === '--cookie') {
      args.cookie = requireValue(arg, next);
      index += 1;
    } else if (arg === '--bearer') {
      args.bearer = requireValue(arg, next);
      index += 1;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--queue-filter') {
      args.queueFilter = requireValue(arg, next);
      index += 1;
    } else if (arg === '--required-statuses') {
      args.requiredStatuses = parseList(requireValue(arg, next), []);
      index += 1;
    } else if (arg === '--allow-missing-credentials') {
      args.allowMissingCredentials = true;
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

  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) {
    throw new Error('--timeout-ms must be at least 1000');
  }
  return args;
}

function parseList(value, fallback) {
  if (value == null || String(value).trim() === '') {
    return [...fallback];
  }
  return String(value)
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`Usage:
  node scripts/smoke/static-page-prewarm-observability.mjs --self-test

  node scripts/smoke/static-page-prewarm-observability.mjs \\
    --base-url https://v3.elepcloud.com \\
    --cookie "session=..." \\
    --queue-filter static_page_image_preview,static_page_publish,codex

This read-only smoke fixes the static-page template prewarm/reuse observability
contract. Self-test uses deterministic fixtures and does not call DataMax.
Live mode only reads workflow queue stats and writes a redacted receipt.
Without credentials, pass --allow-missing-credentials to record the HTTP 401
operator guard as pending instead of failing the smoke.
`);
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function requestHeaders(args) {
  const headers = { accept: 'application/json' };
  if (args.cookie) {
    headers.cookie = args.cookie;
  }
  if (args.bearer) {
    headers.authorization = args.bearer.toLowerCase().startsWith('bearer ')
      ? args.bearer
      : `Bearer ${args.bearer}`;
  }
  return headers;
}

async function requestJson(url, headers, timeoutMs) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { headers, signal: controller.signal });
    const text = await response.text();
    let data = null;
    try {
      data = text ? JSON.parse(text) : null;
    } catch {
      data = null;
    }
    return { response, text, data };
  } finally {
    clearTimeout(timeout);
  }
}

function rawStatusFromEvent(event) {
  const payload = event?.payload || event?.data || event || {};
  const card = payload.reply?.card || payload.card || {};
  return [
    payload.status,
    payload.task_status,
    payload.reply?.task_status,
    card.status,
    card.codex_final_status,
    card.render_output_status,
    card.image_job_status,
  ]
    .filter((value) => value != null)
    .map((value) => String(value).trim())
    .find(Boolean) || '';
}

function normalizeStaticPagePrewarmStatus(event) {
  const eventName = String(event?.event_name || event?.eventName || event?.name || '').trim();
  const rawStatus = rawStatusFromEvent(event);
  const combined = `${eventName} ${rawStatus}`.toLowerCase();

  if (combined.includes('prewarm_skipped') && combined.includes('waiting_for_low_load')) {
    return 'waiting_for_low_load';
  }
  if (
    combined.includes('stable_artifact_reused')
    || combined.includes('static_page_stable_artifact_reused')
    || combined.includes('existing_template')
  ) {
    return 'skipped_existing_template';
  }
  if (combined.includes('failed') || combined.includes('cancelled') || combined.includes('needs_human')) {
    return 'failed';
  }
  if (combined.includes('published') || combined.includes('publish_completed')) {
    return 'published';
  }
  if (combined.includes('running') || combined.includes('rendering') || combined.includes('retrying')) {
    return 'running';
  }
  if (
    combined.includes('queued')
    || combined.includes('planning')
    || combined.includes('accepted')
    || combined.includes('candidate')
  ) {
    return 'queued';
  }
  return 'unknown';
}

function summarizeEvents(events, requiredStatuses) {
  const normalized = events.map((event) => ({
    eventName: event.event_name || event.eventName || event.name || null,
    rawStatus: rawStatusFromEvent(event) || null,
    normalizedStatus: normalizeStaticPagePrewarmStatus(event),
    customerVisible: event.payload?.customer_visible ?? event.payload?.customerVisible ?? null,
  }));
  const counts = {};
  for (const item of normalized) {
    counts[item.normalizedStatus] = (counts[item.normalizedStatus] || 0) + 1;
  }
  const missingStatuses = requiredStatuses.filter((status) => !counts[status]);
  const waitingForLowLoadCount = normalized
    .filter((item) => item.normalizedStatus === 'waiting_for_low_load')
    .length;
  const customerVisiblePrewarmLeakCount = normalized
    .filter((item) => item.normalizedStatus === 'waiting_for_low_load' && item.customerVisible === true)
    .length;
  return {
    normalized,
    counts,
    requiredStatuses,
    missingStatuses,
    waitingForLowLoadCount,
    customerVisiblePrewarmLeakCount,
    prewarmCustomerVisibilityOk: customerVisiblePrewarmLeakCount === 0,
    ok: missingStatuses.length === 0,
  };
}

function queueMatches(queue, filters) {
  const logicalQueue = String(queue.logical_queue || queue.logicalQueue || '').toLowerCase();
  return filters.some((filter) => logicalQueue.includes(filter));
}

function summarizeQueues(queueStats, queueFilter) {
  const filters = queueFilter
    .split(',')
    .map((item) => item.trim().toLowerCase())
    .filter(Boolean);
  const queues = (queueStats?.queues || [])
    .filter((queue) => queueMatches(queue, filters))
    .map((queue) => ({
      logicalQueue: queue.logical_queue || queue.logicalQueue || '',
      queued: Number(queue.queued || 0),
      running: Number(queue.running || 0),
      retrying: Number(queue.retrying || 0),
      succeeded: Number(queue.succeeded || 0),
      failed: Number(queue.failed || 0),
      taskCount: Number(queue.task_count || queue.taskCount || 0),
      nextAvailableAt: queue.next_available_at || queue.nextAvailableAt || null,
    }));
  return {
    filters,
    queues,
    queueCount: queues.length,
    activeCount: queues.reduce((sum, queue) => sum + queue.queued + queue.running + queue.retrying, 0),
    failedCount: queues.reduce((sum, queue) => sum + queue.failed, 0),
  };
}

function buildReport(args, { events = [], queueStats = null, queueHttpStatus = null, queueBodyPrefix = '' }) {
  const eventSummary = summarizeEvents(events, args.requiredStatuses);
  const queueSummary = summarizeQueues(queueStats, args.queueFilter);
  const credentialsProvided = Boolean(args.cookie || args.bearer);
  const queueReadOk = queueHttpStatus >= 200 && queueHttpStatus < 300;
  const missingCredentialPending = Boolean(
    !args.selfTest &&
      args.allowMissingCredentials &&
      !credentialsProvided &&
      queueHttpStatus === 401
  );
  const summary = {
    ok: args.selfTest ? eventSummary.ok : queueReadOk,
    pending: missingCredentialPending,
    failed: args.selfTest ? !eventSummary.ok : !queueReadOk && !missingCredentialPending,
    selfTest: args.selfTest,
    baseUrl: args.baseUrl,
    credentialsProvided,
    requiredStatuses: args.requiredStatuses,
    observedStatuses: Object.keys(eventSummary.counts).sort(),
    missingStatuses: eventSummary.missingStatuses,
    waitingForLowLoadCount: eventSummary.waitingForLowLoadCount,
    customerVisiblePrewarmLeakCount: eventSummary.customerVisiblePrewarmLeakCount,
    prewarmCustomerVisibilityOk: eventSummary.prewarmCustomerVisibilityOk,
    queueCount: queueSummary.queueCount,
    activeQueueTaskCount: queueSummary.activeCount,
    failedQueueTaskCount: queueSummary.failedCount,
    queueHttpStatus,
    generatedAt: new Date().toISOString(),
  };
  return {
    schema: 'datamax.static_page_prewarm_observability.v1',
    summary,
    eventSummary,
    queueSummary,
    queueHttpStatus,
    queueBodyPrefix,
    auth: {
      credentialsProvided,
      cookiePrinted: false,
      bearerPrinted: false,
    },
    redaction: {
      cookiePrinted: false,
      bearerPrinted: false,
      rawEnvValuesPrinted: false,
    },
  };
}

async function writeReport(args, report) {
  const outputDir = join(process.cwd(), args.outputDir);
  await mkdir(outputDir, { recursive: true });
  const suffix = args.selfTest ? '-self-test' : '';
  const reportPath = join(
    outputDir,
    `${new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14)}${suffix}.json`,
  );
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  return reportPath;
}

async function runSelfTest(args) {
  const fixtureArgs = {
    ...args,
    selfTest: true,
    baseUrl: 'https://v3.elepcloud.com',
    requiredStatuses: DEFAULT_REQUIRED_STATUSES,
  };
  const fixtureEvents = [
    {
      event_name: 'assistant_run.external_channel_static_page_publish_queued',
      payload: { status: 'static_page_publish_queued' },
    },
    {
      event_name: 'assistant_run.external_channel_static_page_publish_progress',
      payload: { status: 'static_page_publish_running' },
    },
    {
      event_name: 'assistant_run.external_channel_static_page_publish_completed',
      payload: { status: 'static_page_published' },
    },
    {
      event_name: 'assistant_run.external_channel_static_page_publish_failed',
      payload: { status: 'static_page_publish_failed' },
    },
    {
      event_name: 'assistant_run.external_channel_static_page_stable_artifact_reused',
      payload: { status: 'static_page_stable_artifact_reused' },
    },
    {
      event_name: 'assistant_run.static_page_template_prewarm_skipped',
      payload: {
        status: 'waiting_for_low_load',
        customer_visible: false,
        active_heavy_task_count: 3,
        max_active_heavy_task_count: 2,
      },
    },
  ];
  const fixtureQueueStats = {
    queues: [
      {
        logical_queue: 'static_page_image_preview',
        queued: 1,
        running: 1,
        retrying: 0,
        succeeded: 2,
        failed: 0,
        task_count: 4,
      },
      {
        logical_queue: 'static_page_publish',
        queued: 1,
        running: 0,
        retrying: 1,
        succeeded: 3,
        failed: 1,
        task_count: 6,
      },
      {
        logical_queue: 'external_source',
        queued: 99,
        running: 99,
        task_count: 198,
      },
    ],
  };
  const report = buildReport(fixtureArgs, {
    events: fixtureEvents,
    queueStats: fixtureQueueStats,
    queueHttpStatus: 200,
    queueBodyPrefix: '{"queues":[...]}',
  });
  assert.equal(report.summary.ok, true);
  for (const status of DEFAULT_REQUIRED_STATUSES) {
    assert.equal(report.eventSummary.counts[status], 1, `missing fixture status ${status}`);
  }
  assert.equal(report.eventSummary.normalized.find((item) => item.normalizedStatus === 'waiting_for_low_load')?.customerVisible, false);
  assert.equal(report.summary.waitingForLowLoadCount, 1);
  assert.equal(report.summary.customerVisiblePrewarmLeakCount, 0);
  assert.equal(report.summary.prewarmCustomerVisibilityOk, true);
  assert.equal(report.queueSummary.queueCount, 2);
  assert.equal(report.queueSummary.activeCount, 4);
  const unauthReport = buildReport(
    {
      ...fixtureArgs,
      selfTest: false,
      allowMissingCredentials: true,
      cookie: '',
      bearer: '',
    },
    {
      events: [],
      queueStats: null,
      queueHttpStatus: 401,
      queueBodyPrefix: '{"code":"auth_session_required"}',
    },
  );
  assert.equal(unauthReport.summary.pending, true);
  assert.equal(unauthReport.summary.failed, false);
  assert.equal(unauthReport.auth.credentialsProvided, false);
  const reportPath = await writeReport(fixtureArgs, report);
  console.log(JSON.stringify(report.summary, null, 2));
  console.log(`report=${reportPath}`);
}

async function runLive(args) {
  const headers = requestHeaders(args);
  const base = normalizeBaseUrl(args.baseUrl);
  const queueResult = await requestJson(
    `${base}/v1/workflow-tasks/queue-stats?limit=500`,
    headers,
    args.timeoutMs,
  );
  const report = buildReport(args, {
    events: [],
    queueStats: queueResult.data,
    queueHttpStatus: queueResult.response.status,
    queueBodyPrefix: queueResult.text.slice(0, 300),
  });
  const reportPath = await writeReport(args, report);
  console.log(JSON.stringify(report.summary, null, 2));
  console.log(`report=${reportPath}`);
  if (report.summary.failed) {
    process.exitCode = 1;
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) {
    await runSelfTest(args);
    return;
  }
  await runLive(args);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
