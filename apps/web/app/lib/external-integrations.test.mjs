import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  actionSignalLabel,
  artifactSignalLabel,
  auditItemTypeLabel,
  buildExternalActionPermalink,
  buildExternalAuditQuery,
  buildExternalActionTrace,
  buildThirdPartyApiUrl,
  controlResultLabel,
  driftSignalLabel,
  EXTERNAL_INTEGRATION_MODES,
  externalActionTraceFilename,
  formatObservationTime,
  latestIntegrationActivity,
  normalizeControlResult,
  normalizeAuditItem,
  normalizeIntegrationSummary,
  searchEvidenceSignalLabel,
  signalLabel,
} from './external-integrations.js';

test('buildThirdPartyApiUrl uses v3.elepcloud.com by default', () => {
  assert.equal(
    buildThirdPartyApiUrl('/v1/external/channels/main/events'),
    'https://v3.elepcloud.com/v1/external/channels/main/events',
  );
});

test('external integration modes are generic customer-facing guidance without secrets', () => {
  const modeText = JSON.stringify(EXTERNAL_INTEGRATION_MODES);
  const standardMode = EXTERNAL_INTEGRATION_MODES.find((mode) => mode.key === 'standard_bot');
  const pureMode = EXTERNAL_INTEGRATION_MODES.find((mode) => mode.key === 'pure_third_party');
  assert.equal(EXTERNAL_INTEGRATION_MODES.length, 2);
  assert(standardMode);
  assert(pureMode);
  assert.equal(
    standardMode.documentLinks.find((link) => link.key === 'complete-third-party-html').href,
    '/external-integrations/third-party-integration-api.zh-CN.html',
  );
  assert.equal(
    standardMode.documentLinks.find((link) => link.key === 'complete-third-party-md').download,
    true,
  );
  assert.equal(
    pureMode.documentLinks.find((link) => link.key === 'pure-third-party-html').href,
    '/external-integrations/pure-third-party-integration-guide.zh-CN.html',
  );
  assert.equal(
    pureMode.documentLinks.find((link) => link.key === 'pure-third-party-md').download,
    true,
  );
  assert(!modeText.includes('用户权限'));
  assert(!modeText.includes('v3in_live_'));
  assert(!/Authorization:\s*Bearer\s+[A-Za-z0-9_-]{12,}/.test(modeText));
  assert(!modeText.includes('本次联调'));
});

test('buildExternalAuditQuery encodes fixed audit filters', () => {
  assert.equal(buildExternalAuditQuery({}), '');
  assert.equal(
    buildExternalAuditQuery({ itemType: 'action', actionState: 'result_callback' }),
    '?item_type=action&action_state=result_callback',
  );
  assert.equal(
    buildExternalAuditQuery({ item_type: 'action', action_state: 'waiting_result' }),
    '?item_type=action&action_state=waiting_result',
  );
  assert.equal(
    buildExternalAuditQuery({ itemType: 'search_evidence' }),
    '?item_type=search_evidence',
  );
  assert.equal(
    buildExternalAuditQuery({ itemType: 'action', actionId: 'act-001', limit: 1 }),
    '?item_type=action&action_id=act-001&limit=1',
  );
});

test('buildExternalActionPermalink encodes action drilldown location without home navigation', () => {
  assert.equal(
    buildExternalActionPermalink({
      baseUrl: 'https://v3.elepcloud.com',
      pathname: '/external-integrations',
      integrationId: 'generic-chat-main',
      auditFilterKey: 'callbacks',
      actionId: 'act-001',
    }),
    'https://v3.elepcloud.com/external-integrations?integration_id=generic-chat-main&audit_filter=callbacks&action_id=act-001',
  );
  assert.equal(
    buildExternalActionPermalink({
      baseUrl: 'https://v3.elepcloud.com',
      integrationId: 'generic-chat-main',
      auditFilterKey: 'all',
    }),
    'https://v3.elepcloud.com/external-integrations?integration_id=generic-chat-main',
  );
});

test('buildExternalActionTrace produces redacted operator export', () => {
  const trace = buildExternalActionTrace({
    generatedAt: '2026-05-14T10:00:00.000Z',
    integration: {
      id: 'generic-chat-main',
      kind: 'channel',
      provider: 'generic_chat',
      displayName: 'Generic Chat',
      healthStatus: 'healthy',
      actionSignal: 'result_succeeded',
      artifactSignal: 'none',
      driftSignal: 'ok',
    },
    action: {
      itemType: 'action',
      actionId: 'act-001',
      status: 'external_action_succeeded',
      failureKind: null,
      assistantRunId: 'run-001',
      createdAt: '2026-05-14T09:59:00Z',
      summary: {
        action_type: 'external_business_action.invoke',
        result_callback_received: true,
      },
    },
  });

  assert.equal(trace.report_type, 'external_action_trace');
  assert.equal(trace.integration.id, 'generic-chat-main');
  assert.equal(trace.action.action_id, 'act-001');
  assert.equal(trace.redaction.raw_third_party_payload_included, false);
  assert.equal(trace.action.summary.result_callback_received, true);
  assert.equal(externalActionTraceFilename({ actionId: 'act/001 secret?' }), 'act-001-secret-trace.json');
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
    action_summary: {
      signal: 'result_succeeded',
      total_action_count: 4,
      waiting_result_count: 0,
      result_callback_count: 2,
      result_succeeded_count: 2,
      latest_result_callback_at: '2026-05-14T10:30:00Z',
    },
    drift_summary: {
      signal: 'identity_mapping_gap',
      unmapped_principal_count: 2,
      disabled_principal_count: 0,
    },
    artifact_summary: {
      signal: 'artifact_blocked',
      publish_action_count: 1,
      blocked_count: 1,
      latest_artifact_action_at: '2026-05-14T10:00:00Z',
    },
    search_summary: {
      signal: 'search_evidence_required',
      required_count: 2,
      latest_required_at: '2026-05-14T11:00:00Z',
    },
  });

  assert.equal(integration.id, 'generic-chat-main');
  assert.equal(integration.pendingActionCount, 2);
  assert.equal(integration.blockedActionCount, 1);
  assert.equal(integration.dispatchedActionCount, 3);
  assert.equal(integration.signal, 'blocked');
  assert.equal(integration.actionSignal, 'result_succeeded');
  assert.equal(actionSignalLabel(integration.actionSignal), '结果成功');
  assert.equal(integration.actionSummary.result_callback_count, 2);
  assert.equal(signalLabel(integration.signal), '已阻断');
  assert.equal(integration.driftSignal, 'identity_mapping_gap');
  assert.equal(driftSignalLabel(integration.driftSignal), '身份待映射');
  assert.equal(integration.driftSummary.unmapped_principal_count, 2);
  assert.equal(integration.artifactSignal, 'artifact_blocked');
  assert.equal(artifactSignalLabel(integration.artifactSignal), '产物阻断');
  assert.equal(integration.artifactSummary.publish_action_count, 1);
  assert.equal(integration.searchSignal, 'search_evidence_required');
  assert.equal(integration.searchEvidenceRequiredCount, 2);
  assert.equal(searchEvidenceSignalLabel(integration.searchSignal), '搜索待供料');
  assert.equal(latestIntegrationActivity(integration), '2026-05-14T09:30:00Z');
});

test('normalizeIntegrationSummary marks result failures as operational failures', () => {
  const integration = normalizeIntegrationSummary({
    integration_id: 'generic-chat-main',
    status: 'enabled',
    health_status: 'healthy',
    action_summary: {
      signal: 'result_failed',
      result_failed_count: 1,
      result_callback_count: 1,
    },
  });

  assert.equal(integration.signal, 'failed');
  assert.equal(integration.actionSignal, 'result_failed');
  assert.equal(actionSignalLabel(integration.actionSignal), '结果失败');
});

test('driftSignalLabel covers source recovery states', () => {
  assert.equal(driftSignalLabel('acl_missing'), 'ACL 缺失');
  assert.equal(driftSignalLabel('acl_stale'), 'ACL 过期');
  assert.equal(driftSignalLabel('sync_failed'), '同步失败');
  assert.equal(driftSignalLabel('sync_recovering'), '恢复中');
});

test('artifactSignalLabel covers publish and revoke states', () => {
  assert.equal(artifactSignalLabel('artifact_confirmation_pending'), '撤销待确认');
  assert.equal(artifactSignalLabel('artifact_failed'), '产物失败');
  assert.equal(artifactSignalLabel('artifact_blocked'), '产物阻断');
  assert.equal(artifactSignalLabel('artifact_published'), '已发布');
  assert.equal(artifactSignalLabel('artifact_revoked'), '已撤销');
});

test('actionSignalLabel covers callback lifecycle states', () => {
  assert.equal(actionSignalLabel('waiting_result'), '等待结果');
  assert.equal(actionSignalLabel('result_running'), '结果处理中');
  assert.equal(actionSignalLabel('result_succeeded'), '结果成功');
  assert.equal(actionSignalLabel('result_failed'), '结果失败');
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

test('auditItemTypeLabel covers search evidence items', () => {
  assert.equal(auditItemTypeLabel('message'), '消息');
  assert.equal(auditItemTypeLabel('action'), '动作');
  assert.equal(auditItemTypeLabel('search_evidence'), '搜索证据');
  assert.equal(auditItemTypeLabel('sync'), '同步');
});

test('normalizeControlResult summarizes management control responses', () => {
  const result = normalizeControlResult({
    accepted: true,
    integration_id: 'src-docs',
    integration_kind: 'source',
    action: 'retry',
    status: 'running',
    affected_action_count: 0,
    sync_run_id: 'sync-run-001',
    enqueued_tasks: [{ task_id: 'task-1' }],
  });

  assert.equal(result.integrationId, 'src-docs');
  assert.equal(result.enqueuedTaskCount, 1);
  assert.equal(controlResultLabel(result), '同步已入队：sync-run-001');

  assert.equal(
    controlResultLabel(normalizeControlResult({
      accepted: true,
      action: 'retry',
      affected_action_count: 2,
      enqueued_tasks: [{}, {}],
    })),
    '已入队 2 个动作重试',
  );
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
