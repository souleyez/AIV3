#!/usr/bin/env node

import { chmod, lstat, mkdir, open, readFile, realpath, stat } from 'node:fs/promises';
import { createHash, randomUUID } from 'node:crypto';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, '..', '..');
const defaultFixturePath = resolve(repoRoot, 'fixtures', 'newbai-customer-answer', 'cases.jsonl');
const defaultOutputDir = resolve(repoRoot, 'target', 'semantic-supply-main-live');
const defaultCaseIds = Object.freeze([
  'newbai-customer-001',
  'newbai-customer-002',
  'newbai-customer-003',
  'newbai-customer-004',
  'newbai-customer-005',
  'newbai-customer-006',
  'newbai-customer-007',
  'newbai-customer-008',
  'newbai-customer-011',
]);
const registeredCaseIds = new Set(defaultCaseIds);
const maxLiveCaseCount = defaultCaseIds.length;
const maxPromptChars = 4_000;
const armModes = Object.freeze({ A: 'off', B: 'rerank', C: 'supplement' });
const rawSchemaVersion = 'semantic-supply-main-live-raw.v1';
const summarySchemaVersion = 'semantic-supply-main-live-summary.v1';
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const authEnvNames = Object.freeze({
  cookie: 'SEMANTIC_SUPPLY_MAIN_SESSION_COOKIE',
  email: 'SEMANTIC_SUPPLY_MAIN_AUTH_EMAIL',
  localKey: 'SEMANTIC_SUPPLY_MAIN_LOCAL_KEY',
  secretBindingIds: 'SEMANTIC_SUPPLY_MAIN_SECRET_BINDING_IDS',
  approvedIdentitySha256: 'SEMANTIC_SUPPLY_MAIN_APPROVED_IDENTITY_SHA256',
});
const workflowEventNames = new Set([
  'assistant_run.codex_sidecar_queued',
  'codex_host.fixed_task.queued',
  'codex_host.fixed_task.completed',
  'codex_host.fixed_task.needs_human',
  'codex_host.fixed_task.rejected',
  'codex_host_task.exec_heartbeat',
  'codex_host_task.cloudflare_heartbeat',
  'codex_host_task.poll_retry',
  'codex_host_task.exec_fallback_started',
  'codex_host_task.dry_run_completed',
  'codex_host_task.plan_only_completed',
  'codex_host_task.exec_completed',
  'codex_host_task.completed',
  'codex_host_task.exec_failed',
  'codex_host_task.cancelled',
  'assistant_run.customer_artifact_request_artifacts_ready',
  'assistant_run.generated_static_page_edit_artifacts_ready',
]);
const suppliedItemSourceFields = Object.freeze([
  'type',
  'source',
  'source_id',
  'sourceId',
  'source_locator',
  'sourceLocator',
  'evidence_ref',
  'retrieval_evidence_id',
  'document_id',
  'document_chunk_id',
  'dataset_id',
]);

function usage() {
  return `Usage:
  node scripts/smoke/semantic-supply-main-live.mjs --self-test [--pretty]
  node scripts/smoke/semantic-supply-main-live.mjs --preflight \\
    --ack-controlled-live --approval-id ID --arm A|B|C \\
    --base-url URL --dataset-id UUID [--case-ids CSV] [--pretty]
  node scripts/smoke/semantic-supply-main-live.mjs --execute \\
    --ack-controlled-live --approval-id ID --arm A|B|C \\
    --base-url URL --dataset-id UUID [--case-ids CSV] [--output-dir PATH]

Authentication is environment-only:
  ${authEnvNames.cookie}
or both:
  ${authEnvNames.email}
  ${authEnvNames.localKey}
Optional for an existing cookie:
  ${authEnvNames.secretBindingIds}
Required for every execute/preflight:
  ${authEnvNames.approvedIdentitySha256}

The execute mode performs exactly one main-site stream POST attempt per selected
case, then read-only detail polling. It never creates or modifies business
datasets/assets, edits feature flags, calls an external channel, or retries a
stream/provider request. It does create controlled AssistantRun/event records;
local-key auth creates then revokes a session/audit record; and the model may
freely produce workflow/artifact side effects, which are measured and recorded
in a no-auto-delete cleanup manifest. Depending on server auth semantics,
local-key login may also ensure the approved user record; creation versus reuse
is not observable here and is recorded as an operator-review item.

CURRENT BUILD: preflight/execute are mechanically disabled before filesystem
writes or receipt creation, authentication, or network activity because no
offline promotion candidate exists.`;
}

function parseArgs(argv) {
  const args = {
    mode: null,
    fixturePath: defaultFixturePath,
    outputDir: defaultOutputDir,
    caseIds: [...defaultCaseIds],
    caseIdsExplicit: false,
    baseUrl: process.env.SEMANTIC_SUPPLY_MAIN_BASE_URL || '',
    datasetId: process.env.SEMANTIC_SUPPLY_MAIN_DATASET_ID || '',
    approvalId: '',
    arm: '',
    ackControlledLive: false,
    timeoutMs: 120_000,
    pollIntervalMs: 1_000,
    pollAttempts: 20,
    pretty: false,
    help: false,
  };
  const valueFlags = new Map([
    ['--output-dir', 'outputDir'],
    ['--case-ids', 'caseIds'],
    ['--base-url', 'baseUrl'],
    ['--dataset-id', 'datasetId'],
    ['--approval-id', 'approvalId'],
    ['--arm', 'arm'],
    ['--timeout-ms', 'timeoutMs'],
    ['--poll-interval-ms', 'pollIntervalMs'],
    ['--poll-attempts', 'pollAttempts'],
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (['--self-test', '--preflight', '--execute'].includes(arg)) {
      if (args.mode) throw new Error('choose exactly one of --self-test, --preflight or --execute');
      args.mode = arg.slice(2);
    } else if (valueFlags.has(arg)) {
      const value = argv[index + 1];
      if (!value || value.startsWith('--')) throw new Error(`${arg} requires a value`);
      const field = valueFlags.get(arg);
      if (field === 'caseIds') {
        args.caseIds = value.split(',').map((item) => item.trim()).filter(Boolean);
        args.caseIdsExplicit = true;
      } else if (['timeoutMs', 'pollIntervalMs', 'pollAttempts'].includes(field)) {
        args[field] = Number(value);
      } else if (['fixturePath', 'outputDir'].includes(field)) {
        args[field] = resolve(value);
      } else {
        args[field] = value.trim();
      }
      index += 1;
    } else if (arg === '--ack-controlled-live') {
      args.ackControlledLive = true;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  if (!args.mode && !args.help) throw new Error('choose one mode: --self-test, --preflight or --execute');
  return args;
}

function assertFixedFixturePathBeforeRead(path) {
  if (resolve(path) !== defaultFixturePath) {
    throw new Error('fixture path is fixed; external or alternate paths are not allowed');
  }
}

function stableValue(value) {
  if (Array.isArray(value)) return value.map(stableValue);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(Object.keys(value).sort().map((key) => [key, stableValue(value[key])]));
}

function stableStringify(value) {
  return JSON.stringify(stableValue(value));
}

function sha256(value) {
  return createHash('sha256').update(String(value)).digest('hex');
}

function hashJson(value) {
  return sha256(stableStringify(value));
}

function normalizeBaseUrl(value) {
  const url = new URL(String(value || ''));
  if (url.username || url.password) throw new Error('--base-url must not contain credentials');
  const loopback = new Set(['localhost', '127.0.0.1', '[::1]', '::1']).has(url.hostname.toLowerCase());
  if (url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) {
    throw new Error('--base-url must use https; http is allowed only for loopback');
  }
  url.pathname = '/';
  url.search = '';
  url.hash = '';
  return url.origin;
}

function safeRelativePath(path) {
  const value = relative(repoRoot, path).replaceAll('\\', '/');
  return value.startsWith('..') ? '[outside-repository]' : value;
}

function authenticationEnvironment(env = process.env) {
  return {
    cookie: env[authEnvNames.cookie] || '',
    email: env[authEnvNames.email] || '',
    localKey: env[authEnvNames.localKey] || '',
    secretBindingIds: env[authEnvNames.secretBindingIds] || '',
    approvedIdentitySha256: env[authEnvNames.approvedIdentitySha256] || '',
  };
}

function authAvailability(credentials) {
  const cookie = sessionCookiePair(credentials.cookie);
  const emailAndKey = Boolean(credentials.email.trim() && credentials.localKey.trim());
  return {
    ready: Boolean(cookie || emailAndKey),
    method: cookie ? 'environment-session-cookie' : emailAndKey ? 'environment-local-key-login' : 'missing',
    secret_binding_id_count: normalizeSecretBindingIds(credentials.secretBindingIds).length,
  };
}

function sessionCookiePair(value) {
  const match = String(value || '').match(/(?:^|[;,]\s*)(aidp_v3_session=[^;,\s]+)/iu);
  return match?.[1] || '';
}

function normalizeSecretBindingIds(value) {
  const items = Array.isArray(value) ? value : String(value || '').split(',');
  return [...new Set(items.map((item) => String(item || '').trim()).filter(Boolean))];
}

async function readFixtures(path) {
  const content = await readFile(path, 'utf8');
  const rows = [];
  for (const [offset, line] of content.split(/\r?\n/u).entries()) {
    if (!line.trim()) continue;
    let value;
    try {
      value = JSON.parse(line);
    } catch {
      throw new Error(`fixture line ${offset + 1} is not valid JSON`);
    }
    if (!value || typeof value.case_id !== 'string' || typeof value.prompt !== 'string' || !value.prompt) {
      throw new Error(`fixture line ${offset + 1} requires case_id and prompt`);
    }
    rows.push({ case_id: value.case_id, prompt: value.prompt });
  }
  const ids = rows.map((row) => row.case_id);
  if (new Set(ids).size !== ids.length) throw new Error('fixture case_id values must be unique');
  return { rows, fileSha256: sha256(content) };
}

function selectCases(fixture, caseIds) {
  if (caseIds.length === 0) throw new Error('at least one case id is required');
  if (new Set(caseIds).size !== caseIds.length) throw new Error('--case-ids must not contain duplicates');
  const byId = new Map(fixture.rows.map((row) => [row.case_id, row]));
  const missing = caseIds.filter((caseId) => !byId.has(caseId));
  if (missing.length > 0) throw new Error(`unknown case ids: ${missing.join(',')}`);
  return caseIds.map((caseId) => byId.get(caseId));
}

function validationIssues(args, credentials, cases) {
  const issues = [];
  issues.push('live_execution_disabled_without_offline_promotion_candidate');
  if (!args.ackControlledLive) issues.push('ack_controlled_live_required');
  if (!args.approvalId || args.approvalId.length < 6) issues.push('approval_id_required');
  if (!Object.hasOwn(armModes, args.arm)) issues.push('arm_must_be_A_B_or_C');
  try {
    normalizeBaseUrl(args.baseUrl);
  } catch {
    issues.push('base_url_invalid');
  }
  if (!uuidPattern.test(args.datasetId)) issues.push('dataset_id_must_be_canonical_uuid');
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 10_000) issues.push('timeout_ms_invalid');
  if (!Number.isInteger(args.pollIntervalMs) || args.pollIntervalMs < 250) issues.push('poll_interval_ms_invalid');
  if (!Number.isInteger(args.pollAttempts) || args.pollAttempts < 2) issues.push('poll_attempts_must_be_at_least_2');
  if (!authAvailability(credentials).ready) issues.push('environment_auth_required');
  if (!/^[0-9a-f]{64}$/iu.test(credentials.approvedIdentitySha256.trim())) {
    issues.push('approved_identity_sha256_required');
  }
  if (!Array.isArray(cases) || cases.length === 0) issues.push('selected_cases_required');
  if (resolve(args.fixturePath) !== defaultFixturePath) issues.push('fixture_must_be_registered_default');
  if (cases.length > maxLiveCaseCount) issues.push('selected_case_count_exceeds_registered_limit');
  if (cases.some((item) => !registeredCaseIds.has(item.case_id))) {
    issues.push('selected_cases_must_be_registered');
  }
  if (cases.some((item) => item.prompt.length > maxPromptChars)) {
    issues.push('selected_case_prompt_too_long');
  }
  if (!privateReceiptPermissionsSupported(args.outputDir)) {
    issues.push('private_receipt_output_requires_linux_posix_filesystem');
  }
  if (resolve(args.outputDir) !== defaultOutputDir) {
    issues.push('output_dir_must_be_fixed_private_root');
  }
  return issues;
}

function privateReceiptPermissionsSupported(outputDir) {
  if (process.platform !== 'linux') return false;
  const normalized = resolve(outputDir).replaceAll('\\', '/');
  return !normalized.startsWith('/mnt/');
}

function buildPreflight(args, fixture, cases, credentials) {
  const issues = validationIssues(args, credentials, cases);
  return {
    smoke: 'semantic-supply-main-live',
    schema_version: 'semantic-supply-main-live-preflight.v1',
    mode: 'preflight',
    command_ready: issues.length === 0,
    network_requests: 0,
    filesystem_writes: 0,
    arm: args.arm || null,
    expected_semantic_mode: armModes[args.arm] || null,
    approval_id_sha256: args.approvalId ? sha256(args.approvalId) : null,
    target_origin_sha256: args.baseUrl ? sha256(normalizeBaseUrl(args.baseUrl)) : null,
    dataset_id_sha256: args.datasetId ? sha256(args.datasetId.toLowerCase()) : null,
    fixture_path: safeRelativePath(args.fixturePath),
    fixture_sha256: fixture.fileSha256,
    case_count: cases.length,
    case_set_sha256: hashJson(cases.map((item) => ({
      case_id: item.case_id,
      prompt_sha256: sha256(item.prompt),
    }))),
    auth: authAvailability(credentials),
    request_contract: {
      endpoint: '/api/v3/assistant-runs/stream',
      detail_endpoint_pattern: '/api/v3/assistant-runs/{run_id}',
      stream_request_count_per_case: 1,
      stream_retry_count: 0,
      concurrency: 1,
      payload_keys: ['local_thread_id', 'prompt', 'selected_scope'],
      selected_scope_keys: ['datasets', 'mode'],
      external_channel_used: false,
    },
    issues,
  };
}

function sanitizeError(error, secrets = [], prompts = []) {
  let text = error instanceof Error ? error.message : String(error || 'unknown_error');
  for (const secret of [...secrets, ...prompts].filter((item) => typeof item === 'string' && item.length >= 4)) {
    text = text.split(secret).join('[redacted]');
  }
  return text
    .replace(/aidp_v3_session=[^;"'\s]+/giu, '[redacted-session-cookie]')
    .replace(/"local_key"\s*:\s*"[^"]+"/giu, '"local_key":"[redacted]"')
    .slice(0, 500);
}

async function fetchWithTimeout(fetchImpl, url, options, timeoutMs, parentSignal = null) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(new Error('request_timeout')), timeoutMs);
  const onAbort = () => controller.abort(parentSignal.reason || new Error('request_aborted'));
  if (parentSignal) {
    if (parentSignal.aborted) onAbort();
    else parentSignal.addEventListener('abort', onAbort, { once: true });
  }
  try {
    return await fetchImpl(url, { ...options, redirect: 'error', signal: controller.signal });
  } finally {
    clearTimeout(timeout);
    parentSignal?.removeEventListener('abort', onAbort);
  }
}

async function responsePayload(response) {
  const contentType = response.headers.get('content-type') || '';
  if (contentType.includes('application/json')) return response.json();
  return response.text();
}

function authHeaders(authentication, localThreadId = null, accept = 'application/json') {
  const headers = { accept };
  if (authentication?.cookie) headers.cookie = authentication.cookie;
  if (authentication?.secretBindingIds?.length > 0) {
    headers['x-ai-data-platform-secret-binding-ids'] = authentication.secretBindingIds.join(',');
  }
  if (localThreadId) headers['x-ai-data-platform-local-thread-id'] = localThreadId;
  return headers;
}

function identityMaterial(payload) {
  const user = payload?.user || {};
  const session = payload?.session || {};
  const id = user.id || session.user_id;
  if (!id) return null;
  return { user_id: String(id) };
}

async function prepareAuthentication(args, dependencies) {
  const {
    fetchImpl,
    credentials,
    signal,
    authAttempt,
    registerPartialAuthentication,
    recordJournal,
  } = dependencies;
  const baseUrl = normalizeBaseUrl(args.baseUrl);
  const existingCookie = sessionCookiePair(credentials.cookie);
  if (existingCookie) {
    await recordJournal({ event: 'existing_session_validation_started' });
    const response = await fetchWithTimeout(fetchImpl, `${baseUrl}/api/v3/auth/session`, {
      method: 'GET',
      headers: { accept: 'application/json', cookie: existingCookie },
    }, Math.min(args.timeoutMs, 30_000), signal);
    const payload = await responsePayload(response);
    if (!response.ok || !identityMaterial(payload)) throw new Error(`existing session validation failed: HTTP ${response.status}`);
    return {
      method: 'environment-session-cookie',
      cookie: existingCookie,
      createdSession: false,
      secretBindingIds: normalizeSecretBindingIds(credentials.secretBindingIds),
      identitySha256: hashJson(identityMaterial(payload)),
      loginStatus: null,
      logoutStatus: 'not-applicable',
      logoutVerified: null,
    };
  }

  await recordJournal({
    event: 'local_key_login_attempt',
    approved_identity_sha256: credentials.approvedIdentitySha256.trim().toLowerCase(),
  });
  authAttempt.loginPostDispatched = true;
  let response;
  try {
    response = await fetchWithTimeout(fetchImpl, `${baseUrl}/api/v3/auth/key/login`, {
      method: 'POST',
      headers: { accept: 'application/json', 'content-type': 'application/json' },
      body: JSON.stringify({
        email: credentials.email.trim().toLowerCase(),
        local_key: credentials.localKey.trim(),
        device_fingerprint: `semantic-supply-main-live-${randomUUID()}`,
      }),
    }, Math.min(args.timeoutMs, 30_000), signal);
    authAttempt.responseReceived = true;
    authAttempt.responseStatus = response.status;
  } catch (error) {
    authAttempt.sessionSideEffectUnknown = true;
    throw error;
  }
  const setCookieValues = typeof response.headers.getSetCookie === 'function'
    ? response.headers.getSetCookie()
    : [response.headers.get('set-cookie') || ''];
  const cookie = setCookieValues.map(sessionCookiePair).find(Boolean) || '';
  let partialAuthentication = null;
  if (cookie) {
    partialAuthentication = {
      method: 'environment-local-key-login',
      cookie,
      createdSession: true,
      secretBindingIds: [],
      identitySha256: null,
      loginStatus: response.status,
      logoutStatus: 'pending',
      logoutVerified: false,
    };
    registerPartialAuthentication(partialAuthentication);
    await recordJournal({
      event: 'local_key_session_cookie_observed',
      session_cookie_sha256: sha256(cookie),
      login_status: response.status,
    });
  } else {
    authAttempt.sessionSideEffectUnknown = true;
  }
  let payload;
  try {
    payload = await responsePayload(response);
  } catch (error) {
    if (!cookie) authAttempt.sessionSideEffectUnknown = true;
    throw error;
  }
  if (!response.ok) throw new Error(`local-key login failed: HTTP ${response.status}`);
  if (!cookie) throw new Error('local-key login did not return aidp_v3_session');
  partialAuthentication.secretBindingIds = normalizeSecretBindingIds(payload?.active_secret_binding_ids);
  partialAuthentication.identitySha256 = identityMaterial(payload)
    ? hashJson(identityMaterial(payload))
    : null;
  return partialAuthentication;
}

async function revokeCreatedSession(args, authentication, fetchImpl, recordJournal = async () => {}) {
  if (!authentication?.createdSession) return authentication;
  const baseUrl = normalizeBaseUrl(args.baseUrl);
  try {
    await recordJournal({
      event: 'created_session_logout_attempt',
      session_cookie_sha256: sha256(authentication.cookie),
    });
    const response = await fetchWithTimeout(fetchImpl, `${baseUrl}/api/v3/auth/logout`, {
      method: 'POST',
      headers: { accept: 'application/json', cookie: authentication.cookie },
    }, 30_000);
    authentication.logoutStatus = response.status;
    if (!response.ok) return authentication;
    const verify = await fetchWithTimeout(fetchImpl, `${baseUrl}/api/v3/auth/session`, {
      method: 'GET',
      headers: { accept: 'application/json', cookie: authentication.cookie },
    }, 30_000);
    const payload = await responsePayload(verify);
    authentication.logoutVerified = verify.ok && logoutPayloadConfirmsRevoked(payload);
    await recordJournal({
      event: 'created_session_logout_result',
      logout_status: authentication.logoutStatus,
      logout_verified: authentication.logoutVerified,
    });
  } catch (error) {
    authentication.logoutStatus = 'request-failed';
    authentication.logoutVerified = false;
    authentication.logoutError = sanitizeError(error);
  }
  return authentication;
}

function logoutPayloadConfirmsRevoked(payload) {
  return Boolean(
    payload
      && typeof payload === 'object'
      && !Array.isArray(payload)
      && Object.hasOwn(payload, 'user')
      && Object.hasOwn(payload, 'session')
      && payload.user === null
      && payload.session === null,
  );
}

function buildMinimalPayload(prompt, datasetId, localThreadId) {
  const payload = {
    prompt,
    local_thread_id: localThreadId,
    selected_scope: {
      mode: 'user_selected',
      datasets: [datasetId],
    },
  };
  assertMinimalPayload(payload, prompt, datasetId, localThreadId);
  return payload;
}

function assertMinimalPayload(payload, prompt, datasetId, localThreadId) {
  const keys = Object.keys(payload).sort();
  const scopeKeys = Object.keys(payload.selected_scope || {}).sort();
  if (stableStringify(keys) !== stableStringify(['local_thread_id', 'prompt', 'selected_scope'])) {
    throw new Error('stream payload contains non-minimal top-level fields');
  }
  if (stableStringify(scopeKeys) !== stableStringify(['datasets', 'mode'])) {
    throw new Error('selected_scope contains non-minimal fields');
  }
  if (payload.prompt !== prompt || payload.local_thread_id !== localThreadId) {
    throw new Error('stream payload altered the prompt or local thread id');
  }
  if (payload.selected_scope.mode !== 'user_selected'
    || stableStringify(payload.selected_scope.datasets) !== stableStringify([datasetId])) {
    throw new Error('stream payload did not preserve the exact selected dataset');
  }
  const forbiddenKeys = new Set([
    'intent',
    'policy',
    'supply_policy',
    'route',
    'action',
    'answer',
    'answer_template',
    'startup_briefing',
    'scope_candidates',
    'current_artifact',
    'messages',
  ]);
  const visit = (value) => {
    if (!value || typeof value !== 'object') return;
    for (const [key, nested] of Object.entries(value)) {
      if (forbiddenKeys.has(key)) throw new Error(`forbidden request field: ${key}`);
      visit(nested);
    }
  };
  visit(payload);
}

function parseSseFrame(block) {
  const frame = { event: 'message', data: '', json: null };
  const dataLines = [];
  for (const line of String(block || '').split(/\r?\n/u)) {
    if (!line || line.startsWith(':')) continue;
    const separator = line.indexOf(':');
    const field = separator >= 0 ? line.slice(0, separator) : line;
    const rawValue = separator >= 0 ? line.slice(separator + 1) : '';
    const value = rawValue.startsWith(' ') ? rawValue.slice(1) : rawValue;
    if (field === 'event') frame.event = value || 'message';
    else if (field === 'data') dataLines.push(value);
  }
  frame.data = dataLines.join('\n');
  try {
    frame.json = frame.data ? JSON.parse(frame.data) : null;
  } catch {
    frame.json = null;
  }
  return frame;
}

function parseSseFrames(state, text) {
  state.buffer += text;
  const frames = [];
  let separatorIndex;
  while ((separatorIndex = state.buffer.search(/\r?\n\r?\n/u)) >= 0) {
    const block = state.buffer.slice(0, separatorIndex);
    const match = state.buffer.slice(separatorIndex).match(/^\r?\n\r?\n/u);
    state.buffer = state.buffer.slice(separatorIndex + (match ? match[0].length : 2));
    if (block.trim()) frames.push(parseSseFrame(block));
  }
  return frames;
}

function responseFromCompleted(frame) {
  return frame?.json?.response || frame?.json?.data?.response || null;
}

function runIdFromResponse(response) {
  return response?.assistant_run_id || response?.run?.id || null;
}

function answerFromResponse(response) {
  return String(
    response?.assistant_message?.content
      || response?.response?.assistant_message?.content
      || response?.run?.assistant_message?.content
      || '',
  );
}

function histogram(values) {
  const counts = {};
  for (const value of values) counts[value] = (counts[value] || 0) + 1;
  return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right)));
}

async function postAssistantStream(args, testCase, authentication, fetchImpl, signal, attempt) {
  const { localThreadId, requestPayload: payload } = attempt;
  const startedAt = Date.now();
  attempt.postDispatched = true;
  attempt.dispatchedAt = new Date().toISOString();
  let response;
  try {
    response = await fetchWithTimeout(fetchImpl, `${normalizeBaseUrl(args.baseUrl)}/api/v3/assistant-runs/stream`, {
      method: 'POST',
      headers: {
        ...authHeaders(authentication, localThreadId, 'text/event-stream'),
        'content-type': 'application/json',
      },
      body: JSON.stringify(payload),
    }, args.timeoutMs, signal);
    attempt.responseReceived = true;
    attempt.responseStatus = response.status;
  } catch (error) {
    attempt.transportOutcome = 'unknown_after_dispatch';
    throw error;
  }
  if (!response.ok || !response.body) {
    const body = await response.text();
    attempt.transportOutcome = 'http_non_success';
    throw new Error(`stream failed: HTTP ${response.status}: ${body.slice(0, 300)}`);
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const state = { buffer: '' };
  const frames = [];
  const deltaTexts = [];
  let completedResponse = null;
  while (true) {
    const { done, value } = await reader.read();
    const chunk = decoder.decode(value || new Uint8Array(), { stream: !done });
    for (const frame of parseSseFrames(state, chunk)) {
      frame.at_ms = Date.now() - startedAt;
      frames.push(frame);
      if (frame.event === 'assistant_run.delta') {
        deltaTexts.push(String(frame.json?.delta || frame.json?.data?.delta || ''));
      } else if (frame.event === 'assistant_run.completed') {
        completedResponse = responseFromCompleted(frame);
      }
    }
    if (done) break;
  }
  state.buffer += decoder.decode();
  if (state.buffer.trim()) {
    const trailing = parseSseFrame(state.buffer);
    state.buffer = '';
    trailing.at_ms = Date.now() - startedAt;
    frames.push(trailing);
    if (trailing.event === 'assistant_run.completed') completedResponse = responseFromCompleted(trailing);
  }
  const eventNames = frames.map((frame) => frame.event);
  const runId = runIdFromResponse(completedResponse);
  const answer = answerFromResponse(completedResponse);
  const combinedDeltas = deltaTexts.join('');
  const duplicateFinalDeltaLikely = Boolean(
    answer && combinedDeltas.length >= answer.length * 2 && combinedDeltas.includes(answer + answer),
  );
  const checks = {
    http_200: response.status === 200,
    accepted_once: eventNames.filter((name) => name === 'assistant_run.accepted').length === 1,
    completed_once: eventNames.filter((name) => name === 'assistant_run.completed').length === 1,
    done_once: eventNames.filter((name) => name === 'done').length === 1,
    no_error_event: !eventNames.some((name) => (
      name === 'error' || name === 'assistant_run.failed' || name === 'assistant_run.error'
    )),
    run_id_present: Boolean(runId),
    answer_present: answer.length > 0,
    no_duplicate_final_delta: !duplicateFinalDeltaLikely,
    unparsed_buffer_empty: !state.buffer.trim(),
  };
  attempt.transportOutcome = Object.values(checks).every(Boolean) ? 'terminal_contract_captured' : 'terminal_contract_failed';
  attempt.runId = runId;
  return {
    ok: Object.values(checks).every(Boolean),
    httpStatus: response.status,
    localThreadId,
    requestPayload: payload,
    frames,
    eventNames,
    eventHistogram: histogram(eventNames),
    runId,
    answer,
    completedResponse,
    deltaChars: combinedDeltas.length,
    latencyMs: Date.now() - startedAt,
    firstDeltaAtMs: frames.find((frame) => frame.event === 'assistant_run.delta')?.at_ms ?? null,
    completedAtMs: frames.find((frame) => frame.event === 'assistant_run.completed')?.at_ms ?? null,
    duplicateFinalDeltaLikely,
    checks,
  };
}

function detailStabilityMaterial(detail) {
  const run = detail?.run || {};
  const events = Array.isArray(detail?.events)
    ? [...detail.events].sort((left, right) => (
      Number(left?.sequence_no ?? 0) - Number(right?.sequence_no ?? 0)
    ))
    : [];
  return {
    run_id: run.id || null,
    run_updated_at: run.updated_at || null,
    service_lane: run.service_lane || null,
    selected_scope_sha256: hashJson(run.selected_scope || null),
    evidence_state_sha256: hashJson(run.evidence_state || null),
    execution_trail_sha256: hashJson(run.execution_trail || null),
    runtime_sha256: hashJson(run.runtime || null),
    output_artifacts_sha256: hashJson(run.output_artifacts || []),
    event_count: events.length,
    events: events.map((event) => ({
      id: event.id || null,
      event_name: event.event_name || null,
      sequence_no: event.sequence_no ?? null,
      payload_sha256: hashJson(event.payload || null),
    })),
    diagnostics_sha256: hashJson(detail?.diagnostics || null),
  };
}

function detailTerminalObservation(detail) {
  const eventNames = (Array.isArray(detail?.events) ? detail.events : [])
    .map((event) => event?.event_name)
    .filter((value) => typeof value === 'string');
  const completed = eventNames.includes('assistant_run.completed');
  const terminalFailure = eventNames.some((name) => (
    name === 'assistant_run.failed' || name === 'assistant_run.error'
  ));
  return { completed, terminal_failure: terminalFailure, ready: completed && !terminalFailure };
}

async function getAssistantRunDetail(args, runId, localThreadId, authentication, fetchImpl, signal) {
  const response = await fetchWithTimeout(
    fetchImpl,
    `${normalizeBaseUrl(args.baseUrl)}/api/v3/assistant-runs/${encodeURIComponent(runId)}`,
    { method: 'GET', headers: authHeaders(authentication, localThreadId) },
    Math.min(args.timeoutMs, 60_000),
    signal,
  );
  const payload = await responsePayload(response);
  if (!response.ok) throw new Error(`assistant-run detail failed: HTTP ${response.status}`);
  return payload;
}

async function wait(ms, signal) {
  if (signal?.aborted) throw signal.reason || new Error('request_aborted');
  await new Promise((resolveWait, reject) => {
    const finish = () => {
      signal?.removeEventListener('abort', onAbort);
      resolveWait();
    };
    const timeout = setTimeout(finish, ms);
    const onAbort = () => {
      clearTimeout(timeout);
      signal?.removeEventListener('abort', onAbort);
      reject(signal.reason || new Error('request_aborted'));
    };
    signal?.addEventListener('abort', onAbort, { once: true });
  });
}

async function pollStableDetail(args, stream, authentication, fetchImpl, signal) {
  let previousSignature = null;
  let consecutiveStable = 0;
  let finalDetail = null;
  const polls = [];
  for (let attempt = 1; attempt <= args.pollAttempts; attempt += 1) {
    if (attempt > 1) await wait(args.pollIntervalMs, signal);
    finalDetail = await getAssistantRunDetail(
      args,
      stream.runId,
      stream.localThreadId,
      authentication,
      fetchImpl,
      signal,
    );
    const signature = hashJson(detailStabilityMaterial(finalDetail));
    const terminal = detailTerminalObservation(finalDetail);
    consecutiveStable = terminal.ready && signature === previousSignature ? consecutiveStable + 1 : 1;
    polls.push({ attempt, signature, consecutive_stable: consecutiveStable, terminal });
    previousSignature = signature;
    if (terminal.ready && consecutiveStable >= 2) {
      return { stable: true, detail: finalDetail, polls, stableSignature: signature };
    }
  }
  return { stable: false, detail: finalDetail, polls, stableSignature: previousSignature };
}

function uniqueCanonical(items) {
  const byHash = new Map();
  for (const item of items) byHash.set(hashJson(item), item);
  return [...byHash.entries()].sort(([left], [right]) => left.localeCompare(right));
}

function providerObservation(detail) {
  const usage = detail?.diagnostics?.provider_usage || {};
  const recentEvents = Array.isArray(usage.recent_events) ? usage.recent_events : [];
  const runtime = detail?.run?.runtime || {};
  const pairs = [];
  const addPair = (provider, model) => {
    if (typeof provider === 'string' && provider && typeof model === 'string' && model) {
      pairs.push({ provider, model });
    }
  };
  addPair(runtime.provider, runtime.model);
  for (const event of recentEvents) {
    addPair(event.provider || event.runtime?.provider, event.model || event.runtime?.model);
  }
  const uniquePairs = uniqueCanonical(pairs).map(([, pair]) => pair);
  const providerLanes = [...new Set([
    runtime.lane,
    ...recentEvents.map((event) => event?.lane),
  ].filter((value) => typeof value === 'string' && value))].sort();
  const requestCount = Number.isInteger(usage?.summary?.request_count)
    ? usage.summary.request_count
    : null;
  return {
    provider_model_pairs: uniquePairs,
    provider_model_pair_count: uniquePairs.length,
    provider_lane_count: providerLanes.length,
    provider_lane_hashes: providerLanes.map((lane) => sha256(lane)),
    provider_runtime_snapshot_count: requestCount,
    provider_call_count_exact: null,
    recent_provider_runtime_snapshot_count: recentEvents.length,
    recent_provider_runtime_snapshots_truncated: requestCount === null
      ? null
      : requestCount > recentEvents.length,
    failed_provider_runtime_snapshot_count: Number.isInteger(usage?.summary?.failed_request_count)
      ? usage.summary.failed_request_count
      : null,
    retry_observability: {
      status: 'not_exactly_observable',
      exact_retry_count: null,
      reason_code: 'inner_same_provider_attempts_not_persisted_per_run',
    },
  };
}

function routeObservation(detail) {
  const serviceLane = typeof detail?.run?.service_lane === 'string'
    ? detail.run.service_lane
    : null;
  const eventServiceLanes = [...new Set((Array.isArray(detail?.events) ? detail.events : [])
    .filter((event) => ['assistant_run.started', 'assistant_run.completed'].includes(event?.event_name))
    .map((event) => event?.payload?.service_lane)
    .filter((value) => typeof value === 'string' && value))].sort();
  return {
    main_route_count: serviceLane ? 1 : 0,
    service_lane_sha256: serviceLane ? sha256(serviceLane) : null,
    terminal_event_service_lane_count: eventServiceLanes.length,
    terminal_event_service_lane_hashes: eventServiceLanes.map((value) => sha256(value)),
    terminal_event_service_lane_matches: eventServiceLanes.length === 0
      ? null
      : eventServiceLanes.every((value) => value === serviceLane),
  };
}

function collectWorkflowIds(detail) {
  const directWorkflowId = (value) => {
    const candidate = value?.workflow_execution_id || value?.workflowExecutionId;
    return typeof candidate === 'string' && uuidPattern.test(candidate) ? candidate : null;
  };
  const ids = [];
  for (const item of Array.isArray(detail?.run?.execution_trail) ? detail.run.execution_trail : []) {
    const id = directWorkflowId(item);
    if (id) ids.push(id);
  }
  for (const event of Array.isArray(detail?.events) ? detail.events : []) {
    if (!workflowEventNames.has(event?.event_name)) continue;
    const id = directWorkflowId(event?.payload);
    if (id) ids.push(id);
  }
  for (const artifact of Array.isArray(detail?.run?.output_artifacts) ? detail.run.output_artifacts : []) {
    const candidates = [
      directWorkflowId(artifact),
      directWorkflowId(artifact?.artifact_manifest?.refs),
      directWorkflowId(artifact?.artifactManifest?.refs),
      directWorkflowId(artifact?.payload),
      directWorkflowId(artifact?.payload?.artifact_manifest?.refs),
      directWorkflowId(artifact?.payload?.artifactManifest?.refs),
    ].filter(Boolean);
    for (const ref of Array.isArray(artifact?.data_refs) ? artifact.data_refs : []) {
      if (ref?.kind === 'workflow_execution'
        && typeof ref?.id === 'string'
        && uuidPattern.test(ref.id)) {
        candidates.push(ref.id);
      }
    }
    ids.push(...candidates);
  }
  return [...new Set(ids)].sort();
}

function workflowObservation(detail) {
  const unique = collectWorkflowIds(detail);
  return {
    workflow_count: unique.length,
    workflow_id_hashes: unique.map((id) => sha256(id)),
  };
}

function evidenceObservation(detail, completedResponse) {
  const detailState = detail?.run?.evidence_state;
  const completedState = completedResponse?.evidence_state;
  const evidenceState = detailState && typeof detailState === 'object'
    ? detailState
    : completedState && typeof completedState === 'object' ? completedState : {};
  const suppliedItems = Array.isArray(evidenceState.supplied_items) ? evidenceState.supplied_items : [];
  const sources = suppliedItems
    .map((item) => Object.fromEntries(
      suppliedItemSourceFields
        .filter((field) => item?.[field] !== undefined && item?.[field] !== null)
        .map((field) => [field, item[field]]),
    ))
    .filter((source) => Object.keys(source).some((field) => field !== 'type'));
  const quality = evidenceState.supply_quality && typeof evidenceState.supply_quality === 'object'
    ? evidenceState.supply_quality
    : {};
  const locators = Array.isArray(quality.citationLocators)
    ? quality.citationLocators.filter((value) => typeof value === 'string' && value.trim())
    : [];
  return {
    evidence_state_status_sha256: typeof evidenceState.status === 'string'
      ? sha256(evidenceState.status)
      : null,
    supplied_item_count: suppliedItems.length,
    supplied_items_sha256: hashJson(suppliedItems),
    source_reference_count: sources.length,
    source_references_sha256: hashJson(sources),
    citable_locator_count: locators.length,
    declared_citable_locator_count: Number.isInteger(quality.citationLocatorCount)
      ? quality.citationLocatorCount
      : null,
    declared_citable_locator_count_matches: Number.isInteger(quality.citationLocatorCount)
      ? quality.citationLocatorCount === locators.length
      : null,
    citable_locators_sha256: hashJson(locators),
    used_citation_count: null,
    used_citation_observability: 'not_contracted',
  };
}

function detailBindingIssues(detail, stream, attempt, testCase) {
  const issues = [];
  if (detail?.run?.id !== stream?.runId) issues.push('detail_run_id_mismatch');
  if (detail?.run?.local_thread_id !== attempt?.localThreadId) {
    issues.push('detail_local_thread_id_mismatch');
  }
  if (detail?.run?.user_prompt !== testCase?.prompt) issues.push('detail_user_prompt_mismatch');
  const events = Array.isArray(detail?.events) ? detail.events : [];
  if (events.some((event) => event?.run_id !== stream?.runId)) {
    issues.push('detail_event_run_id_mismatch');
  }
  return issues;
}

function artifactObservation(detail) {
  const artifacts = Array.isArray(detail?.run?.output_artifacts) ? detail.run.output_artifacts : [];
  const sideEffects = artifacts.filter((artifact) => artifact?.type !== 'assistant_message');
  const validManifests = artifacts.flatMap((artifact) => [
    artifact?.artifact_manifest,
    artifact?.artifactManifest,
    artifact?.payload?.artifact_manifest,
    artifact?.payload?.artifactManifest,
  ]).filter((manifest) => (
    manifest?.schema === 'v3.output_artifact_manifest'
      && Number(manifest?.schema_version ?? manifest?.schemaVersion) === 1
  ));
  return {
    total_count: artifacts.length,
    side_effect_count: sideEffects.length,
    valid_output_manifest_count: validManifests.length,
    artifacts_sha256: hashJson(artifacts),
    side_effect_artifacts_sha256: hashJson(sideEffects),
  };
}

function buildCaseSummary(testCase, attempt, stream, detailResult, error = null) {
  const detail = detailResult?.detail || null;
  const artifacts = Array.isArray(detail?.run?.output_artifacts) ? detail.run.output_artifacts : [];
  const assistantMessages = artifacts.filter((artifact) => artifact?.type === 'assistant_message');
  const detailEvents = Array.isArray(detail?.events) ? detail.events : [];
  const detailAnswer = assistantMessages.length === 1 && typeof assistantMessages[0]?.content === 'string'
    ? assistantMessages[0].content
    : '';
  const answer = stream?.answer || detailAnswer;
  return {
    case_id: testCase.case_id,
    question_sha256: sha256(testCase.prompt),
    question_chars: testCase.prompt.length,
    status: !error && stream?.ok && detailResult?.stable ? 'captured' : 'failed',
    error: error ? { error_sha256: sha256(error), error_chars: error.length } : null,
    run_id_sha256: stream?.runId ? sha256(stream.runId) : null,
    request_attempt: {
      post_dispatched: attempt?.postDispatched === true,
      response_received: attempt?.responseReceived === true,
      response_status: attempt?.responseStatus ?? null,
      transport_outcome: attempt?.transportOutcome || 'not_dispatched',
      local_thread_id_sha256: attempt?.localThreadId ? sha256(attempt.localThreadId) : null,
      request_payload_sha256: attempt?.requestPayload ? hashJson(attempt.requestPayload) : null,
      run_id_observed: Boolean(attempt?.runId),
      uncertain_run_created: attempt?.postDispatched === true && !attempt?.runId,
      provider_side_effect_unknown: attempt?.postDispatched === true && !attempt?.runId,
    },
    answer_sha256: answer ? sha256(answer) : null,
    answer_chars: answer.length,
    sse: stream ? {
      http_status: stream.httpStatus,
      event_count: stream.eventNames.length,
      event_histogram: stream.eventHistogram,
      event_order_sha256: hashJson(stream.eventNames),
      delta_chars: stream.deltaChars,
      latency_ms: stream.latencyMs,
      first_delta_at_ms: stream.firstDeltaAtMs,
      completed_at_ms: stream.completedAtMs,
      checks: stream.checks,
    } : null,
    detail: detailResult ? {
      stable: detailResult.stable,
      poll_count: detailResult.polls.length,
      stable_consecutive_required: 2,
      stability_signature_sha256: detailResult.stableSignature,
      terminal: detailTerminalObservation(detail),
      detail_event_count: detailEvents.length,
      detail_event_histogram: histogram(detailEvents.map((event) => (
        event.event_name || event.name || event.kind || 'unknown'
      ))),
      assistant_message_artifact_count: assistantMessages.length,
      assistant_message_matches_stream: stream?.answer && detailAnswer
        ? stream.answer === detailAnswer
        : null,
    } : null,
    evidence: evidenceObservation(detail, stream?.completedResponse),
    provider: providerObservation(detail),
    route: routeObservation(detail),
    workflow: workflowObservation(detail),
    artifacts: artifactObservation(detail),
  };
}

function publicAuthSummary(authentication) {
  return {
    method: authentication?.method || 'not_authenticated',
    created_session: authentication?.createdSession === true,
    identity_sha256: authentication?.identitySha256 || null,
    secret_binding_id_count: authentication?.secretBindingIds?.length || 0,
    secret_binding_ids_sha256: hashJson(
      [...(authentication?.secretBindingIds || [])].sort().map((id) => sha256(id)),
    ),
    login_status: authentication?.loginStatus ?? null,
    logout_status: authentication?.logoutStatus ?? 'not-applicable',
    logout_verified: authentication?.logoutVerified ?? null,
    logout_error: authentication?.logoutError ? {
      error_sha256: sha256(authentication.logoutError),
      error_chars: authentication.logoutError.length,
    } : null,
  };
}

async function collectControlledLive(args, fixture, cases, dependencies) {
  const { fetchImpl, credentials, signal } = dependencies;
  const recordJournal = dependencies.recordJournal || (async () => {});
  const startedAt = new Date().toISOString();
  const rawCases = [];
  const summaries = [];
  const errors = [];
  let authentication = null;
  let fatalError = null;
  const authAttempt = {
    loginPostDispatched: false,
    responseReceived: false,
    responseStatus: null,
    sessionSideEffectUnknown: false,
  };
  try {
    authentication = await prepareAuthentication(args, {
      fetchImpl,
      credentials,
      signal,
      authAttempt,
      recordJournal,
      registerPartialAuthentication: (value) => {
        authentication = value;
      },
    });
    if (!authentication.identitySha256) throw new Error('authenticated identity could not be hashed');
    if (authentication.identitySha256 !== credentials.approvedIdentitySha256.trim().toLowerCase()) {
      throw new Error('authenticated identity does not match approved identity hash');
    }
    for (const testCase of cases) {
      let stream = null;
      let detailResult = null;
      let caseError = null;
      const localThreadId = `semantic-supply-${testCase.case_id}-${randomUUID()}`;
      const attempt = {
        caseId: testCase.case_id,
        localThreadId,
        requestPayload: buildMinimalPayload(testCase.prompt, args.datasetId, localThreadId),
        postDispatched: false,
        responseReceived: false,
        responseStatus: null,
        transportOutcome: 'not_dispatched',
        runId: null,
      };
      try {
        await recordJournal({
          event: 'assistant_stream_attempt',
          case_id: testCase.case_id,
          question_sha256: sha256(testCase.prompt),
          local_thread_id: attempt.localThreadId,
          request_payload_sha256: hashJson(attempt.requestPayload),
        });
        stream = await postAssistantStream(
          args,
          testCase,
          authentication,
          fetchImpl,
          signal,
          attempt,
        );
        await recordJournal({
          event: 'assistant_stream_terminal_observed',
          case_id: testCase.case_id,
          local_thread_id: attempt.localThreadId,
          run_id: attempt.runId,
          transport_outcome: attempt.transportOutcome,
        });
        if (!stream.ok) throw new Error('SSE terminal contract failed');
        detailResult = await pollStableDetail(args, stream, authentication, fetchImpl, signal);
        if (!detailResult.stable) throw new Error('assistant-run detail did not stabilize twice');
        const bindingIssues = detailBindingIssues(
          detailResult.detail,
          stream,
          attempt,
          testCase,
        );
        if (bindingIssues.length > 0) {
          throw new Error(`assistant-run detail binding failed: ${bindingIssues.join(',')}`);
        }
        const selectedDatasets = detailResult.detail?.run?.selected_scope?.datasets || [];
        if (stableStringify(selectedDatasets) !== stableStringify([args.datasetId])) {
          throw new Error('detail selected_scope did not preserve the exact dataset');
        }
      } catch (error) {
        caseError = sanitizeError(
          error,
          [credentials.cookie, credentials.email, credentials.localKey],
          cases.map((item) => item.prompt),
        );
        await recordJournal({
          event: 'assistant_stream_attempt_failed',
          case_id: testCase.case_id,
          local_thread_id: attempt.localThreadId,
          run_id: attempt.runId,
          post_dispatched: attempt.postDispatched,
          transport_outcome: attempt.transportOutcome,
          error_sha256: sha256(caseError),
        });
        errors.push({ case_id: testCase.case_id, error: caseError });
      }
      summaries.push(buildCaseSummary(testCase, attempt, stream, detailResult, caseError));
      rawCases.push({
        case_id: testCase.case_id,
        prompt: testCase.prompt,
        local_thread_id: attempt.localThreadId,
        request_payload: attempt.requestPayload,
        stream_attempt: {
          post_dispatched: attempt.postDispatched,
          dispatched_at: attempt.dispatchedAt || null,
          response_received: attempt.responseReceived,
          response_status: attempt.responseStatus,
          transport_outcome: attempt.transportOutcome,
          run_id: attempt.runId,
          uncertain_run_created: attempt.postDispatched && !attempt.runId,
          provider_side_effect_unknown: attempt.postDispatched && !attempt.runId,
        },
        stream: stream ? {
          ...stream,
          answer: stream.answer,
        } : null,
        detail_poll: detailResult ? {
          stable: detailResult.stable,
          polls: detailResult.polls,
          stable_signature: detailResult.stableSignature,
          final_detail: detailResult.detail,
        } : null,
        error: caseError,
      });
      if (caseError) break;
    }
  } catch (error) {
    fatalError = sanitizeError(
      error,
      [credentials.cookie, credentials.email, credentials.localKey],
      cases.map((item) => item.prompt),
    );
    await recordJournal({ event: 'controlled_live_fatal', error_sha256: sha256(fatalError) });
    errors.push({ case_id: null, error: fatalError });
  } finally {
    if (authentication?.createdSession) {
      await revokeCreatedSession(args, authentication, fetchImpl, recordJournal);
    }
  }
  const finishedAt = new Date().toISOString();
  const auth = {
    ...publicAuthSummary(authentication),
    auth_session_side_effect_unknown: authAttempt.sessionSideEffectUnknown,
  };
  if (auth.created_session && auth.logout_verified !== true) {
    errors.push({ case_id: null, error: 'created_session_logout_not_verified' });
  }
  if (auth.auth_session_side_effect_unknown) {
    errors.push({ case_id: null, error: 'auth_session_side_effect_unknown' });
  }
  const caseSet = cases.map((item) => ({ case_id: item.case_id, prompt_sha256: sha256(item.prompt) }));
  const common = {
    arm: args.arm,
    expected_semantic_mode: armModes[args.arm],
    approval_id_sha256: sha256(args.approvalId),
    target_origin_sha256: sha256(normalizeBaseUrl(args.baseUrl)),
    dataset_id_sha256: sha256(args.datasetId.toLowerCase()),
    fixture_sha256: fixture.fileSha256,
    case_set_sha256: hashJson(caseSet),
    case_count: cases.length,
    identity_sha256: auth.identity_sha256,
    approved_identity_match: auth.identity_sha256
      === credentials.approvedIdentitySha256.trim().toLowerCase(),
    started_at: startedAt,
    finished_at: finishedAt,
  };
  const status = errors.length === 0 && summaries.length === cases.length
    && summaries.every((item) => item.status === 'captured') ? 'captured' : 'failed';
  const summary = {
    smoke: 'semantic-supply-main-live',
    schema_version: summarySchemaVersion,
    mode: 'controlled-live-single-arm',
    status,
    command_ready: status === 'captured',
    decision_eligible: false,
    ...common,
    auth,
    request_contract: {
      business_post_endpoint: '/api/v3/assistant-runs/stream',
      business_detail_endpoint_pattern: '/api/v3/assistant-runs/{run_id}',
      stream_request_attempt_count: rawCases.filter((item) => item.stream_attempt?.post_dispatched).length,
      stream_terminal_capture_count: rawCases.filter((item) => item.stream?.ok).length,
      uncertain_run_created_count: rawCases.filter((item) => item.stream_attempt?.uncertain_run_created).length,
      provider_side_effect_unknown_count: rawCases.filter((item) => item.stream_attempt?.provider_side_effect_unknown).length,
      stream_retry_count: 0,
      concurrency: 1,
      external_channel_used: false,
      payload_keys: ['local_thread_id', 'prompt', 'selected_scope'],
      selected_scope_keys: ['datasets', 'mode'],
    },
    retry_observability: {
      exact_provider_retry_count_available: false,
      exact_provider_retry_count: null,
      reason_code: 'inner_same_provider_attempts_not_persisted_per_run',
    },
    cleanup_manifest: {
      required: true,
      private: true,
      no_auto_delete: true,
      records_all_stream_attempts: true,
    },
    cases: summaries,
    errors: errors.map((item) => ({
      case_id: item.case_id,
      error_sha256: sha256(item.error),
      error_chars: item.error.length,
    })),
  };
  const raw = {
    smoke: 'semantic-supply-main-live',
    schema_version: rawSchemaVersion,
    mode: 'controlled-live-single-arm-private-raw',
    status,
    ...common,
    auth,
    fatal_error: fatalError,
    cases: rawCases,
    errors,
  };
  const cleanup = buildPrivateCleanupManifest(common, authentication, authAttempt, rawCases);
  assertSharedSummarySafe(summary, cases, rawCases.map((item) => item.stream?.answer || ''));
  assertRawReceiptHasNoCredentials(raw, credentials, authentication);
  assertRawReceiptHasNoCredentials(cleanup, credentials, authentication);
  return { status, summary, raw, cleanup };
}

function buildPrivateCleanupManifest(common, authentication, authAttempt, rawCases) {
  const assistantRunAttempts = rawCases.map((item) => ({
    case_id: item.case_id,
    local_thread_id: item.local_thread_id,
    post_dispatched: item.stream_attempt?.post_dispatched === true,
    transport_outcome: item.stream_attempt?.transport_outcome || 'not_dispatched',
    run_id: item.stream_attempt?.run_id || item.stream?.runId || null,
    uncertain_run_created: item.stream_attempt?.uncertain_run_created === true,
    provider_side_effect_unknown: item.stream_attempt?.provider_side_effect_unknown === true,
  }));
  const workflowExecutions = [];
  const outputArtifacts = [];
  for (const item of rawCases) {
    const detail = item.detail_poll?.final_detail || null;
    for (const workflowId of collectWorkflowIds(detail)) {
      workflowExecutions.push({
        case_id: item.case_id,
        run_id: item.stream_attempt?.run_id || item.stream?.runId || null,
        workflow_execution_id: workflowId,
      });
    }
    for (const artifact of Array.isArray(detail?.run?.output_artifacts) ? detail.run.output_artifacts : []) {
      outputArtifacts.push({
        case_id: item.case_id,
        run_id: item.stream_attempt?.run_id || item.stream?.runId || null,
        artifact_id: typeof artifact?.id === 'string' ? artifact.id : null,
        type: typeof artifact?.type === 'string' ? artifact.type : 'unknown',
        status: typeof artifact?.status === 'string' ? artifact.status : null,
        artifact_sha256: hashJson(artifact),
      });
    }
  }
  return {
    smoke: 'semantic-supply-main-live',
    schema_version: 'semantic-supply-main-live-private-cleanup-manifest.v1',
    mode: 'private-review-only-no-auto-delete',
    ...common,
    no_auto_delete: true,
    cleanup_actions_executed: [],
    operator_review_required: true,
    created_resources: {
      approved_identity_record: {
        identity_sha256: authentication?.identitySha256 || null,
        local_key_login_may_ensure_user: authentication?.method === 'environment-local-key-login',
        created_vs_reused_observable: false,
      },
      local_key_session: {
        created: authentication?.createdSession === true,
        login_post_dispatched: authAttempt.loginPostDispatched,
        login_response_received: authAttempt.responseReceived,
        login_response_status: authAttempt.responseStatus,
        auth_session_side_effect_unknown: authAttempt.sessionSideEffectUnknown,
        session_cookie_sha256: authentication?.createdSession && authentication?.cookie
          ? sha256(authentication.cookie)
          : null,
        logout_status: authentication?.logoutStatus ?? 'not-applicable',
        logout_verified: authentication?.logoutVerified ?? null,
      },
      assistant_run_attempts: assistantRunAttempts,
      workflow_executions: workflowExecutions,
      output_artifacts: outputArtifacts,
    },
  };
}

function assertSharedSummarySafe(summary, cases, answers) {
  const serialized = JSON.stringify(summary);
  for (const value of [...cases.map((item) => item.prompt), ...answers].filter((item) => item.length >= 4)) {
    if (serialized.includes(value)) throw new Error('shared summary contains raw question or answer text');
  }
  const forbiddenExactKeys = new Set([
    'prompt',
    'question',
    'answer',
    'assistant_text',
    'completed_response',
    'final_detail',
    'request_payload',
  ]);
  const visit = (value) => {
    if (Array.isArray(value)) return value.forEach(visit);
    if (!value || typeof value !== 'object') return;
    for (const [key, nested] of Object.entries(value)) {
      if (forbiddenExactKeys.has(key)) throw new Error(`shared summary contains forbidden raw field: ${key}`);
      visit(nested);
    }
  };
  visit(summary);
}

function assertRawReceiptHasNoCredentials(raw, credentials, authentication) {
  const serialized = JSON.stringify(raw);
  for (const value of [
    credentials.cookie,
    credentials.email,
    credentials.localKey,
    authentication?.cookie,
    ...normalizeSecretBindingIds(credentials.secretBindingIds),
    ...(authentication?.secretBindingIds || []),
  ]
    .filter((item) => typeof item === 'string' && item.length >= 4)) {
    if (serialized.includes(value)) throw new Error('private raw receipt contains an authentication credential');
  }
  if (/aidp_v3_session=/iu.test(serialized)) {
    throw new Error('private raw receipt contains a session-cookie marker');
  }
}

async function writeJsonExclusive(path, value, mode) {
  const handle = await open(path, 'wx', mode);
  try {
    await handle.writeFile(`${JSON.stringify(value, null, 2)}\n`, 'utf8');
  } finally {
    await handle.close();
  }
  await chmod(path, mode);
  const metadata = await stat(path);
  if ((metadata.mode & 0o777) !== mode) throw new Error(`receipt mode mismatch for ${path}`);
}

async function prepareReceiptRun(args) {
  await mkdir(defaultOutputDir, { recursive: true, mode: 0o700 });
  const repoRealPath = await realpath(repoRoot);
  const outputRealPath = await realpath(defaultOutputDir);
  const expectedOutputPath = resolve(repoRealPath, 'target', 'semantic-supply-main-live');
  if (outputRealPath !== expectedOutputPath) {
    throw new Error('private output root resolves outside the fixed repository target');
  }
  const timestamp = new Date().toISOString().replaceAll(/[-:.]/gu, '').replace('Z', 'Z');
  const prefix = `semantic-supply-main-live-${args.arm}-${timestamp}-${process.pid}-${randomUUID()}`;
  const runDir = resolve(outputRealPath, prefix);
  await mkdir(runDir, { recursive: false, mode: 0o700 });
  const runDirLink = await lstat(runDir);
  const runDirMetadata = await stat(runDir);
  if (runDirLink.isSymbolicLink() || !runDirMetadata.isDirectory() || (runDirMetadata.mode & 0o777) !== 0o700) {
    throw new Error('private receipt run directory is not a new 0700 directory');
  }
  const journalPath = resolve(runDir, `${prefix}.private.journal.jsonl`);
  const journalHandle = await open(journalPath, 'wx', 0o600);
  const record = async (event) => {
    await journalHandle.writeFile(`${JSON.stringify({ at: new Date().toISOString(), ...event })}\n`, 'utf8');
    await journalHandle.sync();
  };
  await record({
    event: 'receipt_run_prepared_before_network',
    arm: args.arm,
    approval_id_sha256: sha256(args.approvalId),
    dataset_id_sha256: sha256(args.datasetId.toLowerCase()),
    no_auto_delete: true,
  });
  return { prefix, runDir, journalPath, journalHandle, record };
}

async function writeReceipts(prepared, collected) {
  const rawPath = resolve(prepared.runDir, `${prepared.prefix}.private.raw.json`);
  const summaryPath = resolve(prepared.runDir, `${prepared.prefix}.summary.json`);
  const cleanupPath = resolve(prepared.runDir, `${prepared.prefix}.private.cleanup.json`);
  await prepared.record({
    event: 'final_receipts_started',
    status: collected.status,
    assistant_run_attempt_count: collected.cleanup.created_resources.assistant_run_attempts.length,
  });
  await writeJsonExclusive(rawPath, collected.raw, 0o600);
  await writeJsonExclusive(summaryPath, collected.summary, 0o644);
  await writeJsonExclusive(cleanupPath, collected.cleanup, 0o600);
  await prepared.record({
    event: 'final_receipts_written',
    raw_sha256: sha256(JSON.stringify(collected.raw)),
    summary_sha256: sha256(JSON.stringify(collected.summary)),
    cleanup_sha256: sha256(JSON.stringify(collected.cleanup)),
  });
  return { rawPath, summaryPath, cleanupPath, journalPath: prepared.journalPath };
}

function buildSseFrame(event, data) {
  return `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
}

function jsonResponse(value, status = 200, headers = {}) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json', ...headers },
  });
}

function selfTestMock({
  failStream = false,
  malformedLoginBody = false,
  loginFailureNoCookie = false,
} = {}) {
  const calls = [];
  let revoked = false;
  const runId = '11111111-1111-4111-8111-111111111111';
  const workflowId = '22222222-2222-4222-8222-222222222222';
  const answer = 'synthetic self test answer';
  const detail = {
    run: {
      id: runId,
      local_thread_id: 'pending-self-test-thread',
      user_prompt: 'pending-self-test-prompt',
      updated_at: '2026-07-15T00:00:00Z',
      service_lane: 'ordinary_chat',
      selected_scope: { mode: 'user_selected', datasets: ['33333333-3333-4333-8333-333333333333'] },
      evidence_state: {
        status: 'supplied',
        supplied_items: [{ type: 'retrieval_evidence', source_locator: 'synthetic://source/1' }],
        supply_quality: { citationLocatorCount: 1, citationLocators: ['synthetic://source/1'] },
      },
      execution_trail: [{ workflow_execution_id: workflowId }],
      output_artifacts: [{ type: 'assistant_message', role: 'assistant', content: answer }],
      runtime: { provider: 'synthetic-provider', model: 'synthetic-model', lane: 'assistant_chat' },
    },
    events: [{
      id: 'event-1',
      run_id: runId,
      sequence_no: 4,
      event_name: 'assistant_run.completed',
      payload: { service_lane: 'ordinary_chat' },
    }],
    diagnostics: {
      provider_usage: {
        summary: { request_count: 1, failed_request_count: 0 },
        recent_events: [{ provider: 'synthetic-provider', model: 'synthetic-model', lane: 'assistant_chat' }],
      },
    },
  };
  const fetchImpl = async (url, options = {}) => {
    const parsed = new URL(url);
    const body = typeof options.body === 'string' && options.body ? JSON.parse(options.body) : null;
    calls.push({
      path: parsed.pathname,
      method: options.method || 'GET',
      body,
      redirect: options.redirect || null,
    });
    if (parsed.pathname === '/api/v3/auth/key/login') {
      if (loginFailureNoCookie) return jsonResponse({ error: 'synthetic_login_failure' }, 500);
      if (malformedLoginBody) {
        return new Response('{malformed', {
          status: 200,
          headers: {
            'content-type': 'application/json',
            'set-cookie': 'aidp_v3_session=self-test-session; Path=/; HttpOnly',
          },
        });
      }
      return jsonResponse({
        user: { id: 'self-test-user' },
        session: { user_id: 'self-test-user' },
        active_secret_binding_ids: ['binding-1'],
      }, 200, { 'set-cookie': 'aidp_v3_session=self-test-session; Path=/; HttpOnly' });
    }
    if (parsed.pathname === '/api/v3/auth/logout') {
      revoked = true;
      return jsonResponse({ ok: true });
    }
    if (parsed.pathname === '/api/v3/auth/session') {
      return revoked
        ? jsonResponse({ user: null, session: null })
        : jsonResponse({ user: { id: 'self-test-user' }, session: { user_id: 'self-test-user' } });
    }
    if (parsed.pathname === '/api/v3/assistant-runs/stream') {
      if (failStream) return jsonResponse({ error: 'synthetic_stream_failure' }, 500);
      detail.run.local_thread_id = body?.local_thread_id;
      detail.run.user_prompt = body?.prompt;
      const completed = {
        assistant_run_id: runId,
        assistant_message: { content: answer },
        evidence_state: detail.run.evidence_state,
      };
      const sse = [
        buildSseFrame('assistant_run.accepted', { index: 0 }),
        buildSseFrame('assistant_run.delta', { index: 1, delta: 'synthetic ' }),
        buildSseFrame('assistant_run.delta', { index: 2, delta: 'answer' }),
        buildSseFrame('assistant_run.completed', { index: 3, response: completed }),
        buildSseFrame('done', { index: 4 }),
      ].join('');
      return new Response(sse, { status: 200, headers: { 'content-type': 'text/event-stream' } });
    }
    if (parsed.pathname === `/api/v3/assistant-runs/${runId}`) return jsonResponse(detail);
    return jsonResponse({ error: 'unexpected_path' }, 404);
  };
  return { fetchImpl, calls, answer };
}

async function runSelfTest() {
  const args = {
    ...parseArgs(['--self-test']),
    mode: 'self-test',
    baseUrl: 'https://self-test.invalid',
    datasetId: '33333333-3333-4333-8333-333333333333',
    approvalId: 'SELFTEST-APPROVAL',
    arm: 'B',
    ackControlledLive: true,
    timeoutMs: 10_000,
    pollIntervalMs: 1,
    pollAttempts: 2,
  };
  const fixture = { fileSha256: sha256('self-test-fixture') };
  const cases = [{ case_id: 'self-test-case', prompt: 'synthetic self test question' }];
  const credentials = {
    cookie: '',
    email: 'self-test@example.invalid',
    localKey: 'self-test-key',
    secretBindingIds: '',
    approvedIdentitySha256: hashJson({ user_id: 'self-test-user' }),
  };
  const successMock = selfTestMock();
  const success = await collectControlledLive(args, fixture, cases, {
    fetchImpl: successMock.fetchImpl,
    credentials,
    signal: new AbortController().signal,
  });
  const request = successMock.calls.find((call) => call.path === '/api/v3/assistant-runs/stream');
  const failureMock = selfTestMock({ failStream: true });
  const failure = await collectControlledLive(args, fixture, cases, {
    fetchImpl: failureMock.fetchImpl,
    credentials,
    signal: new AbortController().signal,
  });
  const malformedLoginMock = selfTestMock({ malformedLoginBody: true });
  const malformedLogin = await collectControlledLive(args, fixture, cases, {
    fetchImpl: malformedLoginMock.fetchImpl,
    credentials,
    signal: new AbortController().signal,
  });
  const failedLoginMock = selfTestMock({ loginFailureNoCookie: true });
  const failedLogin = await collectControlledLive(args, fixture, cases, {
    fetchImpl: failedLoginMock.fetchImpl,
    credentials,
    signal: new AbortController().signal,
  });
  const capturedCase = success.raw.cases[0];
  const capturedDetail = capturedCase.detail_poll.final_detail;
  const capturedAttempt = { localThreadId: capturedCase.local_thread_id };
  const capturedStream = { runId: capturedCase.stream.runId };
  const checks = {
    no_real_network: true,
    no_filesystem_writes: true,
    success_capture_ready: success.status === 'captured' && success.summary.command_ready,
    exact_minimal_payload:
      stableStringify(Object.keys(request?.body || {}).sort())
        === stableStringify(['local_thread_id', 'prompt', 'selected_scope'])
      && stableStringify(Object.keys(request?.body?.selected_scope || {}).sort())
        === stableStringify(['datasets', 'mode'])
      && request?.body?.prompt === cases[0].prompt,
    single_stream_post: successMock.calls.filter((call) => call.path.endsWith('/assistant-runs/stream')).length === 1,
    redirects_fail_closed: [
      ...successMock.calls,
      ...failureMock.calls,
      ...malformedLoginMock.calls,
      ...failedLoginMock.calls,
    ]
      .every((call) => call.redirect === 'error'),
    detail_stable_twice: success.summary.cases[0].detail?.stable === true
      && success.summary.cases[0].detail?.poll_count === 2,
    detail_binding_is_strict:
      detailBindingIssues(capturedDetail, capturedStream, capturedAttempt, cases[0]).length === 0
      && detailBindingIssues(capturedDetail, { runId: '99999999-9999-4999-8999-999999999999' }, capturedAttempt, cases[0]).includes('detail_run_id_mismatch')
      && detailBindingIssues(capturedDetail, capturedStream, { localThreadId: 'wrong-thread' }, cases[0]).includes('detail_local_thread_id_mismatch')
      && detailBindingIssues(capturedDetail, capturedStream, capturedAttempt, { prompt: 'wrong prompt' }).includes('detail_user_prompt_mismatch')
      && detailBindingIssues(
        { ...capturedDetail, events: [{ ...capturedDetail.events[0], run_id: '99999999-9999-4999-8999-999999999999' }] },
        capturedStream,
        capturedAttempt,
        cases[0],
      ).includes('detail_event_run_id_mismatch'),
    summary_has_only_question_answer_hashes:
      !JSON.stringify(success.summary).includes(cases[0].prompt)
      && !JSON.stringify(success.summary).includes(successMock.answer),
    provider_model_route_workflow_artifact_recorded:
      success.summary.cases[0].provider.provider_model_pair_count === 1
      && success.summary.cases[0].provider.provider_lane_count === 1
      && success.summary.cases[0].route.main_route_count === 1
      && success.summary.cases[0].workflow.workflow_count === 1
      && success.summary.cases[0].artifacts.total_count === 1
      && success.summary.cases[0].artifacts.side_effect_count === 0
      && success.summary.cases[0].evidence.supplied_item_count === 1
      && success.summary.cases[0].evidence.citable_locator_count === 1
      && success.summary.cases[0].evidence.used_citation_count === null,
    retry_not_fabricated:
      success.summary.cases[0].provider.retry_observability.exact_retry_count === null
      && success.summary.retry_observability.exact_provider_retry_count_available === false,
    created_session_logged_out_and_verified:
      success.summary.auth.created_session
      && success.summary.auth.logout_status === 200
      && success.summary.auth.logout_verified === true,
    error_path_still_logs_out:
      failure.status === 'failed'
      && failure.summary.auth.logout_status === 200
      && failure.summary.auth.logout_verified === true,
    malformed_login_body_still_revokes_observed_session:
      malformedLogin.status === 'failed'
      && malformedLogin.summary.request_contract.stream_request_attempt_count === 0
      && malformedLogin.summary.auth.created_session === true
      && malformedLogin.summary.auth.logout_verified === true
      && malformedLogin.summary.auth.auth_session_side_effect_unknown === false,
    failed_login_without_cookie_marks_auth_side_effect_unknown:
      failedLogin.status === 'failed'
      && failedLogin.summary.request_contract.stream_request_attempt_count === 0
      && failedLogin.summary.auth.created_session === false
      && failedLogin.summary.auth.auth_session_side_effect_unknown === true,
    logout_requires_explicit_null_contract:
      logoutPayloadConfirmsRevoked({ user: null, session: null })
      && !logoutPayloadConfirmsRevoked({})
      && !logoutPayloadConfirmsRevoked({ user: null })
      && !logoutPayloadConfirmsRevoked({ user: undefined, session: undefined }),
    remote_plain_http_rejected: (() => {
      try {
        normalizeBaseUrl('http://203.0.113.10');
        return false;
      } catch {
        return normalizeBaseUrl('http://127.0.0.1:3000') === 'http://127.0.0.1:3000';
      }
    })(),
    attempts_and_cleanup_are_fail_closed:
      success.summary.request_contract.stream_request_attempt_count === 1
      && success.summary.request_contract.uncertain_run_created_count === 0
      && success.cleanup.created_resources.assistant_run_attempts.length === 1
      && success.cleanup.created_resources.assistant_run_attempts[0].run_id === '11111111-1111-4111-8111-111111111111'
      && failure.summary.request_contract.stream_request_attempt_count === 1
      && failure.summary.request_contract.uncertain_run_created_count === 1
      && failure.cleanup.created_resources.assistant_run_attempts[0].uncertain_run_created === true
      && success.cleanup.no_auto_delete === true,
    prototype_arm_names_rejected:
      ['toString', '__proto__'].every((arm) => validationIssues(
        { ...args, arm },
        credentials,
        cases,
      ).includes('arm_must_be_A_B_or_C')),
    current_build_hard_lock_is_fail_closed: (() => {
      const lockedPreflight = buildPreflight(args, fixture, cases, credentials);
      return lockedPreflight.command_ready === false
        && lockedPreflight.issues.includes(
          'live_execution_disabled_without_offline_promotion_candidate',
        );
    })(),
    alternate_fixture_rejected_before_read: (() => {
      try {
        assertFixedFixturePathBeforeRead('\\\\host\\share\\cases.jsonl');
        return false;
      } catch {
        return true;
      }
    })(),
  };
  return {
    smoke: 'semantic-supply-main-live',
    schema_version: 'semantic-supply-main-live-self-test.v1',
    status: Object.values(checks).every(Boolean) ? 'passed' : 'failed',
    network_requests: 0,
    filesystem_writes: 0,
    decision_eligible: false,
    checks,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(usage());
    return;
  }
  if (args.mode === 'self-test') {
    const report = await runSelfTest();
    console.log(args.pretty ? JSON.stringify(report, null, 2) : `Semantic supply main live self-test: ${report.status}`);
    if (report.status !== 'passed') throw new Error('semantic supply main live self-test failed');
    return;
  }

  assertFixedFixturePathBeforeRead(args.fixturePath);
  const fixtureMetadata = await lstat(defaultFixturePath);
  if (fixtureMetadata.isSymbolicLink() || !fixtureMetadata.isFile()) {
    throw new Error('fixed fixture must be a regular non-symlink file');
  }
  const fixture = await readFixtures(defaultFixturePath);
  const cases = selectCases(fixture, args.caseIds);
  const credentials = authenticationEnvironment();
  const preflight = buildPreflight(args, fixture, cases, credentials);
  if (args.mode === 'preflight') {
    console.log(args.pretty ? JSON.stringify(preflight, null, 2) : JSON.stringify(preflight));
    if (!preflight.command_ready) throw new Error(`preflight failed: ${preflight.issues.join(',')}`);
    return;
  }
  if (!preflight.command_ready) throw new Error(`execute preflight failed: ${preflight.issues.join(',')}`);

  const prepared = await prepareReceiptRun(args);
  const controller = new AbortController();
  let interruptedSignal = null;
  const interrupt = (signalName) => {
    interruptedSignal = signalName;
    controller.abort(new Error(`interrupted:${signalName}`));
  };
  const onSigint = () => interrupt('SIGINT');
  const onSigterm = () => interrupt('SIGTERM');
  process.once('SIGINT', onSigint);
  process.once('SIGTERM', onSigterm);
  let collected;
  let paths;
  try {
    collected = await collectControlledLive(args, fixture, cases, {
      fetchImpl: globalThis.fetch,
      credentials,
      signal: controller.signal,
      recordJournal: prepared.record,
    });
    paths = await writeReceipts(prepared, collected);
  } catch (error) {
    try {
      await prepared.record({
        event: 'execute_or_receipt_failure',
        error_sha256: sha256(error instanceof Error ? error.message : String(error)),
      });
    } catch {
      // The original failure remains authoritative; an unwritable journal is
      // already a fail-closed execute failure.
    }
    throw error;
  } finally {
    process.removeListener('SIGINT', onSigint);
    process.removeListener('SIGTERM', onSigterm);
    try {
      await prepared.journalHandle.sync();
    } finally {
      await prepared.journalHandle.close();
    }
  }
  console.log(`Semantic supply main live: status=${collected.status} arm=${args.arm} cases=${cases.length} decision_eligible=false`);
  console.log(`Private raw receipt (0600): ${paths.rawPath}`);
  console.log(`Private cleanup manifest (0600): ${paths.cleanupPath}`);
  console.log(`Private write-ahead journal (0600): ${paths.journalPath}`);
  console.log(`Shareable summary: ${paths.summaryPath}`);
  if (interruptedSignal) process.exitCode = interruptedSignal === 'SIGINT' ? 130 : 143;
  else if (collected.status !== 'captured') process.exitCode = 1;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
