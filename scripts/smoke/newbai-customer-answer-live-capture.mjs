#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
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
  return [...new Set(collectStrings(response, [
    'artifact_links',
    'artifactLinks',
    'public_url',
    'publicUrl',
    'generated_artifact_url',
    'generatedArtifactUrl',
    'download_url',
    'downloadUrl',
  ]).filter((value) => /^https?:\/\//.test(value) || value.startsWith('/')))];
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

function resultFromResponse(fixture, response) {
  const artifactLinks = extractArtifactLinks(response);
  return {
    case_id: fixture.case_id,
    answer: replyText(response),
    evidence: extractEvidence(response),
    artifacts: artifactLinks.map((url) => ({ url })),
    side_effects: sideEffectsFromResponse(response),
    report_triggered: artifactLinks.length > 0
      || /artifact|static_page|report/i.test(
        `${response?.reply?.reply_type || ''} ${response?.reply?.task_status || ''} ${response?.reply?.card?.type || ''}`,
      ),
  };
}

async function writeJsonl(path, rows) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${rows.map((row) => JSON.stringify(row)).join('\n')}\n`, 'utf8');
}

function runEvaluator(args, resultJsonl, runId, selectedCases) {
  const evaluatorDir = join(args.outputDir, 'evaluator');
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
  return {
    run: true,
    ok: result.status === 0,
    status: result.status,
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
  const evaluatorRun = report.evaluator?.run === true;
  const evaluatorOk = !evaluatorRun || report.evaluator?.ok === true;
  const failedCaseCount = cases.filter((item) => item.status === 'failed').length;
  const capturedCaseCount = cases.filter((item) => item.status === 'captured').length;
  const sampleCaseCount = cases.filter((item) => item.status === 'sample_result').length;
  const plannedCaseCount = cases.filter((item) =>
    item.status === 'planned' || item.status === 'template_guard_requires_explicit_live_approval',
  ).length;
  const mode = String(report.mode || '');
  const checks = {
    baseOkPreserved: report.ok === true,
    selectedCasesPresent: Number(report.selected_case_count || 0) > 0
      && cases.length === Number(report.selected_case_count || 0),
    evaluatorPassedWhenRun: evaluatorOk,
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
  const report = {
    ok: !evaluator.run || evaluator.ok,
    mode: 'self-test',
    run_id: runId,
    fixture_path: args.fixturePath,
    result_jsonl: resultJsonl,
    selected_case_count: selectedCases.length,
    total_fixture_case_count: fixtures.length,
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

async function runLive(args, fixtures, selectedCases, runId) {
  const rows = [];
  const cases = [];
  for (const fixture of selectedCases) {
    const payload = buildPayload(args, fixture, runId);
    const startedAt = Date.now();
    try {
      const response = await sendMessage(args, payload);
      const row = resultFromResponse(fixture, response);
      rows.push(row);
      cases.push({
        ...summarizePayload(fixture, payload),
        status: 'captured',
        latency_ms: Date.now() - startedAt,
        assistant_run_id_present: Boolean(response?.assistant_run_id),
        answer_chars: row.answer.length,
        evidence_count: row.evidence.length,
        artifact_count: row.artifacts.length,
        side_effects: row.side_effects,
      });
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
    }
  }

  const outputDir = join(process.cwd(), args.outputDir);
  const resultJsonl = join(outputDir, `${runId}.results.jsonl`);
  await writeJsonl(resultJsonl, rows);
  const evaluator = args.evaluate
    ? runEvaluator(args, resultJsonl, runId, selectedCases)
    : { run: false };
  const failedCaseCount = cases.filter((item) => item.status === 'failed').length;
  const report = {
    ok: failedCaseCount === 0 && (!evaluator.run || evaluator.ok),
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
