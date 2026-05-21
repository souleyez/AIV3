const SENSITIVE_KEY_MARKERS = [
  'secret',
  'token',
  'password',
  'api_key',
  'apikey',
  'bearer',
  'credential',
];

export const MODEL_GATEWAY_API_PATHS = {
  presets: '/api/v3/model-gateway/presets',
  profiles: '/api/v3/model-gateway/profiles',
  status: '/api/v3/model-gateway/status',
};

function isPlainObject(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function stringOrEmpty(value) {
  return String(value ?? '').trim();
}

function numberOrNull(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function booleanOrFalse(value) {
  return value === true || value === 'true';
}

function keyIsSensitive(key = '') {
  const lower = String(key).toLowerCase();
  return SENSITIVE_KEY_MARKERS.some((marker) => lower.includes(marker));
}

export function redactModelGatewaySecrets(value, parentKey = '') {
  if (keyIsSensitive(parentKey)) {
    return '[redacted]';
  }
  if (Array.isArray(value)) {
    return value.map((item) => redactModelGatewaySecrets(item, parentKey));
  }
  if (isPlainObject(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => [
        key,
        redactModelGatewaySecrets(child, key),
      ]),
    );
  }
  return value;
}

export function normalizeModelGatewayPreset(raw = {}) {
  return {
    id: stringOrEmpty(raw.preset_id || raw.presetId || raw.id),
    label: stringOrEmpty(raw.display_name || raw.displayName || raw.preset_id || raw.id),
    providerId: stringOrEmpty(raw.provider_id || raw.providerId),
    modelId: stringOrEmpty(raw.model_id || raw.modelId),
    lane: stringOrEmpty(raw.lane || 'assistant_chat'),
    wireApi: stringOrEmpty(raw.wire_api || raw.wireApi || 'openai-compatible'),
    baseUrl: stringOrEmpty(raw.base_url || raw.baseUrl),
    apiPath: stringOrEmpty(raw.api_path || raw.apiPath),
    maxConcurrency: numberOrNull(raw.max_concurrency ?? raw.maxConcurrency),
    rpmLimit: numberOrNull(raw.rpm_limit ?? raw.rpmLimit),
    tpmLimit: numberOrNull(raw.tpm_limit ?? raw.tpmLimit),
    timeoutMs: numberOrNull(raw.timeout_ms ?? raw.timeoutMs),
    priority: numberOrNull(raw.priority),
    capabilities: redactModelGatewaySecrets(raw.capabilities || {}),
  };
}

export function normalizeModelGatewayProfile(raw = {}) {
  return {
    id: stringOrEmpty(raw.id),
    profileId: stringOrEmpty(raw.profile_id || raw.profileId),
    displayName: stringOrEmpty(raw.display_name || raw.displayName || raw.profile_id || raw.profileId),
    lane: stringOrEmpty(raw.lane || 'assistant_chat'),
    providerId: stringOrEmpty(raw.provider_id || raw.providerId),
    modelId: stringOrEmpty(raw.model_id || raw.modelId),
    baseUrl: stringOrEmpty(raw.base_url || raw.baseUrl),
    apiPath: stringOrEmpty(raw.api_path || raw.apiPath),
    wireApi: stringOrEmpty(raw.wire_api || raw.wireApi || 'openai-compatible'),
    authMode: stringOrEmpty(raw.auth_mode || raw.authMode || 'env_key'),
    authEnvKeyName: stringOrEmpty(raw.auth_env_key_name || raw.authEnvKeyName),
    hasSecret: booleanOrFalse(raw.has_secret ?? raw.hasSecret),
    recommendedPreset: stringOrEmpty(raw.recommended_preset || raw.recommendedPreset),
    maxConcurrency: numberOrNull(raw.max_concurrency ?? raw.maxConcurrency),
    rpmLimit: numberOrNull(raw.rpm_limit ?? raw.rpmLimit),
    tpmLimit: numberOrNull(raw.tpm_limit ?? raw.tpmLimit),
    timeoutMs: numberOrNull(raw.timeout_ms ?? raw.timeoutMs),
    priority: numberOrNull(raw.priority) ?? 100,
    enabled: raw.enabled !== false,
    capabilities: redactModelGatewaySecrets(raw.capabilities || {}),
    createdAt: raw.created_at || raw.createdAt || null,
    updatedAt: raw.updated_at || raw.updatedAt || null,
  };
}

export function normalizeModelGatewayProfiles(payload) {
  return (Array.isArray(payload) ? payload : []).map(normalizeModelGatewayProfile);
}

export function normalizeModelGatewayStatus(raw = {}) {
  const lanes = Array.isArray(raw.lanes) ? raw.lanes : [];
  const providers = Array.isArray(raw.providers) ? raw.providers : [];
  return {
    generatedAt: raw.generated_at || raw.generatedAt || null,
    lanes: lanes.map((lane) => ({
      lane: stringOrEmpty(lane.lane),
      routingMode: stringOrEmpty(lane.routing_mode || lane.routingMode || 'observe_only'),
      canaryPercent: numberOrNull(lane.canary_percent ?? lane.canaryPercent),
      maxConcurrency: numberOrNull(lane.max_concurrency ?? lane.maxConcurrency),
      activeCount: numberOrNull(lane.active ?? lane.active_count ?? lane.activeCount) ?? 0,
      queuedCount: numberOrNull(lane.queued ?? lane.queued_count ?? lane.queuedCount) ?? 0,
      queueLimit: numberOrNull(lane.queue_limit ?? lane.queueLimit),
      queueTimeoutMs: numberOrNull(lane.queue_timeout_ms ?? lane.queueTimeoutMs),
      profileCount: numberOrNull(lane.profile_count ?? lane.profileCount) ?? 0,
    })),
    providers: providers.map((provider) => redactModelGatewaySecrets({
      profileId: stringOrEmpty(provider.profile_id || provider.profileId),
      displayName: stringOrEmpty(provider.display_name || provider.displayName || provider.profile_id),
      lane: stringOrEmpty(provider.lane || 'assistant_chat'),
      source: stringOrEmpty(provider.source || 'database'),
      provider: stringOrEmpty(provider.provider || provider.provider_id || provider.providerId),
      providerId: stringOrEmpty(provider.provider_id || provider.providerId || provider.provider),
      model: stringOrEmpty(provider.model || provider.model_id || provider.modelId),
      modelId: stringOrEmpty(provider.model_id || provider.modelId || provider.model),
      wireApi: stringOrEmpty(provider.wire_api || provider.wireApi),
      priority: numberOrNull(provider.priority) ?? 100,
      activeCount: numberOrNull(provider.active ?? provider.active_count ?? provider.activeCount) ?? 0,
      queuedCount: numberOrNull(provider.queued ?? provider.queued_count ?? provider.queuedCount) ?? 0,
      maxConcurrency: numberOrNull(provider.max_concurrency ?? provider.maxConcurrency),
      queueLimit: numberOrNull(provider.queue_limit ?? provider.queueLimit),
      queueTimeoutMs: numberOrNull(provider.queue_timeout_ms ?? provider.queueTimeoutMs),
      rpmLimit: numberOrNull(provider.rpm_limit ?? provider.rpmLimit),
      tpmLimit: numberOrNull(provider.tpm_limit ?? provider.tpmLimit),
      requestCount: numberOrNull(provider.request_count ?? provider.requestCount) ?? 0,
      successCount: numberOrNull(provider.success_count ?? provider.successCount) ?? 0,
      failureCount: numberOrNull(provider.failure_count ?? provider.failureCount) ?? 0,
      timeoutCount: numberOrNull(provider.timeout_count ?? provider.timeoutCount) ?? 0,
      rateLimitCount: numberOrNull(provider.rate_limit_count ?? provider.rateLimitCount) ?? 0,
      inputTokens: numberOrNull(provider.input_tokens ?? provider.inputTokens) ?? 0,
      outputTokens: numberOrNull(provider.output_tokens ?? provider.outputTokens) ?? 0,
      runtimeSuccessCount: numberOrNull(provider.runtime_success_count ?? provider.runtimeSuccessCount) ?? 0,
      runtimeFailureCount: numberOrNull(provider.runtime_failure_count ?? provider.runtimeFailureCount) ?? 0,
      runtimeTimeoutCount: numberOrNull(provider.runtime_timeout_count ?? provider.runtimeTimeoutCount) ?? 0,
      runtimeRateLimitCount: numberOrNull(provider.runtime_rate_limit_count ?? provider.runtimeRateLimitCount) ?? 0,
      shadowEvalCount: numberOrNull(provider.shadow_eval_count ?? provider.shadowEvalCount) ?? 0,
      shadowEvalPassCount: numberOrNull(provider.shadow_eval_pass_count ?? provider.shadowEvalPassCount) ?? 0,
      shadowEvalFailCount: numberOrNull(provider.shadow_eval_fail_count ?? provider.shadowEvalFailCount) ?? 0,
      shadowEvalFormatPassCount: numberOrNull(provider.shadow_eval_format_pass_count ?? provider.shadowEvalFormatPassCount) ?? 0,
      shadowEvalRepairCount: numberOrNull(provider.shadow_eval_repair_count ?? provider.shadowEvalRepairCount) ?? 0,
      qualityScore: numberOrNull(provider.quality_score ?? provider.qualityScore),
      formatPassRate: numberOrNull(provider.format_pass_rate ?? provider.formatPassRate),
      repairRate: numberOrNull(provider.repair_rate ?? provider.repairRate),
      lastShadowEvalAt: provider.last_shadow_eval_at || provider.lastShadowEvalAt || null,
      consecutiveFailures: numberOrNull(provider.consecutive_failures ?? provider.consecutiveFailures) ?? 0,
      p50LatencyMs: numberOrNull(provider.p50_latency_ms ?? provider.p50LatencyMs),
      p95LatencyMs: numberOrNull(provider.p95_latency_ms ?? provider.p95LatencyMs),
      circuitOpen: booleanOrFalse(provider.circuit_open ?? provider.circuitOpen),
      circuitState: booleanOrFalse(provider.circuit_open ?? provider.circuitOpen)
        ? 'open'
        : stringOrEmpty(provider.circuit_state || provider.circuitState || 'closed'),
      openedUntil: provider.opened_until || provider.openedUntil || null,
      lastSuccessAt: provider.last_success_at || provider.lastSuccessAt || null,
      lastFailureAt: provider.last_failure_at || provider.lastFailureAt || null,
      lastFailureReason: stringOrEmpty(provider.last_failure_reason || provider.lastFailureReason),
      enabled: provider.enabled !== false,
    })),
  };
}

export function modelGatewayProfileStatusSummary(profile = {}, status = {}) {
  const providerStatus = (status.providers || []).find((item) => item.profileId === profile.profileId) || {};
  const circuit = providerStatus.circuitOpen || providerStatus.circuitState === 'open' ? 'open' : 'closed';
  const enabled = profile.enabled !== false;
  return {
    tone: !enabled ? 'neutral' : circuit === 'open' ? 'critical' : profile.hasSecret ? 'healthy' : 'warning',
    label: !enabled ? '已停用' : circuit === 'open' ? '熔断中' : profile.hasSecret ? '可用' : '缺少密钥',
    detail: `active ${providerStatus.activeCount || 0}/${profile.maxConcurrency || providerStatus.maxConcurrency || '-'}`,
    providerStatus,
  };
}

export function buildModelGatewayProfilePayload(draft = {}) {
  const payload = {
    profile_id: stringOrEmpty(draft.profileId),
    display_name: stringOrEmpty(draft.displayName),
    lane: stringOrEmpty(draft.lane) || 'assistant_chat',
    provider_id: stringOrEmpty(draft.providerId),
    model_id: stringOrEmpty(draft.modelId),
    base_url: stringOrEmpty(draft.baseUrl) || undefined,
    api_path: stringOrEmpty(draft.apiPath) || undefined,
    wire_api: stringOrEmpty(draft.wireApi) || 'openai-compatible',
    auth_mode: stringOrEmpty(draft.authMode) || 'env_key',
    auth_env_key_name: stringOrEmpty(draft.authEnvKeyName) || undefined,
    recommended_preset: stringOrEmpty(draft.recommendedPreset) || undefined,
    max_concurrency: numberOrNull(draft.maxConcurrency) || undefined,
    rpm_limit: numberOrNull(draft.rpmLimit) || undefined,
    tpm_limit: numberOrNull(draft.tpmLimit) || undefined,
    timeout_ms: numberOrNull(draft.timeoutMs) || undefined,
    priority: numberOrNull(draft.priority) || undefined,
    enabled: draft.enabled !== false,
    capabilities: isPlainObject(draft.capabilities) ? draft.capabilities : undefined,
  };
  return Object.fromEntries(
    Object.entries(payload).filter(([, value]) => value !== undefined && value !== ''),
  );
}

async function requestModelGateway(path, options = {}) {
  const response = await fetch(path, {
    ...options,
    headers: {
      'content-type': 'application/json',
      ...(options.headers || {}),
    },
    cache: 'no-store',
  });
  const contentType = response.headers.get('content-type') || '';
  const payload = contentType.includes('application/json') ? await response.json() : await response.text();
  if (!response.ok) {
    const message = isPlainObject(payload) ? payload.message || payload.error : payload;
    throw new Error(message || `model gateway request failed: ${response.status}`);
  }
  return payload;
}

export async function fetchModelGatewayPresets() {
  const payload = await requestModelGateway(MODEL_GATEWAY_API_PATHS.presets);
  return (Array.isArray(payload) ? payload : []).map(normalizeModelGatewayPreset);
}

export async function fetchModelGatewayProfiles() {
  const payload = await requestModelGateway(MODEL_GATEWAY_API_PATHS.profiles);
  return normalizeModelGatewayProfiles(payload);
}

export async function createModelGatewayProfile(draft) {
  const payload = await requestModelGateway(MODEL_GATEWAY_API_PATHS.profiles, {
    method: 'POST',
    body: JSON.stringify(buildModelGatewayProfilePayload(draft)),
  });
  return normalizeModelGatewayProfile(payload);
}

export async function updateModelGatewayProfile(profileId, draft) {
  const payload = await requestModelGateway(`${MODEL_GATEWAY_API_PATHS.profiles}/${encodeURIComponent(profileId)}`, {
    method: 'PATCH',
    body: JSON.stringify(buildModelGatewayProfilePayload(draft)),
  });
  return normalizeModelGatewayProfile(payload);
}

export async function disableModelGatewayProfile(profileId) {
  const payload = await requestModelGateway(`${MODEL_GATEWAY_API_PATHS.profiles}/${encodeURIComponent(profileId)}/disable`, {
    method: 'POST',
    body: JSON.stringify({}),
  });
  return normalizeModelGatewayProfile(payload);
}

export async function testModelGatewayProfile(profileId) {
  return requestModelGateway(`${MODEL_GATEWAY_API_PATHS.profiles}/${encodeURIComponent(profileId)}/test`, {
    method: 'POST',
    body: JSON.stringify({}),
  });
}

export async function fetchModelGatewayStatus() {
  const payload = await requestModelGateway(MODEL_GATEWAY_API_PATHS.status);
  return normalizeModelGatewayStatus(payload);
}
