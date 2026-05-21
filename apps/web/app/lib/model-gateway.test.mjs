import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildModelGatewayProfilePayload,
  modelGatewayProfileStatusSummary,
  normalizeModelGatewayPreset,
  normalizeModelGatewayProfiles,
  normalizeModelGatewayStatus,
  redactModelGatewaySecrets,
} from './model-gateway.js';

test('normalizeModelGatewayPreset maps recommended values', () => {
  const preset = normalizeModelGatewayPreset({
    preset_id: 'minimax/MiniMax-M2.7',
    display_name: 'MiniMax',
    provider_id: 'minimax',
    model_id: 'MiniMax-M2.7',
    max_concurrency: 5,
    timeout_ms: 30000,
    capabilities: { chat: true, api_key_hint: 'hidden' },
  });

  assert.equal(preset.id, 'minimax/MiniMax-M2.7');
  assert.equal(preset.providerId, 'minimax');
  assert.equal(preset.maxConcurrency, 5);
  assert.equal(preset.timeoutMs, 30000);
  assert.equal(preset.capabilities.api_key_hint, '[redacted]');
});

test('normalizeModelGatewayProfiles keeps safe display fields', () => {
  const [profile] = normalizeModelGatewayProfiles([{
    id: 'profile-row-id',
    profile_id: 'openclaw-main',
    display_name: 'OpenClaw Main',
    lane: 'assistant_chat',
    provider_id: 'openclaw',
    model_id: 'default',
    auth_env_key_name: 'OPENCLAW_API_KEY',
    has_secret: true,
    max_concurrency: 15,
    enabled: true,
  }]);

  assert.equal(profile.profileId, 'openclaw-main');
  assert.equal(profile.displayName, 'OpenClaw Main');
  assert.equal(profile.authEnvKeyName, 'OPENCLAW_API_KEY');
  assert.equal(profile.hasSecret, true);
  assert.equal(profile.maxConcurrency, 15);
});

test('redactModelGatewaySecrets removes nested secret material', () => {
  const redacted = redactModelGatewaySecrets({
    api_key: 'sk-live',
    nested: {
      bearer_token: 'token-value',
      safe: 'visible',
    },
  });

  assert.equal(redacted.api_key, '[redacted]');
  assert.equal(redacted.nested.bearer_token, '[redacted]');
  assert.equal(redacted.nested.safe, 'visible');
  assert.equal(JSON.stringify(redacted).includes('sk-live'), false);
});

test('buildModelGatewayProfilePayload omits blank update fields', () => {
  const payload = buildModelGatewayProfilePayload({
    profileId: '',
    displayName: '',
    maxConcurrency: 3,
    enabled: false,
  });

  assert.equal(payload.profile_id, undefined);
  assert.equal(payload.display_name, undefined);
  assert.equal(payload.max_concurrency, 3);
  assert.equal(payload.enabled, false);
});

test('normalizeModelGatewayStatus hides secrets and keeps lane counts readable', () => {
  const status = normalizeModelGatewayStatus({
    generated_at: '2026-05-21T08:00:00Z',
    lanes: [{
      lane: 'assistant_chat',
      routing_mode: 'active',
      canary_percent: 25,
      active: 12,
      queued: 3,
      profile_count: 2,
      queue_timeout_ms: 3000,
    }],
    providers: [{
      profile_id: 'OPENCLAW_MAIN',
      provider_id: 'openclaw',
      model_id: 'default',
      active: 2,
      queued: 1,
      minute_request_count: 7,
      minute_token_count: 900,
      runtime_failure_count: 4,
      would_throttle_count: 2,
      shadow_eval_count: 5,
      shadow_eval_pass_count: 4,
      quality_score: 80,
      format_pass_rate: 80,
      repair_rate: 0,
      last_shadow_eval_at: '2026-05-21T08:01:00Z',
      circuit_open: true,
      api_key: 'sk-hidden',
    }],
  });

  assert.equal(status.generatedAt, '2026-05-21T08:00:00Z');
  assert.equal(status.lanes[0].routingMode, 'active');
  assert.equal(status.lanes[0].activeCount, 12);
  assert.equal(status.lanes[0].queuedCount, 3);
  assert.equal(status.lanes[0].canaryPercent, 25);
  assert.equal(status.lanes[0].profileCount, 2);
  assert.equal(status.providers[0].profileId, 'OPENCLAW_MAIN');
  assert.equal(status.providers[0].providerId, 'openclaw');
  assert.equal(status.providers[0].queuedCount, 1);
  assert.equal(status.providers[0].minuteRequestCount, 7);
  assert.equal(status.providers[0].minuteTokenCount, 900);
  assert.equal(status.providers[0].runtimeFailureCount, 4);
  assert.equal(status.providers[0].wouldThrottleCount, 2);
  assert.equal(status.providers[0].shadowEvalCount, 5);
  assert.equal(status.providers[0].qualityScore, 80);
  assert.equal(status.providers[0].formatPassRate, 80);
  assert.equal(status.providers[0].lastShadowEvalAt, '2026-05-21T08:01:00Z');
  assert.equal(status.providers[0].circuitState, 'open');
  assert.equal(JSON.stringify(status).includes('sk-hidden'), false);
});

test('modelGatewayProfileStatusSummary labels enabled and missing-secret states', () => {
  const ready = modelGatewayProfileStatusSummary(
    { profileId: 'p1', enabled: true, hasSecret: true, maxConcurrency: 5 },
    { providers: [{ profileId: 'p1', activeCount: 1, circuitOpen: false }] },
  );
  const missing = modelGatewayProfileStatusSummary({ profileId: 'p2', enabled: true, hasSecret: false }, { providers: [] });

  assert.equal(ready.tone, 'healthy');
  assert.equal(ready.detail, 'active 1/5');
  assert.equal(missing.tone, 'warning');
  assert.equal(missing.label, '缺少密钥');
});
