#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_CONNECTION_ID = 'generic-chat-main';
const DEFAULT_NORMAL_COUNT = 10;
const DEFAULT_STATIC_PAGE_COUNT = 3;
const DEFAULT_RECONNECT_COUNT = 2;
const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_DISCONNECT_AFTER_EVENTS = 1;

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    connectionId: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_CONNECTION_ID || DEFAULT_CONNECTION_ID,
    bearer: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_BEARER || '',
    normalCount: Number(process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_NORMAL_COUNT || DEFAULT_NORMAL_COUNT),
    staticPageCount: Number(process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_STATIC_PAGE_COUNT || DEFAULT_STATIC_PAGE_COUNT),
    reconnectCount: Number(process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_RECONNECT_COUNT || DEFAULT_RECONNECT_COUNT),
    timeoutMs: Number(process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    disconnectAfterEvents: Number(
      process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_DISCONNECT_AFTER_EVENTS || DEFAULT_DISCONNECT_AFTER_EVENTS,
    ),
    sourceId: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_SOURCE_ID || 'third-party-source-main',
    datasetExternalIds: parseList(process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_DATASET_EXTERNAL_IDS),
    documentExternalIds: parseList(process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_DOCUMENT_EXTERNAL_IDS),
    normalText: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_NORMAL_TEXT
      || '请用两句话回答：这是 V3 第三方流式并发 smoke 普通问答。',
    staticPageText: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_STATIC_PAGE_TEXT
      || '请生成一页经营分析静态页报表，若后台仍在处理请返回可继续轮询的状态。',
    defaultPrompt: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_DEFAULT_PROMPT
      || '请面向业务用户，优先基于本轮文档和数据源回答。',
    platform: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_PLATFORM || 'generic_chat',
    tenantExternalId: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_TENANT_EXTERNAL_ID || 'tenant-ext-smoke',
    botExternalId: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_BOT_EXTERNAL_ID || 'bot-v3',
    outputDir: process.env.EXTERNAL_CHANNEL_STREAMING_SMOKE_OUTPUT_DIR
      || 'target/external-channel-streaming-10way-smoke',
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
    } else if (arg === '--normal-count') {
      args.normalCount = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--static-page-count') {
      args.staticPageCount = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--reconnect-count') {
      args.reconnectCount = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--disconnect-after-events') {
      args.disconnectAfterEvents = Number(requireValue(arg, next));
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
    } else if (arg === '--normal-text') {
      args.normalText = requireValue(arg, next);
      index += 1;
    } else if (arg === '--static-page-text') {
      args.staticPageText = requireValue(arg, next);
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

  assertCount(args.normalCount, '--normal-count', 0, 100);
  assertCount(args.staticPageCount, '--static-page-count', 0, 20);
  assertCount(args.reconnectCount, '--reconnect-count', 0, 20);
  assertCount(args.disconnectAfterEvents, '--disconnect-after-events', 1, 20);
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) {
    throw new Error('--timeout-ms must be at least 1000');
  }
  return args;
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

function assertCount(value, name, min, max) {
  if (!Number.isInteger(value) || value < min || value > max) {
    throw new Error(`${name} must be an integer from ${min} to ${max}`);
  }
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:external-channel-streaming-10way -- \\
    --base-url https://v3.elepcloud.com \\
    --connection-id generic-chat-main \\
    --bearer <token>

Checks:
  - 10 simultaneous normal Q&A streams by default
  - 3 simultaneous static-page/report streams by default
  - 2 disconnect + reconnect streams by default
  - no duplicate completed/final assistant messages
  - static-page streams either publish an artifact URL or return continue_polling

Environment aliases:
  EXTERNAL_CHANNEL_STREAMING_SMOKE_BASE_URL
  EXTERNAL_CHANNEL_STREAMING_SMOKE_CONNECTION_ID
  EXTERNAL_CHANNEL_STREAMING_SMOKE_BEARER
  EXTERNAL_CHANNEL_STREAMING_SMOKE_DATASET_EXTERNAL_IDS
  EXTERNAL_CHANNEL_STREAMING_SMOKE_DOCUMENT_EXTERNAL_IDS
`);
}

function buildPayload(args, task, index, runId) {
  const suffix = `${runId}-${task}-${String(index + 1).padStart(2, '0')}`;
  const isStaticPage = task === 'static_page';
  return {
    platform: args.platform,
    tenant_external_id: args.tenantExternalId,
    bot_external_id: args.botExternalId,
    conversation_external_id: `conv-streaming-${suffix}`,
    thread_external_id: null,
    sender_external_id: `user-streaming-${task}-${String(index + 1).padStart(2, '0')}`,
    sender_display_name: 'V3 streaming smoke',
    message_external_id: `msg-streaming-${suffix}`,
    message_type: 'text',
    text: `${isStaticPage ? args.staticPageText : args.normalText} 并发编号 ${index + 1}。`,
    default_prompt: args.defaultPrompt,
    output_format: 'rich_text',
    render_mode: isStaticPage ? 'artifact' : 'normal',
    artifact_type: isStaticPage ? 'static_page' : null,
    available_document_source_id: args.sourceId || null,
    available_document_external_ids: args.documentExternalIds,
    dataset_external_ids: args.datasetExternalIds,
    documentExternalId: null,
    requested_skills: [],
    mention_external_user_ids: [],
    attachment_refs: [],
    idempotency_key: `external-channel-streaming-10way:${suffix}`,
    received_at: new Date().toISOString(),
    render_options: null,
  };
}

function requestHeaders(args, lastEventId = '') {
  const headers = {
    accept: 'text/event-stream',
    'content-type': 'application/json',
  };
  if (args.bearer) {
    headers.authorization = args.bearer.toLowerCase().startsWith('bearer ')
      ? args.bearer
      : `Bearer ${args.bearer}`;
  }
  if (lastEventId) {
    headers['last-event-id'] = lastEventId;
  }
  return headers;
}

async function postStream(args, payload, options = {}) {
  const startedAt = Date.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), args.timeoutMs);
  const streamState = createStreamState();
  const events = [];
  let httpStatus = null;
  let bodyPrefix = '';

  const requestPayload = options.sinceSequence
    ? { ...payload, stream_since_sequence: options.sinceSequence }
    : payload;
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/events/stream`,
    normalizeBaseUrl(args.baseUrl),
  );

  try {
    const response = await fetch(url, {
      method: 'POST',
      headers: requestHeaders(args, options.lastEventId),
      body: JSON.stringify(requestPayload),
      signal: controller.signal,
    });
    httpStatus = response.status;
    if (!response.body) {
      const text = await response.text();
      bodyPrefix = text.slice(0, 500);
      return summarizeStreamResult({
        ok: false,
        disconnected: false,
        httpStatus,
        latencyMs: Date.now() - startedAt,
        events,
        bodyPrefix,
        error: `empty response body: ${response.status}`,
      });
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let structuredEventsSeen = 0;
    while (true) {
      const { value, done } = await reader.read();
      if (done) {
        break;
      }
      const text = decoder.decode(value, { stream: true });
      if (bodyPrefix.length < 500) {
        bodyPrefix = `${bodyPrefix}${text}`.slice(0, 500);
      }
      for (const frame of parseSseFrames(text, streamState)) {
        events.push(frame);
        if (isStructuredEvent(frame)) {
          structuredEventsSeen += 1;
        }
        if (
          options.disconnectAfterStructuredEvents
          && structuredEventsSeen >= options.disconnectAfterStructuredEvents
          && !hasTerminalFrame(events)
        ) {
          await reader.cancel().catch(() => {});
          return summarizeStreamResult({
            ok: response.ok,
            disconnected: true,
            httpStatus,
            latencyMs: Date.now() - startedAt,
            events,
            bodyPrefix,
            error: null,
          });
        }
      }
    }

    for (const frame of flushSseFrames(streamState)) {
      events.push(frame);
    }
    return summarizeStreamResult({
      ok: response.ok,
      disconnected: false,
      httpStatus,
      latencyMs: Date.now() - startedAt,
      events,
      bodyPrefix,
      error: null,
    });
  } catch (error) {
    return summarizeStreamResult({
      ok: false,
      disconnected: false,
      httpStatus,
      latencyMs: Date.now() - startedAt,
      events,
      bodyPrefix,
      error: error instanceof Error ? error.message : String(error),
    });
  } finally {
    clearTimeout(timeout);
  }
}

function createStreamState() {
  return {
    buffer: '',
    eventName: '',
    dataLines: [],
  };
}

function parseSseFrames(text, state) {
  const frames = [];
  state.buffer += text;
  const lines = state.buffer.split(/\r?\n/);
  state.buffer = lines.pop() ?? '';

  for (const line of lines) {
    if (line === '') {
      const frame = flushSseFrame(state);
      if (frame) {
        frames.push(frame);
      }
    } else if (line.startsWith('event:')) {
      state.eventName = line.slice('event:'.length).trim();
    } else if (line.startsWith('data:')) {
      state.dataLines.push(line.slice('data:'.length).trimStart());
    }
  }
  return frames;
}

function flushSseFrames(state) {
  if (state.buffer) {
    for (const line of state.buffer.split(/\r?\n/)) {
      if (line.startsWith('event:')) {
        state.eventName = line.slice('event:'.length).trim();
      } else if (line.startsWith('data:')) {
        state.dataLines.push(line.slice('data:'.length).trimStart());
      }
    }
    state.buffer = '';
  }
  const frame = flushSseFrame(state);
  return frame ? [frame] : [];
}

function flushSseFrame(state) {
  if (!state.eventName && state.dataLines.length === 0) {
    return null;
  }
  const dataText = state.dataLines.join('\n');
  let data = dataText;
  try {
    data = dataText ? JSON.parse(dataText) : null;
  } catch {
    // Keep raw text for diagnostics.
  }
  const frame = { event: state.eventName || 'message', data };
  state.eventName = '';
  state.dataLines = [];
  return frame;
}

function summarizeStreamResult({ ok, disconnected, httpStatus, latencyMs, events, bodyPrefix, error }) {
  const completedEvents = events.filter((frame) => frame.event === 'external_channel.completed');
  const finalResponses = completedEvents.map((frame) => responseFromCompleted(frame.data)).filter(Boolean);
  const finalTexts = finalResponses
    .map((response) => response?.reply?.text)
    .filter((text) => typeof text === 'string' && text.trim());
  const structuredSequences = events
    .map((frame) => sequenceFromFrame(frame))
    .filter((sequence) => Number.isInteger(sequence));
  const artifactUrl = firstArtifactUrl({ events, finalResponses });
  const continuePolling = events.some(isContinuePollingFrame)
    || finalResponses.some((response) => isContinuePollingValue(response));
  const statusUrl = firstStatusUrl({ events, finalResponses });
  const errorFrame = events.find((frame) => frame.event === 'error' || frame.event.endsWith('.error'));
  const terminalFailure = events.some(isTerminalFailureFrame)
    || finalResponses.some((response) => isTerminalFailureValue(response));
  const duplicateFinalMessages = completedEvents.length > 1 || new Set(finalTexts).size > 1;

  return {
    ok,
    disconnected,
    httpStatus,
    latencyMs,
    eventCount: events.length,
    eventNames: events.map((frame) => frame.event),
    started: events.some((frame) => frame.event === 'external_channel.started'),
    done: events.some((frame) => frame.event === 'done'),
    completedCount: completedEvents.length,
    finalResponseCount: finalResponses.length,
    duplicateFinalMessages,
    finalTextLength: finalTexts.join('').length,
    assistantRunId: firstString([
      ...events.map((frame) => valueAtAnyPath(frame.data, ['assistant_run_id', 'assistantRunId'])),
      ...finalResponses.map((response) => response?.assistant_run_id),
    ]),
    lastEventId: firstString([...events].reverse().map((frame) => valueAtAnyPath(frame.data, ['event_id', 'eventId']))),
    lastSequence: structuredSequences.length ? Math.max(...structuredSequences) : 0,
    minSequence: structuredSequences.length ? Math.min(...structuredSequences) : null,
    maxSequence: structuredSequences.length ? Math.max(...structuredSequences) : null,
    statusUrl,
    artifactUrl,
    continuePolling,
    terminalFailure,
    errorFrame: errorFrame?.data || null,
    error,
    bodyPrefix,
    events: events.map((frame) => compactFrame(frame)),
  };
}

async function runNormalTask(args, index, runId) {
  const payload = buildPayload(args, 'normal', index, runId);
  const result = await postStream(args, payload);
  return {
    taskType: 'normal',
    index,
    conversationExternalId: payload.conversation_external_id,
    messageExternalId: payload.message_external_id,
    idempotencyKey: payload.idempotency_key,
    ok: result.ok
      && result.started
      && result.completedCount === 1
      && !result.duplicateFinalMessages
      && !result.terminalFailure
      && !result.errorFrame,
    checks: {
      transportOk: result.ok,
      started: result.started,
      completedOnce: result.completedCount === 1,
      noDuplicateFinalMessages: !result.duplicateFinalMessages,
      noTerminalFailure: !result.terminalFailure,
      noErrorFrame: !result.errorFrame,
    },
    result,
  };
}

async function runStaticPageTask(args, index, runId) {
  const payload = buildPayload(args, 'static_page', index, runId);
  const result = await postStream(args, payload);
  const longTaskProgressOk = Boolean(result.artifactUrl || result.continuePolling);
  return {
    taskType: 'static_page',
    index,
    conversationExternalId: payload.conversation_external_id,
    messageExternalId: payload.message_external_id,
    idempotencyKey: payload.idempotency_key,
    ok: result.ok
      && result.started
      && !result.duplicateFinalMessages
      && !result.terminalFailure
      && !result.errorFrame
      && longTaskProgressOk,
    checks: {
      transportOk: result.ok,
      started: result.started,
      noDuplicateFinalMessages: !result.duplicateFinalMessages,
      noTerminalFailure: !result.terminalFailure,
      noErrorFrame: !result.errorFrame,
      artifactOrContinuePolling: longTaskProgressOk,
    },
    result,
  };
}

async function runReconnectTask(args, index, runId) {
  const payload = buildPayload(args, 'reconnect', index, runId);
  const first = await postStream(args, payload, {
    disconnectAfterStructuredEvents: args.disconnectAfterEvents,
  });
  const second = await postStream(args, payload, {
    sinceSequence: first.lastSequence,
    lastEventId: first.lastEventId,
  });
  const secondSequences = second.events
    .filter((frame) => frame.event !== 'external_channel.started')
    .map((frame) => sequenceFromFrame(frame))
    .filter((sequence) => Number.isInteger(sequence));
  const replayedOnlyAfterLastSequence = secondSequences.every((sequence) => sequence > first.lastSequence);
  const completedCount = first.completedCount + second.completedCount;
  const duplicateFinalMessages = first.duplicateFinalMessages
    || second.duplicateFinalMessages
    || completedCount > 1;

  return {
    taskType: 'reconnect',
    index,
    conversationExternalId: payload.conversation_external_id,
    messageExternalId: payload.message_external_id,
    idempotencyKey: payload.idempotency_key,
    ok: first.disconnected
      && second.ok
      && second.completedCount === 1
      && replayedOnlyAfterLastSequence
      && !duplicateFinalMessages
      && !second.terminalFailure
      && !second.errorFrame,
    checks: {
      firstDisconnected: first.disconnected,
      reconnectTransportOk: second.ok,
      reconnectCompletedOnce: second.completedCount === 1,
      replayedOnlyAfterLastSequence,
      noDuplicateFinalMessages: !duplicateFinalMessages,
      noTerminalFailure: !second.terminalFailure,
      noErrorFrame: !second.errorFrame,
    },
    first,
    second,
  };
}

function responseFromCompleted(data) {
  return data?.data?.response || data?.response || null;
}

function isStructuredEvent(frame) {
  return Boolean(sequenceFromFrame(frame) || valueAtAnyPath(frame.data, ['schema']));
}

function hasTerminalFrame(events) {
  return events.some((frame) => (
    frame.event === 'done'
    || frame.event === 'error'
    || frame.event === 'external_channel.completed'
  ));
}

function sequenceFromFrame(frame) {
  const value = valueAtAnyPath(frame.data, ['sequence']);
  if (value === null || value === undefined || value === '') {
    return null;
  }
  const number = Number(value);
  return Number.isInteger(number) ? number : null;
}

function isContinuePollingFrame(frame) {
  return String(frame.event || '').includes('continue_polling')
    || isContinuePollingValue(frame.data);
}

function isContinuePollingValue(value) {
  const candidates = collectValuesForKeys(value, [
    'status',
    'task_status',
    'taskStatus',
  ]);
  return candidates.some((item) => /continue_polling/i.test(String(item || '')));
}

function isTerminalFailureFrame(frame) {
  return frame.event === 'error'
    || /failed|cancelled|needs_human/i.test(String(frame.event || ''))
    || isTerminalFailureValue(frame.data);
}

function isTerminalFailureValue(value) {
  const candidates = collectValuesForKeys(value, [
    'status',
    'task_status',
    'taskStatus',
    'reply_type',
    'replyType',
    'render_output_status',
    'renderOutputStatus',
  ]);
  return candidates.some((item) => /failed|cancelled|needs_human/i.test(String(item || '')));
}

function firstArtifactUrl({ events, finalResponses }) {
  const values = [
    ...events.map((frame) => frame.data),
    ...finalResponses,
  ];
  for (const value of values) {
    const link = firstExplicitUrl(value, [
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
    if (link) {
      return link;
    }
  }
  return null;
}

function firstStatusUrl({ events, finalResponses }) {
  const values = [
    ...events.map((frame) => frame.data),
    ...finalResponses,
  ];
  for (const value of values) {
    const link = firstExplicitUrl(value, ['status_url', 'statusUrl']);
    if (link) {
      return link;
    }
  }
  return null;
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

function collectValuesForKeys(value, keys, depth = 0) {
  if (!value || depth > 8) {
    return [];
  }
  if (Array.isArray(value)) {
    return value.flatMap((item) => collectValuesForKeys(item, keys, depth + 1));
  }
  if (typeof value !== 'object') {
    return [];
  }
  const own = keys
    .filter((key) => Object.prototype.hasOwnProperty.call(value, key))
    .map((key) => value[key]);
  return [
    ...own,
    ...Object.values(value).flatMap((child) => collectValuesForKeys(child, keys, depth + 1)),
  ];
}

function valueAtAnyPath(value, keys) {
  if (!value || typeof value !== 'object') {
    return null;
  }
  for (const key of keys) {
    if (Object.prototype.hasOwnProperty.call(value, key)) {
      return value[key];
    }
  }
  return null;
}

function firstString(values) {
  return values
    .map((value) => (value === null || value === undefined ? '' : String(value).trim()))
    .find(Boolean) || null;
}

function compactFrame(frame) {
  const data = frame.data && typeof frame.data === 'object' ? frame.data : {};
  const response = responseFromCompleted(data);
  return {
    event: frame.event,
    sequence: sequenceFromFrame(frame),
    eventId: valueAtAnyPath(data, ['event_id', 'eventId']),
    phase: valueAtAnyPath(data, ['phase']),
    status: valueAtAnyPath(data, ['status']),
    displayText: valueAtAnyPath(data, ['display_text', 'displayText']),
    replyType: response?.reply?.reply_type || data?.reply?.reply_type || null,
    taskStatus: response?.reply?.task_status || data?.reply?.task_status || data?.task_status || null,
    artifactUrl: firstExplicitUrl(data, [
      'artifact_links',
      'artifactLinks',
      'public_url',
      'publicUrl',
      'generated_artifact_url',
      'generatedArtifactUrl',
      'artifact_public_url',
      'artifactPublicUrl',
    ]),
    statusUrl: firstExplicitUrl(data, ['status_url', 'statusUrl']),
  };
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
  const tasks = [
    ...Array.from({ length: args.normalCount }, (_, index) => runNormalTask(args, index, runId)),
    ...Array.from({ length: args.staticPageCount }, (_, index) => runStaticPageTask(args, index, runId)),
    ...Array.from({ length: args.reconnectCount }, (_, index) => runReconnectTask(args, index, runId)),
  ];
  const results = await Promise.all(tasks);
  const latencies = results.flatMap((item) => [
    item.result?.latencyMs,
    item.first?.latencyMs,
    item.second?.latencyMs,
  ].filter((value) => Number.isFinite(value)));
  const summary = {
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    normalCount: args.normalCount,
    staticPageCount: args.staticPageCount,
    reconnectCount: args.reconnectCount,
    totalTaskCount: results.length,
    okCount: results.filter((item) => item.ok).length,
    failedCount: results.filter((item) => !item.ok).length,
    normalOkCount: results.filter((item) => item.taskType === 'normal' && item.ok).length,
    staticPageOkCount: results.filter((item) => item.taskType === 'static_page' && item.ok).length,
    reconnectOkCount: results.filter((item) => item.taskType === 'reconnect' && item.ok).length,
    duplicateFinalMessageCount: results.filter((item) =>
      item.result?.duplicateFinalMessages
      || item.first?.duplicateFinalMessages
      || item.second?.duplicateFinalMessages,
    ).length,
    artifactCount: results.filter((item) =>
      item.result?.artifactUrl
      || item.first?.artifactUrl
      || item.second?.artifactUrl,
    ).length,
    continuePollingCount: results.filter((item) =>
      item.result?.continuePolling
      || item.first?.continuePolling
      || item.second?.continuePolling,
    ).length,
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
