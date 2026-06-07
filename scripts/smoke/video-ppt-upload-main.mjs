#!/usr/bin/env node

import { basename, extname, join } from 'node:path';
import { mkdir, readFile, writeFile } from 'node:fs/promises';

const DEFAULT_BASE_URL = 'https://v3.elepcloud.com';
const DEFAULT_FIXTURE_URL =
  'https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4';
const DEFAULT_FIXTURE_NAME = 'react-in-5-minutes-upload-smoke.mp4';
const DEFAULT_CONTENT_TYPE = 'video/mp4';
const DEFAULT_DATASET_KEY = 'datamax-video-ppt-smoke';
const DEFAULT_DATASET_TITLE = 'DataMax 视频 PPT Smoke';
const DEFAULT_TIMEOUT_MS = 300_000;
const DEFAULT_POLL_INTERVAL_MS = 5_000;
const DEFAULT_OUTPUT_DIR = 'target/video-ppt-upload-main-smoke';
const DEFAULT_PROMPT =
  '请提取刚上传视频里的 PPT/幻灯片/课件，输出 PPTX、video_slides.md 和交付 manifest。';
const REQUIRED_FILE_KINDS = [
  'pptx',
  'video_slides_markdown',
  'final_deliverables_manifest',
  'published_deliverable_manifest',
  'published_version_history',
  'extraction_artifacts_manifest',
];
const DOWNLOAD_FILE_KINDS = new Set(REQUIRED_FILE_KINDS);
const REQUIRED_PPTX_ENTRIES = [
  '[Content_Types].xml',
  'ppt/presentation.xml',
  'ppt/slides/slide1.xml',
];

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    fixtureUrl: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_FIXTURE_URL || DEFAULT_FIXTURE_URL,
    fixtureFile: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_FIXTURE_FILE || '',
    fixtureName: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_FIXTURE_NAME || '',
    contentType: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_CONTENT_TYPE || '',
    localThreadId:
      process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_LOCAL_THREAD_ID
      || `video-ppt-upload-main-${Date.now()}`,
    prompt: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_PROMPT || DEFAULT_PROMPT,
    datasetId: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_DATASET_ID || '',
    datasetKey: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_DATASET_KEY || DEFAULT_DATASET_KEY,
    datasetTitle: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_DATASET_TITLE || DEFAULT_DATASET_TITLE,
    cookie: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_COOKIE || '',
    bearer: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_BEARER || '',
    selfTest: parseBoolean(process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_SELF_TEST),
    timeoutMs: Number(process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    pollIntervalMs: Number(
      process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS,
    ),
    outputDir: process.env.VIDEO_PPT_UPLOAD_MAIN_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--base-url') {
      args.baseUrl = requireValue(arg, next);
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
    } else if (arg === '--local-thread-id') {
      args.localThreadId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--prompt') {
      args.prompt = requireValue(arg, next);
      index += 1;
    } else if (arg === '--dataset-id') {
      args.datasetId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--dataset-key') {
      args.datasetKey = requireValue(arg, next);
      index += 1;
    } else if (arg === '--dataset-title') {
      args.datasetTitle = requireValue(arg, next);
      index += 1;
    } else if (arg === '--cookie') {
      args.cookie = requireValue(arg, next);
      index += 1;
    } else if (arg === '--bearer') {
      args.bearer = requireValue(arg, next);
      index += 1;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--poll-interval-ms') {
      args.pollIntervalMs = Number(requireValue(arg, next));
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

  if (!args.fixtureFile && !args.fixtureUrl) {
    throw new Error('provide --fixture-url or --fixture-file');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 10_000) {
    throw new Error('--timeout-ms must be at least 10000');
  }
  if (!Number.isInteger(args.pollIntervalMs) || args.pollIntervalMs < 500) {
    throw new Error('--poll-interval-ms must be at least 500');
  }
  return args;
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:video-ppt-upload-main -- --self-test

  npm run smoke:video-ppt-upload-main -- \\
    --base-url https://v3.elepcloud.com \\
    --fixture-url https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4 \\
    --local-thread-id video-ppt-upload-main-...

Checks:
  - creates or reuses a local-thread scoped smoke dataset
  - uploads a public fixture video through /api/v3/local-document-uploads
  - registers and ingests the uploaded video document through /api/v3/documents
  - creates a scoped assistant run that explicitly asks to extract PPT/slides/courseware
  - waits for video_extraction_summary to reach final_pptx_ready
  - downloads PPTX/Markdown/manifests through /api/v3/html-artifacts/{id}/files/{index}
  - validates PPTX OOXML entries plus Markdown/PPTX slide-count agreement

Notes:
  - --self-test does not call the network, upload files, create datasets, or create assistant runs
`);
}

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y', 'on'].includes(String(value || '').trim().toLowerCase());
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function requestHeaders(args, {
  accept = 'application/json',
  contentType = 'application/json',
} = {}) {
  const headers = {
    accept,
    'x-ai-data-platform-local-thread-id': args.localThreadId,
  };
  if (contentType) {
    headers['content-type'] = contentType;
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

function fetchWithTimeout(url, options, timeoutMs) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  return fetch(url, { ...options, signal: controller.signal })
    .finally(() => clearTimeout(timeout));
}

async function requestJson(args, path, {
  method = 'GET',
  body = undefined,
  timeoutMs = args.timeoutMs,
} = {}) {
  const response = await fetchWithTimeout(
    `${normalizeBaseUrl(args.baseUrl)}${path}`,
    {
      method,
      headers: requestHeaders(args),
      body: body === undefined ? undefined : JSON.stringify(body),
    },
    timeoutMs,
  );
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${method} ${path} returned HTTP ${response.status}: ${text.slice(0, 240)}`);
  }
  return text ? JSON.parse(text) : {};
}

async function requestForm(args, path, formData) {
  const response = await fetchWithTimeout(
    `${normalizeBaseUrl(args.baseUrl)}${path}`,
    {
      method: 'POST',
      headers: requestHeaders(args, { contentType: null }),
      body: formData,
    },
    args.timeoutMs,
  );
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`POST ${path} returned HTTP ${response.status}: ${text.slice(0, 240)}`);
  }
  return text ? JSON.parse(text) : {};
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

function buildSelectedScope(dataset) {
  const datasetId = dataset?.id || '';
  return {
    mode: 'user_selected',
    datasets: datasetId ? [datasetId] : [],
    selected: datasetId ? [{ type: 'dataset', id: datasetId }] : [],
    conversation_memory: [],
    intent: 'video_ppt_extraction',
    supply_policy: {
      intent: 'video_ppt_extraction',
      historyPolicy: 'intent_gated_selected',
      retrievalPolicy: datasetId ? 'standard' : 'not_requested',
      preferDetail: true,
      noFakeData: true,
    },
  };
}

function buildScopeCandidate(dataset) {
  return {
    type: 'dataset',
    id: dataset?.id || '',
    label: dataset?.title || dataset?.key || 'DataMax video upload smoke dataset',
    confidence: 'high',
    reason: 'Uploaded video smoke dataset for video PPT extraction validation.',
    source: 'video_ppt_upload_main_smoke',
  };
}

async function createAssistantRun(args, dataset, document) {
  const startedAt = Date.now();
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
    const response = await fetchWithTimeout(
      `${normalizeBaseUrl(args.baseUrl)}/api/v3/assistant-runs/stream`,
      {
        method: 'POST',
        headers: requestHeaders(args, {
          accept: 'text/event-stream',
          contentType: 'application/json',
        }),
        body: JSON.stringify({
          prompt: args.prompt,
          local_thread_id: args.localThreadId,
          selected_scope: buildSelectedScope(dataset),
          scope_candidates: [buildScopeCandidate(dataset)].filter((candidate) => candidate.id),
          startup_briefing: {
            source: 'video_ppt_upload_main_smoke',
            selected_dataset: dataset?.id
              ? { id: dataset.id, title: dataset.title || dataset.key || '' }
              : null,
            uploaded_document: document?.id
              ? { id: document.id, title: document.title || '' }
              : null,
          },
          messages: [{ role: 'user', content: args.prompt }],
        }),
      },
      args.timeoutMs,
    );
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
  }
}

function datasetMatches(dataset, args) {
  return dataset?.id === args.datasetId
    || dataset?.key === args.datasetKey
    || dataset?.title === args.datasetTitle;
}

async function ensureSmokeDataset(args) {
  const listed = await requestJson(args, '/api/v3/datasets');
  const existing = (Array.isArray(listed) ? listed : []).find((dataset) => datasetMatches(dataset, args));
  if (existing?.id) {
    return { dataset: existing, created: false };
  }
  if (args.datasetId) {
    throw new Error(`dataset id ${args.datasetId} was not visible to this smoke scope`);
  }

  const body = {
    key: args.datasetKey,
    title: args.datasetTitle,
    description: 'DataMax video PPT upload smoke dataset. Created by controlled smoke script.',
    local_only: true,
    local_thread_id: args.localThreadId,
  };
  try {
    const created = await requestJson(args, '/api/v3/datasets', { method: 'POST', body });
    return { dataset: created, created: true };
  } catch (error) {
    const latest = await requestJson(args, '/api/v3/datasets');
    const fallback = (Array.isArray(latest) ? latest : []).find((dataset) => datasetMatches(dataset, args));
    if (fallback?.id) {
      return { dataset: fallback, created: false, createError: error.message };
    }
    throw error;
  }
}

function inferContentType(fileName, fallback = DEFAULT_CONTENT_TYPE) {
  const lowerName = String(fileName || '').toLowerCase();
  if (lowerName.endsWith('.mp4') || lowerName.endsWith('.m4v')) {
    return 'video/mp4';
  }
  if (lowerName.endsWith('.mov')) {
    return 'video/quicktime';
  }
  if (lowerName.endsWith('.webm')) {
    return 'video/webm';
  }
  if (lowerName.endsWith('.mkv')) {
    return 'video/x-matroska';
  }
  if (lowerName.endsWith('.avi')) {
    return 'video/x-msvideo';
  }
  return fallback || 'application/octet-stream';
}

function safeFixtureName(args, urlContentType = '') {
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
    // Use the default fixture name below.
  }
  if (urlContentType.includes('webm')) {
    return 'video-ppt-upload-smoke.webm';
  }
  return DEFAULT_FIXTURE_NAME;
}

async function loadFixture(args, fixtureDir) {
  await mkdir(fixtureDir, { recursive: true });
  if (args.fixtureFile) {
    const bytes = await readFile(args.fixtureFile);
    const fileName = safeFixtureName(args);
    const contentType = args.contentType || inferContentType(fileName, DEFAULT_CONTENT_TYPE);
    const fixturePath = join(fixtureDir, fileName);
    await writeFile(fixturePath, bytes);
    return {
      bytes,
      fileName,
      contentType,
      sourceKind: 'file',
      fixturePath,
      source: { fileName: basename(args.fixtureFile), bytes: bytes.length },
    };
  }

  const response = await fetchWithTimeout(args.fixtureUrl, {
    method: 'GET',
    headers: requestHeaders(args, { contentType: null, accept: '*/*' }),
  }, args.timeoutMs);
  if (!response.ok) {
    throw new Error(`fixture download returned HTTP ${response.status}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  const responseContentType = response.headers.get('content-type') || '';
  const fileName = safeFixtureName(args, responseContentType);
  const contentType = args.contentType || responseContentType || inferContentType(fileName, DEFAULT_CONTENT_TYPE);
  const fixturePath = join(fixtureDir, fileName);
  await writeFile(fixturePath, bytes);
  return {
    bytes,
    fileName,
    contentType,
    sourceKind: 'url',
    fixturePath,
    source: redactedUrlSummary(args.fixtureUrl),
  };
}

function redactedUrlSummary(rawUrl) {
  try {
    const url = new URL(rawUrl);
    return {
      scheme: url.protocol.replace(/:$/, ''),
      host: url.host,
      extension: extname(url.pathname).slice(0, 12),
    };
  } catch {
    return { scheme: '', host: '', extension: '' };
  }
}

function inferUploadMediaKind(fileName, contentType) {
  const signal = `${fileName || ''} ${contentType || ''}`.toLowerCase();
  return /(^|\W)video\//.test(signal) || /\.(mp4|mov|m4v|webm|mkv|avi|mpeg|mpg)(\W|$)/i.test(signal)
    ? 'video'
    : '';
}

async function uploadFixture(args, fixture) {
  const formData = new FormData();
  const blob = new Blob([fixture.bytes], { type: fixture.contentType });
  formData.append('files', blob, fixture.fileName);
  const response = await requestForm(args, '/api/v3/local-document-uploads', formData);
  const savedFile = Array.isArray(response?.files) ? response.files[0] : null;
  if (!savedFile?.object_key) {
    throw new Error('local upload did not return a saved file object_key');
  }
  return {
    savedFile,
    summary: {
      name: savedFile.name || fixture.fileName,
      size: Number(savedFile.size || fixture.bytes.length),
      contentType: savedFile.content_type || fixture.contentType,
      objectKeyPresent: Boolean(savedFile.object_key),
      localPathPresent: Boolean(savedFile.local_path),
    },
  };
}

async function registerAndIngest(args, dataset, fixture, savedFile) {
  const mediaKind = inferUploadMediaKind(fixture.fileName, savedFile.content_type || fixture.contentType);
  const registered = await requestJson(args, '/api/v3/documents', {
    method: 'POST',
    body: {
      dataset_id: dataset.id,
      title: fixture.fileName,
      object_key: savedFile.object_key,
      content_type: savedFile.content_type || fixture.contentType,
      secret_binding_ids: [],
      metadata: {
        initial_classification: {
          dataset_id: dataset.id,
          dataset_title: dataset.title || dataset.key || '',
          confidence: 'high',
          source: 'video_ppt_upload_main_smoke',
          reason: 'Controlled smoke uploaded a public PPT-playback video fixture.',
          media_kind: mediaKind || undefined,
        },
        processing_policy: {
          foreground_allowed: ['save_file', 'preclassify', 'register_document', 'enqueue_ingest'],
          background_required: [
            'parse_content',
            'vlm_enrichment',
            'media_transcription',
            'indexing',
            'report_supply',
          ],
        },
        parse_state: {
          stage: 'queued',
          user_blocking: false,
        },
        smoke: {
          kind: 'video_ppt_upload_main',
          local_thread_id: args.localThreadId,
        },
      },
    },
  });
  const document = registered.document || registered;
  if (!document?.id) {
    throw new Error('document registration did not return document.id');
  }
  const ingest = await requestJson(args, `/api/v3/documents/${encodeURIComponent(document.id)}/ingest`, {
    method: 'POST',
    body: undefined,
  });
  return { registered, document, ingest };
}

function videoArtifactFromList(artifacts) {
  return (Array.isArray(artifacts) ? artifacts : []).find((artifact) => (
    (artifact.source_type || artifact.sourceType) === 'video_extraction'
    && (artifact.template_id || artifact.templateId) === 'video_extraction_summary'
  ));
}

async function waitForVideoArtifact(args, assistantRunId) {
  const deadline = Date.now() + args.timeoutMs;
  let attempts = 0;
  let lastArtifact = null;
  while (Date.now() <= deadline) {
    attempts += 1;
    const query = new URLSearchParams({
      assistant_run_id: assistantRunId,
      local_thread_id: args.localThreadId,
      limit: '50',
    });
    const artifacts = await requestJson(args, `/api/v3/html-artifacts?${query.toString()}`);
    lastArtifact = videoArtifactFromList(artifacts);
    const state = deliverableState(lastArtifact);
    if (state === 'final_pptx_ready') {
      return {
        ok: true,
        attempts,
        artifact: lastArtifact,
        artifactCount: Array.isArray(artifacts) ? artifacts.length : 0,
      };
    }
    await sleep(args.pollIntervalMs);
  }
  return {
    ok: false,
    attempts,
    artifact: lastArtifact,
    artifactCount: lastArtifact ? 1 : 0,
    error: `video artifact did not reach final_pptx_ready within ${args.timeoutMs}ms`,
  };
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function deliverableStatus(artifact) {
  return artifact?.payload?.deliverable_status
    || artifact?.payload?.deliverableStatus
    || artifact?.payload?.generated_artifacts?.deliverable_status
    || artifact?.payload?.generatedArtifacts?.deliverableStatus
    || null;
}

function deliverableState(artifact) {
  return deliverableStatus(artifact)?.state || null;
}

function generatedFiles(artifact) {
  return artifact?.payload?.generated_artifacts?.files
    || artifact?.payload?.generatedArtifacts?.files
    || [];
}

function fileKind(file) {
  return file?.artifact_kind || file?.artifactKind || file?.kind || '';
}

function fileName(file) {
  return file?.file_name || file?.fileName || file?.name || file?.title || '';
}

function fileFormat(file) {
  return file?.format || file?.mime || file?.content_type || file?.contentType || '';
}

async function downloadRequiredFiles(args, artifact, assistantRunId, outDir) {
  const files = generatedFiles(artifact);
  const downloads = [];
  await mkdir(outDir, { recursive: true });
  for (const [index, file] of files.entries()) {
    const kind = fileKind(file);
    if (!DOWNLOAD_FILE_KINDS.has(kind)) {
      continue;
    }
    const query = new URLSearchParams({
      assistant_run_id: assistantRunId,
      local_thread_id: args.localThreadId,
    });
    const path = `/api/v3/html-artifacts/${encodeURIComponent(artifact.id)}/files/${index}?${query.toString()}`;
    const response = await fetchWithTimeout(
      `${normalizeBaseUrl(args.baseUrl)}${path}`,
      { headers: requestHeaders(args, { accept: 'application/octet-stream', contentType: null }) },
      args.timeoutMs,
    );
    const bytes = Buffer.from(await response.arrayBuffer());
    const disposition = response.headers.get('content-disposition') || '';
    const safeName = downloadNameFromDisposition(disposition)
      || defaultDownloadName(kind, fileName(file));
    const localPath = join(outDir, safeName);
    await writeFile(localPath, bytes);
    downloads.push({
      index,
      kind,
      ok: response.ok,
      status: response.status,
      contentType: response.headers.get('content-type') || '',
      contentDisposition: disposition,
      bytes: bytes.length,
      localPath,
      fileName: safeName,
      prefixHex: bytes.subarray(0, 12).toString('hex'),
    });
  }
  return downloads;
}

function downloadNameFromDisposition(disposition) {
  const match = disposition.match(/filename="([^"]+)"/i);
  return match?.[1]?.trim() || '';
}

function defaultDownloadName(kind, rawName) {
  const fallback = rawName || kind;
  return fallback
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
  if (eocdOffset < 0) {
    throw new Error('PPTX ZIP EOCD not found');
  }
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

function artifactSummary(artifact) {
  const status = deliverableStatus(artifact);
  const files = generatedFiles(artifact);
  return {
    id: artifact?.id || null,
    title: artifact?.title || null,
    sourceType: artifact?.source_type || artifact?.sourceType || null,
    templateId: artifact?.template_id || artifact?.templateId || null,
    createdAt: artifact?.created_at || artifact?.createdAt || null,
    deliverableStatus: status
      ? {
          state: status.state || null,
          fileCount: status.file_count || status.fileCount || null,
          hasPptx: Boolean(status.has_pptx || status.hasPptx),
          hasVideoSlidesMarkdown: Boolean(
            status.has_video_slides_markdown || status.hasVideoSlidesMarkdown,
          ),
          hasSubtitlePageMap: Boolean(status.has_subtitle_page_map || status.hasSubtitlePageMap),
          warningCount: status.warning_count || status.warningCount || 0,
          warningCodes: Array.isArray(status.warnings)
            ? status.warnings.map((warning) => warning.code).filter(Boolean)
            : [],
        }
      : null,
    fileKinds: files.map((file, index) => ({
      index,
      kind: fileKind(file),
      name: fileName(file),
      format: fileFormat(file),
    })),
  };
}

function workflowIdFromIngest(ingest) {
  return ingest?.workflow_execution?.id
    || ingest?.workflowExecution?.id
    || ingest?.workflow_execution?.workflow_id
    || ingest?.workflowExecution?.workflowId
    || null;
}

function buildSelfTestArtifact() {
  return {
    id: 'html-artifact-self-test-video-extraction',
    title: 'Self-test video PPT extraction summary',
    source_type: 'video_extraction',
    template_id: 'video_extraction_summary',
    created_at: new Date().toISOString(),
    payload: {
      deliverable_status: {
        state: 'final_pptx_ready',
        file_count: REQUIRED_FILE_KINDS.length,
        has_pptx: true,
        has_video_slides_markdown: true,
        has_subtitle_page_map: false,
        warning_count: 1,
        warnings: [{ code: 'screenshot_based_pptx' }],
      },
      generated_artifacts: {
        files: REQUIRED_FILE_KINDS.map((kind, index) => ({
          artifact_kind: kind,
          file_name: defaultDownloadName(kind, ''),
          format: index === 0 ? 'pptx' : 'json_or_markdown',
        })),
      },
    },
  };
}

function assertSelfTestUploadContract() {
  const dataset = {
    id: 'dataset-self-test-video-ppt-upload',
    key: DEFAULT_DATASET_KEY,
    title: DEFAULT_DATASET_TITLE,
  };
  const scope = buildSelectedScope(dataset);
  if (scope.intent !== 'video_ppt_extraction' || scope.supply_policy?.intent !== 'video_ppt_extraction') {
    throw new Error('self-test selected scope did not preserve video PPT extraction intent');
  }
  if (!scope.datasets.includes(dataset.id)) {
    throw new Error('self-test selected scope did not include the uploaded-video dataset');
  }

  const candidate = buildScopeCandidate(dataset);
  if (candidate.source !== 'video_ppt_upload_main_smoke' || candidate.confidence !== 'high') {
    throw new Error('self-test scope candidate did not identify the smoke source');
  }

  const artifact = videoArtifactFromList([buildSelfTestArtifact()]);
  if (!artifact || deliverableState(artifact) !== 'final_pptx_ready') {
    throw new Error('self-test video artifact was not detected as final_pptx_ready');
  }
  const summary = artifactSummary(artifact);
  const missingKinds = REQUIRED_FILE_KINDS.filter((kind) =>
    !summary.fileKinds.some((file) => file.kind === kind),
  );
  if (missingKinds.length > 0) {
    throw new Error(`self-test artifact summary missing file kinds: ${missingKinds.join(', ')}`);
  }

  const supported = ['sample.mp4', 'sample.mov', 'sample.m4v', 'sample.webm', 'sample.mkv', 'sample.avi'];
  const unsupported = supported.filter((fileName) =>
    inferUploadMediaKind(fileName, inferContentType(fileName)) !== 'video',
  );
  if (unsupported.length > 0) {
    throw new Error(`self-test upload classifier missed supported videos: ${unsupported.join(', ')}`);
  }

  const source = redactedUrlSummary('https://example.com/private/path/video.mp4?token=secret');
  if (JSON.stringify(source).includes('token') || JSON.stringify(source).includes('/private/path')) {
    throw new Error('self-test redacted URL summary leaked path or query data');
  }

  return {
    selectedScopeIntent: scope.intent,
    candidateSource: candidate.source,
    deliverableState: deliverableState(artifact),
    requiredFileKinds: REQUIRED_FILE_KINDS,
    supportedVideoExtensions: supported.map((fileName) => extname(fileName)),
    sourceSummaryRedacted: true,
  };
}

async function runSelfTest(args) {
  const runId = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
  const contract = assertSelfTestUploadContract();
  const report = {
    schema: 'v3.video_ppt_upload_main_smoke_self_test.v1',
    summary: {
      ok: true,
      selfTest: true,
      runId,
      networkCallsRun: false,
      productionWriteAllowed: false,
      fixtureDownloaded: false,
      uploadAttempted: false,
      assistantRunCreated: false,
    },
    contract,
    safety: {
      sourceUrlsIncluded: false,
      objectKeysIncluded: false,
      cookiesIncluded: false,
      bearerIncluded: false,
      providerPayloadsIncluded: false,
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

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) {
    await runSelfTest(args);
    return;
  }
  const runId = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
  const outputDir = join(process.cwd(), args.outputDir, runId);
  const fixtureDir = join(outputDir, 'fixture');
  const downloadsDir = join(outputDir, 'downloads');
  await mkdir(downloadsDir, { recursive: true });

  const datasetResult = await ensureSmokeDataset(args);
  const fixture = await loadFixture(args, fixtureDir);
  const uploadResult = await uploadFixture(args, fixture);
  const registration = await registerAndIngest(
    args,
    datasetResult.dataset,
    fixture,
    uploadResult.savedFile,
  );
  const createResult = await createAssistantRun(args, datasetResult.dataset, registration.document);
  const assistantRunId = createResult.assistantRunId || '';
  if (!createResult.ok) {
    throw new Error(`assistant run create failed: ${createResult.error || 'missing run id'}`);
  }

  const artifactResult = await waitForVideoArtifact(args, assistantRunId);
  if (!artifactResult.ok || !artifactResult.artifact) {
    throw new Error(artifactResult.error || 'video artifact was not found');
  }

  const downloads = await downloadRequiredFiles(args, artifactResult.artifact, assistantRunId, downloadsDir);
  const validation = await validateDownloads(downloads);
  const summary = {
    ok: validation.ok,
    baseUrl: args.baseUrl,
    localThreadId: args.localThreadId,
    datasetId: datasetResult.dataset?.id || null,
    datasetCreated: Boolean(datasetResult.created),
    documentId: registration.document?.id || null,
    ingestWorkflowId: workflowIdFromIngest(registration.ingest),
    assistantRunId,
    artifactOk: artifactResult.ok,
    artifactId: artifactResult.artifact.id,
    artifactPollAttempts: artifactResult.attempts,
    deliverableState: deliverableState(artifactResult.artifact),
    uploadedFileName: fixture.fileName,
    uploadedFileBytes: fixture.bytes.length,
    uploadedContentType: fixture.contentType,
    uploadObjectKeyPresent: uploadResult.summary.objectKeyPresent,
    fixtureSource: fixture.source,
    requiredFileKinds: REQUIRED_FILE_KINDS,
    downloadedKinds: downloads.map((download) => download.kind),
    pptxSlideCount: validation.pptx?.slideCount ?? null,
    markdownSlideHeadingCount: validation.markdown?.slideHeadingCount ?? null,
    generatedAt: new Date().toISOString(),
  };
  const report = {
    summary,
    dataset: {
      id: datasetResult.dataset?.id || null,
      key: datasetResult.dataset?.key || null,
      title: datasetResult.dataset?.title || null,
      created: Boolean(datasetResult.created),
    },
    upload: uploadResult.summary,
    document: {
      id: registration.document?.id || null,
      title: registration.document?.title || null,
      contentType: registration.document?.content_type || registration.document?.contentType || null,
    },
    ingest: {
      workflowId: workflowIdFromIngest(registration.ingest),
      status: registration.ingest?.workflow_execution?.status
        || registration.ingest?.workflowExecution?.status
        || null,
    },
    createResult,
    artifact: artifactSummary(artifactResult.artifact),
    downloads: downloads.map((download) => ({
      index: download.index,
      kind: download.kind,
      ok: download.ok,
      status: download.status,
      contentType: download.contentType,
      contentDisposition: download.contentDisposition,
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
  if (!summary.ok) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
