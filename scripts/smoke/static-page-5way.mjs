#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_CONNECTION_ID = 'generic-chat-main';
const DEFAULT_CONCURRENCY = 5;
const DEFAULT_TIMEOUT_MS = 90_000;
const DEFAULT_POLL_TIMEOUT_MS = 0;
const DEFAULT_POLL_INTERVAL_MS = 5_000;

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.STATIC_PAGE_5WAY_BASE_URL || DEFAULT_BASE_URL,
    connectionId: process.env.STATIC_PAGE_5WAY_CONNECTION_ID || DEFAULT_CONNECTION_ID,
    bearer: process.env.STATIC_PAGE_5WAY_BEARER || '',
    concurrency: Number(process.env.STATIC_PAGE_5WAY_CONCURRENCY || DEFAULT_CONCURRENCY),
    timeoutMs: Number(process.env.STATIC_PAGE_5WAY_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    pollTimeoutMs: Number(process.env.STATIC_PAGE_5WAY_POLL_TIMEOUT_MS || DEFAULT_POLL_TIMEOUT_MS),
    pollIntervalMs: Number(process.env.STATIC_PAGE_5WAY_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS),
    requireArtifact: parseBoolean(process.env.STATIC_PAGE_5WAY_REQUIRE_ARTIFACT),
    text: process.env.STATIC_PAGE_5WAY_TEXT || '请生成一页经营分析静态页，效果图可在流式过程中展示，确认后自动发布到 V3 产物链接。',
    defaultPrompt: process.env.STATIC_PAGE_5WAY_DEFAULT_PROMPT || '请面向业务用户，优先基于本轮文档和数据源回答。',
    sourceId: process.env.STATIC_PAGE_5WAY_SOURCE_ID || 'third-party-source-main',
    datasetExternalIds: parseList(process.env.STATIC_PAGE_5WAY_DATASET_EXTERNAL_IDS),
    documentExternalIds: parseList(process.env.STATIC_PAGE_5WAY_DOCUMENT_EXTERNAL_IDS),
    platform: process.env.STATIC_PAGE_5WAY_PLATFORM || 'generic_chat',
    tenantExternalId: process.env.STATIC_PAGE_5WAY_TENANT_EXTERNAL_ID || 'tenant-ext-smoke',
    botExternalId: process.env.STATIC_PAGE_5WAY_BOT_EXTERNAL_ID || 'bot-v3',
    outputDir: process.env.STATIC_PAGE_5WAY_OUTPUT_DIR || 'target/static-page-5way-smoke',
    selfTest: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--base-url') {
      args.baseUrl = requireValue(arg, next);
      index += 1;
    } else if (arg === '--connection-id') {
      args.connectionId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--bearer') {
      args.bearer = requireValue(arg, next);
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
    } else if (arg === '--require-artifact') {
      args.requireArtifact = true;
    } else if (arg === '--text') {
      args.text = requireValue(arg, next);
      index += 1;
    } else if (arg === '--source-id') {
      args.sourceId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--dataset-external-ids') {
      args.datasetExternalIds = parseList(requireValue(arg, next));
      index += 1;
    } else if (arg === '--document-external-ids') {
      args.documentExternalIds = parseList(requireValue(arg, next));
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

  if (!Number.isInteger(args.concurrency) || args.concurrency < 1 || args.concurrency > 50) {
    throw new Error('--concurrency must be an integer from 1 to 50');
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

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y'].includes(String(value || '').trim().toLowerCase());
}

function parseList(value) {
  if (!value) {
    return [];
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
  node scripts/smoke/static-page-5way.mjs \\
    --base-url http://127.0.0.1:3000 \\
    --connection-id generic-chat-main \\
    --bearer <token> \\
    --concurrency 5

Optional:
  --poll-timeout-ms 300000    poll status URLs for up to 5 minutes
  --require-artifact          fail unless a final artifact URL is observed
  --dataset-external-ids a,b  scope the static page to dataset groups
  --self-test                 run deterministic fixture checks without calling DataMax
`);
}

function buildPayload(args, index, runId) {
  const suffix = `${runId}-${String(index + 1).padStart(2, '0')}`;
  return {
    platform: args.platform,
    tenant_external_id: args.tenantExternalId,
    bot_external_id: args.botExternalId,
    conversation_external_id: `conv-static-page-5way-${suffix}`,
    thread_external_id: null,
    sender_external_id: `user-static-page-5way-${String(index + 1).padStart(2, '0')}`,
    sender_display_name: 'V3 static page smoke',
    message_external_id: `msg-static-page-5way-${suffix}`,
    message_type: 'text',
    text: `${args.text} 并发编号 ${index + 1}。`,
    default_prompt: args.defaultPrompt,
    output_format: 'rich_text',
    render_mode: 'artifact',
    artifact_type: 'static_page',
    available_document_source_id: args.sourceId || null,
    available_document_external_ids: args.documentExternalIds,
    dataset_external_ids: args.datasetExternalIds,
    requested_skills: [],
    mention_external_user_ids: [],
    attachment_refs: [],
    idempotency_key: `static-page-5way:${suffix}`,
    received_at: new Date().toISOString(),
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
  const payload = buildPayload(args, index, runId);
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/events`,
    normalizeBaseUrl(args.baseUrl),
  );
  const headers = { 'content-type': 'application/json' };
  if (args.bearer) {
    headers.authorization = args.bearer.toLowerCase().startsWith('bearer ')
      ? args.bearer
      : `Bearer ${args.bearer}`;
  }

  try {
    const submit = await requestJson(
      url,
      { method: 'POST', headers, body: JSON.stringify(payload) },
      args.timeoutMs,
    );
    const polls = [];
    let latest = submit.data;
    let artifactUrl = firstArtifactUrl(latest);
    let statusUrl = statusUrlFromResponse(latest, args.baseUrl, args.connectionId);
    let terminalFailure = isTerminalFailure(latest);
    const deadline = Date.now() + args.pollTimeoutMs;
    while (!artifactUrl && !terminalFailure && statusUrl && Date.now() < deadline) {
      await sleep(args.pollIntervalMs);
      const poll = await requestJson(statusUrl, { method: 'GET', headers }, args.timeoutMs);
      latest = poll.data;
      artifactUrl = firstArtifactUrl(latest);
      statusUrl = statusUrlFromResponse(latest, args.baseUrl, args.connectionId) || statusUrl;
      terminalFailure = isTerminalFailure(latest);
      polls.push({
        httpStatus: poll.response.status,
        taskStatus: latest?.reply?.task_status || null,
        replyType: latest?.reply?.reply_type || null,
        hasArtifactUrl: Boolean(artifactUrl),
        terminalFailure,
      });
    }

    const accepted = submit.response.ok && Boolean(submit.data) && !isTerminalFailure(submit.data);
    const hasProgressHandle = Boolean(artifactUrl || statusUrl || latest?.assistant_run_id);
    const ok = accepted && !terminalFailure && (args.requireArtifact ? Boolean(artifactUrl) : hasProgressHandle);
    return {
      index,
      ok,
      accepted,
      httpStatus: submit.response.status,
      latencyMs: Date.now() - startedAt,
      conversationExternalId: payload.conversation_external_id,
      messageExternalId: payload.message_external_id,
      idempotencyKey: payload.idempotency_key,
      assistantRunId: latest?.assistant_run_id || submit.data?.assistant_run_id || null,
      initialTaskStatus: submit.data?.reply?.task_status || null,
      finalTaskStatus: latest?.reply?.task_status || null,
      finalReplyType: latest?.reply?.reply_type || null,
      statusUrl,
      artifactUrl,
      terminalFailure,
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
      conversationExternalId: payload.conversation_external_id,
      messageExternalId: payload.message_external_id,
      idempotencyKey: payload.idempotency_key,
      assistantRunId: null,
      initialTaskStatus: null,
      finalTaskStatus: null,
      finalReplyType: null,
      statusUrl: null,
      artifactUrl: null,
      terminalFailure: false,
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

function firstArtifactUrl(response) {
  const reply = response?.reply;
  if (!reply) {
    return null;
  }
  if (Array.isArray(reply.artifact_links) && reply.artifact_links.length > 0) {
    return String(reply.artifact_links[0]);
  }
  const card = reply.card;
  if (card && typeof card === 'object') {
    for (const key of ['public_url', 'generated_artifact_url', 'html_preview_url', 'html_download_url']) {
      if (card[key]) {
        return String(card[key]);
      }
    }
  }
  return null;
}

function statusUrlFromResponse(response, baseUrl, connectionId) {
  const statusUrl = response?.reply?.card?.status_url;
  if (statusUrl) {
    const text = String(statusUrl);
    if (text.startsWith('http://') || text.startsWith('https://')) {
      return text;
    }
    return `${normalizeBaseUrl(baseUrl)}/${text.replace(/^\/+/, '')}`;
  }
  const runId = response?.assistant_run_id;
  if (runId && connectionId) {
    return `${normalizeBaseUrl(baseUrl)}/v1/external/channels/${encodeURIComponent(connectionId)}/assistant-runs/${encodeURIComponent(runId)}/reply`;
  }
  return null;
}

function isTerminalFailure(response) {
  const statuses = [];
  const reply = response?.reply;
  if (reply) {
    statuses.push(reply.task_status, reply.reply_type);
    if (reply.card && typeof reply.card === 'object') {
      statuses.push(
        reply.card.status,
        reply.card.codex_final_status,
        reply.card.render_output_status,
        reply.card.image_job_status,
      );
    }
  }
  return statuses.filter(Boolean).some((value) => /failed|cancelled|needs_human/i.test(String(value)));
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

async function runSelfTest(args) {
  const runId = `${new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14)}-self-test`;
  const fixtureArgs = {
    ...args,
    baseUrl: 'https://v3.elepcloud.com',
    connectionId: DEFAULT_CONNECTION_ID,
    concurrency: DEFAULT_CONCURRENCY,
    requireArtifact: false,
    pollTimeoutMs: 0,
    datasetExternalIds: ['dataset-a', 'dataset-b'],
    documentExternalIds: ['document-extra'],
  };
  const payloads = Array.from({ length: fixtureArgs.concurrency }, (_, index) => buildPayload(fixtureArgs, index, runId));
  const published = {
    assistant_run_id: 'run-static-page-published',
    reply: {
      reply_type: 'artifact_link',
      task_status: 'static_page_published',
      artifact_links: [
        'https://v3.elepcloud.com/generated-artifacts/database-static-pages/self-test/index.html',
      ],
      card: {
        public_url: 'https://v3.elepcloud.com/generated-artifacts/database-static-pages/self-test/index.html',
      },
    },
  };
  const queued = {
    assistant_run_id: 'run-static-page-queued',
    reply: {
      reply_type: 'task_accepted',
      task_status: 'static_page_queued',
      card: {
        status_url: '/v1/external/channels/generic-chat-main/assistant-runs/run-static-page-queued/reply',
      },
    },
  };
  const failed = {
    assistant_run_id: 'run-static-page-failed',
    reply: {
      reply_type: 'task_failed',
      task_status: 'static_page_failed',
      card: {
        status: 'failed',
      },
    },
  };
  const checks = {
    defaultConcurrencyIsFive: fixtureArgs.concurrency === 5,
    payloadCountMatchesConcurrency: payloads.length === 5,
    payloadsRequestStaticPageArtifact: payloads.every((payload) => (
      payload.render_mode === 'artifact'
        && payload.artifact_type === 'static_page'
        && payload.output_format === 'rich_text'
        && payload.dataset_external_ids.length === 2
        && payload.available_document_external_ids.length === 1
    )),
    artifactUrlDetected: firstArtifactUrl(published)?.endsWith('/index.html') === true,
    statusUrlResolved: statusUrlFromResponse(queued, fixtureArgs.baseUrl, fixtureArgs.connectionId)
      === 'https://v3.elepcloud.com/v1/external/channels/generic-chat-main/assistant-runs/run-static-page-queued/reply',
    terminalFailureDetected: isTerminalFailure(failed) === true,
  };
  const ok = Object.values(checks).every(Boolean);
  const summary = {
    runId,
    selfTest: true,
    ok,
    baseUrl: fixtureArgs.baseUrl,
    connectionId: fixtureArgs.connectionId,
    concurrency: fixtureArgs.concurrency,
    checks,
    generatedAt: new Date().toISOString(),
  };
  const report = {
    summary,
    payloadShape: {
      count: payloads.length,
      first: {
        platform: payloads[0]?.platform,
        render_mode: payloads[0]?.render_mode,
        artifact_type: payloads[0]?.artifact_type,
        dataset_external_ids_count: payloads[0]?.dataset_external_ids?.length || 0,
        available_document_external_ids_count: payloads[0]?.available_document_external_ids?.length || 0,
      },
    },
  };
  const outputDir = join(process.cwd(), args.outputDir);
  await mkdir(outputDir, { recursive: true });
  const reportPath = join(outputDir, `${runId}.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify(summary, null, 2));
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
  const okCount = results.filter((item) => item.ok).length;
  const acceptedCount = results.filter((item) => item.accepted).length;
  const artifactCount = results.filter((item) => item.artifactUrl).length;
  const latencies = results.map((item) => item.latencyMs);
  const summary = {
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    concurrency: args.concurrency,
    okCount,
    failedCount: results.length - okCount,
    acceptedCount,
    artifactCount,
    requireArtifact: args.requireArtifact,
    pollTimeoutMs: args.pollTimeoutMs,
    p50LatencyMs: percentile(latencies, 0.5),
    p95LatencyMs: percentile(latencies, 0.95),
    maxLatencyMs: latencies.length ? Math.max(...latencies) : null,
    generatedAt: new Date().toISOString(),
  };

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
