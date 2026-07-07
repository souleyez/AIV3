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
    } else if (arg === '--text') {
      args.text = requireValue(arg, next);
      index += 1;
    } else if (arg === '--source-id') {
      args.sourceId = requireValue(arg, next);
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

Use --self-test for deterministic offline payload, SSE parser, and summary
checks without calling DataMax or requiring a bearer.
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

function firstExplicitUrl(value, keys, depth = 0, keyMatched = false) {
  if (!value || depth > 8) {
    return null;
  }
  if (typeof value === 'string') {
    return keyMatched && isUrlLike(value) ? value : null;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      const link = firstExplicitUrl(item, keys, depth + 1, keyMatched);
      if (link) {
        return link;
      }
    }
    return null;
  }
  if (typeof value !== 'object') {
    return null;
  }
  for (const key of keys) {
    if (Object.prototype.hasOwnProperty.call(value, key)) {
      const link = firstExplicitUrl(value[key], keys, depth + 1, true);
      if (link) {
        return link;
      }
    }
  }
  for (const child of Object.values(value)) {
    const link = firstExplicitUrl(child, keys, depth + 1, false);
    if (link) {
      return link;
    }
  }
  return null;
}

function isUrlLike(value) {
  const text = String(value || '').trim();
  return text.startsWith('http://') || text.startsWith('https://') || text.startsWith('/');
}

function firstArtifactUrl(value) {
  return firstExplicitUrl(value, [
    'artifact_links',
    'artifactLinks',
    'public_url',
    'publicUrl',
    'generated_artifact_url',
    'generatedArtifactUrl',
    'artifact_public_url',
    'artifactPublicUrl',
    'html_preview_url',
    'htmlPreviewUrl',
    'html_download_url',
    'htmlDownloadUrl',
    'download_url',
    'downloadUrl',
  ]);
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
    const reply = completed?.data?.response?.reply || completed?.data?.reply || null;
    const status = reply?.task_status || completed?.data?.status || null;
    const artifactUrl = firstArtifactUrl(completed?.data);
    const noArtifactLink = !artifactUrl;
    return {
      index,
      ok: response.ok && Boolean(completed) && !error && noArtifactLink,
      httpStatus: response.status,
      latencyMs: Date.now() - startedAt,
      conversationExternalId: payload.conversation_external_id,
      messageExternalId: payload.message_external_id,
      idempotencyKey: payload.idempotency_key,
      assistantRunId: completed?.data?.assistant_run_id || null,
      completed: Boolean(completed),
      replyType: reply?.reply_type || null,
      taskStatus: status,
      artifactUrl,
      noArtifactLink,
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
      artifactUrl: null,
      noArtifactLink: true,
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

function summarizeRun(args, runId, results) {
  const okCount = results.filter((item) => item.ok).length;
  const latencies = results.map((item) => item.latencyMs);
  const normalArtifactLeakCount = results.filter((item) => item.artifactUrl).length;
  const checks = {
    allTasksPassed: results.length - okCount === 0,
    noNormalArtifactLeak: normalArtifactLeakCount === 0,
  };
  return {
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    concurrency: args.concurrency,
    ok: Object.values(checks).every(Boolean),
    checks,
    okCount,
    failedCount: results.length - okCount,
    normalArtifactLeakCount,
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
}

async function runSelfTest(args) {
  const runId = `${new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14)}-self-test`;
  const fixtureArgs = {
    ...args,
    baseUrl: 'https://v3.elepcloud.com',
    connectionId: DEFAULT_CONNECTION_ID,
    bearer: '',
    concurrency: DEFAULT_CONCURRENCY,
  };
  const payloads = Array.from({ length: fixtureArgs.concurrency }, (_, index) => buildPayload(fixtureArgs, index, runId));
  const sseText = [
    'event: external_channel.started',
    'data: {"assistant_run_id":"run-self-test"}',
    '',
    'event: external_channel.delta',
    'data: {"delta":"progress"}',
    '',
    'event: external_channel.completed',
    'data: {"assistant_run_id":"run-self-test","response":{"reply":{"reply_type":"answer","task_status":"answered"}}}',
    '',
  ].join('\n');
  const artifactLeakSseText = [
    'event: external_channel.completed',
    'data: {"assistant_run_id":"run-artifact-leak","response":{"reply":{"reply_type":"answer","task_status":"answered","text":"普通问答不应返回产物链接","card":{"public_url":"/generated-artifacts/self-test/leaked/index.html"},"artifact_links":["/generated-artifacts/self-test/leaked/index.html"]}}}',
    '',
  ].join('\n');
  const errorSseText = [
    'event: external_channel.error',
    'data: {"code":"self_test_error","message":"fixture"}',
    '',
  ].join('\n');
  const events = parseSse(sseText);
  const artifactLeakEvents = parseSse(artifactLeakSseText);
  const errorEvents = parseSse(errorSseText);
  const results = payloads.map((payload, index) => ({
    index,
    ok: true,
    httpStatus: 200,
    latencyMs: 200 + index,
    conversationExternalId: payload.conversation_external_id,
    messageExternalId: payload.message_external_id,
    idempotencyKey: payload.idempotency_key,
    assistantRunId: 'run-self-test',
    completed: true,
    replyType: 'answer',
    taskStatus: index % 2 === 0 ? 'answered' : 'accepted',
    artifactUrl: null,
    noArtifactLink: true,
    error: null,
    eventNames: events.map((item) => item.event),
    bodyPrefix: 'event: external_channel.completed',
  }));
  const artifactLeakUrl = firstArtifactUrl(artifactLeakEvents[0]?.data);
  const artifactLeakResult = {
    index: 99,
    ok: false,
    httpStatus: 200,
    latencyMs: 199,
    conversationExternalId: 'conv-20way-artifact-leak',
    messageExternalId: 'msg-20way-artifact-leak',
    idempotencyKey: 'external-channel-20way:artifact-leak',
    assistantRunId: 'run-artifact-leak',
    completed: true,
    replyType: 'answer',
    taskStatus: 'answered',
    artifactUrl: artifactLeakUrl,
    noArtifactLink: !artifactLeakUrl,
    error: null,
    eventNames: artifactLeakEvents.map((item) => item.event),
    bodyPrefix: 'event: external_channel.completed',
  };
  const artifactLeakSummary = summarizeRun(
    { ...fixtureArgs, concurrency: 1 },
    runId,
    [artifactLeakResult],
  );
  const summary = {
    ...summarizeRun(fixtureArgs, runId, results),
    selfTest: true,
  };
  const checks = {
    defaultConcurrencyIsTwenty: fixtureArgs.concurrency === 20,
    payloadCountMatchesConcurrency: payloads.length === 20,
    payloadsHaveUniqueConversationIds: new Set(payloads.map((payload) => payload.conversation_external_id)).size === 20,
    payloadsHaveUniqueMessageIds: new Set(payloads.map((payload) => payload.message_external_id)).size === 20,
    payloadsHaveUniqueIdempotencyKeys: new Set(payloads.map((payload) => payload.idempotency_key)).size === 20,
    completedEventParsed: events.some((item) => item.event === 'external_channel.completed'
      && item.data?.response?.reply?.task_status === 'answered'),
    errorEventParsed: errorEvents.some((item) => item.event === 'external_channel.error'
      && item.data?.code === 'self_test_error'),
    summaryCountsAllComplete: summary.okCount === 20
      && summary.completedCount === 20
      && summary.answeredCount === 10
      && summary.acceptedOrAnsweredCount === 20
      && summary.failedCount === 0
      && summary.normalArtifactLeakCount === 0
      && summary.ok === true
      && summary.checks?.allTasksPassed === true
      && summary.checks?.noNormalArtifactLeak === true,
    normalArtifactLeakGuardWorks: artifactLeakResult.ok === false
      && artifactLeakResult.noArtifactLink === false
      && Boolean(artifactLeakResult.artifactUrl)
      && artifactLeakSummary.ok === false
      && artifactLeakSummary.failedCount === 1
      && artifactLeakSummary.normalArtifactLeakCount === 1
      && artifactLeakSummary.checks?.noNormalArtifactLeak === false,
    latencyPercentilesComputed: summary.p50LatencyMs === 209 && summary.p95LatencyMs === 218 && summary.maxLatencyMs === 219,
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
        platform: payloads[0]?.platform,
        output_format: payloads[0]?.output_format,
        render_mode: payloads[0]?.render_mode,
        dataset_external_ids_count: payloads[0]?.dataset_external_ids?.length || 0,
        available_document_external_ids_count: payloads[0]?.available_document_external_ids?.length || 0,
      },
    },
    parserShape: {
      eventNames: events.map((item) => item.event),
      artifactLeakEventNames: artifactLeakEvents.map((item) => item.event),
      errorEventNames: errorEvents.map((item) => item.event),
    },
    artifactLeakGuard: artifactLeakResult,
    artifactLeakSummary,
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

  if (!summary.ok) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
