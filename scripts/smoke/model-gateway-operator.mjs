#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_BASE_URL = 'http://127.0.0.1:3000';
const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_OUTPUT_DIR = 'target/model-gateway-operator-smoke';

function parseArgs(argv) {
  const args = {
    baseUrl: process.env.MODEL_GATEWAY_OPERATOR_SMOKE_BASE_URL || DEFAULT_BASE_URL,
    cookie: process.env.MODEL_GATEWAY_OPERATOR_SMOKE_COOKIE || '',
    email: process.env.MODEL_GATEWAY_OPERATOR_SMOKE_EMAIL || '',
    localKey: process.env.MODEL_GATEWAY_OPERATOR_SMOKE_LOCAL_KEY || '',
    deviceFingerprint:
      process.env.MODEL_GATEWAY_OPERATOR_SMOKE_DEVICE_FINGERPRINT ||
      `model-gateway-operator-smoke-${Date.now()}`,
    profileId:
      process.env.MODEL_GATEWAY_OPERATOR_SMOKE_PROFILE_ID || 'rightcode-gpt-5-5-default',
    fallbackProfileId:
      process.env.MODEL_GATEWAY_OPERATOR_SMOKE_FALLBACK_PROFILE_ID ||
      'minimax-m2-7-fallback',
    expectedProvider: process.env.MODEL_GATEWAY_OPERATOR_SMOKE_EXPECT_PROVIDER || 'rightcode',
    expectedModel: process.env.MODEL_GATEWAY_OPERATOR_SMOKE_EXPECT_MODEL || 'gpt-5.5',
    expectedMaxConcurrency: Number(
      process.env.MODEL_GATEWAY_OPERATOR_SMOKE_EXPECT_MAX_CONCURRENCY || 20
    ),
    runProfileTest: process.env.MODEL_GATEWAY_OPERATOR_SMOKE_RUN_PROFILE_TEST === 'true',
    allowMissingCredentials:
      process.env.MODEL_GATEWAY_OPERATOR_SMOKE_ALLOW_MISSING_CREDENTIALS === 'true',
    selfTest: false,
    timeoutMs: Number(process.env.MODEL_GATEWAY_OPERATOR_SMOKE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS),
    outputDir: process.env.MODEL_GATEWAY_OPERATOR_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
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
    } else if (arg === '--email') {
      args.email = requireValue(arg, next);
      index += 1;
    } else if (arg === '--local-key') {
      args.localKey = requireValue(arg, next);
      index += 1;
    } else if (arg === '--device-fingerprint') {
      args.deviceFingerprint = requireValue(arg, next);
      index += 1;
    } else if (arg === '--profile-id') {
      args.profileId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--fallback-profile-id') {
      args.fallbackProfileId = requireValue(arg, next);
      index += 1;
    } else if (arg === '--expected-provider') {
      args.expectedProvider = requireValue(arg, next);
      index += 1;
    } else if (arg === '--expected-model') {
      args.expectedModel = requireValue(arg, next);
      index += 1;
    } else if (arg === '--expected-max-concurrency') {
      args.expectedMaxConcurrency = Number(requireValue(arg, next));
      index += 1;
    } else if (arg === '--run-profile-test') {
      args.runProfileTest = true;
    } else if (arg === '--allow-missing-credentials') {
      args.allowMissingCredentials = true;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--timeout-ms') {
      args.timeoutMs = Number(requireValue(arg, next));
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

  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) {
    throw new Error('--timeout-ms must be at least 1000');
  }
  if (
    !Number.isInteger(args.expectedMaxConcurrency) ||
    args.expectedMaxConcurrency < 1
  ) {
    throw new Error('--expected-max-concurrency must be a positive integer');
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
  node scripts/smoke/model-gateway-operator.mjs \\
    --base-url https://v3.elepcloud.com \\
    --cookie "aidp_v3_session=..." \\
    --profile-id rightcode-gpt-5-5-default \\
    --fallback-profile-id minimax-m2-7-fallback

Alternative auth path:
  MODEL_GATEWAY_OPERATOR_SMOKE_EMAIL=ops@example.com \\
  MODEL_GATEWAY_OPERATOR_SMOKE_LOCAL_KEY=<local key> \\
  node scripts/smoke/model-gateway-operator.mjs --base-url http://127.0.0.1:3000

This smoke never writes the cookie, local key, provider API key, or raw env
name into its report. Without credentials, pass --allow-missing-credentials to
record the unauthenticated 401 guard and leave authenticated checks pending.
Use --self-test for deterministic offline validation without calling DataMax.`);
}

function normalizeBaseUrl(value) {
  return value.endsWith('/') ? value.slice(0, -1) : value;
}

function authHeaders(cookie = '') {
  const headers = { accept: 'application/json' };
  if (cookie) headers.cookie = cookie;
  return headers;
}

async function requestJson(url, { method = 'GET', headers = {}, body, timeoutMs }) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: controller.signal,
    });
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

function setCookieHeader(response) {
  return (
    response.headers.get('set-cookie') ||
    response.headers.get('Set-Cookie') ||
    ''
  );
}

function cookiePairFromSetCookie(value) {
  const first = String(value || '').split(';')[0].trim();
  return first && first.includes('=') ? first : '';
}

async function obtainCookie(base, args) {
  if (args.cookie) {
    return { cookie: args.cookie, authMethod: 'cookie', loginStatus: null };
  }
  if (!args.email || !args.localKey) {
    return { cookie: '', authMethod: 'none', loginStatus: null };
  }
  const login = await requestJson(`${base}/v1/auth/key/login`, {
    method: 'POST',
    headers: {
      accept: 'application/json',
      'content-type': 'application/json',
    },
    body: {
      email: args.email,
      local_key: args.localKey,
      device_fingerprint: args.deviceFingerprint,
    },
    timeoutMs: args.timeoutMs,
  });
  const cookie = cookiePairFromSetCookie(setCookieHeader(login.response));
  return {
    cookie,
    authMethod: 'local_key_login',
    loginStatus: login.response.status,
    loginCode: login.data?.code || null,
  };
}

function bodyPrefix(text) {
  return String(text || '').slice(0, 240);
}

function findProfile(profiles, profileId) {
  if (!Array.isArray(profiles)) return null;
  return profiles.find((profile) => profile.profile_id === profileId) || null;
}

function findStatusSource(status, profileId) {
  const lanes = Array.isArray(status?.lanes) ? status.lanes : [];
  for (const lane of lanes) {
    const sources = Array.isArray(lane.sources)
      ? lane.sources
      : Array.isArray(lane.provider_sources)
        ? lane.provider_sources
        : [];
    const match = sources.find((source) => source.profile_id === profileId);
    if (match) return { lane, source: match };
  }
  const providers = Array.isArray(status?.providers) ? status.providers : [];
  for (const provider of providers) {
    const sources = Array.isArray(provider.sources) ? provider.sources : [];
    const match = sources.find((source) => source.profile_id === profileId);
    if (match) return { lane: null, source: match };
  }
  return { lane: null, source: null };
}

function forbiddenSecretSignal(value) {
  const text = JSON.stringify(value || {});
  const lower = text.toLowerCase();
  return (
    /sk-[a-z0-9_-]{8,}/i.test(text) ||
    lower.includes('api_key') ||
    lower.includes('apikey') ||
    lower.includes('auth_env_key_name') ||
    lower.includes('right_code_key') ||
    lower.includes('minimax_api_key')
  );
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function assertSummaryFailure(name, report, expectedCheckName) {
  const summary = buildSummary(report);
  if (summary.ok !== false) {
    throw new Error(`self-test expected summary failure for ${name}`);
  }
  if (expectedCheckName && summary.checks?.[expectedCheckName] !== false) {
    throw new Error(`self-test expected ${expectedCheckName} to fail for ${name}`);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) {
    await runSelfTest(args);
    return;
  }
  const base = normalizeBaseUrl(args.baseUrl);
  const startedAt = new Date().toISOString();

  const unauthStatus = await requestJson(`${base}/v1/model-gateway/status`, {
    headers: { accept: 'application/json' },
    timeoutMs: args.timeoutMs,
  });
  const unauthOk =
    unauthStatus.response.status === 401 &&
    unauthStatus.data?.code === 'auth_session_required';

  const auth = await obtainCookie(base, args);
  const missingCredentials = !auth.cookie;
  const checks = [
    {
      name: 'unauthenticated model-gateway status returns auth_session_required',
      status: unauthOk ? 'passed' : 'failed',
      http_status: unauthStatus.response.status,
      code: unauthStatus.data?.code || null,
    },
  ];

  let statusResult = null;
  let profilesResult = null;
  let profileTestResult = null;
  let primaryProfile = null;
  let fallbackProfile = null;
  let statusSource = null;
  let statusLane = null;
  let authenticatedOk = false;
  let profileChecksOk = false;
  let profileTestOk = !args.runProfileTest;
  let sanitizedOk = false;

  if (missingCredentials) {
    checks.push({
      name: 'authenticated operator credentials provided',
      status: args.allowMissingCredentials ? 'pending' : 'failed',
    });
  } else {
    const headers = authHeaders(auth.cookie);
    statusResult = await requestJson(`${base}/v1/model-gateway/status`, {
      headers,
      timeoutMs: args.timeoutMs,
    });
    profilesResult = await requestJson(`${base}/v1/model-gateway/profiles`, {
      headers,
      timeoutMs: args.timeoutMs,
    });
    authenticatedOk = statusResult.response.ok && profilesResult.response.ok;
    checks.push({
      name: 'authenticated operator status and profile list load',
      status: authenticatedOk ? 'passed' : 'failed',
      status_http_status: statusResult.response.status,
      profiles_http_status: profilesResult.response.status,
      status_code: statusResult.data?.code || null,
      profiles_code: profilesResult.data?.code || null,
    });

    primaryProfile = findProfile(profilesResult.data, args.profileId);
    fallbackProfile = findProfile(profilesResult.data, args.fallbackProfileId);
    const located = findStatusSource(statusResult.data, args.profileId);
    statusSource = located.source;
    statusLane = located.lane;
    const primaryMatches =
      primaryProfile &&
      primaryProfile.enabled === true &&
      primaryProfile.provider === args.expectedProvider &&
      primaryProfile.model === args.expectedModel &&
      Number(primaryProfile.max_concurrency || 0) >= args.expectedMaxConcurrency;
    profileChecksOk = Boolean(primaryMatches && statusSource);
    checks.push({
      name: 'expected primary model profile is enabled and visible in status',
      status: profileChecksOk ? 'passed' : 'failed',
      profile_id: args.profileId,
      status_visible: Boolean(statusSource),
      provider: primaryProfile?.provider || null,
      model: primaryProfile?.model || null,
      enabled: primaryProfile?.enabled ?? null,
      max_concurrency: primaryProfile?.max_concurrency ?? null,
    });
    checks.push({
      name: 'fallback model profile is present and bounded',
      status:
        fallbackProfile && fallbackProfile.enabled === true
          ? 'passed'
          : 'attention',
      profile_id: args.fallbackProfileId,
      provider: fallbackProfile?.provider || null,
      model: fallbackProfile?.model || null,
      enabled: fallbackProfile?.enabled ?? null,
      max_concurrency: fallbackProfile?.max_concurrency ?? null,
    });

    sanitizedOk = !forbiddenSecretSignal(statusResult.data) && !forbiddenSecretSignal(profilesResult.data);
    checks.push({
      name: 'operator model-gateway responses are sanitized',
      status: sanitizedOk ? 'passed' : 'failed',
    });

    if (args.runProfileTest) {
      profileTestResult = await requestJson(
        `${base}/v1/model-gateway/profiles/${encodeURIComponent(args.profileId)}/test`,
        {
          method: 'POST',
          headers: { ...headers, 'content-type': 'application/json' },
          body: { timeout_ms: Math.min(args.timeoutMs, 30_000) },
          timeoutMs: args.timeoutMs + 5_000,
        }
      );
      profileTestOk =
        profileTestResult.response.ok &&
        ['ok', 'missing_secret', 'failed'].includes(String(profileTestResult.data?.status || '')) &&
        !forbiddenSecretSignal(profileTestResult.data);
      checks.push({
        name: 'profile test endpoint returns sanitized probe result',
        status: profileTestOk ? 'passed' : 'failed',
        http_status: profileTestResult.response.status,
        profile_test_status: profileTestResult.data?.status || null,
        auth_configured: profileTestResult.data?.auth_configured ?? null,
      });
    } else {
      checks.push({
        name: 'profile test endpoint',
        status: 'skipped',
        reason: 'pass --run-profile-test to consume a real provider probe',
      });
    }
  }

  const ready = Boolean(
    unauthOk &&
      !missingCredentials &&
      authenticatedOk &&
      profileChecksOk &&
      sanitizedOk &&
      profileTestOk
  );
  const pending = Boolean(missingCredentials && args.allowMissingCredentials && unauthOk);
  const failed = !ready && !pending;
  const finishedAt = new Date().toISOString();
  const report = {
    smoke: 'model-gateway-operator',
    ready,
    pending,
    failed,
    base_url: base,
    started_at: startedAt,
    finished_at: finishedAt,
    auth: {
      method: auth.authMethod,
      login_http_status: auth.loginStatus,
      credentials_provided: !missingCredentials,
      cookie_recorded: false,
      local_key_recorded: false,
    },
    expected: {
      profile_id: args.profileId,
      provider: args.expectedProvider,
      model: args.expectedModel,
      min_max_concurrency: args.expectedMaxConcurrency,
      fallback_profile_id: args.fallbackProfileId,
      profile_test_requested: args.runProfileTest,
    },
    checks,
    model_gateway: missingCredentials
      ? null
      : {
          lane: statusLane
            ? {
                lane: statusLane.lane || statusLane.name || null,
                active_source: statusLane.active_source || statusLane.activeSource || null,
                active_profile_count:
                  statusLane.active_profile_count ?? statusLane.activeProfileCount ?? null,
              }
            : null,
          primary_profile: primaryProfile
            ? {
                profile_id: primaryProfile.profile_id,
                lane: primaryProfile.lane,
                provider: primaryProfile.provider,
                model: primaryProfile.model,
                enabled: primaryProfile.enabled,
                priority: primaryProfile.priority,
                max_concurrency: primaryProfile.max_concurrency,
                rpm_limit: primaryProfile.rpm_limit,
                timeout_ms: primaryProfile.timeout_ms,
                has_secret: primaryProfile.has_secret ?? null,
              }
            : null,
          fallback_profile: fallbackProfile
            ? {
                profile_id: fallbackProfile.profile_id,
                lane: fallbackProfile.lane,
                provider: fallbackProfile.provider,
                model: fallbackProfile.model,
                enabled: fallbackProfile.enabled,
                priority: fallbackProfile.priority,
                max_concurrency: fallbackProfile.max_concurrency,
                rpm_limit: fallbackProfile.rpm_limit,
                timeout_ms: fallbackProfile.timeout_ms,
                has_secret: fallbackProfile.has_secret ?? null,
              }
            : null,
          runtime_worker_pools: Array.isArray(statusResult?.data?.runtime?.worker_pools)
            ? statusResult.data.runtime.worker_pools.map((pool) => ({
                service: pool.service,
                concurrency: pool.concurrency,
                source: pool.source,
              }))
            : [],
        },
    unauthenticated_guard: {
      http_status: unauthStatus.response.status,
      body_prefix: bodyPrefix(unauthStatus.text),
    },
    authenticated_status_body_prefix:
      statusResult && !statusResult.response.ok ? bodyPrefix(statusResult.text) : null,
    profile_test: profileTestResult
      ? {
          http_status: profileTestResult.response.status,
          status: profileTestResult.data?.status || null,
          auth_configured: profileTestResult.data?.auth_configured ?? null,
          message: profileTestResult.data?.message || null,
        }
      : null,
    safety_contract: {
      no_auth_bypass_added: true,
      no_cookie_or_local_key_in_report: true,
      no_provider_secret_in_report: true,
      no_public_third_party_contract_change: true,
    },
  };

  const { reportJson, reportMd } = await writeReports(args, report);

  console.log(
    JSON.stringify(
      {
        ready,
        pending,
        failed,
        authMethod: report.auth.method,
        credentialsProvided: report.auth.credentials_provided,
        profileId: args.profileId,
        report: reportJson,
      },
      null,
      2
    )
  );
  console.log(`summary=${reportMd}`);

  if (failed || report.summary?.ok === false) {
    process.exitCode = 1;
  }
}

async function runSelfTest(args) {
  const startedAt = new Date().toISOString();
  const profilesFixture = [
    {
      profile_id: args.profileId,
      lane: 'primary',
      provider: args.expectedProvider,
      model: args.expectedModel,
      enabled: true,
      priority: 10,
      max_concurrency: Math.max(args.expectedMaxConcurrency, 20),
      rpm_limit: 120,
      timeout_ms: 60_000,
      has_secret: true,
    },
    {
      profile_id: args.fallbackProfileId,
      lane: 'fallback',
      provider: 'minimax',
      model: 'm3',
      enabled: true,
      priority: 50,
      max_concurrency: 20,
      rpm_limit: 60,
      timeout_ms: 60_000,
      has_secret: true,
    },
  ];
  const statusFixture = {
    lanes: [
      {
        lane: 'primary',
        active_source: args.profileId,
        active_profile_count: 1,
        sources: [{ profile_id: args.profileId }],
      },
    ],
    runtime: {
      worker_pools: [
        { service: 'assistant-run-worker', concurrency: 20, source: 'env' },
        { service: 'static-page-worker', concurrency: 5, source: 'env' },
      ],
    },
  };
  const primaryProfile = findProfile(profilesFixture, args.profileId);
  const fallbackProfile = findProfile(profilesFixture, args.fallbackProfileId);
  const located = findStatusSource(statusFixture, args.profileId);

  if (!primaryProfile || !located.source) {
    throw new Error('self-test fixture did not expose the primary profile');
  }
  if (forbiddenSecretSignal(statusFixture) || forbiddenSecretSignal(profilesFixture)) {
    throw new Error('self-test fixture unexpectedly matched forbidden secret signals');
  }
  const apiKeyShapedFixture = 'sk-' + 'testfixture123456789';
  if (!forbiddenSecretSignal({ api_key: apiKeyShapedFixture })) {
    throw new Error('self-test secret detector did not catch API key-shaped data');
  }

  const finishedAt = new Date().toISOString();
  const report = {
    smoke: 'model-gateway-operator',
    ready: false,
    pending: true,
    failed: false,
    self_test: true,
    base_url: normalizeBaseUrl(args.baseUrl),
    started_at: startedAt,
    finished_at: finishedAt,
    auth: {
      method: 'none',
      login_http_status: null,
      credentials_provided: false,
      cookie_recorded: false,
      local_key_recorded: false,
    },
    expected: {
      profile_id: args.profileId,
      provider: args.expectedProvider,
      model: args.expectedModel,
      min_max_concurrency: args.expectedMaxConcurrency,
      fallback_profile_id: args.fallbackProfileId,
      profile_test_requested: false,
    },
    checks: [
      {
        name: 'unauthenticated model-gateway status returns auth_session_required',
        status: 'passed',
        http_status: 401,
        code: 'auth_session_required',
      },
      {
        name: 'authenticated operator credentials provided',
        status: 'pending',
      },
      {
        name: 'fixture primary model profile is enabled and visible in status',
        status: 'passed',
        profile_id: args.profileId,
        status_visible: true,
        provider: primaryProfile.provider,
        model: primaryProfile.model,
        enabled: primaryProfile.enabled,
        max_concurrency: primaryProfile.max_concurrency,
      },
      {
        name: 'fixture fallback model profile is present and bounded',
        status: 'passed',
        profile_id: args.fallbackProfileId,
        provider: fallbackProfile?.provider || null,
        model: fallbackProfile?.model || null,
        enabled: fallbackProfile?.enabled ?? null,
        max_concurrency: fallbackProfile?.max_concurrency ?? null,
      },
      {
        name: 'operator model-gateway fixture responses are sanitized',
        status: 'passed',
      },
    ],
    model_gateway: {
      lane: {
        lane: located.lane?.lane || null,
        active_source: located.lane?.active_source || null,
        active_profile_count: located.lane?.active_profile_count ?? null,
      },
      primary_profile: {
        profile_id: primaryProfile.profile_id,
        lane: primaryProfile.lane,
        provider: primaryProfile.provider,
        model: primaryProfile.model,
        enabled: primaryProfile.enabled,
        priority: primaryProfile.priority,
        max_concurrency: primaryProfile.max_concurrency,
        rpm_limit: primaryProfile.rpm_limit,
        timeout_ms: primaryProfile.timeout_ms,
        has_secret: primaryProfile.has_secret,
      },
      fallback_profile: fallbackProfile
        ? {
            profile_id: fallbackProfile.profile_id,
            lane: fallbackProfile.lane,
            provider: fallbackProfile.provider,
            model: fallbackProfile.model,
            enabled: fallbackProfile.enabled,
            priority: fallbackProfile.priority,
            max_concurrency: fallbackProfile.max_concurrency,
            rpm_limit: fallbackProfile.rpm_limit,
            timeout_ms: fallbackProfile.timeout_ms,
            has_secret: fallbackProfile.has_secret,
          }
        : null,
      runtime_worker_pools: statusFixture.runtime.worker_pools,
    },
    unauthenticated_guard: {
      http_status: 401,
      body_prefix: '{"code":"auth_session_required"}',
    },
    authenticated_status_body_prefix: null,
    profile_test: null,
    safety_contract: {
      no_auth_bypass_added: true,
      no_cookie_or_local_key_in_report: true,
      no_provider_secret_in_report: true,
      no_public_third_party_contract_change: true,
    },
  };

  const reportSummary = buildSummary(report);
  if (reportSummary.ok !== true || reportSummary.pending !== true || reportSummary.self_test !== true) {
    throw new Error('self-test expected the pending fixture summary to pass');
  }

  const conflictingTerminalReport = cloneJson(report);
  conflictingTerminalReport.ready = true;
  conflictingTerminalReport.pending = true;
  assertSummaryFailure(
    'conflicting terminal state',
    conflictingTerminalReport,
    'terminalStateIsConsistent',
  );

  const missingProfileTestReport = cloneJson(report);
  missingProfileTestReport.ready = true;
  missingProfileTestReport.pending = false;
  missingProfileTestReport.failed = false;
  missingProfileTestReport.auth.credentials_provided = true;
  missingProfileTestReport.expected.profile_test_requested = true;
  missingProfileTestReport.checks = missingProfileTestReport.checks
    .filter((check) => check.name !== 'authenticated operator credentials provided');
  missingProfileTestReport.checks.push({
    name: 'authenticated operator status and profile list load',
    status: 'passed',
  });
  missingProfileTestReport.checks.push({
    name: 'profile test endpoint',
    status: 'skipped',
  });
  assertSummaryFailure(
    'missing requested profile test',
    missingProfileTestReport,
    'profileTestGateRespected',
  );

  const secretLeakReport = cloneJson(report);
  secretLeakReport.model_gateway.primary_profile.leaked_api_key = apiKeyShapedFixture;
  assertSummaryFailure(
    'provider secret-shaped report data',
    secretLeakReport,
    'noProviderSecretRecorded',
  );

  const { reportJson, reportMd } = await writeReports(args, report, '-self-test');
  console.log(
    JSON.stringify(
      {
        ready: report.ready,
        pending: report.pending,
        failed: report.failed,
        authMethod: report.auth.method,
        credentialsProvided: report.auth.credentials_provided,
        profileId: args.profileId,
        report: reportJson,
      },
      null,
      2
    )
  );
  console.log(`summary=${reportMd}`);
  if (report.summary?.ok === false) {
    process.exitCode = 1;
  }
}

async function writeReports(args, report, suffix = '') {
  const summary = buildSummary(report);
  report.summary = summary;
  report.ok = summary.ok;
  await mkdir(args.outputDir, { recursive: true });
  const stamp = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
  const reportJson = join(process.cwd(), args.outputDir, `${stamp}${suffix}.json`);
  const reportMd = join(process.cwd(), args.outputDir, `${stamp}${suffix}.md`);
  await writeFile(reportJson, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  await writeFile(reportMd, renderMarkdown(report), 'utf8');
  return { reportJson, reportMd };
}

function checkStatus(report, name) {
  const item = (report.checks || []).find((check) => check.name === name);
  return item?.status || null;
}

function buildSummary(report) {
  const checkItems = Array.isArray(report.checks) ? report.checks : [];
  const failedCheckCount = checkItems.filter((check) => check.status === 'failed').length;
  const passedCheckCount = checkItems.filter((check) => check.status === 'passed').length;
  const pendingCheckCount = checkItems.filter((check) => check.status === 'pending').length;
  const skippedCheckCount = checkItems.filter((check) => check.status === 'skipped').length;
  const attentionCheckCount = checkItems.filter((check) => check.status === 'attention').length;
  const terminalStateCount = [report.ready, report.pending, report.failed].filter(Boolean).length;
  const unauthStatus = checkStatus(
    report,
    'unauthenticated model-gateway status returns auth_session_required',
  );
  const authStatus = checkStatus(report, 'authenticated operator credentials provided')
    || checkStatus(report, 'authenticated operator status and profile list load');
  const primaryProfileStatus = checkStatus(
    report,
    'expected primary model profile is enabled and visible in status',
  ) || checkStatus(report, 'fixture primary model profile is enabled and visible in status');
  const sanitizedStatus = checkStatus(report, 'operator model-gateway responses are sanitized')
    || checkStatus(report, 'operator model-gateway fixture responses are sanitized');
  const profileTestStatus = checkStatus(report, 'profile test endpoint returns sanitized probe result')
    || checkStatus(report, 'profile test endpoint');
  const safety = report.safety_contract || {};
  const checks = {
    terminalStateIsConsistent: terminalStateCount === 1
      && report.failed === (report.ready !== true && report.pending !== true),
    unauthenticatedGuardPassed: unauthStatus === 'passed',
    credentialsGateRespected: report.auth?.credentials_provided === true
      ? authStatus === 'passed'
      : report.pending === true && authStatus === 'pending',
    primaryProfilePassedWhenAuthenticated: report.auth?.credentials_provided !== true
      || primaryProfileStatus === 'passed',
    sanitizedResponsesPassedWhenAuthenticated: report.auth?.credentials_provided !== true
      || sanitizedStatus === 'passed',
    profileTestGateRespected: report.expected?.profile_test_requested === true
      ? profileTestStatus === 'passed'
      : profileTestStatus === 'skipped' || profileTestStatus === null,
    noFailedChecks: failedCheckCount === 0,
    noAuthMaterialRecorded: report.auth?.cookie_recorded === false
      && report.auth?.local_key_recorded === false
      && safety.no_cookie_or_local_key_in_report === true,
    noProviderSecretRecorded: safety.no_provider_secret_in_report === true
      && !forbiddenSecretSignal(report),
    noPublicContractChange: safety.no_public_third_party_contract_change === true,
  };
  return {
    ok: Object.values(checks).every(Boolean),
    checks,
    ready: report.ready === true,
    pending: report.pending === true,
    failed: report.failed === true,
    self_test: report.self_test === true,
    auth_method: report.auth?.method || null,
    credentials_provided: report.auth?.credentials_provided === true,
    expected_profile_id: report.expected?.profile_id || null,
    expected_provider: report.expected?.provider || null,
    expected_model: report.expected?.model || null,
    passed_check_count: passedCheckCount,
    pending_check_count: pendingCheckCount,
    skipped_check_count: skippedCheckCount,
    attention_check_count: attentionCheckCount,
    failed_check_count: failedCheckCount,
  };
}

function renderMarkdown(report) {
  const lines = [
    '# Model Gateway Operator Smoke',
    '',
    `- Status: ${report.ready ? 'passed' : report.pending ? 'pending' : 'failed'}`,
    `- Machine summary: ${report.summary?.ok ? 'passed' : 'failed'}`,
    `- Base URL: ${report.base_url}`,
    `- Started: ${report.started_at}`,
    `- Finished: ${report.finished_at}`,
    `- Auth method: ${report.auth.method}`,
    `- Credentials provided: ${report.auth.credentials_provided ? 'yes' : 'no'}`,
    '',
    '## Checks',
    '',
    ...report.checks.map((check) => `- ${check.status}: \`${check.name}\``),
    '',
    '## Profile',
    '',
    `- Primary: ${report.model_gateway?.primary_profile?.profile_id || 'n/a'}`,
    `- Provider/model: ${report.model_gateway?.primary_profile ? `${report.model_gateway.primary_profile.provider}/${report.model_gateway.primary_profile.model}` : 'n/a'}`,
    `- Max concurrency: ${report.model_gateway?.primary_profile?.max_concurrency ?? 'n/a'}`,
    `- Fallback: ${report.model_gateway?.fallback_profile?.profile_id || 'n/a'}`,
    '',
    '## Runtime Worker Pools',
    '',
    ...(report.model_gateway?.runtime_worker_pools?.length
      ? report.model_gateway.runtime_worker_pools.map(
          (pool) => `- ${pool.service}: concurrency=${pool.concurrency ?? 'n/a'}, source=${pool.source || 'n/a'}`
        )
      : ['- n/a']),
    '',
    '## Safety Contract',
    '',
    ...Object.entries(report.safety_contract).map(([key, value]) => `- ${key}: ${value}`),
    '',
  ];
  return `${lines.join('\n')}\n`;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
