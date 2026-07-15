#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_CONNECTION_ID = 'generic-chat-main';
const DEFAULT_SOURCE_ID = 'third-party-source-main';
const DEFAULT_FIXTURE_PATH = 'fixtures/newbai-customer-answer/cases.jsonl';
const DEFAULT_OUTPUT_DIR = 'target/newbai-customer-answer-live-capture';
const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_POLL_INTERVAL_MS = 2_000;
const EVALUATION_CLASS = 'legacy_answer_pattern_diagnostic';
const PROMOTION_BLOCKERS = [
  'answer_pattern_oracles_are_not_promotion_gates',
  'no_per_claim_document_chunk_locator_binding',
  'no_linked_execution_safety_receipt',
];
const ORCHESTRATION_PAYLOAD_FIELDS = [
  'default_prompt',
  'output_format',
  'render_mode',
  'render_options',
  'requested_skills',
];
const ACTIVE_REPLY_STATUSES = new Set(['accepted', 'queued', 'processing', 'running']);

function parseArgs(argv) {
  const args = {
    selfTest: false,
    preflight: false,
    runLive: false,
    ackControlledLive: false,
    evaluate: true,
    requireReady: false,
    includeTemplateGuards: false,
    includeAllCases: false,
    fixturePath: process.env.NEWBAI_CUSTOMER_ANSWER_FIXTURE || DEFAULT_FIXTURE_PATH,
    outputDir: process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_CAPTURE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    baseUrl: process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_BASE_URL || DEFAULT_BASE_URL,
    connectionId: process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_CONNECTION_ID || DEFAULT_CONNECTION_ID,
    bearer: process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_BEARER || '',
    sourceId: process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_SOURCE_ID || DEFAULT_SOURCE_ID,
    datasetExternalIds: parseList(process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_DATASET_EXTERNAL_IDS),
    documentExternalIds: parseList(process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_DOCUMENT_EXTERNAL_IDS),
    tenantExternalId:
      process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_TENANT_EXTERNAL_ID || 'tenant-ext-newbai-customer-smoke',
    botExternalId: process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_BOT_EXTERNAL_ID || 'bot-v3',
    senderExternalId:
      process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_SENDER_EXTERNAL_ID || 'user-newbai-customer-answer-smoke',
    timeoutMs: Number(process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    pollIntervalMs: Number(
      process.env.NEWBAI_CUSTOMER_ANSWER_LIVE_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS,
    ),
    caseIds: [],
    categories: [],
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--preflight') {
      args.preflight = true;
    } else if (arg === '--run-live') {
      args.runLive = true;
    } else if (arg === '--ack-controlled-live') {
      args.ackControlledLive = true;
    } else if (arg === '--skip-evaluate') {
      args.evaluate = false;
    } else if (arg === '--require-ready') {
      args.requireReady = true;
    } else if (arg === '--include-template-guards') {
      args.includeTemplateGuards = true;
    } else if (arg === '--include-all-cases') {
      args.includeAllCases = true;
    } else if (arg === '--fixture') {
      args.fixturePath = requireValue(arg, next);
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--base-url') {
      args.baseUrl = requireValue(arg, next);
      index += 1;
    } else if (arg === '--connection-id') {
      args.connectionId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--bearer') {
      args.bearer = requireValue(arg, next);
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
    } else if (arg === '--case-id') {
      args.caseIds.push(requireValue(arg, next));
      index += 1;
    } else if (arg === '--category') {
      args.categories.push(requireValue(arg, next));
      index += 1;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  const modes = [args.selfTest, args.preflight, args.runLive].filter(Boolean).length;
  if (modes !== 1) {
    throw new Error('exactly one of --self-test, --preflight, or --run-live is required');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) {
    throw new Error('--timeout-ms must be at least 1000');
  }
  if (!Number.isInteger(args.pollIntervalMs) || args.pollIntervalMs < 100) {
    throw new Error('--poll-interval-ms must be at least 100');
  }
  if (args.runLive) {
    if (!args.ackControlledLive) {
      throw new Error('--ack-controlled-live is required with --run-live');
    }
    if (!args.bearer) {
      throw new Error('--bearer or NEWBAI_CUSTOMER_ANSWER_LIVE_BEARER is required with --run-live');
    }
    if (args.datasetExternalIds.length === 0 && args.documentExternalIds.length === 0) {
      throw new Error('--dataset-external-ids or --document-external-ids is required with --run-live');
    }
  }
  return args;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:newbai-customer-answer-live-capture -- --self-test
  npm run smoke:newbai-customer-answer-live-capture -- --preflight --dataset-external-ids <newbai-dataset-id>
  npm run smoke:newbai-customer-answer-live-capture -- --run-live --ack-controlled-live --bearer <token> --dataset-external-ids <newbai-dataset-id>

Default live scope skips template_reuse_guard cases because those prompts can
touch static-page routing. Add --include-template-guards or --case-id explicitly
only for approved controlled runs.
`);
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

async function readJsonl(path) {
  const raw = await readFile(path, 'utf8');
  return raw
    .split(/\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line, index) => {
      try {
        return JSON.parse(line);
      } catch (error) {
        throw new Error(`${path}:${index + 1}: invalid JSONL: ${error.message}`);
      }
    });
}

function selectCases(args, fixtures) {
  const explicitCaseIds = new Set(args.caseIds);
  const categories = new Set(args.categories);
  return fixtures.filter((fixture) => {
    if (explicitCaseIds.size > 0) {
      return explicitCaseIds.has(fixture.case_id);
    }
    if (categories.size > 0) {
      return categories.has(fixture.category);
    }
    if (args.includeAllCases) {
      return true;
    }
    if (fixture.category === 'template_reuse_guard' && !args.includeTemplateGuards) {
      return false;
    }
    return true;
  });
}

function makeRunId() {
  const timestamp = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 17);
  return `${timestamp}-${process.pid}`;
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function sha256(value) {
  return createHash('sha256').update(String(value || '')).digest('hex');
}

function buildPayload(args, fixture, runId) {
  const suffix = `${runId}-${fixture.case_id}`;
  return {
    platform: 'generic_chat',
    tenant_external_id: args.tenantExternalId,
    bot_external_id: args.botExternalId,
    conversation_external_id: `conv-newbai-customer-answer-${suffix}`,
    thread_external_id: null,
    sender_external_id: args.senderExternalId,
    sender_display_name: 'DataMax NewBai customer answer smoke',
    message_external_id: `msg-newbai-customer-answer-${suffix}`,
    message_type: 'text',
    text: fixture.prompt,
    available_document_source_id: args.sourceId || null,
    available_document_external_ids: args.documentExternalIds,
    dataset_external_ids: args.datasetExternalIds,
    documentExternalId: null,
    mention_external_user_ids: [],
    attachment_refs: [],
    idempotency_key: `newbai-customer-answer-live:${suffix}`,
    received_at: new Date().toISOString(),
  };
}

function summarizePayload(fixture, payload) {
  return {
    case_id: fixture.case_id,
    category: fixture.category,
    prompt_sha256: sha256(fixture.prompt),
    prompt_chars: fixture.prompt.length,
    conversation_external_id: payload.conversation_external_id,
    message_external_id: payload.message_external_id,
    dataset_external_ids_count: payload.dataset_external_ids.length,
    document_external_ids_count: payload.available_document_external_ids.length,
    orchestration_field_count: ORCHESTRATION_PAYLOAD_FIELDS.filter((key) =>
      Object.hasOwn(payload, key),
    ).length,
  };
}

function requestHeaders(args) {
  const headers = { 'content-type': 'application/json' };
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

async function postEvent(args, payload) {
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/events`,
    normalizeBaseUrl(args.baseUrl),
  );
  const result = await requestJson(
    url,
    { method: 'POST', headers: requestHeaders(args), body: JSON.stringify(payload) },
    args.timeoutMs,
  );
  if (!result.response.ok) {
    throw new Error(`event failed HTTP ${result.response.status}: ${result.text.slice(0, 300)}`);
  }
  return result.data;
}

async function pollReply(args, assistantRunId) {
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/assistant-runs/${encodeURIComponent(
      assistantRunId,
    )}/reply`,
    normalizeBaseUrl(args.baseUrl),
  );
  const startedAt = Date.now();
  let latest = null;
  while (Date.now() - startedAt <= args.timeoutMs) {
    const result = await requestJson(url, { method: 'GET', headers: requestHeaders(args) }, args.timeoutMs);
    if (!result.response.ok) {
      throw new Error(`reply poll failed HTTP ${result.response.status}: ${result.text.slice(0, 300)}`);
    }
    latest = result.data;
    if (!ACTIVE_REPLY_STATUSES.has(replyStatus(latest?.reply))) {
      return latest;
    }
    await sleep(args.pollIntervalMs);
  }
  throw new Error(`reply ${assistantRunId} did not complete: ${JSON.stringify(latest?.reply || null)}`);
}

async function sendMessage(args, payload) {
  const initial = await postEvent(args, payload);
  const assistantRunId = initial?.assistant_run_id || null;
  if (assistantRunId && ACTIVE_REPLY_STATUSES.has(replyStatus(initial?.reply))) {
    return pollReply(args, assistantRunId);
  }
  return initial;
}

function replyStatus(reply) {
  return String(reply?.task_status || reply?.card?.status || reply?.reply_type || '').toLowerCase();
}

function replyText(response) {
  const reply = response?.reply || {};
  const parts = [
    reply.text,
    reply.answer,
    reply.content,
    reply.card?.text,
    reply.card?.summary,
    reply.card?.answer,
    reply.card?.content,
  ].filter((value) => typeof value === 'string' && value.trim());
  return parts.join('\n').trim();
}

function findObjectsWithKey(value, key, found = []) {
  if (!value || typeof value !== 'object') {
    return found;
  }
  if (Object.prototype.hasOwnProperty.call(value, key)) {
    found.push(value);
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      findObjectsWithKey(item, key, found);
    }
  } else {
    for (const item of Object.values(value)) {
      findObjectsWithKey(item, key, found);
    }
  }
  return found;
}

function collectStrings(value, keys, found = []) {
  if (!value || typeof value !== 'object') {
    return found;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      if (typeof item === 'string') {
        found.push(item);
      } else {
        collectStrings(item, keys, found);
      }
    }
    return found;
  }
  for (const [key, item] of Object.entries(value)) {
    if (keys.includes(key)) {
      if (typeof item === 'string') {
        found.push(item);
      } else if (Array.isArray(item)) {
        collectStrings(item, keys, found);
      }
    } else if (item && typeof item === 'object') {
      collectStrings(item, keys, found);
    }
  }
  return found;
}

function extractArtifactLinks(response) {
  const reply = response?.reply && typeof response.reply === 'object' ? response.reply : {};
  const card = reply?.card && typeof reply.card === 'object' ? reply.card : {};
  const explicitKind = `${reply.reply_type || reply.replyType || ''} ${reply.task_status || reply.taskStatus || ''} ${card.type || ''} ${card.status || ''}`;
  const links = [
    ...collectStrings({
      artifact_links: reply.artifact_links,
      artifactLinks: reply.artifactLinks,
    }, ['artifact_links', 'artifactLinks']),
  ];
  if (/artifact|static[_-]?page|report_render|database-static-pages/i.test(explicitKind)) {
    links.push(...collectStrings(card, [
      'public_url',
      'publicUrl',
      'generated_artifact_url',
      'generatedArtifactUrl',
      'download_url',
      'downloadUrl',
    ]));
  }
  links.push(...collectStrings(response, [
    'generated_artifact_url',
    'generatedArtifactUrl',
  ]));
  return [...new Set(links.filter((value) =>
    (typeof value === 'string')
    && (/^https?:\/\//.test(value) || value.startsWith('/'))
    && (
      /\/generated-artifacts\//i.test(value)
      || /\/database-static-pages\//i.test(value)
      || /artifact|static[_-]?page|report_render|database-static-pages/i.test(explicitKind)
      || (reply.artifact_links || reply.artifactLinks)
    )))];
}

function extractEvidence(response) {
  const evidenceContainers = [
    ...findObjectsWithKey(response, 'evidence'),
    ...findObjectsWithKey(response, 'evidences'),
    ...findObjectsWithKey(response, 'retrieval_evidences'),
    ...findObjectsWithKey(response, 'citations'),
    ...findObjectsWithKey(response, 'sources'),
  ];
  const items = [];
  for (const container of evidenceContainers) {
    for (const key of ['evidence', 'evidences', 'retrieval_evidences', 'citations', 'sources']) {
      const value = container[key];
      if (Array.isArray(value)) {
        items.push(...value);
      } else if (value && typeof value === 'object') {
        items.push(value);
      }
    }
  }
  return items
    .filter((item) => item && typeof item === 'object')
    .map((item) => ({
      type: item.type || item.supply_type || item.supplyType || item.kind || null,
      source: item.source || item.source_locator || item.sourceLocator || item.url || item.id || null,
      text: item.text || item.summary || item.content || item.quote || item.title || '',
    }))
    .filter((item) => item.type || item.source || item.text);
}

function sideEffectsFromResponse(response) {
  const reply = response?.reply || {};
  const card = reply.card || {};
  const text = `${replyText(response)}\n${JSON.stringify({
    reply_type: reply.reply_type || reply.replyType,
    task_status: reply.task_status || reply.taskStatus,
    card_type: card.type,
    card_status: card.status,
  })}`;
  const artifactLinks = extractArtifactLinks(response);
  return {
    static_page_published: /static_page|static-page|generated-artifacts|database-static-pages|artifact_link/i.test(
      `${text}\n${artifactLinks.join('\n')}`,
    ),
    report_generation_enqueued: /static_page|artifact_link|queued|processing|report_render/i.test(text)
      && artifactLinks.length > 0,
    new_template_generated: /新建|重新生成|new template|template_generated/i.test(text),
    html_fallback_used: /html\s*兜底|html_fallback|fallback/i.test(text),
  };
}

function valuesForResponseKey(response, key) {
  return findObjectsWithKey(response, key).map((container) => container[key]);
}

function recursivelyReportsSideEffect(value, insideSideEffectContainer = false) {
  if (!value || typeof value !== 'object') {
    return false;
  }
  if (Array.isArray(value)) {
    return value.some((item) => recursivelyReportsSideEffect(item, insideSideEffectContainer));
  }
  return Object.entries(value).some(([key, nestedValue]) => {
    const normalizedKey = key.replace(/[^a-z0-9]/gi, '').toLowerCase();
    const nextInsideSideEffectContainer = insideSideEffectContainer
      || normalizedKey === 'sideeffects';
    const sideEffectKey = /(sideeffect|trigger|triggered|artifact|publish|enqueue|queued|workflow|job|draft|mutation|mutated|write|written|create|created|start|started|generate|generated|render|rendered|export|exported|update|updated)/i
      .test(normalizedKey);
    if (nestedValue === true && (nextInsideSideEffectContainer || sideEffectKey)) {
      return true;
    }
    const statusLikeKey = normalizedKey === 'status' || normalizedKey.endsWith('status');
    if (
      typeof nestedValue === 'string'
      && nestedValue.trim()
      && (nextInsideSideEffectContainer || statusLikeKey)
      && /(queued|enqueued|processing|started|published|generated|rendered|exported|created|written|mutated)/i
        .test(nestedValue)
    ) {
      return true;
    }
    const executionIdentifierKey = /(execution|jobexecution|workflowexecution)(run)?id$/i
      .test(normalizedKey);
    if (
      executionIdentifierKey
      && ((typeof nestedValue === 'string' && nestedValue.trim()) || Number.isFinite(nestedValue))
    ) {
      return true;
    }
    return recursivelyReportsSideEffect(nestedValue, nextInsideSideEffectContainer);
  });
}

function explicitSideEffectObservation(response) {
  const artifactArrayCounts = valuesForResponseKey(response, 'artifacts')
    .filter(Array.isArray)
    .map((items) => items.length);
  const artifactCounts = [
    ...valuesForResponseKey(response, 'artifact_count'),
    ...valuesForResponseKey(response, 'artifactCount'),
  ]
    .map((value) => Number(value))
    .filter((value) => Number.isFinite(value) && value > 0);
  const reportTriggered = [
    ...valuesForResponseKey(response, 'report_triggered'),
    ...valuesForResponseKey(response, 'reportTriggered'),
  ].some((value) => value === true);
  const sideEffectReported = recursivelyReportsSideEffect(response);
  return {
    artifact_count: Math.max(0, ...artifactArrayCounts, ...artifactCounts),
    report_triggered: reportTriggered,
    side_effect_reported: sideEffectReported,
  };
}

function resultFromResponse(fixture, response) {
  const artifactLinks = extractArtifactLinks(response);
  const explicitObservation = explicitSideEffectObservation(response);
  const artifactCount = Math.max(artifactLinks.length, explicitObservation.artifact_count);
  const artifacts = artifactLinks.map((url) => ({ url }));
  while (artifacts.length < artifactCount) {
    artifacts.push({ detected: true });
  }
  const sideEffects = sideEffectsFromResponse(response);
  sideEffects.explicit_side_effect_reported = explicitObservation.side_effect_reported;
  return {
    case_id: fixture.case_id,
    answer: replyText(response),
    evidence: extractEvidence(response),
    artifacts,
    artifact_count: artifactCount,
    side_effects: sideEffects,
    report_triggered: explicitObservation.report_triggered
      || artifactCount > 0
      || /artifact|static_page|report/i.test(
        `${response?.reply?.reply_type || ''} ${response?.reply?.task_status || ''} ${response?.reply?.card?.type || ''}`,
      ),
  };
}

function hasCapturedSideEffect(item) {
  const artifacts = Array.isArray(item?.artifacts) ? item.artifacts : [];
  const artifactCount = Number(item?.artifact_count || artifacts.length || 0);
  const sideEffects = item?.side_effects && typeof item.side_effects === 'object'
    ? item.side_effects
    : {};
  return item?.report_triggered === true
    || artifactCount > 0
    || recursivelyReportsSideEffect(sideEffects, true);
}

function summarizeCapturedSideEffects(items) {
  const capturedItems = items.filter(hasCapturedSideEffect);
  return {
    detected: capturedItems.length > 0,
    case_count: capturedItems.length,
    artifact_count: capturedItems.reduce((sum, item) => {
      const artifacts = Array.isArray(item?.artifacts) ? item.artifacts : [];
      return sum + Number(item?.artifact_count || artifacts.length || 0);
    }, 0),
  };
}

async function writeJsonl(path, rows) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${rows.map((row) => JSON.stringify(row)).join('\n')}\n`, 'utf8');
}

function evaluatorReceiptIntegrity(receipt, selectedCases) {
  const expectedCaseIds = selectedCases.map((fixture) => fixture.case_id).sort();
  const receiptCases = Array.isArray(receipt?.cases) ? receipt.cases : [];
  const receiptCaseIds = receiptCases.map((item) => item?.case_id).filter(Boolean).sort();
  const caseSetOk = JSON.stringify(receiptCaseIds) === JSON.stringify(expectedCaseIds);
  const failedCaseCount = receiptCases.filter((item) => item?.passed !== true).length;
  const passedCaseCount = receiptCases.filter((item) => item?.passed === true).length;
  const allCasesPassed = receiptCases.length === expectedCaseIds.length
    && receiptCases.every((item) => item?.passed === true);
  const countConsistencyOk = Number(receipt?.failed_case_count) === failedCaseCount
    && Number(receipt?.passed_case_count) === passedCaseCount
    && Number(receipt?.case_count) === receiptCases.length;
  return {
    caseSetOk,
    allCasesPassed,
    countConsistencyOk,
    failedCaseCount,
    passedCaseCount,
  };
}

function runEvaluator(args, resultJsonl, runId, selectedCases) {
  const evaluatorDir = join(args.outputDir, 'evaluator', runId);
  const command = [
    'scripts/smoke/newbai-customer-answer.mjs',
    '--fixture',
    args.fixturePath,
    '--results-jsonl',
    resultJsonl,
    '--selected-case-ids',
    selectedCases.map((fixture) => fixture.case_id).join(','),
    '--output-dir',
    evaluatorDir,
    '--pretty',
  ];
  if (args.requireReady) {
    command.push('--require-diagnostic-match');
  }
  const result = spawnSync(process.execPath, command, {
    cwd: process.cwd(),
    encoding: 'utf8',
  });
  let receipt = null;
  let receiptPath = null;
  let receiptError = null;
  try {
    const receiptNames = readdirSync(evaluatorDir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith('.json'))
      .map((entry) => entry.name);
    if (receiptNames.length !== 1) {
      throw new Error(`expected exactly one evaluator JSON receipt, found ${receiptNames.length}`);
    }
    receiptPath = join(evaluatorDir, receiptNames[0]);
    receipt = JSON.parse(readFileSync(receiptPath, 'utf8'));
  } catch (error) {
    receiptError = error instanceof Error ? error.message : String(error);
  }
  const processOk = result.status === 0 && !result.error;
  const reportOk = receipt?.ok === true;
  const diagnosticMatch = receipt?.diagnostic_match === true;
  const permanentlyNonDecision = receipt?.decision_eligible === false;
  const evaluationClassOk = receipt?.evaluation_class === EVALUATION_CLASS;
  const receiptIntegrity = evaluatorReceiptIntegrity(receipt, selectedCases);
  return {
    run: true,
    ok: processOk
      && reportOk
      && diagnosticMatch
      && permanentlyNonDecision
      && evaluationClassOk
      && receiptIntegrity.caseSetOk
      && receiptIntegrity.allCasesPassed
      && receiptIntegrity.countConsistencyOk,
    status: result.status,
    process_ok: processOk,
    process_error: result.error
      ? result.error instanceof Error ? result.error.message : String(result.error)
      : null,
    report_ok: reportOk,
    diagnostic_match: diagnosticMatch,
    evaluation_class_ok: evaluationClassOk,
    case_set_ok: receiptIntegrity.caseSetOk,
    all_cases_passed: receiptIntegrity.allCasesPassed,
    count_consistency_ok: receiptIntegrity.countConsistencyOk,
    decision_eligible: receipt?.decision_eligible ?? null,
    evaluation_class: receipt?.evaluation_class || null,
    failed_cases: Array.isArray(receipt?.cases)
      ? receipt.cases
        .filter((item) => item?.passed === false)
        .map((item) => ({
          case_id: item.case_id,
          failure_reasons: Array.isArray(item.failure_reasons) ? item.failure_reasons : [],
        }))
      : [],
    receipt_json: receiptPath,
    receipt_error: receiptError,
    command: `${process.execPath} ${command.join(' ')}`,
    stdout: String(result.stdout || '').slice(0, 1000),
    stderr: String(result.stderr || '').slice(0, 1000),
    output_dir: evaluatorDir,
    run_id: runId,
  };
}

function renderMarkdown(report) {
  const lines = [
    '# NewBai Customer Answer Live Capture',
    '',
    `- Diagnostic status: ${report.ok ? 'passed' : 'failed'}`,
    `- Evaluation class: ${report.evaluation_class}`,
    `- Decision eligible: ${report.decision_eligible}`,
    `- Mode: ${report.mode}`,
    `- Run ID: ${report.run_id}`,
    `- Network calls run: ${report.safety.network_calls_run}`,
    `- Provider/live allowed: ${report.safety.provider_live_allowed}`,
    `- Static page publish requested: ${report.safety.static_page_publish_requested}`,
    `- Execution safety evidence: ${report.execution_safety_evidence_status}`,
    `- Result JSONL: ${report.result_jsonl || 'none'}`,
    `- Legacy evaluator: ${report.evaluator?.run ? (report.evaluator.ok ? 'passed' : 'failed') : 'not run'}`,
    `- Evaluator diagnostic match: ${report.evaluator?.run ? report.evaluator.diagnostic_match : 'not run'}`,
    `- Captured template/report side effect: ${report.safety?.captured_side_effect_detected ?? false}`,
    `- Promotion blockers: ${report.promotion_blockers.join(', ')}`,
    '',
    '## Cases',
    '',
    ...report.cases.map((item) =>
      `- ${item.case_id}: ${item.category}, prompt_sha256=${item.prompt_sha256}, status=${item.status || 'planned'}`,
    ),
    '',
  ];
  return `${lines.join('\n')}\n`;
}

function buildSummary(report) {
  const cases = Array.isArray(report.cases) ? report.cases : [];
  const mode = String(report.mode || '');
  const evaluatorRun = report.evaluator?.run === true;
  const evaluatorOk = evaluatorRun ? report.evaluator?.ok === true : mode !== 'live';
  const failedCaseCount = cases.filter((item) => item.status === 'failed').length;
  const capturedCaseCount = cases.filter((item) => item.status === 'captured').length;
  const sampleCaseCount = cases.filter((item) => item.status === 'sample_result').length;
  const plannedCaseCount = cases.filter((item) =>
    item.status === 'planned' || item.status === 'template_guard_requires_explicit_live_approval',
  ).length;
  const capturedSideEffects = summarizeCapturedSideEffects(cases);
  const checks = {
    baseOkPreserved: report.ok === true,
    selectedCasesPresent: Number(report.selected_case_count || 0) > 0
      && cases.length === Number(report.selected_case_count || 0),
    evaluatorPassedWhenRun: evaluatorOk,
    evaluatorRequiredForLiveQuality: mode !== 'live' || evaluatorRun,
    noFailedLiveCases: mode !== 'live' || failedCaseCount === 0,
    liveResultJsonlPresent: mode !== 'live' || Boolean(report.result_jsonl),
    selfTestResultJsonlPresent: mode !== 'self-test' || Boolean(report.result_jsonl),
    preflightDoesNotWriteResults: mode !== 'preflight' || report.result_jsonl === null,
    noNetworkForDryModes: (mode !== 'self-test' && mode !== 'preflight')
      || report.safety?.network_calls_run === false,
    liveNetworkOnlyWhenAllowed: mode !== 'live'
      || (
        report.safety?.network_calls_run === true
        && report.safety?.provider_live_allowed === true
      ),
    noStaticPagePublishRequested: report.safety?.static_page_publish_requested === false,
    noCapturedTemplateOrReportSideEffects: mode !== 'live' || !capturedSideEffects.detected,
    bearerNotIncludedInReport: report.safety?.bearer_included_in_report === false,
    orchestrationFieldsAbsent: cases.every((item) => item.orchestration_field_count === 0),
    permanentlyNonDecision: report.decision_eligible === false
      && report.evaluation_class === EVALUATION_CLASS
      && Array.isArray(report.promotion_blockers)
      && report.promotion_blockers.length === PROMOTION_BLOCKERS.length,
  };
  return {
    ok: Object.values(checks).every(Boolean),
    checks,
    mode,
    run_id: report.run_id,
    selected_case_count: Number(report.selected_case_count || 0),
    total_fixture_case_count: Number(report.total_fixture_case_count || 0),
    case_count: cases.length,
    failed_case_count: failedCaseCount,
    captured_case_count: capturedCaseCount,
    sample_case_count: sampleCaseCount,
    planned_case_count: plannedCaseCount,
    evaluator_run: evaluatorRun,
    evaluator_ok: evaluatorOk,
    result_jsonl_present: Boolean(report.result_jsonl),
    captured_side_effect_case_count: capturedSideEffects.case_count,
    captured_artifact_count: capturedSideEffects.artifact_count,
    evaluation_class: EVALUATION_CLASS,
    decision_eligible: false,
  };
}

async function runSelfTest(args, fixtures, selectedCases, runId) {
  const outputDir = join(process.cwd(), args.outputDir);
  const resultJsonl = join(outputDir, `${runId}.results.jsonl`);
  const rows = selectedCases.map((fixture) => ({ case_id: fixture.case_id, ...fixture.sample_result }));
  await writeJsonl(resultJsonl, rows);
  const evaluator = args.evaluate
    ? runEvaluator({ ...args, requireReady: true }, resultJsonl, runId, selectedCases)
    : { run: false };

  const mixedFixture = fixtures.find((fixture) => fixture.case_id === 'newbai-customer-012');
  assert(mixedFixture, 'self-test requires the mixed database/document fixture');
  const failClosedResultJsonl = join(outputDir, `${runId}.fail-closed.results.jsonl`);
  await writeJsonl(failClosedResultJsonl, [{
    case_id: mixedFixture.case_id,
    answer: mixedFixture.sample_result.answer,
    evidence: [],
    artifacts: [{ url: '/generated-artifacts/self-test' }],
    report_triggered: true,
    side_effects: {
      static_page_published: true,
      report_generation_enqueued: true,
      new_template_generated: false,
      html_fallback_used: false,
    },
  }]);
  const failClosedEvaluator = args.evaluate
    ? runEvaluator(
      { ...args, requireReady: false },
      failClosedResultJsonl,
      `${runId}-fail-closed`,
      [mixedFixture],
    )
    : { run: false };
  const capturedSideEffectSummary = buildSummary({
    ok: true,
    mode: 'live',
    run_id: `${runId}-captured-side-effect`,
    result_jsonl: failClosedResultJsonl,
    selected_case_count: 1,
    total_fixture_case_count: fixtures.length,
    decision_eligible: false,
    evaluation_class: EVALUATION_CLASS,
    promotion_blockers: [...PROMOTION_BLOCKERS],
    evaluator: { run: true, ok: true },
    safety: {
      network_calls_run: true,
      provider_live_allowed: true,
      static_page_publish_requested: false,
      bearer_included_in_report: false,
    },
    cases: [{
      case_id: mixedFixture.case_id,
      category: mixedFixture.category,
      status: 'captured',
      orchestration_field_count: 0,
      artifact_count: 1,
      report_triggered: true,
      side_effects: {
        static_page_published: true,
        report_generation_enqueued: true,
      },
    }],
  });
  const liveWithoutEvaluatorSummary = buildSummary({
    ok: true,
    mode: 'live',
    run_id: `${runId}-missing-evaluator`,
    result_jsonl: failClosedResultJsonl,
    selected_case_count: 1,
    total_fixture_case_count: fixtures.length,
    decision_eligible: false,
    evaluation_class: EVALUATION_CLASS,
    promotion_blockers: [...PROMOTION_BLOCKERS],
    evaluator: { run: false },
    safety: {
      network_calls_run: true,
      provider_live_allowed: true,
      static_page_publish_requested: false,
      bearer_included_in_report: false,
    },
    cases: [{
      case_id: mixedFixture.case_id,
      category: mixedFixture.category,
      status: 'captured',
      orchestration_field_count: 0,
      artifact_count: 0,
      report_triggered: false,
      side_effects: {},
    }],
  });
  const citationOnlyResult = resultFromResponse(mixedFixture, {
    reply: {
      reply_type: 'text',
      text: '普通文档引用',
      card: {
        type: 'document_citation',
        download_url: '/documents/example/download',
      },
    },
    citations: [{ source: '/documents/example/download' }],
  });
  const explicitSideEffectResult = resultFromResponse(mixedFixture, {
    reply: {
      reply_type: 'text',
      text: '看似普通的回答',
    },
    diagnostics: {
      report_triggered: true,
      artifact_count: 1,
      artifacts: [{ kind: 'hidden-test-artifact' }],
      side_effects: { workflow_enqueued: true },
    },
  });
  const topLevelEnqueueResult = resultFromResponse(mixedFixture, {
    reply: { reply_type: 'text', text: '普通回答' },
    workflow_enqueued: true,
  });
  const nestedEnqueueResult = resultFromResponse(mixedFixture, {
    reply: { reply_type: 'text', text: '普通回答' },
    side_effects: { workflow: { enqueued: true } },
  });
  const canonicalGeneratedFlagsResult = resultFromResponse(mixedFixture, {
    reply: { reply_type: 'text', text: '普通回答' },
    diagnostics: {
      new_template_generated: true,
      report_rendered: true,
      artifact_exported: true,
      database_updated: true,
      notification_triggered: true,
    },
  });
  const queuedWorkflowReceiptResult = resultFromResponse(mixedFixture, {
    reply: {
      reply_type: 'card',
      task_status: 'data_ingestion_analysis_queued',
      card: { status: 'processing' },
    },
    codex_host_workflow_execution_id: 'workflow-execution-self-test',
  });
  const contradictoryReceiptIntegrity = evaluatorReceiptIntegrity({
    ok: true,
    diagnostic_match: true,
    decision_eligible: false,
    evaluation_class: EVALUATION_CLASS,
    case_count: 1,
    passed_case_count: 1,
    failed_case_count: 0,
    cases: [{ case_id: mixedFixture.case_id, passed: false }],
  }, [mixedFixture]);
  const postSideEffectFixture = fixtures.find((fixture) => fixture.case_id !== mixedFixture.case_id);
  assert(postSideEffectFixture, 'self-test requires a second fixture for safety-abort coverage');
  let safetyAbortSenderCallCount = 0;
  const safetyAbortCapture = await captureLiveCases(
    args,
    [mixedFixture, postSideEffectFixture],
    `${runId}-safety-abort`,
    async () => {
      safetyAbortSenderCallCount += 1;
      return {
        assistant_run_id: 'self-test-run',
        reply: {
          reply_type: 'artifact_link',
          task_status: 'static_page_published',
          artifact_links: ['/generated-artifacts/self-test/index.html'],
        },
      };
    },
  );
  let uncertainAbortSenderCallCount = 0;
  const uncertainAbortCapture = await captureLiveCases(
    args,
    [mixedFixture, postSideEffectFixture],
    `${runId}-uncertain-abort`,
    async () => {
      uncertainAbortSenderCallCount += 1;
      throw new Error('simulated request timeout after possible server acceptance');
    },
  );
  let nestedSideEffectAbortSenderCallCount = 0;
  const nestedSideEffectAbortCapture = await captureLiveCases(
    args,
    [mixedFixture, postSideEffectFixture],
    `${runId}-nested-side-effect-abort`,
    async () => {
      nestedSideEffectAbortSenderCallCount += 1;
      return {
        reply: { reply_type: 'text', text: '普通回答' },
        side_effects: { workflow: { enqueued: true } },
      };
    },
  );
  const regressionChecks = {
    evaluatorProcessZeroInnerFailureIsNotGreen:
      !args.evaluate
      || (
        failClosedEvaluator.status === 0
        && failClosedEvaluator.process_ok === true
        && failClosedEvaluator.report_ok === false
        && failClosedEvaluator.diagnostic_match === false
        && failClosedEvaluator.decision_eligible === false
        && failClosedEvaluator.receipt_error === null
        && failClosedEvaluator.receipt_json.includes(`${runId}-fail-closed`)
        && failClosedEvaluator.ok === false
        && failClosedEvaluator.failed_cases.some((item) =>
          item.case_id === mixedFixture.case_id
          && item.failure_reasons.includes('missing_required_evidence')
          && item.failure_reasons.includes('template_or_report_side_effect'))
      ),
    capturedArtifactSideEffectIsNotGreen:
      capturedSideEffectSummary.ok === false
      && capturedSideEffectSummary.checks.noCapturedTemplateOrReportSideEffects === false,
    liveWithoutEvaluatorIsNotQualityGreen:
      liveWithoutEvaluatorSummary.ok === false
      && liveWithoutEvaluatorSummary.checks.evaluatorRequiredForLiveQuality === false,
    documentDownloadCitationIsNotArtifact:
      citationOnlyResult.artifacts.length === 0
      && citationOnlyResult.report_triggered === false,
    explicitNestedSideEffectFlagsAreNotMissed:
      explicitSideEffectResult.report_triggered === true
      && explicitSideEffectResult.artifact_count === 1
      && explicitSideEffectResult.artifacts.length === 1
      && explicitSideEffectResult.side_effects.explicit_side_effect_reported === true,
    topLevelAndDeepEnqueueFlagsAreNotMissed:
      topLevelEnqueueResult.side_effects.explicit_side_effect_reported === true
      && nestedEnqueueResult.side_effects.explicit_side_effect_reported === true,
    generatedRenderedExportedAndQueuedReceiptsAreNotMissed:
      canonicalGeneratedFlagsResult.side_effects.explicit_side_effect_reported === true
      && queuedWorkflowReceiptResult.side_effects.explicit_side_effect_reported === true,
    contradictoryEvaluatorReceiptIsNotGreen:
      contradictoryReceiptIntegrity.caseSetOk === true
      && contradictoryReceiptIntegrity.allCasesPassed === false
      && contradictoryReceiptIntegrity.countConsistencyOk === false,
    capturedSideEffectStopsLaterProviderCalls:
      safetyAbortSenderCallCount === 1
      && safetyAbortCapture.rows.length === 1
      && safetyAbortCapture.safetyAborted === true
      && safetyAbortCapture.cases.some((item) =>
        item.case_id === postSideEffectFixture.case_id
        && item.status === 'safety_aborted'),
    requestErrorStopsLaterProviderCalls:
      uncertainAbortSenderCallCount === 1
      && uncertainAbortCapture.rows.length === 1
      && uncertainAbortCapture.safetyAborted === true
      && uncertainAbortCapture.safetyAbortReason === 'request_error_uncertain_side_effect'
      && uncertainAbortCapture.cases.some((item) =>
        item.case_id === postSideEffectFixture.case_id
        && item.status === 'safety_aborted_uncertain_side_effect'),
    nestedSideEffectStopsLaterProviderCalls:
      nestedSideEffectAbortSenderCallCount === 1
      && nestedSideEffectAbortCapture.rows.length === 1
      && nestedSideEffectAbortCapture.safetyAborted === true
      && nestedSideEffectAbortCapture.cases.some((item) =>
        item.case_id === postSideEffectFixture.case_id
        && item.status === 'safety_aborted'),
  };
  const failedRegressionChecks = Object.entries(regressionChecks)
    .filter(([, passed]) => !passed)
    .map(([name]) => name);
  assert.deepEqual(
    failedRegressionChecks,
    [],
    `live capture fail-closed regressions failed: ${failedRegressionChecks.join(', ')}`,
  );

  const report = {
    ok: !evaluator.run || evaluator.ok,
    mode: 'self-test',
    run_id: runId,
    fixture_path: args.fixturePath,
    result_jsonl: resultJsonl,
    selected_case_count: selectedCases.length,
    total_fixture_case_count: fixtures.length,
    self_test: {
      checks: regressionChecks,
      fail_closed_evaluator: args.evaluate ? {
        process_ok: failClosedEvaluator.process_ok,
        report_ok: failClosedEvaluator.report_ok,
        diagnostic_match: failClosedEvaluator.diagnostic_match,
        decision_eligible: failClosedEvaluator.decision_eligible,
        ok: failClosedEvaluator.ok,
        failed_cases: failClosedEvaluator.failed_cases,
        receipt_json: failClosedEvaluator.receipt_json,
        receipt_error: failClosedEvaluator.receipt_error,
      } : { run: false },
      captured_side_effect: {
        ok: capturedSideEffectSummary.ok,
        case_count: capturedSideEffectSummary.captured_side_effect_case_count,
        artifact_count: capturedSideEffectSummary.captured_artifact_count,
        no_captured_template_or_report_side_effects:
          capturedSideEffectSummary.checks.noCapturedTemplateOrReportSideEffects,
      },
    },
    safety: {
      network_calls_run: false,
      provider_live_allowed: false,
      provider_call_status: 'not_run',
      mutation_status: 'not_run',
      linked_execution_safety_receipt: false,
      static_page_publish_requested: false,
      bearer_included_in_report: false,
    },
    evaluator,
    cases: selectedCases.map((fixture) => ({
      case_id: fixture.case_id,
      category: fixture.category,
      prompt_sha256: sha256(fixture.prompt),
      orchestration_field_count: 0,
      status: 'sample_result',
    })),
  };
  return writeReport(args, report, runId);
}

async function runPreflight(args, fixtures, selectedCases, runId) {
  const payloads = selectedCases.map((fixture) => buildPayload(args, fixture, runId));
  const report = {
    ok: true,
    mode: 'preflight',
    run_id: runId,
    fixture_path: args.fixturePath,
    result_jsonl: null,
    selected_case_count: selectedCases.length,
    total_fixture_case_count: fixtures.length,
    base_url: args.baseUrl,
    connection_id: args.connectionId,
    dataset_external_ids_count: args.datasetExternalIds.length,
    document_external_ids_count: args.documentExternalIds.length,
    include_template_guards: args.includeTemplateGuards,
    include_all_cases: args.includeAllCases,
    safety: {
      network_calls_run: false,
      provider_live_allowed: false,
      provider_call_status: 'not_run',
      mutation_status: 'not_run',
      linked_execution_safety_receipt: false,
      live_write_approval_required: true,
      static_page_publish_requested: false,
      bearer_included_in_report: false,
    },
    evaluator: { run: false },
    cases: selectedCases.map((fixture, index) => ({
      ...summarizePayload(fixture, payloads[index]),
      status: fixture.category === 'template_reuse_guard' ? 'template_guard_requires_explicit_live_approval' : 'planned',
    })),
  };
  return writeReport(args, report, runId);
}

async function captureLiveCases(args, selectedCases, runId, sender = sendMessage) {
  const rows = [];
  const cases = [];
  let safetyAborted = false;
  let safetyAbortReason = null;
  for (const fixture of selectedCases) {
    const payload = buildPayload(args, fixture, runId);
    const startedAt = Date.now();
    try {
      const response = await sender(args, payload);
      const row = resultFromResponse(fixture, response);
      rows.push(row);
      cases.push({
        ...summarizePayload(fixture, payload),
        status: 'captured',
        latency_ms: Date.now() - startedAt,
        assistant_run_id_present: Boolean(response?.assistant_run_id),
        answer_chars: row.answer.length,
        evidence_count: row.evidence.length,
        artifact_count: row.artifact_count,
        report_triggered: row.report_triggered,
        side_effects: row.side_effects,
      });
      if (hasCapturedSideEffect(row)) {
        safetyAborted = true;
        safetyAbortReason = 'captured_template_or_report_side_effect';
        break;
      }
    } catch (error) {
      rows.push({
        case_id: fixture.case_id,
        answer: '',
        evidence: [],
        artifacts: [],
        side_effects: {
          static_page_published: false,
          report_generation_enqueued: false,
          new_template_generated: false,
          html_fallback_used: false,
        },
        error: error instanceof Error ? error.message : String(error),
      });
      cases.push({
        ...summarizePayload(fixture, payload),
        status: 'failed',
        latency_ms: Date.now() - startedAt,
        error: error instanceof Error ? error.message : String(error),
      });
      safetyAborted = true;
      safetyAbortReason = 'request_error_uncertain_side_effect';
      break;
    }
  }

  if (safetyAborted) {
    const attemptedCaseIds = new Set(cases.map((item) => item.case_id));
    for (const fixture of selectedCases) {
      if (attemptedCaseIds.has(fixture.case_id)) {
        continue;
      }
      cases.push({
        case_id: fixture.case_id,
        category: fixture.category,
        prompt_sha256: sha256(fixture.prompt),
        orchestration_field_count: 0,
        status: safetyAbortReason === 'request_error_uncertain_side_effect'
          ? 'safety_aborted_uncertain_side_effect'
          : 'safety_aborted',
        reason: safetyAbortReason,
      });
    }
  }

  return { rows, cases, safetyAborted, safetyAbortReason };
}

async function runLive(args, fixtures, selectedCases, runId) {
  const { rows, cases, safetyAborted, safetyAbortReason } = await captureLiveCases(
    args,
    selectedCases,
    runId,
  );
  const outputDir = join(process.cwd(), args.outputDir);
  const resultJsonl = join(outputDir, `${runId}.results.jsonl`);
  await writeJsonl(resultJsonl, rows);
  const evaluator = args.evaluate
    ? runEvaluator(args, resultJsonl, runId, selectedCases)
    : { run: false };
  const failedCaseCount = cases.filter((item) => item.status === 'failed').length;
  const capturedSideEffects = summarizeCapturedSideEffects(cases);
  const report = {
    ok: failedCaseCount === 0
      && (!evaluator.run || evaluator.ok)
      && !capturedSideEffects.detected,
    mode: 'live',
    run_id: runId,
    fixture_path: args.fixturePath,
    result_jsonl: resultJsonl,
    selected_case_count: selectedCases.length,
    total_fixture_case_count: fixtures.length,
    base_url: args.baseUrl,
    connection_id: args.connectionId,
    dataset_external_ids_count: args.datasetExternalIds.length,
    document_external_ids_count: args.documentExternalIds.length,
    include_template_guards: args.includeTemplateGuards,
    include_all_cases: args.includeAllCases,
    safety: {
      network_calls_run: true,
      provider_live_allowed: true,
      provider_call_status: 'unknown',
      mutation_status: 'unknown',
      linked_execution_safety_receipt: false,
      static_page_publish_requested: false,
      captured_side_effect_detected: capturedSideEffects.detected,
      captured_side_effect_case_count: capturedSideEffects.case_count,
      captured_artifact_count: capturedSideEffects.artifact_count,
      aborted_after_captured_side_effect:
        safetyAbortReason === 'captured_template_or_report_side_effect',
      aborted_after_uncertain_request_error:
        safetyAbortReason === 'request_error_uncertain_side_effect',
      safety_abort_reason: safetyAbortReason,
      bearer_included_in_report: false,
    },
    evaluator,
    cases,
  };
  return writeReport(args, report, runId);
}

async function writeReport(args, report, runId) {
  report.evaluation_class = EVALUATION_CLASS;
  report.decision_eligible = false;
  report.promotion_blockers = [...PROMOTION_BLOCKERS];
  report.execution_safety_evidence_status = report.mode === 'live'
    ? 'unknown_without_linked_execution_receipt'
    : 'not_run';
  const summary = buildSummary(report);
  report.summary = summary;
  report.ok = summary.ok;
  const outputDir = join(process.cwd(), args.outputDir);
  await mkdir(outputDir, { recursive: true });
  const reportPath = join(outputDir, `${runId}.json`);
  const markdownPath = join(outputDir, `${runId}.md`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  await writeFile(markdownPath, renderMarkdown(report), 'utf8');
  console.log(JSON.stringify({
    ok: report.ok,
    mode: report.mode,
    run_id: report.run_id,
    selected_case_count: report.selected_case_count,
    result_jsonl: report.result_jsonl,
    report: reportPath,
    markdown: markdownPath,
    evaluator_ok: report.evaluator?.run ? report.evaluator.ok : null,
    captured_side_effect_detected: report.safety?.captured_side_effect_detected ?? false,
  }, null, 2));
  if (!report.ok) {
    process.exitCode = 1;
  }
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const fixtures = await readJsonl(args.fixturePath);
  const selectedCases = args.selfTest ? fixtures : selectCases(args, fixtures);
  if (selectedCases.length === 0) {
    throw new Error('no fixture cases selected');
  }
  const runId = makeRunId();
  if (args.selfTest) {
    await runSelfTest(args, fixtures, selectedCases, runId);
  } else if (args.preflight) {
    await runPreflight(args, fixtures, selectedCases, runId);
  } else {
    await runLive(args, fixtures, selectedCases, runId);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : String(error));
  process.exit(1);
});
