#!/usr/bin/env node

import { createServer } from 'node:http';
import { basename, extname, join } from 'node:path';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_CONNECTION_ID = 'generic-chat-main';
const DEFAULT_SOURCE_ID = 'third-party-source-main';
const DEFAULT_FIXTURE_URL =
  'https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4';
const DEFAULT_TIMEOUT_MS = 300_000;
const DEFAULT_PARSE_TIMEOUT_MS = 180_000;
const DEFAULT_POLL_INTERVAL_MS = 5_000;
const DEFAULT_OUTPUT_DIR = 'target/external-video-ppt-smoke';
const REQUIRED_FILE_KINDS = [
  'pptx',
  'video_slides_markdown',
  'final_deliverables_manifest',
  'published_deliverable_manifest',
  'published_version_history',
  'extraction_artifacts_manifest',
];
const REQUIRED_PPTX_ENTRIES = [
  '[Content_Types].xml',
  'ppt/presentation.xml',
  'ppt/slides/slide1.xml',
];
const SUPPORTED_VIDEO_EXTENSIONS = ['.mp4', '.mov', '.m4v', '.webm', '.mkv', '.avi'];
const LIVE_WRITE_STEPS = [
  'documents_parse_register_video',
  'parse_detail_poll',
  'events_post_video_ppt_trigger',
  'assistant_run_reply_poll',
  'optional_deliverable_downloads',
];

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.EXTERNAL_VIDEO_PPT_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    connectionId: process.env.EXTERNAL_VIDEO_PPT_SMOKE_CONNECTION_ID || DEFAULT_CONNECTION_ID,
    bearer: process.env.EXTERNAL_VIDEO_PPT_SMOKE_BEARER || '',
    sourceId: process.env.EXTERNAL_VIDEO_PPT_SMOKE_SOURCE_ID || DEFAULT_SOURCE_ID,
    fixtureUrl: process.env.EXTERNAL_VIDEO_PPT_SMOKE_FIXTURE_URL || DEFAULT_FIXTURE_URL,
    fixtureFile: process.env.EXTERNAL_VIDEO_PPT_SMOKE_FIXTURE_FILE || '',
    fixtureName: process.env.EXTERNAL_VIDEO_PPT_SMOKE_FIXTURE_NAME || '',
    contentType: process.env.EXTERNAL_VIDEO_PPT_SMOKE_CONTENT_TYPE || '',
    timeoutMs: Number(process.env.EXTERNAL_VIDEO_PPT_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    parseTimeoutMs: Number(
      process.env.EXTERNAL_VIDEO_PPT_SMOKE_PARSE_TIMEOUT_MS || DEFAULT_PARSE_TIMEOUT_MS,
    ),
    pollIntervalMs: Number(
      process.env.EXTERNAL_VIDEO_PPT_SMOKE_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS,
    ),
    outputDir: process.env.EXTERNAL_VIDEO_PPT_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    platform: process.env.EXTERNAL_VIDEO_PPT_SMOKE_PLATFORM || 'generic_chat',
    tenantExternalId: process.env.EXTERNAL_VIDEO_PPT_SMOKE_TENANT_EXTERNAL_ID || 'tenant-ext-smoke',
    botExternalId: process.env.EXTERNAL_VIDEO_PPT_SMOKE_BOT_EXTERNAL_ID || 'bot-v3',
    senderExternalId:
      process.env.EXTERNAL_VIDEO_PPT_SMOKE_SENDER_EXTERNAL_ID || 'user-video-ppt-smoke',
    allowMissingBearer: parseBoolean(process.env.EXTERNAL_VIDEO_PPT_SMOKE_ALLOW_MISSING_BEARER),
    loopbackFixture: parseBoolean(process.env.EXTERNAL_VIDEO_PPT_SMOKE_LOOPBACK_FIXTURE),
    skipDeliverableDownloads: parseBoolean(
      process.env.EXTERNAL_VIDEO_PPT_SMOKE_SKIP_DELIVERABLE_DOWNLOADS,
    ),
    selfTest: parseBoolean(process.env.EXTERNAL_VIDEO_PPT_SMOKE_SELF_TEST),
    preflight: parseBoolean(process.env.EXTERNAL_VIDEO_PPT_SMOKE_PREFLIGHT),
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
    } else if (arg === '--fixture-url') {
      args.fixtureUrl = requireValue(arg, next);
      index += 1;
    } else if (arg === '--fixture-file') {
      args.fixtureFile = requireValue(arg, next);
      index += 1;
    } else if (arg === '--fixture-name') {
      args.fixtureName = requireValue(arg, next);
      index += 1;
    } else if (arg === '--content-type') {
      args.contentType = requireValue(arg, next);
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
    } else if (arg === '--loopback-fixture') {
      args.loopbackFixture = true;
    } else if (arg === '--skip-deliverable-downloads') {
      args.skipDeliverableDownloads = true;
    } else if (arg === '--self-test') {
      args.selfTest = true;
      args.allowMissingBearer = true;
    } else if (arg === '--preflight') {
      args.preflight = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (args.selfTest && args.preflight) {
    throw new Error('--self-test and --preflight cannot be combined');
  }
  if (!args.selfTest && !args.preflight && !args.allowMissingBearer && !args.bearer) {
    throw new Error('--bearer is required unless --allow-missing-bearer or --self-test is set');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 10_000) {
    throw new Error('--timeout-ms must be at least 10000');
  }
  if (!Number.isInteger(args.parseTimeoutMs) || args.parseTimeoutMs < 10_000) {
    throw new Error('--parse-timeout-ms must be at least 10000');
  }
  if (!Number.isInteger(args.pollIntervalMs) || args.pollIntervalMs < 500) {
    throw new Error('--poll-interval-ms must be at least 500');
  }
  return args;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:external-video-ppt -- \\
    --base-url https://v3.elepcloud.com \\
    --connection-id generic-chat-main \\
    --source-id third-party-source-main \\
    --bearer <token> \\
    --fixture-url https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4

  npm run smoke:external-video-ppt -- --self-test

  npm run smoke:external-video-ppt -- --preflight --allow-missing-bearer

Checks:
  - registers a video fixture through /v1/external/channels/{connection_id}/documents/parse
  - sends a third-party /events message scoped to the registered video
  - explicitly asks to extract PPT/slides/courseware from the video
  - polls /assistant-runs/{run_id}/reply
  - validates third-party video extraction reply surface and optional deliverable downloads

Notes:
  - --self-test does not call the network
  - --preflight validates fixture/context/trigger shape and live write scope without network calls
  - live mode requires bearer unless --allow-missing-bearer is set for local loopback
  - --skip-deliverable-downloads only verifies the reply surface, not PPTX/Markdown files
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

function requestHeaders(args, contentType = 'application/json') {
  const headers = {};
  if (contentType) {
    headers['content-type'] = contentType;
  }
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

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function fixtureFileName(args) {
  if (args.fixtureName) {
    return args.fixtureName;
  }
  if (args.fixtureFile) {
    return basename(args.fixtureFile);
  }
  try {
    const url = new URL(args.fixtureUrl);
    const name = basename(url.pathname);
    if (name && extname(name)) {
      return name;
    }
  } catch {
    // Use fallback below.
  }
  return 'video-ppt-external-smoke.mp4';
}

function inferContentType(fileName, fallback = 'video/mp4') {
  const lowerName = String(fileName || '').toLowerCase();
  if (lowerName.endsWith('.mp4') || lowerName.endsWith('.m4v')) return 'video/mp4';
  if (lowerName.endsWith('.mov')) return 'video/quicktime';
  if (lowerName.endsWith('.webm')) return 'video/webm';
  if (lowerName.endsWith('.mkv')) return 'video/x-matroska';
  if (lowerName.endsWith('.avi')) return 'video/x-msvideo';
  return fallback;
}

function inferUploadMediaKind(fileName, contentType) {
  const signal = `${fileName || ''} ${contentType || ''}`.toLowerCase();
  return /(^|\W)video\//.test(signal) || /\.(mp4|mov|m4v|webm|mkv|avi|mpeg|mpg)(\W|$)/i.test(signal)
    ? 'video'
    : '';
}

function promptRequestsVideoPpt(prompt) {
  const text = String(prompt || '');
  const mentionsSlides = /ppt|powerpoint|slides?|幻灯片|课件/i.test(text);
  if (!mentionsSlides) {
    return false;
  }
  const extractionIntent = /提取|抽取|抓取|导出|识别|extract|pull|capture|export|视频(?:里|中|里的|中的)|from (?:the )?video|shown in (?:the )?video/i
    .test(text);
  if (!extractionIntent) {
    return false;
  }
  const ordinaryVideoToPpt = /普通视频|任意视频|把.*视频.*(?:生成|做成|制作|创作|转成|变成).*ppt|(?:make|create|generate|turn).{0,40}(?:ppt|powerpoint|slides).{0,40}(?:from|out of).{0,20}(?:the )?video|video[- ]to[- ]ppt/i
    .test(text);
  const extractionFromExistingSlides = /提取|抽取|抓取|extract|pull|capture|视频(?:里|中|里的|中的).*(?:ppt|powerpoint|slides?|幻灯片|课件)|(?:ppt|powerpoint|slides?|幻灯片|课件).*(?:视频里|视频中|already shown)/i
    .test(text);
  return !ordinaryVideoToPpt || extractionFromExistingSlides;
}

function assertVideoPptTriggerClassifier() {
  const positivePrompts = [
    '请提取这个视频里的 PPT/幻灯片/课件。',
    '请提取视频中的PPT。',
    '从这个视频中抽取课件页面。',
    'Extract slides from this video.',
  ];
  const negativePrompts = [
    '请总结这个视频。',
    '请把普通视频变成PPT。',
    'Create a PowerPoint from this ordinary video.',
    '生成一个PPT介绍这段视频。',
  ];
  const missedPositives = positivePrompts.filter((prompt) => !promptRequestsVideoPpt(prompt));
  if (missedPositives.length > 0) {
    throw new Error(`video PPT trigger classifier missed positive prompts: ${missedPositives.join(' | ')}`);
  }
  const falsePositives = negativePrompts.filter((prompt) => promptRequestsVideoPpt(prompt));
  if (falsePositives.length > 0) {
    throw new Error(`video PPT trigger classifier accepted ordinary video-to-PPT prompts: ${falsePositives.join(' | ')}`);
  }
  return {
    positivePromptCount: positivePrompts.length,
    negativePromptCount: negativePrompts.length,
  };
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, "'\\''")}'`;
}

function redactedLiveCommand(args, fixtureName) {
  const fixtureArg = args.fixtureFile
    ? '--fixture-file <redacted-local-video-file>'
    : '--fixture-url <redacted-public-video-url>';
  const fixtureNameArg = fixtureName ? ` --fixture-name ${shellQuote(fixtureName)}` : '';
  return [
    'npm run smoke:external-video-ppt --',
    `--base-url ${shellQuote('<target-v3-base-url>')}`,
    `--connection-id ${shellQuote(args.connectionId || '<connection-id>')}`,
    `--source-id ${shellQuote(args.sourceId || '<source-id>')}`,
    '--bearer <redacted-inbound-bearer>',
    fixtureArg,
    fixtureNameArg.trim(),
    `--output-dir ${shellQuote(args.outputDir)}`,
  ].filter(Boolean).join(' ');
}

async function localFixturePreflight(args) {
  const fixtureStat = await stat(args.fixtureFile);
  if (!fixtureStat.isFile()) {
    throw new Error('--fixture-file must point to a regular video file');
  }
  return {
    kind: 'file',
    fileName: basename(args.fixtureFile),
    bytes: fixtureStat.size,
    localPathRedacted: true,
  };
}

async function fixturePreflight(args) {
  const sourceKind = args.fixtureFile ? 'file' : 'url';
  const fileName = fixtureFileName(args);
  const contentType = args.contentType || inferContentType(fileName, '');
  const extension = extname(fileName).toLowerCase();
  const mediaKind = inferUploadMediaKind(fileName, contentType);
  const source = sourceKind === 'file'
    ? await localFixturePreflight(args)
    : {
        kind: 'url',
        ...redactedUrlSummary(args.fixtureUrl),
      };
  return {
    sourceKind,
    source,
    fileName,
    extension,
    contentType,
    mediaKind,
    supportedExtension: SUPPORTED_VIDEO_EXTENSIONS.includes(extension),
  };
}

async function runPreflight(args) {
  const runId = makeRunId();
  const fixture = await fixturePreflight(args);
  const ids = buildFixtureIds(runId);
  const payload = buildMessagePayload(args, ids, runId);
  const credentialGateSatisfied = Boolean(args.bearer) || args.allowMissingBearer;
  const liveCredentialReady = Boolean(args.bearer);
  const failures = [
    args.connectionId ? '' : 'missing_connection_id',
    args.sourceId ? '' : 'missing_source_id',
    credentialGateSatisfied ? '' : 'missing_bearer_for_live_external_smoke',
    fixture.mediaKind === 'video' ? '' : 'fixture_not_classified_as_video',
    fixture.supportedExtension ? '' : 'unsupported_video_extension',
    promptRequestsVideoPpt(payload.text) ? '' : 'event_text_does_not_request_video_ppt',
    payload.requested_skills?.some((skill) => skill.skill_id === 'video_ppt_extraction')
      ? ''
      : 'missing_video_ppt_requested_skill',
  ].filter(Boolean);
  const report = {
    schema: 'v3.external_video_ppt_smoke_preflight.v1',
    summary: {
      ok: failures.length === 0,
      preflight: true,
      runId,
      networkCallsRun: false,
      productionWriteAllowed: false,
      liveWriteApprovalRequired: true,
      credentialGateSatisfied,
      liveCredentialReady,
      allowMissingBearer: args.allowMissingBearer,
      fixtureDownloaded: false,
      fixtureRegistered: false,
      eventSent: false,
      replyPolled: false,
      deliverablesDownloadedFromNetwork: false,
      failures,
    },
    target: {
      baseUrl: redactedUrlSummary(args.baseUrl),
      connectionIdPresent: Boolean(args.connectionId),
      sourceIdPresent: Boolean(args.sourceId),
      platform: args.platform,
      tenantExternalIdPresent: Boolean(args.tenantExternalId),
      botExternalIdPresent: Boolean(args.botExternalId),
      senderExternalIdPresent: Boolean(args.senderExternalId),
    },
    fixture,
    trigger: {
      textRequestsVideoPpt: promptRequestsVideoPpt(payload.text),
      defaultPromptGuardsAgainstOrdinaryVideoToPpt:
        /不要把普通视频创作成 PPT/.test(payload.default_prompt),
      requestedSkillIds: payload.requested_skills.map((skill) => skill.skill_id),
      expectedAction: payload.requested_skills[0]?.arguments?.expected_action || null,
      availableDocumentSourcePresent: Boolean(payload.available_document_source_id),
      availableDocumentExternalIdsCount: payload.available_document_external_ids.length,
      datasetExternalIdsCount: payload.dataset_external_ids.length,
    },
    liveWriteScope: {
      requiresExplicitApproval: true,
      plannedSteps: LIVE_WRITE_STEPS,
      writesSmokeRecords: true,
      deploysServices: false,
    },
    redaction: {
      rawFixtureUrlIncluded: false,
      localFixturePathIncluded: false,
      bearerIncluded: false,
      objectKeysIncluded: false,
      providerPayloadsIncluded: false,
    },
    commandTemplate: redactedLiveCommand(args, fixture.fileName),
  };
  const serialized = JSON.stringify(report)
    .replace(/--bearer <redacted-inbound-bearer>/g, '--auth <redacted>');
  if (/https?:\/\/|token=|object_key|Bearer\s|\/Users\//i.test(serialized)) {
    throw new Error('preflight report contains unredacted URL, token, object key, bearer marker, or local path');
  }
  await mkdir(args.outputDir, { recursive: true });
  const reportPath = join(args.outputDir, `${runId}-preflight.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({
    ok: report.summary.ok,
    preflight: true,
    runId,
    reportPath,
    fixture: {
      sourceKind: fixture.sourceKind,
      fileName: fixture.fileName,
      mediaKind: fixture.mediaKind,
      supportedExtension: fixture.supportedExtension,
    },
    credentialGateSatisfied,
    liveCredentialReady,
    liveWriteApprovalRequired: true,
  }, null, 2));
  if (!report.summary.ok) {
    process.exitCode = 1;
  }
}

async function loadFixtureBytes(args) {
  if (args.fixtureFile) {
    const bytes = await readFile(args.fixtureFile);
    return {
      bytes,
      fileName: fixtureFileName(args),
      contentType: args.contentType || inferContentType(args.fixtureFile),
      source: { kind: 'file', fileName: basename(args.fixtureFile), bytes: bytes.length },
    };
  }
  const response = await fetch(args.fixtureUrl);
  if (!response.ok) {
    throw new Error(`fixture download returned HTTP ${response.status}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  const responseContentType = response.headers.get('content-type') || '';
  const fileName = fixtureFileName(args);
  return {
    bytes,
    fileName,
    contentType: args.contentType || responseContentType || inferContentType(fileName),
    source: redactedUrlSummary(args.fixtureUrl),
  };
}

async function withVideoFixtureUrl(args, callback) {
  if (!args.loopbackFixture) {
    return callback({
      contentUrl: args.fixtureUrl,
      fileName: fixtureFileName(args),
      contentType: args.contentType || inferContentType(fixtureFileName(args)),
      allowHttpLoopback: false,
      source: redactedUrlSummary(args.fixtureUrl),
    });
  }

  const fixture = await loadFixtureBytes(args);
  const routePath = `/${encodeURIComponent(fixture.fileName)}`;
  const server = createServer((request, response) => {
    const url = new URL(request.url || '/', 'http://127.0.0.1');
    if (url.pathname !== routePath) {
      response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
      response.end('not found');
      return;
    }
    response.writeHead(200, {
      'content-type': fixture.contentType,
      'content-length': String(fixture.bytes.length),
    });
    response.end(fixture.bytes);
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
    return await callback({
      contentUrl: `http://127.0.0.1:${port}${routePath}`,
      fileName: fixture.fileName,
      contentType: fixture.contentType,
      allowHttpLoopback: true,
      source: fixture.source,
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

function redactedUrlSummary(rawUrl) {
  try {
    const url = new URL(rawUrl);
    return {
      kind: 'url',
      scheme: url.protocol.replace(/:$/, ''),
      host: url.host,
      extension: extname(url.pathname).slice(0, 12),
    };
  } catch {
    return { kind: 'url', scheme: '', host: '', extension: '' };
  }
}

function buildFixtureIds(runId) {
  return {
    datasetExternalId: `dmx-video-ppt-dataset-${runId}`,
    documentExternalId: `dmx-video-ppt-document-${runId}`,
    revisionExternalId: `v-${runId}`,
    conversationExternalId: `conv-video-ppt-${runId}`,
  };
}

async function parseVideoFixture(args, fixture, ids, runId) {
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/documents/parse`,
    normalizeBaseUrl(args.baseUrl),
  );
  const body = {
    source_id: args.sourceId,
    dataset_external_id: ids.datasetExternalId,
    dataset_title: 'DataMax Video PPT Smoke',
    document_external_id: ids.documentExternalId,
    revision_external_id: ids.revisionExternalId,
    title: fixture.fileName,
    content_type: fixture.contentType,
    content_url: fixture.contentUrl,
    metadata: {
      smoke: 'external-video-ppt',
      run_id: runId,
      media_kind: 'video',
      trigger_expected: 'extract_video_ppt_transcript',
    },
    idempotency_key: `external-video-ppt:parse:${runId}`,
    allow_http_loopback: fixture.allowHttpLoopback,
  };
  const result = await requestJson(
    url,
    { method: 'POST', headers: requestHeaders(args), body: JSON.stringify(body) },
    args.timeoutMs,
  );
  if (!result.response.ok) {
    throw new Error(`video parse failed HTTP ${result.response.status}: ${result.text.slice(0, 300)}`);
  }
  return {
    accepted: Boolean(result.data?.accepted),
    documentId: result.data?.document?.id || null,
    workflowStatus: result.data?.workflow_execution?.status || null,
    workflowId: result.data?.workflow_execution?.id || null,
  };
}

async function getParseDetail(args, ids) {
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/documents/${encodeURIComponent(
      ids.documentExternalId,
    )}/parse-detail`,
    normalizeBaseUrl(args.baseUrl),
  );
  url.searchParams.set('source_id', args.sourceId);
  const result = await requestJson(url, { method: 'GET', headers: requestHeaders(args) }, args.timeoutMs);
  if (!result.response.ok) {
    throw new Error(`parse detail failed HTTP ${result.response.status}: ${result.text.slice(0, 300)}`);
  }
  return result.data;
}

function parseDetailStatus(detail) {
  const latest = detail?.latest || null;
  const parseStatus = String(detail?.parse_status || detail?.parseStatus || latest?.parse_status || '');
  const modelStatus = String(detail?.model_status || detail?.modelStatus || latest?.model_status || '');
  const failed = /failed|error|cancel/i.test(`${parseStatus} ${modelStatus}`);
  return {
    registered: Boolean(detail?.document || detail?.latest || parseStatus || modelStatus),
    failed,
    parseStatus: parseStatus || null,
    modelStatus: modelStatus || null,
    chunkCount: Number(detail?.chunk_count ?? detail?.chunkCount ?? latest?.chunk_count ?? 0),
    evidenceCount: Number(
      detail?.retrieval_evidence_count ?? latest?.retrieval_evidence_count ?? 0,
    ),
  };
}

async function waitForParseRegistered(args, ids) {
  const startedAt = Date.now();
  let lastStatus = null;
  while (Date.now() - startedAt <= args.parseTimeoutMs) {
    const detail = await getParseDetail(args, ids);
    lastStatus = parseDetailStatus(detail);
    if (lastStatus.registered && !lastStatus.failed) {
      return lastStatus;
    }
    if (lastStatus.failed) {
      throw new Error(`video parse failed: ${JSON.stringify(lastStatus)}`);
    }
    await sleep(args.pollIntervalMs);
  }
  throw new Error(`video parse did not become registered: ${JSON.stringify(lastStatus)}`);
}

function buildMessagePayload(args, ids, runId) {
  return {
    platform: args.platform,
    tenant_external_id: args.tenantExternalId,
    bot_external_id: args.botExternalId,
    conversation_external_id: ids.conversationExternalId,
    thread_external_id: null,
    sender_external_id: args.senderExternalId,
    sender_display_name: 'DataMax video PPT smoke',
    message_external_id: `msg-video-ppt-${runId}`,
    message_type: 'text',
    text: '请提取这个视频里的 PPT/幻灯片/课件，输出 PPTX、video_slides.md 和交付 manifest。',
    default_prompt: '只处理本轮授权的视频素材；不要把普通视频创作成 PPT。',
    output_format: 'rich_text',
    render_mode: 'normal',
    available_document_source_id: args.sourceId,
    available_document_external_ids: [ids.documentExternalId],
    dataset_external_ids: [ids.datasetExternalId],
    documentExternalId: ids.documentExternalId,
    requested_skills: [{
      skill_id: 'video_ppt_extraction',
      mode: 'preferred',
      arguments: {
        expected_action: 'extract_video_ppt_transcript',
      },
    }],
    mention_external_user_ids: [],
    attachment_refs: [],
    idempotency_key: `external-video-ppt:event:${runId}`,
    received_at: new Date().toISOString(),
    render_options: null,
  };
}

function assertSelfTestExternalContract(args, runId) {
  const ids = buildFixtureIds(runId);
  const payload = buildMessagePayload(args, ids, runId);
  const triggerClassifier = assertVideoPptTriggerClassifier();
  if (!promptRequestsVideoPpt(payload.text)) {
    throw new Error('self-test event text did not request video PPT extraction');
  }
  if (!/不要把普通视频创作成 PPT/.test(payload.default_prompt)) {
    throw new Error('self-test default prompt did not guard against ordinary video-to-PPT generation');
  }
  const requestedSkillIds = payload.requested_skills.map((skill) => skill.skill_id);
  if (!requestedSkillIds.includes('video_ppt_extraction')) {
    throw new Error('self-test payload did not request video_ppt_extraction skill');
  }
  const expectedAction = payload.requested_skills[0]?.arguments?.expected_action || '';
  if (expectedAction !== 'extract_video_ppt_transcript') {
    throw new Error('self-test payload did not preserve extract_video_ppt_transcript action');
  }
  if (
    payload.available_document_source_id !== args.sourceId
    || payload.available_document_external_ids.length !== 1
    || payload.available_document_external_ids[0] !== ids.documentExternalId
    || payload.dataset_external_ids.length !== 1
    || payload.dataset_external_ids[0] !== ids.datasetExternalId
  ) {
    throw new Error('self-test payload did not scope the PPT trigger to the registered video document');
  }

  const supported = SUPPORTED_VIDEO_EXTENSIONS.map((extension) => `sample${extension}`);
  const missedSupportedVideos = supported.filter((fileName) => {
    const contentType = inferContentType(fileName, '');
    return inferUploadMediaKind(fileName, contentType) !== 'video'
      || !SUPPORTED_VIDEO_EXTENSIONS.includes(extname(fileName).toLowerCase());
  });
  if (missedSupportedVideos.length > 0) {
    throw new Error(`self-test external classifier missed supported videos: ${missedSupportedVideos.join(', ')}`);
  }

  const unsupportedNonVideos = ['sample.pdf', 'sample.txt', 'sample.jpg'];
  const falsePositiveNonVideos = unsupportedNonVideos.filter((fileName) =>
    inferUploadMediaKind(fileName, inferContentType(fileName, '')) === 'video',
  );
  if (falsePositiveNonVideos.length > 0) {
    throw new Error(`self-test external classifier misclassified non-video files: ${falsePositiveNonVideos.join(', ')}`);
  }

  const source = redactedUrlSummary(redactionProbeVideoUrl());
  if (
    JSON.stringify(source).includes('redaction_probe')
    || JSON.stringify(source).includes('/private/path')
  ) {
    throw new Error('self-test redacted URL summary leaked path or query data');
  }

  return {
    triggerTextRequestsVideoPpt: true,
    defaultPromptGuardsAgainstOrdinaryVideoToPpt: true,
    requestedSkillIds,
    expectedAction,
    triggerClassifier,
    availableDocumentSourcePresent: true,
    availableDocumentExternalIdsCount: payload.available_document_external_ids.length,
    datasetExternalIdsCount: payload.dataset_external_ids.length,
    supportedVideoExtensions: SUPPORTED_VIDEO_EXTENSIONS,
    unsupportedNonVideoExtensionsRejected: true,
    sourceSummaryRedacted: true,
  };
}

function redactionProbeVideoUrl() {
  return [
    'https',
    '://',
    'example.com',
    '/private/path/video.mp4',
    '?',
    'redaction_probe',
    '=secret',
  ].join('');
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

function replyStatus(reply) {
  return String(reply?.task_status || reply?.card?.status || reply?.reply_type || '').toLowerCase();
}

function replyIsActive(reply) {
  const status = replyStatus(reply);
  return [
    'accepted',
    'queued',
    'processing',
    'running',
    'video_extraction_queued',
    'video_processing',
  ].includes(status);
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
    if (!replyIsActive(latest?.reply)) {
      return latest;
    }
    await sleep(args.pollIntervalMs);
  }
  throw new Error(`reply ${assistantRunId} did not complete: ${JSON.stringify(latest?.reply || null)}`);
}

async function sendMessage(args, payload) {
  const initial = await postEvent(args, payload);
  const assistantRunId = initial?.assistant_run_id || null;
  if (assistantRunId && replyIsActive(initial?.reply)) {
    return pollReply(args, assistantRunId);
  }
  return initial;
}

function replyText(response) {
  const reply = response?.reply || {};
  return [
    reply.text,
    reply.card?.text,
    reply.card?.summary,
    reply.card?.status,
    reply.card?.type,
    ...(Array.isArray(reply.artifact_links) ? reply.artifact_links : []),
  ].filter((value) => typeof value === 'string' && value.trim()).join('\n');
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function downloadExportsFromReply(response) {
  const reply = response?.reply || {};
  const card = reply.card || {};
  return [
    ...asArray(card.download_exports),
    ...asArray(card.downloadExports),
  ];
}

function exportKind(item) {
  const signal = [
    item?.kind,
    item?.file_kind,
    item?.fileKind,
    item?.artifact_kind,
    item?.artifactKind,
    item?.label,
    item?.name,
    item?.file_name,
    item?.fileName,
    item?.url,
  ].map((value) => String(value || '').toLowerCase()).join(' ');
  if (/pptx|powerpoint|presentation|video_slides_screenshot/.test(signal)) return 'pptx';
  if (/video_slides|slides\.md|markdown/.test(signal)) return 'video_slides_markdown';
  if (/final_deliverables_manifest/.test(signal)) return 'final_deliverables_manifest';
  if (/published_deliverable_manifest/.test(signal)) return 'published_deliverable_manifest';
  if (/published_version_history/.test(signal)) return 'published_version_history';
  if (/extraction_artifacts_manifest/.test(signal)) return 'extraction_artifacts_manifest';
  return '';
}

function exportUrl(item) {
  return item?.url || item?.download_url || item?.downloadUrl || item?.href || '';
}

function videoReplySurface(response) {
  const reply = response?.reply || {};
  const card = reply.card || {};
  const text = replyText(response);
  const statusSignal = [
    reply.reply_type,
    reply.task_status,
    card.type,
    card.status,
    text,
  ].map((value) => String(value || '').toLowerCase()).join(' ');
  const exports = downloadExportsFromReply(response);
  const exportKinds = [...new Set(exports.map(exportKind).filter(Boolean))];
  const artifactLinks = asArray(reply.artifact_links).filter((value) => typeof value === 'string');
  const unsupported = /login_gated_video_source_not_supported|direct_video_url_required/.test(statusSignal);
  const videoSignal = /video_extraction|video ppt|ppt|幻灯片|课件|final_pptx_ready/.test(statusSignal)
    || exportKinds.length > 0
    || artifactLinks.length > 0;
  return {
    ok: videoSignal && !unsupported,
    unsupported,
    status: reply.task_status || card.status || reply.reply_type || null,
    cardType: card.type || null,
    exportKinds,
    missingExportKinds: REQUIRED_FILE_KINDS.filter((kind) => !exportKinds.includes(kind)),
    exportCount: exports.length,
    artifactLinkCount: artifactLinks.length,
    textLength: text.length,
  };
}

function assertVideoReplySurface(response) {
  const surface = videoReplySurface(response);
  if (!surface.ok) {
    throw new Error(`third-party reply did not expose video PPT surface: ${JSON.stringify(surface)}`);
  }
  return surface;
}

function resolveDownloadUrl(args, rawUrl) {
  if (!rawUrl) return '';
  try {
    return new URL(rawUrl, normalizeBaseUrl(args.baseUrl)).toString();
  } catch {
    return '';
  }
}

async function downloadDeliverables(args, response, outDir) {
  const exports = downloadExportsFromReply(response)
    .map((item) => ({ item, kind: exportKind(item), url: exportUrl(item) }))
    .filter((entry) => entry.kind && entry.url);
  const selected = [];
  for (const kind of REQUIRED_FILE_KINDS) {
    const entry = exports.find((candidate) => candidate.kind === kind);
    if (entry) selected.push(entry);
  }
  const missing = REQUIRED_FILE_KINDS.filter((kind) => !selected.some((entry) => entry.kind === kind));
  if (missing.length) {
    throw new Error(`reply download_exports missing required kinds: ${missing.join(', ')}`);
  }
  await mkdir(outDir, { recursive: true });
  const downloads = [];
  for (const entry of selected) {
    const url = resolveDownloadUrl(args, entry.url);
    const result = await fetch(url, { headers: requestHeaders(args, null) });
    const bytes = Buffer.from(await result.arrayBuffer());
    const name = safeDownloadName(entry.kind, entry.item);
    const localPath = join(outDir, name);
    await writeFile(localPath, bytes);
    downloads.push({
      kind: entry.kind,
      ok: result.ok,
      status: result.status,
      contentType: result.headers.get('content-type') || '',
      bytes: bytes.length,
      localPath,
      fileName: name,
      prefixHex: bytes.subarray(0, 12).toString('hex'),
    });
  }
  return downloads;
}

function safeDownloadName(kind, item) {
  const raw = item?.file_name || item?.fileName || item?.name || `${kind}.bin`;
  return String(raw)
    .replace(/[^\w.-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    || `${kind}.bin`;
}

function zipCentralDirectoryEntries(buffer) {
  const minEocd = 22;
  const maxComment = 0xffff;
  const start = Math.max(0, buffer.length - minEocd - maxComment);
  let eocdOffset = -1;
  for (let index = buffer.length - minEocd; index >= start; index -= 1) {
    if (buffer.readUInt32LE(index) === 0x06054b50) {
      eocdOffset = index;
      break;
    }
  }
  if (eocdOffset < 0) throw new Error('PPTX ZIP EOCD not found');
  const totalEntries = buffer.readUInt16LE(eocdOffset + 10);
  const centralOffset = buffer.readUInt32LE(eocdOffset + 16);
  const names = [];
  let offset = centralOffset;
  for (let entry = 0; entry < totalEntries; entry += 1) {
    if (offset + 46 > buffer.length || buffer.readUInt32LE(offset) !== 0x02014b50) {
      throw new Error(`invalid central directory entry at ${offset}`);
    }
    const nameLength = buffer.readUInt16LE(offset + 28);
    const extraLength = buffer.readUInt16LE(offset + 30);
    const commentLength = buffer.readUInt16LE(offset + 32);
    const nameStart = offset + 46;
    const nameEnd = nameStart + nameLength;
    names.push(buffer.subarray(nameStart, nameEnd).toString('utf8'));
    offset = nameEnd + extraLength + commentLength;
  }
  return names;
}

async function validateDownloads(downloads) {
  const byKind = Object.fromEntries(downloads.map((download) => [download.kind, download]));
  const missing = REQUIRED_FILE_KINDS.filter((kind) => !byKind[kind]);
  const failed = downloads.filter((download) => !download.ok);
  const validation = {
    missingRequiredKinds: missing,
    failedDownloads: failed.map((download) => ({ kind: download.kind, status: download.status })),
    pptx: null,
    markdown: null,
  };
  const pptx = byKind.pptx;
  if (pptx?.ok) {
    const bytes = await readFile(pptx.localPath);
    const entries = zipCentralDirectoryEntries(bytes);
    const slideEntries = entries.filter((entry) => /^ppt\/slides\/slide\d+\.xml$/.test(entry));
    validation.pptx = {
      zipMagic: bytes.subarray(0, 4).toString('hex') === '504b0304',
      requiredEntriesPresent: Object.fromEntries(
        REQUIRED_PPTX_ENTRIES.map((entry) => [entry, entries.includes(entry)]),
      ),
      slideCount: slideEntries.length,
    };
  }
  const markdown = byKind.video_slides_markdown;
  if (markdown?.ok) {
    const bytes = await readFile(markdown.localPath);
    const text = bytes.toString('utf8');
    validation.markdown = {
      bytes: bytes.length,
      hasTitle: /^# Video Slides:/m.test(text),
      slideHeadingCount: (text.match(/^### Slide\s+\d+:/gm) || []).length,
    };
  }
  validation.ok = missing.length === 0
    && failed.length === 0
    && validation.pptx?.zipMagic === true
    && REQUIRED_PPTX_ENTRIES.every((entry) => validation.pptx?.requiredEntriesPresent?.[entry])
    && validation.pptx.slideCount > 0
    && validation.markdown?.hasTitle === true
    && validation.markdown.slideHeadingCount === validation.pptx.slideCount;
  return validation;
}

function minimalPptxFixtureBytes() {
  return minimalZipBytes([
    '[Content_Types].xml',
    '_rels/.rels',
    'ppt/presentation.xml',
    'ppt/_rels/presentation.xml.rels',
    'ppt/slides/slide1.xml',
    'ppt/slides/_rels/slide1.xml.rels',
    'ppt/notesSlides/notesSlide1.xml',
  ]);
}

function minimalZipBytes(entryNames) {
  const localParts = [];
  const centralParts = [];
  let offset = 0;
  for (const entryName of entryNames) {
    const fileNameBytes = Buffer.from(entryName, 'utf8');
    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4);
    localHeader.writeUInt16LE(0, 6);
    localHeader.writeUInt16LE(0, 8);
    localHeader.writeUInt32LE(0, 10);
    localHeader.writeUInt32LE(0, 14);
    localHeader.writeUInt32LE(0, 18);
    localHeader.writeUInt32LE(0, 22);
    localHeader.writeUInt16LE(fileNameBytes.length, 26);
    localHeader.writeUInt16LE(0, 28);
    localParts.push(localHeader, fileNameBytes);

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);
    centralHeader.writeUInt16LE(20, 6);
    centralHeader.writeUInt16LE(0, 8);
    centralHeader.writeUInt16LE(0, 10);
    centralHeader.writeUInt32LE(0, 12);
    centralHeader.writeUInt32LE(0, 16);
    centralHeader.writeUInt32LE(0, 20);
    centralHeader.writeUInt32LE(0, 24);
    centralHeader.writeUInt16LE(fileNameBytes.length, 28);
    centralHeader.writeUInt16LE(0, 30);
    centralHeader.writeUInt16LE(0, 32);
    centralHeader.writeUInt16LE(0, 34);
    centralHeader.writeUInt16LE(0, 36);
    centralHeader.writeUInt32LE(0, 38);
    centralHeader.writeUInt32LE(offset, 42);
    centralParts.push(centralHeader, fileNameBytes);
    offset += localHeader.length + fileNameBytes.length;
  }

  const centralDirectory = Buffer.concat(centralParts);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(0, 4);
  end.writeUInt16LE(0, 6);
  end.writeUInt16LE(entryNames.length, 8);
  end.writeUInt16LE(entryNames.length, 10);
  end.writeUInt32LE(centralDirectory.length, 12);
  end.writeUInt32LE(offset, 16);
  end.writeUInt16LE(0, 20);
  return Buffer.concat([...localParts, centralDirectory, end]);
}

async function buildSelfTestDownloads(args, runId) {
  const downloadsDir = join(process.cwd(), args.outputDir, `${runId}-downloads`);
  await mkdir(downloadsDir, { recursive: true });
  const payloads = {
    pptx: minimalPptxFixtureBytes(),
    video_slides_markdown: Buffer.from('# Video Slides: Self Test\n\n### Slide 1: 00:00\n\nSelf test slide.\n', 'utf8'),
    final_deliverables_manifest: Buffer.from('{"status":"self_test"}\n', 'utf8'),
    published_deliverable_manifest: Buffer.from('{"status":"self_test"}\n', 'utf8'),
    published_version_history: Buffer.from('{"status":"self_test"}\n', 'utf8'),
    extraction_artifacts_manifest: Buffer.from('{"status":"self_test"}\n', 'utf8'),
  };
  const downloads = [];
  for (const kind of REQUIRED_FILE_KINDS) {
    const bytes = payloads[kind];
    const localPath = join(downloadsDir, safeDownloadName(kind, { file_name: `${kind}.bin` }));
    await writeFile(localPath, bytes);
    downloads.push({
      kind,
      ok: true,
      status: 200,
      contentType: kind === 'pptx' ? 'application/vnd.openxmlformats-officedocument.presentationml.presentation' : 'application/octet-stream',
      bytes: bytes.length,
      localPath,
      fileName: safeDownloadName(kind, { file_name: `${kind}.bin` }),
      prefixHex: bytes.subarray(0, 12).toString('hex'),
    });
  }
  return downloads;
}

async function runSelfTest(args) {
  const runId = makeRunId();
  const contract = assertSelfTestExternalContract(args, runId);
  const response = {
    assistant_run_id: `self-test-${runId}`,
    reply: {
      reply_type: 'card',
      task_status: 'video_extraction_summary',
      artifact_links: ['https://v3.elepcloud.com/generated-artifacts/self-test/index.html'],
      card: {
        type: 'video_extraction_summary',
        status: 'final_pptx_ready',
        download_exports: [
          { kind: 'pptx', url: '/v1/external/channels/generic-chat-main/html-artifacts/a/files/0' },
          { kind: 'video_slides_markdown', url: '/v1/external/channels/generic-chat-main/html-artifacts/a/files/1' },
          { kind: 'final_deliverables_manifest', url: '/v1/external/channels/generic-chat-main/html-artifacts/a/files/2' },
          { kind: 'published_deliverable_manifest', url: '/v1/external/channels/generic-chat-main/html-artifacts/a/files/3' },
          { kind: 'published_version_history', url: '/v1/external/channels/generic-chat-main/html-artifacts/a/files/4' },
          { kind: 'extraction_artifacts_manifest', url: '/v1/external/channels/generic-chat-main/html-artifacts/a/files/5' },
        ],
      },
    },
  };
  const surface = assertVideoReplySurface(response);
  const downloads = await buildSelfTestDownloads(args, runId);
  const downloadValidation = await validateDownloads(downloads);
  if (!downloadValidation.ok) {
    throw new Error(`self-test download validation failed: ${JSON.stringify(downloadValidation)}`);
  }
  const report = {
    summary: {
      ok: true,
      selfTest: true,
      runId,
      networkCallsRun: false,
      fixtureRegistered: false,
      eventSent: false,
      deliverablesDownloadedFromNetwork: false,
    },
    surface,
    contract,
    downloadValidation: {
      ok: downloadValidation.ok,
      pptxSlideCount: downloadValidation.pptx?.slideCount || 0,
      markdownSlideHeadingCount: downloadValidation.markdown?.slideHeadingCount || 0,
      requiredEntriesPresent: downloadValidation.pptx?.requiredEntriesPresent || {},
    },
  };
  const serialized = JSON.stringify(report);
  if (/https?:\/\/|token=|object_key|Bearer\s/i.test(serialized)) {
    throw new Error('self-test report contains unredacted URL, token, object key, or bearer marker');
  }
  await mkdir(args.outputDir, { recursive: true });
  const reportPath = join(args.outputDir, `${runId}-self-test.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify({ ok: true, selfTest: true, runId, reportPath }, null, 2));
}

async function runLive(args) {
  const runId = makeRunId();
  const startedAt = new Date().toISOString();
  const outputDir = join(process.cwd(), args.outputDir, runId);
  const downloadsDir = join(outputDir, 'downloads');
  await mkdir(outputDir, { recursive: true });
  const ids = buildFixtureIds(runId);
  let parseResult = null;
  let parseReady = null;
  let response = null;
  let surface = null;
  let downloads = [];
  let validation = null;
  let fixtureSummary = null;

  await withVideoFixtureUrl(args, async (fixture) => {
    fixtureSummary = {
      fileName: fixture.fileName,
      contentType: fixture.contentType,
      source: fixture.source,
      allowHttpLoopback: fixture.allowHttpLoopback,
    };
    parseResult = await parseVideoFixture(args, fixture, ids, runId);
    parseReady = await waitForParseRegistered(args, ids);
    const payload = buildMessagePayload(args, ids, runId);
    response = await sendMessage(args, payload);
    surface = assertVideoReplySurface(response);
    if (!args.skipDeliverableDownloads) {
      downloads = await downloadDeliverables(args, response, downloadsDir);
      validation = await validateDownloads(downloads);
      if (!validation.ok) {
        throw new Error(`download validation failed: ${JSON.stringify(validation)}`);
      }
    }
  });

  const finishedAt = new Date().toISOString();
  const summary = {
    ok: !validation || validation.ok,
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    sourceId: args.sourceId,
    datasetExternalId: ids.datasetExternalId,
    documentExternalId: ids.documentExternalId,
    conversationExternalId: ids.conversationExternalId,
    assistantRunId: response?.assistant_run_id || null,
    replyStatus: response?.reply?.task_status || response?.reply?.card?.status || null,
    surface,
    downloadedKinds: downloads.map((download) => download.kind),
    pptxSlideCount: validation?.pptx?.slideCount ?? null,
    markdownSlideHeadingCount: validation?.markdown?.slideHeadingCount ?? null,
    startedAt,
    finishedAt,
  };
  const report = {
    summary,
    fixture: fixtureSummary,
    parseResult,
    parseReady,
    reply: {
      assistantRunId: response?.assistant_run_id || null,
      replyType: response?.reply?.reply_type || null,
      taskStatus: response?.reply?.task_status || null,
      cardType: response?.reply?.card?.type || null,
      cardStatus: response?.reply?.card?.status || null,
      artifactLinkCount: surface?.artifactLinkCount || 0,
      exportKinds: surface?.exportKinds || [],
    },
    downloads: downloads.map((download) => ({
      kind: download.kind,
      ok: download.ok,
      status: download.status,
      contentType: download.contentType,
      bytes: download.bytes,
      prefixHex: download.prefixHex,
      fileName: download.fileName,
    })),
    validation,
  };
  const reportPath = join(outputDir, 'report.json');
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify(summary, null, 2));
  console.log(`report=${reportPath}`);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) {
    await runSelfTest(args);
  } else if (args.preflight) {
    await runPreflight(args);
  } else {
    await runLive(args);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
