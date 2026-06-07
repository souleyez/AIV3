#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'https://v3.elepcloud.com';
const DEFAULT_TIMEOUT_MS = 240_000;
const DEFAULT_POLL_INTERVAL_MS = 5_000;
const DEFAULT_OUTPUT_DIR = 'target/video-ppt-main-visible-smoke';
const DEFAULT_PROMPT =
  '请提取这个视频里的 PPT：https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4';
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
    baseUrl: process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    prompt: process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_PROMPT || DEFAULT_PROMPT,
    localThreadId:
      process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_LOCAL_THREAD_ID
      || `video-ppt-main-visible-${Date.now()}`,
    assistantRunId: process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_ASSISTANT_RUN_ID || '',
    cookie: process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_COOKIE || '',
    bearer: process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_BEARER || '',
    timeoutMs: Number(process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    pollIntervalMs: Number(
      process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_POLL_INTERVAL_MS || DEFAULT_POLL_INTERVAL_MS,
    ),
    outputDir: process.env.VIDEO_PPT_MAIN_VISIBLE_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--base-url') {
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
  node scripts/smoke/video-ppt-main-visible.mjs \\
    --base-url https://v3.elepcloud.com \\
    --prompt "请提取这个视频里的 PPT：https://..." \\
    --local-thread-id video-ppt-main-visible-...

  node scripts/smoke/video-ppt-main-visible.mjs \\
    --assistant-run-id <existing-run-id> \\
    --local-thread-id <existing-local-thread-id>

Checks:
  - creates or reuses a main-site assistant run
  - waits for a video_extraction_summary HTML artifact
  - requires final_pptx_ready, PPTX, video_slides.md, and delivery manifests
  - downloads required files through /api/v3/html-artifacts/{id}/files/{index}
  - validates PPTX ZIP central directory and Markdown slide count
`);
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function requestHeaders(args, accept = 'application/json') {
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

async function createAssistantRun(args) {
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
      headers: requestHeaders(args, 'text/event-stream'),
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

async function fetchJson(args, path) {
  const response = await fetch(`${normalizeBaseUrl(args.baseUrl)}${path}`, {
    headers: requestHeaders(args),
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`GET ${path} returned HTTP ${response.status}: ${text.slice(0, 240)}`);
  }
  return JSON.parse(text);
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
    const artifacts = await fetchJson(args, `/api/v3/html-artifacts?${query.toString()}`);
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
    const response = await fetch(`${normalizeBaseUrl(args.baseUrl)}${path}`, {
      headers: requestHeaders(args, 'application/octet-stream'),
    });
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
    const bytes = await readLocalDownload(pptx.localPath);
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
    const bytes = await readLocalDownload(markdown.localPath);
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

async function readLocalDownload(localPath) {
  const { readFile } = await import('node:fs/promises');
  return readFile(localPath);
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

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const runId = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
  const outputDir = join(process.cwd(), args.outputDir, runId);
  const downloadsDir = join(outputDir, 'downloads');
  await mkdir(downloadsDir, { recursive: true });

  let createResult = null;
  let assistantRunId = args.assistantRunId.trim();
  if (!assistantRunId) {
    createResult = await createAssistantRun(args);
    assistantRunId = createResult.assistantRunId || '';
    if (!createResult.ok) {
      throw new Error(`assistant run create failed: ${createResult.error || 'missing run id'}`);
    }
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
    assistantRunId,
    localThreadId: args.localThreadId,
    createdRun: !args.assistantRunId,
    createOk: createResult?.ok ?? null,
    artifactOk: artifactResult.ok,
    artifactId: artifactResult.artifact.id,
    artifactPollAttempts: artifactResult.attempts,
    deliverableState: deliverableState(artifactResult.artifact),
    requiredFileKinds: REQUIRED_FILE_KINDS,
    downloadedKinds: downloads.map((download) => download.kind),
    pptxSlideCount: validation.pptx?.slideCount ?? null,
    markdownSlideHeadingCount: validation.markdown?.slideHeadingCount ?? null,
    generatedAt: new Date().toISOString(),
  };
  const report = {
    summary,
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
      localPath: download.localPath,
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
