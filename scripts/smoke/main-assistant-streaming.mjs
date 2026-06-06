#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_TIMEOUT_MS = 120_000;
const DEFAULT_OUTPUT_DIR = 'target/main-assistant-streaming-smoke';
const DEFAULT_CREATE_PROMPT =
  '请用三点说明 DataMax 主站流式输出 smoke 正在运行，每点一句，分别说明新建会话、实时 delta、最终完成事件。';
const DEFAULT_CONTINUE_PROMPT =
  '继续补充三点，每点一句，说明续问流式输出也正常、不会重复最终全文、前端可以边收边显示。';

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.MAIN_ASSISTANT_STREAMING_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    cookie: process.env.MAIN_ASSISTANT_STREAMING_SMOKE_COOKIE || '',
    bearer: process.env.MAIN_ASSISTANT_STREAMING_SMOKE_BEARER || '',
    timeoutMs: Number(process.env.MAIN_ASSISTANT_STREAMING_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    outputDir: process.env.MAIN_ASSISTANT_STREAMING_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    createPrompt: process.env.MAIN_ASSISTANT_STREAMING_SMOKE_CREATE_PROMPT || DEFAULT_CREATE_PROMPT,
    continuePrompt: process.env.MAIN_ASSISTANT_STREAMING_SMOKE_CONTINUE_PROMPT || DEFAULT_CONTINUE_PROMPT,
    localThreadId:
      process.env.MAIN_ASSISTANT_STREAMING_SMOKE_LOCAL_THREAD_ID || `main-assistant-streaming-${Date.now()}`,
    requireLiveDelta: parseBoolean(process.env.MAIN_ASSISTANT_STREAMING_SMOKE_REQUIRE_LIVE_DELTA),
    requireMultipleDeltas: parseBoolean(process.env.MAIN_ASSISTANT_STREAMING_SMOKE_REQUIRE_MULTIPLE_DELTAS),
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
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--create-prompt') {
      args.createPrompt = requireValue(arg, next);
      index += 1;
    } else if (arg === '--continue-prompt') {
      args.continuePrompt = requireValue(arg, next);
      index += 1;
    } else if (arg === '--local-thread-id') {
      args.localThreadId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--require-live-delta') {
      args.requireLiveDelta = true;
    } else if (arg === '--require-multiple-deltas') {
      args.requireMultipleDeltas = true;
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

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y'].includes(String(value || '').trim().toLowerCase());
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`Usage:
  node scripts/smoke/main-assistant-streaming.mjs \\
    --base-url https://v3.elepcloud.com \\
    --cookie "aidp_v3_session=..."

Checks:
  - POST /v1/assistant-runs/stream emits accepted, delta, completed, done
  - POST /v1/assistant-runs/:id/continue/stream emits accepted, delta, completed, done
  - final completed payload does not require a duplicate full-text delta
`);
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function requestHeaders(args) {
  const headers = {
    accept: 'text/event-stream',
    'content-type': 'application/json',
    'x-ai-data-platform-local-thread-id': args.localThreadId,
  };
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

function parseSseFrames(bufferState, text) {
  bufferState.buffer += text;
  const frames = [];
  let separatorIndex;
  while ((separatorIndex = bufferState.buffer.search(/\r?\n\r?\n/)) >= 0) {
    const block = bufferState.buffer.slice(0, separatorIndex);
    const match = bufferState.buffer.slice(separatorIndex).match(/^\r?\n\r?\n/);
    bufferState.buffer = bufferState.buffer.slice(separatorIndex + (match ? match[0].length : 2));
    if (block.trim()) {
      frames.push(parseSseFrame(block));
    }
  }
  return frames;
}

function parseSseFrame(block) {
  const dataLines = [];
  const frame = { event: 'message', data: '' };
  for (const line of String(block || '').split(/\r?\n/)) {
    if (!line || line.startsWith(':')) {
      continue;
    }
    const separator = line.indexOf(':');
    const field = separator >= 0 ? line.slice(0, separator) : line;
    const rawValue = separator >= 0 ? line.slice(separator + 1) : '';
    const value = rawValue.startsWith(' ') ? rawValue.slice(1) : rawValue;
    if (field === 'event') {
      frame.event = value || 'message';
    } else if (field === 'data') {
      dataLines.push(value);
    }
  }
  frame.data = dataLines.join('\n');
  try {
    frame.json = frame.data ? JSON.parse(frame.data) : null;
  } catch {
    frame.json = null;
  }
  return frame;
}

function responseFromCompleted(frame) {
  return frame?.json?.response || frame?.json?.data?.response || null;
}

function assistantTextFromResponse(response) {
  return String(response?.assistant_message?.content || '').trim();
}

function runIdFromResponse(response) {
  return response?.assistant_run_id || response?.run?.id || null;
}

async function postSse(args, path, payload, mode) {
  const startedAt = Date.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), args.timeoutMs);
  const url = new URL(path, normalizeBaseUrl(args.baseUrl));
  const result = {
    mode,
    ok: false,
    httpStatus: null,
    latencyMs: null,
    acceptedCount: 0,
    deltaCount: 0,
    completedCount: 0,
    doneCount: 0,
    errorCount: 0,
    firstDeltaAtMs: null,
    completedAtMs: null,
    deltaChars: 0,
    assistantRunId: null,
    assistantTextChars: 0,
    completedResponsePresent: false,
    duplicateFinalDeltaLikely: false,
    events: [],
    bodyPrefix: '',
    error: null,
  };

  try {
    const response = await fetch(url, {
      method: 'POST',
      headers: requestHeaders(args),
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
    result.httpStatus = response.status;
    if (!response.ok || !response.body) {
      const text = await response.text();
      result.bodyPrefix = text.slice(0, 300);
      result.error = `HTTP ${response.status}`;
      return result;
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    const bufferState = { buffer: '' };
    const deltaTexts = [];
    let completedResponse = null;

    while (true) {
      const { done, value } = await reader.read();
      const chunkText = decoder.decode(value || new Uint8Array(), { stream: !done });
      for (const frame of parseSseFrames(bufferState, chunkText)) {
        const elapsed = Date.now() - startedAt;
        const eventRecord = {
          event: frame.event,
          atMs: elapsed,
          sequence: frame.json?.sequence ?? null,
          status: frame.json?.status || frame.json?.data?.status || null,
          phase: frame.json?.phase || null,
        };
        result.events.push(eventRecord);
        if (frame.event === 'assistant_run.accepted') {
          result.acceptedCount += 1;
        } else if (frame.event === 'assistant_run.delta') {
          const delta = String(frame.json?.delta || frame.json?.data?.delta || '');
          result.deltaCount += 1;
          result.deltaChars += delta.length;
          deltaTexts.push(delta);
          if (result.firstDeltaAtMs === null) {
            result.firstDeltaAtMs = elapsed;
          }
        } else if (frame.event === 'assistant_run.completed') {
          result.completedCount += 1;
          result.completedAtMs = elapsed;
          completedResponse = responseFromCompleted(frame);
          result.completedResponsePresent = Boolean(completedResponse);
          result.assistantRunId = runIdFromResponse(completedResponse);
          result.assistantTextChars = assistantTextFromResponse(completedResponse).length;
        } else if (frame.event === 'done') {
          result.doneCount += 1;
        } else if (frame.event === 'error') {
          result.errorCount += 1;
          result.error = JSON.stringify(frame.json || frame.data).slice(0, 300);
        }
      }
      if (done) {
        break;
      }
    }

    const assistantText = assistantTextFromResponse(completedResponse);
    const combinedDeltas = deltaTexts.join('');
    result.duplicateFinalDeltaLikely = Boolean(
      assistantText
        && combinedDeltas.length >= assistantText.length * 2
        && combinedDeltas.includes(assistantText + assistantText),
    );
    result.ok = result.httpStatus === 200
      && result.acceptedCount === 1
      && result.deltaCount > 0
      && result.completedCount === 1
      && result.doneCount === 1
      && result.errorCount === 0
      && result.completedResponsePresent
      && !result.duplicateFinalDeltaLikely
      && (!args.requireMultipleDeltas || result.deltaCount > 1);
    result.latencyMs = Date.now() - startedAt;
    return result;
  } catch (error) {
    result.latencyMs = Date.now() - startedAt;
    result.error = error instanceof Error ? error.message : String(error);
    return result;
  } finally {
    clearTimeout(timeout);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const runId = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);

  const createPayload = {
    prompt: args.createPrompt,
    local_thread_id: args.localThreadId,
    messages: [{ role: 'user', content: args.createPrompt }],
  };
  const createResult = await postSse(args, '/v1/assistant-runs/stream', createPayload, 'create');

  let continueResult = {
    mode: 'continue',
    ok: false,
    skipped: true,
    error: 'create run did not produce assistantRunId',
  };
  if (createResult.assistantRunId) {
    continueResult = await postSse(
      args,
      `/v1/assistant-runs/${encodeURIComponent(createResult.assistantRunId)}/continue/stream`,
      {
        prompt: args.continuePrompt,
        max_steps: 3,
        messages: [
          { role: 'user', content: args.createPrompt },
          { role: 'assistant', content: '上一轮回复已生成。' },
          { role: 'user', content: args.continuePrompt },
        ],
      },
      'continue',
    );
  }

  const results = [createResult, continueResult];
  const ok = results.every((item) => item.ok);
  const summary = {
    runId,
    baseUrl: args.baseUrl,
    ok,
    createOk: createResult.ok,
    continueOk: continueResult.ok,
    requireLiveDelta: args.requireLiveDelta,
    requireMultipleDeltas: args.requireMultipleDeltas,
    createDeltaCount: createResult.deltaCount || 0,
    continueDeltaCount: continueResult.deltaCount || 0,
    createFirstDeltaAtMs: createResult.firstDeltaAtMs ?? null,
    continueFirstDeltaAtMs: continueResult.firstDeltaAtMs ?? null,
    createLatencyMs: createResult.latencyMs ?? null,
    continueLatencyMs: continueResult.latencyMs ?? null,
    assistantRunId: createResult.assistantRunId || null,
    generatedAt: new Date().toISOString(),
  };

  if (args.requireLiveDelta) {
    summary.ok = summary.ok && summary.createDeltaCount > 0 && summary.continueDeltaCount > 0;
  }

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
