import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  buildThirdPartyApiUrl,
  formatObservationTime,
  latestIntegrationActivity,
  normalizeAuditItem,
  normalizeIntegrationSummary,
  signalLabel,
} from './external-integrations.js';

test('buildThirdPartyApiUrl uses v3.elepcloud.com by default', () => {
  assert.equal(
    buildThirdPartyApiUrl('/v1/external/channels/main/events'),
    'https://v3.elepcloud.com/v1/external/channels/main/events',
  );
});

test('normalizeIntegrationSummary derives operational signal and counts', () => {
  const integration = normalizeIntegrationSummary({
    integration_id: 'generic-chat-main',
    display_name: 'Generic Chat',
    provider: 'generic_chat',
    status: 'enabled',
    health_status: 'healthy',
    pending_action_count: '2',
    blocked_action_count: 1,
    failed_action_count: 0,
    dispatched_action_count: 3,
    latest_action_at: '2026-05-14T09:30:00Z',
  });

  assert.equal(integration.id, 'generic-chat-main');
  assert.equal(integration.pendingActionCount, 2);
  assert.equal(integration.blockedActionCount, 1);
  assert.equal(integration.dispatchedActionCount, 3);
  assert.equal(integration.signal, 'blocked');
  assert.equal(signalLabel(integration.signal), '已阻断');
  assert.equal(latestIntegrationActivity(integration), '2026-05-14T09:30:00Z');
});

test('normalizeAuditItem keeps redacted summary shape stable', () => {
  const item = normalizeAuditItem({
    item_type: 'action',
    created_at: '2026-05-14T09:30:00Z',
    action_id: 'act-001',
    status: 'dispatch_blocked',
    failure_kind: 'dispatch_auth_missing',
    summary: { dispatch_reason: 'dispatch_auth_missing' },
  });

  assert.equal(item.itemType, 'action');
  assert.equal(item.actionId, 'act-001');
  assert.equal(item.summary.dispatch_reason, 'dispatch_auth_missing');
  assert.notEqual(formatObservationTime(item.createdAt), '无记录');
});

test('external integrations page does not include direct home navigation links', () => {
  const source = fs.readFileSync(
    path.join(process.cwd(), 'app', 'external-integrations', 'ExternalIntegrationsPageClient.js'),
    'utf8',
  );

  assert.doesNotMatch(source, /href=(["'])\/\1/);
  assert.doesNotMatch(source, /location\.href\s*=\s*(["'])\/\1/);
  assert.doesNotMatch(source, /router\.push\((["'])\/\1\)/);
});
