#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_CONNECTION_ID = 'generic-chat-main';
const DEFAULT_CONCURRENCY = 20;
const DEFAULT_TIMEOUT_MS = 90_000;

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.EXTERNAL_CHANNEL_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    connectionId: process.env.EXTERNAL_CHANNEL_SMOKE_CONNECTION_ID || DEFAULT_CONNECTION_ID,
    bearer: process.env.EXTERNAL_CHANNEL_SMOKE_BEARER || '',
    concurrency: Number(process.env.EXTERNAL_CHANNEL_SMOKE_CONCURRENCY || DEFAULT_CONCURRENCY),
    timeoutMs: Number(process.env.EXTERNAL_CHANNEL_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    text: process.env.EXTERNAL_CHANNEL_SMOKE_TEXT || '请用一句话回答：这是第三方20路并发smoke测试。',
    sourceId: process.env.EXTERNAL_CHANNEL_SMOKE_SOURCE_ID || 'third-party-source-main',
    platform: process.env.EXTERNAL_CHANNEL_SMOKE_PLATFORM || 'generic_chat',
    tenantExternalId: process.env.EXTERNAL_CHANNEL_SMOKE_TENANT_EXTERNAL_ID || 'tenant-ext-smoke',
    botExternalId: process.env.EXTERNAL_CHANNEL_SMOKE_BOT_EXTERNAL_ID || 'bot-v3',
    outputDir: process.env.EXTERNAL_CHANNEL_SMOKE_OUTPUT_DIR || 'target/external-channel-20way-smoke',
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
    } else if (arg === '--text') {
      args.text = requireValue(arg, next);
      index += 1;
    } else if (arg === '--source-id') {
      args.sourceId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (!Number.isInteger(args.concurrency) || args.concurrency < 1 || args.concurrency > 200) {
    throw new Error('--concurrency must be an integer from 1 to 200');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) {
    throw new Error('--timeout-ms must be at least 1000');
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
  node scripts/smoke/external-channel-20way.mjs \\
    --base-url http://127.0.0.1:3000 \\
    --connection-id generic-chat-main \\
    --bearer <token> \\
    --concurrency 20

Environment aliases:
  EXTERNAL_CHANNEL_SMOKE_BASE_URL
  EXTERNAL_CHANNEL_SMOKE_CONNECTION_ID
  EXTERNAL_CHANNEL_SMOKE_BEARER
  EXTERNAL_CHANNEL_SMOKE_CONCURRENCY
  EXTERNAL_CHANNEL_SMOKE_TIMEOUT_MS
`);
}

function buildPayload(args, index, runId) {
  const suffix = `${runId}-${String(index + 1).padStart(2, '0')}`;
  return {
    platform: args.platform,
    tenant_external_id: args.tenantExternalId,
    bot_external_id: args.botExternalId,
    conversation_external_id: `conv-20way-${suffix}`,
    thread_external_id: null,
    sender_external_id: `user-20way-${String(index + 1).padStart(2, '0')}`,
    sender_display_name: null,
    message_external_id: `msg-20way-${suffix}`,
    message_type: 'text',
    text: `${args.text} 编号 ${index + 1}。`,
    default_prompt: '',
    output_format: 'rich_text',
    render_mode: 'normal',
    available_document_source_id: args.sourceId,
    available_document_external_ids: [],
    dataset_external_ids: [],
    documentExternalId: null,
    requested_skills: [],
    mention_external_user_ids: [],
    attachment_refs: [],
    idempotency_key: `external-channel-20way:${suffix}`,
    received_at: new Date().toISOString(),
    render_options: null,
  };
}

function parseSse(text) {
  const events = [];
  let eventName = '';
  let dataLines = [];

  const flush = () => {
    if (!eventName && dataLines.length === 0) {
      return;
    }
    const dataText = dataLines.join('\n');
    let data = dataText;
    try {
      data = dataText ? JSON.parse(dataText) : null;
    } catch {
      // Keep raw data for diagnostics.
    }
    events.push({ event: eventName || 'message', data });
    eventName = '';
    dataLines = [];
  };

  for (const line of text.split(/\r?\n/)) {
    if (line === '') {
      flush();
    } else if (line.startsWith('event:')) {
      eventName = line.slice('event:'.length).trim();
    } else if (line.startsWith('data:')) {
      dataLines.push(line.slice('data:'.length).trimStart());
    }
  }
  flush();
  return events;
}

async function postOne(args, index, runId) {
  const startedAt = Date.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), args.timeoutMs);
  const payload = buildPayload(args, index, runId);
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/events/stream`,
    normalizeBaseUrl(args.baseUrl),
  );
  const headers = {
    'content-type': 'application/json',
  };
  if (args.bearer) {
    headers.authorization = args.bearer.toLowerCase().startsWith('bearer ')
      ? args.bearer
      : `Bearer ${args.bearer}`;
  }

  try {
    const response = await fetch(url, {
      method: 'POST',
      headers,
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
    const body = await response.text();
    const events = parseSse(body);
    const completed = events.find((item) => item.event === 'external_channel.completed');
    const error = events.find((item) => item.event === 'external_channel.error');
    const reply = completed?.data?.reply || null;
    const status = reply?.task_status || completed?.data?.status || null;
    return {
      index,
      ok: response.ok && Boolean(completed) && !error,
      httpStatus: response.status,
      latencyMs: Date.now() - startedAt,
      conversationExternalId: payload.conversation_external_id,
      messageExternalId: payload.message_external_id,
      idempotencyKey: payload.idempotency_key,
      assistantRunId: completed?.data?.assistant_run_id || null,
      completed: Boolean(completed),
      replyType: reply?.reply_type || null,
      taskStatus: status,
      error: error?.data || null,
      eventNames: events.map((item) => item.event),
      bodyPrefix: body.slice(0, 300),
    };
  } catch (error) {
    return {
      index,
      ok: false,
      httpStatus: null,
      latencyMs: Date.now() - startedAt,
      conversationExternalId: payload.conversation_external_id,
      messageExternalId: payload.message_external_id,
      idempotencyKey: payload.idempotency_key,
      completed: false,
      replyType: null,
      taskStatus: null,
      error: error instanceof Error ? error.message : String(error),
      eventNames: [],
      bodyPrefix: '',
    };
  } finally {
    clearTimeout(timeout);
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

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const runId = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
  const results = await Promise.all(
    Array.from({ length: args.concurrency }, (_, index) => postOne(args, index, runId)),
  );
  const okCount = results.filter((item) => item.ok).length;
  const latencies = results.map((item) => item.latencyMs);
  const summary = {
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    concurrency: args.concurrency,
    okCount,
    failedCount: results.length - okCount,
    p50LatencyMs: percentile(latencies, 0.5),
    p95LatencyMs: percentile(latencies, 0.95),
    maxLatencyMs: latencies.length ? Math.max(...latencies) : null,
    completedCount: results.filter((item) => item.completed).length,
    answeredCount: results.filter((item) => item.taskStatus === 'answered').length,
    acceptedOrAnsweredCount: results.filter((item) =>
      ['accepted', 'answered', 'processing', 'queued'].includes(String(item.taskStatus || '')),
    ).length,
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
