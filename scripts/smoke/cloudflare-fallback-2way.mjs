#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_ALLOWED = 2;
const DEFAULT_MIN_EXPECTED = 1;

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.CLOUDFLARE_FALLBACK_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    cookie: process.env.CLOUDFLARE_FALLBACK_SMOKE_COOKIE || '',
    bearer: process.env.CLOUDFLARE_FALLBACK_SMOKE_BEARER || '',
    timeoutMs: Number(process.env.CLOUDFLARE_FALLBACK_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    maxAllowed: Number(process.env.CLOUDFLARE_FALLBACK_SMOKE_MAX_ALLOWED || DEFAULT_MAX_ALLOWED),
    minExpected: Number(process.env.CLOUDFLARE_FALLBACK_SMOKE_MIN_EXPECTED || DEFAULT_MIN_EXPECTED),
    queueFilter: process.env.CLOUDFLARE_FALLBACK_SMOKE_QUEUE_FILTER || 'codex,static_page_publish',
    outputDir: process.env.CLOUDFLARE_FALLBACK_SMOKE_OUTPUT_DIR || 'target/cloudflare-fallback-2way-smoke',
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
    } else if (arg === '--max-allowed') {
      args.maxAllowed = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--min-expected') {
      args.minExpected = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--queue-filter') {
      args.queueFilter = requireValue(arg, next);
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

  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) {
    throw new Error('--timeout-ms must be at least 1000');
  }
  if (!Number.isInteger(args.maxAllowed) || args.maxAllowed < 1) {
    throw new Error('--max-allowed must be a positive integer');
  }
  if (!Number.isInteger(args.minExpected) || args.minExpected < 0) {
    throw new Error('--min-expected must be zero or a positive integer');
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
  node scripts/smoke/cloudflare-fallback-2way.mjs \\
    --base-url http://127.0.0.1:3000 \\
    --cookie "session=..." \\
    --max-allowed 2 \\
    --min-expected 2

This is a read-only guard smoke. It verifies the Codex host worker cap and
checks current workflow queue running counts for Cloudflare/static-page publish queues.
Use --self-test to run deterministic fixture checks without calling DataMax.
`);
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

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function queueMatches(queue, filters) {
  const value = String(queue.logical_queue || queue.logicalQueue || '').toLowerCase();
  return filters.some((filter) => value.includes(filter));
}

function buildSummary(args, { statusResult, queueResult }) {
  const filters = args.queueFilter
    .split(',')
    .map((item) => item.trim().toLowerCase())
    .filter(Boolean);
  const codexWorker = (statusResult.data?.runtime?.worker_pools || [])
    .find((pool) => pool.service === 'codex-host-agent') || null;
  const codexConcurrency = Number(codexWorker?.concurrency || 0);
  const watchedQueues = (queueResult.data?.queues || [])
    .filter((queue) => queueMatches(queue, filters))
    .map((queue) => ({
      logicalQueue: queue.logical_queue || queue.logicalQueue || '',
      running: Number(queue.running || 0),
      queued: Number(queue.queued || 0),
      retrying: Number(queue.retrying || 0),
      taskCount: Number(queue.task_count || queue.taskCount || 0),
    }));
  const maxRunning = watchedQueues.reduce((max, queue) => Math.max(max, queue.running), 0);
  const checks = {
    modelGatewayStatusLoaded: statusResult.response.ok && Boolean(statusResult.data),
    workflowQueueStatsLoaded: queueResult.response.ok && Boolean(queueResult.data),
    codexWorkerPresent: Boolean(codexWorker),
    codexConcurrencyNonZero: codexConcurrency > 0,
    codexConcurrencyAtLeastMin: codexConcurrency >= args.minExpected,
    codexConcurrencyWithinMax: codexConcurrency <= args.maxAllowed,
    watchedQueuesWithinMax: maxRunning <= args.maxAllowed,
    noCookieOrBearerPrinted: true,
  };
  const ok = Object.values(checks).every(Boolean);
  const summary = {
    ok,
    ready: ok,
    pending: false,
    failed: !ok,
    selfTest: args.selfTest,
    baseUrl: args.baseUrl,
    maxAllowed: args.maxAllowed,
    minExpected: args.minExpected,
    codexConcurrency,
    maxRunning,
    watchedQueueCount: watchedQueues.length,
    checks,
    generatedAt: new Date().toISOString(),
  };
  const report = {
    summary,
    codexWorker,
    watchedQueues,
    statusHttpStatus: statusResult.response.status,
    queueHttpStatus: queueResult.response.status,
    statusBodyPrefix: statusResult.text.slice(0, 300),
    queueBodyPrefix: queueResult.text.slice(0, 300),
    auth: {
      credentialsProvided: Boolean(args.cookie || args.bearer),
      cookiePrinted: false,
      bearerPrinted: false,
    },
    redaction: {
      cookiePrinted: false,
      bearerPrinted: false,
      rawEnvValuesPrinted: false,
    },
  };
  return { summary, report };
}

async function writeReport(args, report) {
  const outputDir = join(process.cwd(), args.outputDir);
  await mkdir(outputDir, { recursive: true });
  const reportPath = join(outputDir, `${new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14)}${args.selfTest ? '-self-test' : ''}.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  return reportPath;
}

async function runSelfTest(args) {
  const fixtureStatus = {
    response: { ok: true, status: 200 },
    text: '{"runtime":{"worker_pools":[{"service":"codex-host-agent","concurrency":2}]}}',
    data: {
      runtime: {
        worker_pools: [
          { service: 'codex-host-agent', concurrency: 2 },
          { service: 'static-page-worker', concurrency: 5 },
        ],
      },
    },
  };
  const fixtureQueues = {
    response: { ok: true, status: 200 },
    text: '{"queues":[]}',
    data: {
      queues: [
        { logical_queue: 'codex', running: 2, queued: 0, retrying: 0, task_count: 12 },
        { logicalQueue: 'static_page_publish', running: 1, queued: 0, retrying: 0, taskCount: 8 },
        { logical_queue: 'external_source', running: 7, queued: 0, retrying: 0, task_count: 20 },
      ],
    },
  };
  const fixtureArgs = {
    ...args,
    maxAllowed: DEFAULT_MAX_ALLOWED,
    minExpected: DEFAULT_MIN_EXPECTED,
    queueFilter: 'codex,static_page_publish',
    selfTest: true,
  };
  const { summary, report } = buildSummary(fixtureArgs, {
    statusResult: fixtureStatus,
    queueResult: fixtureQueues,
  });
  report.summary = summary;
  const overCapQueues = structuredClone(fixtureQueues);
  overCapQueues.data.queues = [
    { logical_queue: 'codex', running: DEFAULT_MAX_ALLOWED + 1, queued: 0, retrying: 0, task_count: 9 },
  ];
  const { summary: overCapSummary } = buildSummary(fixtureArgs, {
    statusResult: fixtureStatus,
    queueResult: overCapQueues,
  });
  const missingWorkerStatus = structuredClone(fixtureStatus);
  missingWorkerStatus.data.runtime.worker_pools = [{ service: 'static-page-worker', concurrency: 5 }];
  const { summary: missingWorkerSummary } = buildSummary(fixtureArgs, {
    statusResult: missingWorkerStatus,
    queueResult: fixtureQueues,
  });
  report.selfTestCases = {
    passingSummaryOk: summary.ok === true,
    overCapSummaryFails: overCapSummary.ok === false
      && overCapSummary.checks.watchedQueuesWithinMax === false,
    missingWorkerSummaryFails: missingWorkerSummary.ok === false
      && missingWorkerSummary.checks.codexWorkerPresent === false,
  };
  if (!Object.values(report.selfTestCases).every(Boolean)) {
    throw new Error(`cloudflare fallback self-test failed: ${JSON.stringify(report.selfTestCases)}`);
  }
  const reportPath = await writeReport(fixtureArgs, report);
  console.log(JSON.stringify(summary, null, 2));
  console.log(`report=${reportPath}`);
  if (!summary.ok) {
    process.exitCode = 1;
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) {
    await runSelfTest(args);
    return;
  }
  const headers = requestHeaders(args);
  const base = normalizeBaseUrl(args.baseUrl);
  const statusResult = await requestJson(`${base}/v1/model-gateway/status`, headers, args.timeoutMs);
  const queueResult = await requestJson(`${base}/v1/workflow-tasks/queue-stats?limit=500`, headers, args.timeoutMs);
  const { summary, report } = buildSummary(args, { statusResult, queueResult });

  const reportPath = await writeReport(args, report);

  console.log(JSON.stringify(summary, null, 2));
  console.log(`report=${reportPath}`);

  if (!summary.ok) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
