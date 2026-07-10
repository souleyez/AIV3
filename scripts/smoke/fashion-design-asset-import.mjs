#!/usr/bin/env node

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { mkdir, stat, writeFile } from 'node:fs/promises';

import {
  assetProfileKindOptions,
  buildFashionDesignImageAssetImportBatchPayload,
  buildFashionDesignImageAssetImportPayload,
  filterAssetProfileHints,
  normalizeFashionDesignAssetImportTaskCardDraft,
  normalizeAssetLibraryScope,
  normalizeFashionDesignImageAssetImportBatchResponse,
  normalizeFashionDesignImageAssetImportResponse,
} from '../../apps/web/app/lib/asset-library-view-model.js';

const ROOT_DIR = path.resolve(fileURLToPath(new URL('../..', import.meta.url)));
const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_OUTPUT_DIR = 'target/fashion-design-asset-import-smoke';
const DEFAULT_TIMEOUT_MS = 120_000;
const PNG_FIXTURE_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
const ZIP_FIXTURE_BASE64 =
  'UEsDBBQAAAAIAPZx6FyTIEkcQwAAAEYAAAAWAAAAbG9va3Mvc3ByaW5nLWRyZXNzLnBuZ+sM8HPn5ZLiYmBg4PX0cAliYGBgBGEONgYGBnnRI51gCRfHkIpbySk/zgfwMzC3MTZEmdRmMzAwMHi6+rmsc0poAgBQSwMEFAAAAAgA9nHoXLm7bhEWAAAAFAAAABMAAABsb29rcy93b3Jrd2Vhci53ZWJwC/J0c6uoqKgId3UKCAuwUMjNT84GAFBLAwQUAAAACAD2cehcGt8ylQgAAAAGAAAACgAAAHJlYWRtZS50eHQrzs4s4OUCAFBLAQIUABQAAAAIAPZx6FyTIEkcQwAAAEYAAAAWAAAAAAAAAAAAAAAAAAAAAABsb29rcy9zcHJpbmctZHJlc3MucG5nUEsBAhQAFAAAAAgA9nHoXLm7bhEWAAAAFAAAABMAAAAAAAAAAAAAAAAAdwAAAGxvb2tzL3dvcmt3ZWFyLndlYnBQSwECFAAUAAAACAD2cehcGt8ylQgAAAAGAAAACgAAAAAAAAAAAAAAAAC+AAAAcmVhZG1lLnR4dFBLBQYAAAAAAwADAL0AAADuAAAAAAA=';

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    outputDir: process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    cookie: process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_COOKIE || '',
    bearer: process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_BEARER || '',
    datasetId: process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_DATASET_ID || '',
    assetLibraryId: process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_ASSET_LIBRARY_ID || '',
    localThreadId: process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_LOCAL_THREAD_ID || '',
    approvalId: process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_APPROVAL_ID || '',
    timeoutMs: Number(process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    selfTest: parseBoolean(process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_SELF_TEST),
    preflight: parseBoolean(process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_PREFLIGHT),
    execute: parseBoolean(process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_EXECUTE),
    ackLiveWrite: parseBoolean(process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_ACK_LIVE_WRITE),
    pretty: parseBoolean(process.env.FASHION_DESIGN_ASSET_IMPORT_SMOKE_PRETTY),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--base-url') {
      args.baseUrl = requireValue(arg, next);
      index += 1;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
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
    } else if (arg === '--asset-library-id') {
      args.assetLibraryId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--local-thread-id') {
      args.localThreadId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--approval-id') {
      args.approvalId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--preflight') {
      args.preflight = true;
    } else if (arg === '--execute') {
      args.execute = true;
    } else if (arg === '--ack-live-write') {
      args.ackLiveWrite = true;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  const modes = [args.selfTest, args.preflight, args.execute].filter(Boolean).length;
  if (modes !== 1) {
    throw new Error('choose exactly one mode: --self-test, --preflight, or --execute');
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 10_000) {
    throw new Error('--timeout-ms must be at least 10000');
  }
  if (args.execute && !args.ackLiveWrite) {
    throw new Error('--execute requires --ack-live-write');
  }
  if (args.execute && !args.approvalId) {
    throw new Error('--execute requires --approval-id for auditability');
  }
  if (args.execute && !args.datasetId) {
    throw new Error('--execute requires --dataset-id for an existing writable V3 dataset');
  }
  if (args.execute && !args.assetLibraryId) {
    throw new Error('--execute requires --asset-library-id for an existing V3 asset library');
  }
  if (args.execute && !args.cookie && !args.bearer) {
    throw new Error('--execute requires --cookie or --bearer for a V3 user session');
  }
  args.baseUrl = normalizeBaseUrl(args.baseUrl);

  return args;
}

function parseBoolean(value) {
  return ['1', 'true', 'yes', 'y', 'on'].includes(String(value || '').trim().toLowerCase());
}

function requireValue(flag, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`
Usage:
  npm run smoke:fashion-design-asset-import -- --self-test
  npm run smoke:fashion-design-asset-import -- --self-test --pretty

  npm run smoke:fashion-design-asset-import -- --preflight --pretty

  npm run smoke:fashion-design-asset-import -- --execute --ack-live-write \\
    --approval-id "<reviewed change/smoke id>" \\
    --base-url https://v3.elepcloud.com \\
    --cookie "<V3 session cookie>" \\
    --dataset-id "<existing dataset uuid>" \\
    --asset-library-id "<existing asset library uuid>"

Checks:
  - writes local-only PNG and ZIP fixtures under target/
  - builds single and batch fashion-design image import payloads
  - validates ZIP package payload shape without network calls
  - validates normalized import responses and asset-library profile hints
  - execute mode uploads fixtures, imports one image plus one ZIP, then checks scope-summary
  - reports only counts, booleans, and hashes; no object keys, URLs, cookies, or tokens
`);
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.]/g, '').replace(/Z$/, `Z-${process.pid}`);
}

function resolveOutputDir(args) {
  return path.resolve(ROOT_DIR, args.outputDir);
}

function normalizeBaseUrl(value) {
  return String(value || '').replace(/\/+$/, '');
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function authHeaders(args) {
  const headers = {};
  if (args.cookie) headers.cookie = args.cookie;
  if (args.bearer) {
    headers.authorization = args.bearer.toLowerCase().startsWith('bearer ')
      ? args.bearer
      : `Bearer ${args.bearer}`;
  }
  if (args.localThreadId) {
    headers['x-ai-data-platform-local-thread-id'] = args.localThreadId;
  }
  return headers;
}

function jsonHeaders(args) {
  return {
    ...authHeaders(args),
    'content-type': 'application/json',
  };
}

function buildApiUrl(args, route) {
  const pathPart = String(route || '').startsWith('/') ? route : `/${route}`;
  return `${args.baseUrl}${pathPart}`;
}

async function fetchWithTimeout(url, options = {}, timeoutMs = DEFAULT_TIMEOUT_MS) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, {
      ...options,
      signal: controller.signal,
    });
  } finally {
    clearTimeout(timer);
  }
}

async function requestJson(args, route, options = {}) {
  const response = await fetchWithTimeout(buildApiUrl(args, route), {
    method: options.method || 'GET',
    headers: options.headers || jsonHeaders(args),
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  }, args.timeoutMs);
  const text = await response.text();
  let body = null;
  if (text) {
    try {
      body = JSON.parse(text);
    } catch {
      body = { raw_text: text.slice(0, 300) };
    }
  }
  if (!response.ok) {
    const code = body?.error?.code || body?.error || body?.code || response.status;
    throw new Error(`${options.method || 'GET'} ${route} failed: ${response.status} ${code}`);
  }
  return body;
}

async function requestForm(args, route, formData) {
  const response = await fetchWithTimeout(buildApiUrl(args, route), {
    method: 'POST',
    headers: authHeaders(args),
    body: formData,
  }, args.timeoutMs);
  const text = await response.text();
  let body = null;
  if (text) {
    try {
      body = JSON.parse(text);
    } catch {
      body = { raw_text: text.slice(0, 300) };
    }
  }
  if (!response.ok) {
    const code = body?.error?.code || body?.error || body?.code || response.status;
    throw new Error(`POST ${route} failed: ${response.status} ${code}`);
  }
  return body;
}

function redactedUrlSummary(rawUrl) {
  try {
    const url = new URL(rawUrl);
    return {
      scheme: url.protocol.replace(/:$/, ''),
      host: url.host,
    };
  } catch {
    return { scheme: '', host: '' };
  }
}

async function writeFixtureFiles(args) {
  const runId = makeRunId();
  const fixtureDir = path.join(resolveOutputDir(args), 'fixtures', runId);
  await mkdir(fixtureDir, { recursive: true });

  const pngBytes = Buffer.from(PNG_FIXTURE_BASE64, 'base64');
  const zipBytes = Buffer.from(ZIP_FIXTURE_BASE64, 'base64');
  const pngPath = path.join(fixtureDir, 'spring-dress.png');
  const zipPath = path.join(fixtureDir, 'fashion-gallery.zip');
  await writeFile(pngPath, pngBytes);
  await writeFile(zipPath, zipBytes);

  const pngStat = await stat(pngPath);
  const zipStat = await stat(zipPath);
  return {
    runId,
    pngPath,
    zipPath,
    pngBuffer: pngBytes,
    zipBuffer: zipBytes,
    pngBytes: pngStat.size,
    zipBytes: zipStat.size,
    pngSha256: sha256(pngBytes),
    zipSha256: sha256(zipBytes),
  };
}

function fixtureProfilePayload() {
  return {
    category: 'dress',
    audience: 'women',
    season: 'spring_summer',
    styles: ['commute', 'light_luxury'],
    silhouettes: ['a_line'],
    collars: ['round_neck'],
    sleeves: ['puff_sleeve'],
    waists: ['high_waist'],
    hems: ['midi'],
    materials: ['cotton_linen'],
    crafts: ['pleated'],
    colors: ['green', 'white'],
    patterns: ['floral'],
    scenes: ['retail_display'],
    noun_terms: ['spring_summer', 'dress', 'puff_sleeve', 'floral'],
    caption: '春夏通勤连衣裙设计图，圆领、泡泡袖、高腰和碎花元素。',
  };
}

function buildPayloadEvidence(fixtures) {
  const datasetId = '11111111-1111-4111-8111-111111111111';
  const assetLibraryId = '22222222-2222-4222-8222-222222222222';
  const metadata = {
    smoke: 'fashion_design_asset_import',
    source: 'self_test_fixture',
    original_name: '客户款式图.png',
    sha256: 'abc123',
    raw_provider_payload: { should_not_surface: true },
    nested: {
      object_key: 'objects/private/smoke-look.png',
      local_path: `${ROOT_DIR}/target/private-look.png`,
      label: 'safe-smoke-label',
    },
  };
  const profilePayload = fixtureProfilePayload();

  const single = buildFashionDesignImageAssetImportPayload({
    datasetId,
    assetLibraryId,
    title: '春夏连衣裙灵感图',
    objectKey: fixtures.pngPath,
    contentType: 'image/png',
    profilePayload,
    metadata,
  });
  assert.deepEqual(single.errors, []);
  assert.equal(single.payload.dataset_id, datasetId);
  assert.equal(single.payload.asset_library_id, assetLibraryId);
  assert.equal(single.payload.content_type, 'image/png');
  assert.equal(single.payload.profile_payload.category, 'dress');

  const batch = buildFashionDesignImageAssetImportBatchPayload({
    datasetId,
    assetLibraryId,
    title: '春夏女装设计图',
    imageUrlsText: `${fixtures.pngPath}\nobject-store/fashion/workwear.webp`,
    packages: [
      {
        externalId: 'zip-pack-001',
        title: '服装设计 ZIP 包',
        objectKey: fixtures.zipPath,
        contentType: 'application/zip',
        metadata: {
          smoke_package: true,
          original_name: '客户图库.zip',
          path: fixtures.zipPath,
          object_key: fixtures.zipPath,
          raw_provider_payload: { should_not_surface: true },
        },
      },
    ],
    profilePayload,
    metadata,
  });
  assert.deepEqual(batch.errors, []);
  assert.equal(batch.payload.dataset_id, datasetId);
  assert.equal(batch.payload.assets.length, 2);
  assert.equal(batch.payload.packages.length, 1);
  assert.equal(batch.payload.assets[0].content_type, 'image/png');
  assert.equal(batch.payload.assets[1].content_type, 'image/webp');
  assert.equal(batch.payload.packages[0].content_type, 'application/zip');
  assert.equal(batch.payload.assets[0].metadata.source, undefined);
  assert.equal(batch.payload.assets[0].metadata.source_present, true);
  assert.equal(batch.payload.assets[0].metadata.source_kind, 'object_key');
  assert.equal(batch.payload.assets[1].metadata.source, undefined);
  assert.equal(batch.payload.assets[1].metadata.source_kind, 'object_key');
  assert.equal(single.payload.metadata.source, undefined);
  assert.equal(single.payload.metadata.original_name, undefined);
  assert.equal(single.payload.metadata.sha256, undefined);
  assert.equal(single.payload.metadata.raw_provider_payload, undefined);
  assert.equal(single.payload.metadata.nested.object_key, undefined);
  assert.equal(single.payload.metadata.nested.local_path, undefined);
  assert.equal(single.payload.metadata.nested.label, 'safe-smoke-label');
  assert.equal(batch.payload.metadata.source, undefined);
  assert.equal(batch.payload.metadata.raw_provider_payload, undefined);
  assert.equal(batch.payload.packages[0].metadata.package_source, undefined);
  assert.equal(batch.payload.packages[0].metadata.package_source_present, true);
  assert.equal(batch.payload.packages[0].metadata.original_name, undefined);
  assert.equal(batch.payload.packages[0].metadata.path, undefined);
  assert.equal(batch.payload.packages[0].metadata.object_key, undefined);
  assert.equal(batch.payload.packages[0].metadata.raw_provider_payload, undefined);

  return {
    single,
    batch,
    summary: {
      singlePayloadValid: true,
      batchPayloadValid: true,
      batchAssetCount: batch.payload.assets.length,
      batchPackageCount: batch.payload.packages.length,
      packageContentType: batch.payload.packages[0].content_type,
      profileCategory: single.payload.profile_payload.category,
      profileTermCount: single.payload.profile_payload.noun_terms.length,
      payloadItemMetadataRedacted: true,
      payloadRootMetadataRedacted: true,
      payloadPackageMetadataRedacted: true,
    },
  };
}

function buildScopeGuardEvidence() {
  const single = buildFashionDesignImageAssetImportPayload({
    datasetId: '11111111-1111-4111-8111-111111111111',
    collectionId: '33333333-3333-4333-8333-333333333333',
    title: '孤立图库分组导入',
    imageUrl: 'https://example.com/assets/dress-001.png',
    contentType: 'image/png',
  });
  const batch = buildFashionDesignImageAssetImportBatchPayload({
    datasetId: '11111111-1111-4111-8111-111111111111',
    collectionId: '33333333-3333-4333-8333-333333333333',
    imageUrlsText: 'object-store/fashion/dress-001.png',
  });
  const expectedError = '请选择资产库后再选择图库分组。';

  assert.equal(single.errors.includes(expectedError), true);
  assert.equal(batch.errors.includes(expectedError), true);

  return {
    summary: {
      singleCollectionRequiresAssetLibrary: true,
      batchCollectionRequiresAssetLibrary: true,
      orphanCollectionPayloadRejected: true,
    },
  };
}

function buildMockImportResponse(payloadEvidence) {
  const item = (index, title, status = 'pending') => ({
    asset: {
      id: `asset-${index}`,
      title,
      asset_kind: 'image',
      source_kind: 'fashion_design_image_import',
      source_id: `objects/private/import-response-${index}.png`,
      object_key: `objects/private/import-response-${index}.png`,
      metadata: {
        raw_provider_payload: { should_not_surface: true },
      },
      profile_count: 1,
    },
    dataset_membership: {
      dataset_id: payloadEvidence.single.payload.dataset_id,
      asset_id: `asset-${index}`,
      membership_kind: 'imported',
    },
    parse_run: {
      id: `parse-${index}`,
      asset_id: `asset-${index}`,
      parser_name: 'datamax-fashion-image-parser',
      parser_version: '2026-06-17',
      status,
      error_message: `private object path objects/private/import-response-${index}.png`,
      metadata: {
        source: `objects/private/import-response-${index}.png`,
      },
    },
    profile: {
      id: `profile-${index}`,
      asset_id: `asset-${index}`,
      profile_kind: 'fashion_design_image_v1',
      profile_version: 'v1',
      attributes: {
        ...fixtureProfilePayload(),
        raw_provider_payload: { should_not_surface: true },
      },
      embedding_status: 'not_requested',
    },
  });

  const single = normalizeFashionDesignImageAssetImportResponse(item(1, '春夏连衣裙灵感图'));
  const batch = normalizeFashionDesignImageAssetImportBatchResponse({
    accepted: true,
    asset_count: 4,
    package_count: 1,
    expanded_asset_count: 2,
    items: [
      item(1, '春夏连衣裙灵感图'),
      item(2, '春夏女装设计图 2'),
      item(3, '服装设计 ZIP 包 - spring-dress.png'),
      item(4, '服装设计 ZIP 包 - workwear.webp'),
    ],
  });

  assert.equal(single.parseRunStatus, 'pending');
  assert.equal(single.profileKind, 'fashion_design_image_v1');
  assert.equal(batch.accepted, true);
  assert.equal(batch.assetCount, 4);
  assert.equal(batch.packageCount, 1);
  assert.equal(batch.expandedAssetCount, 2);
  assert.equal(batch.items.every((normalized) => normalized.parseRunStatus === 'pending'), true);
  const serialized = JSON.stringify({ single, batch });
  assert.equal(/objects\/private/i.test(serialized), false);
  assert.equal(/raw_provider_payload/i.test(serialized), false);

  return {
    single,
    batch,
    summary: {
      singleResponseNormalized: true,
      batchResponseNormalized: true,
      normalizedBatchAssetCount: batch.assetCount,
      normalizedPackageCount: batch.packageCount,
      normalizedExpandedAssetCount: batch.expandedAssetCount,
      pendingParseRunCount: batch.items.filter((normalized) => normalized.parseRunStatus === 'pending').length,
      normalizedImportResponseRedacted: true,
    },
  };
}

function buildScopeEvidence() {
  const scope = normalizeAssetLibraryScope({
    assets: [
      {
        id: 'asset-spring-dress',
        title: '春夏连衣裙灵感图',
        asset_kind: 'image',
        source_kind: 'fashion_design_image_import',
        source_id: 'objects/private/spring-dress.png',
        object_key: 'objects/private/spring-dress.png',
        metadata: {
          raw_provider_payload: { should_not_surface: true },
        },
        profile_count: 1,
      },
    ],
    asset_profile_hints: [
      {
        asset_id: 'asset-spring-dress',
        title: '春夏连衣裙灵感图',
        asset_kind: 'image',
        source_kind: 'fashion_design_image_import',
        profile_kind: 'fashion_design_image_v1',
        summary: '品类: 连衣裙；季节: 春夏；袖型: 泡泡袖；图案: 碎花',
        noun_terms: ['连衣裙', '春夏', '泡泡袖', '碎花'],
        facets: ['品类: 连衣裙', '季节: 春夏', '袖型: 泡泡袖'],
      },
      {
        asset_id: 'asset-workwear',
        title: '通勤西装参考图',
        asset_kind: 'image',
        source_kind: 'fashion_design_image_import',
        profile_kind: 'fashion_design_image_v1',
        summary: '品类: 西装；季节: 秋冬；风格: 商务',
        noun_terms: ['西装', '秋冬', '商务'],
        facets: ['品类: 西装', '季节: 秋冬'],
      },
    ],
    asset_profile_hint_count: 2,
    asset_parse_status_counts: {
      pending: 2,
      completed: 1,
    },
    asset_parse_run_count: 3,
  });
  const kinds = assetProfileKindOptions(scope);
  const springDressResults = filterAssetProfileHints(scope, {
    query: '泡泡袖',
    assetKind: 'image',
  });
  assert.equal(scope.assetProfileHints.length, 2);
  assert.equal(scope.assets.length, 1);
  assert.equal(scope.assets[0].storageLocatorPresent, true);
  assert.equal(scope.assets[0].sourceIdPresent, true);
  assert.equal(Object.hasOwn(scope.assets[0], 'object_key'), false);
  assert.equal(Object.hasOwn(scope.assets[0], 'objectKey'), false);
  assert.equal(Object.hasOwn(scope.assets[0], 'source_id'), false);
  assert.equal(Object.hasOwn(scope.assets[0], 'sourceId'), false);
  assert.equal(Object.hasOwn(scope.assets[0], 'metadata'), false);
  assert.deepEqual(kinds, ['image']);
  assert.equal(springDressResults[0]?.assetId, 'asset-spring-dress');

  return {
    scope,
    summary: {
      scopeNormalized: true,
      scopeHintCount: scope.assetProfileHintCount,
      assetKinds: kinds,
      scopeAssetCount: scope.assets.length,
      scopeAssetLocatorsRedacted: scope.assets.every((asset) => (
        asset.storageLocatorPresent
        && asset.sourceIdPresent
        && !Object.hasOwn(asset, 'object_key')
        && !Object.hasOwn(asset, 'objectKey')
        && !Object.hasOwn(asset, 'source_id')
        && !Object.hasOwn(asset, 'sourceId')
        && !Object.hasOwn(asset, 'metadata')
      )),
      parseStatusPendingCount: scope.assetParseStatusCounts.pending || 0,
      parseRunCount: scope.assetParseRunCount,
      designQueryFilterHit: springDressResults[0]?.assetId === 'asset-spring-dress',
    },
  };
}

function buildPlanningEvidence(scopeEvidence) {
  const scope = scopeEvidence.scope;
  const statusCounts = scope.assetParseStatusCounts || {};
  const pendingCount = Number(statusCounts.pending || 0);
  const completedCount = Number(statusCounts.completed || 0);
  const parseRunCount = Number(scope.assetParseRunCount || 0);
  const profileHintCount = Number(scope.assetProfileHintCount || scope.assetProfileHints.length || 0);
  const attentionAssets = scope.assetProfileHints.slice(0, 2).map((hint, index) => ({
    asset_id: hint.assetId || `asset-${index + 1}`,
    title: hint.title || '',
    asset_kind: hint.assetKind || '',
    source_kind: hint.sourceKind || '',
    content_type: 'image/png',
    model_status: index < pendingCount ? 'pending' : 'completed',
    parse_status: index < pendingCount ? 'pending' : 'completed',
    error_code: '',
    updated_at: 'fixture-redacted-timestamp',
  })).filter((asset) => asset.model_status !== 'completed');
  const assetParseStatusSummary = {
    count: parseRunCount > 0 ? 1 : 0,
    scanned_asset_count: parseRunCount,
    not_ready_asset_count: pendingCount,
    failed_asset_count: Number(statusCounts.failed || 0),
    retrying_asset_count: Number(statusCounts.retrying || 0),
    status_counts: statusCounts,
    attention_assets: attentionAssets,
  };
  const staticPageEvidenceSummary = {
    asset_profile_summary: {
      count: profileHintCount,
      primary_terms: scope.assetProfileHints.flatMap((hint) => hint.nounTerms || []).slice(0, 8),
    },
    asset_parse_status_summary: assetParseStatusSummary,
  };
  const reportPlannerAst = {
    asset_profile_summary: staticPageEvidenceSummary.asset_profile_summary,
    asset_parse_status_summary: assetParseStatusSummary,
    modules: [
      { kind: 'scope_summary' },
      {
        kind: 'asset_materials',
        binding_slot: 'assets.profile_summary',
        asset_parse_status_summary: assetParseStatusSummary,
      },
    ],
  };

  assert.equal(staticPageEvidenceSummary.asset_parse_status_summary.status_counts.pending, 2);
  assert.equal(reportPlannerAst.asset_parse_status_summary.scanned_asset_count, 3);
  assert.equal(reportPlannerAst.modules[1].kind, 'asset_materials');
  assert.equal(reportPlannerAst.modules[1].asset_parse_status_summary.attention_assets.length, 2);
  const serialized = JSON.stringify({ staticPageEvidenceSummary, reportPlannerAst });
  assert.equal(/object[_-]?key/i.test(serialized), false);
  assert.equal(/raw_provider_payload/i.test(serialized), false);
  assert.equal(serialized.includes(ROOT_DIR), false);

  return {
    summary: {
      staticPageEvidenceParseStatusVisible: true,
      reportPlannerAstParseStatusVisible: true,
      reportPlannerAssetMaterialsInsertedForPendingAssets: true,
      planningPendingAssetCount: pendingCount,
      planningCompletedAssetCount: completedCount,
      planningParseRunCount: parseRunCount,
      planningAttentionAssetCount: attentionAssets.length,
      planningSensitiveFieldsRedacted: true,
    },
  };
}

function buildFollowupDryRunEvidence(responseEvidence, scopeEvidence) {
  const pendingItems = responseEvidence.batch.items
    .filter((item) => item.parseRunStatus === 'pending');
  const parseStatusTransitions = [
    buildParserStatusTransitionDryRun('pending', true),
    buildParserStatusTransitionDryRun('parsing', false),
    buildParserStatusTransitionDryRun('failed', true),
    buildParserStatusTransitionDryRun('completed', true),
  ];
  const parseStatusTransitionDryRunReady = parseStatusTransitions.some((transition) => (
    transition.current_status === 'pending'
    && transition.next_status === 'completed'
    && transition.should_materialize_retrieval_evidence
  )) && parseStatusTransitions.some((transition) => (
    transition.current_status === 'parsing'
    && transition.next_status === 'retrying'
    && !transition.should_materialize_retrieval_evidence
  ));
  const retrievalEvidencePreviews = scopeEvidence.scope.assetProfileHints
    .slice(0, 8)
    .map((hint) => ({
      asset_id: hint.assetId,
      title: hint.title,
      asset_kind: hint.assetKind,
      source_kind: 'asset_profile',
      profile_kind: hint.profileKind,
      terms: (hint.nounTerms || []).slice(0, 8),
      facets: (hint.facets || []).slice(0, 6),
      summary_preview: String(hint.summary || '').slice(0, 120),
      retrieval_text_preview: materializeAssetProfileRetrievalTextPreview(hint),
      write_policy: 'dry_run_only_without_execute',
    }));
  const retrievalEvidenceWritePlans = retrievalEvidencePreviews
    .map((preview) => buildRetrievalEvidenceWritePlan(preview));
  const retrievalEvidenceAdapters = retrievalEvidencePreviews
    .map((preview) => buildRetrievalEvidenceAdapterDryRun(preview));
  const assetRetrievalEvidenceWriterReadinessDryRuns = retrievalEvidenceAdapters
    .map((adapter) => buildAssetRetrievalEvidenceLiveWriterReadinessDryRun({
      profileReady: Boolean(adapter.evidence_draft?.manifest?.asset_profile?.profile_schema),
      materializedTextReady: Boolean(adapter.evidence_draft?.content_excerpt_present),
      datasetMembershipVerified: true,
      assetEvidenceTableAvailable: true,
      unionSearchAdapterAvailable: true,
    }));
  const thirdPartyPrivateAssetImportEndpointGuard =
    buildThirdPartyPrivateAssetImportEndpointGuardDryRun({
      publicDocsAssetImportsOpen: false,
      inboundSecretConfigured: true,
      connectionScopeReady: true,
      sourceScopeReady: true,
      datasetScopeSupplied: true,
      assetLibraryScopeSupplied: true,
      assetsSupplied: true,
    });
  const mainSiteGalleryTaskCard = normalizeFashionDesignAssetImportTaskCardDraft({
    importId: 'fixture-import-redacted',
    assetLibraryId: 'asset-library-present',
    assetLibraryName: '服装设计资产库',
    datasetId: 'dataset-present',
    promptSummary: '导入设计图资产，解析画像后用于图库筛选、问答和报表。',
    batchResponse: {
      accepted: true,
      asset_count: responseEvidence.batch.assetCount,
      items: responseEvidence.batch.items.map((item) => ({
        asset: {
          id: item.assetId,
          title: item.title,
        },
        parse_run: {
          id: item.parseRun?.id || 'parse-redacted',
          status: item.parseRunStatus || 'pending',
        },
      })),
    },
    scope: {
      summary: {
        asset_parse_status_counts: scopeEvidence.scope.assetParseStatusCounts,
        asset_parse_run_count: scopeEvidence.scope.assetParseRunCount,
      },
    },
  });
  const mainSiteGalleryTaskCardDryRun = buildMainSiteGalleryTaskCardDryRun({
    importRequestReady: true,
    normalizedResponseReady: responseEvidence.summary.batchResponseNormalized === true,
    scopeSummaryReady: scopeEvidence.summary.scopeNormalized === true,
    parseStatusReady: scopeEvidence.summary.parseRunCount > 0,
    fieldLedgerReady: true,
    stableTaskIdentityReady: Boolean(mainSiteGalleryTaskCard.id),
    taskCard: mainSiteGalleryTaskCard,
  });
  const retrievalStorageMappings = retrievalEvidencePreviews
    .map((preview) => buildRetrievalStorageMappingDryRun(preview));
  const migrationSketches = retrievalEvidencePreviews
    .map(() => buildAssetRetrievalEvidenceMigrationSketchDryRun());
  const unionSearchAdapters = retrievalEvidencePreviews
    .map((preview) => buildUnionSearchNoWriteAdapterDraft(preview));
  const mergeRankFixtures = retrievalEvidencePreviews
    .map((preview) => buildUnionSearchMergeRankFixtureDryRun(preview));
  const explainDebugSummaries = retrievalEvidencePreviews
    .map((preview) => buildUnionSearchExplainDebugSummaryDryRun(preview));
  const modelFacingCompressions = retrievalEvidencePreviews
    .map((preview) => buildModelFacingSupplyCompressionDryRun(preview));
  const assistantRunSupplyGateDryRuns = retrievalEvidencePreviews
    .map((preview) => buildAssistantRunAssetDocumentSupplyGateDryRun([
      buildDocumentEvidenceFixtureForAssistantRun(),
      buildAssetProfileHintFixtureForAssistantRun(preview),
    ]));
  const assistantRunAssetOnlySupplyGateDryRuns = retrievalEvidencePreviews
    .map((preview) => buildAssistantRunAssetDocumentSupplyGateDryRun([
      buildAssetProfileHintFixtureForAssistantRun(preview),
    ]));
  const assistantRunSupplyProgressEvents = retrievalEvidencePreviews
    .map((preview) => buildAssistantRunSupplyProgressEventsDryRun([
      buildDocumentEvidenceFixtureForAssistantRun(),
      buildAssetProfileHintFixtureForAssistantRun(preview),
    ]));
  const assistantRunAssetOnlySupplyProgressEvents = retrievalEvidencePreviews
    .map((preview) => buildAssistantRunSupplyProgressEventsDryRun([
      buildAssetProfileHintFixtureForAssistantRun(preview),
    ]));
  const assistantRunParseProgressEvents = retrievalEvidencePreviews
    .slice(0, 1)
    .map((preview) => buildAssistantRunSupplyProgressEventsDryRun([
      buildDocumentEvidenceFixtureForAssistantRun(),
      buildAssetProfileHintFixtureForAssistantRun(preview),
      buildAssetParseStatusFixtureForAssistantRun(),
    ]));
  const assistantRunProgressContractDriftGuards = [
    ...assistantRunSupplyProgressEvents,
    ...assistantRunAssetOnlySupplyProgressEvents,
    ...assistantRunParseProgressEvents,
  ].map((progress) => buildAssistantRunSupplyProgressContractDriftGuardDryRun(progress));
  const parserLiveAdapterReadinessDryRuns = [
    buildParserLiveAdapterReadinessDryRun(
      buildParserWorkerInputContract({ sourceRefPresent: true, sourceKind: 'object_key', contentType: 'image/png' }),
      buildParserWorkerOutputContract({ profileReady: true, retrievalReady: true }),
    ),
    buildParserLiveAdapterReadinessDryRun(
      buildParserWorkerInputContract({ sourceRefPresent: true, sourceKind: 'object_key', contentType: 'image/webp' }),
      buildParserWorkerOutputContract({ profileReady: false, retrievalReady: false }),
    ),
  ];
  const duplicateFirstPlan = retrievalEvidencePreviews[0]
    ? buildRetrievalEvidenceWritePlan(retrievalEvidencePreviews[0])
    : null;
  const writePlanKeys = retrievalEvidenceWritePlans.map((plan) => plan.idempotency_key);
  const retrievalEvidenceIdempotencyPlanned = retrievalEvidenceWritePlans.length > 0
    && new Set(writePlanKeys).size === writePlanKeys.length
    && duplicateFirstPlan?.idempotency_key === retrievalEvidenceWritePlans[0]?.idempotency_key
    && retrievalEvidenceWritePlans.every((plan) => (
      plan.ready
      && plan.action === 'upsert_retrieval_evidence'
      && plan.write_policy === 'upsert_by_idempotency_key_after_profile_available'
      && Array.isArray(plan.dedupe_scope)
      && plan.dedupe_scope.includes('asset_id')
    ));
  const retrievalEvidenceAdapterDryRunReady = retrievalEvidenceAdapters.length > 0
    && retrievalEvidenceAdapters.every((adapter) => (
      adapter.adapter_contract === 'asset_profile_retrieval_evidence_writer_v1'
      && adapter.status === 'ready'
      && adapter.evidence_draft?.manifest?.lexical?.status === 'materialized'
      && adapter.write_plan?.idempotency_key_present === true
    ));
  const profileEvidenceWriteOrderConstrained = retrievalEvidenceAdapters.length > 0
    && retrievalEvidenceAdapters.every((adapter) => {
      const order = adapter.write_order || [];
      return order[0]?.action === 'upsert_asset_profile'
        && order[1]?.action === 'materialize_retrieval_evidence_text'
        && order[2]?.action === 'upsert_retrieval_evidence'
        && order[3]?.action === 'mark_parse_run_completed'
        && order[3]?.requires === 'retrieval_evidence_upserted'
        && adapter.completion_gate?.do_not_mark_completed_until_evidence_upsert_succeeds === true
        && adapter.completion_gate?.partial_failure_next_status === 'retrying';
    });
  const retrievalEvidenceTextMaterialized = retrievalEvidencePreviews.every((preview) => (
    preview.retrieval_text_preview.includes('asset_title:')
    && preview.retrieval_text_preview.includes('profile_kind:')
    && preview.retrieval_text_preview.includes('summary:')
  ));
  const assetProfileRetrievalStorageMappingDryRunReady = retrievalStorageMappings.length > 0
    && retrievalStorageMappings.every((mapping) => (
      mapping.mapping_contract === 'asset_profile_retrieval_storage_mapping_v1'
      && mapping.selected_strategy === 'add_asset_retrieval_evidences_table_then_union_search'
      && mapping.ready_for_migration_design === true
      && mapping.production_write_allowed === false
      && mapping.search_integration?.method === 'union_document_and_asset_evidence_search'
    ));
  const syntheticDocumentChunkRejected = retrievalStorageMappings.length > 0
    && retrievalStorageMappings.every((mapping) => (
      mapping.current_storage_constraints?.asset_profile_has_document_chunk === false
      && (mapping.options || []).some((option) => (
        option.strategy === 'reuse_retrieval_evidences_with_synthetic_document_chunk'
        && option.recommended === false
      ))
    ));
  const assetEvidenceTableRecommended = retrievalStorageMappings.length > 0
    && retrievalStorageMappings.every((mapping) => (
      mapping.proposed_asset_evidence_table?.table_name === 'asset_retrieval_evidences'
      && (mapping.proposed_asset_evidence_table?.unique_key || []).includes('asset_id')
      && (mapping.options || []).some((option) => (
        option.strategy === 'add_asset_retrieval_evidences_table_then_union_search'
        && option.recommended === true
      ))
    ));
  const assetRetrievalEvidenceMigrationSketchReady = migrationSketches.length > 0
    && migrationSketches.every((sketch) => (
      sketch.migration_contract === 'asset_retrieval_evidences_migration_sketch_v1'
      && sketch.table?.name === 'asset_retrieval_evidences'
      && sketch.membership_guard?.required === true
      && sketch.search_result_contract?.source_kind === 'asset_profile'
      && sketch.production_migration_allowed === false
      && sketch.production_write_allowed === false
    ));
  const unionSearchNoWriteAdapterDraftReady = unionSearchAdapters.length > 0
    && unionSearchAdapters.every((adapter) => (
      adapter.adapter_contract === 'asset_profile_union_search_no_write_adapter_v1'
      && adapter.no_write === true
      && adapter.search_plan?.method === 'union_document_and_asset_evidence_search'
      && adapter.search_plan?.document_search_unchanged === true
      && adapter.result_contract?.source_kind === 'asset_profile'
      && adapter.production_write_allowed === false
    ));
  const assetSearchMembershipGuardPlanned = migrationSketches.length > 0
    && unionSearchAdapters.length > 0
    && migrationSketches.every((sketch) => (
      sketch.membership_guard?.membership_table === 'dataset_asset_memberships'
      && (sketch.membership_guard?.join_keys || []).includes('asset_id')
      && sketch.membership_guard?.hidden_asset_policy === 'deny'
    ))
    && unionSearchAdapters.every((adapter) => (
      adapter.membership_guard?.table === 'dataset_asset_memberships'
      && adapter.membership_guard?.deny_hidden_assets === true
      && adapter.membership_guard?.expiry_filter_required === true
    ));
  const unionSearchMergeRankFixtureReady = mergeRankFixtures.length > 0
    && mergeRankFixtures.every((fixture) => (
      fixture.fixture_contract === 'asset_profile_union_search_merge_rank_fixture_v1'
      && fixture.no_write === true
      && Array.isArray(fixture.merged_results)
      && fixture.merged_results.length >= 2
      && fixture.merged_results[0]?.source_kind === 'asset_profile'
      && fixture.merged_results.some((result) => result.source_kind === 'document_chunk')
      && fixture.production_write_allowed === false
    ));
  const sourceKindRegressionDryRunReady = mergeRankFixtures.length > 0
    && mergeRankFixtures.every((fixture) => (
      fixture.source_kind_regression?.mixed_sources_present === true
      && fixture.source_kind_regression?.asset_profile_source_kind_preserved === true
      && fixture.source_kind_regression?.document_source_kind_preserved === true
      && fixture.source_kind_regression?.result_source_kind_required === true
    ));
  const unionSearchMergeRankNoRawLocator = mergeRankFixtures.length > 0
    && mergeRankFixtures.every((fixture) => (
      fixture.source_kind_regression?.raw_locator_excluded === true
      && fixture.source_kind_regression?.provider_payload_excluded === true
      && (fixture.merged_results || []).every((result) => (
        result.raw_locator_included === false
        && result.provider_payload_included === false
      ))
    ));
  const unionSearchExplainDebugSummaryReady = explainDebugSummaries.length > 0
    && explainDebugSummaries.every((debug) => (
      debug.debug_contract === 'asset_profile_union_search_explain_debug_summary_v1'
      && debug.no_write === true
      && debug.summary?.top_source_kind === 'asset_profile'
      && debug.summary?.asset_profile_is_exact_document_citation === false
      && debug.redaction?.raw_locator_excluded === true
      && debug.production_write_allowed === false
    ));
  const modelFacingSupplyCompressionDryRunReady = modelFacingCompressions.length > 0
    && modelFacingCompressions.every((compression) => (
      compression.compression_contract === 'asset_profile_model_facing_supply_compression_v1'
      && compression.no_write === true
      && compression.model_supply?.compressed_text
      && compression.compression_checks?.asset_profile_exact_citation_disallowed === true
      && compression.compression_checks?.document_evidence_required_for_exact_claims === true
      && compression.production_write_allowed === false
    ));
  const modelFacingSupplyCompressionNoRawLocator = modelFacingCompressions.length > 0
    && modelFacingCompressions.every((compression) => (
      compression.compression_checks?.raw_locator_excluded === true
      && compression.compression_checks?.provider_payload_excluded === true
      && compression.compression_checks?.secret_material_excluded === true
    ));
  const assistantRunSupplyBudgetQualityGateDryRunReady = assistantRunSupplyGateDryRuns.length > 0
    && assistantRunSupplyGateDryRuns.every((gate) => (
      gate.contract === 'assistant_run_asset_document_supply_budget_quality_gate_dry_run_v1'
      && gate.no_write === true
      && gate.budget_policy?.bucket_limits?.retrieval >= 1
      && gate.budget_policy?.bucket_limits?.asset_profile >= 1
      && gate.quality_gate?.status === 'grounded_with_asset_context'
      && gate.quality_gate?.next_action === 'answer_with_citation_constraints'
      && gate.production_write_allowed === false
    ));
  const assetDocumentSupplySourceKindDedupeReady = assistantRunSupplyGateDryRuns.length > 0
    && assistantRunSupplyGateDryRuns.every((gate) => (
      (gate.source_kind_dedupe?.source_kind_order || []).includes('document_chunk')
      && (gate.source_kind_dedupe?.source_kind_order || []).includes('asset_profile')
      && gate.source_kind_dedupe?.source_kind_counts?.document_chunk === 1
      && gate.source_kind_dedupe?.source_kind_counts?.asset_profile === 1
      && gate.source_kind_dedupe?.document_chunk_and_asset_profile_keep_separate === true
      && gate.citation_policy?.asset_profile_exact_citation_allowed === false
    ));
  const insufficientSupplyContinuePlanned = assistantRunAssetOnlySupplyGateDryRuns.length > 0
    && assistantRunAssetOnlySupplyGateDryRuns.every((gate) => (
      gate.quality_gate?.status === 'partial_asset_context_only'
      && gate.quality_gate?.needs_more_supply === true
      && gate.quality_gate?.next_action === 'continue_expand_supply'
      && (gate.quality_gate?.continue_when || []).includes('missing_document_evidence_for_exact_claim')
    ));
  const assistantRunSupplyGateNoRawLocator = [
    ...assistantRunSupplyGateDryRuns,
    ...assistantRunAssetOnlySupplyGateDryRuns,
  ].every((gate) => (
    !/object[_-]?key/i.test(JSON.stringify(gate))
    && !/raw_provider_payload/i.test(JSON.stringify(gate))
  ));
  const assistantRunSupplyProgressEventsDryRunReady = assistantRunSupplyProgressEvents.length > 0
    && assistantRunSupplyProgressEvents.every((progress) => (
      progress.contract === 'assistant_run_supply_progress_events_dry_run_v1'
      && progress.no_write === true
      && progress.non_blocking === true
      && progress.continuation_policy?.answer_with_current_evidence_first === true
      && progress.continuation_policy?.continue_actions_after_reply === true
      && progress.continuation_policy?.final_failure_without_answer === false
      && hasAssistantRunProgressEvent(progress, 'supply_ready')
      && hasAssistantRunProgressEvent(progress, 'asset_profile_signal')
      && progress.production_write_allowed === false
    ));
  const assistantRunProgressNonBlocking = [
    ...assistantRunSupplyProgressEvents,
    ...assistantRunAssetOnlySupplyProgressEvents,
    ...assistantRunParseProgressEvents,
  ].every((progress) => (
    progress.non_blocking === true
    && (progress.events || []).every((event) => (
      event.blocking === false
      && event.third_party_visible === true
      && event.task_card_visible === true
      && event.sensitive_payload_included === false
    ))
  ));
  const assistantRunProgressInsufficientSupplyContinues = assistantRunAssetOnlySupplyProgressEvents.length > 0
    && assistantRunAssetOnlySupplyProgressEvents.every((progress) => (
      progress.quality_gate_next_action === 'continue_expand_supply'
      && hasAssistantRunProgressEvent(progress, 'supply_expanding')
      && progress.continuation_policy?.final_failure_without_answer === false
      && progress.continuation_policy?.parse_or_supply_gap_is_not_terminal === true
    ));
  const assistantRunProgressParseWaitingOrRetryVisible = assistantRunParseProgressEvents.length > 0
    && assistantRunParseProgressEvents.every((progress) => (
      hasAssistantRunProgressEvent(progress, 'parse_waiting_or_retry')
      && progress.continuation_policy?.parse_or_supply_gap_is_not_terminal === true
    ));
  const assistantRunProgressNoRawLocator = [
    ...assistantRunSupplyProgressEvents,
    ...assistantRunAssetOnlySupplyProgressEvents,
    ...assistantRunParseProgressEvents,
  ].every((progress) => (
    !/object[_-]?key/i.test(JSON.stringify(progress))
    && !/raw_provider_payload/i.test(JSON.stringify(progress))
  ));
  const assistantRunProgressContractDriftGuardReady = assistantRunProgressContractDriftGuards.length > 0
    && assistantRunProgressContractDriftGuards.every((guard) => (
      guard.contract === 'assistant_run_supply_progress_contract_drift_guard_dry_run_v1'
      && guard.no_write === true
      && guard.external_sse_guard?.schema === 'v3.external_channel.sse.v1'
      && guard.external_sse_guard?.new_progress_events_are_internal_dry_run_only === true
      && guard.production_write_allowed === false
    ));
  const assistantRunProgressDoesNotMutatePublicSseFields = assistantRunProgressContractDriftGuards.length > 0
    && assistantRunProgressContractDriftGuards.every((guard) => (
      guard.external_sse_guard?.public_stream_field_mutation_allowed === false
      && guard.external_sse_guard?.new_progress_events_emit_live_sse === false
      && (guard.external_sse_guard?.public_envelope_fields || []).includes('display_text')
      && (guard.external_sse_guard?.public_envelope_fields || []).includes('poll_after_seconds')
    ));
  const assistantRunProgressNoCallbackOrWrite = assistantRunProgressContractDriftGuards.length > 0
    && assistantRunProgressContractDriftGuards.every((guard) => (
      guard.third_party_guard?.callback_triggered === false
      && guard.third_party_guard?.public_request_or_response_field_added === false
      && guard.third_party_guard?.requires_new_third_party_contract === false
      && guard.production_write_allowed === false
    ));
  const assistantRunProgressFailureDoesNotBlockAnswer = assistantRunProgressContractDriftGuards.length > 0
    && assistantRunProgressContractDriftGuards.every((guard) => (
      guard.answer_liveness_guard?.failure_status_blocks_answer === false
      && guard.answer_liveness_guard?.parse_or_supply_gap_is_terminal === false
      && guard.answer_liveness_guard?.answer_with_current_evidence_first === true
      && guard.answer_liveness_guard?.continue_actions_after_reply === true
      && guard.answer_liveness_guard?.final_failure_without_answer === false
    ));
  const assistantRunProgressTaskCardDetailOnly = assistantRunProgressContractDriftGuards.length > 0
    && assistantRunProgressContractDriftGuards.every((guard) => (
      guard.main_site_task_card_guard?.expected_consumer === 'artifact_task_cards'
      && guard.main_site_task_card_guard?.allowed_surface === 'detail_progress_summary'
      && guard.main_site_task_card_guard?.creates_task_card_without_user_action === false
      && guard.main_site_task_card_guard?.mutates_task_card_status_enum === false
      && guard.main_site_task_card_guard?.task_card_detail_only === true
    ));
  const parserLiveAdapterReadinessDryRunReady = parserLiveAdapterReadinessDryRuns.length > 0
    && parserLiveAdapterReadinessDryRuns.some((readiness) => (
      readiness.contract === 'fashion_design_image_parser_live_adapter_readiness_dry_run_v1'
      && readiness.adapter_status === 'ready_for_profile_and_retrieval_commit'
      && readiness.adapter_ready === true
      && readiness.validation?.retrieval_evidence_ready === true
    ))
    && parserLiveAdapterReadinessDryRuns.some((readiness) => (
      readiness.adapter_status === 'ready_for_partial_retry_adapter'
      && readiness.adapter_ready === true
      && readiness.validation?.retrieval_evidence_ready === false
    ));
  const parserLiveAdapterNoNetworkOrModelCall = parserLiveAdapterReadinessDryRuns.length > 0
    && parserLiveAdapterReadinessDryRuns.every((readiness) => (
      readiness.no_network === true
      && readiness.no_model_call === true
      && readiness.no_write === true
      && readiness.no_callback === true
      && readiness.production_write_allowed === false
    ));
  const parserLiveAdapterRequiresEndpointOnly = parserLiveAdapterReadinessDryRuns.length > 0
    && parserLiveAdapterReadinessDryRuns.every((readiness) => (
      readiness.live_execute_still_requires_worker_endpoint === true
      && readiness.worker_endpoint_value_included === false
      && readiness.worker_request_payload_included === false
      && readiness.worker_response_payload_included === false
    ));
  const parserLiveAdapterContractsStable = parserLiveAdapterReadinessDryRuns.length > 0
    && parserLiveAdapterReadinessDryRuns.every((readiness) => (
      readiness.validation?.worker_input_contract_ok === true
      && readiness.validation?.worker_output_contract_ok === true
      && readiness.validation?.worker_output_accepted === true
      && readiness.validation?.sensitive_material_excluded === true
    ));
  const parserLiveAdapterNoRawPayload = parserLiveAdapterReadinessDryRuns.length > 0
    && parserLiveAdapterReadinessDryRuns.every((readiness) => (
      readiness.redaction?.provider_payload_excluded === true
      && readiness.redaction?.provider_raw_material_excluded === true
      && readiness.redaction?.raw_object_locator_excluded === true
    ));
  const parserLiveAdapterRedacted = !/https?:\/\//i.test(JSON.stringify(parserLiveAdapterReadinessDryRuns))
    && !/object[_-]?key/i.test(JSON.stringify(parserLiveAdapterReadinessDryRuns))
    && !/raw_provider_payload/i.test(JSON.stringify(parserLiveAdapterReadinessDryRuns))
    && !JSON.stringify(parserLiveAdapterReadinessDryRuns).includes(ROOT_DIR);
  const assetRetrievalEvidenceWriterReadinessReady =
    assetRetrievalEvidenceWriterReadinessDryRuns.length > 0
    && assetRetrievalEvidenceWriterReadinessDryRuns.every((readiness) => (
      readiness.contract
        === 'fashion_design_asset_retrieval_evidence_live_writer_readiness_dry_run_v1'
      && readiness.writer_status === 'ready_for_controlled_writer_review'
      && readiness.writer_ready_for_operator_review === true
      && readiness.production_write_allowed === false
    ));
  const assetRetrievalEvidenceWriterFieldLedgerReady =
    assetRetrievalEvidenceWriterReadinessDryRuns.length > 0
    && assetRetrievalEvidenceWriterReadinessDryRuns.every((readiness) => (
      readiness.field_ledger?.record_counts_only === true
      && readiness.field_ledger?.raw_locator_fields_allowed === false
      && readiness.field_ledger?.provider_payload_fields_allowed === false
      && (readiness.field_ledger?.allowed_shared_receipt_fields || []).includes('membership_guard')
    ));
  const assetRetrievalEvidenceWriterIdempotent =
    assetRetrievalEvidenceWriterReadinessDryRuns.length > 0
    && assetRetrievalEvidenceWriterReadinessDryRuns.every((readiness) => (
      readiness.idempotency?.strategy === 'upsert_by_dataset_asset_profile_parser_version'
      && readiness.idempotency?.key_template_redacted === true
      && (readiness.idempotency?.dedupe_scope || []).includes('asset_id')
      && readiness.idempotency?.retry_safe === true
    ));
  const assetRetrievalEvidenceWriterMembershipGuardReady =
    assetRetrievalEvidenceWriterReadinessDryRuns.length > 0
    && assetRetrievalEvidenceWriterReadinessDryRuns.every((readiness) => (
      readiness.membership_guard?.table === 'dataset_asset_memberships'
      && readiness.membership_guard?.tenant_scope_required === true
      && readiness.membership_guard?.dataset_scope_required === true
      && readiness.membership_guard?.deny_hidden_assets === true
    ));
  const assetRetrievalEvidenceWriterRollbackAuditReady =
    assetRetrievalEvidenceWriterReadinessDryRuns.length > 0
    && assetRetrievalEvidenceWriterReadinessDryRuns.every((readiness) => (
      readiness.rollback_requirements?.reviewed_manifest_required === true
      && readiness.rollback_requirements?.automatic_rollback_allowed === false
      && readiness.audit_requirements?.record_counts_only === true
      && readiness.audit_requirements?.record_provider_payload === false
    ));
  const assetRetrievalEvidenceWriterRedacted =
    !/https?:\/\//i.test(JSON.stringify(assetRetrievalEvidenceWriterReadinessDryRuns))
    && !/object[_-]?key/i.test(JSON.stringify(assetRetrievalEvidenceWriterReadinessDryRuns))
    && !/raw_provider_payload/i.test(JSON.stringify(assetRetrievalEvidenceWriterReadinessDryRuns))
    && !JSON.stringify(assetRetrievalEvidenceWriterReadinessDryRuns).includes(ROOT_DIR);
  const thirdPartyPrivateAssetImportEndpointGuardReady =
    thirdPartyPrivateAssetImportEndpointGuard.contract
      === 'fashion_design_third_party_asset_import_private_endpoint_guard_dry_run_v1'
    && thirdPartyPrivateAssetImportEndpointGuard.endpoint_status
      === 'ready_for_private_endpoint_review'
    && thirdPartyPrivateAssetImportEndpointGuard.private_endpoint_ready === true
    && thirdPartyPrivateAssetImportEndpointGuard.production_write_allowed === false;
  const thirdPartyPrivateAssetImportDefaultPrivate =
    thirdPartyPrivateAssetImportEndpointGuard.privacy_policy?.third_party_assets_private_by_default === true
    && thirdPartyPrivateAssetImportEndpointGuard.privacy_policy?.main_site_default_visibility === false
    && thirdPartyPrivateAssetImportEndpointGuard.privacy_policy?.tenant_scope_required === true
    && thirdPartyPrivateAssetImportEndpointGuard.privacy_policy?.connection_scope_required === true;
  const thirdPartyPrivateAssetImportStructuredJsonReady =
    thirdPartyPrivateAssetImportEndpointGuard.response_contract?.reply_shape === 'structured_json'
    && thirdPartyPrivateAssetImportEndpointGuard.response_contract?.customer_storage_ready === true
    && (thirdPartyPrivateAssetImportEndpointGuard.response_contract?.required_fields || [])
      .includes('assets');
  const thirdPartyPrivateAssetImportScopeGuardReady =
    thirdPartyPrivateAssetImportEndpointGuard.dataset_attachment_policy?.dataset_external_ids_supported === true
    && thirdPartyPrivateAssetImportEndpointGuard.dataset_attachment_policy?.asset_library_external_id_supported === true
    && thirdPartyPrivateAssetImportEndpointGuard.dataset_attachment_policy
      ?.membership_write_requires_private_endpoint_execute === true;
  const thirdPartyPrivateAssetImportPublicDocsClosed =
    thirdPartyPrivateAssetImportEndpointGuard.public_docs_asset_imports_closed === true
    && thirdPartyPrivateAssetImportEndpointGuard.public_contract_changed === false
    && thirdPartyPrivateAssetImportEndpointGuard.operator_controls
      ?.public_docs_change_required_before_general_release === true;
  const thirdPartyPrivateAssetImportEndpointGuardRedacted =
    !/https?:\/\//i.test(JSON.stringify(thirdPartyPrivateAssetImportEndpointGuard))
    && !/object[_-]?key/i.test(JSON.stringify(thirdPartyPrivateAssetImportEndpointGuard))
    && !/raw_provider_payload/i.test(JSON.stringify(thirdPartyPrivateAssetImportEndpointGuard))
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i
      .test(JSON.stringify(thirdPartyPrivateAssetImportEndpointGuard))
    && !JSON.stringify(thirdPartyPrivateAssetImportEndpointGuard).includes(ROOT_DIR);
  const mainSiteGalleryTaskCardReady =
    mainSiteGalleryTaskCardDryRun.contract
      === 'fashion_design_main_site_gallery_task_card_dry_run_v1'
    && mainSiteGalleryTaskCardDryRun.task_status === 'ready_for_task_card_dry_run'
    && mainSiteGalleryTaskCardDryRun.task_card_ready === true
    && mainSiteGalleryTaskCard.type === 'asset_gallery_import_task';
  const mainSiteGalleryTaskCardStable =
    mainSiteGalleryTaskCard.behavior?.cardPersistsAfterCreation === true
    && mainSiteGalleryTaskCard.behavior?.selectedCardRefreshOnly === true
    && mainSiteGalleryTaskCard.behavior?.noGlobalPolling === true
    && mainSiteGalleryTaskCardDryRun.refresh_policy?.card_persists_after_creation === true
    && mainSiteGalleryTaskCardDryRun.refresh_policy?.selected_card_refresh_only === true
    && mainSiteGalleryTaskCardDryRun.refresh_policy?.no_global_polling === true;
  const mainSiteGalleryTaskCardDetailReady =
    mainSiteGalleryTaskCard.detail?.dataSources?.rawValuesIncluded === false
    && mainSiteGalleryTaskCard.detail?.fieldLedger?.recordCountsOnly === true
    && mainSiteGalleryTaskCard.detail?.validationReceipt?.noUnrelatedArtifactInjection === true
    && mainSiteGalleryTaskCardDryRun.detail_contract?.shows_data_sources === true
    && mainSiteGalleryTaskCardDryRun.detail_contract?.shows_field_ledger === true
    && mainSiteGalleryTaskCardDryRun.detail_contract?.shows_parse_status === true;
  const mainSiteGalleryTaskCardNoSideEffects =
    mainSiteGalleryTaskCardDryRun.no_write === true
    && mainSiteGalleryTaskCardDryRun.no_task_creation === true
    && mainSiteGalleryTaskCardDryRun.no_artifact_creation === true
    && mainSiteGalleryTaskCardDryRun.no_chat_message === true
    && mainSiteGalleryTaskCardDryRun.refresh_policy?.no_unrelated_artifact_injection === true;
  const mainSiteGalleryTaskCardRedacted =
    !/https?:\/\//i.test(JSON.stringify({
      mainSiteGalleryTaskCard,
      mainSiteGalleryTaskCardDryRun,
    }))
    && !/object[_-]?key/i.test(JSON.stringify({
      mainSiteGalleryTaskCard,
      mainSiteGalleryTaskCardDryRun,
    }))
    && !/raw_provider_payload/i.test(JSON.stringify({
      mainSiteGalleryTaskCard,
      mainSiteGalleryTaskCardDryRun,
    }))
    && !JSON.stringify({
      mainSiteGalleryTaskCard,
      mainSiteGalleryTaskCardDryRun,
    }).includes(ROOT_DIR);
  const summary = {
    parseQueueDryRunReady: pendingItems.length > 0,
    parseQueuePendingCount: pendingItems.length,
    parseQueueTaskKind: 'fashion_design_image_ocr_vlm',
    parseQueueParser: 'datamax-fashion-image-parser@2026-06-17',
    parseQueueWritePolicy: 'dry_run_only_without_execute',
    parseStatusTransitionDryRunReady,
    parseStatusTransitionCount: parseStatusTransitions.length,
    parseStatusTransitionNextStatuses: parseStatusTransitions
      .map((transition) => transition.next_status)
      .filter((status, index, statuses) => statuses.indexOf(status) === index),
    retrievalEvidenceDryRunReady: retrievalEvidencePreviews.length > 0,
    retrievalEvidenceTextMaterialized,
    retrievalEvidenceIdempotencyPlanned,
    retrievalEvidenceAdapterDryRunReady,
    profileEvidenceWriteOrderConstrained,
    retrievalEvidenceWritePlanCount: retrievalEvidenceWritePlans.length,
    retrievalEvidenceAdapterCount: retrievalEvidenceAdapters.length,
    retrievalStorageMappingCount: retrievalStorageMappings.length,
    migrationSketchCount: migrationSketches.length,
    unionSearchAdapterCount: unionSearchAdapters.length,
    unionSearchMergeRankFixtureCount: mergeRankFixtures.length,
    unionSearchExplainDebugSummaryCount: explainDebugSummaries.length,
    modelFacingSupplyCompressionCount: modelFacingCompressions.length,
    assistantRunSupplyGateDryRunCount: assistantRunSupplyGateDryRuns.length,
    retrievalEvidencePreviewCount: retrievalEvidencePreviews.length,
    retrievalEvidenceSourceKind: 'asset_profile',
    retrievalEvidenceWritePolicy: 'upsert_by_idempotency_key_after_profile_available',
    assetProfileRetrievalStorageMappingDryRunReady,
    syntheticDocumentChunkRejected,
    assetEvidenceTableRecommended,
    assetRetrievalEvidenceMigrationSketchReady,
    unionSearchNoWriteAdapterDraftReady,
    assetSearchMembershipGuardPlanned,
    unionSearchMergeRankFixtureReady,
    sourceKindRegressionDryRunReady,
    unionSearchMergeRankNoRawLocator,
    unionSearchExplainDebugSummaryReady,
    modelFacingSupplyCompressionDryRunReady,
    modelFacingSupplyCompressionNoRawLocator,
    assistantRunSupplyBudgetQualityGateDryRunReady,
    assetDocumentSupplySourceKindDedupeReady,
    insufficientSupplyContinuePlanned,
    assistantRunSupplyGateNoRawLocator,
    assistantRunSupplyProgressEventsDryRunReady,
    assistantRunProgressNonBlocking,
    assistantRunProgressInsufficientSupplyContinues,
    assistantRunProgressParseWaitingOrRetryVisible,
    assistantRunProgressNoRawLocator,
    assistantRunProgressContractDriftGuardReady,
    assistantRunProgressDoesNotMutatePublicSseFields,
    assistantRunProgressNoCallbackOrWrite,
    assistantRunProgressFailureDoesNotBlockAnswer,
    assistantRunProgressTaskCardDetailOnly,
    parserLiveAdapterReadinessDryRunReady,
    parserLiveAdapterReadinessDryRunCount: parserLiveAdapterReadinessDryRuns.length,
    parserLiveAdapterStatuses: parserLiveAdapterReadinessDryRuns
      .map((readiness) => readiness.adapter_status)
      .filter((status, index, statuses) => statuses.indexOf(status) === index),
    parserLiveAdapterNoNetworkOrModelCall,
    parserLiveAdapterRequiresEndpointOnly,
    parserLiveAdapterContractsStable,
    parserLiveAdapterNoRawPayload,
    parserLiveAdapterRedacted,
    assetRetrievalEvidenceWriterReadinessReady,
    assetRetrievalEvidenceWriterReadinessCount: assetRetrievalEvidenceWriterReadinessDryRuns.length,
    assetRetrievalEvidenceWriterStatuses: assetRetrievalEvidenceWriterReadinessDryRuns
      .map((readiness) => readiness.writer_status)
      .filter((status, index, statuses) => statuses.indexOf(status) === index),
    assetRetrievalEvidenceWriterFieldLedgerReady,
    assetRetrievalEvidenceWriterIdempotent,
    assetRetrievalEvidenceWriterMembershipGuardReady,
    assetRetrievalEvidenceWriterRollbackAuditReady,
    assetRetrievalEvidenceWriterRedacted,
    thirdPartyPrivateAssetImportEndpointGuardReady,
    thirdPartyPrivateAssetImportEndpointStatus:
      thirdPartyPrivateAssetImportEndpointGuard.endpoint_status,
    thirdPartyPrivateAssetImportDefaultPrivate,
    thirdPartyPrivateAssetImportStructuredJsonReady,
    thirdPartyPrivateAssetImportScopeGuardReady,
    thirdPartyPrivateAssetImportPublicDocsClosed,
    thirdPartyPrivateAssetImportEndpointGuardRedacted,
    mainSiteGalleryTaskCardReady,
    mainSiteGalleryTaskCardStatus: mainSiteGalleryTaskCardDryRun.task_status,
    mainSiteGalleryTaskCardStable,
    mainSiteGalleryTaskCardDetailReady,
    mainSiteGalleryTaskCardNoSideEffects,
    mainSiteGalleryTaskCardRedacted,
    followupDryRunRedacted: true,
  };

  assert.equal(summary.parseQueuePendingCount, 4);
  assert.equal(summary.parseStatusTransitionDryRunReady, true);
  assert.equal(summary.retrievalEvidencePreviewCount, 2);
  assert.equal(summary.retrievalEvidenceTextMaterialized, true);
  assert.equal(summary.retrievalEvidenceIdempotencyPlanned, true);
  assert.equal(summary.retrievalEvidenceAdapterDryRunReady, true);
  assert.equal(summary.profileEvidenceWriteOrderConstrained, true);
  assert.equal(summary.assetProfileRetrievalStorageMappingDryRunReady, true);
  assert.equal(summary.syntheticDocumentChunkRejected, true);
  assert.equal(summary.assetEvidenceTableRecommended, true);
  assert.equal(summary.assetRetrievalEvidenceMigrationSketchReady, true);
  assert.equal(summary.unionSearchNoWriteAdapterDraftReady, true);
  assert.equal(summary.assetSearchMembershipGuardPlanned, true);
  assert.equal(summary.unionSearchMergeRankFixtureReady, true);
  assert.equal(summary.sourceKindRegressionDryRunReady, true);
  assert.equal(summary.unionSearchMergeRankNoRawLocator, true);
  assert.equal(summary.unionSearchExplainDebugSummaryReady, true);
  assert.equal(summary.modelFacingSupplyCompressionDryRunReady, true);
  assert.equal(summary.modelFacingSupplyCompressionNoRawLocator, true);
  assert.equal(summary.assistantRunSupplyBudgetQualityGateDryRunReady, true);
  assert.equal(summary.assetDocumentSupplySourceKindDedupeReady, true);
  assert.equal(summary.insufficientSupplyContinuePlanned, true);
  assert.equal(summary.assistantRunSupplyGateNoRawLocator, true);
  assert.equal(summary.assistantRunSupplyProgressEventsDryRunReady, true);
  assert.equal(summary.assistantRunProgressNonBlocking, true);
  assert.equal(summary.assistantRunProgressInsufficientSupplyContinues, true);
  assert.equal(summary.assistantRunProgressParseWaitingOrRetryVisible, true);
  assert.equal(summary.assistantRunProgressNoRawLocator, true);
  assert.equal(summary.assistantRunProgressContractDriftGuardReady, true);
  assert.equal(summary.assistantRunProgressDoesNotMutatePublicSseFields, true);
  assert.equal(summary.assistantRunProgressNoCallbackOrWrite, true);
  assert.equal(summary.assistantRunProgressFailureDoesNotBlockAnswer, true);
  assert.equal(summary.assistantRunProgressTaskCardDetailOnly, true);
  assert.equal(summary.parserLiveAdapterReadinessDryRunReady, true);
  assert.equal(summary.parserLiveAdapterNoNetworkOrModelCall, true);
  assert.equal(summary.parserLiveAdapterRequiresEndpointOnly, true);
  assert.equal(summary.parserLiveAdapterContractsStable, true);
  assert.equal(summary.parserLiveAdapterNoRawPayload, true);
  assert.equal(summary.parserLiveAdapterRedacted, true);
  assert.equal(summary.assetRetrievalEvidenceWriterReadinessReady, true);
  assert.equal(summary.assetRetrievalEvidenceWriterFieldLedgerReady, true);
  assert.equal(summary.assetRetrievalEvidenceWriterIdempotent, true);
  assert.equal(summary.assetRetrievalEvidenceWriterMembershipGuardReady, true);
  assert.equal(summary.assetRetrievalEvidenceWriterRollbackAuditReady, true);
  assert.equal(summary.assetRetrievalEvidenceWriterRedacted, true);
  assert.equal(summary.thirdPartyPrivateAssetImportEndpointGuardReady, true);
  assert.equal(summary.thirdPartyPrivateAssetImportDefaultPrivate, true);
  assert.equal(summary.thirdPartyPrivateAssetImportStructuredJsonReady, true);
  assert.equal(summary.thirdPartyPrivateAssetImportScopeGuardReady, true);
  assert.equal(summary.thirdPartyPrivateAssetImportPublicDocsClosed, true);
  assert.equal(summary.thirdPartyPrivateAssetImportEndpointGuardRedacted, true);
  assert.equal(summary.mainSiteGalleryTaskCardReady, true);
  assert.equal(summary.mainSiteGalleryTaskCardStable, true);
  assert.equal(summary.mainSiteGalleryTaskCardDetailReady, true);
  assert.equal(summary.mainSiteGalleryTaskCardNoSideEffects, true);
  assert.equal(summary.mainSiteGalleryTaskCardRedacted, true);
  const serialized = JSON.stringify({
    summary,
    parseStatusTransitions,
    retrievalEvidencePreviews,
    retrievalEvidenceWritePlans,
    retrievalEvidenceAdapters,
    retrievalStorageMappings,
    migrationSketches,
    unionSearchAdapters,
    mergeRankFixtures,
    explainDebugSummaries,
    modelFacingCompressions,
    assistantRunSupplyGateDryRuns,
    assistantRunAssetOnlySupplyGateDryRuns,
    assistantRunSupplyProgressEvents,
    assistantRunAssetOnlySupplyProgressEvents,
    assistantRunParseProgressEvents,
    assistantRunProgressContractDriftGuards,
    parserLiveAdapterReadinessDryRuns,
    assetRetrievalEvidenceWriterReadinessDryRuns,
    thirdPartyPrivateAssetImportEndpointGuard,
    mainSiteGalleryTaskCard,
    mainSiteGalleryTaskCardDryRun,
  });
  assert.equal(/object[_-]?key/i.test(serialized), false);
  assert.equal(/raw_provider_payload/i.test(serialized), false);
  assert.equal(serialized.includes(ROOT_DIR), false);

  return {
    parseStatusTransitions,
    retrievalEvidencePreviews,
    retrievalEvidenceWritePlans,
    retrievalEvidenceAdapters,
    retrievalStorageMappings,
    migrationSketches,
    unionSearchAdapters,
    mergeRankFixtures,
    explainDebugSummaries,
    modelFacingCompressions,
    assistantRunSupplyGateDryRuns,
    assistantRunAssetOnlySupplyGateDryRuns,
    assistantRunSupplyProgressEvents,
    assistantRunAssetOnlySupplyProgressEvents,
    assistantRunParseProgressEvents,
    assistantRunProgressContractDriftGuards,
    parserLiveAdapterReadinessDryRuns,
    assetRetrievalEvidenceWriterReadinessDryRuns,
    thirdPartyPrivateAssetImportEndpointGuard,
    mainSiteGalleryTaskCard,
    mainSiteGalleryTaskCardDryRun,
    summary,
  };
}

function buildParserStatusTransitionDryRun(currentStatus, retrievalReady) {
  const normalized = normalizeParserStatus(currentStatus);
  const nextStatus = normalized === 'completed' ? 'completed' : (retrievalReady ? 'completed' : 'retrying');
  const transitionAction = normalized === 'completed'
    ? 'keep_completed'
    : (retrievalReady ? 'mark_completed_and_materialize_retrieval_evidence' : 'mark_retrying_wait_for_profile');
  return {
    task_kind: 'fashion_design_image_ocr_vlm',
    parser_name: 'datamax-fashion-image-parser',
    parser_version: '2026-06-17',
    profile_schema: 'fashion_design_image_v1',
    current_status: normalized,
    worker_status: retrievalReady ? 'completed' : 'partial_completed',
    next_status: nextStatus,
    transition_action: transitionAction,
    should_write_profile: retrievalReady,
    should_materialize_retrieval_evidence: retrievalReady,
    dry_run_only: true,
  };
}

function normalizeParserStatus(status) {
  const value = String(status || '').trim().toLowerCase();
  if (['queued', 'pending'].includes(value)) return 'pending';
  if (['running', 'processing', 'parsing'].includes(value)) return 'parsing';
  if (['retry', 'retrying'].includes(value)) return 'retrying';
  if (['done', 'succeeded', 'success', 'completed'].includes(value)) return 'completed';
  if (['partial', 'partial_completed'].includes(value)) return 'partial_completed';
  if (['failed', 'error'].includes(value)) return 'failed';
  return 'unknown';
}

function buildParserWorkerInputContract({
  sourceRefPresent = true,
  sourceKind = 'object_key',
  contentType = 'image/png',
  profileSeeded = false,
} = {}) {
  return {
    task_kind: 'fashion_design_image_ocr_vlm',
    parser_name: 'datamax-fashion-image-parser',
    parser_version: '2026-06-17',
    profile_schema: 'fashion_design_image_v1',
    source_ref_present: Boolean(sourceRefPresent),
    source_kind: sourceKind,
    content_type: contentType,
    profile_seeded: Boolean(profileSeeded),
    requested_outputs: [
      'fashion_design_image_v1',
      'retrieval_evidence_text',
    ],
  };
}

function buildParserWorkerOutputContract({
  profileReady = true,
  retrievalReady = true,
} = {}) {
  return {
    status: profileReady ? 'completed' : 'partial_completed',
    parser_name: 'datamax-fashion-image-parser',
    parser_version: '2026-06-17',
    profile_schema: 'fashion_design_image_v1',
    profile_payload: profileReady
      ? {
        category: 'dress',
        colors: ['green', 'white'],
        caption: '春夏连衣裙灵感图',
      }
      : {},
    retrieval_evidence: {
      source_kind: 'asset_profile',
      materialization: 'profile_to_text',
      ready: Boolean(retrievalReady),
    },
  };
}

function buildParserLiveAdapterReadinessDryRun(workerInputContract, workerOutputContract) {
  const workerStatus = normalizeParserStatus(workerOutputContract?.status);
  const inputContractOk = parserWorkerInputContractOk(workerInputContract);
  const outputContractOk = parserWorkerOutputContractOk(workerOutputContract);
  const workerOutputAccepted = ['completed', 'partial_completed'].includes(workerStatus);
  const retrievalReady = Boolean(workerOutputContract?.retrieval_evidence?.ready);
  const profilePayloadReady = Object.keys(workerOutputContract?.profile_payload || {}).length > 0;
  const sensitiveMaterialExcluded = !containsForbiddenParserContractMaterial(workerInputContract)
    && !containsForbiddenParserContractMaterial(workerOutputContract);
  const adapterReady = inputContractOk
    && outputContractOk
    && workerOutputAccepted
    && sensitiveMaterialExcluded;
  const adapterStatus = !inputContractOk
    ? 'blocked_worker_input_contract'
    : !outputContractOk
      ? 'blocked_worker_output_contract'
      : !workerOutputAccepted
        ? 'blocked_worker_status'
        : !sensitiveMaterialExcluded
          ? 'blocked_sensitive_material'
          : retrievalReady
            ? 'ready_for_profile_and_retrieval_commit'
            : 'ready_for_partial_retry_adapter';
  const nextAction = !adapterReady
    ? 'review_blocked_adapter_contract'
    : retrievalReady
      ? 'controlled_live_adapter_commit_review'
      : 'controlled_live_adapter_retry_review';

  return {
    contract: 'fashion_design_image_parser_live_adapter_readiness_dry_run_v1',
    mode: 'dry_run',
    task_kind: 'fashion_design_image_ocr_vlm',
    parser_name: 'datamax-fashion-image-parser',
    parser_version: '2026-06-17',
    profile_schema: 'fashion_design_image_v1',
    adapter_status: adapterStatus,
    adapter_ready: adapterReady,
    no_network: true,
    no_model_call: true,
    no_write: true,
    no_callback: true,
    production_write_allowed: false,
    live_execute_still_requires_worker_endpoint: true,
    worker_endpoint_value_included: false,
    worker_request_payload_included: false,
    worker_response_payload_included: false,
    validation: {
      worker_input_contract_ok: inputContractOk,
      worker_output_contract_ok: outputContractOk,
      worker_output_accepted: workerOutputAccepted,
      worker_status: workerStatus,
      profile_payload_ready: profilePayloadReady,
      retrieval_evidence_ready: retrievalReady,
      sensitive_material_excluded: sensitiveMaterialExcluded,
    },
    operator_next_step: {
      action: nextAction,
      worker_endpoint_required: true,
      worker_endpoint_value_included: false,
      operator_review_required: true,
    },
    redaction: {
      provider_raw_material_excluded: true,
      provider_payload_excluded: true,
      raw_object_locator_excluded: true,
      source_url_value_included: false,
      local_path_value_included: false,
      secret_material_included: false,
    },
  };
}

function parserWorkerInputContractOk(contract) {
  return contract?.task_kind === 'fashion_design_image_ocr_vlm'
    && contract?.parser_name === 'datamax-fashion-image-parser'
    && contract?.parser_version === '2026-06-17'
    && contract?.profile_schema === 'fashion_design_image_v1'
    && contract?.source_ref_present === true
    && ['external_id', 'object_key', 'image_url'].includes(contract?.source_kind)
    && Array.isArray(contract?.requested_outputs)
    && contract.requested_outputs.includes('fashion_design_image_v1')
    && contract.requested_outputs.includes('retrieval_evidence_text');
}

function parserWorkerOutputContractOk(contract) {
  return contract?.parser_name === 'datamax-fashion-image-parser'
    && contract?.parser_version === '2026-06-17'
    && contract?.profile_schema === 'fashion_design_image_v1'
    && ['completed', 'partial_completed'].includes(normalizeParserStatus(contract?.status))
    && contract?.retrieval_evidence?.source_kind === 'asset_profile'
    && contract?.retrieval_evidence?.materialization === 'profile_to_text';
}

function containsForbiddenParserContractMaterial(value) {
  if (!value) return false;
  if (Array.isArray(value)) {
    return value.some(containsForbiddenParserContractMaterial);
  }
  if (typeof value === 'object') {
    return Object.entries(value).some(([key, item]) => (
      forbiddenParserContractKey(key) || containsForbiddenParserContractMaterial(item)
    ));
  }
  if (typeof value === 'string') {
    return forbiddenParserContractValue(value);
  }
  return false;
}

function forbiddenParserContractKey(key) {
  return [
    'raw_provider_payload',
    'provider_payload',
    'unknown_provider_blob',
    'object_key',
    'local_path',
    'image_url',
    'source_url',
    'url',
    'bearer',
    'authorization',
    'cookie',
  ].includes(String(key || '').toLowerCase());
}

function forbiddenParserContractValue(value) {
  const text = String(value || '').trim();
  const lower = text.toLowerCase();
  return lower.includes('http://')
    || lower.includes('https://')
    || lower.includes('objects/private')
    || lower.includes('raw_provider_payload')
    || lower.includes('bearer ')
    || lower.includes('authorization:')
    || lower.includes('cookie=')
    || text.includes(':\\')
    || lower.includes('/users/');
}

function buildRetrievalEvidenceWritePlan(preview) {
  const assetToken = safePlanToken(preview.asset_id);
  const profileToken = safePlanToken(preview.profile_kind);
  const parserToken = safePlanToken('datamax-fashion-image-parser');
  const parserVersionToken = safePlanToken('2026-06-17');
  return {
    action: 'upsert_retrieval_evidence',
    source_kind: 'asset_profile',
    source_locator: `asset-profile://${assetToken}/${profileToken}`,
    idempotency_key: `asset-profile:${assetToken}:${profileToken}:${parserToken}:${parserVersionToken}`,
    dedupe_scope: ['tenant_id', 'dataset_id', 'asset_id', 'profile_kind', 'parser_name', 'parser_version'],
    profile_schema: preview.profile_kind,
    parser_name: 'datamax-fashion-image-parser',
    parser_version: '2026-06-17',
    write_policy: 'upsert_by_idempotency_key_after_profile_available',
    evidence_text_present: Boolean(preview.retrieval_text_preview),
    evidence_text_chars: String(preview.retrieval_text_preview || '').length,
    dry_run_only: true,
    ready: Boolean(preview.retrieval_text_preview),
  };
}

function buildRetrievalEvidenceAdapterDryRun(preview) {
  const writePlan = buildRetrievalEvidenceWritePlan(preview);
  return {
    adapter_contract: 'asset_profile_retrieval_evidence_writer_v1',
    mode: 'dry_run',
    status: writePlan.ready ? 'ready' : 'blocked',
    source_kind: 'asset_profile',
    profile_schema: preview.profile_kind,
    input_contract: {
      asset_id_present: Boolean(preview.asset_id),
      profile_schema_present: Boolean(preview.profile_kind),
      requires_profile_ready: true,
      requires_dataset_membership: true,
      requires_materialized_text: true,
    },
    evidence_draft: {
      source_kind: 'asset_profile',
      source_locator_present: true,
      content_excerpt_present: Boolean(preview.retrieval_text_preview),
      summary_present: Boolean(preview.summary_preview),
      payload_filter_key: `asset_profile:${safePlanToken(preview.profile_kind)}`,
      embedding_model: 'asset_profile_text_v1',
      recall_score: 1,
      manifest: {
        schema_version: 'asset_profile_retrieval_evidence_dry_run_v1',
        generator: 'asset-profile-materializer',
        asset_profile: {
          asset_id_present: Boolean(preview.asset_id),
          asset_kind: preview.asset_kind,
          source_kind: preview.source_kind,
          profile_schema: preview.profile_kind,
        },
        lexical: {
          status: preview.retrieval_text_preview ? 'materialized' : 'missing_text',
          language: 'simple',
          search_text_present: Boolean(preview.retrieval_text_preview),
          search_terms_count: (preview.terms || []).length,
        },
      },
    },
    write_plan: {
      action: writePlan.action,
      source_kind: writePlan.source_kind,
      write_policy: writePlan.write_policy,
      ready: writePlan.ready,
      dedupe_scope: writePlan.dedupe_scope,
      idempotency_key_present: Boolean(writePlan.idempotency_key),
    },
    write_order: [
      {
        step: 1,
        action: 'upsert_asset_profile',
        required_before: 'materialize_retrieval_evidence_text',
      },
      {
        step: 2,
        action: 'materialize_retrieval_evidence_text',
        requires: 'asset_profile_upserted',
      },
      {
        step: 3,
        action: 'upsert_retrieval_evidence',
        requires: 'materialized_retrieval_evidence_text',
        idempotent: true,
      },
      {
        step: 4,
        action: 'mark_parse_run_completed',
        requires: 'retrieval_evidence_upserted',
      },
    ],
    completion_gate: {
      mark_completed_after: ['asset_profile_upserted', 'retrieval_evidence_upserted'],
      partial_failure_next_status: 'retrying',
      do_not_mark_completed_until_evidence_upsert_succeeds: true,
    },
    dry_run_only: true,
  };
}

function buildAssetRetrievalEvidenceLiveWriterReadinessDryRun({
  profileReady,
  materializedTextReady,
  datasetMembershipVerified,
  assetEvidenceTableAvailable,
  unionSearchAdapterAvailable,
} = {}) {
  const writerReady = Boolean(
    profileReady
    && materializedTextReady
    && datasetMembershipVerified
    && assetEvidenceTableAvailable
    && unionSearchAdapterAvailable,
  );
  const writerStatus = !profileReady
    ? 'blocked_profile_not_ready'
    : !materializedTextReady
      ? 'blocked_materialized_text_missing'
      : !datasetMembershipVerified
        ? 'blocked_dataset_membership_guard'
        : !assetEvidenceTableAvailable
          ? 'blocked_asset_evidence_table_missing'
          : !unionSearchAdapterAvailable
            ? 'blocked_union_search_adapter_missing'
            : 'ready_for_controlled_writer_review';

  return {
    contract: 'fashion_design_asset_retrieval_evidence_live_writer_readiness_dry_run_v1',
    mode: 'dry_run',
    surface: 'asset_retrieval_evidence_writer_review',
    source_kind: 'asset_profile',
    profile_schema: 'fashion_design_image_v1',
    writer_status: writerStatus,
    writer_ready_for_operator_review: writerReady,
    release_blocker_count: writerReady ? 0 : 1,
    release_blockers: writerReady ? [] : [writerStatus],
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    production_write_allowed: false,
    schema_migration_required_before_live_write: !assetEvidenceTableAvailable,
    reviewed_migration_required: true,
    live_write_still_requires_operator_ack: true,
    input_readiness: {
      asset_profile_ready: Boolean(profileReady),
      materialized_retrieval_text_ready: Boolean(materializedTextReady),
      dataset_membership_verified: Boolean(datasetMembershipVerified),
      asset_evidence_table_available: Boolean(assetEvidenceTableAvailable),
      union_search_adapter_available: Boolean(unionSearchAdapterAvailable),
    },
    write_order: [
      {
        step: 1,
        action: 'verify_asset_profile_ready',
        required_before: 'materialize_retrieval_text',
      },
      {
        step: 2,
        action: 'materialize_retrieval_text',
        requires: 'asset_profile_ready',
      },
      {
        step: 3,
        action: 'verify_dataset_asset_membership',
        requires: 'tenant_and_dataset_scope',
      },
      {
        step: 4,
        action: 'upsert_asset_retrieval_evidence',
        requires: 'membership_verified_and_materialized_text',
        idempotent: true,
      },
      {
        step: 5,
        action: 'verify_union_search_source_kind',
        requires: 'asset_retrieval_evidence_upserted',
      },
    ],
    field_ledger: {
      allowed_writer_fields: [
        'tenant_id',
        'dataset_id',
        'asset_id',
        'profile_schema',
        'parser_name',
        'parser_version',
        'source_kind',
        'materialized_text',
        'summary',
        'terms',
        'facets',
        'thumbnail_ref',
        'created_at',
        'updated_at',
      ],
      allowed_shared_receipt_fields: [
        'source_kind',
        'profile_schema',
        'writer_status',
        'record_counts',
        'membership_guard',
        'idempotency',
        'rollback_audit',
      ],
      record_counts_only: true,
      raw_locator_fields_allowed: false,
      provider_payload_fields_allowed: false,
    },
    idempotency: {
      strategy: 'upsert_by_dataset_asset_profile_parser_version',
      key_template_redacted: true,
      dedupe_scope: [
        'tenant_id',
        'dataset_id',
        'asset_id',
        'profile_schema',
        'parser_name',
        'parser_version',
      ],
      retry_safe: true,
    },
    membership_guard: {
      table: 'dataset_asset_memberships',
      tenant_scope_required: true,
      dataset_scope_required: true,
      deny_hidden_assets: true,
      verify_before_write: true,
    },
    search_contract: {
      union_search_adapter_required: true,
      source_kind: 'asset_profile',
      joins_document_evidence: true,
      text_search_ready: Boolean(materializedTextReady && unionSearchAdapterAvailable),
      image_embedding_required_for_mvp: false,
    },
    rollback_requirements: {
      reviewed_manifest_required: true,
      automatic_rollback_allowed: false,
      rollback_by_dedupe_scope: true,
      rollback_counts_only_receipt: true,
    },
    audit_requirements: {
      record_counts_only: true,
      record_provider_payload: false,
      record_raw_locator: false,
      record_operator_ack: true,
    },
    operator_controls: {
      operator_ack_required: true,
      migration_review_required: true,
      can_run_as_live_without_private_values: false,
    },
    redaction: {
      raw_object_locator_excluded: true,
      source_url_value_included: false,
      provider_payload_excluded: true,
      secret_material_included: false,
    },
    validation: {
      field_ledger_ready: true,
      idempotency_ready: true,
      membership_guard_ready: Boolean(datasetMembershipVerified),
      rollback_audit_ready: true,
      shared_receipt_safe: true,
    },
  };
}

function buildThirdPartyPrivateAssetImportEndpointGuardDryRun({
  publicDocsAssetImportsOpen,
  inboundSecretConfigured,
  connectionScopeReady,
  sourceScopeReady,
  datasetScopeSupplied,
  assetLibraryScopeSupplied,
  assetsSupplied,
} = {}) {
  const publicDocsClosed = !publicDocsAssetImportsOpen;
  const privateEndpointReady = Boolean(
    publicDocsClosed
    && inboundSecretConfigured
    && connectionScopeReady
    && sourceScopeReady
    && datasetScopeSupplied
    && assetLibraryScopeSupplied
    && assetsSupplied,
  );
  const endpointStatus = publicDocsAssetImportsOpen
    ? 'blocked_public_docs_expose_unreviewed_asset_imports'
    : !inboundSecretConfigured
      ? 'blocked_inbound_secret_not_configured'
      : !connectionScopeReady
        ? 'blocked_connection_scope'
        : !sourceScopeReady
          ? 'blocked_source_scope'
          : !datasetScopeSupplied
            ? 'blocked_dataset_scope_missing'
            : !assetLibraryScopeSupplied
              ? 'blocked_asset_library_scope_missing'
              : !assetsSupplied
                ? 'blocked_assets_missing'
                : 'ready_for_private_endpoint_review';

  return {
    contract: 'fashion_design_third_party_asset_import_private_endpoint_guard_dry_run_v1',
    mode: 'dry_run',
    surface: 'third_party_private_asset_import_endpoint_review',
    endpoint_shape: {
      method: 'POST',
      path_template: '/v1/external/channels/{connection_id}/asset-imports',
      query_required: false,
      public_docs_exposed: Boolean(publicDocsAssetImportsOpen),
    },
    endpoint_status: endpointStatus,
    private_endpoint_ready: privateEndpointReady,
    release_blocker_count: privateEndpointReady ? 0 : 1,
    release_blockers: privateEndpointReady ? [] : [endpointStatus],
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_external_sse: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    public_docs_asset_imports_closed: publicDocsClosed,
    input_readiness: {
      inbound_secret_configured: Boolean(inboundSecretConfigured),
      connection_scope_ready: Boolean(connectionScopeReady),
      source_scope_ready: Boolean(sourceScopeReady),
      dataset_scope_supplied: Boolean(datasetScopeSupplied),
      asset_library_scope_supplied: Boolean(assetLibraryScopeSupplied),
      assets_supplied: Boolean(assetsSupplied),
    },
    request_contract: {
      required_fields: [
        'asset_library_external_id',
        'dataset_external_ids',
        'assets',
      ],
      optional_fields: [
        'asset_collection_external_id',
        'asset_domain',
        'profile_schema',
        'asset_external_id',
        'filename',
        'content_type',
      ],
      asset_input_modes: [
        'url',
        'object_ref',
        'attachment_ref',
      ],
      default_profile_schema: 'fashion_design_image_v1',
      structured_request_only: true,
      raw_binary_in_json_allowed: false,
    },
    response_contract: {
      reply_shape: 'structured_json',
      required_fields: [
        'accepted',
        'import_id',
        'asset_count',
        'parse_status',
        'assets',
      ],
      asset_fields: [
        'asset_external_id',
        'status',
        'profile_schema',
        'parse_run_id',
        'structured_profile',
      ],
      statuses: [
        'accepted',
        'queued',
        'processing',
        'completed',
        'partial_completed',
        'failed',
      ],
      customer_storage_ready: true,
    },
    privacy_policy: {
      third_party_assets_private_by_default: true,
      main_site_default_visibility: false,
      tenant_scope_required: true,
      connection_scope_required: true,
      duplicate_public_assets_not_created: true,
      cross_customer_dedupe_allowed_without_visibility_merge: true,
    },
    dataset_attachment_policy: {
      dataset_external_ids_supported: true,
      asset_library_external_id_supported: true,
      asset_collection_external_id_supported: true,
      membership_write_requires_private_endpoint_execute: true,
      chat_scope_still_prefers_dataset_external_ids: true,
    },
    field_ledger: {
      allowed_shared_receipt_fields: [
        'contract',
        'mode',
        'surface',
        'endpoint_status',
        'private_endpoint_ready',
        'release_blocker_count',
        'public_docs_asset_imports_closed',
        'request_contract',
        'response_contract',
        'privacy_policy',
        'dataset_attachment_policy',
        'redaction',
      ],
      record_counts_only: true,
      raw_asset_values_allowed: false,
      secret_material_allowed: false,
      source_url_values_allowed: false,
    },
    operator_controls: {
      private_channel_only: true,
      public_docs_change_required_before_general_release: true,
      controlled_execute_after_review_only: true,
      task_creation_allowed: false,
      callback_allowed: false,
    },
    redaction: {
      inbound_secret_value_included: false,
      connection_id_value_included: false,
      source_id_value_included: false,
      dataset_external_id_values_included: false,
      asset_library_external_id_value_included: false,
      asset_external_id_values_included: false,
      source_url_values_included: false,
      raw_asset_payload_included: false,
    },
    validation: {
      public_contract_stable: true,
      private_visibility_guard_ready: privateEndpointReady,
      structured_json_ready: true,
      dataset_attachment_guard_ready: Boolean(datasetScopeSupplied && assetLibraryScopeSupplied),
      safe_for_shared_validation_receipt: privateEndpointReady,
    },
  };
}

function buildMainSiteGalleryTaskCardDryRun({
  importRequestReady,
  normalizedResponseReady,
  scopeSummaryReady,
  parseStatusReady,
  fieldLedgerReady,
  stableTaskIdentityReady,
  taskCard,
} = {}) {
  const taskCardReady = Boolean(
    importRequestReady
    && normalizedResponseReady
    && scopeSummaryReady
    && parseStatusReady
    && fieldLedgerReady
    && stableTaskIdentityReady,
  );
  const taskStatus = !importRequestReady
    ? 'blocked_import_request_missing'
    : !normalizedResponseReady
      ? 'blocked_normalized_response_missing'
      : !scopeSummaryReady
        ? 'blocked_scope_summary_missing'
        : !parseStatusReady
          ? 'blocked_parse_status_missing'
          : !fieldLedgerReady
            ? 'blocked_field_ledger_missing'
            : !stableTaskIdentityReady
              ? 'blocked_unstable_task_identity'
              : 'ready_for_task_card_dry_run';

  return {
    contract: 'fashion_design_main_site_gallery_task_card_dry_run_v1',
    mode: 'dry_run',
    surface: 'main_site_asset_gallery_task_card_detail_review',
    task_status: taskStatus,
    task_card_ready: taskCardReady,
    release_blocker_count: taskCardReady ? 0 : 1,
    release_blockers: taskCardReady ? [] : [taskStatus],
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_external_sse: true,
    no_chat_message: true,
    no_task_creation: true,
    no_artifact_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    planned_task_card: {
      type: 'asset_gallery_import_task',
      stable_identity_source: 'asset_library_id_dataset_id_import_batch_id',
      title_strategy: 'asset_library_name_or_import_title',
      status_sequence: [
        'queued',
        'running',
        'retrying',
        'completed',
        'failed',
      ],
      active_statuses: [
        'queued',
        'running',
        'retrying',
      ],
      terminal_statuses: [
        'completed',
        'failed',
        'cancelled',
      ],
      open_behavior: 'open_task_detail_panel',
      delete_behavior: 'dismiss_or_delete_failed_dry_run_only',
    },
    task_card_preview: {
      id_present: Boolean(taskCard?.id),
      type: taskCard?.type || '',
      status: taskCard?.status || '',
      title_present: Boolean(taskCard?.title),
      source_line_count: (taskCard?.sourceLines || []).length,
      parse_status_entry_count: (taskCard?.parseStatusEntries || []).length,
    },
    detail_contract: {
      sections: [
        'generation_prompt',
        'data_sources',
        'parse_status_summary',
        'field_ledger',
        'validation_receipt',
      ],
      shows_generation_prompt: true,
      shows_data_sources: true,
      shows_parse_status: true,
      shows_field_ledger: true,
      shows_validation_receipt: true,
      raw_locator_values_allowed: false,
      provider_payload_allowed: false,
    },
    refresh_policy: {
      card_persists_after_creation: true,
      selected_card_refresh_only: true,
      no_global_polling: true,
      no_unrelated_artifact_injection: true,
      does_not_post_to_main_chat: true,
      local_visibility_ref_required: true,
    },
    input_readiness: {
      import_request_ready: Boolean(importRequestReady),
      normalized_response_ready: Boolean(normalizedResponseReady),
      scope_summary_ready: Boolean(scopeSummaryReady),
      parse_status_ready: Boolean(parseStatusReady),
      field_ledger_ready: Boolean(fieldLedgerReady),
      stable_task_identity_ready: Boolean(stableTaskIdentityReady),
    },
    field_ledger: {
      allowed_card_fields: [
        'id',
        'type',
        'title',
        'status',
        'subtitle',
        'sourceLines',
        'editInfo',
        'detail',
      ],
      allowed_detail_fields: [
        'promptSummary',
        'dataSources',
        'parseStatusSummary',
        'fieldLedger',
        'validationReceipt',
      ],
      raw_asset_values_allowed: false,
      source_url_values_allowed: false,
      secret_material_allowed: false,
      record_counts_only: true,
    },
    validation: {
      safe_for_main_site_task_card: taskCardReady,
      task_card_contract_stable: Boolean(stableTaskIdentityReady),
      detail_contract_ready: Boolean(fieldLedgerReady && parseStatusReady),
      shared_receipt_safe: taskCardReady,
      public_third_party_contract_changed: false,
    },
    redaction: {
      raw_asset_locator_excluded: true,
      source_url_value_included: false,
      provider_payload_excluded: true,
      secret_material_included: false,
      local_path_value_included: false,
    },
  };
}

function buildRetrievalStorageMappingDryRun(preview) {
  return {
    mapping_contract: 'asset_profile_retrieval_storage_mapping_v1',
    mode: 'dry_run',
    source_kind: 'asset_profile',
    profile_schema: preview.profile_kind,
    current_storage_constraints: {
      retrieval_evidences_requires_document_id: true,
      retrieval_evidences_requires_document_chunk_id: true,
      retrieval_evidences_conflict_key: ['execution_id', 'document_chunk_id'],
      retrieval_worker_input_source: 'document_chunks',
      asset_profile_has_document_chunk: false,
    },
    options: [
      {
        strategy: 'reuse_retrieval_evidences_with_synthetic_document_chunk',
        recommended: false,
        reason_codes: [
          'pollutes_document_corpus',
          'fake_chunk_lifecycle_mismatch',
          'permission_scope_mismatch',
          'deletion_and_dedup_semantics_mismatch',
        ],
      },
      {
        strategy: 'extend_retrieval_evidences_with_nullable_asset_refs',
        recommended: false,
        reason_codes: [
          'touches_existing_document_evidence_semantics',
          'requires_existing_retrieval_worker_migration',
          'higher_regression_surface',
        ],
      },
      {
        strategy: 'add_asset_retrieval_evidences_table_then_union_search',
        recommended: true,
        reason_codes: [
          'keeps_asset_refs_native',
          'avoids_document_spoofing',
          'preserves_existing_document_retrieval_contract',
          'supports_asset_lifecycle_and_permissions',
        ],
      },
    ],
    selected_strategy: 'add_asset_retrieval_evidences_table_then_union_search',
    proposed_asset_evidence_table: {
      table_name: 'asset_retrieval_evidences',
      fields: [
        'tenant_id',
        'dataset_id',
        'asset_id',
        'profile_kind',
        'parser_name',
        'parser_version',
        'source_locator',
        'content_excerpt',
        'summary',
        'payload_filter_key',
        'embedding_model',
        'recall_score',
        'evidence_manifest',
        'search_text',
        'search_terms',
        'indexed_content_hash',
        'created_at',
      ],
      unique_key: ['tenant_id', 'dataset_id', 'asset_id', 'profile_kind', 'parser_name', 'parser_version'],
    },
    search_integration: {
      method: 'union_document_and_asset_evidence_search',
      requires_query_update: true,
      returns_source_kind: 'asset_profile',
      document_evidence_table_unchanged: true,
    },
    migration_guardrails: {
      production_write_allowed: false,
      requires_reviewed_migration: true,
      requires_search_result_source_kind: true,
      requires_asset_dataset_membership_check: true,
    },
    ready_for_migration_design: true,
    production_write_allowed: false,
  };
}

function buildAssetRetrievalEvidenceMigrationSketchDryRun() {
  return {
    migration_contract: 'asset_retrieval_evidences_migration_sketch_v1',
    mode: 'reviewed_sketch_only',
    table: {
      name: 'asset_retrieval_evidences',
      purpose: 'store searchable evidence materialized from asset profiles without spoofing document chunks',
      columns: [
        { name: 'id', kind: 'uuid', required: true },
        { name: 'tenant_id', kind: 'uuid', required: true },
        { name: 'dataset_id', kind: 'uuid', required: true },
        { name: 'asset_id', kind: 'uuid', required: true },
        { name: 'profile_kind', kind: 'text', required: true },
        { name: 'parser_name', kind: 'text', required: true },
        { name: 'parser_version', kind: 'text', required: true },
        { name: 'source_locator', kind: 'text', required: true },
        { name: 'content_excerpt', kind: 'text', required: true },
        { name: 'summary', kind: 'text', required: false },
        { name: 'payload_filter_key', kind: 'text', required: true },
        { name: 'embedding_model', kind: 'text', required: false },
        { name: 'recall_score', kind: 'double precision', required: false },
        { name: 'evidence_manifest', kind: 'jsonb', required: true },
        { name: 'search_text', kind: 'text', required: true },
        { name: 'search_terms', kind: 'text[]', required: true },
        { name: 'search_tsv', kind: 'tsvector', required: true },
        { name: 'indexed_content_hash', kind: 'text', required: true },
        { name: 'created_at', kind: 'timestamptz', required: true },
        { name: 'indexed_at', kind: 'timestamptz', required: true },
      ],
      unique_key: ['tenant_id', 'dataset_id', 'asset_id', 'profile_kind', 'parser_name', 'parser_version'],
    },
    indexes: [
      {
        name: 'asset_retrieval_evidences_scope_profile_idx',
        method: 'btree',
        columns: ['tenant_id', 'dataset_id', 'profile_kind', 'created_at desc'],
      },
      {
        name: 'asset_retrieval_evidences_asset_profile_idx',
        method: 'btree',
        columns: ['tenant_id', 'asset_id', 'profile_kind', 'created_at desc'],
      },
      {
        name: 'asset_retrieval_evidences_search_terms_gin_idx',
        method: 'gin',
        columns: ['search_terms'],
      },
      {
        name: 'asset_retrieval_evidences_search_tsv_gin_idx',
        method: 'gin',
        columns: ['search_tsv'],
      },
      {
        name: 'asset_retrieval_evidences_content_hash_idx',
        method: 'btree',
        columns: ['tenant_id', 'asset_id', 'indexed_content_hash', 'indexed_at desc'],
      },
    ],
    membership_guard: {
      required: true,
      membership_table: 'dataset_asset_memberships',
      join_keys: ['tenant_id', 'dataset_id', 'asset_id'],
      expiry_filter_required: true,
      hidden_asset_policy: 'deny',
    },
    search_result_contract: {
      source_kind: 'asset_profile',
      source_id_field: 'asset_id',
      scope_id_field: 'dataset_id',
      profile_kind_field: 'profile_kind',
      content_field: 'content_excerpt',
      manifest_field: 'evidence_manifest',
      must_not_include: [
        'raw_storage_locator',
        'provider_payload',
        'auth_material',
        'session_material',
        'filesystem_path',
      ],
    },
    review_gates: [
      'operator_reviewed_migration',
      'backfill_plan_reviewed',
      'rollback_plan_reviewed',
      'small_batch_first',
      'search_source_kind_regression_test',
    ],
    production_migration_allowed: false,
    production_write_allowed: false,
  };
}

function buildUnionSearchNoWriteAdapterDraft(preview) {
  return {
    adapter_contract: 'asset_profile_union_search_no_write_adapter_v1',
    mode: 'dry_run',
    no_write: true,
    source_kind: 'asset_profile',
    profile_schema: preview.profile_kind,
    input_contract: {
      requires_query_text: true,
      requires_dataset_scope: true,
      requires_asset_dataset_membership: true,
      document_retrieval_input_unchanged: true,
      asset_evidence_table_required: 'asset_retrieval_evidences',
    },
    search_plan: {
      method: 'union_document_and_asset_evidence_search',
      steps: [
        { step: 1, action: 'resolve_dataset_scope' },
        { step: 2, action: 'run_existing_document_retrieval' },
        {
          step: 3,
          action: 'query_asset_retrieval_evidences',
          table: 'asset_retrieval_evidences',
          membership_guard: 'dataset_asset_memberships',
        },
        { step: 4, action: 'merge_rank_and_limit_results' },
      ],
      query_terms_count: (preview.terms || []).length,
      ranking_policy: 'score_then_source_kind_then_recency',
      document_search_unchanged: true,
    },
    membership_guard: {
      required: true,
      table: 'dataset_asset_memberships',
      join_keys: ['tenant_id', 'dataset_id', 'asset_id'],
      deny_hidden_assets: true,
      expiry_filter_required: true,
    },
    result_contract: {
      source_kind: 'asset_profile',
      source_id_field: 'asset_id',
      profile_kind_field: 'profile_kind',
      title_present: Boolean(preview.title),
      summary_present: Boolean(preview.summary_preview),
      terms_count: (preview.terms || []).length,
      must_not_include_raw_locator: true,
      must_not_include_provider_payload: true,
    },
    guardrails: {
      requires_search_result_source_kind: true,
      requires_asset_dataset_membership_check: true,
      requires_current_tenant_filter: true,
      no_schema_migration_in_this_step: true,
      production_write_allowed: false,
    },
    ready_for_adapter_design: true,
    production_write_allowed: false,
  };
}

function buildUnionSearchMergeRankFixtureDryRun(preview) {
  return {
    fixture_contract: 'asset_profile_union_search_merge_rank_fixture_v1',
    mode: 'dry_run',
    no_write: true,
    input_results: {
      document_results: [
        {
          source_kind: 'document_chunk',
          source_id_present: true,
          title: 'document evidence fixture',
          score: 0.72,
          recency_rank: 2,
          membership_guard_passed: true,
          raw_locator_included: false,
          provider_payload_included: false,
        },
      ],
      asset_profile_results: [
        {
          source_kind: 'asset_profile',
          source_id_present: Boolean(preview.asset_id),
          profile_schema: preview.profile_kind,
          title_present: Boolean(preview.title),
          score: 0.88,
          recency_rank: 1,
          membership_guard_passed: true,
          raw_locator_included: false,
          provider_payload_included: false,
        },
      ],
    },
    merge_policy: {
      method: 'stable_score_desc_source_kind_recency',
      max_results: 8,
      requires_source_kind: true,
      requires_membership_guard: true,
      document_search_unchanged: true,
      asset_search_no_write: true,
    },
    merged_results: [
      {
        rank: 1,
        source_kind: 'asset_profile',
        source_id_present: Boolean(preview.asset_id),
        profile_schema: preview.profile_kind,
        score: 0.88,
        sort_reason: 'higher_score_then_recency',
        membership_guard_passed: true,
        raw_locator_included: false,
        provider_payload_included: false,
      },
      {
        rank: 2,
        source_kind: 'document_chunk',
        source_id_present: true,
        score: 0.72,
        sort_reason: 'document_evidence_retained',
        membership_guard_passed: true,
        raw_locator_included: false,
        provider_payload_included: false,
      },
    ],
    source_kind_regression: {
      mixed_sources_present: true,
      asset_profile_source_kind_preserved: true,
      document_source_kind_preserved: true,
      result_source_kind_required: true,
      membership_guard_checked: true,
      raw_locator_excluded: true,
      provider_payload_excluded: true,
    },
    ready_for_search_merge_regression: true,
    production_write_allowed: false,
  };
}

function buildUnionSearchExplainDebugSummaryDryRun(preview) {
  const fixture = buildUnionSearchMergeRankFixtureDryRun(preview);
  return {
    debug_contract: 'asset_profile_union_search_explain_debug_summary_v1',
    mode: 'dry_run',
    no_write: true,
    summary: {
      query_scope_resolved: true,
      document_result_count: 1,
      asset_profile_result_count: 1,
      merged_result_count: (fixture.merged_results || []).length,
      top_source_kind: fixture.merged_results?.[0]?.source_kind || null,
      source_kind_mix: ['asset_profile', 'document_chunk'],
      membership_guard_status: 'checked',
      rank_reason_codes: [
        'asset_profile_higher_score',
        'document_evidence_retained',
        'source_kind_preserved',
      ],
      asset_profile_is_exact_document_citation: false,
    },
    safe_debug_fields: [
      'rank',
      'source_kind',
      'score_bucket',
      'reason_codes',
      'membership_guard_status',
    ],
    redaction: {
      raw_locator_excluded: true,
      provider_payload_excluded: true,
      secret_material_excluded: true,
    },
    ready_for_union_search_debug_panel: true,
    production_write_allowed: false,
  };
}

function buildModelFacingSupplyCompressionDryRun(preview) {
  const terms = (preview.terms || []).slice(0, 8);
  const facets = (preview.facets || []).slice(0, 4);
  const compressedText = [
    `asset signal: ${preview.title || ''}`,
    `profile: ${preview.profile_kind || ''}`,
    `summary: ${preview.summary_preview || ''}`,
    `terms: ${terms.join('、')}`,
    `facets: ${facets.join('；')}`,
  ].join('; ').slice(0, 480);
  return {
    compression_contract: 'asset_profile_model_facing_supply_compression_v1',
    mode: 'dry_run',
    no_write: true,
    source_kind: 'asset_profile',
    profile_schema: preview.profile_kind,
    model_supply: {
      compressed_text: compressedText,
      compressed_text_chars: compressedText.length,
      term_count: terms.length,
      facet_count: facets.length,
      citation_policy: 'asset_profile_is_understanding_signal_not_exact_document_quote',
      exact_claim_policy: 'require_document_database_or_media_evidence_for_exact_claims',
    },
    compression_checks: {
      raw_locator_excluded: true,
      provider_payload_excluded: true,
      secret_material_excluded: true,
      asset_profile_exact_citation_disallowed: true,
      document_evidence_required_for_exact_claims: true,
    },
    ready_for_model_supply_compression: Boolean(compressedText),
    production_write_allowed: false,
  };
}

function buildDocumentEvidenceFixtureForAssistantRun() {
  return {
    type: 'retrieval_evidence',
    source: 'document_chunk_fallback',
    summary: 'document evidence fixture',
    content_excerpt: 'The source document provides exact wording and claim support.',
  };
}

function buildAssetProfileHintFixtureForAssistantRun(preview) {
  return {
    type: 'asset_profile_hint',
    source: 'asset_profile',
    asset_id: preview.asset_id,
    title: preview.title,
    asset_kind: preview.asset_kind,
    source_kind: 'asset_profile',
    profile_kind: preview.profile_kind,
    summary: preview.summary_preview,
    noun_terms: preview.terms || [],
    facets: preview.facets || [],
  };
}

function buildAssetParseStatusFixtureForAssistantRun() {
  return {
    type: 'asset_parse_status',
    not_ready_asset_count: 2,
    failed_asset_count: 1,
    retrying_asset_count: 1,
    status_counts: {
      pending: 1,
      retrying: 1,
    },
  };
}

function buildAssistantRunAssetDocumentSupplyGateDryRun(items) {
  const bucketLimits = {
    document_status: 2,
    structured_fact: 4,
    spreadsheet: 2,
    retrieval: 6,
    database: 4,
    memory: 3,
    asset_profile: 4,
    other: 2,
  };
  const compactedItems = items.map((item) => compactAssistantRunSupplyItem(item));
  const sourceKindCounts = {};
  const citationLabels = {};
  const sourceKindOrder = [];
  let documentEvidenceCount = 0;
  let assetProfileSignalCount = 0;
  for (const item of compactedItems) {
    const sourceKind = assistantRunSupplySourceKindForGate(item);
    if (!sourceKindOrder.includes(sourceKind)) sourceKindOrder.push(sourceKind);
    sourceKindCounts[sourceKind] = (sourceKindCounts[sourceKind] || 0) + 1;
    const label = assistantRunSupplyCitationLabelForGate(item);
    citationLabels[label] = (citationLabels[label] || 0) + 1;
    if (label === 'document_evidence') documentEvidenceCount += 1;
    if (label === 'asset_profile_signal') assetProfileSignalCount += 1;
  }
  const serialized = JSON.stringify(compactedItems);
  const approxPromptChars = serialized.length;
  const hasDocumentEvidence = documentEvidenceCount > 0;
  const hasAssetProfileSignal = assetProfileSignalCount > 0;
  const needsMoreSupply = !hasDocumentEvidence;
  const status = hasDocumentEvidence && hasAssetProfileSignal
    ? 'grounded_with_asset_context'
    : hasDocumentEvidence
      ? 'grounded_without_asset_context'
      : hasAssetProfileSignal
        ? 'partial_asset_context_only'
        : 'missing_evidence';
  return {
    contract: 'assistant_run_asset_document_supply_budget_quality_gate_dry_run_v1',
    mode: 'dry_run',
    no_write: true,
    budget_policy: {
      source: 'assistant_run_model_budgeted_supply_items',
      global_item_limit: 24,
      bucket_limits: bucketLimits,
      included_by_type: countBy(compactedItems, (item) => item.type || 'unknown'),
      omitted_by_type: {},
      model_supplied_item_count: compactedItems.length,
      omitted_supplied_item_count: 0,
      approx_prompt_chars: approxPromptChars,
      approx_prompt_tokens: Math.ceil(approxPromptChars / 4),
      token_estimate_policy: 'chars_div_4_ceil_dry_run_only',
    },
    source_kind_dedupe: {
      policy: 'preserve_first_seen_source_kind_then_count_duplicates',
      source_kind_order: sourceKindOrder,
      source_kind_counts: sourceKindCounts,
      document_chunk_and_asset_profile_keep_separate: true,
    },
    citation_policy: {
      labels: citationLabels,
      document_evidence_label: 'document_evidence',
      asset_profile_label: 'asset_profile_signal',
      asset_profile_exact_citation_allowed: false,
      exact_claims_require_document_database_or_media_evidence: true,
    },
    quality_gate: {
      status,
      document_evidence_count: documentEvidenceCount,
      asset_profile_signal_count: assetProfileSignalCount,
      needs_more_supply: needsMoreSupply,
      next_action: needsMoreSupply ? 'continue_expand_supply' : 'answer_with_citation_constraints',
      continue_when: [
        'missing_document_evidence_for_exact_claim',
        'only_asset_profile_signal_available',
        'budget_omitted_relevant_evidence_available',
      ],
    },
    model_guidance: [
      'Use asset_profile_signal only as compact asset understanding, not as an exact citation.',
      'Use document_evidence, database aggregates, or media evidence for exact claims, counts, and quotations.',
      'If quality_gate.next_action is continue_expand_supply, answer with current evidence if possible and keep expanding supply before final high-confidence claims.',
    ],
    production_write_allowed: false,
  };
}

function buildAssistantRunSupplyProgressEventsDryRun(items) {
  const supplyGate = buildAssistantRunAssetDocumentSupplyGateDryRun(items);
  const events = [];
  const suppliedItemCount = supplyGate.budget_policy?.model_supplied_item_count || 0;
  const qualityStatus = supplyGate.quality_gate?.status || 'missing_evidence';
  const nextAction = supplyGate.quality_gate?.next_action || 'continue_expand_supply';
  const assetProfileSignalCount = supplyGate.quality_gate?.asset_profile_signal_count || 0;

  if (suppliedItemCount > 0) {
    events.push(buildAssistantRunSupplyProgressEvent(
      'supply_ready',
      'supply_ready',
      'ready',
      'supply available; answer first and continue follow-up actions',
      true,
    ));
  }
  if (assetProfileSignalCount > 0) {
    events.push(buildAssistantRunSupplyProgressEvent(
      'asset_profile_signal',
      'asset_profile_signal',
      'ready',
      'asset profile is an understanding signal, not an exact citation',
      true,
    ));
  }
  if (nextAction === 'continue_expand_supply') {
    events.push(buildAssistantRunSupplyProgressEvent(
      'supply_expanding',
      'supply_expanding',
      qualityStatus,
      'insufficient supply continues evidence expansion without blocking the current reply',
      true,
    ));
  }
  if (assistantRunSupplyItemsNeedParseWaitOrRetry(items)) {
    events.push(buildAssistantRunSupplyProgressEvent(
      'parse_waiting_or_retry',
      'parse_waiting_or_retry',
      'processing',
      'parse pending, failed, degraded, or retrying is visible progress and not terminal',
      true,
    ));
  }

  return {
    contract: 'assistant_run_supply_progress_events_dry_run_v1',
    mode: 'dry_run',
    no_write: true,
    non_blocking: true,
    consumer_targets: ['main_site_task_card', 'third_party_stream'],
    source_gate_contract: supplyGate.contract,
    quality_gate_status: qualityStatus,
    quality_gate_next_action: nextAction,
    events,
    continuation_policy: {
      answer_with_current_evidence_first: true,
      continue_actions_after_reply: true,
      final_failure_without_answer: false,
      parse_or_supply_gap_is_not_terminal: true,
    },
    redaction: {
      raw_locator_excluded: true,
      provider_payload_excluded: true,
      auth_material_excluded: true,
      local_path_excluded: true,
      raw_source_body_excluded: true,
    },
    production_write_allowed: false,
  };
}

function buildAssistantRunSupplyProgressEvent(eventType, phase, status, message, modelVisible) {
  return {
    event_type: eventType,
    phase,
    status,
    message,
    blocking: false,
    model_visible: modelVisible,
    third_party_visible: true,
    task_card_visible: true,
    sensitive_payload_included: false,
  };
}

function buildAssistantRunSupplyProgressContractDriftGuardDryRun(progressEvents) {
  return {
    contract: 'assistant_run_supply_progress_contract_drift_guard_dry_run_v1',
    mode: 'dry_run',
    no_write: true,
    source_contract: progressEvents.contract || null,
    source_event_types: (progressEvents.events || []).map((event) => event.event_type).filter(Boolean),
    external_sse_guard: {
      schema: 'v3.external_channel.sse.v1',
      public_envelope_fields: [
        'schema',
        'event_id',
        'sequence',
        'assistant_run_id',
        'idempotency_key',
        'conversation_external_id',
        'phase',
        'status',
        'display_text',
        'status_url',
        'poll_after_seconds',
        'data',
      ],
      public_stream_field_mutation_allowed: false,
      new_progress_events_emit_live_sse: false,
      new_progress_events_are_internal_dry_run_only: true,
    },
    third_party_guard: {
      callback_triggered: false,
      public_request_or_response_field_added: false,
      requires_new_third_party_contract: false,
      third_party_visible_progress_is_summary_only: true,
    },
    main_site_task_card_guard: {
      expected_consumer: 'artifact_task_cards',
      allowed_surface: 'detail_progress_summary',
      creates_task_card_without_user_action: false,
      mutates_task_card_status_enum: false,
      task_card_detail_only: true,
    },
    answer_liveness_guard: {
      failure_status_blocks_answer: false,
      parse_or_supply_gap_is_terminal: false,
      answer_with_current_evidence_first:
        progressEvents.continuation_policy?.answer_with_current_evidence_first ?? null,
      continue_actions_after_reply:
        progressEvents.continuation_policy?.continue_actions_after_reply ?? null,
      final_failure_without_answer:
        progressEvents.continuation_policy?.final_failure_without_answer ?? null,
    },
    redaction_guard: {
      all_events_non_blocking: (progressEvents.events || []).every((event) => (
        event.blocking === false && event.sensitive_payload_included === false
      )),
      raw_locator_excluded: progressEvents.redaction?.raw_locator_excluded ?? true,
      provider_payload_excluded: progressEvents.redaction?.provider_payload_excluded ?? true,
      auth_material_excluded: progressEvents.redaction?.auth_material_excluded ?? true,
    },
    production_write_allowed: false,
  };
}

function assistantRunSupplyItemsNeedParseWaitOrRetry(items) {
  return items.some((item) => (
    (item.type === 'asset_parse_status' || item.type === 'document_parse_status')
    && (
      numericSum(item, [
        'not_ready_asset_count',
        'notReadyAssetCount',
        'failed_asset_count',
        'failedAssetCount',
        'retrying_asset_count',
        'retryingAssetCount',
        'pending_document_count',
        'pendingDocumentCount',
        'failed_document_count',
        'failedDocumentCount',
        'retrying_document_count',
        'retryingDocumentCount',
        'degraded_document_count',
        'degradedDocumentCount',
      ]) > 0
      || Object.entries(item.status_counts || item.statusCounts || {}).some(([status, count]) => (
        ['pending', 'parsing', 'retrying', 'failed', 'degraded'].includes(status)
        && Number(count || 0) > 0
      ))
    )
  ));
}

function hasAssistantRunProgressEvent(progress, eventType) {
  return (progress.events || []).some((event) => event.event_type === eventType);
}

function compactAssistantRunSupplyItem(item) {
  if (item.type === 'asset_profile_hint') {
    return {
      type: 'asset_profile_hint',
      source: 'asset_profile',
      asset_id: item.asset_id,
      title: item.title,
      asset_kind: item.asset_kind,
      source_kind: item.source_kind,
      profile_kind: item.profile_kind,
      summary: item.summary,
      noun_terms: item.noun_terms,
      facets: item.facets,
    };
  }
  if (item.type === 'retrieval_evidence') {
    return {
      type: 'retrieval_evidence',
      source: item.source,
      summary: item.summary,
      content_excerpt: item.content_excerpt,
    };
  }
  return { type: item.type || 'unknown', summary: item.summary || '' };
}

function assistantRunSupplySourceKindForGate(item) {
  if (item.type === 'asset_profile_hint') return 'asset_profile';
  const source = item.source_kind || item.source || item.type || 'unknown';
  return source === 'document_chunk_fallback' ? 'document_chunk' : source;
}

function assistantRunSupplyCitationLabelForGate(item) {
  if (item.type === 'asset_profile_hint') return 'asset_profile_signal';
  if (item.type === 'retrieval_evidence' || item.type === 'search_evidence') return 'document_evidence';
  if (item.type === 'database_aggregate' || item.type === 'dataset_fact_snapshot' || item.type === 'dataset_entity_scan') {
    return 'deterministic_structured_evidence';
  }
  if (item.type === 'spreadsheet_row_analysis') return 'spreadsheet_evidence';
  if (item.type === 'document_parse_status' || item.type === 'asset_parse_status') return 'parse_status';
  if (item.type === 'conversation_memory_item') return 'conversation_memory';
  return 'other_context';
}

function countBy(items, keyFn) {
  const counts = {};
  for (const item of items) {
    const key = keyFn(item);
    counts[key] = (counts[key] || 0) + 1;
  }
  return counts;
}

function numericSum(item, keys) {
  return keys.reduce((sum, key) => sum + Number(item[key] || 0), 0);
}

function safePlanToken(value) {
  const token = String(value || '')
    .trim()
    .replace(/\s+/g, ' ')
    .replace(/[^A-Za-z0-9_:-]/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 160);
  return token || 'unknown';
}

function materializeAssetProfileRetrievalTextPreview(hint) {
  return [
    ['asset_title', hint.title],
    ['asset_kind', hint.assetKind],
    ['source_kind', hint.sourceKind],
    ['profile_kind', hint.profileKind],
    ['summary', hint.summary],
    ['terms', (hint.nounTerms || []).join('、')],
    ['facets', (hint.facets || []).join('；')],
  ]
    .map(([label, value]) => [label, String(value || '').trim().replace(/\s+/g, ' ')])
    .filter(([, value]) => value)
    .map(([label, value]) => `${label}: ${value}`)
    .join('\n')
    .slice(0, 1600);
}

function buildRedactedLiveCommand(args) {
  const command = [
    'npm run smoke:fashion-design-asset-import -- --execute --ack-live-write',
    '--approval-id <reviewed-smoke-id>',
    '--base-url <main-site-base>',
    args.datasetId ? '--dataset-id <existing-dataset-uuid>' : '--dataset-id <required-existing-dataset-uuid>',
    args.assetLibraryId ? '--asset-library-id <existing-asset-library-uuid>' : '--asset-library-id <required-existing-asset-library-uuid>',
    '<plus-v3-session-auth-material>',
  ].join(' ');
  if (/https?:\/\/|Bearer\s|cookie=|Authorization|[A-Za-z]:\\|\/Users\//i.test(command)) {
    throw new Error('redacted command template contains unsafe material');
  }
  return command;
}

function buildOperatorManifestDryRun(args) {
  const authSupplied = Boolean(args.cookie || args.bearer);
  const datasetIdSupplied = Boolean(args.datasetId);
  const assetLibraryIdSupplied = Boolean(args.assetLibraryId);
  const ackLiveWriteSupplied = Boolean(args.ackLiveWrite);
  const approvalIdSupplied = Boolean(args.approvalId);
  const localThreadIdSupplied = Boolean(args.localThreadId);
  const executeReady = authSupplied
    && datasetIdSupplied
    && assetLibraryIdSupplied
    && ackLiveWriteSupplied
    && approvalIdSupplied;

  return {
    contract: 'fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1',
    mode: 'dry_run',
    no_write: true,
    action: 'fashion_design_image_asset_import_live_execute',
    execute_ready: executeReady,
    required_inputs: {
      v3_user_session_required: true,
      v3_user_session_supplied: authSupplied,
      existing_dataset_required: true,
      existing_dataset_supplied: datasetIdSupplied,
      existing_asset_library_required: true,
      existing_asset_library_supplied: assetLibraryIdSupplied,
      ack_live_write_required: true,
      ack_live_write_supplied: ackLiveWriteSupplied,
      approval_id_required: true,
      approval_id_supplied: approvalIdSupplied,
      local_thread_id_optional: true,
      local_thread_id_supplied: localThreadIdSupplied,
    },
    planned_steps: [
      {
        step: 'upload_png_and_zip_fixture',
        requires: ['v3_user_session', 'ack_live_write', 'approval_id'],
        write_scope: 'temporary_upload_objects',
      },
      {
        step: 'attach_existing_dataset_to_existing_asset_library',
        requires: ['existing_dataset', 'existing_asset_library', 'dataset_manage_permission'],
        write_scope: 'dataset_asset_library_membership',
      },
      {
        step: 'create_fashion_design_asset_import_batch',
        requires: ['uploaded_fixture_objects', 'existing_dataset', 'existing_asset_library'],
        write_scope: 'asset_items_dataset_memberships_parse_runs_profiles',
      },
      {
        step: 'read_asset_library_scope_summary',
        requires: ['existing_asset_library'],
        write_scope: 'read_only_verification',
      },
    ],
    review_gates: {
      operator_review_required: true,
      approval_id_hash_only_in_receipts: true,
      manifest_must_be_reviewed_before_execute: true,
      dataset_and_asset_library_must_already_exist: true,
      dataset_must_be_visible_and_manageable_by_session_user: true,
      asset_library_must_belong_to_same_tenant: true,
      execute_requires_ack_live_write: true,
    },
    rollback_plan: {
      operator_review_required: true,
      rollback_strategy: 'delete_or_archive_smoke_assets_and_memberships_by_approval_hash_after_review',
      cleanup_manifest_required: true,
      automatic_cleanup_allowed: false,
      manual_db_changes_allowed_without_review: false,
    },
    audit_requirements: {
      record_approval_id_hash: true,
      record_counts_only: true,
      record_dataset_id_value: false,
      record_asset_library_id_value: false,
      record_auth_material: false,
      record_object_locators: false,
      record_source_urls: false,
      record_local_paths: false,
    },
    redaction: {
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_url_excluded: true,
      local_path_excluded: true,
    },
    production_write_allowed: false,
  };
}

function summarizeOperatorManifest(manifest) {
  const requiredInputs = manifest.required_inputs || {};
  const requiredPairs = [
    ['v3_user_session', requiredInputs.v3_user_session_required, requiredInputs.v3_user_session_supplied],
    ['existing_dataset', requiredInputs.existing_dataset_required, requiredInputs.existing_dataset_supplied],
    ['existing_asset_library', requiredInputs.existing_asset_library_required, requiredInputs.existing_asset_library_supplied],
    ['ack_live_write', requiredInputs.ack_live_write_required, requiredInputs.ack_live_write_supplied],
    ['approval_id', requiredInputs.approval_id_required, requiredInputs.approval_id_supplied],
  ];
  const missingRequiredInputs = requiredPairs
    .filter(([, required, supplied]) => required && !supplied)
    .map(([name]) => name);
  const suppliedRequiredInputCount = requiredPairs
    .filter(([, required, supplied]) => required && supplied)
    .length;
  return {
    contract: manifest.contract,
    mode: manifest.mode,
    noWrite: Boolean(manifest.no_write),
    executeReady: Boolean(manifest.execute_ready),
    productionWriteAllowed: Boolean(manifest.production_write_allowed),
    plannedStepCount: Array.isArray(manifest.planned_steps) ? manifest.planned_steps.length : 0,
    suppliedRequiredInputCount,
    missingRequiredInputs,
    operatorReviewRequired: Boolean(manifest.review_gates?.operator_review_required),
    manifestReviewRequired: Boolean(manifest.review_gates?.manifest_must_be_reviewed_before_execute),
    approvalIdHashOnlyInReceipts: Boolean(manifest.review_gates?.approval_id_hash_only_in_receipts),
    cleanupManifestRequired: Boolean(manifest.rollback_plan?.cleanup_manifest_required),
    automaticCleanupAllowed: Boolean(manifest.rollback_plan?.automatic_cleanup_allowed),
    recordCountsOnly: Boolean(manifest.audit_requirements?.record_counts_only),
    rawSessionExcluded: Boolean(manifest.redaction?.raw_session_excluded),
    rawDatasetIdExcluded: Boolean(manifest.redaction?.raw_dataset_id_excluded),
    rawAssetLibraryIdExcluded: Boolean(manifest.redaction?.raw_asset_library_id_excluded),
    rawApprovalIdExcluded: Boolean(manifest.redaction?.raw_approval_id_excluded),
    rawObjectLocatorExcluded: Boolean(manifest.redaction?.raw_object_locator_excluded),
    rawUrlExcluded: Boolean(manifest.redaction?.raw_url_excluded),
    localPathExcluded: Boolean(manifest.redaction?.local_path_excluded),
  };
}

function operatorManifestExpectedExecuteReady(args) {
  return Boolean(args.cookie || args.bearer)
    && Boolean(args.datasetId)
    && Boolean(args.assetLibraryId)
    && Boolean(args.ackLiveWrite)
    && Boolean(args.approvalId);
}

function operatorManifestRedacted(manifest, args) {
  const serialized = JSON.stringify(manifest);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorManifestEvidence(args) {
  const manifest = buildOperatorManifestDryRun(args);
  const summary = summarizeOperatorManifest(manifest);
  return {
    manifest,
    summary,
    checks: {
      operatorManifestDryRunReady: manifest.contract === 'fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1'
        && manifest.mode === 'dry_run'
        && manifest.no_write === true
        && manifest.production_write_allowed === false,
      operatorManifestRequiredInputsTracked:
        manifest.required_inputs?.v3_user_session_required === true
        && manifest.required_inputs?.existing_dataset_required === true
        && manifest.required_inputs?.existing_asset_library_required === true
        && manifest.required_inputs?.ack_live_write_required === true
        && manifest.required_inputs?.approval_id_required === true,
      operatorManifestExecuteReadyMatchesInputs:
        manifest.execute_ready === operatorManifestExpectedExecuteReady(args),
      operatorManifestRequiresManualReview:
        manifest.review_gates?.operator_review_required === true
        && manifest.review_gates?.manifest_must_be_reviewed_before_execute === true
        && manifest.review_gates?.execute_requires_ack_live_write === true,
      operatorManifestRunbookReady:
        Array.isArray(manifest.planned_steps)
        && manifest.planned_steps.length === 4
        && manifest.rollback_plan?.cleanup_manifest_required === true
        && manifest.rollback_plan?.automatic_cleanup_allowed === false,
      operatorManifestAuditCountsOnly:
        manifest.audit_requirements?.record_counts_only === true
        && manifest.audit_requirements?.record_auth_material === false
        && manifest.audit_requirements?.record_object_locators === false
        && manifest.audit_requirements?.record_source_urls === false,
      operatorManifestRedacted: operatorManifestRedacted(manifest, args),
    },
  };
}

function buildPostExecuteCleanupManifestDryRun({
  approvalHashSupplied,
  liveReceiptSupplied,
  assetCount,
  datasetMembershipCount,
  parseRunCount,
  profileCount,
}) {
  const countsSupplied = Number(assetCount || 0) > 0
    && Number(datasetMembershipCount || 0) > 0
    && Number(parseRunCount || 0) > 0
    && Number(profileCount || 0) > 0;
  return {
    contract: 'fashion_design_asset_import_post_execute_cleanup_manifest_dry_run_v1',
    mode: 'dry_run',
    no_write: true,
    no_delete: true,
    action: 'fashion_design_image_asset_import_cleanup_manifest',
    cleanup_manifest_ready: Boolean(approvalHashSupplied && liveReceiptSupplied && countsSupplied),
    required_inputs: {
      approval_hash_required: true,
      approval_hash_supplied: Boolean(approvalHashSupplied),
      live_execute_receipt_required: true,
      live_execute_receipt_supplied: Boolean(liveReceiptSupplied),
      counts_required: true,
      counts_supplied: countsSupplied,
    },
    receipt_shape: {
      record_counts_only: true,
      approval_hash_value_excluded_from_shared_receipt: true,
      raw_asset_ids_excluded_from_shared_receipt: true,
      raw_dataset_ids_excluded_from_shared_receipt: true,
      raw_asset_library_ids_excluded_from_shared_receipt: true,
      raw_object_locators_excluded_from_shared_receipt: true,
      raw_source_urls_excluded_from_shared_receipt: true,
    },
    selection_policy: {
      scope: 'same_tenant_reviewed_smoke_records_only',
      match_by_approval_hash: true,
      match_by_parser: 'datamax-fashion-image-parser',
      match_by_parser_version: '2026-06-17',
      match_by_profile_kind: 'fashion_design_image_v1',
      private_operator_manifest_may_include_record_ids_after_review: true,
      shared_receipt_must_not_include_record_ids: true,
    },
    counts: {
      asset_count: Number(assetCount || 0),
      dataset_membership_count: Number(datasetMembershipCount || 0),
      parse_run_count: Number(parseRunCount || 0),
      profile_count: Number(profileCount || 0),
    },
    planned_cleanup_actions: [
      {
        step: 'review_matched_smoke_assets',
        write_scope: 'read_only_verification',
      },
      {
        step: 'remove_or_archive_dataset_asset_memberships',
        write_scope: 'dataset_asset_memberships',
      },
      {
        step: 'remove_or_archive_asset_profiles',
        write_scope: 'asset_profiles',
      },
      {
        step: 'mark_or_remove_parse_runs',
        write_scope: 'asset_parse_runs',
      },
      {
        step: 'remove_or_archive_asset_items',
        write_scope: 'asset_items',
      },
      {
        step: 'review_temporary_upload_objects',
        write_scope: 'temporary_upload_objects_after_storage_review',
      },
    ],
    execution_controls: {
      operator_review_required: true,
      cleanup_manifest_must_be_reviewed_before_action: true,
      automatic_cleanup_allowed: false,
      manual_db_changes_allowed_without_review: false,
      temporary_object_cleanup_requires_storage_review: true,
    },
    audit_requirements: {
      record_approval_hash: true,
      record_counts_only: true,
      record_auth_material: false,
      record_object_locators: false,
      record_source_urls: false,
      record_local_paths: false,
    },
    redaction: {
      raw_approval_hash_value_excluded: true,
      raw_asset_ids_excluded: true,
      raw_dataset_ids_excluded: true,
      raw_asset_library_ids_excluded: true,
      raw_object_locator_excluded: true,
      raw_url_excluded: true,
      local_path_excluded: true,
    },
    production_write_allowed: false,
  };
}

function summarizePostExecuteCleanupManifest(manifest) {
  return {
    contract: manifest.contract,
    mode: manifest.mode,
    noWrite: Boolean(manifest.no_write),
    noDelete: Boolean(manifest.no_delete),
    cleanupManifestReady: Boolean(manifest.cleanup_manifest_ready),
    productionWriteAllowed: Boolean(manifest.production_write_allowed),
    assetCount: Number(manifest.counts?.asset_count || 0),
    datasetMembershipCount: Number(manifest.counts?.dataset_membership_count || 0),
    parseRunCount: Number(manifest.counts?.parse_run_count || 0),
    profileCount: Number(manifest.counts?.profile_count || 0),
    plannedCleanupActionCount: Array.isArray(manifest.planned_cleanup_actions)
      ? manifest.planned_cleanup_actions.length
      : 0,
    approvalHashSupplied: Boolean(manifest.required_inputs?.approval_hash_supplied),
    liveReceiptSupplied: Boolean(manifest.required_inputs?.live_execute_receipt_supplied),
    countsSupplied: Boolean(manifest.required_inputs?.counts_supplied),
    operatorReviewRequired: Boolean(manifest.execution_controls?.operator_review_required),
    automaticCleanupAllowed: Boolean(manifest.execution_controls?.automatic_cleanup_allowed),
    recordCountsOnly: Boolean(manifest.audit_requirements?.record_counts_only),
    rawApprovalHashValueExcluded: Boolean(manifest.redaction?.raw_approval_hash_value_excluded),
    rawAssetIdsExcluded: Boolean(manifest.redaction?.raw_asset_ids_excluded),
    rawObjectLocatorExcluded: Boolean(manifest.redaction?.raw_object_locator_excluded),
    rawUrlExcluded: Boolean(manifest.redaction?.raw_url_excluded),
    localPathExcluded: Boolean(manifest.redaction?.local_path_excluded),
  };
}

function postExecuteCleanupManifestExpectedReady(manifest) {
  return Boolean(
    manifest.required_inputs?.approval_hash_supplied
      && manifest.required_inputs?.live_execute_receipt_supplied
      && manifest.required_inputs?.counts_supplied,
  );
}

function cleanupManifestRedacted(manifest, args) {
  const serialized = JSON.stringify(manifest);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildPostExecuteCleanupManifestEvidence(args, responseEvidence, liveReceiptSupplied) {
  const assetCount = Number(responseEvidence.summary?.normalizedBatchAssetCount || 0);
  const manifest = buildPostExecuteCleanupManifestDryRun({
    approvalHashSupplied: Boolean(args.approvalId),
    liveReceiptSupplied,
    assetCount,
    datasetMembershipCount: assetCount,
    parseRunCount: Number(responseEvidence.summary?.pendingParseRunCount || 0),
    profileCount: assetCount,
  });
  const summary = summarizePostExecuteCleanupManifest(manifest);
  return {
    manifest,
    summary,
    checks: {
      cleanupManifestDryRunReady: manifest.contract === 'fashion_design_asset_import_post_execute_cleanup_manifest_dry_run_v1'
        && manifest.mode === 'dry_run'
        && manifest.no_write === true
        && manifest.no_delete === true
        && manifest.production_write_allowed === false,
      cleanupManifestRequiresPostExecuteReceipt:
        manifest.required_inputs?.approval_hash_required === true
        && manifest.required_inputs?.live_execute_receipt_required === true
        && manifest.required_inputs?.counts_required === true,
      cleanupManifestReadyMatchesInputs:
        manifest.cleanup_manifest_ready === postExecuteCleanupManifestExpectedReady(manifest),
      cleanupManifestNoAutoCleanup:
        manifest.execution_controls?.operator_review_required === true
        && manifest.execution_controls?.cleanup_manifest_must_be_reviewed_before_action === true
        && manifest.execution_controls?.automatic_cleanup_allowed === false
        && manifest.execution_controls?.manual_db_changes_allowed_without_review === false,
      cleanupManifestRunbookReady:
        Array.isArray(manifest.planned_cleanup_actions)
        && manifest.planned_cleanup_actions.length === 6
        && manifest.selection_policy?.match_by_approval_hash === true
        && manifest.selection_policy?.shared_receipt_must_not_include_record_ids === true,
      cleanupManifestAuditCountsOnly:
        manifest.audit_requirements?.record_counts_only === true
        && manifest.audit_requirements?.record_auth_material === false
        && manifest.audit_requirements?.record_object_locators === false
        && manifest.audit_requirements?.record_source_urls === false,
      cleanupManifestRedacted: cleanupManifestRedacted(manifest, args),
    },
  };
}

function buildOperatorHandoffSummaryDryRun(operatorManifest, cleanupManifest) {
  const executeReady = Boolean(operatorManifest.execute_ready);
  const liveReceiptSupplied = Boolean(cleanupManifest.required_inputs?.live_execute_receipt_supplied);
  const cleanupManifestReady = Boolean(cleanupManifest.cleanup_manifest_ready);
  const cleanupAutoAllowed = Boolean(cleanupManifest.execution_controls?.automatic_cleanup_allowed);
  const manualReviewRequired = Boolean(
    operatorManifest.review_gates?.operator_review_required
      || cleanupManifest.execution_controls?.operator_review_required,
  );
  const importedAssetCount = Number(cleanupManifest.counts?.asset_count || 0);
  const executeState = liveReceiptSupplied
    ? 'executed_receipt_available'
    : executeReady
      ? 'ready_for_controlled_execute'
      : 'blocked_missing_execute_inputs';
  const cleanupState = cleanupManifestReady
    ? 'cleanup_manifest_ready_for_review'
    : liveReceiptSupplied
      ? 'cleanup_manifest_waiting_for_counts_or_review'
      : 'cleanup_waiting_for_live_receipt';
  const nextOperatorAction = !executeReady
    ? 'supply_reviewed_session_dataset_asset_library_ack_and_approval'
    : !liveReceiptSupplied
      ? 'review_manifest_then_run_controlled_execute'
      : cleanupManifestReady
        ? 'review_cleanup_manifest_before_any_cleanup'
        : 'review_live_receipt_and_build_cleanup_manifest';
  return {
    contract: 'fashion_design_asset_import_operator_handoff_summary_dry_run_v1',
    mode: 'dry_run',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    task_card_summary: {
      status: liveReceiptSupplied ? 'executed' : executeReady ? 'ready' : 'waiting_for_inputs',
      execute_state: executeState,
      cleanup_state: cleanupState,
      next_operator_action: nextOperatorAction,
      manual_review_required: manualReviewRequired,
      cleanup_auto_allowed: cleanupAutoAllowed,
      imported_asset_count: importedAssetCount,
    },
    validation_summary: {
      execute_ready: executeReady,
      live_receipt_supplied: liveReceiptSupplied,
      cleanup_manifest_ready: cleanupManifestReady,
      cleanup_auto_allowed: cleanupAutoAllowed,
      manual_review_required: manualReviewRequired,
      imported_asset_count: importedAssetCount,
      safe_for_shared_receipt: true,
    },
    operator_controls: {
      execute_requires_manifest_review: true,
      cleanup_requires_manifest_review: true,
      cleanup_auto_allowed: cleanupAutoAllowed,
      callback_allowed: false,
      task_creation_allowed: false,
    },
    redaction: {
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_approval_hash_value_excluded: true,
      raw_asset_ids_excluded: true,
      raw_object_locator_excluded: true,
      raw_url_excluded: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorHandoffSummary(summary) {
  return {
    contract: summary.contract,
    mode: summary.mode,
    noWrite: Boolean(summary.no_write),
    noDelete: Boolean(summary.no_delete),
    noCallback: Boolean(summary.no_callback),
    noTaskCreation: Boolean(summary.no_task_creation),
    publicContractChanged: Boolean(summary.public_contract_changed),
    productionWriteAllowed: Boolean(summary.production_write_allowed),
    taskStatus: summary.task_card_summary?.status || 'unknown',
    executeState: summary.task_card_summary?.execute_state || 'unknown',
    cleanupState: summary.task_card_summary?.cleanup_state || 'unknown',
    nextOperatorAction: summary.task_card_summary?.next_operator_action || 'unknown',
    manualReviewRequired: Boolean(summary.task_card_summary?.manual_review_required),
    cleanupAutoAllowed: Boolean(summary.task_card_summary?.cleanup_auto_allowed),
    importedAssetCount: Number(summary.task_card_summary?.imported_asset_count || 0),
    safeForSharedReceipt: Boolean(summary.validation_summary?.safe_for_shared_receipt),
  };
}

function operatorHandoffExpectedExecuteState(operatorManifest, cleanupManifest) {
  if (cleanupManifest.required_inputs?.live_execute_receipt_supplied) {
    return 'executed_receipt_available';
  }
  return operatorManifest.execute_ready
    ? 'ready_for_controlled_execute'
    : 'blocked_missing_execute_inputs';
}

function operatorHandoffExpectedCleanupState(cleanupManifest) {
  if (cleanupManifest.cleanup_manifest_ready) return 'cleanup_manifest_ready_for_review';
  if (cleanupManifest.required_inputs?.live_execute_receipt_supplied) {
    return 'cleanup_manifest_waiting_for_counts_or_review';
  }
  return 'cleanup_waiting_for_live_receipt';
}

function operatorHandoffRedacted(summary, args) {
  const serialized = JSON.stringify(summary);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorHandoffEvidence(args, operatorManifestEvidence, cleanupManifestEvidence) {
  const handoff = buildOperatorHandoffSummaryDryRun(
    operatorManifestEvidence.manifest,
    cleanupManifestEvidence.manifest,
  );
  const summary = summarizeOperatorHandoffSummary(handoff);
  return {
    handoff,
    summary,
    checks: {
      operatorHandoffDryRunReady:
        handoff.contract === 'fashion_design_asset_import_operator_handoff_summary_dry_run_v1'
        && handoff.mode === 'dry_run'
        && handoff.no_write === true
        && handoff.no_delete === true
        && handoff.no_callback === true
        && handoff.no_task_creation === true
        && handoff.production_write_allowed === false,
      operatorHandoffExecuteStateConsistent:
        handoff.task_card_summary?.execute_state
          === operatorHandoffExpectedExecuteState(
            operatorManifestEvidence.manifest,
            cleanupManifestEvidence.manifest,
          ),
      operatorHandoffCleanupStateConsistent:
        handoff.task_card_summary?.cleanup_state
          === operatorHandoffExpectedCleanupState(cleanupManifestEvidence.manifest),
      operatorHandoffTaskCardSummarySafe:
        ['waiting_for_inputs', 'ready', 'executed'].includes(handoff.task_card_summary?.status)
        && typeof handoff.task_card_summary?.next_operator_action === 'string'
        && handoff.task_card_summary.next_operator_action.length > 0,
      operatorHandoffValidationSummarySafe:
        handoff.validation_summary?.safe_for_shared_receipt === true
        && Number.isFinite(Number(handoff.validation_summary?.imported_asset_count || 0)),
      operatorHandoffNoSideEffects:
        handoff.operator_controls?.callback_allowed === false
        && handoff.operator_controls?.task_creation_allowed === false
        && handoff.operator_controls?.cleanup_auto_allowed === false
        && handoff.public_contract_changed === false,
      operatorHandoffRedacted: operatorHandoffRedacted(handoff, args),
    },
  };
}

function buildOperatorHandoffContractDriftGuardDryRun(handoff) {
  const detailStatus = handoff.task_card_summary?.status || 'unknown';
  const executeState = handoff.task_card_summary?.execute_state || 'unknown';
  const cleanupState = handoff.task_card_summary?.cleanup_state || 'unknown';
  const allowedDetailStatuses = ['waiting_for_inputs', 'ready', 'executed'];
  const allowedExecuteStates = [
    'blocked_missing_execute_inputs',
    'ready_for_controlled_execute',
    'executed_receipt_available',
  ];
  const allowedCleanupStates = [
    'cleanup_waiting_for_live_receipt',
    'cleanup_manifest_waiting_for_counts_or_review',
    'cleanup_manifest_ready_for_review',
  ];
  const detailStatusAllowed = allowedDetailStatuses.includes(detailStatus);
  const executeStateAllowed = allowedExecuteStates.includes(executeState);
  const cleanupStateAllowed = allowedCleanupStates.includes(cleanupState);
  return {
    contract: 'fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run_v1',
    mode: 'dry_run',
    surface: 'task_card_detail_panel_only',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    existing_task_card_status_enum_mutation_allowed: false,
    new_task_card_creation_allowed: false,
    third_party_callback_allowed: false,
    detail_status: detailStatus,
    execute_state: executeState,
    cleanup_state: cleanupState,
    allowed_detail_statuses: allowedDetailStatuses,
    allowed_execute_states: allowedExecuteStates,
    allowed_cleanup_states: allowedCleanupStates,
    validation: {
      detail_status_allowed: detailStatusAllowed,
      execute_state_allowed: executeStateAllowed,
      cleanup_state_allowed: cleanupStateAllowed,
      safe_for_task_card_detail: detailStatusAllowed && executeStateAllowed && cleanupStateAllowed,
      safe_for_validation_receipt: true,
    },
    field_contract: {
      allowed_top_level_fields: [
        'contract',
        'mode',
        'surface',
        'task_card_detail',
        'validation_detail',
        'operator_controls',
        'redaction',
      ],
      summary_must_not_replace_task_card_status: true,
      summary_must_not_emit_external_sse: true,
      summary_must_not_trigger_callback: true,
    },
    operator_controls: {
      detail_display_only: true,
      callback_allowed: false,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      cleanup_auto_allowed: false,
    },
    redaction: {
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_approval_hash_value_excluded: true,
      raw_asset_ids_excluded: true,
      raw_object_locator_excluded: true,
      raw_url_excluded: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorHandoffContractDriftGuard(guard) {
  return {
    contract: guard.contract,
    mode: guard.mode,
    surface: guard.surface,
    noWrite: Boolean(guard.no_write),
    noDelete: Boolean(guard.no_delete),
    noCallback: Boolean(guard.no_callback),
    noTaskCreation: Boolean(guard.no_task_creation),
    publicContractChanged: Boolean(guard.public_contract_changed),
    productionWriteAllowed: Boolean(guard.production_write_allowed),
    detailStatus: guard.detail_status,
    executeState: guard.execute_state,
    cleanupState: guard.cleanup_state,
    detailStatusAllowed: Boolean(guard.validation?.detail_status_allowed),
    executeStateAllowed: Boolean(guard.validation?.execute_state_allowed),
    cleanupStateAllowed: Boolean(guard.validation?.cleanup_state_allowed),
    safeForTaskCardDetail: Boolean(guard.validation?.safe_for_task_card_detail),
    summaryMustNotReplaceTaskCardStatus:
      Boolean(guard.field_contract?.summary_must_not_replace_task_card_status),
    detailDisplayOnly: Boolean(guard.operator_controls?.detail_display_only),
  };
}

function operatorHandoffContractGuardRedacted(guard, args) {
  const serialized = JSON.stringify(guard);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorHandoffContractGuardEvidence(args, operatorHandoffEvidence) {
  const guard = buildOperatorHandoffContractDriftGuardDryRun(operatorHandoffEvidence.handoff);
  const summary = summarizeOperatorHandoffContractDriftGuard(guard);
  return {
    guard,
    summary,
    checks: {
      operatorHandoffContractDriftGuardReady:
        guard.contract === 'fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run_v1'
        && guard.mode === 'dry_run'
        && guard.surface === 'task_card_detail_panel_only'
        && guard.no_write === true
        && guard.no_delete === true
        && guard.no_callback === true
        && guard.no_task_creation === true
        && guard.production_write_allowed === false,
      operatorHandoffDetailStatusAllowed:
        guard.validation?.detail_status_allowed === true
        && guard.validation?.execute_state_allowed === true
        && guard.validation?.cleanup_state_allowed === true,
      operatorHandoffDetailOnly:
        guard.operator_controls?.detail_display_only === true
        && guard.field_contract?.summary_must_not_replace_task_card_status === true,
      operatorHandoffDoesNotMutateTaskCardStatus:
        guard.existing_task_card_status_enum_mutation_allowed === false
        && guard.operator_controls?.task_status_mutation_allowed === false,
      operatorHandoffDoesNotCreateTaskCard:
        guard.new_task_card_creation_allowed === false
        && guard.operator_controls?.task_creation_allowed === false,
      operatorHandoffNoCallbackOrPublicContractChange:
        guard.third_party_callback_allowed === false
        && guard.operator_controls?.callback_allowed === false
        && guard.public_contract_changed === false,
      operatorHandoffContractGuardRedacted:
        operatorHandoffContractGuardRedacted(guard, args),
    },
  };
}

function buildOperatorReadinessRollupDryRun(operatorManifest, handoffGuard, cleanupManifest) {
  const executeManifestContractOk =
    operatorManifest.contract === 'fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1';
  const handoffGuardContractOk =
    handoffGuard.contract === 'fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run_v1';
  const cleanupManifestContractOk =
    cleanupManifest.contract === 'fashion_design_asset_import_post_execute_cleanup_manifest_dry_run_v1';
  const executeReady = Boolean(operatorManifest.execute_ready);
  const handoffDetailSafe = Boolean(handoffGuard.validation?.safe_for_task_card_detail);
  const cleanupManifestReady = Boolean(cleanupManifest.cleanup_manifest_ready);
  const cleanupAutoAllowed = Boolean(cleanupManifest.execution_controls?.automatic_cleanup_allowed);
  const liveReceiptSupplied =
    Boolean(cleanupManifest.required_inputs?.live_execute_receipt_supplied);
  const inputContractsOk =
    executeManifestContractOk && handoffGuardContractOk && cleanupManifestContractOk;
  const sideEffectGuardsOk =
    operatorManifest.no_write === true
    && handoffGuard.no_write === true
    && handoffGuard.no_delete === true
    && handoffGuard.no_callback === true
    && handoffGuard.no_task_creation === true
    && handoffGuard.public_contract_changed === false
    && cleanupManifest.no_write === true
    && cleanupManifest.no_delete === true
    && cleanupAutoAllowed === false
    && operatorManifest.production_write_allowed === false
    && handoffGuard.production_write_allowed === false
    && cleanupManifest.production_write_allowed === false;
  const readyForOperatorExecute =
    inputContractsOk
    && sideEffectGuardsOk
    && executeReady
    && handoffDetailSafe
    && !liveReceiptSupplied;
  const readyForOperatorCleanupReview =
    inputContractsOk
    && sideEffectGuardsOk
    && handoffDetailSafe
    && liveReceiptSupplied
    && cleanupManifestReady
    && !cleanupAutoAllowed;
  const overallStatus = !inputContractsOk
    ? 'blocked_contract_mismatch'
    : !sideEffectGuardsOk || !handoffDetailSafe
      ? 'blocked_safety_guard'
      : readyForOperatorCleanupReview
        ? 'ready_for_cleanup_review'
        : liveReceiptSupplied
          ? 'executed_waiting_for_cleanup_manifest'
          : readyForOperatorExecute
            ? 'ready_for_execute_review'
            : 'waiting_for_execute_inputs';
  const nextOperatorAction = {
    blocked_contract_mismatch: 'refresh_dry_run_inputs_before_review',
    blocked_safety_guard: 'fix_guard_or_manifest_before_review',
    ready_for_cleanup_review: 'review_cleanup_manifest_before_any_cleanup',
    executed_waiting_for_cleanup_manifest: 'build_or_complete_cleanup_manifest',
    ready_for_execute_review: 'review_rollup_and_manifest_then_run_controlled_execute',
    waiting_for_execute_inputs: 'supply_reviewed_session_dataset_asset_library_ack_and_approval',
  }[overallStatus];
  return {
    contract: 'fashion_design_asset_import_operator_readiness_rollup_dry_run_v1',
    mode: 'dry_run',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    inputs: {
      execute_manifest_contract_ok: executeManifestContractOk,
      handoff_guard_contract_ok: handoffGuardContractOk,
      cleanup_manifest_contract_ok: cleanupManifestContractOk,
      input_contracts_ok: inputContractsOk,
      execute_ready: executeReady,
      handoff_detail_safe: handoffDetailSafe,
      cleanup_manifest_ready: cleanupManifestReady,
      cleanup_auto_allowed: cleanupAutoAllowed,
      live_receipt_supplied: liveReceiptSupplied,
    },
    readiness: {
      overall_status: overallStatus,
      ready_for_operator_execute: readyForOperatorExecute,
      ready_for_operator_cleanup_review: readyForOperatorCleanupReview,
      side_effect_guards_ok: sideEffectGuardsOk,
      next_operator_action: nextOperatorAction,
      manual_review_required: true,
    },
    release_gate_summary: {
      reviewed_execute_manifest_required: true,
      reviewed_cleanup_manifest_required_after_execute: true,
      public_contract_change_allowed: false,
      external_callback_allowed: false,
      task_creation_allowed: false,
      automatic_cleanup_allowed: false,
      record_counts_only: true,
      safe_for_validation_receipt: inputContractsOk && sideEffectGuardsOk,
    },
    operator_controls: {
      execute_requires_operator_review: true,
      cleanup_requires_operator_review: true,
      live_execute_requires_ack: true,
      cleanup_auto_allowed: false,
      callback_allowed: false,
      task_creation_allowed: false,
      status_mutation_allowed: false,
    },
    redaction: {
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_approval_hash_value_excluded: true,
      raw_asset_ids_excluded: true,
      raw_object_locator_excluded: true,
      raw_url_excluded: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorReadinessRollup(rollup) {
  return {
    contract: rollup.contract,
    mode: rollup.mode,
    noWrite: Boolean(rollup.no_write),
    noDelete: Boolean(rollup.no_delete),
    noCallback: Boolean(rollup.no_callback),
    noTaskCreation: Boolean(rollup.no_task_creation),
    publicContractChanged: Boolean(rollup.public_contract_changed),
    productionWriteAllowed: Boolean(rollup.production_write_allowed),
    inputContractsOk: Boolean(rollup.inputs?.input_contracts_ok),
    executeReady: Boolean(rollup.inputs?.execute_ready),
    handoffDetailSafe: Boolean(rollup.inputs?.handoff_detail_safe),
    cleanupManifestReady: Boolean(rollup.inputs?.cleanup_manifest_ready),
    cleanupAutoAllowed: Boolean(rollup.inputs?.cleanup_auto_allowed),
    liveReceiptSupplied: Boolean(rollup.inputs?.live_receipt_supplied),
    overallStatus: rollup.readiness?.overall_status || 'unknown',
    readyForOperatorExecute: Boolean(rollup.readiness?.ready_for_operator_execute),
    readyForOperatorCleanupReview:
      Boolean(rollup.readiness?.ready_for_operator_cleanup_review),
    sideEffectGuardsOk: Boolean(rollup.readiness?.side_effect_guards_ok),
    nextOperatorAction: rollup.readiness?.next_operator_action || 'unknown',
    safeForValidationReceipt:
      Boolean(rollup.release_gate_summary?.safe_for_validation_receipt),
  };
}

function operatorReadinessRollupRedacted(rollup, args) {
  const serialized = JSON.stringify(rollup);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorReadinessRollupEvidence(
  args,
  operatorManifestEvidence,
  cleanupManifestEvidence,
  operatorHandoffContractGuardEvidence,
) {
  const rollup = buildOperatorReadinessRollupDryRun(
    operatorManifestEvidence.manifest,
    operatorHandoffContractGuardEvidence.guard,
    cleanupManifestEvidence.manifest,
  );
  const summary = summarizeOperatorReadinessRollup(rollup);
  return {
    rollup,
    summary,
    checks: {
      operatorReadinessRollupDryRunReady:
        rollup.contract === 'fashion_design_asset_import_operator_readiness_rollup_dry_run_v1'
        && rollup.mode === 'dry_run'
        && rollup.no_write === true
        && rollup.no_delete === true
        && rollup.no_callback === true
        && rollup.no_task_creation === true
        && rollup.production_write_allowed === false,
      operatorReadinessRollupInputContractsOk:
        rollup.inputs?.execute_manifest_contract_ok === true
        && rollup.inputs?.handoff_guard_contract_ok === true
        && rollup.inputs?.cleanup_manifest_contract_ok === true
        && rollup.inputs?.input_contracts_ok === true,
      operatorReadinessRollupExecuteGateConsistent:
        rollup.readiness?.ready_for_operator_execute
          === Boolean(
            rollup.inputs?.execute_ready
              && rollup.inputs?.handoff_detail_safe
              && rollup.readiness?.side_effect_guards_ok
              && !rollup.inputs?.live_receipt_supplied,
          ),
      operatorReadinessRollupCleanupGateConsistent:
        rollup.readiness?.ready_for_operator_cleanup_review
          === Boolean(
            rollup.inputs?.handoff_detail_safe
              && rollup.inputs?.live_receipt_supplied
              && rollup.inputs?.cleanup_manifest_ready
              && rollup.readiness?.side_effect_guards_ok
              && !rollup.inputs?.cleanup_auto_allowed,
          ),
      operatorReadinessRollupNoSideEffects:
        rollup.operator_controls?.callback_allowed === false
        && rollup.operator_controls?.task_creation_allowed === false
        && rollup.operator_controls?.status_mutation_allowed === false
        && rollup.release_gate_summary?.public_contract_change_allowed === false
        && rollup.release_gate_summary?.automatic_cleanup_allowed === false,
      operatorReadinessRollupRedacted:
        operatorReadinessRollupRedacted(rollup, args),
    },
  };
}

function redactedLiveCommandTemplateSafe(commandTemplate) {
  return typeof commandTemplate === 'string'
    && commandTemplate.includes('npm run smoke:fashion-design-asset-import')
    && commandTemplate.includes('--execute --ack-live-write')
    && commandTemplate.includes('--approval-id <reviewed-smoke-id>')
    && commandTemplate.includes('--base-url <main-site-base>')
    && commandTemplate.includes('<plus-v3-session-auth-material>')
    && !/https?:\/\//i.test(commandTemplate)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(commandTemplate)
    && !/[A-Za-z]:\\|\/Users\//i.test(commandTemplate);
}

function buildOperatorReadinessContractDriftGuardDryRun({
  readinessRollup,
  commandTemplateRedacted,
  publicDocsAssetImportsOpen,
}) {
  const rollupContractOk =
    readinessRollup.contract === 'fashion_design_asset_import_operator_readiness_rollup_dry_run_v1';
  const rollupSideEffectGuardsOk =
    readinessRollup.no_write === true
    && readinessRollup.no_delete === true
    && readinessRollup.no_callback === true
    && readinessRollup.no_task_creation === true
    && readinessRollup.public_contract_changed === false
    && readinessRollup.production_write_allowed === false
    && readinessRollup.release_gate_summary?.safe_for_validation_receipt === true;
  const rollupSerialized = JSON.stringify(readinessRollup);
  const rollupContainsCommandMaterial =
    /npm run|--execute|--base-url|<main-site-base>|<plus-v3-session-auth-material>/i
      .test(rollupSerialized);
  const publicDocsAssetImportsClosed = !publicDocsAssetImportsOpen;
  const safeForPrivateOperatorReview =
    rollupContractOk
    && rollupSideEffectGuardsOk
    && !rollupContainsCommandMaterial
    && commandTemplateRedacted;
  const safeForPublicDocs = publicDocsAssetImportsClosed;
  const driftStatus = !rollupContractOk
    ? 'blocked_rollup_contract_mismatch'
    : !rollupSideEffectGuardsOk
      ? 'blocked_rollup_safety_guard'
      : rollupContainsCommandMaterial
        ? 'blocked_rollup_command_material'
        : !commandTemplateRedacted
          ? 'blocked_unredacted_command_template'
          : !publicDocsAssetImportsClosed
            ? 'blocked_public_docs_expose_unreviewed_asset_imports'
            : 'safe_private_runbook_only';
  return {
    contract: 'fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1',
    mode: 'dry_run',
    surface: 'private_operator_runbook_and_validation_only',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    drift_status: driftStatus,
    readiness_rollup_contract_ok: rollupContractOk,
    readiness_rollup_side_effect_guards_ok: rollupSideEffectGuardsOk,
    rollup_contains_command_material: rollupContainsCommandMaterial,
    command_template_redacted: commandTemplateRedacted,
    command_template_placeholder_only: commandTemplateRedacted,
    public_docs_asset_imports_open: publicDocsAssetImportsOpen,
    public_docs_asset_imports_closed: publicDocsAssetImportsClosed,
    validation: {
      safe_for_private_operator_review: safeForPrivateOperatorReview,
      safe_for_public_docs: safeForPublicDocs,
      safe_for_shared_validation_receipt: safeForPrivateOperatorReview && safeForPublicDocs,
    },
    forbidden_public_material: {
      base_url_included: false,
      session_material_included: false,
      dataset_value_included: false,
      asset_library_value_included: false,
      approval_value_included: false,
      object_locator_value_included: false,
      executable_command_args_included: false,
      raw_command_included: false,
    },
    operator_controls: {
      private_runbook_only: true,
      public_docs_change_allowed: false,
      callback_allowed: false,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      cleanup_auto_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorReadinessContractGuard(guard) {
  return {
    contract: guard.contract,
    mode: guard.mode,
    surface: guard.surface,
    noWrite: Boolean(guard.no_write),
    noDelete: Boolean(guard.no_delete),
    noCallback: Boolean(guard.no_callback),
    noTaskCreation: Boolean(guard.no_task_creation),
    publicContractChanged: Boolean(guard.public_contract_changed),
    productionWriteAllowed: Boolean(guard.production_write_allowed),
    driftStatus: guard.drift_status || 'unknown',
    rollupContractOk: Boolean(guard.readiness_rollup_contract_ok),
    rollupSideEffectGuardsOk: Boolean(guard.readiness_rollup_side_effect_guards_ok),
    rollupContainsCommandMaterial: Boolean(guard.rollup_contains_command_material),
    commandTemplateRedacted: Boolean(guard.command_template_redacted),
    publicDocsAssetImportsClosed: Boolean(guard.public_docs_asset_imports_closed),
    safeForPrivateOperatorReview:
      Boolean(guard.validation?.safe_for_private_operator_review),
    safeForPublicDocs: Boolean(guard.validation?.safe_for_public_docs),
    safeForSharedValidationReceipt:
      Boolean(guard.validation?.safe_for_shared_validation_receipt),
    privateRunbookOnly: Boolean(guard.operator_controls?.private_runbook_only),
  };
}

function operatorReadinessContractGuardRedacted(guard, args) {
  const serialized = JSON.stringify(guard);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorReadinessContractGuardEvidence(args, operatorReadinessRollupEvidence) {
  const commandTemplate = buildRedactedLiveCommand(args);
  const commandTemplateRedacted = redactedLiveCommandTemplateSafe(commandTemplate);
  const publicDocsAssetImportsOpen = false;
  const guard = buildOperatorReadinessContractDriftGuardDryRun({
    readinessRollup: operatorReadinessRollupEvidence.rollup,
    commandTemplateRedacted,
    publicDocsAssetImportsOpen,
  });
  const summary = summarizeOperatorReadinessContractGuard(guard);
  return {
    guard,
    summary,
    checks: {
      operatorReadinessContractDriftGuardReady:
        guard.contract
          === 'fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1'
        && guard.mode === 'dry_run'
        && guard.surface === 'private_operator_runbook_and_validation_only'
        && guard.drift_status === 'safe_private_runbook_only'
        && guard.no_write === true
        && guard.no_delete === true
        && guard.no_callback === true
        && guard.no_task_creation === true
        && guard.production_write_allowed === false,
      operatorReadinessRollupContainsNoCommandMaterial:
        guard.rollup_contains_command_material === false
        && guard.forbidden_public_material?.executable_command_args_included === false
        && guard.forbidden_public_material?.raw_command_included === false,
      operatorReadinessPrivateRunbookOnly:
        guard.operator_controls?.private_runbook_only === true
        && guard.operator_controls?.public_docs_change_allowed === false,
      operatorReadinessCommandTemplateRedacted:
        guard.command_template_redacted === true
        && guard.command_template_placeholder_only === true,
      operatorReadinessPublicDocsDoNotExposeAssetImports:
        guard.public_docs_asset_imports_open === false
        && guard.public_docs_asset_imports_closed === true
        && guard.validation?.safe_for_public_docs === true,
      operatorReadinessContractGuardRedacted:
        operatorReadinessContractGuardRedacted(guard, args),
    },
  };
}

function buildPrivateRunbookReceiptPackageDryRun(readinessContractGuard) {
  const guardContractOk =
    readinessContractGuard.contract
      === 'fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1';
  const guardSafeForSharedReceipt =
    Boolean(readinessContractGuard.validation?.safe_for_shared_validation_receipt);
  const commandTemplateRedacted = Boolean(readinessContractGuard.command_template_redacted);
  const publicDocsClosed = Boolean(readinessContractGuard.public_docs_asset_imports_closed);
  const forbidden = readinessContractGuard.forbidden_public_material || {};
  const rawMaterialExcluded =
    forbidden.base_url_included === false
    && forbidden.session_material_included === false
    && forbidden.dataset_value_included === false
    && forbidden.asset_library_value_included === false
    && forbidden.approval_value_included === false
    && forbidden.object_locator_value_included === false
    && forbidden.executable_command_args_included === false
    && forbidden.raw_command_included === false;
  const sharedReceiptSafe =
    guardContractOk && guardSafeForSharedReceipt && rawMaterialExcluded && publicDocsClosed;
  const privateRunbookReady =
    guardContractOk
    && commandTemplateRedacted
    && rawMaterialExcluded
    && readinessContractGuard.operator_controls?.private_runbook_only === true;
  const packageReadyForReview = privateRunbookReady && sharedReceiptSafe;
  const packageStatus = !guardContractOk
    ? 'blocked_guard_contract_mismatch'
    : !rawMaterialExcluded
      ? 'blocked_raw_material_present'
      : !privateRunbookReady
        ? 'blocked_private_runbook_not_ready'
        : !sharedReceiptSafe
          ? 'blocked_shared_validation_receipt_not_safe'
          : 'ready_for_operator_review';
  return {
    contract: 'fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run_v1',
    mode: 'dry_run',
    surface: 'private_runbook_and_shared_validation_receipt_split',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    package_status: packageStatus,
    private_operator_runbook: {
      operator_only: true,
      included_in_shared_validation_receipt: false,
      contains_redacted_command_template: commandTemplateRedacted,
      contains_raw_session: false,
      contains_raw_dataset: false,
      contains_raw_asset_library: false,
      contains_raw_approval: false,
      contains_raw_object_locator: false,
      pre_execute_review_required: true,
      allowed_sections: [
        'redacted_command_template_placeholders',
        'required_inputs_checklist',
        'manual_review_gates',
        'rollback_requirements',
      ],
    },
    shared_validation_receipt: {
      contains_private_runbook: false,
      contains_command_template: false,
      contains_raw_session: false,
      contains_raw_dataset: false,
      contains_raw_asset_library: false,
      contains_raw_approval: false,
      contains_raw_object_locator: false,
      contains_status_labels: true,
      contains_counts_only: true,
      public_docs_asset_imports_closed: publicDocsClosed,
    },
    separation: {
      private_runbook_separate_from_shared_receipt: true,
      shared_receipt_safe: sharedReceiptSafe,
      pre_execute_raw_material_excluded: rawMaterialExcluded,
      command_template_private_only: true,
    },
    validation: {
      package_ready_for_review: packageReadyForReview,
      private_runbook_ready: privateRunbookReady,
      shared_validation_receipt_safe: sharedReceiptSafe,
      pre_execute_raw_material_excluded: rawMaterialExcluded,
    },
    operator_controls: {
      manual_review_required: true,
      callback_allowed: false,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      cleanup_auto_allowed: false,
      public_docs_change_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded_from_shared_receipt: true,
      local_path_excluded: true,
    },
  };
}

function summarizePrivateRunbookReceiptPackage(pkg) {
  return {
    contract: pkg.contract,
    mode: pkg.mode,
    surface: pkg.surface,
    packageStatus: pkg.package_status || 'unknown',
    noWrite: Boolean(pkg.no_write),
    noDelete: Boolean(pkg.no_delete),
    noCallback: Boolean(pkg.no_callback),
    noTaskCreation: Boolean(pkg.no_task_creation),
    publicContractChanged: Boolean(pkg.public_contract_changed),
    productionWriteAllowed: Boolean(pkg.production_write_allowed),
    privateRunbookReady: Boolean(pkg.validation?.private_runbook_ready),
    sharedValidationReceiptSafe: Boolean(pkg.validation?.shared_validation_receipt_safe),
    packageReadyForReview: Boolean(pkg.validation?.package_ready_for_review),
    privateRunbookSeparateFromSharedReceipt:
      Boolean(pkg.separation?.private_runbook_separate_from_shared_receipt),
    preExecuteRawMaterialExcluded:
      Boolean(pkg.separation?.pre_execute_raw_material_excluded),
    commandTemplatePrivateOnly: Boolean(pkg.separation?.command_template_private_only),
    sharedReceiptContainsPrivateRunbook:
      Boolean(pkg.shared_validation_receipt?.contains_private_runbook),
    sharedReceiptContainsCommandTemplate:
      Boolean(pkg.shared_validation_receipt?.contains_command_template),
  };
}

function privateRunbookReceiptPackageRedacted(pkg, args) {
  const serialized = JSON.stringify(pkg);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildPrivateRunbookReceiptPackageEvidence(args, operatorReadinessContractGuardEvidence) {
  const pkg = buildPrivateRunbookReceiptPackageDryRun(
    operatorReadinessContractGuardEvidence.guard,
  );
  const summary = summarizePrivateRunbookReceiptPackage(pkg);
  return {
    package: pkg,
    summary,
    checks: {
      operatorPrivateRunbookReceiptPackageReady:
        pkg.contract
          === 'fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run_v1'
        && pkg.mode === 'dry_run'
        && pkg.surface === 'private_runbook_and_shared_validation_receipt_split'
        && pkg.package_status === 'ready_for_operator_review'
        && pkg.no_write === true
        && pkg.no_delete === true
        && pkg.no_callback === true
        && pkg.no_task_creation === true
        && pkg.production_write_allowed === false,
      operatorPrivateRunbookSeparatedFromSharedReceipt:
        pkg.private_operator_runbook?.included_in_shared_validation_receipt === false
        && pkg.shared_validation_receipt?.contains_private_runbook === false
        && pkg.separation?.private_runbook_separate_from_shared_receipt === true,
      operatorPrivateRunbookPreExecuteNoRawMaterial:
        pkg.private_operator_runbook?.contains_raw_session === false
        && pkg.private_operator_runbook?.contains_raw_dataset === false
        && pkg.private_operator_runbook?.contains_raw_asset_library === false
        && pkg.private_operator_runbook?.contains_raw_approval === false
        && pkg.private_operator_runbook?.contains_raw_object_locator === false
        && pkg.separation?.pre_execute_raw_material_excluded === true,
      operatorSharedValidationReceiptSafe:
        pkg.shared_validation_receipt?.contains_command_template === false
        && pkg.shared_validation_receipt?.contains_raw_session === false
        && pkg.shared_validation_receipt?.contains_raw_dataset === false
        && pkg.shared_validation_receipt?.contains_raw_asset_library === false
        && pkg.shared_validation_receipt?.contains_raw_approval === false
        && pkg.shared_validation_receipt?.contains_raw_object_locator === false
        && pkg.validation?.shared_validation_receipt_safe === true,
      operatorReceiptPackageNoSideEffects:
        pkg.operator_controls?.callback_allowed === false
        && pkg.operator_controls?.task_creation_allowed === false
        && pkg.operator_controls?.task_status_mutation_allowed === false
        && pkg.operator_controls?.public_docs_change_allowed === false,
      operatorReceiptPackageRedacted:
        privateRunbookReceiptPackageRedacted(pkg, args),
    },
  };
}

function buildReceiptPackageDisplayContractDriftGuardDryRun(pkg) {
  const packageContractOk =
    pkg.contract === 'fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run_v1';
  const packageReadyForReview = Boolean(pkg.validation?.package_ready_for_review);
  const privateRunbookSeparate =
    Boolean(pkg.separation?.private_runbook_separate_from_shared_receipt);
  const sharedReceiptSafe = Boolean(pkg.validation?.shared_validation_receipt_safe);
  const privateRunbookHiddenFromSharedReceipt =
    pkg.shared_validation_receipt?.contains_private_runbook === false
    && pkg.private_operator_runbook?.included_in_shared_validation_receipt === false;
  const commandTemplateExcludedFromSharedReceipt =
    pkg.shared_validation_receipt?.contains_command_template === false;
  const noSideEffects =
    pkg.no_write === true
    && pkg.no_delete === true
    && pkg.no_callback === true
    && pkg.no_task_creation === true
    && pkg.public_contract_changed === false
    && pkg.production_write_allowed === false;
  const displaySafe =
    packageContractOk
    && packageReadyForReview
    && privateRunbookSeparate
    && sharedReceiptSafe
    && privateRunbookHiddenFromSharedReceipt
    && commandTemplateExcludedFromSharedReceipt
    && noSideEffects;
  const displayStatus = !packageContractOk
    ? 'blocked_package_contract_mismatch'
    : !packageReadyForReview
      ? 'blocked_package_not_ready'
      : !privateRunbookSeparate || !privateRunbookHiddenFromSharedReceipt
        ? 'blocked_private_runbook_shared_receipt_leak'
        : !commandTemplateExcludedFromSharedReceipt
          ? 'blocked_command_template_shared_receipt_leak'
          : !sharedReceiptSafe
            ? 'blocked_shared_receipt_not_safe'
            : !noSideEffects
              ? 'blocked_side_effect_guard'
              : 'ready_for_detail_display';
  return {
    contract: 'fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run_v1',
    mode: 'dry_run',
    surface: 'task_card_detail_and_validation_receipt_only',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    display_status: displayStatus,
    package_contract_ok: packageContractOk,
    package_ready_for_review: packageReadyForReview,
    private_runbook_separate_from_shared_receipt: privateRunbookSeparate,
    shared_validation_receipt_safe: sharedReceiptSafe,
    private_runbook_hidden_from_shared_receipt: privateRunbookHiddenFromSharedReceipt,
    command_template_excluded_from_shared_receipt: commandTemplateExcludedFromSharedReceipt,
    no_side_effects: noSideEffects,
    allowed_detail_fields: [
      'package_status',
      'private_runbook_ready',
      'shared_validation_receipt_safe',
      'pre_execute_raw_material_excluded',
      'manual_review_required',
    ],
    forbidden_display_behaviors: {
      creates_task_card: false,
      mutates_task_card_status_enum: false,
      emits_external_sse: false,
      triggers_callback: false,
      changes_public_docs: false,
      includes_private_runbook_in_shared_receipt: false,
      includes_command_template_in_shared_receipt: false,
    },
    validation: {
      safe_for_task_card_detail: displaySafe,
      safe_for_validation_ledger: displaySafe,
      safe_for_public_contract_guard: displaySafe,
    },
    operator_controls: {
      detail_display_only: true,
      callback_allowed: false,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      public_docs_change_allowed: false,
      cleanup_auto_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded_from_shared_receipt: true,
      local_path_excluded: true,
    },
  };
}

function summarizeReceiptPackageDisplayContractGuard(guard) {
  return {
    contract: guard.contract,
    mode: guard.mode,
    surface: guard.surface,
    displayStatus: guard.display_status || 'unknown',
    noWrite: Boolean(guard.no_write),
    noDelete: Boolean(guard.no_delete),
    noCallback: Boolean(guard.no_callback),
    noTaskCreation: Boolean(guard.no_task_creation),
    publicContractChanged: Boolean(guard.public_contract_changed),
    productionWriteAllowed: Boolean(guard.production_write_allowed),
    packageContractOk: Boolean(guard.package_contract_ok),
    packageReadyForReview: Boolean(guard.package_ready_for_review),
    privateRunbookSeparateFromSharedReceipt:
      Boolean(guard.private_runbook_separate_from_shared_receipt),
    privateRunbookHiddenFromSharedReceipt:
      Boolean(guard.private_runbook_hidden_from_shared_receipt),
    commandTemplateExcludedFromSharedReceipt:
      Boolean(guard.command_template_excluded_from_shared_receipt),
    safeForTaskCardDetail: Boolean(guard.validation?.safe_for_task_card_detail),
    safeForValidationLedger: Boolean(guard.validation?.safe_for_validation_ledger),
    detailDisplayOnly: Boolean(guard.operator_controls?.detail_display_only),
  };
}

function receiptPackageDisplayContractGuardRedacted(guard, args) {
  const serialized = JSON.stringify(guard);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildReceiptPackageDisplayContractGuardEvidence(args, privateRunbookReceiptPackageEvidence) {
  const guard = buildReceiptPackageDisplayContractDriftGuardDryRun(
    privateRunbookReceiptPackageEvidence.package,
  );
  const summary = summarizeReceiptPackageDisplayContractGuard(guard);
  return {
    guard,
    summary,
    checks: {
      operatorReceiptPackageDisplayContractGuardReady:
        guard.contract
          === 'fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run_v1'
        && guard.mode === 'dry_run'
        && guard.surface === 'task_card_detail_and_validation_receipt_only'
        && guard.display_status === 'ready_for_detail_display'
        && guard.no_write === true
        && guard.no_delete === true
        && guard.no_callback === true
        && guard.no_task_creation === true
        && guard.production_write_allowed === false,
      operatorReceiptPackageDetailOnly:
        guard.operator_controls?.detail_display_only === true
        && guard.validation?.safe_for_task_card_detail === true
        && guard.validation?.safe_for_validation_ledger === true,
      operatorReceiptPackageDoesNotCreateTaskCard:
        guard.forbidden_display_behaviors?.creates_task_card === false
        && guard.operator_controls?.task_creation_allowed === false,
      operatorReceiptPackageDoesNotMutateStatusEnum:
        guard.forbidden_display_behaviors?.mutates_task_card_status_enum === false
        && guard.operator_controls?.task_status_mutation_allowed === false,
      operatorReceiptPackageSharedReceiptExcludesPrivateMaterial:
        guard.private_runbook_hidden_from_shared_receipt === true
        && guard.command_template_excluded_from_shared_receipt === true
        && guard.forbidden_display_behaviors?.includes_private_runbook_in_shared_receipt === false
        && guard.forbidden_display_behaviors?.includes_command_template_in_shared_receipt === false,
      operatorReceiptPackageDisplayContractGuardRedacted:
        receiptPackageDisplayContractGuardRedacted(guard, args),
    },
  };
}

function buildOperatorValidationRollupDryRun({
  readinessContractGuard,
  receiptPackage,
  displayGuard,
}) {
  const readinessGuardContractOk =
    readinessContractGuard.contract
      === 'fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1';
  const receiptPackageContractOk =
    receiptPackage.contract
      === 'fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run_v1';
  const displayGuardContractOk =
    displayGuard.contract
      === 'fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run_v1';
  const readinessGuardReady =
    readinessGuardContractOk
    && readinessContractGuard.validation?.safe_for_shared_validation_receipt === true
    && readinessContractGuard.operator_controls?.private_runbook_only === true
    && readinessContractGuard.command_template_redacted === true
    && readinessContractGuard.public_docs_asset_imports_closed === true;
  const receiptPackageReady =
    receiptPackageContractOk
    && receiptPackage.validation?.package_ready_for_review === true
    && receiptPackage.validation?.shared_validation_receipt_safe === true
    && receiptPackage.separation?.private_runbook_separate_from_shared_receipt === true
    && receiptPackage.separation?.pre_execute_raw_material_excluded === true;
  const displayGuardReady =
    displayGuardContractOk
    && displayGuard.validation?.safe_for_task_card_detail === true
    && displayGuard.validation?.safe_for_validation_ledger === true
    && displayGuard.operator_controls?.detail_display_only === true
    && displayGuard.forbidden_display_behaviors?.creates_task_card === false
    && displayGuard.forbidden_display_behaviors?.mutates_task_card_status_enum === false
    && displayGuard.forbidden_display_behaviors?.includes_private_runbook_in_shared_receipt === false
    && displayGuard.forbidden_display_behaviors?.includes_command_template_in_shared_receipt === false;
  const sideEffectGuardsOk =
    readinessContractGuard.no_write === true
    && readinessContractGuard.no_delete === true
    && readinessContractGuard.no_callback === true
    && readinessContractGuard.no_task_creation === true
    && readinessContractGuard.public_contract_changed === false
    && readinessContractGuard.production_write_allowed === false
    && receiptPackage.no_write === true
    && receiptPackage.no_delete === true
    && receiptPackage.no_callback === true
    && receiptPackage.no_task_creation === true
    && receiptPackage.public_contract_changed === false
    && receiptPackage.production_write_allowed === false
    && displayGuard.no_write === true
    && displayGuard.no_delete === true
    && displayGuard.no_callback === true
    && displayGuard.no_task_creation === true
    && displayGuard.public_contract_changed === false
    && displayGuard.production_write_allowed === false;
  const validationRollupReady =
    readinessGuardReady && receiptPackageReady && displayGuardReady && sideEffectGuardsOk;
  const releaseBlockerCount = [
    readinessGuardReady,
    receiptPackageReady,
    displayGuardReady,
    sideEffectGuardsOk,
  ].filter((ready) => !ready).length;
  const rollupStatus = validationRollupReady
    ? 'ready_for_operator_validation_review'
    : !readinessGuardReady
      ? 'blocked_readiness_contract_guard'
      : !receiptPackageReady
        ? 'blocked_private_runbook_receipt_package'
        : !displayGuardReady
          ? 'blocked_display_contract_guard'
          : 'blocked_side_effect_guard';
  return {
    contract: 'fashion_design_asset_import_operator_validation_rollup_dry_run_v1',
    mode: 'dry_run',
    surface: 'operator_validation_release_gate_summary',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    rollup_status: rollupStatus,
    validation_rollup_ready: validationRollupReady,
    release_blocker_count: releaseBlockerCount,
    inputs: {
      readiness_guard_contract_ok: readinessGuardContractOk,
      receipt_package_contract_ok: receiptPackageContractOk,
      display_guard_contract_ok: displayGuardContractOk,
      readiness_guard_ready: readinessGuardReady,
      receipt_package_ready: receiptPackageReady,
      display_guard_ready: displayGuardReady,
      side_effect_guards_ok: sideEffectGuardsOk,
    },
    release_gate_summary: {
      single_operator_gate_ready: validationRollupReady,
      ready_for_live_execute: false,
      live_execute_still_requires_private_inputs: true,
      operator_credentials_required: true,
      reviewed_dataset_required: true,
      reviewed_asset_library_required: true,
      reviewed_approval_required: true,
      ack_live_write_required: true,
      safe_for_shared_validation_receipt: validationRollupReady,
    },
    forbidden_release_material: {
      base_url_included: false,
      session_material_included: false,
      dataset_value_included: false,
      asset_library_value_included: false,
      approval_value_included: false,
      object_locator_value_included: false,
      private_runbook_in_shared_receipt: false,
      command_template_in_shared_receipt: false,
    },
    operator_controls: {
      manual_review_required: true,
      detail_display_only: true,
      callback_allowed: false,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      public_docs_change_allowed: false,
      cleanup_auto_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded_from_shared_receipt: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorValidationRollup(rollup) {
  return {
    contract: rollup.contract,
    mode: rollup.mode,
    surface: rollup.surface,
    rollupStatus: rollup.rollup_status || 'unknown',
    noWrite: Boolean(rollup.no_write),
    noDelete: Boolean(rollup.no_delete),
    noCallback: Boolean(rollup.no_callback),
    noTaskCreation: Boolean(rollup.no_task_creation),
    publicContractChanged: Boolean(rollup.public_contract_changed),
    productionWriteAllowed: Boolean(rollup.production_write_allowed),
    validationRollupReady: Boolean(rollup.validation_rollup_ready),
    releaseBlockerCount: Number(rollup.release_blocker_count || 0),
    readinessGuardReady: Boolean(rollup.inputs?.readiness_guard_ready),
    receiptPackageReady: Boolean(rollup.inputs?.receipt_package_ready),
    displayGuardReady: Boolean(rollup.inputs?.display_guard_ready),
    sideEffectGuardsOk: Boolean(rollup.inputs?.side_effect_guards_ok),
    singleOperatorGateReady:
      Boolean(rollup.release_gate_summary?.single_operator_gate_ready),
    readyForLiveExecute: Boolean(rollup.release_gate_summary?.ready_for_live_execute),
    liveExecuteStillRequiresPrivateInputs:
      Boolean(rollup.release_gate_summary?.live_execute_still_requires_private_inputs),
  };
}

function operatorValidationRollupRedacted(rollup, args) {
  const serialized = JSON.stringify(rollup);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorValidationRollupEvidence(
  args,
  operatorReadinessContractGuardEvidence,
  privateRunbookReceiptPackageEvidence,
  receiptPackageDisplayContractGuardEvidence,
) {
  const rollup = buildOperatorValidationRollupDryRun({
    readinessContractGuard: operatorReadinessContractGuardEvidence.guard,
    receiptPackage: privateRunbookReceiptPackageEvidence.package,
    displayGuard: receiptPackageDisplayContractGuardEvidence.guard,
  });
  const summary = summarizeOperatorValidationRollup(rollup);
  return {
    rollup,
    summary,
    checks: {
      operatorValidationRollupDryRunReady:
        rollup.contract === 'fashion_design_asset_import_operator_validation_rollup_dry_run_v1'
        && rollup.mode === 'dry_run'
        && rollup.surface === 'operator_validation_release_gate_summary'
        && rollup.rollup_status === 'ready_for_operator_validation_review'
        && rollup.no_write === true
        && rollup.no_delete === true
        && rollup.no_callback === true
        && rollup.no_task_creation === true
        && rollup.production_write_allowed === false,
      operatorValidationRollupInputContractsOk:
        rollup.inputs?.readiness_guard_contract_ok === true
        && rollup.inputs?.receipt_package_contract_ok === true
        && rollup.inputs?.display_guard_contract_ok === true,
      operatorValidationRollupSingleGateReady:
        rollup.validation_rollup_ready === true
        && rollup.release_gate_summary?.single_operator_gate_ready === true
        && rollup.release_blocker_count === 0,
      operatorValidationRollupLiveExecuteStillRequiresPrivateInputs:
        rollup.release_gate_summary?.ready_for_live_execute === false
        && rollup.release_gate_summary?.live_execute_still_requires_private_inputs === true
        && rollup.release_gate_summary?.operator_credentials_required === true,
      operatorValidationRollupNoSideEffects:
        rollup.operator_controls?.callback_allowed === false
        && rollup.operator_controls?.task_creation_allowed === false
        && rollup.operator_controls?.task_status_mutation_allowed === false
        && rollup.operator_controls?.public_docs_change_allowed === false,
      operatorValidationRollupRedacted:
        operatorValidationRollupRedacted(rollup, args),
    },
  };
}

function buildOperatorValidationRollupDisplayContractGuardDryRun(validationRollup) {
  const rollupContractOk =
    validationRollup.contract === 'fashion_design_asset_import_operator_validation_rollup_dry_run_v1';
  const rollupReady = Boolean(validationRollup.validation_rollup_ready);
  const notLiveExecuteReady =
    validationRollup.release_gate_summary?.ready_for_live_execute === false;
  const livePrivateInputsRequired =
    validationRollup.release_gate_summary?.live_execute_still_requires_private_inputs === true;
  const sharedReceiptSafe =
    validationRollup.release_gate_summary?.safe_for_shared_validation_receipt === true;
  const noPrivateMaterial =
    validationRollup.forbidden_release_material?.base_url_included === false
    && validationRollup.forbidden_release_material?.session_material_included === false
    && validationRollup.forbidden_release_material?.dataset_value_included === false
    && validationRollup.forbidden_release_material?.asset_library_value_included === false
    && validationRollup.forbidden_release_material?.approval_value_included === false
    && validationRollup.forbidden_release_material?.object_locator_value_included === false
    && validationRollup.forbidden_release_material?.private_runbook_in_shared_receipt === false
    && validationRollup.forbidden_release_material?.command_template_in_shared_receipt === false;
  const noSideEffects =
    validationRollup.no_write === true
    && validationRollup.no_delete === true
    && validationRollup.no_callback === true
    && validationRollup.no_task_creation === true
    && validationRollup.public_contract_changed === false
    && validationRollup.production_write_allowed === false
    && validationRollup.operator_controls?.callback_allowed === false
    && validationRollup.operator_controls?.task_creation_allowed === false
    && validationRollup.operator_controls?.task_status_mutation_allowed === false
    && validationRollup.operator_controls?.public_docs_change_allowed === false;
  const displaySafe =
    rollupContractOk
    && rollupReady
    && notLiveExecuteReady
    && livePrivateInputsRequired
    && sharedReceiptSafe
    && noPrivateMaterial
    && noSideEffects;
  const displayStatus = !rollupContractOk
    ? 'blocked_validation_rollup_contract_mismatch'
    : !rollupReady
      ? 'blocked_validation_rollup_not_ready'
      : !notLiveExecuteReady || !livePrivateInputsRequired
        ? 'blocked_live_execute_gate_confusion'
        : !noPrivateMaterial
          ? 'blocked_private_material_leak'
          : !noSideEffects
            ? 'blocked_side_effect_guard'
            : 'ready_for_validation_gate_display';

  return {
    contract: 'fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run_v1',
    mode: 'dry_run',
    surface: 'validation_rollup_detail_and_markdown_only',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    display_status: displayStatus,
    rollup_contract_ok: rollupContractOk,
    rollup_ready: rollupReady,
    not_live_execute_ready: notLiveExecuteReady,
    live_private_inputs_required: livePrivateInputsRequired,
    shared_receipt_safe: sharedReceiptSafe,
    no_private_material: noPrivateMaterial,
    no_side_effects: noSideEffects,
    allowed_display_fields: [
      'rollup_status',
      'release_blocker_count',
      'validation_rollup_ready',
      'ready_for_live_execute',
      'live_execute_still_requires_private_inputs',
    ],
    forbidden_display_behaviors: {
      auto_triggers_live_execute: false,
      creates_task_card: false,
      updates_task_card: false,
      mutates_task_card_status_enum: false,
      emits_external_sse: false,
      triggers_callback: false,
      includes_private_runbook: false,
      includes_command_template: false,
      includes_raw_input_requirements: false,
    },
    validation: {
      safe_for_task_card_detail: displaySafe,
      safe_for_validation_ledger: displaySafe,
      safe_for_markdown_summary: displaySafe,
    },
    operator_controls: {
      detail_display_only: true,
      manual_review_required: true,
      callback_allowed: false,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      public_docs_change_allowed: false,
      cleanup_auto_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded_from_shared_receipt: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorValidationRollupDisplayContractGuard(guard) {
  return {
    contract: guard.contract,
    mode: guard.mode,
    surface: guard.surface,
    displayStatus: guard.display_status || 'unknown',
    noWrite: Boolean(guard.no_write),
    noDelete: Boolean(guard.no_delete),
    noCallback: Boolean(guard.no_callback),
    noTaskCreation: Boolean(guard.no_task_creation),
    publicContractChanged: Boolean(guard.public_contract_changed),
    productionWriteAllowed: Boolean(guard.production_write_allowed),
    rollupContractOk: Boolean(guard.rollup_contract_ok),
    rollupReady: Boolean(guard.rollup_ready),
    notLiveExecuteReady: Boolean(guard.not_live_execute_ready),
    livePrivateInputsRequired: Boolean(guard.live_private_inputs_required),
    sharedReceiptSafe: Boolean(guard.shared_receipt_safe),
    noPrivateMaterial: Boolean(guard.no_private_material),
    noSideEffects: Boolean(guard.no_side_effects),
    safeForTaskCardDetail: Boolean(guard.validation?.safe_for_task_card_detail),
    safeForValidationLedger: Boolean(guard.validation?.safe_for_validation_ledger),
    safeForMarkdownSummary: Boolean(guard.validation?.safe_for_markdown_summary),
    detailDisplayOnly: Boolean(guard.operator_controls?.detail_display_only),
  };
}

function operatorValidationRollupDisplayContractGuardRedacted(guard, args) {
  const serialized = JSON.stringify(guard);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorValidationRollupDisplayContractGuardEvidence(args, operatorValidationRollupEvidence) {
  const guard = buildOperatorValidationRollupDisplayContractGuardDryRun(
    operatorValidationRollupEvidence.rollup,
  );
  const summary = summarizeOperatorValidationRollupDisplayContractGuard(guard);
  return {
    guard,
    summary,
    checks: {
      operatorValidationRollupDisplayContractGuardReady:
        guard.contract
          === 'fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run_v1'
        && guard.mode === 'dry_run'
        && guard.surface === 'validation_rollup_detail_and_markdown_only'
        && guard.display_status === 'ready_for_validation_gate_display'
        && guard.no_write === true
        && guard.no_delete === true
        && guard.no_callback === true
        && guard.no_task_creation === true
        && guard.production_write_allowed === false,
      operatorValidationRollupDisplayOnly:
        guard.operator_controls?.detail_display_only === true
        && guard.validation?.safe_for_task_card_detail === true
        && guard.validation?.safe_for_validation_ledger === true
        && guard.validation?.safe_for_markdown_summary === true,
      operatorValidationRollupDoesNotTriggerLiveExecute:
        guard.not_live_execute_ready === true
        && guard.live_private_inputs_required === true
        && guard.forbidden_display_behaviors?.auto_triggers_live_execute === false,
      operatorValidationRollupDoesNotCreateOrUpdateTaskCard:
        guard.forbidden_display_behaviors?.creates_task_card === false
        && guard.forbidden_display_behaviors?.updates_task_card === false
        && guard.forbidden_display_behaviors?.mutates_task_card_status_enum === false
        && guard.operator_controls?.task_creation_allowed === false
        && guard.operator_controls?.task_status_mutation_allowed === false,
      operatorValidationRollupSharedReceiptExcludesPrivateMaterial:
        guard.no_private_material === true
        && guard.forbidden_display_behaviors?.includes_private_runbook === false
        && guard.forbidden_display_behaviors?.includes_command_template === false
        && guard.forbidden_display_behaviors?.includes_raw_input_requirements === false,
      operatorValidationRollupDisplayContractGuardRedacted:
        operatorValidationRollupDisplayContractGuardRedacted(guard, args),
    },
  };
}

function buildOperatorValidationReleaseSummaryDryRun(operatorManifest, validationRollupDisplayGuard) {
  const executeManifestContractOk =
    operatorManifest.contract === 'fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1';
  const displayGuardContractOk =
    validationRollupDisplayGuard.contract
      === 'fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run_v1';
  const requiredInputs = operatorManifest.required_inputs || {};
  const requiredPrivateInputs = [
    ['v3_user_session', requiredInputs.v3_user_session_required, requiredInputs.v3_user_session_supplied],
    ['existing_dataset', requiredInputs.existing_dataset_required, requiredInputs.existing_dataset_supplied],
    ['existing_asset_library', requiredInputs.existing_asset_library_required, requiredInputs.existing_asset_library_supplied],
    ['ack_live_write', requiredInputs.ack_live_write_required, requiredInputs.ack_live_write_supplied],
    ['approval_id', requiredInputs.approval_id_required, requiredInputs.approval_id_supplied],
  ];
  const missingPrivateInputs = requiredPrivateInputs
    .filter(([, required, supplied]) => required === true && supplied !== true)
    .map(([name]) => name);
  const requiredPrivateInputsDeclared = requiredPrivateInputs
    .every(([, required]) => required === true);
  const privateInputsOnlyMissing = requiredPrivateInputsDeclared;
  const manifestNoSideEffects =
    operatorManifest.mode === 'dry_run'
    && operatorManifest.no_write === true
    && operatorManifest.production_write_allowed === false
    && operatorManifest.review_gates?.operator_review_required === true
    && operatorManifest.review_gates?.manifest_must_be_reviewed_before_execute === true
    && operatorManifest.review_gates?.execute_requires_ack_live_write === true
    && operatorManifest.audit_requirements?.record_counts_only === true
    && operatorManifest.audit_requirements?.record_auth_material === false
    && operatorManifest.audit_requirements?.record_object_locators === false
    && operatorManifest.audit_requirements?.record_source_urls === false
    && operatorManifest.audit_requirements?.record_local_paths === false;
  const displayGuardReady =
    validationRollupDisplayGuard.display_status === 'ready_for_validation_gate_display'
    && validationRollupDisplayGuard.validation?.safe_for_task_card_detail === true
    && validationRollupDisplayGuard.validation?.safe_for_validation_ledger === true
    && validationRollupDisplayGuard.validation?.safe_for_markdown_summary === true
    && validationRollupDisplayGuard.not_live_execute_ready === true
    && validationRollupDisplayGuard.live_private_inputs_required === true;
  const publicContractStable =
    operatorManifest.public_contract_changed !== true
    && validationRollupDisplayGuard.public_contract_changed === false
    && validationRollupDisplayGuard.production_write_allowed === false;
  const taskCardContractStable =
    validationRollupDisplayGuard.forbidden_display_behaviors?.creates_task_card === false
    && validationRollupDisplayGuard.forbidden_display_behaviors?.updates_task_card === false
    && validationRollupDisplayGuard.forbidden_display_behaviors?.mutates_task_card_status_enum === false
    && validationRollupDisplayGuard.operator_controls?.task_creation_allowed === false
    && validationRollupDisplayGuard.operator_controls?.task_status_mutation_allowed === false;
  const sharedReceiptStable =
    validationRollupDisplayGuard.no_private_material === true
    && validationRollupDisplayGuard.forbidden_display_behaviors?.includes_private_runbook === false
    && validationRollupDisplayGuard.forbidden_display_behaviors?.includes_command_template === false
    && validationRollupDisplayGuard.forbidden_display_behaviors?.includes_raw_input_requirements === false;
  const releaseReadyForOperatorPrivateInputInjection =
    executeManifestContractOk
    && displayGuardContractOk
    && manifestNoSideEffects
    && displayGuardReady
    && privateInputsOnlyMissing
    && publicContractStable
    && taskCardContractStable
    && sharedReceiptStable;
  const releaseStatus = !executeManifestContractOk || !displayGuardContractOk
    ? 'blocked_release_summary_contract_mismatch'
    : !displayGuardReady
      ? 'blocked_validation_display_guard'
      : !manifestNoSideEffects
        ? 'blocked_manifest_side_effect_guard'
        : !publicContractStable
          ? 'blocked_public_contract_drift'
          : !taskCardContractStable
            ? 'blocked_task_card_contract_drift'
            : !sharedReceiptStable
              ? 'blocked_shared_receipt_contract_drift'
              : !privateInputsOnlyMissing
                ? 'blocked_private_input_manifest_shape'
                : 'ready_for_private_operator_input_review';

  return {
    contract: 'fashion_design_asset_import_operator_validation_release_summary_dry_run_v1',
    mode: 'dry_run',
    surface: 'operator_execute_preflight_release_summary',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    release_status: releaseStatus,
    execute_manifest_contract_ok: executeManifestContractOk,
    display_guard_contract_ok: displayGuardContractOk,
    manifest_no_side_effects: manifestNoSideEffects,
    display_guard_ready: displayGuardReady,
    private_inputs_only_missing: privateInputsOnlyMissing,
    missing_private_inputs: missingPrivateInputs,
    release_ready_for_operator_private_input_injection:
      releaseReadyForOperatorPrivateInputInjection,
    ready_for_live_execute: false,
    live_execute_still_requires_private_values: true,
    private_input_placeholders: {
      operator_session_required: Boolean(requiredInputs.v3_user_session_required),
      operator_session_supplied: Boolean(requiredInputs.v3_user_session_supplied),
      dataset_required: Boolean(requiredInputs.existing_dataset_required),
      dataset_supplied: Boolean(requiredInputs.existing_dataset_supplied),
      asset_library_required: Boolean(requiredInputs.existing_asset_library_required),
      asset_library_supplied: Boolean(requiredInputs.existing_asset_library_supplied),
      ack_live_write_required: Boolean(requiredInputs.ack_live_write_required),
      ack_live_write_supplied: Boolean(requiredInputs.ack_live_write_supplied),
      approval_required: Boolean(requiredInputs.approval_id_required),
      approval_supplied: Boolean(requiredInputs.approval_id_supplied),
    },
    unchanged_contracts: {
      public_third_party_contract: publicContractStable,
      task_card_status_enum: taskCardContractStable,
      shared_validation_receipt: sharedReceiptStable,
      asset_imports_public_contract: publicContractStable,
    },
    forbidden_release_behaviors: {
      auto_execute: false,
      public_contract_change: false,
      task_card_create: false,
      task_card_update: false,
      task_card_status_enum_mutation: false,
      shared_receipt_private_material: false,
      callback_dispatch: false,
      external_sse: false,
    },
    validation: {
      ready_for_release_summary_review: releaseReadyForOperatorPrivateInputInjection,
      safe_for_validation_ledger: releaseReadyForOperatorPrivateInputInjection,
      safe_for_markdown_summary: releaseReadyForOperatorPrivateInputInjection,
      execute_manifest_private_inputs_declared: requiredPrivateInputsDeclared,
    },
    operator_controls: {
      manual_private_input_injection_required: true,
      detail_display_only: true,
      live_execute_allowed: false,
      callback_allowed: false,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      public_docs_change_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded_from_shared_receipt: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorValidationReleaseSummary(summary) {
  return {
    contract: summary.contract,
    mode: summary.mode,
    surface: summary.surface,
    releaseStatus: summary.release_status || 'unknown',
    noWrite: Boolean(summary.no_write),
    noDelete: Boolean(summary.no_delete),
    noCallback: Boolean(summary.no_callback),
    noTaskCreation: Boolean(summary.no_task_creation),
    publicContractChanged: Boolean(summary.public_contract_changed),
    productionWriteAllowed: Boolean(summary.production_write_allowed),
    manifestNoSideEffects: Boolean(summary.manifest_no_side_effects),
    displayGuardReady: Boolean(summary.display_guard_ready),
    privateInputsOnlyMissing: Boolean(summary.private_inputs_only_missing),
    missingPrivateInputCount: Array.isArray(summary.missing_private_inputs)
      ? summary.missing_private_inputs.length
      : 0,
    releaseReadyForOperatorPrivateInputInjection:
      Boolean(summary.release_ready_for_operator_private_input_injection),
    readyForLiveExecute: Boolean(summary.ready_for_live_execute),
    liveExecuteStillRequiresPrivateValues:
      Boolean(summary.live_execute_still_requires_private_values),
    contractsStable:
      Boolean(summary.unchanged_contracts?.public_third_party_contract)
      && Boolean(summary.unchanged_contracts?.task_card_status_enum)
      && Boolean(summary.unchanged_contracts?.shared_validation_receipt)
      && Boolean(summary.unchanged_contracts?.asset_imports_public_contract),
    detailDisplayOnly: Boolean(summary.operator_controls?.detail_display_only),
  };
}

function operatorValidationReleaseSummaryRedacted(summary, args) {
  const serialized = JSON.stringify(summary);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorValidationReleaseSummaryEvidence(
  args,
  operatorManifestEvidence,
  operatorValidationRollupDisplayContractGuardEvidence,
) {
  const releaseSummary = buildOperatorValidationReleaseSummaryDryRun(
    operatorManifestEvidence.manifest,
    operatorValidationRollupDisplayContractGuardEvidence.guard,
  );
  const summary = summarizeOperatorValidationReleaseSummary(releaseSummary);
  return {
    releaseSummary,
    summary,
    checks: {
      operatorValidationReleaseSummaryReady:
        releaseSummary.contract
          === 'fashion_design_asset_import_operator_validation_release_summary_dry_run_v1'
        && releaseSummary.mode === 'dry_run'
        && releaseSummary.surface === 'operator_execute_preflight_release_summary'
        && releaseSummary.release_status === 'ready_for_private_operator_input_review'
        && releaseSummary.no_write === true
        && releaseSummary.no_delete === true
        && releaseSummary.no_callback === true
        && releaseSummary.no_task_creation === true
        && releaseSummary.production_write_allowed === false,
      operatorValidationReleaseSummaryPrivateInputsOnly:
        releaseSummary.private_inputs_only_missing === true
        && releaseSummary.validation?.execute_manifest_private_inputs_declared === true
        && releaseSummary.operator_controls?.manual_private_input_injection_required === true
        && releaseSummary.ready_for_live_execute === false
        && releaseSummary.live_execute_still_requires_private_values === true,
      operatorValidationReleaseSummaryContractsStable:
        releaseSummary.unchanged_contracts?.public_third_party_contract === true
        && releaseSummary.unchanged_contracts?.task_card_status_enum === true
        && releaseSummary.unchanged_contracts?.shared_validation_receipt === true
        && releaseSummary.unchanged_contracts?.asset_imports_public_contract === true,
      operatorValidationReleaseSummaryDoesNotCreateOrMutateTaskCard:
        releaseSummary.forbidden_release_behaviors?.task_card_create === false
        && releaseSummary.forbidden_release_behaviors?.task_card_update === false
        && releaseSummary.forbidden_release_behaviors?.task_card_status_enum_mutation === false
        && releaseSummary.operator_controls?.task_creation_allowed === false
        && releaseSummary.operator_controls?.task_status_mutation_allowed === false,
      operatorValidationReleaseSummaryDoesNotChangeSharedReceipt:
        releaseSummary.forbidden_release_behaviors?.shared_receipt_private_material === false
        && releaseSummary.unchanged_contracts?.shared_validation_receipt === true,
      operatorValidationReleaseSummaryRedacted:
        operatorValidationReleaseSummaryRedacted(releaseSummary, args),
    },
  };
}

function buildOperatorValidationReleaseSummaryFieldLedgerGuardDryRun(releaseSummary) {
  const releaseSummaryContractOk =
    releaseSummary.contract
      === 'fashion_design_asset_import_operator_validation_release_summary_dry_run_v1';
  const releaseSummaryReady =
    releaseSummary.release_status === 'ready_for_private_operator_input_review'
    && releaseSummary.release_ready_for_operator_private_input_injection === true
    && releaseSummary.validation?.ready_for_release_summary_review === true
    && releaseSummary.validation?.safe_for_validation_ledger === true
    && releaseSummary.validation?.safe_for_markdown_summary === true;
  const detailDisplayOnly = releaseSummary.operator_controls?.detail_display_only === true;
  const noLiveExecute =
    releaseSummary.ready_for_live_execute === false
    && releaseSummary.live_execute_still_requires_private_values === true
    && releaseSummary.operator_controls?.live_execute_allowed === false;
  const publicContractStable =
    releaseSummary.unchanged_contracts?.public_third_party_contract === true
    && releaseSummary.unchanged_contracts?.asset_imports_public_contract === true
    && releaseSummary.forbidden_release_behaviors?.public_contract_change === false
    && releaseSummary.public_contract_changed === false;
  const taskCardContractStable =
    releaseSummary.unchanged_contracts?.task_card_status_enum === true
    && releaseSummary.forbidden_release_behaviors?.task_card_create === false
    && releaseSummary.forbidden_release_behaviors?.task_card_update === false
    && releaseSummary.forbidden_release_behaviors?.task_card_status_enum_mutation === false
    && releaseSummary.operator_controls?.task_creation_allowed === false
    && releaseSummary.operator_controls?.task_status_mutation_allowed === false;
  const sharedValidationReceiptStable =
    releaseSummary.unchanged_contracts?.shared_validation_receipt === true
    && releaseSummary.forbidden_release_behaviors?.shared_receipt_private_material === false;
  const noSideEffects =
    releaseSummary.no_write === true
    && releaseSummary.no_delete === true
    && releaseSummary.no_callback === true
    && releaseSummary.no_task_creation === true
    && releaseSummary.production_write_allowed === false
    && releaseSummary.forbidden_release_behaviors?.auto_execute === false
    && releaseSummary.forbidden_release_behaviors?.callback_dispatch === false
    && releaseSummary.forbidden_release_behaviors?.external_sse === false
    && releaseSummary.operator_controls?.callback_allowed === false
    && releaseSummary.operator_controls?.public_docs_change_allowed === false;
  const safeForFieldLedger =
    releaseSummaryContractOk
    && releaseSummaryReady
    && detailDisplayOnly
    && noLiveExecute
    && publicContractStable
    && taskCardContractStable
    && sharedValidationReceiptStable
    && noSideEffects;
  const ledgerStatus = !releaseSummaryContractOk
    ? 'blocked_release_summary_contract_mismatch'
    : !releaseSummaryReady
      ? 'blocked_release_summary_not_ready'
      : !detailDisplayOnly || !noLiveExecute || !noSideEffects
        ? 'blocked_live_execute_or_side_effect_risk'
        : !publicContractStable
          ? 'blocked_public_contract_drift'
          : !taskCardContractStable
            ? 'blocked_task_card_contract_drift'
            : !sharedValidationReceiptStable
              ? 'blocked_shared_validation_receipt_drift'
              : 'ready_for_release_summary_field_ledger';

  return {
    contract:
      'fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run_v1',
    mode: 'dry_run',
    surface: 'task_detail_validation_ledger_field_allowlist',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    ledger_status: ledgerStatus,
    release_summary_contract_ok: releaseSummaryContractOk,
    release_summary_ready: releaseSummaryReady,
    detail_display_only: detailDisplayOnly,
    no_live_execute: noLiveExecute,
    public_contract_stable: publicContractStable,
    task_card_contract_stable: taskCardContractStable,
    shared_validation_receipt_stable: sharedValidationReceiptStable,
    no_side_effects: noSideEffects,
    allowed_task_detail_fields: [
      'release_status',
      'missing_private_inputs',
      'release_ready_for_operator_private_input_injection',
      'ready_for_live_execute',
      'live_execute_still_requires_private_values',
      'unchanged_contracts',
      'forbidden_release_behaviors',
      'redaction',
    ],
    allowed_validation_ledger_fields: [
      'contract',
      'mode',
      'surface',
      'release_status',
      'release_ready_for_operator_private_input_injection',
      'ready_for_live_execute',
      'live_execute_still_requires_private_values',
      'unchanged_contracts',
      'validation',
      'operator_controls',
      'redaction',
    ],
    forbidden_field_material: {
      private_input_values: false,
      raw_session: false,
      raw_dataset_id: false,
      raw_asset_library_id: false,
      raw_approval_id: false,
      raw_object_locator: false,
      command_template: false,
      callback_payload: false,
      external_sse_payload: false,
    },
    forbidden_ledger_behaviors: {
      auto_execute: false,
      creates_task_card: false,
      updates_task_card: false,
      mutates_task_card_status_enum: false,
      changes_shared_validation_receipt: false,
      emits_external_sse: false,
      triggers_callback: false,
    },
    validation: {
      safe_for_task_card_detail: safeForFieldLedger,
      safe_for_validation_ledger: safeForFieldLedger,
      safe_for_markdown_summary: safeForFieldLedger,
    },
    operator_controls: {
      field_ledger_only: true,
      operator_review_required: true,
      live_execute_allowed: false,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      callback_allowed: false,
      public_docs_change_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded_from_shared_receipt: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorValidationReleaseSummaryFieldLedgerGuard(guard) {
  return {
    contract: guard.contract,
    mode: guard.mode,
    surface: guard.surface,
    ledgerStatus: guard.ledger_status || 'unknown',
    noWrite: Boolean(guard.no_write),
    noDelete: Boolean(guard.no_delete),
    noCallback: Boolean(guard.no_callback),
    noTaskCreation: Boolean(guard.no_task_creation),
    publicContractChanged: Boolean(guard.public_contract_changed),
    productionWriteAllowed: Boolean(guard.production_write_allowed),
    releaseSummaryContractOk: Boolean(guard.release_summary_contract_ok),
    releaseSummaryReady: Boolean(guard.release_summary_ready),
    detailDisplayOnly: Boolean(guard.detail_display_only),
    noLiveExecute: Boolean(guard.no_live_execute),
    publicContractStable: Boolean(guard.public_contract_stable),
    taskCardContractStable: Boolean(guard.task_card_contract_stable),
    sharedValidationReceiptStable: Boolean(guard.shared_validation_receipt_stable),
    noSideEffects: Boolean(guard.no_side_effects),
    allowedTaskDetailFieldCount: Array.isArray(guard.allowed_task_detail_fields)
      ? guard.allowed_task_detail_fields.length
      : 0,
    allowedValidationLedgerFieldCount: Array.isArray(guard.allowed_validation_ledger_fields)
      ? guard.allowed_validation_ledger_fields.length
      : 0,
    fieldLedgerOnly: Boolean(guard.operator_controls?.field_ledger_only),
  };
}

function operatorValidationReleaseSummaryFieldLedgerGuardRedacted(guard, args) {
  const serialized = JSON.stringify(guard);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorValidationReleaseSummaryFieldLedgerGuardEvidence(
  args,
  operatorValidationReleaseSummaryEvidence,
) {
  const guard = buildOperatorValidationReleaseSummaryFieldLedgerGuardDryRun(
    operatorValidationReleaseSummaryEvidence.releaseSummary,
  );
  const summary = summarizeOperatorValidationReleaseSummaryFieldLedgerGuard(guard);
  return {
    guard,
    summary,
    checks: {
      operatorValidationReleaseSummaryFieldLedgerGuardReady:
        guard.contract
          === 'fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run_v1'
        && guard.mode === 'dry_run'
        && guard.surface === 'task_detail_validation_ledger_field_allowlist'
        && guard.ledger_status === 'ready_for_release_summary_field_ledger'
        && guard.no_write === true
        && guard.no_delete === true
        && guard.no_callback === true
        && guard.no_task_creation === true
        && guard.production_write_allowed === false,
      operatorValidationReleaseSummaryFieldLedgerOnly:
        guard.operator_controls?.field_ledger_only === true
        && guard.validation?.safe_for_task_card_detail === true
        && guard.validation?.safe_for_validation_ledger === true
        && guard.validation?.safe_for_markdown_summary === true,
      operatorValidationReleaseSummaryFieldLedgerNoLiveExecute:
        guard.no_live_execute === true
        && guard.forbidden_ledger_behaviors?.auto_execute === false
        && guard.operator_controls?.live_execute_allowed === false,
      operatorValidationReleaseSummaryFieldLedgerTaskCardStable:
        guard.task_card_contract_stable === true
        && guard.forbidden_ledger_behaviors?.creates_task_card === false
        && guard.forbidden_ledger_behaviors?.updates_task_card === false
        && guard.forbidden_ledger_behaviors?.mutates_task_card_status_enum === false,
      operatorValidationReleaseSummaryFieldLedgerSharedReceiptStable:
        guard.shared_validation_receipt_stable === true
        && guard.forbidden_ledger_behaviors?.changes_shared_validation_receipt === false
        && guard.forbidden_field_material?.private_input_values === false,
      operatorValidationReleaseSummaryFieldLedgerGuardRedacted:
        operatorValidationReleaseSummaryFieldLedgerGuardRedacted(guard, args),
    },
  };
}

function buildOperatorReadyHandoffReceiptDryRun(
  operatorManifest,
  operatorValidationReleaseSummary,
  operatorValidationReleaseSummaryFieldLedgerGuard,
) {
  const executeManifestContractOk =
    operatorManifest.contract === 'fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1';
  const releaseSummaryContractOk =
    operatorValidationReleaseSummary.contract
      === 'fashion_design_asset_import_operator_validation_release_summary_dry_run_v1';
  const fieldLedgerContractOk =
    operatorValidationReleaseSummaryFieldLedgerGuard.contract
      === 'fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run_v1';
  const requiredInputs = operatorManifest.required_inputs || {};
  const privateInputLabels = [
    'v3_user_session',
    'existing_dataset',
    'existing_asset_library',
    'ack_live_write',
    'approval_id',
  ];
  const privateInputsDeclared =
    requiredInputs.v3_user_session_required === true
    && requiredInputs.existing_dataset_required === true
    && requiredInputs.existing_asset_library_required === true
    && requiredInputs.ack_live_write_required === true
    && requiredInputs.approval_id_required === true
    && operatorValidationReleaseSummary.validation?.execute_manifest_private_inputs_declared === true;
  const manifestReviewGateReady =
    executeManifestContractOk
    && operatorManifest.mode === 'dry_run'
    && operatorManifest.no_write === true
    && operatorManifest.production_write_allowed === false
    && operatorManifest.review_gates?.operator_review_required === true
    && operatorManifest.review_gates?.manifest_must_be_reviewed_before_execute === true
    && operatorManifest.review_gates?.execute_requires_ack_live_write === true
    && operatorManifest.audit_requirements?.record_counts_only === true
    && operatorManifest.audit_requirements?.record_auth_material === false
    && operatorManifest.audit_requirements?.record_object_locators === false
    && operatorManifest.audit_requirements?.record_source_urls === false;
  const releaseSummaryReady =
    releaseSummaryContractOk
    && operatorValidationReleaseSummary.release_status === 'ready_for_private_operator_input_review'
    && operatorValidationReleaseSummary.release_ready_for_operator_private_input_injection === true
    && operatorValidationReleaseSummary.ready_for_live_execute === false
    && operatorValidationReleaseSummary.live_execute_still_requires_private_values === true;
  const fieldLedgerReady =
    fieldLedgerContractOk
    && operatorValidationReleaseSummaryFieldLedgerGuard.ledger_status
      === 'ready_for_release_summary_field_ledger'
    && operatorValidationReleaseSummaryFieldLedgerGuard.validation?.safe_for_task_card_detail === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.validation?.safe_for_validation_ledger === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.operator_controls?.field_ledger_only === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.no_live_execute === true;
  const publicContractStable =
    operatorValidationReleaseSummary.unchanged_contracts?.public_third_party_contract === true
    && operatorValidationReleaseSummary.unchanged_contracts?.asset_imports_public_contract === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.public_contract_stable === true
    && operatorValidationReleaseSummary.public_contract_changed === false
    && operatorValidationReleaseSummaryFieldLedgerGuard.public_contract_changed === false;
  const taskCardContractStable =
    operatorValidationReleaseSummary.unchanged_contracts?.task_card_status_enum === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.task_card_contract_stable === true
    && operatorValidationReleaseSummary.forbidden_release_behaviors?.task_card_create === false
    && operatorValidationReleaseSummary.forbidden_release_behaviors?.task_card_update === false
    && operatorValidationReleaseSummary.forbidden_release_behaviors?.task_card_status_enum_mutation === false;
  const sharedValidationReceiptStable =
    operatorValidationReleaseSummary.unchanged_contracts?.shared_validation_receipt === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.shared_validation_receipt_stable === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.forbidden_ledger_behaviors
      ?.changes_shared_validation_receipt === false;
  const noSideEffects =
    operatorValidationReleaseSummary.no_write === true
    && operatorValidationReleaseSummary.no_delete === true
    && operatorValidationReleaseSummary.no_callback === true
    && operatorValidationReleaseSummary.no_task_creation === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.no_write === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.no_delete === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.no_callback === true
    && operatorValidationReleaseSummaryFieldLedgerGuard.no_task_creation === true
    && operatorValidationReleaseSummary.production_write_allowed === false
    && operatorValidationReleaseSummaryFieldLedgerGuard.production_write_allowed === false
    && operatorValidationReleaseSummary.forbidden_release_behaviors?.auto_execute === false
    && operatorValidationReleaseSummaryFieldLedgerGuard.forbidden_ledger_behaviors?.auto_execute === false
    && operatorValidationReleaseSummary.operator_controls?.callback_allowed === false
    && operatorValidationReleaseSummaryFieldLedgerGuard.operator_controls?.callback_allowed === false;
  const handoffReadyForOperatorPrivateValues =
    executeManifestContractOk
    && releaseSummaryContractOk
    && fieldLedgerContractOk
    && privateInputsDeclared
    && manifestReviewGateReady
    && releaseSummaryReady
    && fieldLedgerReady
    && publicContractStable
    && taskCardContractStable
    && sharedValidationReceiptStable
    && noSideEffects;
  const handoffStatus = !executeManifestContractOk
    || !releaseSummaryContractOk
    || !fieldLedgerContractOk
    ? 'blocked_operator_handoff_contract_mismatch'
    : !privateInputsDeclared || !manifestReviewGateReady
      ? 'blocked_execute_manifest_review_gate'
      : !releaseSummaryReady
        ? 'blocked_release_summary_not_ready'
        : !fieldLedgerReady
          ? 'blocked_field_ledger_not_ready'
          : !publicContractStable
            ? 'blocked_public_contract_drift'
            : !taskCardContractStable
              ? 'blocked_task_card_contract_drift'
              : !sharedValidationReceiptStable
                ? 'blocked_shared_validation_receipt_drift'
                : !noSideEffects
                  ? 'blocked_side_effect_guard'
                  : 'ready_for_operator_private_values';

  return {
    contract: 'fashion_design_asset_import_operator_ready_handoff_receipt_dry_run_v1',
    mode: 'dry_run',
    surface: 'operator_ready_private_execute_handoff_receipt',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    handoff_status: handoffStatus,
    execute_manifest_contract_ok: executeManifestContractOk,
    release_summary_contract_ok: releaseSummaryContractOk,
    field_ledger_contract_ok: fieldLedgerContractOk,
    private_inputs_declared: privateInputsDeclared,
    manifest_review_gate_ready: manifestReviewGateReady,
    release_summary_ready: releaseSummaryReady,
    field_ledger_ready: fieldLedgerReady,
    public_contract_stable: publicContractStable,
    task_card_contract_stable: taskCardContractStable,
    shared_validation_receipt_stable: sharedValidationReceiptStable,
    no_side_effects: noSideEffects,
    handoff_ready_for_operator_private_values: handoffReadyForOperatorPrivateValues,
    ready_for_live_execute: false,
    live_execute_still_requires_private_values: true,
    operator_private_input_labels: privateInputLabels,
    operator_next_step: {
      private_value_injection_required: true,
      operator_review_required: true,
      controlled_execute_after_review_only: true,
      public_contract_change_required: false,
      task_card_status_enum_change_required: false,
      shared_validation_receipt_change_required: false,
      main_site_display_field_change_required: false,
    },
    unchanged_contracts: {
      public_third_party_contract: publicContractStable,
      task_card_status_enum: taskCardContractStable,
      shared_validation_receipt: sharedValidationReceiptStable,
      main_site_display_fields: fieldLedgerReady,
      asset_imports_public_contract: publicContractStable,
    },
    forbidden_handoff_behaviors: {
      auto_execute: false,
      creates_task_card: false,
      updates_task_card: false,
      mutates_task_card_status_enum: false,
      changes_shared_validation_receipt: false,
      changes_public_contract: false,
      changes_main_site_display_fields: false,
      emits_external_sse: false,
      triggers_callback: false,
      includes_private_input_values: false,
      includes_command_template: false,
    },
    validation: {
      safe_for_operator_handoff_receipt: handoffReadyForOperatorPrivateValues,
      safe_for_validation_ledger: handoffReadyForOperatorPrivateValues,
      safe_for_markdown_summary: handoffReadyForOperatorPrivateValues,
    },
    operator_controls: {
      private_value_injection_required: true,
      operator_review_required: true,
      live_execute_allowed: false,
      controlled_execute_after_review_only: true,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      callback_allowed: false,
      public_docs_change_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded_from_shared_receipt: true,
      local_path_excluded: true,
    },
  };
}

function summarizeOperatorReadyHandoffReceipt(receipt) {
  return {
    contract: receipt.contract,
    mode: receipt.mode,
    surface: receipt.surface,
    handoffStatus: receipt.handoff_status || 'unknown',
    noWrite: Boolean(receipt.no_write),
    noDelete: Boolean(receipt.no_delete),
    noCallback: Boolean(receipt.no_callback),
    noTaskCreation: Boolean(receipt.no_task_creation),
    productionWriteAllowed: Boolean(receipt.production_write_allowed),
    privateInputsDeclared: Boolean(receipt.private_inputs_declared),
    manifestReviewGateReady: Boolean(receipt.manifest_review_gate_ready),
    releaseSummaryReady: Boolean(receipt.release_summary_ready),
    fieldLedgerReady: Boolean(receipt.field_ledger_ready),
    publicContractStable: Boolean(receipt.public_contract_stable),
    taskCardContractStable: Boolean(receipt.task_card_contract_stable),
    sharedValidationReceiptStable: Boolean(receipt.shared_validation_receipt_stable),
    noSideEffects: Boolean(receipt.no_side_effects),
    handoffReadyForOperatorPrivateValues:
      Boolean(receipt.handoff_ready_for_operator_private_values),
    readyForLiveExecute: Boolean(receipt.ready_for_live_execute),
    liveExecuteStillRequiresPrivateValues:
      Boolean(receipt.live_execute_still_requires_private_values),
    privateInputLabelCount: Array.isArray(receipt.operator_private_input_labels)
      ? receipt.operator_private_input_labels.length
      : 0,
  };
}

function operatorReadyHandoffReceiptRedacted(receipt, args) {
  const serialized = JSON.stringify(receipt);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildOperatorReadyHandoffReceiptEvidence(
  args,
  operatorManifestEvidence,
  operatorValidationReleaseSummaryEvidence,
  operatorValidationReleaseSummaryFieldLedgerGuardEvidence,
) {
  const receipt = buildOperatorReadyHandoffReceiptDryRun(
    operatorManifestEvidence.manifest,
    operatorValidationReleaseSummaryEvidence.releaseSummary,
    operatorValidationReleaseSummaryFieldLedgerGuardEvidence.guard,
  );
  const summary = summarizeOperatorReadyHandoffReceipt(receipt);
  return {
    receipt,
    summary,
    checks: {
      operatorReadyHandoffReceiptReady:
        receipt.contract
          === 'fashion_design_asset_import_operator_ready_handoff_receipt_dry_run_v1'
        && receipt.mode === 'dry_run'
        && receipt.surface === 'operator_ready_private_execute_handoff_receipt'
        && receipt.handoff_status === 'ready_for_operator_private_values'
        && receipt.no_write === true
        && receipt.no_delete === true
        && receipt.no_callback === true
        && receipt.no_task_creation === true
        && receipt.production_write_allowed === false,
      operatorReadyHandoffRequiresOnlyPrivateValues:
        receipt.handoff_ready_for_operator_private_values === true
        && receipt.operator_next_step?.private_value_injection_required === true
        && receipt.operator_next_step?.operator_review_required === true
        && receipt.ready_for_live_execute === false
        && receipt.live_execute_still_requires_private_values === true,
      operatorReadyHandoffContractsStable:
        receipt.unchanged_contracts?.public_third_party_contract === true
        && receipt.unchanged_contracts?.task_card_status_enum === true
        && receipt.unchanged_contracts?.shared_validation_receipt === true
        && receipt.unchanged_contracts?.main_site_display_fields === true
        && receipt.unchanged_contracts?.asset_imports_public_contract === true,
      operatorReadyHandoffNoTaskCardOrSharedReceiptMutation:
        receipt.forbidden_handoff_behaviors?.creates_task_card === false
        && receipt.forbidden_handoff_behaviors?.updates_task_card === false
        && receipt.forbidden_handoff_behaviors?.mutates_task_card_status_enum === false
        && receipt.forbidden_handoff_behaviors?.changes_shared_validation_receipt === false,
      operatorReadyHandoffNoExternalEffects:
        receipt.forbidden_handoff_behaviors?.auto_execute === false
        && receipt.forbidden_handoff_behaviors?.emits_external_sse === false
        && receipt.forbidden_handoff_behaviors?.triggers_callback === false
        && receipt.operator_controls?.live_execute_allowed === false,
      operatorReadyHandoffReceiptRedacted:
        operatorReadyHandoffReceiptRedacted(receipt, args),
    },
  };
}

function buildFinalValidationReadinessBundleDryRun(operatorReadyHandoffReceipt) {
  const handoffContractOk =
    operatorReadyHandoffReceipt.contract
      === 'fashion_design_asset_import_operator_ready_handoff_receipt_dry_run_v1';
  const handoffReady = handoffContractOk
    && operatorReadyHandoffReceipt.mode === 'dry_run'
    && operatorReadyHandoffReceipt.surface === 'operator_ready_private_execute_handoff_receipt'
    && operatorReadyHandoffReceipt.handoff_status === 'ready_for_operator_private_values'
    && operatorReadyHandoffReceipt.handoff_ready_for_operator_private_values === true
    && operatorReadyHandoffReceipt.ready_for_live_execute === false
    && operatorReadyHandoffReceipt.live_execute_still_requires_private_values === true;
  const unchanged = operatorReadyHandoffReceipt.unchanged_contracts || {};
  const contractsStable = unchanged.public_third_party_contract === true
    && unchanged.task_card_status_enum === true
    && unchanged.shared_validation_receipt === true
    && unchanged.main_site_display_fields === true
    && unchanged.asset_imports_public_contract === true
    && operatorReadyHandoffReceipt.public_contract_changed === false;
  const forbidden = operatorReadyHandoffReceipt.forbidden_handoff_behaviors || {};
  const noTaskCardOrSharedReceiptMutation =
    forbidden.creates_task_card === false
    && forbidden.updates_task_card === false
    && forbidden.mutates_task_card_status_enum === false
    && forbidden.changes_shared_validation_receipt === false
    && forbidden.changes_main_site_display_fields === false;
  const controls = operatorReadyHandoffReceipt.operator_controls || {};
  const noExternalEffects = operatorReadyHandoffReceipt.no_write === true
    && operatorReadyHandoffReceipt.no_delete === true
    && operatorReadyHandoffReceipt.no_callback === true
    && operatorReadyHandoffReceipt.no_task_creation === true
    && operatorReadyHandoffReceipt.production_write_allowed === false
    && forbidden.auto_execute === false
    && forbidden.emits_external_sse === false
    && forbidden.triggers_callback === false
    && controls.live_execute_allowed === false
    && controls.callback_allowed === false
    && controls.task_creation_allowed === false
    && controls.task_status_mutation_allowed === false;
  const redaction = operatorReadyHandoffReceipt.redaction || {};
  const redactionReady = redaction.raw_base_url_excluded === true
    && redaction.raw_session_excluded === true
    && redaction.raw_dataset_id_excluded === true
    && redaction.raw_asset_library_id_excluded === true
    && redaction.raw_approval_id_excluded === true
    && redaction.raw_object_locator_excluded === true
    && forbidden.includes_private_input_values === false
    && forbidden.includes_command_template === false;
  const bundleReadyForOperatorReview = handoffReady
    && contractsStable
    && noTaskCardOrSharedReceiptMutation
    && noExternalEffects
    && redactionReady;
  const bundleStatus = !handoffContractOk
    ? 'blocked_handoff_contract_mismatch'
    : !handoffReady
      ? 'blocked_handoff_not_ready'
      : !contractsStable
        ? 'blocked_contract_stability'
        : !noTaskCardOrSharedReceiptMutation
          ? 'blocked_mutation_guard'
          : !noExternalEffects
            ? 'blocked_side_effect_guard'
            : !redactionReady
              ? 'blocked_redaction_guard'
              : 'ready_for_operator_review_bundle';
  const releaseBlockers = bundleReadyForOperatorReview ? [] : [bundleStatus];

  return {
    contract: 'fashion_design_asset_import_final_validation_readiness_bundle_dry_run_v1',
    mode: 'dry_run',
    surface: 'final_validation_readiness_bundle',
    no_write: true,
    no_delete: true,
    no_callback: true,
    no_task_creation: true,
    public_contract_changed: false,
    production_write_allowed: false,
    bundle_status: bundleStatus,
    bundle_ready_for_operator_review: bundleReadyForOperatorReview,
    release_blocker_count: releaseBlockers.length,
    release_blockers: releaseBlockers,
    ready_for_live_execute: false,
    live_execute_still_requires_private_values: true,
    handoff_status: operatorReadyHandoffReceipt.handoff_status || 'unknown',
    operator_private_input_label_count:
      Array.isArray(operatorReadyHandoffReceipt.operator_private_input_labels)
        ? operatorReadyHandoffReceipt.operator_private_input_labels.length
        : 0,
    included_sections: [
      'execute_manifest_review_gate',
      'operator_validation_release_summary',
      'release_summary_field_ledger',
      'operator_ready_handoff_receipt',
    ],
    validation_readiness: {
      handoff_receipt_ready: handoffReady,
      private_values_only: handoffReady,
      public_contract_stable: unchanged.public_third_party_contract === true,
      task_card_contract_stable: unchanged.task_card_status_enum === true,
      shared_validation_receipt_stable: unchanged.shared_validation_receipt === true,
      main_site_display_fields_stable: unchanged.main_site_display_fields === true,
      asset_imports_public_contract_stable:
        unchanged.asset_imports_public_contract === true,
      no_task_card_or_shared_receipt_mutation: noTaskCardOrSharedReceiptMutation,
      no_external_effects: noExternalEffects,
      redaction_ready: redactionReady,
      single_gate_ready: bundleReadyForOperatorReview,
    },
    operator_next_step: {
      private_value_injection_required: true,
      operator_review_required: true,
      final_bundle_review_required: true,
      controlled_execute_after_review_only: true,
      public_contract_change_required: false,
      task_card_status_enum_change_required: false,
      shared_validation_receipt_change_required: false,
      main_site_display_field_change_required: false,
    },
    unchanged_contracts: {
      public_third_party_contract: unchanged.public_third_party_contract === true,
      task_card_status_enum: unchanged.task_card_status_enum === true,
      shared_validation_receipt: unchanged.shared_validation_receipt === true,
      main_site_display_fields: unchanged.main_site_display_fields === true,
      asset_imports_public_contract: unchanged.asset_imports_public_contract === true,
    },
    forbidden_bundle_behaviors: {
      auto_execute: false,
      creates_task_card: false,
      updates_task_card: false,
      mutates_task_card_status_enum: false,
      changes_shared_validation_receipt: false,
      changes_public_contract: false,
      changes_main_site_display_fields: false,
      emits_external_sse: false,
      triggers_callback: false,
      includes_private_input_values: false,
      includes_command_template: false,
    },
    validation: {
      safe_for_final_validation_bundle: bundleReadyForOperatorReview,
      safe_for_validation_ledger: bundleReadyForOperatorReview,
      safe_for_markdown_summary: bundleReadyForOperatorReview,
    },
    operator_controls: {
      final_bundle_only: true,
      private_value_injection_required: true,
      operator_review_required: true,
      live_execute_allowed: false,
      controlled_execute_after_review_only: true,
      task_creation_allowed: false,
      task_status_mutation_allowed: false,
      callback_allowed: false,
      public_docs_change_allowed: false,
    },
    redaction: {
      raw_base_url_excluded: true,
      raw_session_excluded: true,
      raw_dataset_id_excluded: true,
      raw_asset_library_id_excluded: true,
      raw_approval_id_excluded: true,
      raw_object_locator_excluded: true,
      raw_command_excluded_from_shared_receipt: true,
      local_path_excluded: true,
    },
  };
}

function summarizeFinalValidationReadinessBundle(bundle) {
  return {
    contract: bundle.contract,
    mode: bundle.mode,
    surface: bundle.surface,
    bundleStatus: bundle.bundle_status || 'unknown',
    bundleReadyForOperatorReview:
      Boolean(bundle.bundle_ready_for_operator_review),
    releaseBlockerCount: Number(bundle.release_blocker_count || 0),
    readyForLiveExecute: Boolean(bundle.ready_for_live_execute),
    liveExecuteStillRequiresPrivateValues:
      Boolean(bundle.live_execute_still_requires_private_values),
    singleGateReady: Boolean(bundle.validation_readiness?.single_gate_ready),
    privateValuesOnly: Boolean(bundle.validation_readiness?.private_values_only),
    publicContractStable:
      Boolean(bundle.validation_readiness?.public_contract_stable),
    taskCardContractStable:
      Boolean(bundle.validation_readiness?.task_card_contract_stable),
    sharedValidationReceiptStable:
      Boolean(bundle.validation_readiness?.shared_validation_receipt_stable),
    mainSiteDisplayFieldsStable:
      Boolean(bundle.validation_readiness?.main_site_display_fields_stable),
    noExternalEffects: Boolean(bundle.validation_readiness?.no_external_effects),
    redactionReady: Boolean(bundle.validation_readiness?.redaction_ready),
  };
}

function finalValidationReadinessBundleRedacted(bundle, args) {
  const serialized = JSON.stringify(bundle);
  const unsafeNeedles = [
    args.cookie,
    args.bearer,
    args.datasetId,
    args.assetLibraryId,
    args.approvalId,
    args.localThreadId,
  ]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  return !/https?:\/\//i.test(serialized)
    && !/object[_-]?key/i.test(serialized)
    && !/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized)
    && !serialized.includes(ROOT_DIR)
    && unsafeNeedles.every((needle) => !serialized.includes(needle));
}

function buildFinalValidationReadinessBundleEvidence(
  args,
  operatorReadyHandoffReceiptEvidence,
) {
  const bundle = buildFinalValidationReadinessBundleDryRun(
    operatorReadyHandoffReceiptEvidence.receipt,
  );
  const summary = summarizeFinalValidationReadinessBundle(bundle);
  return {
    bundle,
    summary,
    checks: {
      finalValidationReadinessBundleReady:
        bundle.contract
          === 'fashion_design_asset_import_final_validation_readiness_bundle_dry_run_v1'
        && bundle.mode === 'dry_run'
        && bundle.surface === 'final_validation_readiness_bundle'
        && bundle.bundle_status === 'ready_for_operator_review_bundle'
        && bundle.no_write === true
        && bundle.no_delete === true
        && bundle.no_callback === true
        && bundle.no_task_creation === true
        && bundle.production_write_allowed === false,
      finalValidationBundleSingleGateReady:
        bundle.bundle_ready_for_operator_review === true
        && bundle.validation_readiness?.single_gate_ready === true
        && bundle.release_blocker_count === 0,
      finalValidationBundleRequiresOnlyPrivateValues:
        bundle.operator_next_step?.private_value_injection_required === true
        && bundle.operator_next_step?.operator_review_required === true
        && bundle.ready_for_live_execute === false
        && bundle.live_execute_still_requires_private_values === true,
      finalValidationBundleContractsStable:
        bundle.unchanged_contracts?.public_third_party_contract === true
        && bundle.unchanged_contracts?.task_card_status_enum === true
        && bundle.unchanged_contracts?.shared_validation_receipt === true
        && bundle.unchanged_contracts?.main_site_display_fields === true
        && bundle.unchanged_contracts?.asset_imports_public_contract === true,
      finalValidationBundleNoExternalEffects:
        bundle.forbidden_bundle_behaviors?.auto_execute === false
        && bundle.forbidden_bundle_behaviors?.emits_external_sse === false
        && bundle.forbidden_bundle_behaviors?.triggers_callback === false
        && bundle.operator_controls?.live_execute_allowed === false
        && bundle.operator_controls?.task_creation_allowed === false,
      finalValidationBundleRedacted:
        finalValidationReadinessBundleRedacted(bundle, args),
    },
  };
}

function buildPreflightReport(
  args,
  fixtures,
  payloadEvidence,
  responseEvidence,
  scopeEvidence,
  planningEvidence,
  followupEvidence,
  scopeGuardEvidence,
) {
  const operatorManifestEvidence = buildOperatorManifestEvidence(args);
  const cleanupManifestEvidence = buildPostExecuteCleanupManifestEvidence(
    args,
    responseEvidence,
    false,
  );
  const operatorHandoffEvidence = buildOperatorHandoffEvidence(
    args,
    operatorManifestEvidence,
    cleanupManifestEvidence,
  );
  const operatorHandoffContractGuardEvidence = buildOperatorHandoffContractGuardEvidence(
    args,
    operatorHandoffEvidence,
  );
  const operatorReadinessRollupEvidence = buildOperatorReadinessRollupEvidence(
    args,
    operatorManifestEvidence,
    cleanupManifestEvidence,
    operatorHandoffContractGuardEvidence,
  );
  const operatorReadinessContractGuardEvidence = buildOperatorReadinessContractGuardEvidence(
    args,
    operatorReadinessRollupEvidence,
  );
  const privateRunbookReceiptPackageEvidence = buildPrivateRunbookReceiptPackageEvidence(
    args,
    operatorReadinessContractGuardEvidence,
  );
  const receiptPackageDisplayContractGuardEvidence =
    buildReceiptPackageDisplayContractGuardEvidence(
      args,
      privateRunbookReceiptPackageEvidence,
    );
  const operatorValidationRollupEvidence = buildOperatorValidationRollupEvidence(
    args,
    operatorReadinessContractGuardEvidence,
    privateRunbookReceiptPackageEvidence,
    receiptPackageDisplayContractGuardEvidence,
  );
  const operatorValidationRollupDisplayContractGuardEvidence =
    buildOperatorValidationRollupDisplayContractGuardEvidence(
      args,
      operatorValidationRollupEvidence,
    );
  const operatorValidationReleaseSummaryEvidence =
    buildOperatorValidationReleaseSummaryEvidence(
      args,
      operatorManifestEvidence,
      operatorValidationRollupDisplayContractGuardEvidence,
    );
  const operatorValidationReleaseSummaryFieldLedgerGuardEvidence =
    buildOperatorValidationReleaseSummaryFieldLedgerGuardEvidence(
      args,
      operatorValidationReleaseSummaryEvidence,
    );
  const operatorReadyHandoffReceiptEvidence =
    buildOperatorReadyHandoffReceiptEvidence(
      args,
      operatorManifestEvidence,
      operatorValidationReleaseSummaryEvidence,
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence,
    );
  const finalValidationReadinessBundleEvidence =
    buildFinalValidationReadinessBundleEvidence(
      args,
      operatorReadyHandoffReceiptEvidence,
    );
  const report = {
    smoke: 'fashion-design-asset-import',
    mode: 'preflight',
    ok: true,
    preflight: true,
    generated_at: new Date().toISOString(),
    target: {
      base: redactedUrlSummary(args.baseUrl),
      datasetIdPresent: Boolean(args.datasetId),
      assetLibraryIdPresent: Boolean(args.assetLibraryId),
      authSupplied: Boolean(args.cookie || args.bearer),
      approvalSupplied: Boolean(args.approvalId),
    },
    fixture_summary: {
      png_fixture_available: fixtures.pngBytes > 0,
      zip_fixture_available: fixtures.zipBytes > 0,
      png_bytes: fixtures.pngBytes,
      zip_bytes: fixtures.zipBytes,
      png_sha256: fixtures.pngSha256,
      zip_sha256: fixtures.zipSha256,
      zip_expected_image_count: 2,
      zip_expected_skipped_file_count: 1,
    },
    import_summary: payloadEvidence.summary,
    response_summary: responseEvidence.summary,
    scope_summary: scopeEvidence.summary,
    planning_summary: planningEvidence.summary,
    followup_summary: followupEvidence.summary,
    scope_guard_summary: scopeGuardEvidence.summary,
    operator_manifest: operatorManifestEvidence.manifest,
    operator_manifest_summary: operatorManifestEvidence.summary,
    cleanup_manifest: cleanupManifestEvidence.manifest,
    cleanup_manifest_summary: cleanupManifestEvidence.summary,
    operator_handoff: operatorHandoffEvidence.handoff,
    operator_handoff_summary: operatorHandoffEvidence.summary,
    operator_handoff_contract_guard: operatorHandoffContractGuardEvidence.guard,
    operator_handoff_contract_guard_summary: operatorHandoffContractGuardEvidence.summary,
    operator_readiness_rollup: operatorReadinessRollupEvidence.rollup,
    operator_readiness_rollup_summary: operatorReadinessRollupEvidence.summary,
    operator_readiness_contract_guard: operatorReadinessContractGuardEvidence.guard,
    operator_readiness_contract_guard_summary: operatorReadinessContractGuardEvidence.summary,
    private_runbook_receipt_package: privateRunbookReceiptPackageEvidence.package,
    private_runbook_receipt_package_summary: privateRunbookReceiptPackageEvidence.summary,
    receipt_package_display_contract_guard: receiptPackageDisplayContractGuardEvidence.guard,
    receipt_package_display_contract_guard_summary:
      receiptPackageDisplayContractGuardEvidence.summary,
    operator_validation_rollup: operatorValidationRollupEvidence.rollup,
    operator_validation_rollup_summary: operatorValidationRollupEvidence.summary,
    operator_validation_rollup_display_contract_guard:
      operatorValidationRollupDisplayContractGuardEvidence.guard,
    operator_validation_rollup_display_contract_guard_summary:
      operatorValidationRollupDisplayContractGuardEvidence.summary,
    operator_validation_release_summary:
      operatorValidationReleaseSummaryEvidence.releaseSummary,
    operator_validation_release_summary_summary:
      operatorValidationReleaseSummaryEvidence.summary,
    operator_validation_release_summary_field_ledger_guard:
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence.guard,
    operator_validation_release_summary_field_ledger_guard_summary:
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence.summary,
    operator_ready_handoff_receipt: operatorReadyHandoffReceiptEvidence.receipt,
    operator_ready_handoff_receipt_summary:
      operatorReadyHandoffReceiptEvidence.summary,
    final_validation_readiness_bundle:
      finalValidationReadinessBundleEvidence.bundle,
    final_validation_readiness_bundle_summary:
      finalValidationReadinessBundleEvidence.summary,
    live_write_scope: {
      networkCallsRun: false,
      productionWriteAllowed: false,
      liveWriteApprovalRequired: true,
      liveWriteApprovalGateEnforced: true,
      plannedSteps: [
        'upload_png_and_zip_fixture',
        'attach_existing_dataset_to_existing_asset_library',
        'create_fashion_design_asset_import_batch',
        'read_asset_library_scope_summary',
      ],
    },
    redaction: {
      rawBaseUrlIncluded: false,
      rawLocalPathIncluded: false,
      uploadReferenceIncluded: false,
      sessionSecretIncluded: false,
      apiTokenIncluded: false,
      providerPayloadsIncluded: false,
    },
    commandTemplate: buildRedactedLiveCommand(args),
    checks: {
      realFixturesWritten: fixtures.pngBytes > 0 && fixtures.zipBytes > 0,
      singleImportPayloadValid: payloadEvidence.summary.singlePayloadValid,
      batchImportPayloadValid: payloadEvidence.summary.batchPayloadValid,
      payloadItemMetadataRedacted: payloadEvidence.summary.payloadItemMetadataRedacted,
      payloadRootMetadataRedacted: payloadEvidence.summary.payloadRootMetadataRedacted,
      payloadPackageMetadataRedacted: payloadEvidence.summary.payloadPackageMetadataRedacted,
      zipPackagePayloadValid: payloadEvidence.summary.batchPackageCount === 1
        && payloadEvidence.summary.packageContentType === 'application/zip',
      normalizedResponseIncludesParseRun: responseEvidence.summary.pendingParseRunCount === 4,
      normalizedImportResponseRedacted: responseEvidence.summary.normalizedImportResponseRedacted,
      scopeProfileHintsUsable: scopeEvidence.summary.scopeNormalized
        && scopeEvidence.summary.designQueryFilterHit,
      scopeAssetsRedacted: scopeEvidence.summary.scopeAssetLocatorsRedacted,
      scopeParseStatusVisible: scopeEvidence.summary.parseStatusPendingCount === 2
        && scopeEvidence.summary.parseRunCount === 3,
      staticPagePlanningSeesParseStatus: planningEvidence.summary.staticPageEvidenceParseStatusVisible
        && planningEvidence.summary.planningPendingAssetCount === 2,
      reportPlannerAstSeesParseStatus: planningEvidence.summary.reportPlannerAstParseStatusVisible
        && planningEvidence.summary.reportPlannerAssetMaterialsInsertedForPendingAssets,
      planningLayerRedacted: planningEvidence.summary.planningSensitiveFieldsRedacted,
      parseQueueDryRunReady: followupEvidence.summary.parseQueueDryRunReady
        && followupEvidence.summary.parseQueuePendingCount === 4,
      parseStatusTransitionDryRunReady:
        followupEvidence.summary.parseStatusTransitionDryRunReady,
      retrievalEvidenceDryRunReady: followupEvidence.summary.retrievalEvidenceDryRunReady
        && followupEvidence.summary.retrievalEvidencePreviewCount === 2,
      retrievalEvidenceTextMaterialized:
        followupEvidence.summary.retrievalEvidenceTextMaterialized,
      retrievalEvidenceIdempotencyPlanned:
        followupEvidence.summary.retrievalEvidenceIdempotencyPlanned,
      retrievalEvidenceAdapterDryRunReady:
        followupEvidence.summary.retrievalEvidenceAdapterDryRunReady,
      profileEvidenceWriteOrderConstrained:
        followupEvidence.summary.profileEvidenceWriteOrderConstrained,
      assetProfileRetrievalStorageMappingDryRunReady:
        followupEvidence.summary.assetProfileRetrievalStorageMappingDryRunReady,
      syntheticDocumentChunkRejected:
        followupEvidence.summary.syntheticDocumentChunkRejected,
      assetEvidenceTableRecommended:
        followupEvidence.summary.assetEvidenceTableRecommended,
      assetRetrievalEvidenceMigrationSketchReady:
        followupEvidence.summary.assetRetrievalEvidenceMigrationSketchReady,
      unionSearchNoWriteAdapterDraftReady:
        followupEvidence.summary.unionSearchNoWriteAdapterDraftReady,
      assetSearchMembershipGuardPlanned:
        followupEvidence.summary.assetSearchMembershipGuardPlanned,
      unionSearchMergeRankFixtureReady:
        followupEvidence.summary.unionSearchMergeRankFixtureReady,
      sourceKindRegressionDryRunReady:
        followupEvidence.summary.sourceKindRegressionDryRunReady,
      unionSearchMergeRankNoRawLocator:
        followupEvidence.summary.unionSearchMergeRankNoRawLocator,
      unionSearchExplainDebugSummaryReady:
        followupEvidence.summary.unionSearchExplainDebugSummaryReady,
      modelFacingSupplyCompressionDryRunReady:
        followupEvidence.summary.modelFacingSupplyCompressionDryRunReady,
      modelFacingSupplyCompressionNoRawLocator:
        followupEvidence.summary.modelFacingSupplyCompressionNoRawLocator,
      assistantRunSupplyBudgetQualityGateDryRunReady:
        followupEvidence.summary.assistantRunSupplyBudgetQualityGateDryRunReady,
      assetDocumentSupplySourceKindDedupeReady:
        followupEvidence.summary.assetDocumentSupplySourceKindDedupeReady,
      insufficientSupplyContinuePlanned:
        followupEvidence.summary.insufficientSupplyContinuePlanned,
      assistantRunSupplyGateNoRawLocator:
        followupEvidence.summary.assistantRunSupplyGateNoRawLocator,
      assistantRunSupplyProgressEventsDryRunReady:
        followupEvidence.summary.assistantRunSupplyProgressEventsDryRunReady,
      assistantRunProgressNonBlocking:
        followupEvidence.summary.assistantRunProgressNonBlocking,
      assistantRunProgressInsufficientSupplyContinues:
        followupEvidence.summary.assistantRunProgressInsufficientSupplyContinues,
      assistantRunProgressParseWaitingOrRetryVisible:
        followupEvidence.summary.assistantRunProgressParseWaitingOrRetryVisible,
      assistantRunProgressNoRawLocator:
        followupEvidence.summary.assistantRunProgressNoRawLocator,
      assistantRunProgressContractDriftGuardReady:
        followupEvidence.summary.assistantRunProgressContractDriftGuardReady,
      assistantRunProgressDoesNotMutatePublicSseFields:
        followupEvidence.summary.assistantRunProgressDoesNotMutatePublicSseFields,
      assistantRunProgressNoCallbackOrWrite:
        followupEvidence.summary.assistantRunProgressNoCallbackOrWrite,
      assistantRunProgressFailureDoesNotBlockAnswer:
        followupEvidence.summary.assistantRunProgressFailureDoesNotBlockAnswer,
      assistantRunProgressTaskCardDetailOnly:
        followupEvidence.summary.assistantRunProgressTaskCardDetailOnly,
      parserLiveAdapterReadinessDryRunReady:
        followupEvidence.summary.parserLiveAdapterReadinessDryRunReady,
      parserLiveAdapterNoNetworkOrModelCall:
        followupEvidence.summary.parserLiveAdapterNoNetworkOrModelCall,
      parserLiveAdapterRequiresEndpointOnly:
        followupEvidence.summary.parserLiveAdapterRequiresEndpointOnly,
      parserLiveAdapterContractsStable:
        followupEvidence.summary.parserLiveAdapterContractsStable,
      parserLiveAdapterNoRawPayload:
        followupEvidence.summary.parserLiveAdapterNoRawPayload,
      parserLiveAdapterRedacted:
        followupEvidence.summary.parserLiveAdapterRedacted,
      assetRetrievalEvidenceWriterReadinessReady:
        followupEvidence.summary.assetRetrievalEvidenceWriterReadinessReady,
      assetRetrievalEvidenceWriterFieldLedgerReady:
        followupEvidence.summary.assetRetrievalEvidenceWriterFieldLedgerReady,
      assetRetrievalEvidenceWriterIdempotent:
        followupEvidence.summary.assetRetrievalEvidenceWriterIdempotent,
      assetRetrievalEvidenceWriterMembershipGuardReady:
        followupEvidence.summary.assetRetrievalEvidenceWriterMembershipGuardReady,
      assetRetrievalEvidenceWriterRollbackAuditReady:
        followupEvidence.summary.assetRetrievalEvidenceWriterRollbackAuditReady,
      assetRetrievalEvidenceWriterRedacted:
        followupEvidence.summary.assetRetrievalEvidenceWriterRedacted,
      thirdPartyPrivateAssetImportEndpointGuardReady:
        followupEvidence.summary.thirdPartyPrivateAssetImportEndpointGuardReady,
      thirdPartyPrivateAssetImportDefaultPrivate:
        followupEvidence.summary.thirdPartyPrivateAssetImportDefaultPrivate,
      thirdPartyPrivateAssetImportStructuredJsonReady:
        followupEvidence.summary.thirdPartyPrivateAssetImportStructuredJsonReady,
      thirdPartyPrivateAssetImportScopeGuardReady:
        followupEvidence.summary.thirdPartyPrivateAssetImportScopeGuardReady,
      thirdPartyPrivateAssetImportPublicDocsClosed:
        followupEvidence.summary.thirdPartyPrivateAssetImportPublicDocsClosed,
      thirdPartyPrivateAssetImportEndpointGuardRedacted:
        followupEvidence.summary.thirdPartyPrivateAssetImportEndpointGuardRedacted,
      mainSiteGalleryTaskCardReady:
        followupEvidence.summary.mainSiteGalleryTaskCardReady,
      mainSiteGalleryTaskCardStable:
        followupEvidence.summary.mainSiteGalleryTaskCardStable,
      mainSiteGalleryTaskCardDetailReady:
        followupEvidence.summary.mainSiteGalleryTaskCardDetailReady,
      mainSiteGalleryTaskCardNoSideEffects:
        followupEvidence.summary.mainSiteGalleryTaskCardNoSideEffects,
      mainSiteGalleryTaskCardRedacted:
        followupEvidence.summary.mainSiteGalleryTaskCardRedacted,
      followupDryRunRedacted: followupEvidence.summary.followupDryRunRedacted,
      collectionScopedSingleImportRequiresAssetLibrary:
        scopeGuardEvidence.summary.singleCollectionRequiresAssetLibrary,
      collectionScopedBatchImportRequiresAssetLibrary:
        scopeGuardEvidence.summary.batchCollectionRequiresAssetLibrary,
      operatorManifestDryRunReady:
        operatorManifestEvidence.checks.operatorManifestDryRunReady,
      operatorManifestRequiredInputsTracked:
        operatorManifestEvidence.checks.operatorManifestRequiredInputsTracked,
      operatorManifestExecuteReadyMatchesInputs:
        operatorManifestEvidence.checks.operatorManifestExecuteReadyMatchesInputs,
      operatorManifestRequiresManualReview:
        operatorManifestEvidence.checks.operatorManifestRequiresManualReview,
      operatorManifestRunbookReady:
        operatorManifestEvidence.checks.operatorManifestRunbookReady,
      operatorManifestAuditCountsOnly:
        operatorManifestEvidence.checks.operatorManifestAuditCountsOnly,
      operatorManifestRedacted:
        operatorManifestEvidence.checks.operatorManifestRedacted,
      cleanupManifestDryRunReady:
        cleanupManifestEvidence.checks.cleanupManifestDryRunReady,
      cleanupManifestRequiresPostExecuteReceipt:
        cleanupManifestEvidence.checks.cleanupManifestRequiresPostExecuteReceipt,
      cleanupManifestReadyMatchesInputs:
        cleanupManifestEvidence.checks.cleanupManifestReadyMatchesInputs,
      cleanupManifestNoAutoCleanup:
        cleanupManifestEvidence.checks.cleanupManifestNoAutoCleanup,
      cleanupManifestRunbookReady:
        cleanupManifestEvidence.checks.cleanupManifestRunbookReady,
      cleanupManifestAuditCountsOnly:
        cleanupManifestEvidence.checks.cleanupManifestAuditCountsOnly,
      cleanupManifestRedacted:
        cleanupManifestEvidence.checks.cleanupManifestRedacted,
      operatorHandoffDryRunReady:
        operatorHandoffEvidence.checks.operatorHandoffDryRunReady,
      operatorHandoffExecuteStateConsistent:
        operatorHandoffEvidence.checks.operatorHandoffExecuteStateConsistent,
      operatorHandoffCleanupStateConsistent:
        operatorHandoffEvidence.checks.operatorHandoffCleanupStateConsistent,
      operatorHandoffTaskCardSummarySafe:
        operatorHandoffEvidence.checks.operatorHandoffTaskCardSummarySafe,
      operatorHandoffValidationSummarySafe:
        operatorHandoffEvidence.checks.operatorHandoffValidationSummarySafe,
      operatorHandoffNoSideEffects:
        operatorHandoffEvidence.checks.operatorHandoffNoSideEffects,
      operatorHandoffRedacted:
        operatorHandoffEvidence.checks.operatorHandoffRedacted,
      operatorHandoffContractDriftGuardReady:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffContractDriftGuardReady,
      operatorHandoffDetailStatusAllowed:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDetailStatusAllowed,
      operatorHandoffDetailOnly:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDetailOnly,
      operatorHandoffDoesNotMutateTaskCardStatus:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDoesNotMutateTaskCardStatus,
      operatorHandoffDoesNotCreateTaskCard:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDoesNotCreateTaskCard,
      operatorHandoffNoCallbackOrPublicContractChange:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffNoCallbackOrPublicContractChange,
      operatorHandoffContractGuardRedacted:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffContractGuardRedacted,
      operatorReadinessRollupDryRunReady:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupDryRunReady,
      operatorReadinessRollupInputContractsOk:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupInputContractsOk,
      operatorReadinessRollupExecuteGateConsistent:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupExecuteGateConsistent,
      operatorReadinessRollupCleanupGateConsistent:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupCleanupGateConsistent,
      operatorReadinessRollupNoSideEffects:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupNoSideEffects,
      operatorReadinessRollupRedacted:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupRedacted,
      operatorReadinessContractDriftGuardReady:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessContractDriftGuardReady,
      operatorReadinessRollupContainsNoCommandMaterial:
        operatorReadinessContractGuardEvidence.checks
          .operatorReadinessRollupContainsNoCommandMaterial,
      operatorReadinessPrivateRunbookOnly:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessPrivateRunbookOnly,
      operatorReadinessCommandTemplateRedacted:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessCommandTemplateRedacted,
      operatorReadinessPublicDocsDoNotExposeAssetImports:
        operatorReadinessContractGuardEvidence.checks
          .operatorReadinessPublicDocsDoNotExposeAssetImports,
      operatorReadinessContractGuardRedacted:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessContractGuardRedacted,
      operatorPrivateRunbookReceiptPackageReady:
        privateRunbookReceiptPackageEvidence.checks.operatorPrivateRunbookReceiptPackageReady,
      operatorPrivateRunbookSeparatedFromSharedReceipt:
        privateRunbookReceiptPackageEvidence.checks
          .operatorPrivateRunbookSeparatedFromSharedReceipt,
      operatorPrivateRunbookPreExecuteNoRawMaterial:
        privateRunbookReceiptPackageEvidence.checks
          .operatorPrivateRunbookPreExecuteNoRawMaterial,
      operatorSharedValidationReceiptSafe:
        privateRunbookReceiptPackageEvidence.checks.operatorSharedValidationReceiptSafe,
      operatorReceiptPackageNoSideEffects:
        privateRunbookReceiptPackageEvidence.checks.operatorReceiptPackageNoSideEffects,
      operatorReceiptPackageRedacted:
        privateRunbookReceiptPackageEvidence.checks.operatorReceiptPackageRedacted,
      operatorReceiptPackageDisplayContractGuardReady:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDisplayContractGuardReady,
      operatorReceiptPackageDetailOnly:
        receiptPackageDisplayContractGuardEvidence.checks.operatorReceiptPackageDetailOnly,
      operatorReceiptPackageDoesNotCreateTaskCard:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDoesNotCreateTaskCard,
      operatorReceiptPackageDoesNotMutateStatusEnum:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDoesNotMutateStatusEnum,
      operatorReceiptPackageSharedReceiptExcludesPrivateMaterial:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageSharedReceiptExcludesPrivateMaterial,
      operatorReceiptPackageDisplayContractGuardRedacted:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDisplayContractGuardRedacted,
      operatorValidationRollupDryRunReady:
        operatorValidationRollupEvidence.checks.operatorValidationRollupDryRunReady,
      operatorValidationRollupInputContractsOk:
        operatorValidationRollupEvidence.checks.operatorValidationRollupInputContractsOk,
      operatorValidationRollupSingleGateReady:
        operatorValidationRollupEvidence.checks.operatorValidationRollupSingleGateReady,
      operatorValidationRollupLiveExecuteStillRequiresPrivateInputs:
        operatorValidationRollupEvidence.checks
          .operatorValidationRollupLiveExecuteStillRequiresPrivateInputs,
      operatorValidationRollupNoSideEffects:
        operatorValidationRollupEvidence.checks.operatorValidationRollupNoSideEffects,
      operatorValidationRollupRedacted:
        operatorValidationRollupEvidence.checks.operatorValidationRollupRedacted,
      operatorValidationRollupDisplayContractGuardReady:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayContractGuardReady,
      operatorValidationRollupDisplayOnly:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayOnly,
      operatorValidationRollupDoesNotTriggerLiveExecute:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDoesNotTriggerLiveExecute,
      operatorValidationRollupDoesNotCreateOrUpdateTaskCard:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDoesNotCreateOrUpdateTaskCard,
      operatorValidationRollupSharedReceiptExcludesPrivateMaterial:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupSharedReceiptExcludesPrivateMaterial,
      operatorValidationRollupDisplayContractGuardRedacted:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayContractGuardRedacted,
      operatorValidationReleaseSummaryReady:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryReady,
      operatorValidationReleaseSummaryPrivateInputsOnly:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryPrivateInputsOnly,
      operatorValidationReleaseSummaryContractsStable:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryContractsStable,
      operatorValidationReleaseSummaryDoesNotCreateOrMutateTaskCard:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryDoesNotCreateOrMutateTaskCard,
      operatorValidationReleaseSummaryDoesNotChangeSharedReceipt:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryDoesNotChangeSharedReceipt,
      operatorValidationReleaseSummaryRedacted:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryRedacted,
      operatorValidationReleaseSummaryFieldLedgerGuardReady:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerGuardReady,
      operatorValidationReleaseSummaryFieldLedgerOnly:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerOnly,
      operatorValidationReleaseSummaryFieldLedgerNoLiveExecute:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerNoLiveExecute,
      operatorValidationReleaseSummaryFieldLedgerTaskCardStable:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerTaskCardStable,
      operatorValidationReleaseSummaryFieldLedgerSharedReceiptStable:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerSharedReceiptStable,
      operatorValidationReleaseSummaryFieldLedgerGuardRedacted:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerGuardRedacted,
      operatorReadyHandoffReceiptReady:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffReceiptReady,
      operatorReadyHandoffRequiresOnlyPrivateValues:
        operatorReadyHandoffReceiptEvidence.checks
          .operatorReadyHandoffRequiresOnlyPrivateValues,
      operatorReadyHandoffContractsStable:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffContractsStable,
      operatorReadyHandoffNoTaskCardOrSharedReceiptMutation:
        operatorReadyHandoffReceiptEvidence.checks
          .operatorReadyHandoffNoTaskCardOrSharedReceiptMutation,
      operatorReadyHandoffNoExternalEffects:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffNoExternalEffects,
      operatorReadyHandoffReceiptRedacted:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffReceiptRedacted,
      finalValidationReadinessBundleReady:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationReadinessBundleReady,
      finalValidationBundleSingleGateReady:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleSingleGateReady,
      finalValidationBundleRequiresOnlyPrivateValues:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleRequiresOnlyPrivateValues,
      finalValidationBundleContractsStable:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleContractsStable,
      finalValidationBundleNoExternalEffects:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleNoExternalEffects,
      finalValidationBundleRedacted:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleRedacted,
      noNetworkOrProductionWrite: true,
      liveWriteGateDocumented: true,
    },
  };
  return attachMachineSummary(report);
}

async function uploadLiveFixtures(args, fixtures) {
  const formData = new FormData();
  formData.append('files', new Blob([fixtures.pngBuffer], { type: 'image/png' }), 'spring-dress.png');
  formData.append('files', new Blob([fixtures.zipBuffer], { type: 'application/zip' }), 'fashion-gallery.zip');
  const response = await requestForm(args, '/api/v3/local-document-uploads', formData);
  const files = Array.isArray(response?.files) ? response.files : [];
  const pngFile = files[0] || null;
  const zipFile = files[1] || null;
  if (!pngFile?.object_key || !zipFile?.object_key) {
    throw new Error('local upload did not return saved references for both PNG and ZIP fixtures');
  }
  return {
    pngFile,
    zipFile,
    summary: {
      savedFileCount: files.length,
      pngSaved: Boolean(pngFile.object_key),
      zipSaved: Boolean(zipFile.object_key),
      pngSize: Number(pngFile.size || fixtures.pngBytes) || 0,
      zipSize: Number(zipFile.size || fixtures.zipBytes) || 0,
      pngContentType: pngFile.content_type || 'image/png',
      zipContentType: zipFile.content_type || 'application/zip',
    },
  };
}

function buildLiveBatchPayload(args, fixtures, upload) {
  const approvalHash = sha256(Buffer.from(args.approvalId));
  const batch = buildFashionDesignImageAssetImportBatchPayload({
    datasetId: args.datasetId,
    assetLibraryId: args.assetLibraryId,
    title: `服装设计资产 smoke ${fixtures.runId.slice(0, 12)}`,
    imageUrlsText: upload.pngFile.object_key,
    packages: [
      {
        externalId: `zip-pack-${fixtures.runId}`,
        title: '服装设计 ZIP 包',
        objectKey: upload.zipFile.object_key,
        contentType: upload.zipFile.content_type || 'application/zip',
        metadata: {
          smoke_package: true,
          approval_hash: approvalHash,
        },
      },
    ],
    profilePayload: fixtureProfilePayload(),
    metadata: {
      smoke: 'fashion_design_asset_import_live',
      run_id: fixtures.runId,
      approval_hash: approvalHash,
    },
  });
  assert.deepEqual(batch.errors, []);
  assert.equal(batch.payload.dataset_id, args.datasetId);
  assert.equal(batch.payload.asset_library_id, args.assetLibraryId);
  assert.equal(batch.payload.assets.length, 1);
  assert.equal(batch.payload.packages.length, 1);
  return batch;
}

async function executeLive(args) {
  const fixtures = await writeFixtureFiles(args);
  const operatorManifestEvidence = buildOperatorManifestEvidence(args);
  const upload = await uploadLiveFixtures(args, fixtures);
  const membershipResponse = await requestJson(
    args,
    `/api/v3/asset-libraries/${encodeURIComponent(args.assetLibraryId)}/datasets/${encodeURIComponent(args.datasetId)}`,
    {
      method: 'POST',
      body: { role: 'source', priority: 0 },
    },
  );
  const membershipDatasetId = String(
    membershipResponse?.membership?.dataset_id
      || membershipResponse?.membership?.datasetId
      || '',
  ).trim();
  const membershipAssetLibraryId = String(
    membershipResponse?.membership?.asset_library_id
      || membershipResponse?.membership?.assetLibraryId
      || membershipResponse?.asset_library?.id
      || membershipResponse?.assetLibrary?.id
      || '',
  ).trim();
  const membershipMatchesRequest = membershipDatasetId === args.datasetId
    && membershipAssetLibraryId === args.assetLibraryId;
  const batch = buildLiveBatchPayload(args, fixtures, upload);
  const importResponse = await requestJson(args, '/api/v3/asset-imports/fashion-design-images/batch', {
    method: 'POST',
    body: batch.payload,
  });
  const normalizedImport = normalizeFashionDesignImageAssetImportBatchResponse(importResponse);
  const scopeResponse = await requestJson(
    args,
    `/api/v3/asset-libraries/${encodeURIComponent(args.assetLibraryId)}/scope-summary`,
  );
  const normalizedScope = normalizeAssetLibraryScope(scopeResponse);
  const designHits = filterAssetProfileHints(normalizedScope, {
    query: '泡泡袖',
    assetKind: 'image',
    limit: 50,
  });
  const cleanupManifestEvidence = buildPostExecuteCleanupManifestEvidence(
    args,
    {
      summary: {
        normalizedBatchAssetCount: normalizedImport.assetCount,
        pendingParseRunCount: normalizedImport.items
          .filter((item) => item.parseRunStatus === 'pending').length,
      },
    },
    true,
  );
  const operatorHandoffEvidence = buildOperatorHandoffEvidence(
    args,
    operatorManifestEvidence,
    cleanupManifestEvidence,
  );
  const operatorHandoffContractGuardEvidence = buildOperatorHandoffContractGuardEvidence(
    args,
    operatorHandoffEvidence,
  );
  const operatorReadinessRollupEvidence = buildOperatorReadinessRollupEvidence(
    args,
    operatorManifestEvidence,
    cleanupManifestEvidence,
    operatorHandoffContractGuardEvidence,
  );
  const operatorReadinessContractGuardEvidence = buildOperatorReadinessContractGuardEvidence(
    args,
    operatorReadinessRollupEvidence,
  );
  const privateRunbookReceiptPackageEvidence = buildPrivateRunbookReceiptPackageEvidence(
    args,
    operatorReadinessContractGuardEvidence,
  );
  const receiptPackageDisplayContractGuardEvidence =
    buildReceiptPackageDisplayContractGuardEvidence(
      args,
      privateRunbookReceiptPackageEvidence,
    );
  const operatorValidationRollupEvidence = buildOperatorValidationRollupEvidence(
    args,
    operatorReadinessContractGuardEvidence,
    privateRunbookReceiptPackageEvidence,
    receiptPackageDisplayContractGuardEvidence,
  );
  const operatorValidationRollupDisplayContractGuardEvidence =
    buildOperatorValidationRollupDisplayContractGuardEvidence(
      args,
      operatorValidationRollupEvidence,
    );
  const operatorValidationReleaseSummaryEvidence =
    buildOperatorValidationReleaseSummaryEvidence(
      args,
      operatorManifestEvidence,
      operatorValidationRollupDisplayContractGuardEvidence,
    );
  const operatorValidationReleaseSummaryFieldLedgerGuardEvidence =
    buildOperatorValidationReleaseSummaryFieldLedgerGuardEvidence(
      args,
      operatorValidationReleaseSummaryEvidence,
    );
  const operatorReadyHandoffReceiptEvidence =
    buildOperatorReadyHandoffReceiptEvidence(
      args,
      operatorManifestEvidence,
      operatorValidationReleaseSummaryEvidence,
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence,
    );
  const finalValidationReadinessBundleEvidence =
    buildFinalValidationReadinessBundleEvidence(
      args,
      operatorReadyHandoffReceiptEvidence,
    );

  const report = {
    smoke: 'fashion-design-asset-import',
    mode: 'execute',
    ok: true,
    execute: true,
    generated_at: new Date().toISOString(),
    target: {
      base: redactedUrlSummary(args.baseUrl),
      datasetIdPresent: true,
      assetLibraryIdPresent: true,
      authSupplied: true,
      approvalSupplied: true,
      localThreadIdSupplied: Boolean(args.localThreadId),
    },
    fixture_summary: {
      png_fixture_available: fixtures.pngBytes > 0,
      zip_fixture_available: fixtures.zipBytes > 0,
      png_bytes: fixtures.pngBytes,
      zip_bytes: fixtures.zipBytes,
      png_sha256: fixtures.pngSha256,
      zip_sha256: fixtures.zipSha256,
    },
    upload_summary: upload.summary,
    membership_summary: {
      responsePresent: Boolean(membershipResponse),
      datasetMatched: membershipDatasetId === args.datasetId,
      assetLibraryMatched: membershipAssetLibraryId === args.assetLibraryId,
    },
    import_summary: {
      accepted: normalizedImport.accepted,
      assetCount: normalizedImport.assetCount,
      packageCount: normalizedImport.packageCount,
      expandedAssetCount: normalizedImport.expandedAssetCount,
      pendingParseRunCount: normalizedImport.items
        .filter((item) => item.parseRunStatus === 'pending').length,
      profileKindCount: normalizedImport.items
        .filter((item) => item.profileKind === 'fashion_design_image_v1').length,
    },
    scope_summary: {
      scopeNormalized: true,
      scopeHintCount: normalizedScope.assetProfileHintCount,
      assetKinds: assetProfileKindOptions(normalizedScope),
      parseStatusCounts: normalizedScope.assetParseStatusCounts,
      parseRunCount: normalizedScope.assetParseRunCount,
      designQueryFilterHit: designHits.length > 0,
    },
    operator_manifest: operatorManifestEvidence.manifest,
    operator_manifest_summary: operatorManifestEvidence.summary,
    cleanup_manifest: cleanupManifestEvidence.manifest,
    cleanup_manifest_summary: cleanupManifestEvidence.summary,
    operator_handoff: operatorHandoffEvidence.handoff,
    operator_handoff_summary: operatorHandoffEvidence.summary,
    operator_handoff_contract_guard: operatorHandoffContractGuardEvidence.guard,
    operator_handoff_contract_guard_summary: operatorHandoffContractGuardEvidence.summary,
    operator_readiness_rollup: operatorReadinessRollupEvidence.rollup,
    operator_readiness_rollup_summary: operatorReadinessRollupEvidence.summary,
    operator_readiness_contract_guard: operatorReadinessContractGuardEvidence.guard,
    operator_readiness_contract_guard_summary: operatorReadinessContractGuardEvidence.summary,
    private_runbook_receipt_package: privateRunbookReceiptPackageEvidence.package,
    private_runbook_receipt_package_summary: privateRunbookReceiptPackageEvidence.summary,
    receipt_package_display_contract_guard: receiptPackageDisplayContractGuardEvidence.guard,
    receipt_package_display_contract_guard_summary:
      receiptPackageDisplayContractGuardEvidence.summary,
    operator_validation_rollup: operatorValidationRollupEvidence.rollup,
    operator_validation_rollup_summary: operatorValidationRollupEvidence.summary,
    operator_validation_rollup_display_contract_guard:
      operatorValidationRollupDisplayContractGuardEvidence.guard,
    operator_validation_rollup_display_contract_guard_summary:
      operatorValidationRollupDisplayContractGuardEvidence.summary,
    operator_validation_release_summary:
      operatorValidationReleaseSummaryEvidence.releaseSummary,
    operator_validation_release_summary_summary:
      operatorValidationReleaseSummaryEvidence.summary,
    operator_validation_release_summary_field_ledger_guard:
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence.guard,
    operator_validation_release_summary_field_ledger_guard_summary:
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence.summary,
    operator_ready_handoff_receipt: operatorReadyHandoffReceiptEvidence.receipt,
    operator_ready_handoff_receipt_summary:
      operatorReadyHandoffReceiptEvidence.summary,
    final_validation_readiness_bundle:
      finalValidationReadinessBundleEvidence.bundle,
    final_validation_readiness_bundle_summary:
      finalValidationReadinessBundleEvidence.summary,
    redaction: {
      rawBaseUrlIncluded: false,
      rawLocalPathIncluded: false,
      uploadReferenceIncluded: false,
      sessionSecretIncluded: false,
      apiTokenIncluded: false,
      providerPayloadsIncluded: false,
    },
    checks: {
      liveWriteApprovalSatisfied: args.ackLiveWrite && Boolean(args.approvalId),
      fixtureUploadAccepted: upload.summary.savedFileCount >= 2
        && upload.summary.pngSaved
        && upload.summary.zipSaved,
      datasetAssetLibraryMembershipUpserted: membershipMatchesRequest,
      batchImportAccepted: normalizedImport.accepted,
      directAndPackageAssetsImported: normalizedImport.assetCount >= 3
        && normalizedImport.packageCount === 1
        && normalizedImport.expandedAssetCount >= 2,
      parseRunsCreated: normalizedImport.items.length === normalizedImport.assetCount
        && normalizedImport.items.every((item) => Boolean(item.parseRunStatus)),
      fashionProfilesCreated: normalizedImport.items
        .some((item) => item.profileKind === 'fashion_design_image_v1'),
      scopeProfileHintsUsable: designHits.length > 0,
      scopeParseStatusVisible: normalizedScope.assetParseRunCount > 0
        && Object.keys(normalizedScope.assetParseStatusCounts || {}).length > 0,
      operatorManifestDryRunReady:
        operatorManifestEvidence.checks.operatorManifestDryRunReady,
      operatorManifestRequiredInputsTracked:
        operatorManifestEvidence.checks.operatorManifestRequiredInputsTracked,
      operatorManifestExecuteReadyMatchesInputs:
        operatorManifestEvidence.checks.operatorManifestExecuteReadyMatchesInputs,
      operatorManifestRequiresManualReview:
        operatorManifestEvidence.checks.operatorManifestRequiresManualReview,
      operatorManifestRunbookReady:
        operatorManifestEvidence.checks.operatorManifestRunbookReady,
      operatorManifestAuditCountsOnly:
        operatorManifestEvidence.checks.operatorManifestAuditCountsOnly,
      operatorManifestRedacted:
        operatorManifestEvidence.checks.operatorManifestRedacted,
      cleanupManifestDryRunReady:
        cleanupManifestEvidence.checks.cleanupManifestDryRunReady,
      cleanupManifestRequiresPostExecuteReceipt:
        cleanupManifestEvidence.checks.cleanupManifestRequiresPostExecuteReceipt,
      cleanupManifestReadyMatchesInputs:
        cleanupManifestEvidence.checks.cleanupManifestReadyMatchesInputs,
      cleanupManifestNoAutoCleanup:
        cleanupManifestEvidence.checks.cleanupManifestNoAutoCleanup,
      cleanupManifestRunbookReady:
        cleanupManifestEvidence.checks.cleanupManifestRunbookReady,
      cleanupManifestAuditCountsOnly:
        cleanupManifestEvidence.checks.cleanupManifestAuditCountsOnly,
      cleanupManifestRedacted:
        cleanupManifestEvidence.checks.cleanupManifestRedacted,
      operatorHandoffDryRunReady:
        operatorHandoffEvidence.checks.operatorHandoffDryRunReady,
      operatorHandoffExecuteStateConsistent:
        operatorHandoffEvidence.checks.operatorHandoffExecuteStateConsistent,
      operatorHandoffCleanupStateConsistent:
        operatorHandoffEvidence.checks.operatorHandoffCleanupStateConsistent,
      operatorHandoffTaskCardSummarySafe:
        operatorHandoffEvidence.checks.operatorHandoffTaskCardSummarySafe,
      operatorHandoffValidationSummarySafe:
        operatorHandoffEvidence.checks.operatorHandoffValidationSummarySafe,
      operatorHandoffNoSideEffects:
        operatorHandoffEvidence.checks.operatorHandoffNoSideEffects,
      operatorHandoffRedacted:
        operatorHandoffEvidence.checks.operatorHandoffRedacted,
      operatorHandoffContractDriftGuardReady:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffContractDriftGuardReady,
      operatorHandoffDetailStatusAllowed:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDetailStatusAllowed,
      operatorHandoffDetailOnly:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDetailOnly,
      operatorHandoffDoesNotMutateTaskCardStatus:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDoesNotMutateTaskCardStatus,
      operatorHandoffDoesNotCreateTaskCard:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDoesNotCreateTaskCard,
      operatorHandoffNoCallbackOrPublicContractChange:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffNoCallbackOrPublicContractChange,
      operatorHandoffContractGuardRedacted:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffContractGuardRedacted,
      operatorReadinessRollupDryRunReady:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupDryRunReady,
      operatorReadinessRollupInputContractsOk:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupInputContractsOk,
      operatorReadinessRollupExecuteGateConsistent:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupExecuteGateConsistent,
      operatorReadinessRollupCleanupGateConsistent:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupCleanupGateConsistent,
      operatorReadinessRollupNoSideEffects:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupNoSideEffects,
      operatorReadinessRollupRedacted:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupRedacted,
      operatorReadinessContractDriftGuardReady:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessContractDriftGuardReady,
      operatorReadinessRollupContainsNoCommandMaterial:
        operatorReadinessContractGuardEvidence.checks
          .operatorReadinessRollupContainsNoCommandMaterial,
      operatorReadinessPrivateRunbookOnly:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessPrivateRunbookOnly,
      operatorReadinessCommandTemplateRedacted:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessCommandTemplateRedacted,
      operatorReadinessPublicDocsDoNotExposeAssetImports:
        operatorReadinessContractGuardEvidence.checks
          .operatorReadinessPublicDocsDoNotExposeAssetImports,
      operatorReadinessContractGuardRedacted:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessContractGuardRedacted,
      operatorPrivateRunbookReceiptPackageReady:
        privateRunbookReceiptPackageEvidence.checks.operatorPrivateRunbookReceiptPackageReady,
      operatorPrivateRunbookSeparatedFromSharedReceipt:
        privateRunbookReceiptPackageEvidence.checks
          .operatorPrivateRunbookSeparatedFromSharedReceipt,
      operatorPrivateRunbookPreExecuteNoRawMaterial:
        privateRunbookReceiptPackageEvidence.checks
          .operatorPrivateRunbookPreExecuteNoRawMaterial,
      operatorSharedValidationReceiptSafe:
        privateRunbookReceiptPackageEvidence.checks.operatorSharedValidationReceiptSafe,
      operatorReceiptPackageNoSideEffects:
        privateRunbookReceiptPackageEvidence.checks.operatorReceiptPackageNoSideEffects,
      operatorReceiptPackageRedacted:
        privateRunbookReceiptPackageEvidence.checks.operatorReceiptPackageRedacted,
      operatorReceiptPackageDisplayContractGuardReady:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDisplayContractGuardReady,
      operatorReceiptPackageDetailOnly:
        receiptPackageDisplayContractGuardEvidence.checks.operatorReceiptPackageDetailOnly,
      operatorReceiptPackageDoesNotCreateTaskCard:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDoesNotCreateTaskCard,
      operatorReceiptPackageDoesNotMutateStatusEnum:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDoesNotMutateStatusEnum,
      operatorReceiptPackageSharedReceiptExcludesPrivateMaterial:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageSharedReceiptExcludesPrivateMaterial,
      operatorReceiptPackageDisplayContractGuardRedacted:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDisplayContractGuardRedacted,
      operatorValidationRollupDryRunReady:
        operatorValidationRollupEvidence.checks.operatorValidationRollupDryRunReady,
      operatorValidationRollupInputContractsOk:
        operatorValidationRollupEvidence.checks.operatorValidationRollupInputContractsOk,
      operatorValidationRollupSingleGateReady:
        operatorValidationRollupEvidence.checks.operatorValidationRollupSingleGateReady,
      operatorValidationRollupLiveExecuteStillRequiresPrivateInputs:
        operatorValidationRollupEvidence.checks
          .operatorValidationRollupLiveExecuteStillRequiresPrivateInputs,
      operatorValidationRollupNoSideEffects:
        operatorValidationRollupEvidence.checks.operatorValidationRollupNoSideEffects,
      operatorValidationRollupRedacted:
        operatorValidationRollupEvidence.checks.operatorValidationRollupRedacted,
      operatorValidationRollupDisplayContractGuardReady:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayContractGuardReady,
      operatorValidationRollupDisplayOnly:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayOnly,
      operatorValidationRollupDoesNotTriggerLiveExecute:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDoesNotTriggerLiveExecute,
      operatorValidationRollupDoesNotCreateOrUpdateTaskCard:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDoesNotCreateOrUpdateTaskCard,
      operatorValidationRollupSharedReceiptExcludesPrivateMaterial:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupSharedReceiptExcludesPrivateMaterial,
      operatorValidationRollupDisplayContractGuardRedacted:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayContractGuardRedacted,
      operatorValidationReleaseSummaryReady:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryReady,
      operatorValidationReleaseSummaryPrivateInputsOnly:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryPrivateInputsOnly,
      operatorValidationReleaseSummaryContractsStable:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryContractsStable,
      operatorValidationReleaseSummaryDoesNotCreateOrMutateTaskCard:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryDoesNotCreateOrMutateTaskCard,
      operatorValidationReleaseSummaryDoesNotChangeSharedReceipt:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryDoesNotChangeSharedReceipt,
      operatorValidationReleaseSummaryRedacted:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryRedacted,
      operatorValidationReleaseSummaryFieldLedgerGuardReady:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerGuardReady,
      operatorValidationReleaseSummaryFieldLedgerOnly:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerOnly,
      operatorValidationReleaseSummaryFieldLedgerNoLiveExecute:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerNoLiveExecute,
      operatorValidationReleaseSummaryFieldLedgerTaskCardStable:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerTaskCardStable,
      operatorValidationReleaseSummaryFieldLedgerSharedReceiptStable:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerSharedReceiptStable,
      operatorValidationReleaseSummaryFieldLedgerGuardRedacted:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerGuardRedacted,
      operatorReadyHandoffReceiptReady:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffReceiptReady,
      operatorReadyHandoffRequiresOnlyPrivateValues:
        operatorReadyHandoffReceiptEvidence.checks
          .operatorReadyHandoffRequiresOnlyPrivateValues,
      operatorReadyHandoffContractsStable:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffContractsStable,
      operatorReadyHandoffNoTaskCardOrSharedReceiptMutation:
        operatorReadyHandoffReceiptEvidence.checks
          .operatorReadyHandoffNoTaskCardOrSharedReceiptMutation,
      operatorReadyHandoffNoExternalEffects:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffNoExternalEffects,
      operatorReadyHandoffReceiptRedacted:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffReceiptRedacted,
      finalValidationReadinessBundleReady:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationReadinessBundleReady,
      finalValidationBundleSingleGateReady:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleSingleGateReady,
      finalValidationBundleRequiresOnlyPrivateValues:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleRequiresOnlyPrivateValues,
      finalValidationBundleContractsStable:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleContractsStable,
      finalValidationBundleNoExternalEffects:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleNoExternalEffects,
      finalValidationBundleRedacted:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleRedacted,
      noRawSensitiveMaterialInReceipt: true,
    },
  };
  return { report: attachMachineSummary(report), runId: fixtures.runId, suffix: 'execute' };
}

function assertReportSafe(report) {
  const serialized = JSON.stringify(report);
  assert.equal(/https?:\/\//i.test(serialized), false, 'report must not include source URLs');
  assert.equal(/object[_-]?key/i.test(serialized), false, 'report must not include object keys');
  assert.equal(/Bearer\s|Authorization|cookie|sk-[A-Za-z0-9_-]{8,}/i.test(serialized), false, 'report leaked auth material');
  assert.equal(serialized.includes(ROOT_DIR), false, 'report must not include absolute local paths');
}

function attachMachineSummary(report) {
  const mode = report.mode || (report.self_test ? 'self_test' : report.preflight ? 'preflight' : 'execute');
  const checks = report.checks || {};
  const ok = Object.values(checks).every(Boolean);
  report.ok = ok;
  report.summary = {
    ok,
    ready: ok && mode === 'execute',
    pending: ok && mode === 'preflight',
    failed: !ok,
    mode,
    checks,
  };
  return report;
}

function buildMarkdown(report) {
  return [
    '# Fashion Design Asset Import Smoke',
    '',
    `- ok: ${report.ok}`,
    `- ready: ${Boolean(report.summary?.ready)}`,
    `- pending: ${Boolean(report.summary?.pending)}`,
    `- failed: ${Boolean(report.summary?.failed)}`,
    `- mode: ${report.mode}`,
    `- self_test: ${Boolean(report.self_test)}`,
    `- preflight: ${Boolean(report.preflight)}`,
    `- execute: ${Boolean(report.execute)}`,
    `- png_fixture_available: ${report.fixture_summary.png_fixture_available}`,
    `- zip_fixture_available: ${report.fixture_summary.zip_fixture_available}`,
    `- batch_asset_count: ${report.import_summary.batchAssetCount ?? report.import_summary.assetCount ?? 0}`,
    `- batch_package_count: ${report.import_summary.batchPackageCount ?? report.import_summary.packageCount ?? 0}`,
    `- normalized_expanded_asset_count: ${report.response_summary?.normalizedExpandedAssetCount ?? report.import_summary.expandedAssetCount ?? 0}`,
    `- scope_hint_count: ${report.scope_summary.scopeHintCount}`,
    `- scope_parse_run_count: ${report.scope_summary.parseRunCount ?? report.planning_summary?.planningParseRunCount ?? 0}`,
    `- planning_parse_status_visible: ${Boolean(report.planning_summary?.staticPageEvidenceParseStatusVisible || report.checks?.scopeParseStatusVisible)}`,
    `- parse_queue_dry_run_ready: ${Boolean(report.followup_summary?.parseQueueDryRunReady)}`,
    `- parse_status_transition_dry_run_ready: ${Boolean(report.followup_summary?.parseStatusTransitionDryRunReady)}`,
    `- retrieval_evidence_dry_run_ready: ${Boolean(report.followup_summary?.retrievalEvidenceDryRunReady)}`,
    `- retrieval_evidence_idempotency_planned: ${Boolean(report.followup_summary?.retrievalEvidenceIdempotencyPlanned)}`,
    `- retrieval_evidence_adapter_dry_run_ready: ${Boolean(report.followup_summary?.retrievalEvidenceAdapterDryRunReady)}`,
    `- profile_evidence_write_order_constrained: ${Boolean(report.followup_summary?.profileEvidenceWriteOrderConstrained)}`,
    `- asset_profile_retrieval_storage_mapping_dry_run_ready: ${Boolean(report.followup_summary?.assetProfileRetrievalStorageMappingDryRunReady)}`,
    `- synthetic_document_chunk_rejected: ${Boolean(report.followup_summary?.syntheticDocumentChunkRejected)}`,
    `- asset_evidence_table_recommended: ${Boolean(report.followup_summary?.assetEvidenceTableRecommended)}`,
    `- asset_retrieval_evidence_migration_sketch_ready: ${Boolean(report.followup_summary?.assetRetrievalEvidenceMigrationSketchReady)}`,
    `- union_search_no_write_adapter_draft_ready: ${Boolean(report.followup_summary?.unionSearchNoWriteAdapterDraftReady)}`,
    `- asset_search_membership_guard_planned: ${Boolean(report.followup_summary?.assetSearchMembershipGuardPlanned)}`,
    `- union_search_merge_rank_fixture_ready: ${Boolean(report.followup_summary?.unionSearchMergeRankFixtureReady)}`,
    `- source_kind_regression_dry_run_ready: ${Boolean(report.followup_summary?.sourceKindRegressionDryRunReady)}`,
    `- union_search_merge_rank_no_raw_locator: ${Boolean(report.followup_summary?.unionSearchMergeRankNoRawLocator)}`,
    `- union_search_explain_debug_summary_ready: ${Boolean(report.followup_summary?.unionSearchExplainDebugSummaryReady)}`,
    `- model_facing_supply_compression_dry_run_ready: ${Boolean(report.followup_summary?.modelFacingSupplyCompressionDryRunReady)}`,
    `- model_facing_supply_compression_no_raw_locator: ${Boolean(report.followup_summary?.modelFacingSupplyCompressionNoRawLocator)}`,
    `- assistant_run_supply_budget_quality_gate_dry_run_ready: ${Boolean(report.followup_summary?.assistantRunSupplyBudgetQualityGateDryRunReady)}`,
    `- asset_document_supply_source_kind_dedupe_ready: ${Boolean(report.followup_summary?.assetDocumentSupplySourceKindDedupeReady)}`,
    `- insufficient_supply_continue_planned: ${Boolean(report.followup_summary?.insufficientSupplyContinuePlanned)}`,
    `- assistant_run_supply_gate_no_raw_locator: ${Boolean(report.followup_summary?.assistantRunSupplyGateNoRawLocator)}`,
    `- assistant_run_supply_progress_events_dry_run_ready: ${Boolean(report.followup_summary?.assistantRunSupplyProgressEventsDryRunReady)}`,
    `- assistant_run_progress_non_blocking: ${Boolean(report.followup_summary?.assistantRunProgressNonBlocking)}`,
    `- assistant_run_progress_insufficient_supply_continues: ${Boolean(report.followup_summary?.assistantRunProgressInsufficientSupplyContinues)}`,
    `- assistant_run_progress_parse_waiting_or_retry_visible: ${Boolean(report.followup_summary?.assistantRunProgressParseWaitingOrRetryVisible)}`,
    `- assistant_run_progress_no_raw_locator: ${Boolean(report.followup_summary?.assistantRunProgressNoRawLocator)}`,
    `- assistant_run_progress_contract_drift_guard_ready: ${Boolean(report.followup_summary?.assistantRunProgressContractDriftGuardReady)}`,
    `- assistant_run_progress_does_not_mutate_public_sse_fields: ${Boolean(report.followup_summary?.assistantRunProgressDoesNotMutatePublicSseFields)}`,
    `- assistant_run_progress_no_callback_or_write: ${Boolean(report.followup_summary?.assistantRunProgressNoCallbackOrWrite)}`,
    `- assistant_run_progress_failure_does_not_block_answer: ${Boolean(report.followup_summary?.assistantRunProgressFailureDoesNotBlockAnswer)}`,
    `- assistant_run_progress_task_card_detail_only: ${Boolean(report.followup_summary?.assistantRunProgressTaskCardDetailOnly)}`,
    `- parser_live_adapter_readiness_dry_run_ready: ${Boolean(report.followup_summary?.parserLiveAdapterReadinessDryRunReady)}`,
    `- parser_live_adapter_statuses: ${(report.followup_summary?.parserLiveAdapterStatuses || []).join(',') || 'none'}`,
    `- parser_live_adapter_no_network_or_model_call: ${Boolean(report.followup_summary?.parserLiveAdapterNoNetworkOrModelCall)}`,
    `- parser_live_adapter_requires_endpoint_only: ${Boolean(report.followup_summary?.parserLiveAdapterRequiresEndpointOnly)}`,
    `- parser_live_adapter_contracts_stable: ${Boolean(report.followup_summary?.parserLiveAdapterContractsStable)}`,
    `- parser_live_adapter_no_raw_payload: ${Boolean(report.followup_summary?.parserLiveAdapterNoRawPayload)}`,
    `- parser_live_adapter_redacted: ${Boolean(report.followup_summary?.parserLiveAdapterRedacted)}`,
    `- asset_retrieval_evidence_writer_readiness_ready: ${Boolean(report.followup_summary?.assetRetrievalEvidenceWriterReadinessReady)}`,
    `- asset_retrieval_evidence_writer_statuses: ${(report.followup_summary?.assetRetrievalEvidenceWriterStatuses || []).join(',') || 'none'}`,
    `- asset_retrieval_evidence_writer_field_ledger_ready: ${Boolean(report.followup_summary?.assetRetrievalEvidenceWriterFieldLedgerReady)}`,
    `- asset_retrieval_evidence_writer_idempotent: ${Boolean(report.followup_summary?.assetRetrievalEvidenceWriterIdempotent)}`,
    `- asset_retrieval_evidence_writer_membership_guard_ready: ${Boolean(report.followup_summary?.assetRetrievalEvidenceWriterMembershipGuardReady)}`,
    `- asset_retrieval_evidence_writer_rollback_audit_ready: ${Boolean(report.followup_summary?.assetRetrievalEvidenceWriterRollbackAuditReady)}`,
    `- asset_retrieval_evidence_writer_redacted: ${Boolean(report.followup_summary?.assetRetrievalEvidenceWriterRedacted)}`,
    `- third_party_private_asset_import_endpoint_guard_ready: ${Boolean(report.followup_summary?.thirdPartyPrivateAssetImportEndpointGuardReady)}`,
    `- third_party_private_asset_import_endpoint_status: ${report.followup_summary?.thirdPartyPrivateAssetImportEndpointStatus || 'unknown'}`,
    `- third_party_private_asset_import_default_private: ${Boolean(report.followup_summary?.thirdPartyPrivateAssetImportDefaultPrivate)}`,
    `- third_party_private_asset_import_structured_json_ready: ${Boolean(report.followup_summary?.thirdPartyPrivateAssetImportStructuredJsonReady)}`,
    `- third_party_private_asset_import_scope_guard_ready: ${Boolean(report.followup_summary?.thirdPartyPrivateAssetImportScopeGuardReady)}`,
    `- third_party_private_asset_import_public_docs_closed: ${Boolean(report.followup_summary?.thirdPartyPrivateAssetImportPublicDocsClosed)}`,
    `- third_party_private_asset_import_endpoint_guard_redacted: ${Boolean(report.followup_summary?.thirdPartyPrivateAssetImportEndpointGuardRedacted)}`,
    `- main_site_gallery_task_card_ready: ${Boolean(report.followup_summary?.mainSiteGalleryTaskCardReady)}`,
    `- main_site_gallery_task_card_status: ${report.followup_summary?.mainSiteGalleryTaskCardStatus || 'unknown'}`,
    `- main_site_gallery_task_card_stable: ${Boolean(report.followup_summary?.mainSiteGalleryTaskCardStable)}`,
    `- main_site_gallery_task_card_detail_ready: ${Boolean(report.followup_summary?.mainSiteGalleryTaskCardDetailReady)}`,
    `- main_site_gallery_task_card_no_side_effects: ${Boolean(report.followup_summary?.mainSiteGalleryTaskCardNoSideEffects)}`,
    `- main_site_gallery_task_card_redacted: ${Boolean(report.followup_summary?.mainSiteGalleryTaskCardRedacted)}`,
    `- operator_manifest_dry_run_ready: ${Boolean(report.operator_manifest_summary?.noWrite)}`,
    `- operator_manifest_execute_ready: ${Boolean(report.operator_manifest_summary?.executeReady)}`,
    `- operator_manifest_planned_step_count: ${report.operator_manifest_summary?.plannedStepCount ?? 0}`,
    `- operator_manifest_missing_required_inputs: ${(report.operator_manifest_summary?.missingRequiredInputs || []).join(',') || 'none'}`,
    `- operator_manifest_redacted: ${Boolean(report.summary?.checks?.operatorManifestRedacted || report.checks?.operatorManifestRedacted)}`,
    `- cleanup_manifest_dry_run_ready: ${Boolean(report.cleanup_manifest_summary?.noWrite && report.cleanup_manifest_summary?.noDelete)}`,
    `- cleanup_manifest_ready: ${Boolean(report.cleanup_manifest_summary?.cleanupManifestReady)}`,
    `- cleanup_manifest_action_count: ${report.cleanup_manifest_summary?.plannedCleanupActionCount ?? 0}`,
    `- cleanup_manifest_redacted: ${Boolean(report.summary?.checks?.cleanupManifestRedacted || report.checks?.cleanupManifestRedacted)}`,
    `- operator_handoff_status: ${report.operator_handoff_summary?.taskStatus || 'unknown'}`,
    `- operator_handoff_execute_state: ${report.operator_handoff_summary?.executeState || 'unknown'}`,
    `- operator_handoff_cleanup_state: ${report.operator_handoff_summary?.cleanupState || 'unknown'}`,
    `- operator_handoff_redacted: ${Boolean(report.summary?.checks?.operatorHandoffRedacted || report.checks?.operatorHandoffRedacted)}`,
    `- operator_handoff_contract_guard_ready: ${Boolean(report.summary?.checks?.operatorHandoffContractDriftGuardReady || report.checks?.operatorHandoffContractDriftGuardReady)}`,
    `- operator_handoff_detail_only: ${Boolean(report.operator_handoff_contract_guard_summary?.detailDisplayOnly)}`,
    `- operator_handoff_no_task_creation: ${Boolean(report.operator_handoff_contract_guard_summary?.noTaskCreation)}`,
    `- operator_handoff_no_callback: ${Boolean(report.operator_handoff_contract_guard_summary?.noCallback)}`,
    `- operator_readiness_status: ${report.operator_readiness_rollup_summary?.overallStatus || 'unknown'}`,
    `- operator_readiness_ready_for_execute: ${Boolean(report.operator_readiness_rollup_summary?.readyForOperatorExecute)}`,
    `- operator_readiness_ready_for_cleanup: ${Boolean(report.operator_readiness_rollup_summary?.readyForOperatorCleanupReview)}`,
    `- operator_readiness_redacted: ${Boolean(report.summary?.checks?.operatorReadinessRollupRedacted || report.checks?.operatorReadinessRollupRedacted)}`,
    `- operator_readiness_contract_guard_status: ${report.operator_readiness_contract_guard_summary?.driftStatus || 'unknown'}`,
    `- operator_readiness_private_runbook_only: ${Boolean(report.operator_readiness_contract_guard_summary?.privateRunbookOnly)}`,
    `- operator_readiness_command_template_redacted: ${Boolean(report.operator_readiness_contract_guard_summary?.commandTemplateRedacted)}`,
    `- operator_readiness_public_docs_closed: ${Boolean(report.operator_readiness_contract_guard_summary?.publicDocsAssetImportsClosed)}`,
    `- operator_readiness_contract_guard_redacted: ${Boolean(report.summary?.checks?.operatorReadinessContractGuardRedacted || report.checks?.operatorReadinessContractGuardRedacted)}`,
    `- private_runbook_receipt_package_status: ${report.private_runbook_receipt_package_summary?.packageStatus || 'unknown'}`,
    `- private_runbook_separate_from_shared_receipt: ${Boolean(report.private_runbook_receipt_package_summary?.privateRunbookSeparateFromSharedReceipt)}`,
    `- private_runbook_pre_execute_raw_material_excluded: ${Boolean(report.private_runbook_receipt_package_summary?.preExecuteRawMaterialExcluded)}`,
    `- shared_validation_receipt_safe: ${Boolean(report.private_runbook_receipt_package_summary?.sharedValidationReceiptSafe)}`,
    `- receipt_package_redacted: ${Boolean(report.summary?.checks?.operatorReceiptPackageRedacted || report.checks?.operatorReceiptPackageRedacted)}`,
    `- receipt_package_display_contract_guard_status: ${report.receipt_package_display_contract_guard_summary?.displayStatus || 'unknown'}`,
    `- receipt_package_detail_only: ${Boolean(report.receipt_package_display_contract_guard_summary?.detailDisplayOnly)}`,
    `- receipt_package_shared_receipt_excludes_private_material: ${Boolean(report.receipt_package_display_contract_guard_summary?.privateRunbookHiddenFromSharedReceipt && report.receipt_package_display_contract_guard_summary?.commandTemplateExcludedFromSharedReceipt)}`,
    `- receipt_package_display_contract_guard_redacted: ${Boolean(report.summary?.checks?.operatorReceiptPackageDisplayContractGuardRedacted || report.checks?.operatorReceiptPackageDisplayContractGuardRedacted)}`,
    `- operator_validation_rollup_status: ${report.operator_validation_rollup_summary?.rollupStatus || 'unknown'}`,
    `- operator_validation_single_gate_ready: ${Boolean(report.operator_validation_rollup_summary?.singleOperatorGateReady)}`,
    `- operator_validation_release_blocker_count: ${report.operator_validation_rollup_summary?.releaseBlockerCount ?? 'unknown'}`,
    `- operator_validation_live_execute_still_requires_private_inputs: ${Boolean(report.operator_validation_rollup_summary?.liveExecuteStillRequiresPrivateInputs)}`,
    `- operator_validation_rollup_redacted: ${Boolean(report.summary?.checks?.operatorValidationRollupRedacted || report.checks?.operatorValidationRollupRedacted)}`,
    `- operator_validation_rollup_display_guard_status: ${report.operator_validation_rollup_display_contract_guard_summary?.displayStatus || 'unknown'}`,
    `- operator_validation_rollup_display_only: ${Boolean(report.operator_validation_rollup_display_contract_guard_summary?.detailDisplayOnly)}`,
    `- operator_validation_rollup_does_not_trigger_live_execute: ${Boolean(report.summary?.checks?.operatorValidationRollupDoesNotTriggerLiveExecute || report.checks?.operatorValidationRollupDoesNotTriggerLiveExecute)}`,
    `- operator_validation_rollup_display_guard_redacted: ${Boolean(report.summary?.checks?.operatorValidationRollupDisplayContractGuardRedacted || report.checks?.operatorValidationRollupDisplayContractGuardRedacted)}`,
    `- operator_validation_release_summary_status: ${report.operator_validation_release_summary_summary?.releaseStatus || 'unknown'}`,
    `- operator_validation_release_private_inputs_only: ${Boolean(report.operator_validation_release_summary_summary?.privateInputsOnlyMissing)}`,
    `- operator_validation_release_contracts_stable: ${Boolean(report.operator_validation_release_summary_summary?.contractsStable)}`,
    `- operator_validation_release_summary_redacted: ${Boolean(report.summary?.checks?.operatorValidationReleaseSummaryRedacted || report.checks?.operatorValidationReleaseSummaryRedacted)}`,
    `- operator_validation_release_field_ledger_status: ${report.operator_validation_release_summary_field_ledger_guard_summary?.ledgerStatus || 'unknown'}`,
    `- operator_validation_release_field_ledger_only: ${Boolean(report.operator_validation_release_summary_field_ledger_guard_summary?.fieldLedgerOnly)}`,
    `- operator_validation_release_field_ledger_no_live_execute: ${Boolean(report.operator_validation_release_summary_field_ledger_guard_summary?.noLiveExecute)}`,
    `- operator_validation_release_field_ledger_redacted: ${Boolean(report.summary?.checks?.operatorValidationReleaseSummaryFieldLedgerGuardRedacted || report.checks?.operatorValidationReleaseSummaryFieldLedgerGuardRedacted)}`,
    `- operator_ready_handoff_status: ${report.operator_ready_handoff_receipt_summary?.handoffStatus || 'unknown'}`,
    `- operator_ready_handoff_requires_only_private_values: ${Boolean(report.operator_ready_handoff_receipt_summary?.handoffReadyForOperatorPrivateValues)}`,
    `- operator_ready_handoff_contracts_stable: ${Boolean(report.operator_ready_handoff_receipt_summary?.publicContractStable && report.operator_ready_handoff_receipt_summary?.taskCardContractStable && report.operator_ready_handoff_receipt_summary?.sharedValidationReceiptStable)}`,
    `- operator_ready_handoff_redacted: ${Boolean(report.summary?.checks?.operatorReadyHandoffReceiptRedacted || report.checks?.operatorReadyHandoffReceiptRedacted)}`,
    `- final_validation_bundle_status: ${report.final_validation_readiness_bundle_summary?.bundleStatus || 'unknown'}`,
    `- final_validation_bundle_ready: ${Boolean(report.final_validation_readiness_bundle_summary?.bundleReadyForOperatorReview)}`,
    `- final_validation_bundle_release_blockers: ${report.final_validation_readiness_bundle_summary?.releaseBlockerCount ?? 'unknown'}`,
    `- final_validation_bundle_redacted: ${Boolean(report.summary?.checks?.finalValidationBundleRedacted || report.checks?.finalValidationBundleRedacted)}`,
    '',
    '## Checks',
    '',
    ...Object.entries(report.summary?.checks || report.checks || {}).map(([key, value]) => `- ${key}: ${value}`),
    '',
  ].join('\n');
}

async function runSelfTest(args) {
  const fixtures = await writeFixtureFiles(args);
  const payloadEvidence = buildPayloadEvidence(fixtures);
  const responseEvidence = buildMockImportResponse(payloadEvidence);
  const scopeEvidence = buildScopeEvidence();
  const planningEvidence = buildPlanningEvidence(scopeEvidence);
  const followupEvidence = buildFollowupDryRunEvidence(responseEvidence, scopeEvidence);
  const scopeGuardEvidence = buildScopeGuardEvidence();
  const operatorManifestEvidence = buildOperatorManifestEvidence(args);
  const cleanupManifestEvidence = buildPostExecuteCleanupManifestEvidence(
    args,
    responseEvidence,
    false,
  );
  const operatorHandoffEvidence = buildOperatorHandoffEvidence(
    args,
    operatorManifestEvidence,
    cleanupManifestEvidence,
  );
  const operatorHandoffContractGuardEvidence = buildOperatorHandoffContractGuardEvidence(
    args,
    operatorHandoffEvidence,
  );
  const operatorReadinessRollupEvidence = buildOperatorReadinessRollupEvidence(
    args,
    operatorManifestEvidence,
    cleanupManifestEvidence,
    operatorHandoffContractGuardEvidence,
  );
  const operatorReadinessContractGuardEvidence = buildOperatorReadinessContractGuardEvidence(
    args,
    operatorReadinessRollupEvidence,
  );
  const privateRunbookReceiptPackageEvidence = buildPrivateRunbookReceiptPackageEvidence(
    args,
    operatorReadinessContractGuardEvidence,
  );
  const receiptPackageDisplayContractGuardEvidence =
    buildReceiptPackageDisplayContractGuardEvidence(
      args,
      privateRunbookReceiptPackageEvidence,
    );
  const operatorValidationRollupEvidence = buildOperatorValidationRollupEvidence(
    args,
    operatorReadinessContractGuardEvidence,
    privateRunbookReceiptPackageEvidence,
    receiptPackageDisplayContractGuardEvidence,
  );
  const operatorValidationRollupDisplayContractGuardEvidence =
    buildOperatorValidationRollupDisplayContractGuardEvidence(
      args,
      operatorValidationRollupEvidence,
    );
  const operatorValidationReleaseSummaryEvidence =
    buildOperatorValidationReleaseSummaryEvidence(
      args,
      operatorManifestEvidence,
      operatorValidationRollupDisplayContractGuardEvidence,
    );
  const operatorValidationReleaseSummaryFieldLedgerGuardEvidence =
    buildOperatorValidationReleaseSummaryFieldLedgerGuardEvidence(
      args,
      operatorValidationReleaseSummaryEvidence,
    );
  const operatorReadyHandoffReceiptEvidence =
    buildOperatorReadyHandoffReceiptEvidence(
      args,
      operatorManifestEvidence,
      operatorValidationReleaseSummaryEvidence,
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence,
    );
  const finalValidationReadinessBundleEvidence =
    buildFinalValidationReadinessBundleEvidence(
      args,
      operatorReadyHandoffReceiptEvidence,
    );

  const report = {
    smoke: 'fashion-design-asset-import',
    mode: 'self_test',
    ok: true,
    self_test: true,
    generated_at: new Date().toISOString(),
    fixture_summary: {
      png_fixture_available: fixtures.pngBytes > 0,
      zip_fixture_available: fixtures.zipBytes > 0,
      png_bytes: fixtures.pngBytes,
      zip_bytes: fixtures.zipBytes,
      png_sha256: fixtures.pngSha256,
      zip_sha256: fixtures.zipSha256,
      zip_expected_image_count: 2,
      zip_expected_skipped_file_count: 1,
    },
    import_summary: payloadEvidence.summary,
    response_summary: responseEvidence.summary,
    scope_summary: scopeEvidence.summary,
    planning_summary: planningEvidence.summary,
    followup_summary: followupEvidence.summary,
    scope_guard_summary: scopeGuardEvidence.summary,
    operator_manifest: operatorManifestEvidence.manifest,
    operator_manifest_summary: operatorManifestEvidence.summary,
    cleanup_manifest: cleanupManifestEvidence.manifest,
    cleanup_manifest_summary: cleanupManifestEvidence.summary,
    operator_handoff: operatorHandoffEvidence.handoff,
    operator_handoff_summary: operatorHandoffEvidence.summary,
    operator_handoff_contract_guard: operatorHandoffContractGuardEvidence.guard,
    operator_handoff_contract_guard_summary: operatorHandoffContractGuardEvidence.summary,
    operator_readiness_rollup: operatorReadinessRollupEvidence.rollup,
    operator_readiness_rollup_summary: operatorReadinessRollupEvidence.summary,
    operator_readiness_contract_guard: operatorReadinessContractGuardEvidence.guard,
    operator_readiness_contract_guard_summary: operatorReadinessContractGuardEvidence.summary,
    private_runbook_receipt_package: privateRunbookReceiptPackageEvidence.package,
    private_runbook_receipt_package_summary: privateRunbookReceiptPackageEvidence.summary,
    receipt_package_display_contract_guard: receiptPackageDisplayContractGuardEvidence.guard,
    receipt_package_display_contract_guard_summary:
      receiptPackageDisplayContractGuardEvidence.summary,
    operator_validation_rollup: operatorValidationRollupEvidence.rollup,
    operator_validation_rollup_summary: operatorValidationRollupEvidence.summary,
    operator_validation_rollup_display_contract_guard:
      operatorValidationRollupDisplayContractGuardEvidence.guard,
    operator_validation_rollup_display_contract_guard_summary:
      operatorValidationRollupDisplayContractGuardEvidence.summary,
    operator_validation_release_summary:
      operatorValidationReleaseSummaryEvidence.releaseSummary,
    operator_validation_release_summary_summary:
      operatorValidationReleaseSummaryEvidence.summary,
    operator_validation_release_summary_field_ledger_guard:
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence.guard,
    operator_validation_release_summary_field_ledger_guard_summary:
      operatorValidationReleaseSummaryFieldLedgerGuardEvidence.summary,
    operator_ready_handoff_receipt: operatorReadyHandoffReceiptEvidence.receipt,
    operator_ready_handoff_receipt_summary:
      operatorReadyHandoffReceiptEvidence.summary,
    final_validation_readiness_bundle:
      finalValidationReadinessBundleEvidence.bundle,
    final_validation_readiness_bundle_summary:
      finalValidationReadinessBundleEvidence.summary,
    checks: {
      realFixturesWritten: fixtures.pngBytes > 0 && fixtures.zipBytes > 0,
      singleImportPayloadValid: payloadEvidence.summary.singlePayloadValid,
      batchImportPayloadValid: payloadEvidence.summary.batchPayloadValid,
      payloadItemMetadataRedacted: payloadEvidence.summary.payloadItemMetadataRedacted,
      payloadRootMetadataRedacted: payloadEvidence.summary.payloadRootMetadataRedacted,
      payloadPackageMetadataRedacted: payloadEvidence.summary.payloadPackageMetadataRedacted,
      zipPackagePayloadValid: payloadEvidence.summary.batchPackageCount === 1
        && payloadEvidence.summary.packageContentType === 'application/zip',
      normalizedResponseIncludesParseRun: responseEvidence.summary.pendingParseRunCount === 4,
      normalizedImportResponseRedacted: responseEvidence.summary.normalizedImportResponseRedacted,
      scopeProfileHintsUsable: scopeEvidence.summary.scopeNormalized
        && scopeEvidence.summary.designQueryFilterHit,
      scopeAssetsRedacted: scopeEvidence.summary.scopeAssetLocatorsRedacted,
      scopeParseStatusVisible: scopeEvidence.summary.parseStatusPendingCount === 2
        && scopeEvidence.summary.parseRunCount === 3,
      staticPagePlanningSeesParseStatus: planningEvidence.summary.staticPageEvidenceParseStatusVisible
        && planningEvidence.summary.planningPendingAssetCount === 2,
      reportPlannerAstSeesParseStatus: planningEvidence.summary.reportPlannerAstParseStatusVisible
        && planningEvidence.summary.reportPlannerAssetMaterialsInsertedForPendingAssets,
      planningLayerRedacted: planningEvidence.summary.planningSensitiveFieldsRedacted,
      parseQueueDryRunReady: followupEvidence.summary.parseQueueDryRunReady
        && followupEvidence.summary.parseQueuePendingCount === 4,
      parseStatusTransitionDryRunReady:
        followupEvidence.summary.parseStatusTransitionDryRunReady,
      retrievalEvidenceDryRunReady: followupEvidence.summary.retrievalEvidenceDryRunReady
        && followupEvidence.summary.retrievalEvidencePreviewCount === 2,
      retrievalEvidenceTextMaterialized:
        followupEvidence.summary.retrievalEvidenceTextMaterialized,
      retrievalEvidenceIdempotencyPlanned:
        followupEvidence.summary.retrievalEvidenceIdempotencyPlanned,
      retrievalEvidenceAdapterDryRunReady:
        followupEvidence.summary.retrievalEvidenceAdapterDryRunReady,
      profileEvidenceWriteOrderConstrained:
        followupEvidence.summary.profileEvidenceWriteOrderConstrained,
      assetProfileRetrievalStorageMappingDryRunReady:
        followupEvidence.summary.assetProfileRetrievalStorageMappingDryRunReady,
      syntheticDocumentChunkRejected:
        followupEvidence.summary.syntheticDocumentChunkRejected,
      assetEvidenceTableRecommended:
        followupEvidence.summary.assetEvidenceTableRecommended,
      assetRetrievalEvidenceMigrationSketchReady:
        followupEvidence.summary.assetRetrievalEvidenceMigrationSketchReady,
      unionSearchNoWriteAdapterDraftReady:
        followupEvidence.summary.unionSearchNoWriteAdapterDraftReady,
      assetSearchMembershipGuardPlanned:
        followupEvidence.summary.assetSearchMembershipGuardPlanned,
      unionSearchMergeRankFixtureReady:
        followupEvidence.summary.unionSearchMergeRankFixtureReady,
      sourceKindRegressionDryRunReady:
        followupEvidence.summary.sourceKindRegressionDryRunReady,
      unionSearchMergeRankNoRawLocator:
        followupEvidence.summary.unionSearchMergeRankNoRawLocator,
      unionSearchExplainDebugSummaryReady:
        followupEvidence.summary.unionSearchExplainDebugSummaryReady,
      modelFacingSupplyCompressionDryRunReady:
        followupEvidence.summary.modelFacingSupplyCompressionDryRunReady,
      modelFacingSupplyCompressionNoRawLocator:
        followupEvidence.summary.modelFacingSupplyCompressionNoRawLocator,
      assistantRunSupplyBudgetQualityGateDryRunReady:
        followupEvidence.summary.assistantRunSupplyBudgetQualityGateDryRunReady,
      assetDocumentSupplySourceKindDedupeReady:
        followupEvidence.summary.assetDocumentSupplySourceKindDedupeReady,
      insufficientSupplyContinuePlanned:
        followupEvidence.summary.insufficientSupplyContinuePlanned,
      assistantRunSupplyGateNoRawLocator:
        followupEvidence.summary.assistantRunSupplyGateNoRawLocator,
      assistantRunSupplyProgressEventsDryRunReady:
        followupEvidence.summary.assistantRunSupplyProgressEventsDryRunReady,
      assistantRunProgressNonBlocking:
        followupEvidence.summary.assistantRunProgressNonBlocking,
      assistantRunProgressInsufficientSupplyContinues:
        followupEvidence.summary.assistantRunProgressInsufficientSupplyContinues,
      assistantRunProgressParseWaitingOrRetryVisible:
        followupEvidence.summary.assistantRunProgressParseWaitingOrRetryVisible,
      assistantRunProgressNoRawLocator:
        followupEvidence.summary.assistantRunProgressNoRawLocator,
      assistantRunProgressContractDriftGuardReady:
        followupEvidence.summary.assistantRunProgressContractDriftGuardReady,
      assistantRunProgressDoesNotMutatePublicSseFields:
        followupEvidence.summary.assistantRunProgressDoesNotMutatePublicSseFields,
      assistantRunProgressNoCallbackOrWrite:
        followupEvidence.summary.assistantRunProgressNoCallbackOrWrite,
      assistantRunProgressFailureDoesNotBlockAnswer:
        followupEvidence.summary.assistantRunProgressFailureDoesNotBlockAnswer,
      assistantRunProgressTaskCardDetailOnly:
        followupEvidence.summary.assistantRunProgressTaskCardDetailOnly,
      parserLiveAdapterReadinessDryRunReady:
        followupEvidence.summary.parserLiveAdapterReadinessDryRunReady,
      parserLiveAdapterNoNetworkOrModelCall:
        followupEvidence.summary.parserLiveAdapterNoNetworkOrModelCall,
      parserLiveAdapterRequiresEndpointOnly:
        followupEvidence.summary.parserLiveAdapterRequiresEndpointOnly,
      parserLiveAdapterContractsStable:
        followupEvidence.summary.parserLiveAdapterContractsStable,
      parserLiveAdapterNoRawPayload:
        followupEvidence.summary.parserLiveAdapterNoRawPayload,
      parserLiveAdapterRedacted:
        followupEvidence.summary.parserLiveAdapterRedacted,
      assetRetrievalEvidenceWriterReadinessReady:
        followupEvidence.summary.assetRetrievalEvidenceWriterReadinessReady,
      assetRetrievalEvidenceWriterFieldLedgerReady:
        followupEvidence.summary.assetRetrievalEvidenceWriterFieldLedgerReady,
      assetRetrievalEvidenceWriterIdempotent:
        followupEvidence.summary.assetRetrievalEvidenceWriterIdempotent,
      assetRetrievalEvidenceWriterMembershipGuardReady:
        followupEvidence.summary.assetRetrievalEvidenceWriterMembershipGuardReady,
      assetRetrievalEvidenceWriterRollbackAuditReady:
        followupEvidence.summary.assetRetrievalEvidenceWriterRollbackAuditReady,
      assetRetrievalEvidenceWriterRedacted:
        followupEvidence.summary.assetRetrievalEvidenceWriterRedacted,
      thirdPartyPrivateAssetImportEndpointGuardReady:
        followupEvidence.summary.thirdPartyPrivateAssetImportEndpointGuardReady,
      thirdPartyPrivateAssetImportDefaultPrivate:
        followupEvidence.summary.thirdPartyPrivateAssetImportDefaultPrivate,
      thirdPartyPrivateAssetImportStructuredJsonReady:
        followupEvidence.summary.thirdPartyPrivateAssetImportStructuredJsonReady,
      thirdPartyPrivateAssetImportScopeGuardReady:
        followupEvidence.summary.thirdPartyPrivateAssetImportScopeGuardReady,
      thirdPartyPrivateAssetImportPublicDocsClosed:
        followupEvidence.summary.thirdPartyPrivateAssetImportPublicDocsClosed,
      thirdPartyPrivateAssetImportEndpointGuardRedacted:
        followupEvidence.summary.thirdPartyPrivateAssetImportEndpointGuardRedacted,
      mainSiteGalleryTaskCardReady:
        followupEvidence.summary.mainSiteGalleryTaskCardReady,
      mainSiteGalleryTaskCardStable:
        followupEvidence.summary.mainSiteGalleryTaskCardStable,
      mainSiteGalleryTaskCardDetailReady:
        followupEvidence.summary.mainSiteGalleryTaskCardDetailReady,
      mainSiteGalleryTaskCardNoSideEffects:
        followupEvidence.summary.mainSiteGalleryTaskCardNoSideEffects,
      mainSiteGalleryTaskCardRedacted:
        followupEvidence.summary.mainSiteGalleryTaskCardRedacted,
      followupDryRunRedacted: followupEvidence.summary.followupDryRunRedacted,
      collectionScopedSingleImportRequiresAssetLibrary:
        scopeGuardEvidence.summary.singleCollectionRequiresAssetLibrary,
      collectionScopedBatchImportRequiresAssetLibrary:
        scopeGuardEvidence.summary.batchCollectionRequiresAssetLibrary,
      operatorManifestDryRunReady:
        operatorManifestEvidence.checks.operatorManifestDryRunReady,
      operatorManifestRequiredInputsTracked:
        operatorManifestEvidence.checks.operatorManifestRequiredInputsTracked,
      operatorManifestExecuteReadyMatchesInputs:
        operatorManifestEvidence.checks.operatorManifestExecuteReadyMatchesInputs,
      operatorManifestRequiresManualReview:
        operatorManifestEvidence.checks.operatorManifestRequiresManualReview,
      operatorManifestRunbookReady:
        operatorManifestEvidence.checks.operatorManifestRunbookReady,
      operatorManifestAuditCountsOnly:
        operatorManifestEvidence.checks.operatorManifestAuditCountsOnly,
      operatorManifestRedacted:
        operatorManifestEvidence.checks.operatorManifestRedacted,
      cleanupManifestDryRunReady:
        cleanupManifestEvidence.checks.cleanupManifestDryRunReady,
      cleanupManifestRequiresPostExecuteReceipt:
        cleanupManifestEvidence.checks.cleanupManifestRequiresPostExecuteReceipt,
      cleanupManifestReadyMatchesInputs:
        cleanupManifestEvidence.checks.cleanupManifestReadyMatchesInputs,
      cleanupManifestNoAutoCleanup:
        cleanupManifestEvidence.checks.cleanupManifestNoAutoCleanup,
      cleanupManifestRunbookReady:
        cleanupManifestEvidence.checks.cleanupManifestRunbookReady,
      cleanupManifestAuditCountsOnly:
        cleanupManifestEvidence.checks.cleanupManifestAuditCountsOnly,
      cleanupManifestRedacted:
        cleanupManifestEvidence.checks.cleanupManifestRedacted,
      operatorHandoffDryRunReady:
        operatorHandoffEvidence.checks.operatorHandoffDryRunReady,
      operatorHandoffExecuteStateConsistent:
        operatorHandoffEvidence.checks.operatorHandoffExecuteStateConsistent,
      operatorHandoffCleanupStateConsistent:
        operatorHandoffEvidence.checks.operatorHandoffCleanupStateConsistent,
      operatorHandoffTaskCardSummarySafe:
        operatorHandoffEvidence.checks.operatorHandoffTaskCardSummarySafe,
      operatorHandoffValidationSummarySafe:
        operatorHandoffEvidence.checks.operatorHandoffValidationSummarySafe,
      operatorHandoffNoSideEffects:
        operatorHandoffEvidence.checks.operatorHandoffNoSideEffects,
      operatorHandoffRedacted:
        operatorHandoffEvidence.checks.operatorHandoffRedacted,
      operatorHandoffContractDriftGuardReady:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffContractDriftGuardReady,
      operatorHandoffDetailStatusAllowed:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDetailStatusAllowed,
      operatorHandoffDetailOnly:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDetailOnly,
      operatorHandoffDoesNotMutateTaskCardStatus:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDoesNotMutateTaskCardStatus,
      operatorHandoffDoesNotCreateTaskCard:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffDoesNotCreateTaskCard,
      operatorHandoffNoCallbackOrPublicContractChange:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffNoCallbackOrPublicContractChange,
      operatorHandoffContractGuardRedacted:
        operatorHandoffContractGuardEvidence.checks.operatorHandoffContractGuardRedacted,
      operatorReadinessRollupDryRunReady:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupDryRunReady,
      operatorReadinessRollupInputContractsOk:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupInputContractsOk,
      operatorReadinessRollupExecuteGateConsistent:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupExecuteGateConsistent,
      operatorReadinessRollupCleanupGateConsistent:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupCleanupGateConsistent,
      operatorReadinessRollupNoSideEffects:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupNoSideEffects,
      operatorReadinessRollupRedacted:
        operatorReadinessRollupEvidence.checks.operatorReadinessRollupRedacted,
      operatorReadinessContractDriftGuardReady:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessContractDriftGuardReady,
      operatorReadinessRollupContainsNoCommandMaterial:
        operatorReadinessContractGuardEvidence.checks
          .operatorReadinessRollupContainsNoCommandMaterial,
      operatorReadinessPrivateRunbookOnly:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessPrivateRunbookOnly,
      operatorReadinessCommandTemplateRedacted:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessCommandTemplateRedacted,
      operatorReadinessPublicDocsDoNotExposeAssetImports:
        operatorReadinessContractGuardEvidence.checks
          .operatorReadinessPublicDocsDoNotExposeAssetImports,
      operatorReadinessContractGuardRedacted:
        operatorReadinessContractGuardEvidence.checks.operatorReadinessContractGuardRedacted,
      operatorPrivateRunbookReceiptPackageReady:
        privateRunbookReceiptPackageEvidence.checks.operatorPrivateRunbookReceiptPackageReady,
      operatorPrivateRunbookSeparatedFromSharedReceipt:
        privateRunbookReceiptPackageEvidence.checks
          .operatorPrivateRunbookSeparatedFromSharedReceipt,
      operatorPrivateRunbookPreExecuteNoRawMaterial:
        privateRunbookReceiptPackageEvidence.checks
          .operatorPrivateRunbookPreExecuteNoRawMaterial,
      operatorSharedValidationReceiptSafe:
        privateRunbookReceiptPackageEvidence.checks.operatorSharedValidationReceiptSafe,
      operatorReceiptPackageNoSideEffects:
        privateRunbookReceiptPackageEvidence.checks.operatorReceiptPackageNoSideEffects,
      operatorReceiptPackageRedacted:
        privateRunbookReceiptPackageEvidence.checks.operatorReceiptPackageRedacted,
      operatorReceiptPackageDisplayContractGuardReady:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDisplayContractGuardReady,
      operatorReceiptPackageDetailOnly:
        receiptPackageDisplayContractGuardEvidence.checks.operatorReceiptPackageDetailOnly,
      operatorReceiptPackageDoesNotCreateTaskCard:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDoesNotCreateTaskCard,
      operatorReceiptPackageDoesNotMutateStatusEnum:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDoesNotMutateStatusEnum,
      operatorReceiptPackageSharedReceiptExcludesPrivateMaterial:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageSharedReceiptExcludesPrivateMaterial,
      operatorReceiptPackageDisplayContractGuardRedacted:
        receiptPackageDisplayContractGuardEvidence.checks
          .operatorReceiptPackageDisplayContractGuardRedacted,
      operatorValidationRollupDryRunReady:
        operatorValidationRollupEvidence.checks.operatorValidationRollupDryRunReady,
      operatorValidationRollupInputContractsOk:
        operatorValidationRollupEvidence.checks.operatorValidationRollupInputContractsOk,
      operatorValidationRollupSingleGateReady:
        operatorValidationRollupEvidence.checks.operatorValidationRollupSingleGateReady,
      operatorValidationRollupLiveExecuteStillRequiresPrivateInputs:
        operatorValidationRollupEvidence.checks
          .operatorValidationRollupLiveExecuteStillRequiresPrivateInputs,
      operatorValidationRollupNoSideEffects:
        operatorValidationRollupEvidence.checks.operatorValidationRollupNoSideEffects,
      operatorValidationRollupRedacted:
        operatorValidationRollupEvidence.checks.operatorValidationRollupRedacted,
      operatorValidationRollupDisplayContractGuardReady:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayContractGuardReady,
      operatorValidationRollupDisplayOnly:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayOnly,
      operatorValidationRollupDoesNotTriggerLiveExecute:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDoesNotTriggerLiveExecute,
      operatorValidationRollupDoesNotCreateOrUpdateTaskCard:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDoesNotCreateOrUpdateTaskCard,
      operatorValidationRollupSharedReceiptExcludesPrivateMaterial:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupSharedReceiptExcludesPrivateMaterial,
      operatorValidationRollupDisplayContractGuardRedacted:
        operatorValidationRollupDisplayContractGuardEvidence.checks
          .operatorValidationRollupDisplayContractGuardRedacted,
      operatorValidationReleaseSummaryReady:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryReady,
      operatorValidationReleaseSummaryPrivateInputsOnly:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryPrivateInputsOnly,
      operatorValidationReleaseSummaryContractsStable:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryContractsStable,
      operatorValidationReleaseSummaryDoesNotCreateOrMutateTaskCard:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryDoesNotCreateOrMutateTaskCard,
      operatorValidationReleaseSummaryDoesNotChangeSharedReceipt:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryDoesNotChangeSharedReceipt,
      operatorValidationReleaseSummaryRedacted:
        operatorValidationReleaseSummaryEvidence.checks
          .operatorValidationReleaseSummaryRedacted,
      operatorValidationReleaseSummaryFieldLedgerGuardReady:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerGuardReady,
      operatorValidationReleaseSummaryFieldLedgerOnly:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerOnly,
      operatorValidationReleaseSummaryFieldLedgerNoLiveExecute:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerNoLiveExecute,
      operatorValidationReleaseSummaryFieldLedgerTaskCardStable:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerTaskCardStable,
      operatorValidationReleaseSummaryFieldLedgerSharedReceiptStable:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerSharedReceiptStable,
      operatorValidationReleaseSummaryFieldLedgerGuardRedacted:
        operatorValidationReleaseSummaryFieldLedgerGuardEvidence.checks
          .operatorValidationReleaseSummaryFieldLedgerGuardRedacted,
      operatorReadyHandoffReceiptReady:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffReceiptReady,
      operatorReadyHandoffRequiresOnlyPrivateValues:
        operatorReadyHandoffReceiptEvidence.checks
          .operatorReadyHandoffRequiresOnlyPrivateValues,
      operatorReadyHandoffContractsStable:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffContractsStable,
      operatorReadyHandoffNoTaskCardOrSharedReceiptMutation:
        operatorReadyHandoffReceiptEvidence.checks
          .operatorReadyHandoffNoTaskCardOrSharedReceiptMutation,
      operatorReadyHandoffNoExternalEffects:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffNoExternalEffects,
      operatorReadyHandoffReceiptRedacted:
        operatorReadyHandoffReceiptEvidence.checks.operatorReadyHandoffReceiptRedacted,
      finalValidationReadinessBundleReady:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationReadinessBundleReady,
      finalValidationBundleSingleGateReady:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleSingleGateReady,
      finalValidationBundleRequiresOnlyPrivateValues:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleRequiresOnlyPrivateValues,
      finalValidationBundleContractsStable:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleContractsStable,
      finalValidationBundleNoExternalEffects:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleNoExternalEffects,
      finalValidationBundleRedacted:
        finalValidationReadinessBundleEvidence.checks
          .finalValidationBundleRedacted,
      noNetworkOrProductionWrite: true,
    },
  };
  attachMachineSummary(report);
  assert.equal(report.ok, true);
  return { report, runId: fixtures.runId, suffix: 'self-test' };
}

async function runPreflight(args) {
  const fixtures = await writeFixtureFiles(args);
  const payloadEvidence = buildPayloadEvidence(fixtures);
  const responseEvidence = buildMockImportResponse(payloadEvidence);
  const scopeEvidence = buildScopeEvidence();
  const planningEvidence = buildPlanningEvidence(scopeEvidence);
  const followupEvidence = buildFollowupDryRunEvidence(responseEvidence, scopeEvidence);
  const scopeGuardEvidence = buildScopeGuardEvidence();
  const report = buildPreflightReport(
    args,
    fixtures,
    payloadEvidence,
    responseEvidence,
    scopeEvidence,
    planningEvidence,
    followupEvidence,
    scopeGuardEvidence,
  );
  return { report, runId: fixtures.runId, suffix: 'preflight' };
}

async function writeReport(args, runId, suffix, report) {
  assertReportSafe(report);

  const outputDir = resolveOutputDir(args);
  await mkdir(outputDir, { recursive: true });
  const reportPath = path.join(outputDir, `${runId}-${suffix}.json`);
  const markdownPath = path.join(outputDir, `${runId}-${suffix}.md`);
  await writeFile(reportPath, `${JSON.stringify(report, null, args.pretty ? 2 : 0)}\n`, 'utf8');
  await writeFile(markdownPath, buildMarkdown(report), 'utf8');
  return { reportPath, markdownPath };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  let result;
  if (args.selfTest) {
    result = await runSelfTest(args);
  } else if (args.preflight) {
    result = await runPreflight(args);
  } else {
    result = await executeLive(args);
  }
  const { report, runId, suffix } = result;
  const { reportPath, markdownPath } = await writeReport(args, runId, suffix, report);

  console.log(JSON.stringify({
    ok: report.ok,
    mode: report.mode,
    selfTest: Boolean(report.self_test),
    preflight: Boolean(report.preflight),
    execute: Boolean(report.execute),
    runId,
    reportPath,
    markdownPath,
  }, null, 2));
  if (!report.ok) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error?.stack || error?.message || error);
  process.exit(1);
});
