#!/usr/bin/env node

import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { buildArtifactTaskCards } from '../../apps/web/app/lib/artifact-task-cards.js';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_OUTPUT_DIR = 'target/v3-client-artifact-joint-smoke';
const DEFAULT_TIMEOUT_MS = 120_000;
const MANIFEST_SCHEMA = 'v3.client_artifact_manifest.v1';
const MANIFEST_SOURCE = 'enterprise-codex-client';
const DEFAULT_CODEX_CONTROL_BASE_URL = 'https://ad.goods-editor.com';
const DEFAULT_CODEX_ACTIVATION_ENDPOINT = '/api/codex/clients/activate';
const DEFAULT_CODEX_HEARTBEAT_ENDPOINT = '/api/codex/clients/heartbeat';
const DEFAULT_CODEX_REVOKE_ENDPOINT = '/api/codex/clients/self-revoke';
const DEFAULT_CODEX_ACTIVATION_TOKEN_ENV = 'CODEX_CLIENT_ACTIVATION_TOKEN';

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.V3_CLIENT_ARTIFACT_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    cookie: process.env.V3_CLIENT_ARTIFACT_SMOKE_COOKIE || '',
    bearer: process.env.V3_CLIENT_ARTIFACT_SMOKE_BEARER || '',
    outputDir: process.env.V3_CLIENT_ARTIFACT_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    timeoutMs: Number(process.env.V3_CLIENT_ARTIFACT_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    datasetId: process.env.V3_CLIENT_ARTIFACT_SMOKE_DATASET_ID || '',
    assetLibraryId: process.env.V3_CLIENT_ARTIFACT_SMOKE_ASSET_LIBRARY_ID || '',
    clientId: process.env.V3_CLIENT_ARTIFACT_SMOKE_CLIENT_ID || '',
    runId: process.env.V3_CLIENT_ARTIFACT_SMOKE_RUN_ID || makeRunId(),
    selfTest: parseBoolean(process.env.V3_CLIENT_ARTIFACT_SMOKE_SELF_TEST),
    preflight: parseBoolean(process.env.V3_CLIENT_ARTIFACT_SMOKE_PREFLIGHT),
    execute: parseBoolean(process.env.V3_CLIENT_ARTIFACT_SMOKE_EXECUTE),
    ackControlledLive: parseBoolean(process.env.V3_CLIENT_ARTIFACT_SMOKE_ACK_CONTROLLED_LIVE),
    skipPublicPublish: parseBoolean(process.env.V3_CLIENT_ARTIFACT_SMOKE_SKIP_PUBLIC_PUBLISH),
    checkPublicUrl: parseBoolean(process.env.V3_CLIENT_ARTIFACT_SMOKE_CHECK_PUBLIC_URL),
    pretty: parseBoolean(process.env.V3_CLIENT_ARTIFACT_SMOKE_PRETTY),
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
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--dataset-id') {
      args.datasetId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--asset-library-id') {
      args.assetLibraryId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--client-id') {
      args.clientId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--run-id') {
      args.runId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--preflight') {
      args.preflight = true;
    } else if (arg === '--execute') {
      args.execute = true;
    } else if (arg === '--ack-controlled-live') {
      args.ackControlledLive = true;
    } else if (arg === '--skip-public-publish') {
      args.skipPublicPublish = true;
    } else if (arg === '--check-public-url') {
      args.checkPublicUrl = true;
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
  if (args.execute && !args.ackControlledLive) {
    throw new Error('--execute requires --ack-controlled-live');
  }
  if (args.execute && !args.cookie && !args.bearer) {
    throw new Error('--execute requires --cookie or --bearer for a V3 user session');
  }
  if (args.checkPublicUrl && args.skipPublicPublish) {
    throw new Error('--check-public-url cannot be combined with --skip-public-publish');
  }
  args.baseUrl = normalizeBaseUrl(args.baseUrl);
  args.datasetId ||= `dmx-client-smoke-dataset-${args.runId}`;
  args.assetLibraryId ||= `dmx-client-smoke-asset-${args.runId}`;
  args.clientId ||= `dmx-client-smoke-device-${args.runId}`;
  return args;
}

function printHelp() {
  console.log(`Usage:
  npm run smoke:v3-client-artifact-joint -- --self-test

  npm run smoke:v3-client-artifact-joint -- --preflight \\
    --base-url https://v3.elepcloud.com

  npm run smoke:v3-client-artifact-joint -- --execute --ack-controlled-live \\
    --base-url https://v3.elepcloud.com \\
    --cookie "<V3 session cookie>"

Live execute flow:
  V3 creates config package
  -> smoke simulates client reading it
  -> smoke uploads index.html + report.md as enterprise-codex-client artifact
  -> V3 private publishes the artifact
  -> V3 public-publishes the HTML as a generated-artifact sandbox page
  -> receipt verifies task-card normalization and returned URLs

Environment aliases:
  V3_CLIENT_ARTIFACT_SMOKE_BASE_URL
  V3_CLIENT_ARTIFACT_SMOKE_COOKIE
  V3_CLIENT_ARTIFACT_SMOKE_BEARER
  V3_CLIENT_ARTIFACT_SMOKE_DATASET_ID
  V3_CLIENT_ARTIFACT_SMOKE_ASSET_LIBRARY_ID
`);
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

function normalizeBaseUrl(value) {
  return String(value || '').replace(/\/+$/, '');
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

function authHeaders(args) {
  const headers = {};
  if (args.cookie) headers.cookie = args.cookie;
  if (args.bearer) {
    headers.authorization = args.bearer.toLowerCase().startsWith('bearer ')
      ? args.bearer
      : `Bearer ${args.bearer}`;
  }
  return headers;
}

function jsonHeaders(args) {
  return {
    ...authHeaders(args),
    'content-type': 'application/json',
  };
}

async function fetchJson(url, options = {}, timeoutMs = DEFAULT_TIMEOUT_MS) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, {
      ...options,
      signal: controller.signal,
    });
    const text = await response.text();
    let body = null;
    if (text) {
      try {
        body = JSON.parse(text);
      } catch {
        body = { raw_text: text.slice(0, 500) };
      }
    }
    if (!response.ok) {
      const code = body?.error?.code || body?.code || response.status;
      throw new Error(`${options.method || 'GET'} ${url} failed: ${response.status} ${code}`);
    }
    return body;
  } finally {
    clearTimeout(timer);
  }
}

async function fetchText(url, options = {}, timeoutMs = DEFAULT_TIMEOUT_MS) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, {
      ...options,
      signal: controller.signal,
    });
    const text = await response.text();
    if (!response.ok) {
      throw new Error(`${options.method || 'GET'} ${url} failed: ${response.status}`);
    }
    return text;
  } finally {
    clearTimeout(timer);
  }
}

function buildConfigPackageRequest(args) {
  return {
    client_id: args.clientId,
    v3_base_url: args.baseUrl,
    asset_library_ids: [args.assetLibraryId],
    dataset_ids: [args.datasetId],
    skill_packs: ['static-page-edit', 'report-revision'],
    artifact_upload: {
      mode: 'session_token',
      endpoint: '/v1/client-artifacts',
    },
    codex_control: {
      base_url: DEFAULT_CODEX_CONTROL_BASE_URL,
      activation_endpoint: DEFAULT_CODEX_ACTIVATION_ENDPOINT,
      heartbeat_endpoint: DEFAULT_CODEX_HEARTBEAT_ENDPOINT,
      revoke_endpoint: DEFAULT_CODEX_REVOKE_ENDPOINT,
      activation_token_env: DEFAULT_CODEX_ACTIVATION_TOKEN_ENV,
      terminal_id: `term-smoke-${args.runId}`,
      terminal_label: 'joint-smoke',
      session_ttl_seconds: 30 * 24 * 60 * 60,
    },
    expires_at: new Date(Date.now() + 60 * 60 * 1000).toISOString(),
    metadata: {
      smoke: true,
      run_id: args.runId,
      source: 'v3-client-artifact-joint-smoke',
    },
  };
}

function buildClientArtifactManifest(args, packageView = {}) {
  const createdAt = new Date().toISOString();
  return {
    schema: MANIFEST_SCHEMA,
    source: MANIFEST_SOURCE,
    tenant_id: packageView.tenant_id || `tenant-smoke-${args.runId}`,
    user_id: packageView.user_id || `user-smoke-${args.runId}`,
    client_id: args.clientId,
    task_id: `client-task-${args.runId}`,
    asset_library_ids: [args.assetLibraryId],
    dataset_ids: [args.datasetId],
    title: `DataMax client artifact joint smoke ${args.runId}`,
    artifact_type: 'static_page',
    files: [
      {
        filename: 'index.html',
        content_type: 'text/html',
        role: 'primary_html',
      },
      {
        filename: 'report.md',
        content_type: 'text/markdown',
        role: 'source_summary',
      },
    ],
    evidence_refs: [
      { kind: 'dataset', id: args.datasetId },
      { kind: 'asset_library', id: args.assetLibraryId },
    ],
    created_at: createdAt,
    metadata: {
      smoke: true,
      run_id: args.runId,
      config_package_id: packageView.package_id || null,
    },
  };
}

function buildIndexHtml(args) {
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <title>DataMax Client Artifact Smoke ${args.runId}</title>
  <script>window.__should_be_sanitized = true;</script>
</head>
<body onload="window.__unsafe = true">
  <main>
    <h1>DataMax Client Artifact Smoke</h1>
    <p>Run ${args.runId}</p>
    <section data-dataset="${args.datasetId}">
      <h2>经营分析页面</h2>
      <p>本页面模拟 Codex 企业客户端回写的 HTML 产物。</p>
    </section>
  </main>
</body>
</html>`;
}

function buildReportMarkdown(args) {
  return `# DataMax Client Artifact Joint Smoke

- run_id: ${args.runId}
- dataset_id: ${args.datasetId}
- asset_library_id: ${args.assetLibraryId}
- expected: V3 receives, publishes, and exposes the HTML artifact through task cards.
`;
}

function validateManifest(manifest, files) {
  assert.equal(manifest.schema, MANIFEST_SCHEMA);
  assert.equal(manifest.source, MANIFEST_SOURCE);
  assert.equal(manifest.files.length, files.length);
  manifest.files.forEach((file, index) => {
    assert.equal(file.filename, files[index].filename);
    assert.ok(file.content_type);
    assert.ok(file.role);
  });
  assert.ok(manifest.dataset_ids.length);
  assert.ok(manifest.asset_library_ids.length);
}

function hasOwnDeep(value, keyName) {
  if (!value || typeof value !== 'object') return false;
  if (Object.hasOwn(value, keyName)) return true;
  if (Array.isArray(value)) {
    return value.some((item) => hasOwnDeep(item, keyName));
  }
  return Object.values(value).some((item) => hasOwnDeep(item, keyName));
}

function hasAuthMaterial(value) {
  const text = JSON.stringify(value || {});
  return /aidp_v3_session=|Bearer\s+[A-Za-z0-9._-]+|V3_CLIENT_ARTIFACT_SMOKE_(?:COOKIE|BEARER)|raw[_-]?(?:cookie|bearer)/i
    .test(text);
}

function platformHealthOk(value, expectedStatus) {
  return !value?.error &&
    value?.service === 'platform-api' &&
    value?.status === expectedStatus;
}

function buildReceiptSummary(receipt) {
  const mode = receipt?.mode || null;
  const checks = {
    modeRecognized: ['self_test', 'preflight', 'execute'].includes(mode),
    noAuthMaterialRecorded: !hasAuthMaterial(receipt),
    noActivationTokenIncluded: !hasOwnDeep(receipt, 'activation_token'),
  };

  if (mode === 'self_test') {
    checks.packageRequestHasDatasetAndAssetLibrary =
      Number(receipt.package_request?.dataset_count || 0) > 0 &&
      Number(receipt.package_request?.asset_library_count || 0) > 0;
    checks.codexControlIsEnvOnly =
      receipt.package_request?.codex_control?.activation_token_env === DEFAULT_CODEX_ACTIVATION_TOKEN_ENV &&
      receipt.package_request?.codex_control?.has_activation_token === false;
    checks.manifestContractPresent =
      receipt.manifest?.schema === MANIFEST_SCHEMA &&
      receipt.manifest?.source === MANIFEST_SOURCE &&
      Number(receipt.manifest?.file_count || 0) >= 2;
    checks.taskCardCanOpenHtml =
      Boolean(receipt.task_card?.html_url) &&
      Number(receipt.task_card?.file_count || 0) >= 1;
  } else if (mode === 'preflight') {
    checks.healthOk = platformHealthOk(receipt.health, 'ok');
    checks.readyOk = platformHealthOk(receipt.ready, 'ready');
    checks.executeStillRequiresExplicitLiveMode =
      receipt.execute_required_for_mutating_joint_smoke === true;
  } else if (mode === 'execute') {
    checks.packageCreated = Boolean(receipt.package_id);
    checks.artifactUploaded = Boolean(receipt.artifact_id);
    checks.privatePublishCompleted = receipt.private_publish_status === 'published';
    checks.taskCardCanOpenHtml =
      Boolean(receipt.task_card?.html_url) &&
      Number(receipt.task_card?.file_count || 0) >= 1;
    checks.publicPublishStateConsistent =
      receipt.skipped_public_publish === true
        ? receipt.public_url === null
        : Boolean(receipt.public_url);
    checks.publicUrlCheckRespected =
      receipt.public_url_check === null || receipt.public_url_check?.ok === true;
  }

  const ok = Object.values(checks).every(Boolean);
  return {
    ok,
    checks,
    mode,
    ready: ok && mode === 'execute',
    pending: ok && mode === 'preflight',
    failed: !ok,
    self_test: mode === 'self_test',
  };
}

function assertReceiptSummaryFailure(name, receipt, expectedCheckName) {
  const summary = buildReceiptSummary(receipt);
  assert.equal(summary.ok, false, `${name} should fail receipt summary`);
  if (expectedCheckName) {
    assert.equal(
      summary.checks[expectedCheckName],
      false,
      `${name} should fail ${expectedCheckName}`,
    );
  }
}

function buildMultipart(manifest, files) {
  const form = new FormData();
  form.append(
    'manifest',
    new Blob([JSON.stringify(manifest)], { type: 'application/json' }),
    'manifest.json',
  );
  for (const file of files) {
    form.append(
      'files',
      new Blob([file.content], { type: file.contentType }),
      file.filename,
    );
  }
  return form;
}

async function writeReceipt(args, receipt) {
  receipt.summary = buildReceiptSummary(receipt);
  receipt.ok = receipt.summary.ok;
  await mkdir(args.outputDir, { recursive: true });
  const receiptPath = join(args.outputDir, `receipt-${args.runId}.json`);
  await writeFile(receiptPath, JSON.stringify(receipt, null, args.pretty ? 2 : 0));
  return receiptPath;
}

function validateClientArtifactTaskCard(artifact) {
  const cards = buildArtifactTaskCards({
    clientArtifacts: [artifact],
  });
  assert.equal(cards.length, 1);
  const [card] = cards;
  assert.equal(card.kind, 'v3_client_artifact');
  assert.equal(card.status, 'published');
  assert.ok(card.canOpen);
  assert.ok(card.files.some((file) => file.kind === 'client_html' && file.canOpen));
  return {
    card_id: card.id,
    status: card.status,
    primary_file_id: card.primaryFileId,
    file_count: card.files.length,
    html_url: card.files.find((file) => file.kind === 'client_html')?.url || '',
  };
}

function selfTest(args) {
  const packageRequest = buildConfigPackageRequest(args);
  const packageView = {
    package_id: `v3cp_${args.runId}`,
    tenant_id: `tenant-smoke-${args.runId}`,
    user_id: `user-smoke-${args.runId}`,
    client_id: args.clientId,
    artifact_upload: packageRequest.artifact_upload,
    codex_control: packageRequest.codex_control,
    dataset_ids: packageRequest.dataset_ids,
    asset_library_ids: packageRequest.asset_library_ids,
  };
  const manifest = buildClientArtifactManifest(args, packageView);
  const files = [
    { filename: 'index.html', contentType: 'text/html', content: buildIndexHtml(args) },
    { filename: 'report.md', contentType: 'text/markdown', content: buildReportMarkdown(args) },
  ];
  validateManifest(manifest, files);
  assert.equal(packageRequest.codex_control.base_url, DEFAULT_CODEX_CONTROL_BASE_URL);
  assert.equal(packageRequest.codex_control.activation_token_env, DEFAULT_CODEX_ACTIVATION_TOKEN_ENV);
  assert.equal(Object.hasOwn(packageRequest.codex_control, 'activation_token'), false);

  const simulatedArtifact = {
    artifact_id: `v3ca_${args.runId}`,
    title: manifest.title,
    status: 'published',
    task_id: manifest.task_id,
    dataset_ids: manifest.dataset_ids,
    asset_library_ids: manifest.asset_library_ids,
    files: [
      {
        file_index: 0,
        filename: 'index.html',
        content_type: 'text/html',
        role: 'primary_html',
        size_bytes: files[0].content.length,
        sha256: 'self-test',
        download_url: `/v1/client-artifacts/v3ca_${args.runId}/files/0`,
        preview_url: `/v1/client-artifacts/v3ca_${args.runId}/files/0/preview`,
        public_url: `/generated-artifacts/client-artifacts/v3ca_${args.runId}/html-0/index.html`,
      },
      {
        file_index: 1,
        filename: 'report.md',
        content_type: 'text/markdown',
        role: 'source_summary',
        size_bytes: files[1].content.length,
        sha256: 'self-test-md',
        download_url: `/v1/client-artifacts/v3ca_${args.runId}/files/1`,
      },
    ],
    manifest,
    created_at: new Date().toISOString(),
  };
  const card = validateClientArtifactTaskCard(simulatedArtifact);
  assert.equal(card.html_url, simulatedArtifact.files[0].public_url);
  const receipt = {
    mode: 'self_test',
    ok: true,
    run_id: args.runId,
    package_request: {
      client_id: packageRequest.client_id,
      artifact_upload: packageRequest.artifact_upload,
      codex_control: {
        base_url: packageRequest.codex_control.base_url,
        activation_endpoint: packageRequest.codex_control.activation_endpoint,
        heartbeat_endpoint: packageRequest.codex_control.heartbeat_endpoint,
        revoke_endpoint: packageRequest.codex_control.revoke_endpoint,
        activation_token_env: packageRequest.codex_control.activation_token_env,
        has_activation_token: Object.hasOwn(packageRequest.codex_control, 'activation_token'),
      },
      dataset_count: packageRequest.dataset_ids.length,
      asset_library_count: packageRequest.asset_library_ids.length,
    },
    manifest: {
      schema: manifest.schema,
      source: manifest.source,
      file_count: manifest.files.length,
      artifact_type: manifest.artifact_type,
    },
    task_card: card,
  };
  const summary = buildReceiptSummary(receipt);
  assert.equal(summary.ok, true, 'self-test fixture receipt summary should pass');

  const leakedTokenReceipt = structuredClone(receipt);
  leakedTokenReceipt.package_request.codex_control.activation_token = 'raw-activation-token';
  assertReceiptSummaryFailure(
    'activation token leak',
    leakedTokenReceipt,
    'noActivationTokenIncluded',
  );

  const brokenPreflightReceipt = {
    mode: 'preflight',
    ok: false,
    health: { service: 'platform-api', status: 'ok' },
    ready: { error: 'connection refused' },
    execute_required_for_mutating_joint_smoke: true,
  };
  assertReceiptSummaryFailure('broken preflight ready check', brokenPreflightReceipt, 'readyOk');

  const htmlPreflightReceipt = {
    mode: 'preflight',
    ok: true,
    health: { raw_text: '<!DOCTYPE html><html></html>' },
    ready: { raw_text: '<!DOCTYPE html><html></html>' },
    execute_required_for_mutating_joint_smoke: true,
  };
  assertReceiptSummaryFailure('wrong public base HTML preflight', htmlPreflightReceipt, 'healthOk');

  const missingTaskCardReceipt = structuredClone(receipt);
  missingTaskCardReceipt.mode = 'execute';
  missingTaskCardReceipt.package_id = 'v3cp_smoke';
  missingTaskCardReceipt.artifact_id = 'v3ca_smoke';
  missingTaskCardReceipt.private_publish_status = 'published';
  missingTaskCardReceipt.public_url = 'https://v3.elepcloud.com/generated-artifacts/client-artifacts/smoke/index.html';
  missingTaskCardReceipt.public_url_check = null;
  missingTaskCardReceipt.skipped_public_publish = false;
  missingTaskCardReceipt.task_card = null;
  assertReceiptSummaryFailure('execute without task card', missingTaskCardReceipt, 'taskCardCanOpenHtml');

  return receipt;
}

async function preflight(args) {
  const health = await fetchJson(`${args.baseUrl}/healthz`, {}, args.timeoutMs).catch((error) => ({
    error: error.message,
  }));
  const ready = await fetchJson(`${args.baseUrl}/readyz`, {}, args.timeoutMs).catch((error) => ({
    error: error.message,
  }));
  const ok = !health.error && !ready.error;
  return {
    mode: 'preflight',
    ok,
    run_id: args.runId,
    base_url: args.baseUrl,
    health,
    ready,
    auth_supplied: Boolean(args.cookie || args.bearer),
    execute_required_for_mutating_joint_smoke: true,
  };
}

async function executeLive(args) {
  const packageRequest = buildConfigPackageRequest(args);
  const packageResponse = await fetchJson(
    `${args.baseUrl}/v1/client-config-packages`,
    {
      method: 'POST',
      headers: jsonHeaders(args),
      body: JSON.stringify(packageRequest),
    },
    args.timeoutMs,
  );
  const packageView = packageResponse.package || packageResponse;
  assert.ok(packageView.package_id);
  assert.equal(packageView.artifact_upload?.endpoint, '/v1/client-artifacts');
  assert.equal(packageView.codex_control?.base_url, DEFAULT_CODEX_CONTROL_BASE_URL);
  assert.equal(packageView.codex_control?.activation_token_env, DEFAULT_CODEX_ACTIVATION_TOKEN_ENV);
  assert.equal(Object.hasOwn(packageView.codex_control || {}, 'activation_token'), false);

  const fetchedPackage = await fetchJson(
    `${args.baseUrl}/v1/client-config-packages/${encodeURIComponent(packageView.package_id)}`,
    {},
    args.timeoutMs,
  );
  assert.equal(fetchedPackage.package_id, packageView.package_id);

  const manifest = buildClientArtifactManifest(args, packageView);
  const files = [
    { filename: 'index.html', contentType: 'text/html', content: buildIndexHtml(args) },
    { filename: 'report.md', contentType: 'text/markdown', content: buildReportMarkdown(args) },
  ];
  validateManifest(manifest, files);
  const form = buildMultipart(manifest, files);
  const uploadResponse = await fetchJson(
    `${args.baseUrl}/v1/client-artifacts`,
    {
      method: 'POST',
      headers: authHeaders(args),
      body: form,
    },
    args.timeoutMs,
  );
  const uploadedArtifact = uploadResponse.artifact;
  assert.ok(uploadedArtifact?.artifact_id);
  assert.ok(uploadedArtifact.files?.some((file) => file.filename === 'index.html'));

  const privatePublishResponse = await fetchJson(
    `${args.baseUrl}/v1/client-artifacts/${encodeURIComponent(uploadedArtifact.artifact_id)}/publish`,
    {
      method: 'POST',
      headers: authHeaders(args),
    },
    args.timeoutMs,
  );
  const privateArtifact = privatePublishResponse.artifact;
  assert.equal(privateArtifact.status, 'published');
  assert.ok(privateArtifact.files?.some((file) => file.preview_url || file.previewUrl));

  let publicPublish = null;
  let publicUrlCheck = null;
  let finalArtifact = privateArtifact;
  if (!args.skipPublicPublish) {
    publicPublish = await fetchJson(
      `${args.baseUrl}/v1/client-artifacts/${encodeURIComponent(uploadedArtifact.artifact_id)}/publish-public`,
      {
        method: 'POST',
        headers: authHeaders(args),
      },
      args.timeoutMs,
    );
    assert.ok(publicPublish.public_url);
    finalArtifact = publicPublish.artifact;
    assert.ok(finalArtifact.files?.some((file) => file.public_url || file.publicUrl));

    if (args.checkPublicUrl) {
      const html = await fetchText(publicPublish.public_url, {}, args.timeoutMs);
      publicUrlCheck = {
        ok: html.includes('sandbox=""') && !html.toLowerCase().includes('<script'),
        bytes: html.length,
        contains_sandbox: html.includes('sandbox=""'),
        contains_script_tag: html.toLowerCase().includes('<script'),
      };
      assert.equal(publicUrlCheck.ok, true);
    }
  }

  const card = validateClientArtifactTaskCard(finalArtifact);
  return {
    mode: 'execute',
    ok: true,
    run_id: args.runId,
    base_url: args.baseUrl,
    package_id: packageView.package_id,
    artifact_id: uploadedArtifact.artifact_id,
    private_publish_status: privateArtifact.status,
    public_url: publicPublish?.public_url || null,
    html_artifact_id: publicPublish?.html_artifact?.id || null,
    task_card: card,
    public_url_check: publicUrlCheck,
    skipped_public_publish: args.skipPublicPublish,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  let receipt;
  if (args.selfTest) {
    receipt = selfTest(args);
  } else if (args.preflight) {
    receipt = await preflight(args);
  } else {
    receipt = await executeLive(args);
  }
  const receiptPath = await writeReceipt(args, receipt);
  console.log(JSON.stringify({ ...receipt, receipt_path: receiptPath }, null, args.pretty ? 2 : 0));
  if (receipt.summary?.ok === false) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error?.stack || error?.message || String(error));
  process.exit(1);
});
