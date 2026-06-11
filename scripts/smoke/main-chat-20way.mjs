#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_CONCURRENCY = 20;
const DEFAULT_TIMEOUT_MS = 60_000;
const DEFAULT_POLL_TIMEOUT_MS = 0;
const DEFAULT_POLL_INTERVAL_MS = 3_000;

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.MAIN_CHAT_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    datasetId: process.env.MAIN_CHAT_SMOKE_DATASET_ID || '',
    cookie: process.env.MAIN_CHAT_SMOKE_COOKIE || '',
    bearer: process.env.MAIN_CHAT_SMOKE_BEARER || '',
    activeSecretBindingIds: process.env.MAIN_CHAT_SMOKE_ACTIVE_SECRET_BINDING_IDS || '',
    concurrency: Number(process.env.MAIN_CHAT_SMOKE_CONCURRENCY || DEFAULT_CONCURRENCY),
    timeoutMs: Number(process.env.MAIN_CHAT_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    pollTimeoutMs: Number(process.env.MAIN_CHAT_SMOKE_POLL_TIMEOUT_MS || DEFAULT_POLL_TIMEOUT_MS),
    pollIntervalMs: Number(process.env.MAIN_CHAT_SMOKE_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS),
    prompt: process.env.MAIN_CHAT_SMOKE_PROMPT || '请用一句话回答：这是主站20路并发smoke测试。',
    titlePrefix: process.env.MAIN_CHAT_SMOKE_TITLE_PREFIX || '主站20路smoke',
    outputDir: process.env.MAIN_CHAT_SMOKE_OUTPUT_DIR || 'target/main-chat-20way-smoke',
    selfTest: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--base-url') {
      args.baseUrl = requireValue(arg, next);
      index += 1;
    } else if (arg === '--dataset-id') {
      args.datasetId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--cookie') {
      args.cookie = requireValue(arg, next);
      index += 1;
    } else if (arg === '--bearer') {
      args.bearer = requireValue(arg, next);
      index += 1;
    } else if (arg === '--active-secret-binding-ids') {
      args.activeSecretBindingIds = requireValue(arg, next);
      index += 1;
    } else if (arg === '--concurrency') {
      args.concurrency = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--poll-timeout-ms') {
      args.pollTimeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--poll-interval-ms') {
      args.pollIntervalMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--prompt') {
      args.prompt = requireValue(arg, next);
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
    throw new Error('--dataset-id or MAIN_CHAT_SMOKE_DATASET_ID is required');
  }
  if (!Number.isInteger(args.concurrency) || args.concurrency < 1 || args.concurrency > 100) {
    throw new Error('--concurrency must be an integer from 1 to 100');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) {
    throw new Error('--timeout-ms must be at least 1000');
  }
  if (!Number.isInteger(args.pollTimeoutMs) || args.pollTimeoutMs < 0) {
    throw new Error('--poll-timeout-ms must be zero or a positive integer');
  }
  if (!Number.isInteger(args.pollIntervalMs) || args.pollIntervalMs < 500) {
    throw new Error('--poll-interval-ms must be at least 500');
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
  node scripts/smoke/main-chat-20way.mjs \\
    --base-url http://127.0.0.1:3000 \\
    --dataset-id <dataset-uuid> \\
    --cookie "session=..."

Optional:
  --poll-timeout-ms 120000       wait for assistant messages after enqueue
  --active-secret-binding-ids a,b forward selected local secret bindings
  --self-test                    run deterministic fixture checks without calling DataMax
`);
}

function requestHeaders(args) {
  const headers = { 'content-type': 'application/json' };
  if (args.cookie) {
    headers.cookie = args.cookie;
  }
  if (args.bearer) {
    headers.authorization = args.bearer.toLowerCase().startsWith('bearer ')
      ? args.bearer
      : `Bearer ${args.bearer}`;
  }
  if (args.activeSecretBindingIds) {
    headers['x-active-secret-binding-ids'] = args.activeSecretBindingIds;
  }
  return headers;
}

function buildPayload(args, index, runId) {
  const number = String(index + 1).padStart(2, '0');
  return {
    prompt: `${args.prompt} 编号 ${number}。`,
    title: `${args.titlePrefix}-${runId}-${number}`,
    local_thread_id: `main-chat-20way-${runId}-${number}`,
  };
}

async function requestJson(url, options, timeoutMs) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { ...options, signal: controller.signal });
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

async function postOne(args, index, runId) {
  const startedAt = Date.now();
  const headers = requestHeaders(args);
  const payload = buildPayload(args, index, runId);
  const url = new URL(
    `/v1/datasets/${encodeURIComponent(args.datasetId)}/chat-sessions`,
    normalizeBaseUrl(args.baseUrl),
  );

  try {
    const submit = await requestJson(
      url,
      { method: 'POST', headers, body: JSON.stringify(payload) },
      args.timeoutMs,
    );
    const sessionId = submit.data?.chat_session?.id || null;
    const workflowExecutionId = submit.data?.workflow_execution?.id || null;
    let assistantMessageObserved = false;
    const polls = [];
    const deadline = Date.now() + args.pollTimeoutMs;
    while (sessionId && !assistantMessageObserved && Date.now() < deadline) {
      await sleep(args.pollIntervalMs);
      const messages = await requestJson(
        new URL(`/v1/chat-sessions/${encodeURIComponent(sessionId)}/messages`, normalizeBaseUrl(args.baseUrl)),
        { method: 'GET', headers },
        args.timeoutMs,
      );
      const messageItems = Array.isArray(messages.data) ? messages.data : [];
      assistantMessageObserved = messageItems.some((item) => item.role === 'assistant' && String(item.content || '').trim());
      polls.push({
        httpStatus: messages.response.status,
        messageCount: messageItems.length,
        assistantMessageObserved,
      });
    }

    const accepted = submit.response.status === 201 && Boolean(sessionId) && Boolean(workflowExecutionId);
    const ok = accepted && (args.pollTimeoutMs > 0 ? assistantMessageObserved : true);
    return {
      index,
      ok,
      accepted,
      httpStatus: submit.response.status,
      latencyMs: Date.now() - startedAt,
      sessionId,
      workflowExecutionId,
      workflowStage: submit.data?.workflow_execution?.stage || null,
      assistantMessageObserved,
      pollCount: polls.length,
      polls,
      bodyPrefix: submit.text.slice(0, 300),
      error: null,
    };
  } catch (error) {
    return {
      index,
      ok: false,
      accepted: false,
      httpStatus: null,
      latencyMs: Date.now() - startedAt,
      sessionId: null,
      workflowExecutionId: null,
      workflowStage: null,
      assistantMessageObserved: false,
      pollCount: 0,
      polls: [],
      bodyPrefix: '',
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function percentile(values, ratio) {
  if (values.length === 0) {
    return null;
  }
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.ceil(sorted.length * ratio) - 1);
  return sorted[index];
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function summarizeRun(args, runId, results) {
  const okCount = results.filter((item) => item.ok).length;
  const acceptedCount = results.filter((item) => item.accepted).length;
  const assistantMessageCount = results.filter((item) => item.assistantMessageObserved).length;
  const latencies = results.map((item) => item.latencyMs);
  return {
    runId,
    baseUrl: args.baseUrl,
    datasetId: args.datasetId,
    concurrency: args.concurrency,
    okCount,
    failedCount: results.length - okCount,
    acceptedCount,
    assistantMessageCount,
    pollTimeoutMs: args.pollTimeoutMs,
    p50LatencyMs: percentile(latencies, 0.5),
    p95LatencyMs: percentile(latencies, 0.95),
    maxLatencyMs: latencies.length ? Math.max(...latencies) : null,
    generatedAt: new Date().toISOString(),
  };
}

async function runSelfTest(args) {
  const runId = `${new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14)}-self-test`;
  const fixtureArgs = {
    ...args,
    baseUrl: 'https://doc.elepcloud.com',
    datasetId: '00000000-0000-0000-0000-000000000020',
    cookie: '',
    bearer: '',
    activeSecretBindingIds: '',
    concurrency: DEFAULT_CONCURRENCY,
    pollTimeoutMs: 120_000,
  };
  const payloads = Array.from({ length: fixtureArgs.concurrency }, (_, index) => buildPayload(fixtureArgs, index, runId));
  const results = payloads.map((payload, index) => ({
    index,
    ok: true,
    accepted: true,
    httpStatus: 201,
    latencyMs: 100 + index,
    sessionId: `session-${index + 1}`,
    workflowExecutionId: `workflow-${index + 1}`,
    workflowStage: 'queued',
    assistantMessageObserved: true,
    pollCount: 1,
    polls: [{ httpStatus: 200, messageCount: 2, assistantMessageObserved: true }],
    bodyPrefix: '{"chat_session":{"id":"redacted"}}',
    error: null,
  }));
  const summary = {
    ...summarizeRun(fixtureArgs, runId, results),
    selfTest: true,
  };
  const checks = {
    defaultConcurrencyIsTwenty: fixtureArgs.concurrency === 20,
    payloadCountMatchesConcurrency: payloads.length === 20,
    payloadsHaveUniqueThreads: new Set(payloads.map((payload) => payload.local_thread_id)).size === 20,
    payloadsHaveExpectedTitlePrefix: payloads.every((payload) => payload.title.startsWith(`${fixtureArgs.titlePrefix}-${runId}-`)),
    promptNumberingPreserved: payloads[0]?.prompt.includes('编号 01') === true
      && payloads[19]?.prompt.includes('编号 20') === true,
    summaryCountsAllAccepted: summary.okCount === 20
      && summary.acceptedCount === 20
      && summary.assistantMessageCount === 20
      && summary.failedCount === 0,
    latencyPercentilesComputed: summary.p50LatencyMs === 109 && summary.p95LatencyMs === 118 && summary.maxLatencyMs === 119,
  };
  const ok = Object.values(checks).every(Boolean);
  const report = {
    summary: {
      ...summary,
      ok,
      checks,
      generatedAt: new Date().toISOString(),
    },
    payloadShape: {
      count: payloads.length,
      first: {
        hasPrompt: Boolean(payloads[0]?.prompt),
        hasTitle: Boolean(payloads[0]?.title),
        hasLocalThreadId: Boolean(payloads[0]?.local_thread_id),
      },
    },
  };
  const outputDir = join(process.cwd(), args.outputDir);
  await mkdir(outputDir, { recursive: true });
  const reportPath = join(outputDir, `${runId}.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify(report.summary, null, 2));
  console.log(`report=${reportPath}`);
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
  const runId = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
  const results = await Promise.all(
    Array.from({ length: args.concurrency }, (_, index) => postOne(args, index, runId)),
  );
  const summary = summarizeRun(args, runId, results);

  const outputDir = join(process.cwd(), args.outputDir);
  await mkdir(outputDir, { recursive: true });
  const report = { summary, results };
  const reportPath = join(outputDir, `${runId}.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');

  console.log(JSON.stringify(summary, null, 2));
  console.log(`report=${reportPath}`);

  if (summary.failedCount > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
