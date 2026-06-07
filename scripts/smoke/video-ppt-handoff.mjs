#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'https://v3.elepcloud.com';
const DEFAULT_CONNECTION_ID = 'generic-chat-main';
const DEFAULT_SOURCE_ID = 'third-party-source-main';
const DEFAULT_TIMEOUT_MS = 120_000;
const DEFAULT_POLL_INTERVAL_MS = 3_000;
const DEFAULT_OUTPUT_DIR = 'target/video-ppt-handoff-smoke';
const DEFAULT_PROMPT = '请提取这个视频里的 PPT：https://weixin.qq.com/sph/AhfmOtV8P5';
const VALID_MODES = new Set(['self-test', 'main', 'external', 'both']);
const REQUIRED_NEXT_STEPS = [
  'upload_video_file',
  'provide_direct_video_url',
  'request_authorized_capture',
];

function parseArgs(argv) {
  const args = {
    mode: process.env.VIDEO_PPT_HANDOFF_SMOKE_MODE || 'self-test',
    baseUrl: process.env.VIDEO_PPT_HANDOFF_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    prompt: process.env.VIDEO_PPT_HANDOFF_SMOKE_PROMPT || DEFAULT_PROMPT,
    localThreadId:
      process.env.VIDEO_PPT_HANDOFF_SMOKE_LOCAL_THREAD_ID
      || `video-ppt-handoff-${Date.now()}`,
    assistantRunId: process.env.VIDEO_PPT_HANDOFF_SMOKE_ASSISTANT_RUN_ID || '',
    cookie: process.env.VIDEO_PPT_HANDOFF_SMOKE_COOKIE || '',
    bearer: process.env.VIDEO_PPT_HANDOFF_SMOKE_BEARER || '',
    connectionId: process.env.VIDEO_PPT_HANDOFF_SMOKE_CONNECTION_ID || DEFAULT_CONNECTION_ID,
    sourceId: process.env.VIDEO_PPT_HANDOFF_SMOKE_SOURCE_ID || DEFAULT_SOURCE_ID,
    platform: process.env.VIDEO_PPT_HANDOFF_SMOKE_PLATFORM || 'generic_chat',
    tenantExternalId:
      process.env.VIDEO_PPT_HANDOFF_SMOKE_TENANT_EXTERNAL_ID || 'tenant-ext-smoke',
    botExternalId: process.env.VIDEO_PPT_HANDOFF_SMOKE_BOT_EXTERNAL_ID || 'bot-v3',
    senderExternalId:
      process.env.VIDEO_PPT_HANDOFF_SMOKE_SENDER_EXTERNAL_ID || 'user-video-handoff-smoke',
    timeoutMs: Number(process.env.VIDEO_PPT_HANDOFF_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    pollIntervalMs: Number(
      process.env.VIDEO_PPT_HANDOFF_SMOKE_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS,
    ),
    outputDir: process.env.VIDEO_PPT_HANDOFF_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    allowMissingBearer: parseBoolean(process.env.VIDEO_PPT_HANDOFF_SMOKE_ALLOW_MISSING_BEARER),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--mode') {
      args.mode = requireValue(arg, next);
      index += 1;
    } else if (arg === '--base-url') {
      args.baseUrl = requireValue(arg, next);
      index += 1;
    } else if (arg === '--prompt') {
      args.prompt = requireValue(arg, next);
      index += 1;
    } else if (arg === '--local-thread-id') {
      args.localThreadId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--assistant-run-id') {
      args.assistantRunId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--cookie') {
      args.cookie = requireValue(arg, next);
      index += 1;
    } else if (arg === '--bearer') {
      args.bearer = requireValue(arg, next);
      index += 1;
    } else if (arg === '--connection-id') {
      args.connectionId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--source-id') {
      args.sourceId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--tenant-external-id') {
      args.tenantExternalId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--bot-external-id') {
      args.botExternalId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--sender-external-id') {
      args.senderExternalId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--poll-interval-ms') {
      args.pollIntervalMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--allow-missing-bearer') {
      args.allowMissingBearer = true;
    } else if (arg === '--self-test') {
      args.mode = 'self-test';
      args.allowMissingBearer = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (!VALID_MODES.has(args.mode)) {
    throw new Error(`--mode must be one of: ${[...VALID_MODES].join(', ')}`);
  }
  if ((args.mode === 'external' || args.mode === 'both')
    && !args.allowMissingBearer
    && !args.bearer) {
    throw new Error('--bearer is required for external live mode unless --allow-missing-bearer is set');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 10_000) {
    throw new Error('--timeout-ms must be at least 10000');
  }
  if (!Number.isInteger(args.pollIntervalMs) || args.pollIntervalMs < 500) {
    throw new Error('--poll-interval-ms must be at least 500');
  }
  return args;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:video-ppt-handoff -- --self-test

  npm run smoke:video-ppt-handoff -- \\
    --mode main \\
    --base-url https://v3.elepcloud.com \\
    --local-thread-id video-ppt-handoff-20260607-01

  npm run smoke:video-ppt-handoff -- \\
    --mode external \\
    --base-url https://v3.elepcloud.com \\
    --connection-id generic-chat-main \\
    --source-id third-party-source-main \\
    --bearer <token>

Checks:
  - sends or reuses a WeChat Video Channels / login-gated video PPT request
  - verifies wechat_video_login_handoff or an equivalent unsupported-source card
  - requires failure_reason=login_gated_video_source_not_supported
  - requires three next steps: upload video file, provide anonymous direct video URL, request authorized capture
  - rejects success/download signals such as final_pptx_ready or download_exports
  - does not fetch the WeChat URL, download video, extract frames, OCR, or generate PPT

Notes:
  - default mode is self-test and does not call the network
  - main/external/both modes write only lightweight smoke chat/event records
  - external live mode requires an approved inbound bearer unless --allow-missing-bearer is used for local testing
`);
}

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y', 'on'].includes(String(value || '').trim().toLowerCase());
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function mainHeaders(args, accept = 'application/json') {
  const headers = {
    accept,
    'x-ai-data-platform-local-thread-id': args.localThreadId,
  };
  if (accept !== 'application/octet-stream') {
    headers['content-type'] = 'application/json';
  }
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

function externalHeaders(args) {
  const headers = {
    'content-type': 'application/json',
  };
  if (args.bearer) {
    headers.authorization = args.bearer.toLowerCase().startsWith('bearer ')
      ? args.bearer
      : `Bearer ${args.bearer}`;
  }
  return headers;
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

function runIdFromResponse(response) {
  return response?.assistant_run_id || response?.run?.id || null;
}

async function createMainAssistantRun(args) {
  const startedAt = Date.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), args.timeoutMs);
  const result = {
    ok: false,
    httpStatus: null,
    acceptedCount: 0,
    deltaCount: 0,
    completedCount: 0,
    doneCount: 0,
    errorCount: 0,
    assistantRunId: null,
    latencyMs: null,
    events: [],
    error: null,
  };

  try {
    const response = await fetch(`${normalizeBaseUrl(args.baseUrl)}/v1/assistant-runs/stream`, {
      method: 'POST',
      headers: mainHeaders(args, 'text/event-stream'),
      body: JSON.stringify({
        prompt: args.prompt,
        local_thread_id: args.localThreadId,
        messages: [{ role: 'user', content: args.prompt }],
      }),
      signal: controller.signal,
    });
    result.httpStatus = response.status;
    if (!response.ok || !response.body) {
      result.error = `HTTP ${response.status}: ${(await response.text()).slice(0, 240)}`;
      return result;
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    const bufferState = { buffer: '' };
    while (true) {
      const { done, value } = await reader.read();
      const chunkText = decoder.decode(value || new Uint8Array(), { stream: !done });
      for (const frame of parseSseFrames(bufferState, chunkText)) {
        result.events.push({
          event: frame.event,
          sequence: frame.json?.sequence ?? null,
          status: frame.json?.status || frame.json?.data?.status || null,
          phase: frame.json?.phase || null,
        });
        if (frame.event === 'assistant_run.accepted') {
          result.acceptedCount += 1;
        } else if (frame.event === 'assistant_run.delta') {
          result.deltaCount += 1;
        } else if (frame.event === 'assistant_run.completed') {
          result.completedCount += 1;
          result.assistantRunId = runIdFromResponse(responseFromCompleted(frame));
        } else if (frame.event === 'done') {
          result.doneCount += 1;
        } else if (frame.event === 'error') {
          result.errorCount += 1;
          result.error = JSON.stringify(frame.json || frame.data).slice(0, 240);
        }
      }
      if (done) {
        break;
      }
    }
    result.ok = response.status === 200
      && result.acceptedCount === 1
      && result.completedCount === 1
      && result.doneCount === 1
      && result.errorCount === 0
      && Boolean(result.assistantRunId);
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

async function fetchMainArtifacts(args, assistantRunId) {
  const query = new URLSearchParams({
    assistant_run_id: assistantRunId,
    local_thread_id: args.localThreadId,
    limit: '50',
  });
  const result = await requestJson(
    `${normalizeBaseUrl(args.baseUrl)}/api/v3/html-artifacts?${query.toString()}`,
    { method: 'GET', headers: mainHeaders(args) },
    args.timeoutMs,
  );
  if (!result.response.ok) {
    throw new Error(`main artifact list failed HTTP ${result.response.status}: ${result.text.slice(0, 240)}`);
  }
  return result.data;
}

function handoffArtifactFromList(artifacts) {
  return (Array.isArray(artifacts) ? artifacts : []).find((artifact) => (
    (artifact.template_id || artifact.templateId) === 'wechat_video_login_handoff'
    || artifact?.payload?.failure_reason === 'login_gated_video_source_not_supported'
    || artifact?.payload?.failureReason === 'login_gated_video_source_not_supported'
  ));
}

async function waitForMainHandoff(args, assistantRunId) {
  const deadline = Date.now() + args.timeoutMs;
  let attempts = 0;
  let lastArtifacts = [];
  while (Date.now() <= deadline) {
    attempts += 1;
    lastArtifacts = await fetchMainArtifacts(args, assistantRunId);
    const artifact = handoffArtifactFromList(lastArtifacts);
    if (artifact) {
      return {
        ok: true,
        attempts,
        artifact,
        artifactCount: Array.isArray(lastArtifacts) ? lastArtifacts.length : 0,
      };
    }
    await sleep(args.pollIntervalMs);
  }
  return {
    ok: false,
    attempts,
    artifact: null,
    artifactCount: Array.isArray(lastArtifacts) ? lastArtifacts.length : 0,
    error: `main handoff artifact was not found within ${args.timeoutMs}ms`,
  };
}

async function runMainHandoff(args) {
  let createResult = null;
  let assistantRunId = args.assistantRunId.trim();
  if (!assistantRunId) {
    createResult = await createMainAssistantRun(args);
    assistantRunId = createResult.assistantRunId || '';
    if (!createResult.ok) {
      throw new Error(`main assistant run create failed: ${createResult.error || 'missing run id'}`);
    }
  }
  const artifactResult = await waitForMainHandoff(args, assistantRunId);
  if (!artifactResult.ok || !artifactResult.artifact) {
    throw new Error(artifactResult.error || 'main handoff artifact was not found');
  }
  const surface = assertHandoffSurface('main', artifactResult.artifact);
  return {
    ok: surface.ok,
    assistantRunId,
    localThreadId: args.localThreadId,
    createdRun: !args.assistantRunId,
    createOk: createResult?.ok ?? null,
    artifactId: artifactResult.artifact.id || null,
    artifactPollAttempts: artifactResult.attempts,
    artifactCount: artifactResult.artifactCount,
    surface,
  };
}

function buildExternalPayload(args, runId) {
  return {
    platform: args.platform,
    tenant_external_id: args.tenantExternalId,
    bot_external_id: args.botExternalId,
    conversation_external_id: `conv-video-handoff-${runId}`,
    thread_external_id: null,
    sender_external_id: args.senderExternalId,
    sender_display_name: 'DataMax video handoff smoke',
    message_external_id: `msg-video-handoff-${runId}`,
    message_type: 'text',
    text: args.prompt,
    default_prompt: '遇到视频号或登录态视频来源时，只返回受限来源 handoff，不抓视频、不抽帧、不生成 PPT。',
    output_format: 'rich_text',
    render_mode: 'normal',
    requested_skills: [{
      skill_id: 'video_ppt_extraction',
      mode: 'unsupported_source_check',
      arguments: {
        expected_reason: 'login_gated_video_source_not_supported',
      },
    }],
    mention_external_user_ids: [],
    attachment_refs: [],
    idempotency_key: `video-ppt-handoff:event:${runId}`,
    received_at: new Date().toISOString(),
    render_options: null,
  };
}

async function postExternalEvent(args, payload) {
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/events`,
    normalizeBaseUrl(args.baseUrl),
  );
  const result = await requestJson(
    url,
    { method: 'POST', headers: externalHeaders(args), body: JSON.stringify(payload) },
    args.timeoutMs,
  );
  if (!result.response.ok) {
    throw new Error(`external event failed HTTP ${result.response.status}: ${result.text.slice(0, 240)}`);
  }
  return result.data;
}

function replyStatus(reply) {
  return String(reply?.task_status || reply?.card?.status || reply?.reply_type || '').toLowerCase();
}

function replyIsActive(reply) {
  return [
    'accepted',
    'queued',
    'processing',
    'running',
    'video_extraction_queued',
    'video_processing',
  ].includes(replyStatus(reply));
}

async function pollExternalReply(args, assistantRunId) {
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/assistant-runs/${encodeURIComponent(
      assistantRunId,
    )}/reply`,
    normalizeBaseUrl(args.baseUrl),
  );
  const startedAt = Date.now();
  let latest = null;
  while (Date.now() - startedAt <= args.timeoutMs) {
    const result = await requestJson(url, { method: 'GET', headers: externalHeaders(args) }, args.timeoutMs);
    if (!result.response.ok) {
      throw new Error(`external reply poll failed HTTP ${result.response.status}: ${result.text.slice(0, 240)}`);
    }
    latest = result.data;
    if (!replyIsActive(latest?.reply)) {
      return latest;
    }
    await sleep(args.pollIntervalMs);
  }
  throw new Error(`external reply ${assistantRunId} did not complete`);
}

async function sendExternalHandoff(args, runId) {
  const payload = buildExternalPayload(args, runId);
  const initial = await postExternalEvent(args, payload);
  const assistantRunId = initial?.assistant_run_id || null;
  const response = assistantRunId && replyIsActive(initial?.reply)
    ? await pollExternalReply(args, assistantRunId)
    : initial;
  const surface = assertHandoffSurface('external', response);
  return {
    ok: surface.ok,
    connectionId: args.connectionId,
    sourceId: args.sourceId,
    conversationExternalId: payload.conversation_external_id,
    assistantRunId: response?.assistant_run_id || assistantRunId || null,
    replyStatus: response?.reply?.task_status || response?.reply?.card?.status || response?.reply?.reply_type || null,
    surface,
  };
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function compactStrings(values) {
  return values.flatMap((value) => {
    if (value === null || value === undefined) return [];
    if (typeof value === 'string') return value.trim() ? [value.trim()] : [];
    if (typeof value === 'number' || typeof value === 'boolean') return [String(value)];
    return [];
  });
}

function redactedPromptSummary(prompt) {
  const promptText = String(prompt || '');
  const lower = promptText.toLowerCase();
  const shortCodeMatch = promptText.match(/(?:weixin\.qq\.com|channels\.weixin\.qq\.com)\/sph\/([A-Za-z0-9_-]+)/i);
  return {
    mentionsWeChatVideo: lower.includes('weixin.qq.com/sph/')
      || lower.includes('channels.weixin.qq.com/sph/')
      || promptText.includes('视频号'),
    shortCode: shortCodeMatch?.[1] || null,
    wantsSlideOutput: lower.includes('ppt')
      || lower.includes('powerpoint')
      || promptText.includes('课件')
      || promptText.includes('幻灯片'),
  };
}

function mainArtifactSurface(artifact) {
  const payload = artifact?.payload || {};
  const nextStepKeys = asArray(payload.supported_next_steps || payload.supportedNextSteps)
    .map((value) => String(value || ''));
  const nextStepTexts = asArray(payload.supportedNextSteps)
    .flatMap((item) => compactStrings([item?.title, item?.detail]));
  const signalParts = compactStrings([
    artifact?.source_type,
    artifact?.sourceType,
    artifact?.template_id,
    artifact?.templateId,
    artifact?.title,
    payload.status,
    payload.failure_reason,
    payload.failureReason,
    payload.sourcePlatform,
    payload.source_platform,
    payload.blockedReason,
    payload.blocked_reason,
    payload.targetArtifact,
    payload.target_artifact,
    ...nextStepKeys,
    ...nextStepTexts,
  ]);
  return {
    rawKind: 'main_artifact',
    templateId: artifact?.template_id || artifact?.templateId || null,
    sourceType: artifact?.source_type || artifact?.sourceType || null,
    status: payload.status || null,
    failureReason: payload.failure_reason || payload.failureReason || null,
    nextStepKeys,
    artifactLinkCount: 0,
    downloadExportCount: 0,
    signal: signalParts.join('\n'),
  };
}

function externalReplySurface(response) {
  const reply = response?.reply || {};
  const card = reply.card || {};
  const nextStepKeys = [
    ...asArray(card.supported_next_steps),
    ...asArray(card.supportedNextSteps).map((item) => (
      typeof item === 'string' ? item : item?.key || item?.id || item?.value || ''
    )),
    ...asArray(reply.supported_next_steps),
  ].map((value) => String(value || '')).filter(Boolean);
  const nextStepTexts = asArray(card.supportedNextSteps)
    .flatMap((item) => compactStrings([item?.title, item?.detail]));
  const artifactLinks = asArray(reply.artifact_links).filter((value) => typeof value === 'string');
  const downloadExports = [
    ...asArray(card.download_exports),
    ...asArray(card.downloadExports),
  ];
  const signalParts = compactStrings([
    reply.reply_type,
    reply.task_status,
    reply.text,
    card.type,
    card.status,
    card.text,
    card.summary,
    card.title,
    card.failure_reason,
    card.failureReason,
    card.reason,
    card.sourcePlatform,
    card.source_platform,
    card.blockedReason,
    card.blocked_reason,
    ...nextStepKeys,
    ...nextStepTexts,
    ...artifactLinks,
  ]);
  return {
    rawKind: 'external_reply',
    templateId: card.template_id || card.templateId || card.type || null,
    sourceType: card.source_type || card.sourceType || null,
    status: reply.task_status || card.status || reply.reply_type || null,
    failureReason: card.failure_reason || card.failureReason || card.reason || null,
    nextStepKeys,
    artifactLinkCount: artifactLinks.length,
    downloadExportCount: downloadExports.length,
    signal: signalParts.join('\n'),
  };
}

function surfaceFor(kind, value) {
  return kind === 'main' ? mainArtifactSurface(value) : externalReplySurface(value);
}

function hasChineseNextSteps(signal) {
  return /上传视频文件/.test(signal)
    && /(匿名|直接|直连).*视频\s*URL|视频\s*URL/.test(signal)
    && /授权录屏/.test(signal);
}

function assertHandoffSurface(kind, value) {
  const surface = surfaceFor(kind, value);
  const signal = surface.signal;
  const lowerSignal = signal.toLowerCase();
  const hasRequiredReason = lowerSignal.includes('login_gated_video_source_not_supported');
  const hasHandoffType = lowerSignal.includes('wechat_video_login_handoff')
    || lowerSignal.includes('unsupported_login_gated_source')
    || lowerSignal.includes('unsupported_source')
    || hasRequiredReason;
  const hasRequiredNextStepKeys = REQUIRED_NEXT_STEPS.every((step) => surface.nextStepKeys.includes(step));
  const hasActionableNextSteps = hasRequiredNextStepKeys || hasChineseNextSteps(signal);
  const unsafeSuccessSignal = /final_pptx_ready|video_extraction_summary|download_exports|generated_artifacts|frame_extraction|frame_count|ocr_completed/i.test(signal)
    || surface.downloadExportCount > 0;
  const unsafeArtifactLinkSignal = surface.artifactLinkCount > 0;
  const unsafeCredentialRequest = /(请|需要|提供|上传).{0,12}(cookie|账号|密码|二维码|扫码|登录态)/i.test(signal)
    && !/(不会|不要|不需要).{0,12}(cookie|账号|密码|二维码|扫码|登录态)/i.test(signal);
  const rawSourceLeaked = /(?:weixin\.qq\.com|channels\.weixin\.qq\.com)\/sph\//i.test(signal);
  const ok = hasHandoffType
    && hasRequiredReason
    && hasActionableNextSteps
    && !unsafeSuccessSignal
    && !unsafeArtifactLinkSignal
    && !unsafeCredentialRequest
    && !rawSourceLeaked;
  const summary = {
    ok,
    rawKind: surface.rawKind,
    templateId: surface.templateId,
    sourceType: surface.sourceType,
    status: surface.status,
    failureReason: surface.failureReason,
    nextStepKeys: surface.nextStepKeys,
    hasRequiredReason,
    hasHandoffType,
    hasActionableNextSteps,
    unsafeSuccessSignal,
    unsafeArtifactLinkSignal,
    unsafeCredentialRequest,
    rawSourceLeaked,
    artifactLinkCount: surface.artifactLinkCount,
    downloadExportCount: surface.downloadExportCount,
    signalLength: signal.length,
  };
  if (!ok) {
    throw new Error(`${kind} handoff surface validation failed: ${JSON.stringify(summary)}`);
  }
  return summary;
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function validationSummaryFromError(error) {
  const message = error instanceof Error ? error.message : String(error);
  const marker = ' handoff surface validation failed: ';
  const markerIndex = message.indexOf(marker);
  if (markerIndex < 0) {
    return null;
  }
  try {
    return JSON.parse(message.slice(markerIndex + marker.length));
  } catch {
    return null;
  }
}

function assertHandoffSurfaceRejects({ name, kind, value, expected }) {
  try {
    const summary = assertHandoffSurface(kind, value);
    throw new Error(`${name} unexpectedly passed handoff validation: ${JSON.stringify(summary)}`);
  } catch (error) {
    const summary = validationSummaryFromError(error);
    if (!summary) {
      throw error;
    }
    for (const [key, expectedValue] of Object.entries(expected)) {
      if (summary[key] !== expectedValue) {
        throw new Error(
          `${name} rejected for the wrong reason: expected ${key}=${expectedValue}, got ${summary[key]}; summary=${JSON.stringify(summary)}`,
        );
      }
    }
    return {
      name,
      kind,
      rejected: true,
      expected,
      summary,
    };
  }
}

function runNegativeHandoffSurfaceCases(mainArtifact, externalResponse) {
  const missingReasonMain = cloneJson(mainArtifact);
  delete missingReasonMain.payload.failure_reason;

  const successExternal = cloneJson(externalResponse);
  successExternal.reply.task_status = 'video_extraction_summary';
  successExternal.reply.text = '视频 PPT 提取已完成，final_pptx_ready，可下载 PPTX。';
  successExternal.reply.card.type = 'video_extraction_summary';
  successExternal.reply.card.status = 'final_pptx_ready';
  successExternal.reply.card.download_exports = [{ kind: 'pptx', url: '/download/video-slides.pptx' }];

  const artifactLinkExternal = cloneJson(externalResponse);
  artifactLinkExternal.reply.artifact_links = ['artifact://video-ppt-handoff-should-not-expose'];

  const rawSourceMain = cloneJson(mainArtifact);
  rawSourceMain.payload.blockedReason = '当前不抓取 https://weixin.qq.com/sph/AhfmOtV8P5，也不声称已生成 PPT。';

  const credentialRequestMain = cloneJson(mainArtifact);
  credentialRequestMain.payload.supportedNextSteps[2].detail = '请提供 cookie 或二维码登录态后继续处理。';

  return [
    assertHandoffSurfaceRejects({
      name: 'main_missing_failure_reason',
      kind: 'main',
      value: missingReasonMain,
      expected: { hasRequiredReason: false },
    }),
    assertHandoffSurfaceRejects({
      name: 'external_success_download_exports',
      kind: 'external',
      value: successExternal,
      expected: { unsafeSuccessSignal: true },
    }),
    assertHandoffSurfaceRejects({
      name: 'external_artifact_link_leak',
      kind: 'external',
      value: artifactLinkExternal,
      expected: { unsafeArtifactLinkSignal: true },
    }),
    assertHandoffSurfaceRejects({
      name: 'main_raw_source_url_leak',
      kind: 'main',
      value: rawSourceMain,
      expected: { rawSourceLeaked: true },
    }),
    assertHandoffSurfaceRejects({
      name: 'main_credential_request',
      kind: 'main',
      value: credentialRequestMain,
      expected: { unsafeCredentialRequest: true },
    }),
  ];
}

async function runSelfTest(args, runId) {
  const mainArtifact = {
    id: `html-artifact-wechat-video-login-handoff-${runId}`,
    source_type: 'video_extraction',
    template_id: 'wechat_video_login_handoff',
    title: '视频来源受限 · 微信视频号',
    payload: {
      status: 'unsupported_source',
      failure_reason: 'login_gated_video_source_not_supported',
      sourcePlatform: '微信视频号',
      shortCode: 'AhfmOtV8P5',
      targetArtifact: '视频 PPT 提取',
      blockedReason: '当前没有拿到可处理的视频文件，不能声称已经看过视频或已经生成 PPT。',
      supportedNextSteps: [
        {
          title: '上传视频文件',
          detail: '用户或第三方系统上传原始视频文件后，再明确触发提取视频里的 PPT。',
        },
        {
          title: '提供匿名直连视频 URL',
          detail: '第三方系统可先把视频保存到自己的对象存储，再把可下载的视频地址交给 DataMax。',
        },
        {
          title: '申请授权录屏处理',
          detail: '确有授权但拿不到视频文件时，进入 operator 审批的短时录屏兜底流程。',
        },
      ],
      supported_next_steps: REQUIRED_NEXT_STEPS,
    },
  };
  const externalResponse = {
    assistant_run_id: `external-handoff-${runId}`,
    reply: {
      reply_type: 'card',
      task_status: 'login_gated_video_source_not_supported',
      text: '当前不能自动从微信视频号或登录态页面拿到视频文件。请上传视频文件、提供匿名直连视频 URL，或申请授权录屏处理。',
      card: {
        type: 'wechat_video_login_handoff',
        status: 'login_gated_video_source_not_supported',
        failure_reason: 'login_gated_video_source_not_supported',
        supported_next_steps: REQUIRED_NEXT_STEPS,
      },
    },
  };
  const mainSurface = assertHandoffSurface('main', mainArtifact);
  const externalSurface = assertHandoffSurface('external', externalResponse);
  const negativeCases = runNegativeHandoffSurfaceCases(mainArtifact, externalResponse);
  const report = {
    summary: {
      ok: true,
      selfTest: true,
      runId,
      prompt: redactedPromptSummary(args.prompt),
      networkCallsRun: false,
      providerCalled: false,
      reactToolchainCalled: false,
      videoFetchAttempted: false,
      videoDownloaded: false,
      framesExtracted: false,
      ocrRun: false,
      pptGenerated: false,
      finalPptxReadyExposed: false,
      artifactLinksExposed: mainSurface.artifactLinkCount > 0 || externalSurface.artifactLinkCount > 0,
      downloadExportsExposed: mainSurface.downloadExportCount > 0 || externalSurface.downloadExportCount > 0,
      negativeFixtureCount: negativeCases.length,
      negativeFixturesRejected: negativeCases.filter((item) => item.rejected).length,
    },
    main: mainSurface,
    external: externalSurface,
    negativeCases,
  };
  await mkdir(args.outputDir, { recursive: true });
  const reportPath = join(args.outputDir, `${runId}-self-test.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({ ok: true, selfTest: true, runId, reportPath }, null, 2));
}

async function runLive(args, runId) {
  const startedAt = new Date().toISOString();
  const outputDir = join(process.cwd(), args.outputDir, runId);
  await mkdir(outputDir, { recursive: true });
  const result = {
    main: null,
    external: null,
  };
  if (args.mode === 'main' || args.mode === 'both') {
    try {
      result.main = await runMainHandoff(args);
    } catch (error) {
      result.main = {
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }
  if (args.mode === 'external' || args.mode === 'both') {
    try {
      result.external = await sendExternalHandoff(args, runId);
    } catch (error) {
      result.external = {
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }
  const summary = {
    ok: Boolean(result.main?.ok ?? true) && Boolean(result.external?.ok ?? true),
    mode: args.mode,
    baseUrl: args.baseUrl,
    runId,
    prompt: redactedPromptSummary(args.prompt),
    mainOk: result.main?.ok ?? null,
    externalOk: result.external?.ok ?? null,
    startedAt,
    finishedAt: new Date().toISOString(),
  };
  const report = {
    summary,
    main: result.main,
    external: result.external,
  };
  const reportPath = join(outputDir, 'report.json');
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify(summary, null, 2));
  console.log(`report=${reportPath}`);
  if (!summary.ok) {
    process.exitCode = 1;
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const runId = makeRunId();
  if (args.mode === 'self-test') {
    await runSelfTest(args, runId);
  } else {
    await runLive(args, runId);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
