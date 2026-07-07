#!/usr/bin/env node

import { createServer } from 'node:http';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_CONNECTION_ID = 'generic-chat-main';
const DEFAULT_SOURCE_ID = 'third-party-source-main';
const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_PARSE_TIMEOUT_MS = 180_000;
const DEFAULT_POLL_INTERVAL_MS = 2_000;

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    connectionId:
      process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_CONNECTION_ID || DEFAULT_CONNECTION_ID,
    bearer: process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_BEARER || '',
    sourceId: process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_SOURCE_ID || DEFAULT_SOURCE_ID,
    timeoutMs: Number(
      process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS,
    ),
    parseTimeoutMs: Number(
      process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_PARSE_TIMEOUT_MS || DEFAULT_PARSE_TIMEOUT_MS,
    ),
    pollIntervalMs: Number(
      process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS,
    ),
    outputDir:
      process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_OUTPUT_DIR ||
      'target/external-scoped-document-chat-smoke',
    allowMissingBearer: parseBoolean(process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_ALLOW_MISSING_BEARER),
    platform: process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_PLATFORM || 'generic_chat',
    tenantExternalId:
      process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_TENANT_EXTERNAL_ID || 'tenant-ext-smoke',
    botExternalId: process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_BOT_EXTERNAL_ID || 'bot-v3',
    senderExternalId:
      process.env.EXTERNAL_SCOPED_DOCUMENT_SMOKE_SENDER_EXTERNAL_ID ||
      'user-scoped-document-smoke',
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
    } else if (arg === '--source-id') {
      args.sourceId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--parse-timeout-ms') {
      args.parseTimeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--poll-interval-ms') {
      args.pollIntervalMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
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
    } else if (arg === '--allow-missing-bearer') {
      args.allowMissingBearer = true;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (!args.selfTest && !args.allowMissingBearer && !args.bearer) {
    throw new Error('--bearer is required unless --allow-missing-bearer is set');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) {
    throw new Error('--timeout-ms must be at least 1000');
  }
  if (!Number.isInteger(args.parseTimeoutMs) || args.parseTimeoutMs < 1000) {
    throw new Error('--parse-timeout-ms must be at least 1000');
  }
  if (!Number.isInteger(args.pollIntervalMs) || args.pollIntervalMs < 100) {
    throw new Error('--poll-interval-ms must be at least 100');
  }
  return args;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:external-scoped-document-chat -- \\
    --base-url https://v3.elepcloud.com \\
    --connection-id generic-chat-main \\
    --bearer <token>

This smoke should run on the same host as DataMax when using the built-in
loopback fixtures. It posts temporary Markdown fixtures through
/documents/parse, waits until they are indexed, then verifies:
  - dataset_external_ids authorize a stable document group
  - explicit document IDs outside the dataset are unioned in
  - the same conversation_external_id restores the prior scope
  - another conversation_external_id cannot see that restored scope
  - attachment_refs filename matching can include an uploaded reference doc

Environment aliases:
  EXTERNAL_SCOPED_DOCUMENT_SMOKE_BASE_URL
  EXTERNAL_SCOPED_DOCUMENT_SMOKE_CONNECTION_ID
  EXTERNAL_SCOPED_DOCUMENT_SMOKE_BEARER
  EXTERNAL_SCOPED_DOCUMENT_SMOKE_SOURCE_ID

Use --self-test for deterministic offline payload and parser checks without
calling DataMax or requiring a bearer.
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

function requestHeaders(args) {
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

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

function buildFixtures(runId) {
  const datasetExternalId = `dmx-smoke-scope-${runId}`;
  const extraDatasetExternalId = `dmx-smoke-extra-${runId}`;
  const attachmentDatasetExternalId = `dmx-smoke-attachment-${runId}`;
  const alphaToken = `DMX-SCOPE-ALPHA-${runId}`;
  const betaToken = `DMX-SCOPE-BETA-${runId}`;
  const extraToken = `DMX-SCOPE-EXTRA-${runId}`;
  const attachmentToken = `DMX-SCOPE-ATTACH-${runId}`;
  const attachmentTitle = `DataMax Scope Smoke Attachment ${runId}.md`;
  return {
    datasetExternalId,
    extraDatasetExternalId,
    attachmentDatasetExternalId,
    expected: {
      alphaToken,
      betaToken,
      extraToken,
      attachmentToken,
    },
    documents: [
      {
        key: 'alpha',
        path: '/alpha.md',
        datasetExternalId,
        documentExternalId: `dmx-smoke-alpha-${runId}`,
        title: `DataMax Scope Smoke Alpha ${runId}.md`,
        body: `# DataMax scoped smoke alpha\n\nThe alpha scope token is ${alphaToken}.\n`,
      },
      {
        key: 'beta',
        path: '/beta.md',
        datasetExternalId,
        documentExternalId: `dmx-smoke-beta-${runId}`,
        title: `DataMax Scope Smoke Beta ${runId}.md`,
        body: `# DataMax scoped smoke beta\n\nThe beta scope token is ${betaToken}.\n`,
      },
      {
        key: 'extra',
        path: '/extra.md',
        datasetExternalId: extraDatasetExternalId,
        documentExternalId: `dmx-smoke-extra-${runId}`,
        title: `DataMax Scope Smoke Extra ${runId}.md`,
        body: `# DataMax scoped smoke extra\n\nThe explicit extra scope token is ${extraToken}.\n`,
      },
      {
        key: 'attachment',
        path: '/attachment.md',
        datasetExternalId: attachmentDatasetExternalId,
        documentExternalId: `dmx-smoke-attachment-${runId}`,
        title: attachmentTitle,
        body: `# DataMax scoped smoke attachment\n\nThe attachment scope token is ${attachmentToken}.\n`,
      },
    ],
    attachmentRef: {
      attachment_external_id: `dmx-smoke-attachment-ref-${runId}`,
      filename: attachmentTitle,
      content_type: 'text/markdown',
      size_bytes: 128,
      download_url_redacted: `loopback-fixture://${attachmentTitle}`,
    },
  };
}

async function withFixtureServer(fixtures, callback) {
  const byPath = new Map(fixtures.documents.map((fixture) => [fixture.path, fixture]));
  const server = createServer((request, response) => {
    const url = new URL(request.url || '/', 'http://127.0.0.1');
    const fixture = byPath.get(url.pathname);
    if (!fixture) {
      response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
      response.end('not found');
      return;
    }
    response.writeHead(200, { 'content-type': 'text/markdown; charset=utf-8' });
    response.end(fixture.body);
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      server.off('error', reject);
      resolve();
    });
  });
  try {
    const address = server.address();
    const port = typeof address === 'object' && address ? address.port : null;
    if (!port) {
      throw new Error('failed to allocate fixture server port');
    }
    return await callback(`http://127.0.0.1:${port}`);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
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

async function parseFixture(args, fixture, fixtureBaseUrl, runId) {
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/documents/parse`,
    normalizeBaseUrl(args.baseUrl),
  );
  const body = {
    source_id: args.sourceId,
    dataset_external_id: fixture.datasetExternalId,
    document_external_id: fixture.documentExternalId,
    revision_external_id: `v-${runId}`,
    title: fixture.title,
    content_type: 'text/markdown',
    content_url: `${fixtureBaseUrl}${fixture.path}`,
    metadata: {
      smoke: 'external-scoped-document-chat',
      fixture_key: fixture.key,
      run_id: runId,
    },
    idempotency_key: `external-scoped-document-chat:parse:${runId}:${fixture.key}`,
    allow_http_loopback: true,
  };
  const result = await requestJson(
    url,
    { method: 'POST', headers: requestHeaders(args), body: JSON.stringify(body) },
    args.timeoutMs,
  );
  if (!result.response.ok) {
    throw new Error(
      `parse ${fixture.key} failed HTTP ${result.response.status}: ${result.text.slice(0, 300)}`,
    );
  }
  return {
    key: fixture.key,
    documentExternalId: fixture.documentExternalId,
    datasetExternalId: fixture.datasetExternalId,
    title: fixture.title,
    accepted: Boolean(result.data?.accepted),
    documentId: result.data?.document?.id || null,
    workflowStatus: result.data?.workflow_execution?.status || null,
  };
}

async function getParseDetail(args, fixture) {
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/documents/${encodeURIComponent(
      fixture.documentExternalId,
    )}/parse-detail`,
    normalizeBaseUrl(args.baseUrl),
  );
  url.searchParams.set('source_id', args.sourceId);
  const result = await requestJson(url, { method: 'GET', headers: requestHeaders(args) }, args.timeoutMs);
  if (!result.response.ok) {
    throw new Error(
      `parse detail ${fixture.key} failed HTTP ${result.response.status}: ${result.text.slice(
        0,
        300,
      )}`,
    );
  }
  return result.data;
}

function parseDetailReady(detail) {
  const latest = detail?.latest || null;
  const parseStatus = String(detail?.parse_status || detail?.parseStatus || latest?.parse_status || '');
  const modelStatus = String(detail?.model_status || detail?.modelStatus || latest?.model_status || '');
  const chunkCount = Number(detail?.chunk_count ?? detail?.chunkCount ?? latest?.chunk_count ?? 0);
  const evidenceCount = Number(
    detail?.retrieval_evidence_count ?? latest?.retrieval_evidence_count ?? 0,
  );
  const failed = /failed|error|cancel/i.test(`${parseStatus} ${modelStatus}`);
  return {
    ready: !failed && chunkCount > 0 && evidenceCount > 0,
    failed,
    parseStatus: parseStatus || null,
    modelStatus: modelStatus || null,
    chunkCount,
    evidenceCount,
  };
}

async function waitForParseReady(args, fixture) {
  const startedAt = Date.now();
  let lastStatus = null;
  while (Date.now() - startedAt <= args.parseTimeoutMs) {
    const detail = await getParseDetail(args, fixture);
    lastStatus = parseDetailReady(detail);
    if (lastStatus.ready) {
      return lastStatus;
    }
    if (lastStatus.failed) {
      throw new Error(`parse ${fixture.key} failed: ${JSON.stringify(lastStatus)}`);
    }
    await sleep(args.pollIntervalMs);
  }
  throw new Error(`parse ${fixture.key} did not become ready: ${JSON.stringify(lastStatus)}`);
}

function buildMessagePayload(args, runId, caseId, overrides = {}) {
  return {
    platform: args.platform,
    tenant_external_id: args.tenantExternalId,
    bot_external_id: args.botExternalId,
    conversation_external_id: overrides.conversationExternalId || `conv-scoped-${runId}`,
    thread_external_id: null,
    sender_external_id: args.senderExternalId,
    sender_display_name: 'DataMax scoped document smoke',
    message_external_id: `msg-scoped-${runId}-${caseId}`,
    message_type: 'text',
    text: overrides.text || '',
    default_prompt: '请严格依据本轮授权文档范围回答；不要编造未授权资料。',
    output_format: 'rich_text',
    render_mode: 'normal',
    available_document_source_id: args.sourceId,
    available_document_external_ids: overrides.documentExternalIds || [],
    dataset_external_ids: overrides.datasetExternalIds || [],
    documentExternalId: overrides.documentExternalId || null,
    requested_skills: [],
    mention_external_user_ids: [],
    attachment_refs: overrides.attachmentRefs || [],
    idempotency_key: `external-scoped-document-chat:${runId}:${caseId}`,
    received_at: new Date().toISOString(),
    render_options: null,
  };
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
    throw new Error(`event ${payload.message_external_id} failed HTTP ${result.response.status}: ${result.text.slice(0, 300)}`);
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
    const status = replyStatus(latest?.reply);
    if (!['accepted', 'queued', 'processing', 'running'].includes(status)) {
      return latest;
    }
    await sleep(args.pollIntervalMs);
  }
  throw new Error(`reply ${assistantRunId} did not complete: ${JSON.stringify(latest?.reply || null)}`);
}

async function sendMessage(args, payload) {
  const initial = await postEvent(args, payload);
  const initialStatus = replyStatus(initial?.reply);
  const assistantRunId = initial?.assistant_run_id || null;
  if (assistantRunId && ['accepted', 'queued', 'processing', 'running'].includes(initialStatus)) {
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
    reply.card?.text,
    reply.card?.summary,
    reply.card?.question,
    reply.card?.status,
    ...(Array.isArray(reply.artifact_links) ? reply.artifact_links : []),
  ].filter((value) => typeof value === 'string' && value.trim());
  return parts.join('\n');
}

function assertIncludes(text, token, label) {
  if (!text.includes(token)) {
    throw new Error(`${label} did not include expected token ${token}; text prefix: ${text.slice(0, 240)}`);
  }
}

function assertExcludes(text, token, label) {
  if (text.includes(token)) {
    throw new Error(`${label} unexpectedly leaked token ${token}; text prefix: ${text.slice(0, 240)}`);
  }
}

function summarizeCase(caseId, payload, response, expectedTokens = []) {
  const text = replyText(response);
  return {
    caseId,
    ok: true,
    conversationExternalId: payload.conversation_external_id,
    messageExternalId: payload.message_external_id,
    assistantRunId: response?.assistant_run_id || null,
    replyType: response?.reply?.reply_type || null,
    taskStatus: response?.reply?.task_status || null,
    textLength: text.length,
    expectedTokenHits: expectedTokens.map((token) => ({ token, present: text.includes(token) })),
  };
}

function caseByIdFragment(cases, fragment) {
  return cases.find((item) => String(item.caseId || '').includes(fragment)) || null;
}

function allExpectedTokensPresent(caseSummary) {
  return Boolean(caseSummary)
    && caseSummary.textLength > 0
    && caseSummary.expectedTokenHits.length > 0
    && caseSummary.expectedTokenHits.every((hit) => hit.present === true);
}

function allExpectedTokensAbsent(caseSummary) {
  return Boolean(caseSummary)
    && caseSummary.textLength > 0
    && caseSummary.expectedTokenHits.length > 0
    && caseSummary.expectedTokenHits.every((hit) => hit.present === false);
}

function buildScopedDocumentChatChecks(parseReady, cases) {
  const datasetUnion = caseByIdFragment(cases, 'dataset-union');
  const followup = caseByIdFragment(cases, 'same-conversation-followup');
  const isolated = caseByIdFragment(cases, 'isolated-conversation-no-scope');
  const attachment = caseByIdFragment(cases, 'attachment-title-scope');
  return {
    parseReadyAllSucceeded: parseReady.length > 0
      && parseReady.every((item) => item.ready === true && item.failed === false),
    datasetUnionIncludesAuthorizedTokens: allExpectedTokensPresent(datasetUnion),
    sameConversationFollowupInheritsScope: allExpectedTokensPresent(followup),
    isolatedConversationDoesNotLeakScope: allExpectedTokensAbsent(isolated),
    attachmentScopeIncludesAttachmentToken: allExpectedTokensPresent(attachment),
  };
}

async function runSelfTest(args) {
  const runId = `${makeRunId()}-self-test`;
  const fixtures = buildFixtures(runId);
  const primaryConversationId = `conv-scoped-${runId}`;
  const isolatedConversationId = `conv-scoped-isolated-${runId}`;
  const attachmentConversationId = `conv-scoped-attachment-${runId}`;
  const extraDocumentExternalId = fixtures.documents.find((item) => item.key === 'extra').documentExternalId;

  const datasetUnionPayload = buildMessagePayload(args, runId, 'dataset-union', {
    conversationExternalId: primaryConversationId,
    datasetExternalIds: [fixtures.datasetExternalId],
    documentExternalIds: [extraDocumentExternalId],
    text: 'self-test dataset union',
  });
  const followupPayload = buildMessagePayload(args, runId, 'same-conversation-followup', {
    conversationExternalId: primaryConversationId,
    text: 'self-test follow-up inherits scope',
  });
  const isolatedPayload = buildMessagePayload(args, runId, 'isolated-conversation-no-scope', {
    conversationExternalId: isolatedConversationId,
    text: 'self-test isolated scope',
  });
  const attachmentPayload = buildMessagePayload(args, runId, 'attachment-title-scope', {
    conversationExternalId: attachmentConversationId,
    attachmentRefs: [fixtures.attachmentRef],
    text: 'self-test attachment scope',
  });

  const readyStatus = parseDetailReady({
    parse_status: 'parsed',
    model_status: 'indexed',
    chunk_count: 2,
    retrieval_evidence_count: 2,
  });
  const failedStatus = parseDetailReady({
    parse_status: 'failed',
    model_status: 'failed',
    chunk_count: 0,
    retrieval_evidence_count: 0,
  });
  const datasetUnionResponse = {
    assistant_run_id: 'run-self-test-dataset-union',
    reply: {
      reply_type: 'answered',
      task_status: 'answered',
      text: `${fixtures.expected.alphaToken}\n${fixtures.expected.betaToken}\n${fixtures.expected.extraToken}`,
    },
  };
  const followupResponse = {
    assistant_run_id: 'run-self-test-followup',
    reply: {
      reply_type: 'answered',
      task_status: 'answered',
      text: fixtures.expected.extraToken,
    },
  };
  const isolatedResponse = {
    assistant_run_id: 'run-self-test-isolated',
    reply: {
      reply_type: 'answered',
      task_status: 'answered',
      text: '当前会话没有可见授权资料。',
    },
  };
  const attachmentResponse = {
    assistant_run_id: 'run-self-test-attachment',
    reply: {
      reply_type: 'answered',
      task_status: 'answered',
      text: fixtures.expected.attachmentToken,
    },
  };
  const cases = [
    summarizeCase('dataset-union', datasetUnionPayload, datasetUnionResponse, [
      fixtures.expected.alphaToken,
      fixtures.expected.betaToken,
      fixtures.expected.extraToken,
    ]),
    summarizeCase('same-conversation-followup', followupPayload, followupResponse, [
      fixtures.expected.extraToken,
    ]),
    summarizeCase('isolated-conversation-no-scope', isolatedPayload, isolatedResponse, [
      fixtures.expected.extraToken,
    ]),
    summarizeCase('attachment-title-scope', attachmentPayload, attachmentResponse, [
      fixtures.expected.attachmentToken,
    ]),
  ];
  const scopedSummaryChecks = buildScopedDocumentChatChecks([readyStatus], cases);
  const markdown = renderMarkdown({
    ok: true,
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    sourceId: args.sourceId,
    startedAt: new Date().toISOString(),
    finishedAt: new Date().toISOString(),
    fixtureSummary: {
      datasetExternalId: fixtures.datasetExternalId,
      extraDatasetExternalId: fixtures.extraDatasetExternalId,
      attachmentDatasetExternalId: fixtures.attachmentDatasetExternalId,
      documentCount: fixtures.documents.length,
    },
    parseReady: [
      {
        key: 'alpha',
        documentExternalId: fixtures.documents[0].documentExternalId,
        ...readyStatus,
      },
    ],
    cases,
  });
  const checks = {
    fixtureDocumentCount: fixtures.documents.length === 4,
    datasetUnionUsesDatasetExternalIds:
      datasetUnionPayload.dataset_external_ids.length === 1
      && datasetUnionPayload.dataset_external_ids[0] === fixtures.datasetExternalId,
    datasetUnionUsesAvailableDocumentExternalIds:
      datasetUnionPayload.available_document_external_ids.length === 1
      && datasetUnionPayload.available_document_external_ids[0] === extraDocumentExternalId,
    sameConversationFollowupCarriesNoRepeatedScope:
      followupPayload.conversation_external_id === primaryConversationId
      && followupPayload.dataset_external_ids.length === 0
      && followupPayload.available_document_external_ids.length === 0,
    isolatedConversationIsDifferent:
      isolatedPayload.conversation_external_id !== primaryConversationId
      && isolatedPayload.dataset_external_ids.length === 0
      && isolatedPayload.available_document_external_ids.length === 0,
    attachmentRefCarriesFilename:
      attachmentPayload.attachment_refs.length === 1
      && attachmentPayload.attachment_refs[0].filename === fixtures.attachmentRef.filename,
    parseReadyRequiresChunksAndEvidence: readyStatus.ready === true && readyStatus.failed === false,
    parseFailedDetected: failedStatus.ready === false && failedStatus.failed === true,
    replyTextIncludesExpectedUnionTokens:
      cases[0].expectedTokenHits.every((hit) => hit.present === true),
    isolatedReplyDoesNotLeakExtraToken:
      cases[2].expectedTokenHits.every((hit) => hit.present === false),
    scopedSummaryChecksMachineOk:
      scopedSummaryChecks.parseReadyAllSucceeded === true
      && scopedSummaryChecks.datasetUnionIncludesAuthorizedTokens === true
      && scopedSummaryChecks.sameConversationFollowupInheritsScope === true
      && scopedSummaryChecks.isolatedConversationDoesNotLeakScope === true
      && scopedSummaryChecks.attachmentScopeIncludesAttachmentToken === true,
    markdownRendered: markdown.includes('# External Scoped Document Chat Smoke'),
  };
  const ok = Object.values(checks).every(Boolean);
  const summary = {
    smoke: 'external-scoped-document-chat',
    selfTest: true,
    ok,
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    sourceId: args.sourceId,
    checks,
    scopedSummaryChecks,
    payloadShape: {
      datasetUnion: {
        datasetExternalIdCount: datasetUnionPayload.dataset_external_ids.length,
        availableDocumentExternalIdCount: datasetUnionPayload.available_document_external_ids.length,
      },
      sameConversationFollowup: {
        conversationExternalId: followupPayload.conversation_external_id,
        datasetExternalIdCount: followupPayload.dataset_external_ids.length,
        availableDocumentExternalIdCount: followupPayload.available_document_external_ids.length,
      },
      isolatedConversation: {
        conversationExternalId: isolatedPayload.conversation_external_id,
        datasetExternalIdCount: isolatedPayload.dataset_external_ids.length,
        availableDocumentExternalIdCount: isolatedPayload.available_document_external_ids.length,
      },
      attachment: {
        attachmentRefCount: attachmentPayload.attachment_refs.length,
      },
    },
    generatedAt: new Date().toISOString(),
  };
  await mkdir(args.outputDir, { recursive: true });
  const reportPath = join(args.outputDir, `${runId}.json`);
  await writeFile(reportPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({ ok, selfTest: true, runId, reportPath }, null, 2));
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
  const runId = makeRunId();
  const startedAt = new Date().toISOString();
  const fixtures = buildFixtures(runId);
  const parseResults = [];
  const parseReady = [];
  const cases = [];

  await withFixtureServer(fixtures, async (fixtureBaseUrl) => {
    for (const fixture of fixtures.documents) {
      parseResults.push(await parseFixture(args, fixture, fixtureBaseUrl, runId));
    }
    for (const fixture of fixtures.documents) {
      parseReady.push({
        key: fixture.key,
        documentExternalId: fixture.documentExternalId,
        ...(await waitForParseReady(args, fixture)),
      });
    }
  });

  const primaryConversationId = `conv-scoped-${runId}`;
  const isolatedConversationId = `conv-scoped-isolated-${runId}`;
  const attachmentConversationId = `conv-scoped-attachment-${runId}`;

  const datasetUnionPayload = buildMessagePayload(args, runId, 'dataset-union', {
    conversationExternalId: primaryConversationId,
    datasetExternalIds: [fixtures.datasetExternalId],
    documentExternalIds: [fixtures.documents.find((item) => item.key === 'extra').documentExternalId],
    text:
      '请只依据本轮授权资料回答，列出 alpha、beta、extra 三个 DataMax scope smoke token，逐项写出 token。',
  });
  const datasetUnionResponse = await sendMessage(args, datasetUnionPayload);
  const datasetUnionText = replyText(datasetUnionResponse);
  assertIncludes(datasetUnionText, fixtures.expected.alphaToken, 'dataset union');
  assertIncludes(datasetUnionText, fixtures.expected.betaToken, 'dataset union');
  assertIncludes(datasetUnionText, fixtures.expected.extraToken, 'dataset union');
  cases.push(
    summarizeCase(datasetUnionPayload.message_external_id, datasetUnionPayload, datasetUnionResponse, [
      fixtures.expected.alphaToken,
      fixtures.expected.betaToken,
      fixtures.expected.extraToken,
    ]),
  );

  const followupPayload = buildMessagePayload(args, runId, 'same-conversation-followup', {
    conversationExternalId: primaryConversationId,
    text: '继续沿用刚才同一会话的资料范围，只回答 extra 的 DataMax scope smoke token。',
  });
  const followupResponse = await sendMessage(args, followupPayload);
  const followupText = replyText(followupResponse);
  assertIncludes(followupText, fixtures.expected.extraToken, 'same-conversation follow-up');
  cases.push(
    summarizeCase(followupPayload.message_external_id, followupPayload, followupResponse, [
      fixtures.expected.extraToken,
    ]),
  );

  const isolatedPayload = buildMessagePayload(args, runId, 'isolated-conversation-no-scope', {
    conversationExternalId: isolatedConversationId,
    text: '继续沿用刚才同一会话的资料范围，只回答 extra 的 DataMax scope smoke token。',
  });
  const isolatedResponse = await sendMessage(args, isolatedPayload);
  const isolatedText = replyText(isolatedResponse);
  assertExcludes(isolatedText, fixtures.expected.extraToken, 'isolated conversation');
  cases.push(
    summarizeCase(isolatedPayload.message_external_id, isolatedPayload, isolatedResponse, [
      fixtures.expected.extraToken,
    ]),
  );

  const attachmentPayload = buildMessagePayload(args, runId, 'attachment-title-scope', {
    conversationExternalId: attachmentConversationId,
    attachmentRefs: [fixtures.attachmentRef],
    text: `请根据附件 ${fixtures.attachmentRef.filename} 回答 attachment 的 DataMax scope smoke token。`,
  });
  const attachmentResponse = await sendMessage(args, attachmentPayload);
  const attachmentText = replyText(attachmentResponse);
  assertIncludes(attachmentText, fixtures.expected.attachmentToken, 'attachment title scope');
  cases.push(
    summarizeCase(attachmentPayload.message_external_id, attachmentPayload, attachmentResponse, [
      fixtures.expected.attachmentToken,
    ]),
  );

  const finishedAt = new Date().toISOString();
  const checks = buildScopedDocumentChatChecks(parseReady, cases);
  const ok = Object.values(checks).every(Boolean);
  const summary = {
    smoke: 'external-scoped-document-chat',
    ok,
    checks,
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    sourceId: args.sourceId,
    startedAt,
    finishedAt,
    fixtureSummary: {
      datasetExternalId: fixtures.datasetExternalId,
      extraDatasetExternalId: fixtures.extraDatasetExternalId,
      attachmentDatasetExternalId: fixtures.attachmentDatasetExternalId,
      documentCount: fixtures.documents.length,
    },
    parseResults,
    parseReady,
    cases,
  };

  await mkdir(args.outputDir, { recursive: true });
  const reportPath = join(args.outputDir, `${runId}.json`);
  const markdownPath = join(args.outputDir, `${runId}.md`);
  await writeFile(reportPath, JSON.stringify(summary, null, 2), 'utf8');
  await writeFile(markdownPath, renderMarkdown(summary), 'utf8');
  console.log(JSON.stringify({ ok, runId, reportPath, markdownPath, caseCount: cases.length }, null, 2));
  if (!ok) {
    process.exitCode = 1;
  }
}

function renderMarkdown(summary) {
  const lines = [
    '# External Scoped Document Chat Smoke',
    '',
    `- Status: ${summary.ok ? 'passed' : 'failed'}`,
    `- Run ID: ${summary.runId}`,
    `- Base URL: ${summary.baseUrl}`,
    `- Connection: ${summary.connectionId}`,
    `- Source: ${summary.sourceId}`,
    `- Started: ${summary.startedAt}`,
    `- Finished: ${summary.finishedAt}`,
    '',
    '## Fixture Scope',
    '',
    `- Dataset group: \`${summary.fixtureSummary.datasetExternalId}\``,
    `- Explicit extra group: \`${summary.fixtureSummary.extraDatasetExternalId}\``,
    `- Attachment group: \`${summary.fixtureSummary.attachmentDatasetExternalId}\``,
    `- Documents parsed: ${summary.fixtureSummary.documentCount}`,
    '',
    '## Parse Readiness',
    '',
    ...summary.parseReady.map(
      (item) =>
        `- ${item.key}: parse=${item.parseStatus || 'unknown'}, model=${item.modelStatus || 'unknown'}, chunks=${item.chunkCount}, evidence=${item.evidenceCount}`,
    ),
    '',
    '## Cases',
    '',
    ...summary.cases.map(
      (item) =>
        `- ${item.caseId}: ${item.replyType || 'unknown'} / ${item.taskStatus || 'unknown'}, textLength=${item.textLength}, expectedHits=${item.expectedTokenHits
          .map((hit) => `${hit.token}:${hit.present}`)
          .join(', ')}`,
    ),
    '',
  ];
  return `${lines.join('\n')}\n`;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : String(error));
  process.exit(1);
});
