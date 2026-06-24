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
    selfTest: parseBoolean(process.env.MAIN_ASSISTANT_STREAMING_SMOKE_SELF_TEST),
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

Optional:
  --self-test                    run deterministic offline SSE parser and summary checks
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

function buildEmptySseResult(mode) {
  return {
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
}

function recordSseFrame(result, frame, elapsed, deltaTexts, completedResponseRef) {
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
    completedResponseRef.value = responseFromCompleted(frame);
    result.completedResponsePresent = Boolean(completedResponseRef.value);
    result.assistantRunId = runIdFromResponse(completedResponseRef.value);
    result.assistantTextChars = assistantTextFromResponse(completedResponseRef.value).length;
  } else if (frame.event === 'done') {
    result.doneCount += 1;
  } else if (frame.event === 'error') {
    result.errorCount += 1;
    result.error = JSON.stringify(frame.json || frame.data).slice(0, 300);
  }
}

function finalizeSseResult(args, result, completedResponse, deltaTexts, startedAt) {
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
}

async function postSse(args, path, payload, mode) {
  const startedAt = Date.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), args.timeoutMs);
  const url = new URL(path, normalizeBaseUrl(args.baseUrl));
  const result = buildEmptySseResult(mode);

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
    const completedResponseRef = { value: null };

    while (true) {
      const { done, value } = await reader.read();
      const chunkText = decoder.decode(value || new Uint8Array(), { stream: !done });
      for (const frame of parseSseFrames(bufferState, chunkText)) {
        const elapsed = Date.now() - startedAt;
        recordSseFrame(result, frame, elapsed, deltaTexts, completedResponseRef);
      }
      if (done) {
        break;
      }
    }

    return finalizeSseResult(args, result, completedResponseRef.value, deltaTexts, startedAt);
  } catch (error) {
    result.latencyMs = Date.now() - startedAt;
    result.error = error instanceof Error ? error.message : String(error);
    return result;
  } finally {
    clearTimeout(timeout);
  }
}

function buildSseFrame(event, data) {
  return `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
}

function splitFixtureStream(text) {
  const first = Math.max(1, Math.floor(text.length / 5));
  const second = Math.max(first + 1, Math.floor(text.length / 2));
  return [text.slice(0, first), text.slice(first, second), text.slice(second)];
}

function parseFixtureSse(args, mode, chunks) {
  const startedAt = Date.now();
  const result = buildEmptySseResult(mode);
  result.httpStatus = 200;
  result.bodyPrefix = chunks.join('').slice(0, 300);
  const bufferState = { buffer: '' };
  const deltaTexts = [];
  const completedResponseRef = { value: null };
  let elapsed = 0;
  for (const chunk of chunks) {
    elapsed += 25;
    for (const frame of parseSseFrames(bufferState, chunk)) {
      recordSseFrame(result, frame, elapsed, deltaTexts, completedResponseRef);
    }
  }
  if (bufferState.buffer.trim()) {
    result.errorCount += 1;
    result.error = `unparsed SSE buffer: ${bufferState.buffer.slice(0, 120)}`;
  }
  return finalizeSseResult(args, result, completedResponseRef.value, deltaTexts, startedAt);
}

function buildStreamingSummary(args, runId, createResult, continueResult) {
  const summary = {
    runId,
    baseUrl: args.baseUrl,
    ok: createResult.ok && continueResult.ok,
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
  return summary;
}

async function runSelfTest(args) {
  const runId = `${new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14)}-self-test`;
  const fixtureArgs = {
    ...args,
    baseUrl: 'https://doc.elepcloud.com',
    cookie: '',
    bearer: '',
    localThreadId: `main-assistant-streaming-${runId}`,
    requireLiveDelta: true,
    requireMultipleDeltas: true,
  };

  const createText = '新建会话已收到，实时 delta 已出现，最终完成事件已到达。';
  const continueText = '续问流式输出正常，前端可以边收边显示，并且最终全文不会重复追加。';
  const createStream = [
    buildSseFrame('assistant_run.accepted', { status: 'accepted', assistant_run_id: 'run-create-self-test' }),
    buildSseFrame('assistant_run.delta', { sequence: 1, delta: '新建会话已收到，' }),
    buildSseFrame('assistant_run.delta', { sequence: 2, delta: '实时 delta 已出现，' }),
    buildSseFrame('assistant_run.completed', {
      response: {
        assistant_run_id: 'run-create-self-test',
        assistant_message: { content: createText },
      },
    }),
    buildSseFrame('done', { status: 'done' }),
  ].join('');
  const continueStream = [
    buildSseFrame('assistant_run.accepted', { status: 'accepted', assistant_run_id: 'run-continue-self-test' }),
    buildSseFrame('assistant_run.delta', { data: { delta: '续问流式输出正常，' } }),
    buildSseFrame('assistant_run.delta', { data: { delta: '前端可以边收边显示。' } }),
    buildSseFrame('assistant_run.completed', {
      data: {
        response: {
          assistant_run_id: 'run-continue-self-test',
          assistant_message: { content: continueText },
        },
      },
    }),
    buildSseFrame('done', { status: 'done' }),
  ].join('');
  const duplicateText = '最终全文不应作为重复 delta 再追加。';
  const duplicateStream = [
    buildSseFrame('assistant_run.accepted', { status: 'accepted', assistant_run_id: 'run-duplicate-self-test' }),
    buildSseFrame('assistant_run.delta', { delta: duplicateText + duplicateText }),
    buildSseFrame('assistant_run.completed', {
      response: {
        assistant_run_id: 'run-duplicate-self-test',
        assistant_message: { content: duplicateText },
      },
    }),
    buildSseFrame('done', { status: 'done' }),
  ].join('');

  const createResult = parseFixtureSse(fixtureArgs, 'create', splitFixtureStream(createStream));
  const continueResult = parseFixtureSse(fixtureArgs, 'continue', splitFixtureStream(continueStream));
  const duplicateResult = parseFixtureSse(
    { ...fixtureArgs, requireMultipleDeltas: false },
    'duplicate-guard',
    splitFixtureStream(duplicateStream),
  );
  const summary = {
    ...buildStreamingSummary(fixtureArgs, runId, createResult, continueResult),
    selfTest: true,
  };
  const checks = {
    selfTestUsesFixtureBaseUrl: fixtureArgs.baseUrl === 'https://doc.elepcloud.com',
    createAcceptedDeltaCompletedDoneParsed: createResult.acceptedCount === 1
      && createResult.deltaCount === 2
      && createResult.completedCount === 1
      && createResult.doneCount === 1,
    continueAcceptedDeltaCompletedDoneParsed: continueResult.acceptedCount === 1
      && continueResult.deltaCount === 2
      && continueResult.completedCount === 1
      && continueResult.doneCount === 1,
    completedResponseSupportsTopLevelShape: createResult.assistantRunId === 'run-create-self-test'
      && createResult.assistantTextChars === createText.length,
    completedResponseSupportsNestedDataShape: continueResult.assistantRunId === 'run-continue-self-test'
      && continueResult.assistantTextChars === continueText.length,
    splitChunksProduceNoUnparsedBuffer: !createResult.error && !continueResult.error,
    noDuplicateFinalFullTextRequired: createResult.ok && continueResult.ok
      && !createResult.duplicateFinalDeltaLikely
      && !continueResult.duplicateFinalDeltaLikely,
    duplicateFinalFullTextRejected: duplicateResult.duplicateFinalDeltaLikely === true
      && duplicateResult.ok === false,
    summaryRequiresLiveDeltas: summary.ok === true
      && summary.createDeltaCount === 2
      && summary.continueDeltaCount === 2,
  };
  const ok = Object.values(checks).every(Boolean);
  const report = {
    summary: {
      ...summary,
      ok,
      checks,
      generatedAt: new Date().toISOString(),
    },
    results: [createResult, continueResult, duplicateResult],
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
  const summary = buildStreamingSummary(args, runId, createResult, continueResult);
  summary.ok = ok;

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
