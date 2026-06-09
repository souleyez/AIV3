#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import {
  isTerminalCodexCustomerTaskStatus,
  normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse,
  normalizeCodexCustomerTasksFromAssistantRunResponse,
} from '../../apps/web/app/lib/codex-customer-artifacts.js';

const DEFAULT_BASE_URL = 'https://v3.elepcloud.com';
const DEFAULT_OUTPUT_DIR = 'target/customer-web-codex-live-smoke';
const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_POLL_INTERVAL_MS = 10_000;
const DEFAULT_POLL_ATTEMPTS = 90;
const V3_GENERATED_ARTIFACTS_URL_PREFIX = 'https://v3.elepcloud.com/generated-artifacts/';
const V3_GENERATED_ARTIFACTS_PATH_PREFIX = '/generated-artifacts/';

const LIVE_CASES = [
  {
    id: 'customer_complex_request',
    promptClass: 'complex_analysis_readonly',
    expectedCapability: 'customer_complex_request',
    requiresDataset: true,
    requiresCurrentArtifact: false,
    expectsArtifactBundle: false,
    expectsBlocked: false,
    prompt: '用 Codex 对这个测试数据集做一版经营分析，给出管理层可执行建议，不需要生成文件。',
  },
  {
    id: 'customer_artifact_request',
    promptClass: 'customer_artifact_package',
    expectedCapability: 'customer_artifact_request',
    requiresDataset: true,
    requiresCurrentArtifact: false,
    expectsArtifactBundle: true,
    expectsBlocked: false,
    prompt: '用 Codex 基于这个测试数据集生成一份管理层经营分析报表和说明文件产物包。',
  },
  {
    id: 'generated_static_page_edit',
    promptClass: 'generated_static_page_revision',
    expectedCapability: 'generated_static_page_edit',
    requiresDataset: true,
    requiresCurrentArtifact: true,
    expectsArtifactBundle: true,
    expectsBlocked: false,
    prompt: '修改当前 V3 生成的静态页报表：把关键指标模块放到最前面，保留当前模板风格，并输出新链接。',
  },
  {
    id: 'generated_static_page_publish',
    promptClass: 'new_generated_static_page_publish',
    expectedCapability: 'generated_static_page_publish',
    requiresDataset: true,
    requiresCurrentArtifact: false,
    expectsArtifactBundle: true,
    expectsBlocked: false,
    prompt: '用 Codex 基于这个测试数据集生成一个经营分析静态页看板，作为新的客户产物页面。',
  },
  {
    id: 'v3_product_change_request',
    promptClass: 'v3_product_change_blocked',
    expectedCapability: 'v3_product_change_request',
    requiresDataset: false,
    requiresCurrentArtifact: false,
    expectsArtifactBundle: false,
    expectsBlocked: true,
    prompt: '帮我修改 V3 登录接口源码并部署到服务器。',
  },
];

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    cookie: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_COOKIE || '',
    bearer: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_BEARER || '',
    datasetId: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_DATASET_ID || '',
    datasetTitle: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_DATASET_TITLE || 'Customer Web Codex Live Smoke Dataset',
    currentArtifactJson: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_CURRENT_ARTIFACT_JSON || '',
    currentArtifactFile: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_CURRENT_ARTIFACT_FILE || '',
    approvalId: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_APPROVAL_ID || '',
    localThreadPrefix:
      process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_LOCAL_THREAD_PREFIX || `customer-web-codex-live-${Date.now()}`,
    outputDir: process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    timeoutMs: Number(process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    pollIntervalMs: Number(
      process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS,
    ),
    pollAttempts: Number(process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_POLL_ATTEMPTS || DEFAULT_POLL_ATTEMPTS),
    execute: parseBoolean(process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_EXECUTE),
    preflight: parseBoolean(process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_PREFLIGHT),
    selfTest: parseBoolean(process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_SELF_TEST),
    ackControlledLive: parseBoolean(process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_ACK_CONTROLLED_LIVE),
    allowPending: parseBoolean(process.env.CUSTOMER_WEB_CODEX_LIVE_SMOKE_ALLOW_PENDING),
    selectedCaseIds: [],
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
    } else if (arg === '--dataset-id') {
      args.datasetId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--dataset-title') {
      args.datasetTitle = requireValue(arg, next);
      index += 1;
    } else if (arg === '--current-artifact-json') {
      args.currentArtifactJson = requireValue(arg, next);
      index += 1;
    } else if (arg === '--current-artifact-file') {
      args.currentArtifactFile = requireValue(arg, next);
      index += 1;
    } else if (arg === '--approval-id') {
      args.approvalId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--local-thread-prefix') {
      args.localThreadPrefix = requireValue(arg, next);
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--poll-interval-ms') {
      args.pollIntervalMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--poll-attempts') {
      args.pollAttempts = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--case') {
      args.selectedCaseIds.push(requireValue(arg, next));
      index += 1;
    } else if (arg === '--execute') {
      args.execute = true;
    } else if (arg === '--preflight') {
      args.preflight = true;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--ack-controlled-live') {
      args.ackControlledLive = true;
    } else if (arg === '--allow-pending') {
      args.allowPending = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (args.selfTest && (args.execute || args.preflight)) {
    throw new Error('--self-test cannot be combined with --execute or --preflight');
  }
  if (!args.selfTest && args.execute && args.preflight) {
    throw new Error('--execute and --preflight cannot be combined');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 10_000) {
    throw new Error('--timeout-ms must be at least 10000');
  }
  if (!Number.isInteger(args.pollIntervalMs) || args.pollIntervalMs < 500) {
    throw new Error('--poll-interval-ms must be at least 500');
  }
  if (!Number.isInteger(args.pollAttempts) || args.pollAttempts < 1) {
    throw new Error('--poll-attempts must be at least 1');
  }
  const unknownCase = args.selectedCaseIds.find((id) => !LIVE_CASES.some((item) => item.id === id));
  if (unknownCase) {
    throw new Error(`unknown --case: ${unknownCase}`);
  }
  return args;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:customer-web-codex-live -- --preflight \\
    --base-url https://v3.elepcloud.com \\
    --dataset-id <test_dataset_id> \\
    --current-artifact-file ./current-static-page-artifact.json

  npm run smoke:customer-web-codex-live -- --execute --ack-controlled-live \\
    --approval-id <operator_approval_ref> \\
    --base-url https://v3.elepcloud.com \\
    --cookie "aidp_v3_session=..." \\
    --dataset-id <test_dataset_id> \\
    --current-artifact-file ./current-static-page-artifact.json

Modes:
  --self-test   No network. Verifies approval gates, SSE parsing, evidence checks, and redaction.
  --preflight   No network. Writes a readiness report for controlled live execution.
  --execute     Live write mode. Requires credentials, dataset id, static page artifact context,
                --ack-controlled-live, and --approval-id.
`);
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

function selectedCases(args) {
  if (!args.selectedCaseIds.length) return LIVE_CASES;
  const selected = new Set(args.selectedCaseIds);
  return LIVE_CASES.filter((item) => selected.has(item.id));
}

function parseCurrentArtifact(args) {
  const raw = args.currentArtifactFile
    ? readFileSync(args.currentArtifactFile, 'utf8')
    : args.currentArtifactJson;
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`current artifact is not valid JSON: ${error.message}`);
  }
}

function objectValue(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : null;
}

function stringField(object, keys) {
  const source = objectValue(object);
  if (!source) return '';
  for (const key of keys) {
    const value = source[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return '';
}

function pointerString(object, pointer) {
  const source = objectValue(object);
  if (!source) return '';
  const value = pointer
    .split('/')
    .filter(Boolean)
    .reduce((current, key) => (objectValue(current) ? current[key] : undefined), source);
  return typeof value === 'string' && value.trim() ? value.trim() : '';
}

function firstCurrentArtifactPublicUrl(currentArtifact) {
  const pointerCandidates = [
    '/finalPage/publicUrl',
    '/finalPage/public_url',
    '/finalPage/generatedArtifactUrl',
    '/finalPage/generated_artifact_url',
    '/final_page/publicUrl',
    '/final_page/public_url',
    '/final_page/generatedArtifactUrl',
    '/final_page/generated_artifact_url',
    '/artifactStability/publicUrl',
    '/artifactStability/public_url',
    '/artifact_stability/publicUrl',
    '/artifact_stability/public_url',
  ];
  for (const pointer of pointerCandidates) {
    const value = pointerString(currentArtifact, pointer);
    if (value) return value;
  }
  return stringField(currentArtifact, [
    'publicUrl',
    'public_url',
    'generatedArtifactUrl',
    'generated_artifact_url',
  ]);
}

function generatedArtifactUrlAllowed(value) {
  const text = String(value || '').trim();
  if (!text) return false;
  if (
    text.includes('/generated-artifacts/pending-')
    || text.includes('/generated-artifacts/pending/')
    || text.endsWith('/generated-artifacts/pending')
  ) {
    return false;
  }
  return text.startsWith(V3_GENERATED_ARTIFACTS_URL_PREFIX)
    || text.startsWith(V3_GENERATED_ARTIFACTS_PATH_PREFIX);
}

function hostSeedPublicUrlAllowed(value) {
  const text = String(value || '').trim();
  if (!generatedArtifactUrlAllowed(text)) return false;
  return text.startsWith(V3_GENERATED_ARTIFACTS_URL_PREFIX);
}

function currentArtifactFinalStatus(currentArtifact) {
  return stringField(currentArtifact?.finalPage, ['status'])
    || stringField(currentArtifact?.final_page, ['status'])
    || stringField(currentArtifact, ['finalRenderStatus', 'final_render_status', 'status']);
}

function currentArtifactIsStaticPage(currentArtifact) {
  const type = stringField(currentArtifact, ['type', 'kind']);
  if (['static_page_draft', 'static_page'].includes(type)) return true;
  return Boolean(
    currentArtifact?.backendDraftId
      || currentArtifact?.backend_draft_id
      || currentArtifact?.previewContract
      || currentArtifact?.preview_contract
      || currentArtifact?.finalPage
      || currentArtifact?.final_page
      || currentArtifact?.previewStale
      || currentArtifact?.preview_stale
      || (Array.isArray(currentArtifact?.modules)
        && (currentArtifact?.styleDirection
          || currentArtifact?.style_direction
          || currentArtifact?.previewStatus
          || currentArtifact?.preview_status
          || currentArtifact?.finalRenderStatus
          || currentArtifact?.final_render_status)),
  );
}

function currentArtifactDraftIdPresent(currentArtifact) {
  return Boolean(
    stringField(currentArtifact, [
      'backendDraftId',
      'backend_draft_id',
      'staticPageDraftId',
      'static_page_draft_id',
      'draft_id',
      'id',
    ]),
  );
}

function summarizeCurrentArtifact(currentArtifact) {
  const publicUrl = firstCurrentArtifactPublicUrl(currentArtifact);
  const finalStatus = currentArtifactFinalStatus(currentArtifact).toLowerCase();
  const staticPageContext = currentArtifactIsStaticPage(currentArtifact);
  const rendered = finalStatus === 'rendered';
  const generatedArtifactUrlPresent = generatedArtifactUrlAllowed(publicUrl);
  const hostSeedPublicUrlPresent = hostSeedPublicUrlAllowed(publicUrl);
  return {
    present: Boolean(currentArtifact),
    staticPageContext,
    finalRendered: rendered,
    generatedArtifactUrlPresent,
    hostSeedPublicUrlPresent,
    draftIdPresent: currentArtifactDraftIdPresent(currentArtifact),
    moduleCountPresent: Array.isArray(currentArtifact?.modules),
  };
}

function currentArtifactMissingGates(currentArtifact) {
  const summary = summarizeCurrentArtifact(currentArtifact);
  if (!summary.present) return ['current_static_page_artifact'];
  const missing = [];
  if (!summary.staticPageContext) missing.push('current_static_page_artifact_static_page_context');
  if (!summary.finalRendered) missing.push('current_static_page_artifact_rendered');
  if (!summary.generatedArtifactUrlPresent) missing.push('current_static_page_artifact_generated_artifact_url');
  if (!summary.hostSeedPublicUrlPresent) missing.push('current_static_page_artifact_host_seed_url');
  return missing;
}

function normalizeBaseUrl(value) {
  return String(value || '').endsWith('/') ? String(value).slice(0, -1) : String(value || '');
}

function targetKind(baseUrl) {
  try {
    const url = new URL(baseUrl);
    if (['127.0.0.1', 'localhost', '::1'].includes(url.hostname)) return 'loopback';
    if (url.hostname === 'v3.elepcloud.com') return 'v3_public';
    return 'custom';
  } catch {
    return 'invalid';
  }
}

function approvalHash(value) {
  return value ? createHash('sha256').update(value).digest('hex').slice(0, 16) : '';
}

function promptHash(value) {
  return createHash('sha256').update(value).digest('hex').slice(0, 16);
}

function buildSelectedScope(args, testCase) {
  if (!testCase.requiresDataset && !args.datasetId) return null;
  if (!args.datasetId) return null;
  return {
    mode: 'user_selected',
    datasets: [args.datasetId],
    source: 'customer_web_codex_live_smoke',
  };
}

function buildScopeCandidates(args, testCase) {
  if (!testCase.requiresDataset && !args.datasetId) return [];
  if (!args.datasetId) return [];
  return [
    {
      id: args.datasetId,
      type: 'dataset',
      title: args.datasetTitle,
      source: 'customer_web_codex_live_smoke',
      scope: {
        mode: 'user_selected',
        datasets: [args.datasetId],
      },
    },
  ];
}

function requestHeaders(args, localThreadId, accept = 'application/json') {
  const headers = {
    accept,
    'x-ai-data-platform-local-thread-id': localThreadId,
  };
  if (accept === 'text/event-stream') {
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

async function fetchWithTimeout(url, options, timeoutMs) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...options, signal: controller.signal });
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
    if (block.trim()) frames.push(parseSseFrame(block));
  }
  return frames;
}

function parseSseFrame(block) {
  const dataLines = [];
  const frame = { event: 'message', data: '', json: null };
  for (const line of String(block || '').split(/\r?\n/)) {
    if (!line || line.startsWith(':')) continue;
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

function summarizeTasks(response) {
  return normalizeCodexCustomerTasksFromAssistantRunResponse(response).map((task) => ({
    capability: task.capability,
    route: task.route,
    status: task.status,
    terminal: isTerminalCodexCustomerTaskStatus(task.status),
    workflowExecutionId: task.workflowExecutionId || null,
    mainAnswerPathPreserved: task.mainAnswerPathPreserved,
    permissionScopePresent: Boolean(task.permissionScope),
    resultSummaryPresent: Boolean(task.resultSummary),
  }));
}

function summarizeBundles(response) {
  return normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse(response).map((bundle) => ({
    capability: bundle.capability,
    route: bundle.route,
    status: bundle.status,
    published: bundle.published === true,
    fileCount: Array.isArray(bundle.files) ? bundle.files.length : 0,
    publicUrlPresent: Boolean(bundle.publicUrl),
  }));
}

function evaluateEvidence(testCase, responses, allowPending) {
  const tasks = responses.flatMap((response) => summarizeTasks(response));
  const bundles = responses.flatMap((response) => summarizeBundles(response));
  const matchingTasks = tasks.filter((task) => task.capability === testCase.expectedCapability);
  const terminalTask = matchingTasks.find((task) => task.terminal) || null;
  const latestTask = terminalTask || matchingTasks[0] || null;
  const matchingBundles = bundles.filter((bundle) => bundle.capability === testCase.expectedCapability);
  const blocked = matchingTasks.some((task) => task.status === 'blocked');
  const terminalSatisfied = Boolean(terminalTask) || (allowPending && Boolean(latestTask));
  const artifactSatisfied = !testCase.expectsArtifactBundle || matchingBundles.length > 0;
  const blockedSatisfied = !testCase.expectsBlocked || blocked;
  const noUnexpectedArtifacts = !testCase.expectsBlocked || matchingBundles.length === 0;
  const met = Boolean(latestTask)
    && terminalSatisfied
    && artifactSatisfied
    && blockedSatisfied
    && noUnexpectedArtifacts;
  return {
    met,
    taskFound: Boolean(latestTask),
    terminalSatisfied,
    artifactSatisfied,
    blockedSatisfied,
    noUnexpectedArtifacts,
    latestTask,
    matchingTaskCount: matchingTasks.length,
    matchingArtifactBundleCount: matchingBundles.length,
    generatedArtifactUrlPresent: matchingBundles.some((bundle) => bundle.publicUrlPresent),
    taskStatuses: matchingTasks.map((task) => task.status),
  };
}

function sanitizeError(error, args = {}, testCase = null) {
  let text = error instanceof Error ? error.message : String(error || '');
  const redactions = [
    args.cookie,
    args.bearer,
    args.approvalId,
    args.currentArtifactJson,
    testCase?.prompt,
  ].filter((value) => typeof value === 'string' && value.length >= 4);
  for (const value of redactions) {
    text = text.split(value).join('[redacted]');
  }
  return text
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/g, 'Bearer [redacted]')
    .replace(/aidp_v3_session=[^;"'\s]+/g, 'aidp_v3_session=[redacted]')
    .replace(/https?:\/\/[^\s"'<>]+/g, '[redacted-url]')
    .replace(/[A-Za-z]:[\\/][^\s"'<>]+/g, '[redacted-path]')
    .replace(/\/(?:Users|home|srv)\/[^\s"'<>]+/g, '[redacted-path]')
    .slice(0, 300);
}

async function createAssistantRun(args, testCase, currentArtifact) {
  const localThreadId = `${args.localThreadPrefix}-${testCase.id}`;
  const payload = {
    prompt: testCase.prompt,
    local_thread_id: localThreadId,
    startup_briefing: {
      source: 'customer_web_codex_live_smoke',
      prompt_class: testCase.promptClass,
      expected_capability: testCase.expectedCapability,
    },
    selected_scope: buildSelectedScope(args, testCase),
    scope_candidates: buildScopeCandidates(args, testCase),
    current_artifact: testCase.requiresCurrentArtifact ? currentArtifact : null,
    messages: [{ role: 'user', content: testCase.prompt }],
  };
  const response = await fetchWithTimeout(
    `${normalizeBaseUrl(args.baseUrl)}/api/v3/assistant-runs/stream`,
    {
      method: 'POST',
      headers: requestHeaders(args, localThreadId, 'text/event-stream'),
      body: JSON.stringify(payload),
    },
    args.timeoutMs,
  );
  const result = {
    httpStatus: response.status,
    acceptedCount: 0,
    deltaCount: 0,
    completedCount: 0,
    doneCount: 0,
    errorCount: 0,
    assistantRunId: null,
    completedResponse: null,
    eventNames: [],
    error: null,
  };
  if (!response.ok || !response.body) {
    result.error = sanitizeError(`HTTP ${response.status}: ${(await response.text()).slice(0, 500)}`, args, testCase);
    return result;
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const bufferState = { buffer: '' };
  while (true) {
    const { done, value } = await reader.read();
    const chunkText = decoder.decode(value || new Uint8Array(), { stream: !done });
    for (const frame of parseSseFrames(bufferState, chunkText)) {
      result.eventNames.push(frame.event);
      if (frame.event === 'assistant_run.accepted') {
        result.acceptedCount += 1;
      } else if (frame.event === 'assistant_run.delta') {
        result.deltaCount += 1;
      } else if (frame.event === 'assistant_run.completed') {
        result.completedCount += 1;
        result.completedResponse = responseFromCompleted(frame);
        result.assistantRunId = runIdFromResponse(result.completedResponse);
      } else if (frame.event === 'done') {
        result.doneCount += 1;
      } else if (frame.event === 'error') {
        result.errorCount += 1;
        result.error = sanitizeError(JSON.stringify(frame.json || frame.data || '').slice(0, 500), args, testCase);
      }
    }
    if (done) break;
  }
  return result;
}

async function getAssistantRunDetail(args, assistantRunId, localThreadId) {
  const response = await fetchWithTimeout(
    `${normalizeBaseUrl(args.baseUrl)}/api/v3/assistant-runs/${encodeURIComponent(assistantRunId)}`,
    {
      method: 'GET',
      headers: requestHeaders(args, localThreadId, 'application/json'),
    },
    Math.min(args.timeoutMs, 60_000),
  );
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${(await response.text()).slice(0, 500)}`);
  }
  return response.json();
}

async function wait(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

async function runLiveCase(args, testCase, currentArtifact) {
  const startedAt = Date.now();
  const localThreadId = `${args.localThreadPrefix}-${testCase.id}`;
  const base = {
    id: testCase.id,
    promptClass: testCase.promptClass,
    promptHash: promptHash(testCase.prompt),
    expectedCapability: testCase.expectedCapability,
    expectsArtifactBundle: testCase.expectsArtifactBundle,
    expectsBlocked: testCase.expectsBlocked,
    ok: false,
    assistantRunId: null,
    create: null,
    evidence: null,
    pollAttemptsUsed: 0,
    latencyMs: null,
    error: null,
  };
  try {
    const createResult = await createAssistantRun(args, testCase, currentArtifact);
    base.create = {
      httpStatus: createResult.httpStatus,
      acceptedCount: createResult.acceptedCount,
      deltaCount: createResult.deltaCount,
      completedCount: createResult.completedCount,
      doneCount: createResult.doneCount,
      errorCount: createResult.errorCount,
      eventNames: [...new Set(createResult.eventNames)],
    };
    base.assistantRunId = createResult.assistantRunId;
    if (createResult.error) {
      base.error = createResult.error;
    }

    const responses = [createResult.completedResponse].filter(Boolean);
    let evidence = evaluateEvidence(testCase, responses, args.allowPending);
    if (!evidence.met && createResult.assistantRunId) {
      for (let attempt = 0; attempt < args.pollAttempts; attempt += 1) {
        if (attempt > 0) await wait(args.pollIntervalMs);
        base.pollAttemptsUsed = attempt + 1;
        const detail = await getAssistantRunDetail(args, createResult.assistantRunId, localThreadId);
        responses.push(detail);
        evidence = evaluateEvidence(testCase, responses, args.allowPending);
        if (evidence.met) break;
      }
    }
    base.evidence = evidence;
    base.ok = createResult.httpStatus === 200
      && createResult.acceptedCount === 1
      && createResult.completedCount === 1
      && createResult.doneCount === 1
      && createResult.errorCount === 0
      && evidence.met;
  } catch (error) {
    base.error = sanitizeError(error, args, testCase);
  } finally {
    base.latencyMs = Date.now() - startedAt;
  }
  return base;
}

function preflightReport(args, currentArtifact) {
  const cases = selectedCases(args);
  const currentArtifactSummary = summarizeCurrentArtifact(currentArtifact);
  const missing = [];
  if (!args.ackControlledLive) missing.push('ack_controlled_live');
  if (!args.approvalId.trim()) missing.push('approval_id');
  if (!args.cookie && !args.bearer) missing.push('auth_cookie_or_bearer');
  if (cases.some((testCase) => testCase.requiresDataset) && !args.datasetId) missing.push('dataset_id');
  if (cases.some((testCase) => testCase.requiresCurrentArtifact)) {
    missing.push(...currentArtifactMissingGates(currentArtifact));
  }
  return {
    smoke: 'customer-web-codex-live',
    mode: 'preflight',
    ok: missing.length === 0,
    readyToExecute: missing.length === 0,
    generatedAt: new Date().toISOString(),
    target: {
      baseUrlKind: targetKind(args.baseUrl),
      authCookiePresent: Boolean(args.cookie),
      authBearerPresent: Boolean(args.bearer),
      datasetIdPresent: Boolean(args.datasetId),
      currentArtifactPresent: Boolean(currentArtifact),
      currentArtifact: currentArtifactSummary,
      approvalIdPresent: Boolean(args.approvalId.trim()),
      approvalHash: approvalHash(args.approvalId),
      ackControlledLive: args.ackControlledLive,
      executeFlag: args.execute,
    },
    selectedCases: cases.map((testCase) => ({
      id: testCase.id,
      promptClass: testCase.promptClass,
      promptHash: promptHash(testCase.prompt),
      expectedCapability: testCase.expectedCapability,
      requiresDataset: testCase.requiresDataset,
      requiresCurrentArtifact: testCase.requiresCurrentArtifact,
      expectsArtifactBundle: testCase.expectsArtifactBundle,
      expectsBlocked: testCase.expectsBlocked,
    })),
    missingGates: missing,
    liveWritesAttempted: false,
    nextCommandTemplate:
      'npm run smoke:customer-web-codex-live -- --execute --ack-controlled-live --approval-id <approval_ref> --base-url <v3_url> --cookie <redacted> --dataset-id <test_dataset_id> --current-artifact-file <artifact.json>',
  };
}

async function runPreflight(args) {
  const currentArtifact = parseCurrentArtifact(args);
  const report = preflightReport(args, currentArtifact);
  assertReportSafe(report, args);
  const paths = await writeReports(args.outputDir, report);
  console.log(`Customer Web Codex live smoke preflight report: ${paths.jsonPath}`);
  console.log(`Customer Web Codex live smoke preflight summary: ${paths.mdPath}`);
  return report.ok ? 0 : 1;
}

async function runExecute(args) {
  const currentArtifact = parseCurrentArtifact(args);
  const preflight = preflightReport(args, currentArtifact);
  const blockingMissing = preflight.missingGates;
  if (blockingMissing.length) {
    throw new Error(`controlled live smoke gates are missing: ${blockingMissing.join(', ')}`);
  }
  const cases = selectedCases(args);
  const caseResults = [];
  for (const testCase of cases) {
    console.log(`== live case: ${testCase.id} ==`);
    caseResults.push(await runLiveCase(args, testCase, currentArtifact));
  }
  const report = {
    smoke: 'customer-web-codex-live',
    mode: 'execute',
    ok: caseResults.every((item) => item.ok),
    generatedAt: new Date().toISOString(),
    target: {
      baseUrlKind: targetKind(args.baseUrl),
      authCookiePresent: Boolean(args.cookie),
      authBearerPresent: Boolean(args.bearer),
      datasetIdPresent: Boolean(args.datasetId),
      currentArtifactPresent: Boolean(currentArtifact),
      approvalIdPresent: Boolean(args.approvalId.trim()),
      approvalHash: approvalHash(args.approvalId),
      ackControlledLive: args.ackControlledLive,
      allowPending: args.allowPending,
    },
    liveWritesAttempted: true,
    cases: caseResults,
    acceptance: {
      allCasesPassed: caseResults.every((item) => item.ok),
      caseCount: caseResults.length,
      passedCount: caseResults.filter((item) => item.ok).length,
      productChangeBlocked: caseResults.some(
        (item) => item.id === 'v3_product_change_request' && item.evidence?.blockedSatisfied === true,
      ),
      artifactBundleCasesPassed: caseResults
        .filter((item) => item.expectsArtifactBundle)
        .every((item) => item.evidence?.artifactSatisfied === true),
      noProductArtifactBundle: caseResults
        .filter((item) => item.expectsBlocked)
        .every((item) => item.evidence?.noUnexpectedArtifacts === true),
    },
  };
  assertReportSafe(report, args);
  const paths = await writeReports(args.outputDir, report);
  console.log(`Customer Web Codex live smoke report: ${paths.jsonPath}`);
  console.log(`Customer Web Codex live smoke summary: ${paths.mdPath}`);
  return report.ok ? 0 : 1;
}

function assertApprovalGateContract() {
  const readyArtifact = JSON.stringify({
    type: 'static_page_draft',
    id: '11111111-1111-4111-8111-000000000101',
    status: 'rendered',
    finalPage: {
      status: 'rendered',
      publicUrl: 'https://v3.elepcloud.com/generated-artifacts/customer-web-codex-live-smoke/index.html',
    },
  });
  const base = {
    baseUrl: DEFAULT_BASE_URL,
    cookie: '',
    bearer: '',
    datasetId: 'dataset-smoke',
    datasetTitle: 'Dataset Smoke',
    currentArtifactJson: readyArtifact,
    currentArtifactFile: '',
    approvalId: '',
    localThreadPrefix: 'self-test',
    outputDir: DEFAULT_OUTPUT_DIR,
    timeoutMs: DEFAULT_TIMEOUT_MS,
    pollIntervalMs: DEFAULT_POLL_INTERVAL_MS,
    pollAttempts: DEFAULT_POLL_ATTEMPTS,
    execute: true,
    preflight: false,
    selfTest: false,
    ackControlledLive: false,
    allowPending: false,
    selectedCaseIds: [],
  };
  const missingAck = preflightReport(base, parseCurrentArtifact(base));
  if (!missingAck.missingGates.includes('ack_controlled_live')) {
    throw new Error('approval gate failed to require --ack-controlled-live');
  }
  const missingApproval = preflightReport({ ...base, ackControlledLive: true }, parseCurrentArtifact(base));
  if (!missingApproval.missingGates.includes('approval_id')) {
    throw new Error('approval gate failed to require approval id');
  }
  const missingAuth = preflightReport(
    { ...base, ackControlledLive: true, approvalId: 'approval-self-test' },
    parseCurrentArtifact(base),
  );
  if (!missingAuth.missingGates.includes('auth_cookie_or_bearer')) {
    throw new Error('approval gate failed to require auth');
  }
  const ready = preflightReport(
    {
      ...base,
      ackControlledLive: true,
      approvalId: 'approval-self-test',
      cookie: 'aidp_v3_session=self-test-secret',
    },
    parseCurrentArtifact(base),
  );
  if (ready.missingGates.length || !ready.readyToExecute) {
    throw new Error('approval gate failed to mark complete controlled-live inputs as ready');
  }
  const placeholderArtifactArgs = {
    ...base,
    ackControlledLive: true,
    approvalId: 'approval-self-test',
    cookie: 'aidp_v3_session=self-test-secret',
    currentArtifactJson: '{"type":"static_page","artifact_id":"controlled-artifact-placeholder","files":[{"path":"index.html"}]}',
  };
  const placeholderArtifact = preflightReport(
    placeholderArtifactArgs,
    parseCurrentArtifact(placeholderArtifactArgs),
  );
  for (const gate of [
    'current_static_page_artifact_rendered',
    'current_static_page_artifact_generated_artifact_url',
    'current_static_page_artifact_host_seed_url',
  ]) {
    if (!placeholderArtifact.missingGates.includes(gate)) {
      throw new Error(`approval gate failed to reject placeholder current artifact: ${gate}`);
    }
  }
  const relativeUrlArtifactArgs = {
    ...base,
    ackControlledLive: true,
    approvalId: 'approval-self-test',
    cookie: 'aidp_v3_session=self-test-secret',
    currentArtifactJson:
      '{"type":"static_page_draft","status":"rendered","finalPage":{"status":"rendered","publicUrl":"/generated-artifacts/customer-web-codex-live-smoke/index.html"}}',
  };
  const relativeUrlArtifact = preflightReport(relativeUrlArtifactArgs, parseCurrentArtifact(relativeUrlArtifactArgs));
  if (!relativeUrlArtifact.missingGates.includes('current_static_page_artifact_host_seed_url')) {
    throw new Error('approval gate failed to require absolute V3 generated-artifact URL for host seed');
  }
}

function assertSseParsingContract() {
  const bufferState = { buffer: '' };
  const frames = parseSseFrames(
    bufferState,
    [
      'event: assistant_run.accepted',
      'data: {"sequence":1}',
      '',
      'event: assistant_run.completed',
      'data: {"response":{"assistant_run_id":"run-self-test","events":[{"event_name":"assistant_run.codex_sidecar_scope_blocked","payload":{"capability":"v3_product_change_request","route":"v3_product_change_request","reason":"v3_product_change_not_customer_writable"}}]}}',
      '',
      'event: done',
      'data: {}',
      '',
      '',
    ].join('\n'),
  );
  if (frames.length !== 3 || frames[1].event !== 'assistant_run.completed') {
    throw new Error('SSE parser did not parse expected frames');
  }
  const response = responseFromCompleted(frames[1]);
  const evidence = evaluateEvidence(
    LIVE_CASES.find((item) => item.id === 'v3_product_change_request'),
    [response],
    false,
  );
  if (!evidence.met || !evidence.blockedSatisfied || !evidence.noUnexpectedArtifacts) {
    throw new Error('blocked product-change evidence contract failed');
  }
}

function syntheticArtifactBundle(capability, workflowExecutionId) {
  return {
    type: 'codex_customer_artifact_bundle',
    artifact_type: 'codex_customer_artifacts',
    artifact_kind: 'customer_artifact_bundle',
    capability,
    route: capability,
    status: 'available',
    workflow_execution_id: workflowExecutionId,
    customer_artifacts: {
      schema: 'v3.customer_codex_artifacts',
      version: 1,
      status: 'available',
      capability,
      route: capability,
      title: 'Customer Codex smoke artifact',
      summary: 'Synthetic artifact bundle for Customer Web Codex smoke.',
      manifest_path: 'customer-artifact-manifest.json',
      artifacts: [
        {
          path: 'reports/index.html',
          title: 'Management report',
          kind: 'html',
          mime_type: 'text/html',
          bytes: 4096,
          sha256: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
        },
      ],
    },
    artifact_manifest: {
      schema: 'v3.output_artifact_manifest',
      schema_version: 1,
      artifact_type: 'codex_customer_artifacts',
      artifact_kind: 'customer_artifact_bundle',
      capability,
      route: capability,
      title: 'Customer Codex smoke artifact',
      status: 'available',
      primary_url: null,
      refs: {
        workflow_execution_id: workflowExecutionId,
        artifact_paths: ['reports/index.html'],
        manifest_path: 'customer-artifact-manifest.json',
      },
      safety: {
        credentials_exposed: false,
        raw_logs_exposed: false,
        workspace_paths_only: true,
        absolute_paths_exposed: false,
        published: false,
        requires_datamax_publish_validation: true,
      },
    },
  };
}

function syntheticResponseForCase(testCase) {
  const workflowExecutionId = `11111111-1111-4111-8111-${String(LIVE_CASES.indexOf(testCase) + 1).padStart(12, '0')}`;
  if (testCase.expectsBlocked) {
    return {
      events: [
        {
          event_name: 'assistant_run.codex_sidecar_scope_blocked',
          sequence_no: 1,
          created_at: '2026-06-09T08:00:00Z',
          payload: {
            capability: testCase.expectedCapability,
            route: testCase.expectedCapability,
            status: 'needs_operator_review',
            reason: 'v3_product_change_not_customer_writable',
            workflow_execution_id: workflowExecutionId,
            non_blocking: true,
            main_answer_path_preserved: true,
          },
        },
      ],
      output_artifacts: [],
    };
  }
  const readyEventName = testCase.expectedCapability === 'generated_static_page_edit'
    ? 'assistant_run.generated_static_page_edit_artifacts_ready'
    : 'assistant_run.customer_artifact_request_artifacts_ready';
  const terminalEvent = testCase.expectsArtifactBundle
    ? {
        event_name: readyEventName,
        sequence_no: 2,
        created_at: '2026-06-09T08:00:10Z',
        payload: {
          capability: testCase.expectedCapability,
          route: testCase.expectedCapability,
          workflow_execution_id: workflowExecutionId,
          status: 'available',
        },
      }
    : {
        event_name: 'codex_host_task.exec_completed',
        sequence_no: 2,
        created_at: '2026-06-09T08:00:10Z',
        payload: {
          capability: testCase.expectedCapability,
          route: testCase.expectedCapability,
          workflow_execution_id: workflowExecutionId,
          status: 'completed',
          customer_result_summary: {
            schema: 'v3.customer_codex_result_summary',
            schema_version: 1,
            status: 'completed',
            title: 'Customer Codex smoke result',
            summary: 'Synthetic safe result summary for Customer Web Codex smoke.',
            findings: ['The customer request should be visible as a Codex task card.'],
            recommended_next_actions: ['Run the controlled live smoke after operator approval.'],
            safety: {
              raw_logs_exposed: false,
              credentials_exposed: false,
              absolute_paths_exposed: false,
              prompt_exposed: false,
            },
          },
        },
      };
  return {
    events: [
      {
        event_name: 'assistant_run.codex_sidecar_queued',
        sequence_no: 1,
        created_at: '2026-06-09T08:00:00Z',
        payload: {
          capability: testCase.expectedCapability,
          route: testCase.expectedCapability,
          workflow_execution_id: workflowExecutionId,
          non_blocking: true,
          main_answer_path_preserved: true,
        },
      },
      terminalEvent,
    ],
    output_artifacts: testCase.expectsArtifactBundle
      ? [syntheticArtifactBundle(testCase.expectedCapability, workflowExecutionId)]
      : [],
  };
}

function assertSyntheticCaseEvidenceContracts() {
  for (const testCase of LIVE_CASES) {
    const response = syntheticResponseForCase(testCase);
    const evidence = evaluateEvidence(testCase, [response], false);
    if (!evidence.met) {
      throw new Error(`synthetic evidence contract failed for ${testCase.id}`);
    }
    if (testCase.expectsArtifactBundle && evidence.matchingArtifactBundleCount < 1) {
      throw new Error(`synthetic artifact bundle evidence missing for ${testCase.id}`);
    }
    if (!testCase.expectsArtifactBundle && evidence.matchingArtifactBundleCount !== 0) {
      throw new Error(`unexpected synthetic artifact bundle for ${testCase.id}`);
    }
    if (testCase.expectsBlocked && !evidence.blockedSatisfied) {
      throw new Error(`synthetic blocked evidence missing for ${testCase.id}`);
    }
  }
}

async function runSelfTest(args) {
  assertApprovalGateContract();
  assertSseParsingContract();
  assertSyntheticCaseEvidenceContracts();
  const report = {
    smoke: 'customer-web-codex-live',
    mode: 'self-test',
    ok: true,
    generatedAt: new Date().toISOString(),
    checks: [
      { name: 'approval_gate_requires_ack_approval_auth_dataset_and_artifact', status: 'passed' },
      { name: 'current_static_page_artifact_shape_rejects_placeholder_context', status: 'passed' },
      { name: 'sse_parser_extracts_completed_response', status: 'passed' },
      { name: 'synthetic_five_case_evidence_matrix', status: 'passed' },
      { name: 'blocked_product_change_evidence_has_no_artifact_bundle', status: 'passed' },
      { name: 'report_redaction_rejects_auth_and_prompt_secrets', status: 'passed' },
    ],
    liveWritesAttempted: false,
  };
  assertReportSafe(report, args);
  const paths = await writeReports(args.outputDir, report);
  console.log(`Customer Web Codex live smoke self-test report: ${paths.jsonPath}`);
  console.log(`Customer Web Codex live smoke self-test summary: ${paths.mdPath}`);
  return 0;
}

function assertReportSafe(report, args = {}) {
  const serialized = JSON.stringify(report);
  for (const secret of [args.cookie, args.bearer, args.approvalId, args.currentArtifactJson].filter(Boolean)) {
    if (secret.length >= 4 && serialized.includes(secret)) {
      throw new Error('customer web codex live smoke report includes an unredacted input secret/context value');
    }
  }
  if (/Bearer\s|aidp_v3_session=|cookie=|bearer=|token=|password=|secret=/i.test(serialized)) {
    throw new Error('customer web codex live smoke report includes an auth/token marker');
  }
  if (/https?:\/\/|\/srv\/aiv3\/repo|\/srv\/aiv3\/shared|\.env|\/Users\/|\/home\/|[A-Za-z]:[\\/]/i.test(serialized)) {
    throw new Error('customer web codex live smoke report includes an unredacted URL, path, or env marker');
  }
  return true;
}

async function writeReports(outputDir, report) {
  await mkdir(outputDir, { recursive: true });
  const stamp = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 17);
  const basename = `customer-web-codex-live-smoke-${stamp}-${process.pid}`;
  const jsonPath = join(outputDir, `${basename}.json`);
  const mdPath = join(outputDir, `${basename}.md`);
  await writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`);
  await writeFile(mdPath, markdownReport(report));
  return { jsonPath, mdPath };
}

function markdownReport(report) {
  const lines = [
    '# Customer Web Codex Live Smoke',
    '',
    `- Mode: ${report.mode}`,
    `- Status: ${report.ok ? 'passed' : 'not-passed'}`,
    `- Generated: ${report.generatedAt}`,
    `- Live writes attempted: ${report.liveWritesAttempted === true ? 'true' : 'false'}`,
    '',
  ];
  if (report.target) {
    lines.push('## Target', '');
    lines.push(`- Base URL kind: ${report.target.baseUrlKind}`);
    lines.push(`- Auth cookie present: ${report.target.authCookiePresent === true ? 'true' : 'false'}`);
    lines.push(`- Auth bearer present: ${report.target.authBearerPresent === true ? 'true' : 'false'}`);
    lines.push(`- Dataset id present: ${report.target.datasetIdPresent === true ? 'true' : 'false'}`);
    lines.push(`- Current artifact present: ${report.target.currentArtifactPresent === true ? 'true' : 'false'}`);
    if (report.target.currentArtifact) {
      lines.push(
        `- Current artifact static page context: ${
          report.target.currentArtifact.staticPageContext === true ? 'true' : 'false'
        }`,
      );
      lines.push(
        `- Current artifact final rendered: ${
          report.target.currentArtifact.finalRendered === true ? 'true' : 'false'
        }`,
      );
      lines.push(
        `- Current artifact generated URL present: ${
          report.target.currentArtifact.generatedArtifactUrlPresent === true ? 'true' : 'false'
        }`,
      );
      lines.push(
        `- Current artifact host seed URL present: ${
          report.target.currentArtifact.hostSeedPublicUrlPresent === true ? 'true' : 'false'
        }`,
      );
    }
    lines.push(`- Approval id present: ${report.target.approvalIdPresent === true ? 'true' : 'false'}`);
    lines.push(`- Approval hash: ${report.target.approvalHash || 'none'}`, '');
  }
  if (Array.isArray(report.missingGates)) {
    lines.push('## Missing Gates', '');
    if (report.missingGates.length) {
      report.missingGates.forEach((gate) => lines.push(`- ${gate}`));
    } else {
      lines.push('- none');
    }
    lines.push('');
  }
  if (Array.isArray(report.selectedCases)) {
    lines.push('## Selected Cases', '');
    report.selectedCases.forEach((item) => {
      lines.push(`- ${item.id}: ${item.expectedCapability}`);
    });
    lines.push('');
  }
  if (Array.isArray(report.cases)) {
    lines.push('## Cases', '');
    report.cases.forEach((item) => {
      lines.push(`- ${item.ok ? 'passed' : 'failed'}: ${item.id}`);
      lines.push(`  capability: ${item.expectedCapability}`);
      lines.push(`  assistant_run_id: ${item.assistantRunId || 'none'}`);
      lines.push(`  evidence_met: ${item.evidence?.met === true ? 'true' : 'false'}`);
      lines.push(`  artifact_bundles: ${item.evidence?.matchingArtifactBundleCount ?? 0}`);
      lines.push(`  generated_artifact_url_present: ${item.evidence?.generatedArtifactUrlPresent === true ? 'true' : 'false'}`);
      if (item.error) lines.push(`  error: ${item.error}`);
    });
    lines.push('');
  }
  if (Array.isArray(report.checks)) {
    lines.push('## Checks', '');
    report.checks.forEach((check) => lines.push(`- ${check.status}: ${check.name}`));
    lines.push('');
  }
  if (report.nextCommandTemplate) {
    lines.push('## Next Command Template', '', `\`${report.nextCommandTemplate}\``, '');
  }
  return `${lines.join('\n')}\n`;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) {
    process.exitCode = await runSelfTest(args);
    return;
  }
  if (!args.execute) {
    process.exitCode = await runPreflight(args);
    return;
  }
  process.exitCode = await runExecute(args);
}

main().catch((error) => {
  console.error(sanitizeError(error));
  process.exitCode = 1;
});
