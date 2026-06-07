#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_CONNECTION_ID = 'generic-chat-main';
const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_EXPECTED_TITLE = '新世界百货经营管理月报表';
const DEFAULT_EXPECTED_FOCUS = '取高机会';
const DEFAULT_TEXT = '请根据当前数据集生成新百经营分析月报，重点看取高机会、销售缺口、哪些门店需要助推；正常回答同时推送报表链接。';
const DEFAULT_PROMPT = '请面向业务用户，优先基于本轮文档和数据源回答。';
const PRIMARY_XINBAI_TEMPLATE_ID = 'xinbai-functional-modular-template-20260604';
const FILE_CHECKS = [
  ['html', 'index.html'],
  ['data', 'data.json'],
  ['snapshot', 'data-snapshot.json'],
  ['table', 'table-data.csv'],
  ['ppt', 'report.ppt'],
  ['markdown', 'report.md'],
];

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    connectionId: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_CONNECTION_ID || DEFAULT_CONNECTION_ID,
    bearer: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_BEARER || '',
    timeoutMs: Number(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    sourceId: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_SOURCE_ID || 'third-party-source-main',
    datasetExternalIds: parseList(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_DATASET_EXTERNAL_IDS),
    documentExternalIds: parseList(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_DOCUMENT_EXTERNAL_IDS),
    text: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_TEXT || DEFAULT_TEXT,
    defaultPrompt: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_DEFAULT_PROMPT || DEFAULT_PROMPT,
    platform: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_PLATFORM || 'generic_chat',
    tenantExternalId: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_TENANT_EXTERNAL_ID || 'tenant-ext-smoke',
    botExternalId: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_BOT_EXTERNAL_ID || 'bot-v3',
    senderExternalId: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_SENDER_EXTERNAL_ID || 'user-report-export-smoke',
    outputDir: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_OUTPUT_DIR || 'target/external-report-export-smoke',
    expectedTitle: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_EXPECTED_TITLE || DEFAULT_EXPECTED_TITLE,
    expectedFocus: process.env.EXTERNAL_REPORT_EXPORT_SMOKE_EXPECTED_FOCUS || DEFAULT_EXPECTED_FOCUS,
    allowEmptyScope: parseBoolean(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_ALLOW_EMPTY_SCOPE),
    allowMissingBearer: parseBoolean(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_ALLOW_MISSING_BEARER),
    skipJson: parseBoolean(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_SKIP_JSON),
    skipStream: parseBoolean(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_SKIP_STREAM),
    skipFileChecks: parseBoolean(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_SKIP_FILE_CHECKS),
    requireTextLink: parseBoolean(process.env.EXTERNAL_REPORT_EXPORT_SMOKE_REQUIRE_TEXT_LINK),
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
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
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
    } else if (arg === '--text') {
      args.text = requireValue(arg, next);
      index += 1;
    } else if (arg === '--default-prompt') {
      args.defaultPrompt = requireValue(arg, next);
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
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--expected-title') {
      args.expectedTitle = requireValue(arg, next);
      index += 1;
    } else if (arg === '--no-expected-title') {
      args.expectedTitle = '';
    } else if (arg === '--expected-focus') {
      args.expectedFocus = requireValue(arg, next);
      index += 1;
    } else if (arg === '--no-expected-focus') {
      args.expectedFocus = '';
    } else if (arg === '--allow-empty-scope') {
      args.allowEmptyScope = true;
    } else if (arg === '--allow-missing-bearer') {
      args.allowMissingBearer = true;
    } else if (arg === '--skip-json') {
      args.skipJson = true;
    } else if (arg === '--skip-stream') {
      args.skipStream = true;
    } else if (arg === '--skip-file-checks') {
      args.skipFileChecks = true;
    } else if (arg === '--no-require-text-link') {
      args.requireTextLink = false;
    } else if (arg === '--require-text-link') {
      args.requireTextLink = true;
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
  if (args.selfTest) {
    args.allowEmptyScope = true;
    args.allowMissingBearer = true;
    args.requireTextLink = true;
    args.skipFileChecks = true;
  }
  if (!args.allowMissingBearer && !args.bearer) {
    throw new Error('--bearer is required unless --allow-missing-bearer is set');
  }
  if (!args.allowEmptyScope && args.datasetExternalIds.length === 0 && args.documentExternalIds.length === 0) {
    throw new Error('--dataset-external-ids or --document-external-ids is required unless --allow-empty-scope is set');
  }
  if (args.skipJson && args.skipStream) {
    throw new Error('at least one of JSON or SSE smoke must run');
  }
  return args;
}

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y'].includes(String(value || '').trim().toLowerCase());
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

function printHelp() {
  console.log(`Usage:
  npm run smoke:external-report-export -- \\
    --base-url https://v3.elepcloud.com \\
    --connection-id generic-chat-main \\
    --bearer <token> \\
    --dataset-external-ids <dataset-id>

Checks:
  - JSON /v1/external/channels/:id/events returns one report link and report card
  - SSE /v1/external/channels/:id/events/stream publishes the same report surface
  - card title defaults to "${DEFAULT_EXPECTED_TITLE}"
  - report URL focus defaults to "${DEFAULT_EXPECTED_FOCUS}" for the default prompt
  - card exposes table_data_url, ppt_download_url, markdown/text download URL
  - download_exports has at least three entries
  - index.html, data.json, data-snapshot.json, table-data.csv, report.ppt, report.md are HTTP 200
  - assistant text has at most one report URL; the customer report link is delivered through artifact/card fields
  - --self-test runs a deterministic offline contract fixture without calling DataMax

Environment aliases:
  EXTERNAL_REPORT_EXPORT_SMOKE_BASE_URL
  EXTERNAL_REPORT_EXPORT_SMOKE_CONNECTION_ID
  EXTERNAL_REPORT_EXPORT_SMOKE_BEARER
  EXTERNAL_REPORT_EXPORT_SMOKE_DATASET_EXTERNAL_IDS
  EXTERNAL_REPORT_EXPORT_SMOKE_DOCUMENT_EXTERNAL_IDS
  EXTERNAL_REPORT_EXPORT_SMOKE_EXPECTED_FOCUS
  EXTERNAL_REPORT_EXPORT_SMOKE_REQUIRE_TEXT_LINK
`);
}

function buildPayload(args, mode, runId) {
  const suffix = `${runId}-${mode}`;
  return {
    platform: args.platform,
    tenant_external_id: args.tenantExternalId,
    bot_external_id: args.botExternalId,
    conversation_external_id: `conv-report-export-${suffix}`,
    thread_external_id: null,
    sender_external_id: args.senderExternalId,
    sender_display_name: 'DataMax report export smoke',
    message_external_id: `msg-report-export-${suffix}`,
    message_type: 'text',
    text: args.text,
    default_prompt: args.defaultPrompt,
    output_format: 'rich_text',
    render_mode: 'artifact',
    artifact_type: 'static_page',
    available_document_source_id: args.sourceId || null,
    available_document_external_ids: args.documentExternalIds,
    dataset_external_ids: args.datasetExternalIds,
    documentExternalId: null,
    requested_skills: [],
    mention_external_user_ids: [],
    attachment_refs: [],
    idempotency_key: `external-report-export:${suffix}`,
    received_at: new Date().toISOString(),
    render_options: null,
  };
}

function buildSelfTestReportSurface(args) {
  const focus = args.expectedFocus || DEFAULT_EXPECTED_FOCUS;
  const publicUrl = `${normalizeBaseUrl(args.baseUrl)}/generated-artifacts/database-static-pages/${PRIMARY_XINBAI_TEMPLATE_ID}/index.html?focus=${encodeURIComponent(focus)}`;
  const tableUrl = siblingUrl(publicUrl, 'table-data.csv');
  const pptUrl = siblingUrl(publicUrl, 'report.ppt');
  const markdownUrl = siblingUrl(publicUrl, 'report.md');
  return {
    reply: {
      reply_type: 'artifact_link',
      task_status: 'static_page_published',
      text: `已基于当前经营数据保留正常回答：取高机会和销售缺口需要优先关注。页面链接：[点击查看报表](${publicUrl})`,
      artifact_links: [publicUrl],
      card: {
        type: 'v3_static_page_report',
        title: args.expectedTitle || DEFAULT_EXPECTED_TITLE,
        public_url: publicUrl,
        generated_artifact_url: publicUrl,
        artifact_links: [publicUrl],
        table_data_url: tableUrl,
        ppt_download_url: pptUrl,
        markdown_download_url: markdownUrl,
        download_exports: [
          { kind: 'table', label: 'table-data.csv', url: tableUrl },
          { kind: 'ppt', label: 'report.ppt', url: pptUrl },
          { kind: 'markdown', label: 'report.md', url: markdownUrl },
        ],
      },
    },
  };
}

async function runSelfTest(args) {
  const runId = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
  const surface = buildSelfTestReportSurface(args);
  const results = [];
  for (const mode of ['json', 'stream']) {
    const analysis = await analyzeReportSurface(args, [surface], {
      mode,
      httpStatus: 200,
      latencyMs: 0,
    });
    const primaryTemplateUsed = Boolean(analysis.publicUrl?.includes(`/${PRIMARY_XINBAI_TEMPLATE_ID}/`));
    const fallbackTemplateUsed = /fallback|prewarm|smoke/i.test(analysis.publicUrl || '');
    const normalAnswerPreserved = surface.reply.text.includes('保留正常回答')
      && surface.reply.text.includes('取高机会')
      && !/正在处理|稍后查看|已收到/.test(surface.reply.text);
    results.push({
      mode,
      ok: analysis.ok && primaryTemplateUsed && !fallbackTemplateUsed && normalAnswerPreserved,
      ...analysis,
      primaryTemplateUsed,
      fallbackTemplateUsed,
      normalAnswerPreserved,
    });
  }
  const summary = {
    runId,
    selfTest: true,
    modeCount: results.length,
    okCount: results.filter((item) => item.ok).length,
    failedCount: results.filter((item) => !item.ok).length,
    expectedTitle: args.expectedTitle || null,
    expectedFocus: args.expectedFocus || null,
    primaryTemplateId: PRIMARY_XINBAI_TEMPLATE_ID,
    singleReportLinkRequired: true,
    exportFilesRequired: ['table-data.csv', 'report.ppt', 'report.md'],
    dataMaxCalled: false,
    bearerConfigured: false,
    generatedAt: new Date().toISOString(),
  };
  const outputDir = join(process.cwd(), args.outputDir);
  await mkdir(outputDir, { recursive: true });
  const report = { summary, results };
  const reportPath = join(outputDir, `${runId}-self-test.json`);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');

  console.log(JSON.stringify(summary, null, 2));
  console.log(`report=${reportPath}`);

  if (summary.failedCount > 0) {
    process.exitCode = 1;
  }
}

function requestHeaders(args, stream = false) {
  const headers = {
    'content-type': 'application/json',
  };
  if (stream) {
    headers.accept = 'text/event-stream';
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

async function runJsonSmoke(args, runId) {
  const startedAt = Date.now();
  const payload = buildPayload(args, 'json', runId);
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/events`,
    normalizeBaseUrl(args.baseUrl),
  );
  try {
    const submit = await requestJson(
      url,
      { method: 'POST', headers: requestHeaders(args), body: JSON.stringify(payload) },
      args.timeoutMs,
    );
    const analysis = await analyzeReportSurface(args, [submit.data], {
      mode: 'json',
      httpStatus: submit.response.status,
      latencyMs: Date.now() - startedAt,
    });
    return {
      mode: 'json',
      ok: submit.response.ok && analysis.ok,
      httpStatus: submit.response.status,
      latencyMs: Date.now() - startedAt,
      conversationExternalId: payload.conversation_external_id,
      messageExternalId: payload.message_external_id,
      idempotencyKey: payload.idempotency_key,
      bodyPrefix: submit.response.ok ? '' : submit.text.slice(0, 300),
      ...analysis,
    };
  } catch (error) {
    return failedResult('json', payload, Date.now() - startedAt, error);
  }
}

async function runStreamSmoke(args, runId) {
  const startedAt = Date.now();
  const payload = buildPayload(args, 'stream', runId);
  const url = new URL(
    `/v1/external/channels/${encodeURIComponent(args.connectionId)}/events/stream`,
    normalizeBaseUrl(args.baseUrl),
  );
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), args.timeoutMs);
  const frames = [];
  let httpStatus = null;
  let bodyPrefix = '';

  try {
    const response = await fetch(url, {
      method: 'POST',
      headers: requestHeaders(args, true),
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
    httpStatus = response.status;
    if (!response.ok || !response.body) {
      bodyPrefix = (await response.text()).slice(0, 300);
      const analysis = await analyzeReportSurface(args, [], {
        mode: 'stream',
        httpStatus,
        latencyMs: Date.now() - startedAt,
      });
      return {
        mode: 'stream',
        ok: false,
        httpStatus,
        latencyMs: Date.now() - startedAt,
        conversationExternalId: payload.conversation_external_id,
        messageExternalId: payload.message_external_id,
        idempotencyKey: payload.idempotency_key,
        bodyPrefix,
        eventCount: 0,
        terminalEvent: null,
        ...analysis,
      };
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    let done = false;
    while (!done) {
      const chunk = await reader.read();
      done = chunk.done;
      buffer += decoder.decode(chunk.value || new Uint8Array(), { stream: !done });
      const blocks = buffer.split(/\r?\n\r?\n/);
      buffer = blocks.pop() || '';
      for (const block of blocks) {
        const frame = parseSseBlock(block);
        if (!frame) {
          continue;
        }
        frames.push(frame);
        if (isTerminalFrame(frame)) {
          done = true;
          try {
            await reader.cancel();
          } catch {
            // The stream may already be closed after a terminal event.
          }
          break;
        }
      }
    }
    if (buffer.trim()) {
      const frame = parseSseBlock(buffer);
      if (frame) {
        frames.push(frame);
      }
    }

    const analysis = await analyzeReportSurface(args, frames.map((frame) => frame.data), {
      mode: 'stream',
      httpStatus,
      latencyMs: Date.now() - startedAt,
    });
    const terminalEvent = frames.findLast((frame) => isTerminalFrame(frame))?.event || null;
    return {
      mode: 'stream',
      ok: response.ok && analysis.ok,
      httpStatus,
      latencyMs: Date.now() - startedAt,
      conversationExternalId: payload.conversation_external_id,
      messageExternalId: payload.message_external_id,
      idempotencyKey: payload.idempotency_key,
      bodyPrefix,
      eventCount: frames.length,
      terminalEvent,
      compactEvents: frames.map(compactFrame),
      ...analysis,
    };
  } catch (error) {
    return failedResult('stream', payload, Date.now() - startedAt, error, {
      httpStatus,
      bodyPrefix,
      eventCount: frames.length,
      terminalEvent: frames.findLast((frame) => isTerminalFrame(frame))?.event || null,
      compactEvents: frames.map(compactFrame),
    });
  } finally {
    clearTimeout(timeout);
  }
}

function failedResult(mode, payload, latencyMs, error, extras = {}) {
  return {
    mode,
    ok: false,
    httpStatus: extras.httpStatus ?? null,
    latencyMs,
    conversationExternalId: payload.conversation_external_id,
    messageExternalId: payload.message_external_id,
    idempotencyKey: payload.idempotency_key,
    bodyPrefix: extras.bodyPrefix ?? '',
    eventCount: extras.eventCount ?? null,
    terminalEvent: extras.terminalEvent ?? null,
    compactEvents: extras.compactEvents ?? [],
    replyType: null,
    taskStatus: null,
    title: null,
    publicUrl: null,
    publicUrlFocus: null,
    artifactLinkCount: 0,
    downloadExportsCount: 0,
    exportUrls: {},
    textMetrics: emptyTextMetrics(),
    fileChecks: [],
    checks: [],
    errors: [error instanceof Error ? error.message : String(error)],
  };
}

async function analyzeReportSurface(args, values, metadata) {
  const responses = values.flatMap((value) => findObjectsWithKey(value, 'reply'));
  const response = responses[0] || {};
  const reply = response.reply || {};
  const replyValues = response.reply ? [reply, reply.card].filter(Boolean) : [];
  const allValues = [...values, ...responses, ...replyValues].filter(Boolean);
  const title = firstString(collectValuesForKeys(allValues, [
    'title',
    'report_title',
    'reportTitle',
    'display_title',
    'displayTitle',
  ]));
  const artifactLinks = uniqueStrings([
    ...collectExplicitUrls(allValues, [
      'artifact_links',
      'artifactLinks',
      'public_url',
      'publicUrl',
      'generated_artifact_url',
      'generatedArtifactUrl',
      'download_url',
      'downloadUrl',
      'html_download_url',
      'htmlDownloadUrl',
      'html_preview_url',
      'htmlPreviewUrl',
    ], args.baseUrl),
  ]);
  const publicUrl = artifactLinks.find((item) => /\/index\.html(\?|#|$)/.test(item)) || artifactLinks[0] || null;
  const publicUrlFocus = focusFromUrl(publicUrl);
  const exportUrls = inferExportUrls(args.baseUrl, publicUrl, allValues);
  const downloadExports = collectDownloadExports(allValues, args.baseUrl);
  const answerTextCandidates = [
    ...values.flatMap((item) => [
      item?.text,
      item?.answer,
      item?.content,
      item?.output_text,
      item?.outputText,
      ...collectValuesForKeys(item, ['answer_text', 'answerText']),
    ]),
    ...responses.flatMap((item) => {
      const itemReply = item.reply || {};
      return [
        itemReply.text,
        itemReply.answer,
        itemReply.content,
        item.output_text,
        item.outputText,
        ...collectValuesForKeys(itemReply, ['answer_text', 'answerText']),
      ];
    }),
  ].map((item) => (typeof item === 'string' ? item.trim() : ''))
    .filter(Boolean);
  const answerText = answerTextCandidates.find((item) => analyzeAnswerText(item).reportLinkPresent)
    || answerTextCandidates.at(-1)
    || '';
  const textMetrics = analyzeAnswerText(answerText);
  const fileChecks = args.skipFileChecks || !publicUrl
    ? []
    : await checkArtifactFiles(args.baseUrl, publicUrl, exportUrls, args.timeoutMs);

  const checks = [];
  addCheck(checks, 'http_ok', metadata.httpStatus === null || (metadata.httpStatus >= 200 && metadata.httpStatus < 300));
  addCheck(checks, 'reply_surface_present', responses.length > 0 || artifactLinks.length > 0 || Boolean(title));
  addCheck(checks, 'public_url_present', Boolean(publicUrl));
  addCheck(checks, 'artifact_link_present', artifactLinks.length > 0);
  addCheck(checks, 'single_text_report_url', textMetrics.rawUrlMentionCount <= 1);
  addCheck(checks, 'single_text_markdown_link', textMetrics.markdownLinkCount <= 1);
  addCheck(checks, 'text_link_present', !args.requireTextLink || textMetrics.reportLinkPresent);
  addCheck(checks, 'download_exports_present', downloadExports.length >= 3);
  addCheck(checks, 'table_data_url_present', Boolean(exportUrls.table));
  addCheck(checks, 'ppt_download_url_present', Boolean(exportUrls.ppt));
  addCheck(checks, 'markdown_download_url_present', Boolean(exportUrls.markdown));
  if (args.expectedTitle) {
    addCheck(checks, 'expected_title', title === args.expectedTitle);
  }
  if (args.expectedFocus) {
    addCheck(checks, 'expected_focus', publicUrlFocus === args.expectedFocus);
  }
  for (const check of fileChecks) {
    addCheck(checks, `${check.kind}_file_ok`, check.ok);
  }
  const errors = checks.filter((check) => !check.ok).map((check) => check.name);

  return {
    ok: errors.length === 0,
    replyType: reply.reply_type
      || reply.replyType
      || firstString(collectValuesForKeys(allValues, ['reply_type', 'replyType']))
      || null,
    taskStatus: reply.task_status
      || reply.taskStatus
      || firstString(collectValuesForKeys(allValues, ['task_status', 'taskStatus']))
      || null,
    title,
    publicUrl,
    publicUrlFocus,
    artifactLinkCount: artifactLinks.length,
    downloadExportsCount: downloadExports.length,
    exportUrls,
    textMetrics,
    fileChecks,
    checks,
    errors,
  };
}

function inferExportUrls(baseUrl, publicUrl, values) {
  const urls = {
    data: firstUrl(values, ['data_url', 'dataUrl'], baseUrl),
    table: firstUrl(values, ['table_data_url', 'tableDataUrl', 'csv_url', 'csvUrl'], baseUrl),
    ppt: firstUrl(values, ['ppt_download_url', 'pptDownloadUrl', 'ppt_url', 'pptUrl'], baseUrl),
    markdown: firstUrl(values, [
      'markdown_download_url',
      'markdownDownloadUrl',
      'text_download_url',
      'textDownloadUrl',
      'md_url',
      'mdUrl',
    ], baseUrl),
  };
  if (publicUrl) {
    urls.data ||= siblingUrl(publicUrl, 'data.json');
    urls.table ||= siblingUrl(publicUrl, 'table-data.csv');
    urls.ppt ||= siblingUrl(publicUrl, 'report.ppt');
    urls.markdown ||= siblingUrl(publicUrl, 'report.md');
  }
  return urls;
}

async function checkArtifactFiles(baseUrl, publicUrl, exportUrls, timeoutMs) {
  const urlByKind = {
    html: publicUrl,
    data: exportUrls.data || siblingUrl(publicUrl, 'data.json'),
    snapshot: siblingUrl(publicUrl, 'data-snapshot.json'),
    table: exportUrls.table || siblingUrl(publicUrl, 'table-data.csv'),
    ppt: exportUrls.ppt || siblingUrl(publicUrl, 'report.ppt'),
    markdown: exportUrls.markdown || siblingUrl(publicUrl, 'report.md'),
  };
  const checks = [];
  for (const [kind, fallbackName] of FILE_CHECKS) {
    const url = normalizeUrl(urlByKind[kind] || siblingUrl(publicUrl, fallbackName), baseUrl);
    const startedAt = Date.now();
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const response = await fetch(url, { method: 'GET', signal: controller.signal });
      const body = await response.arrayBuffer();
      checks.push({
        kind,
        url,
        ok: response.ok && body.byteLength > 0,
        httpStatus: response.status,
        byteLength: body.byteLength,
        latencyMs: Date.now() - startedAt,
      });
    } catch (error) {
      checks.push({
        kind,
        url,
        ok: false,
        httpStatus: null,
        byteLength: 0,
        latencyMs: Date.now() - startedAt,
        error: error instanceof Error ? error.message : String(error),
      });
    } finally {
      clearTimeout(timeout);
    }
  }
  return checks;
}

function collectDownloadExports(values, baseUrl) {
  const exports = [];
  for (const value of collectValuesForKeys(values, ['download_exports', 'downloadExports'])) {
    if (!Array.isArray(value)) {
      continue;
    }
    for (const item of value) {
      if (!item || typeof item !== 'object') {
        continue;
      }
      const url = normalizeUrl(item.url || item.href || item.download_url || item.downloadUrl, baseUrl);
      if (!url) {
        continue;
      }
      exports.push({
        label: item.label || item.title || item.name || null,
        kind: item.kind || item.type || null,
        url,
      });
    }
  }
  return uniqueBy(exports, (item) => item.url);
}

function analyzeAnswerText(text) {
  if (!text) {
    return emptyTextMetrics();
  }
  const markdownLinks = text.match(/\[[^\]]+\]\(https?:\/\/[^)]+\)/g) || [];
  const rawUrls = text.match(/https?:\/\/[^\s)）]+/g) || [];
  return {
    answerCharCount: text.length,
    markdownLinkCount: markdownLinks.length,
    rawUrlMentionCount: rawUrls.length,
    reportLinkPresent: markdownLinks.length > 0 || /点击查看报表|打开报表|查看报表/.test(text),
  };
}

function emptyTextMetrics() {
  return {
    answerCharCount: 0,
    markdownLinkCount: 0,
    rawUrlMentionCount: 0,
    reportLinkPresent: false,
  };
}

function parseSseBlock(block) {
  const frame = { event: 'message', id: null, data: null };
  const dataLines = [];
  for (const rawLine of block.split(/\r?\n/)) {
    const line = rawLine.trimEnd();
    if (!line || line.startsWith(':')) {
      continue;
    }
    const separator = line.indexOf(':');
    const key = separator >= 0 ? line.slice(0, separator) : line;
    const rawValue = separator >= 0 ? line.slice(separator + 1) : '';
    const value = rawValue.startsWith(' ') ? rawValue.slice(1) : rawValue;
    if (key === 'event') {
      frame.event = value || 'message';
    } else if (key === 'id') {
      frame.id = value || null;
    } else if (key === 'data') {
      dataLines.push(value);
    }
  }
  if (dataLines.length === 0) {
    return null;
  }
  const dataText = dataLines.join('\n');
  try {
    frame.data = JSON.parse(dataText);
  } catch {
    frame.data = dataText;
  }
  return frame;
}

function isTerminalFrame(frame) {
  const event = String(frame.event || '');
  if (/completed|failed|cancelled|error/i.test(event)) {
    return true;
  }
  const candidates = collectValuesForKeys(frame.data, [
    'status',
    'task_status',
    'taskStatus',
    'reply_type',
    'replyType',
  ]);
  return candidates.some((item) => /static_page_published|completed|failed|cancelled|needs_human/i.test(String(item || '')));
}

function compactFrame(frame) {
  const data = frame.data && typeof frame.data === 'object' ? frame.data : {};
  const responses = findObjectsWithKey(data, 'reply');
  const reply = responses[0]?.reply || {};
  return {
    event: frame.event,
    id: frame.id,
    sequence: firstString(collectValuesForKeys(data, ['sequence'])),
    phase: firstString(collectValuesForKeys(data, ['phase'])),
    status: firstString(collectValuesForKeys(data, ['status'])),
    taskStatus: reply.task_status || reply.taskStatus || firstString(collectValuesForKeys(data, ['task_status', 'taskStatus'])),
    replyType: reply.reply_type || reply.replyType || firstString(collectValuesForKeys(data, ['reply_type', 'replyType'])),
    publicUrl: firstUrl([data], [
      'public_url',
      'publicUrl',
      'generated_artifact_url',
      'generatedArtifactUrl',
      'artifact_links',
      'artifactLinks',
    ], ''),
  };
}

function addCheck(checks, name, ok) {
  checks.push({ name, ok: Boolean(ok) });
}

function firstUrl(values, keys, baseUrl) {
  return collectExplicitUrls(values, keys, baseUrl)[0] || null;
}

function collectExplicitUrls(values, keys, baseUrl) {
  return uniqueStrings(
    collectValuesForKeys(values, keys)
      .flatMap((value) => flattenValues(value))
      .map((value) => normalizeUrl(value, baseUrl))
      .filter(Boolean),
  );
}

function collectValuesForKeys(value, keys, depth = 0) {
  if (!value || depth > 10) {
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

function findObjectsWithKey(value, key, depth = 0) {
  if (!value || depth > 10) {
    return [];
  }
  if (Array.isArray(value)) {
    return value.flatMap((item) => findObjectsWithKey(item, key, depth + 1));
  }
  if (typeof value !== 'object') {
    return [];
  }
  const own = Object.prototype.hasOwnProperty.call(value, key) ? [value] : [];
  return [
    ...own,
    ...Object.values(value).flatMap((child) => findObjectsWithKey(child, key, depth + 1)),
  ];
}

function flattenValues(value, depth = 0) {
  if (value === null || value === undefined || depth > 5) {
    return [];
  }
  if (Array.isArray(value)) {
    return value.flatMap((item) => flattenValues(item, depth + 1));
  }
  if (typeof value === 'object') {
    return Object.values(value).flatMap((item) => flattenValues(item, depth + 1));
  }
  return [value];
}

function firstString(values) {
  return values
    .map((value) => (value === null || value === undefined ? '' : String(value).trim()))
    .find(Boolean) || null;
}

function uniqueStrings(values) {
  return [...new Set(values.map((value) => String(value)).filter(Boolean))];
}

function uniqueBy(values, keyFn) {
  const seen = new Set();
  const output = [];
  for (const value of values) {
    const key = keyFn(value);
    if (!key || seen.has(key)) {
      continue;
    }
    seen.add(key);
    output.push(value);
  }
  return output;
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function normalizeUrl(value, baseUrl) {
  const text = String(value || '').trim();
  if (!text) {
    return null;
  }
  if (text.startsWith('http://') || text.startsWith('https://')) {
    return text;
  }
  if (text.startsWith('/')) {
    return `${normalizeBaseUrl(baseUrl)}${text}`;
  }
  return null;
}

function focusFromUrl(value) {
  if (!value) {
    return null;
  }
  try {
    const url = new URL(value);
    const focus = url.searchParams.get('focus');
    return focus && focus.trim() ? focus.trim() : null;
  } catch {
    return null;
  }
}

function siblingUrl(publicUrl, fileName) {
  try {
    const url = new URL(publicUrl);
    url.search = '';
    url.hash = '';
    const parts = url.pathname.split('/');
    parts[parts.length - 1] = fileName;
    url.pathname = parts.join('/');
    return url.toString();
  } catch {
    return null;
  }
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
  if (args.selfTest) {
    await runSelfTest(args);
    return;
  }
  const runId = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
  const results = [];
  if (!args.skipJson) {
    results.push(await runJsonSmoke(args, runId));
  }
  if (!args.skipStream) {
    results.push(await runStreamSmoke(args, runId));
  }

  const latencies = results.map((item) => item.latencyMs).filter((value) => Number.isFinite(value));
  const summary = {
    runId,
    baseUrl: args.baseUrl,
    connectionId: args.connectionId,
    modeCount: results.length,
    okCount: results.filter((item) => item.ok).length,
    failedCount: results.filter((item) => !item.ok).length,
    datasetExternalIdCount: args.datasetExternalIds.length,
    documentExternalIdCount: args.documentExternalIds.length,
    bearerConfigured: Boolean(args.bearer),
    expectedTitle: args.expectedTitle || null,
    expectedFocus: args.expectedFocus || null,
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
