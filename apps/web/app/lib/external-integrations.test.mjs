import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  actionSignalLabel,
  artifactSignalLabel,
  auditItemTypeLabel,
  buildOperationsSummary,
  buildExternalActionPermalink,
  buildExternalAuditQuery,
  buildExternalActionTrace,
  buildDatabaseSourceStatusExport,
  buildThirdPartyApiUrl,
  controlResultLabel,
  databaseSourceHealthSignalLabel,
  databaseSourceMetrics,
  databaseSourceReadOnlyStatus,
  databaseSourceReadOnlyStatusLabel,
  databaseSourceReadiness,
  databaseSourceSummary,
  databaseSourceSyncRuns,
  databaseSourceTablePreview,
  driftSignalLabel,
  EXTERNAL_INTEGRATION_MODES,
  codexExecutorInspectSummary,
  externalConversationStatusLabel,
  externalActionTraceFilename,
  databaseSourceStatusExportFilename,
  documentProcessingStatusLabel,
  formatExternalConversationDuration,
  formatWorkflowDuration,
  formatObservationTime,
  latestIntegrationActivity,
  inboundAuthSummary,
  normalizeControlResult,
  normalizeAuditItem,
  normalizeCodexExecutorTask,
  normalizeDatabaseSourceStatus,
  normalizeDocumentProcessingDiagnostic,
  normalizeDocumentProcessingDiagnostics,
  normalizeExternalConversationTest,
  normalizeExternalConversationTimeline,
  normalizeIntegrationSummary,
  normalizeWorkflowQueueStats,
  normalizeWorkflowTask,
  outboundReplyDispatchSignalLabel,
  outboundReplyDispatchSummary,
  searchEvidenceSignalLabel,
  signalLabel,
  workflowQueueLabel,
  workflowStatusLabel,
  workflowTaskKeyLabel,
} from './external-integrations.js';

test('buildThirdPartyApiUrl uses v3.elepcloud.com by default', () => {
  assert.equal(
    buildThirdPartyApiUrl('/v1/external/channels/main/events'),
    'https://v3.elepcloud.com/v1/external/channels/main/events',
  );
});

test('inboundAuthSummary redacts token material and exposes temporary state only', () => {
  const summary = inboundAuthSummary({
    inbound_auth_configured: true,
    inbound_auth_mode: 'bearer',
    temporary_access: {
      temporary: true,
      expired: false,
      expires_at: '2026-06-05T00:00:00Z',
      token_rotated_at: '2026-06-04T00:00:00Z',
    },
  });
  assert.deepEqual(summary, {
    configured: true,
    mode: 'bearer',
    temporary: true,
    expired: false,
    expiresAt: '2026-06-05T00:00:00Z',
    tokenRotatedAt: '2026-06-04T00:00:00Z',
  });
  assert(!JSON.stringify(summary).includes('v3in_'));
});

test('normalizeControlResult captures one-time inbound token response', () => {
  const result = normalizeControlResult({
    accepted: true,
    integration_id: 'generic-chat-customer-a',
    integration_kind: 'channel',
    action: 'create_channel',
    status: 'enabled',
    message: 'created',
    affected_action_count: 0,
    inbound_bearer_token: 'v3in_test_token',
    token_expires_at: '2026-06-05T00:00:00Z',
  });
  assert.equal(result.inboundBearerToken, 'v3in_test_token');
  assert.equal(result.tokenExpiresAt, '2026-06-05T00:00:00Z');
  assert.equal(controlResultLabel(result), '第三方通道已创建，token 仅本次展示');
});

test('external integration modes are generic customer-facing guidance without secrets', () => {
  const modeText = JSON.stringify(EXTERNAL_INTEGRATION_MODES);
  const standardMode = EXTERNAL_INTEGRATION_MODES.find((mode) => mode.key === 'standard_bot');
  const pureMode = EXTERNAL_INTEGRATION_MODES.find((mode) => mode.key === 'pure_third_party');
  const databaseMode = EXTERNAL_INTEGRATION_MODES.find((mode) => mode.key === 'database_integration');
  assert.equal(EXTERNAL_INTEGRATION_MODES.length, 3);
  assert(standardMode);
  assert(pureMode);
  assert(databaseMode);
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
  assert.equal(
    databaseMode.documentLinks.find((link) => link.key === 'database-complete-third-party-html').href,
    '/external-integrations/third-party-integration-api.zh-CN.html#section-26',
  );
  assert.equal(
    databaseMode.documentLinks.find((link) => link.key === 'database-complete-third-party-md').download,
    true,
  );
  assert(standardMode.checklist.includes('平台事件验签与消息解密'));
  assert(standardMode.checklist.includes('用户确认、动作派发和结果回调'));
  assert(pureMode.checklist.includes('文档解析：parse 与 parse-detail'));
  assert(pureMode.checklist.includes('按模板生成产物：artifact_type + template'));
  assert(databaseMode.checklist.includes('同步入库：数据库行清洗为 DataMax 数据集证据'));
  assert(databaseMode.checklist.includes('安全边界：不传密码、不执行任意 SQL、不暴露原始表 dump'));
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
    buildExternalAuditQuery({ itemType: 'outbound_reply' }),
    '?item_type=outbound_reply',
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

test('buildDatabaseSourceStatusExport produces redacted database status summary', () => {
  const status = normalizeDatabaseSourceStatus({
    status: {
      dataset: {
        dataset_id: 'ds-001',
        title: 'HY SQL',
        dataset_external_id: 'hy-sql-main',
      },
      dataset_readiness: {
        signal: 'ready',
        document_count: 2,
        indexed_document_count: 2,
        indexed_chunk_count: 8,
      },
      sync_readiness: {
        signal: 'ready',
        row_count: 2,
        failed_row_count: 1,
        row_failure_groups: [{
          table: 'bi_traffic_area',
          reason: 'mapped row has empty identity columns',
          sample_count: 1,
          reported_failed_row_count: 1,
          sample_source_primary_keys: ['42'],
        }],
      },
      recent_sync_runs: [{
        sync_run_id: 'run-001',
        sync_kind: 'full',
        status: 'succeeded',
        row_failure_groups: [{
          table: 'bi_traffic_area',
          reason: 'mapped row has empty identity columns',
          sample_count: 1,
          reported_failed_row_count: 1,
        }],
        counts: {
          documents_ingested: 2,
          row_count: 2,
          failed_row_count: 1,
        },
        checkpoint_summary: {
          has_checkpoint: true,
          cursor_present: true,
        },
      }],
      health_findings: {
        signal: 'attention',
        warning_count: 1,
        items: [{
          severity: 'warning',
          code: 'row_conversion_failures',
          title: '存在行级转换失败',
          message: '部分数据库行未能转换为文档。',
          count: 1,
        }],
      },
    },
  });
  const report = buildDatabaseSourceStatusExport({
    generatedAt: '2026-05-23T10:00:00Z',
    integration: {
      id: 'hy-sql-status',
      kind: 'source',
      provider: 'database',
      displayName: 'HY SQL',
      healthStatus: 'healthy',
      driftSignal: 'ok',
      configSummary: {
        database_source: {
          kind: 'mysql',
          database: 'hy_sql',
          connection_env: 'THIRD_PARTY_HY_SQL_DATABASE_URL',
          default_dataset_id: 'ds-001',
          table_count: 1,
          tables: ['bi_traffic_area'],
        },
      },
    },
    status,
  });

  assert.equal(report.report_type, 'database_source_status_summary');
  assert.equal(report.generated_at, '2026-05-23T10:00:00Z');
  assert.equal(report.database_source.connection_env, 'THIRD_PARTY_HY_SQL_DATABASE_URL');
  assert.equal(report.read_only_status.label, '只读可用');
  assert.equal(report.dataset.datasetExternalId, 'hy-sql-main');
  assert.equal(report.sync_readiness.rowFailureGroups[0].reportedFailedRowCount, 1);
  assert.equal(report.recent_sync_runs[0].checkpointSummary.cursorPresent, true);
  assert.equal(report.redaction.raw_database_credentials_included, false);
  assert.equal(report.redaction.raw_source_cursors_included, false);
  assert.equal(databaseSourceStatusExportFilename({ id: 'hy/sql status?' }), 'hy-sql-status-database-status.json');
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

test('outboundReplyDispatchSummary reports async reply dispatch readiness', () => {
  const ready = outboundReplyDispatchSummary({
    configSummary: {
      outbound_reply_dispatch: {
        endpoint_configured: true,
        auth_configured: true,
        auth_mode: 'signature_and_bearer',
        auth_source: 'reply_specific',
        endpoint_host: 'third.example.com',
        ready: true,
      },
    },
  });
  const missingAuth = outboundReplyDispatchSummary({
    configSummary: {
      outbound_reply_dispatch: {
        endpoint_configured: true,
        auth_mode: 'none',
        ready: false,
      },
    },
  });
  const missingEndpoint = outboundReplyDispatchSummary({
    configSummary: {
      outbound_reply_dispatch: {
        auth_configured: true,
        auth_mode: 'bearer',
        ready: false,
      },
    },
  });

  assert.equal(ready.signal, 'ready');
  assert.equal(ready.authSource, 'reply_specific');
  assert.equal(ready.endpointHost, 'third.example.com');
  assert.equal(outboundReplyDispatchSignalLabel(ready.signal), '可主动回推');
  assert.equal(missingAuth.signal, 'missing_auth');
  assert.equal(outboundReplyDispatchSignalLabel(missingAuth.signal), '待配置鉴权');
  assert.equal(missingEndpoint.signal, 'missing_endpoint');
  assert.equal(outboundReplyDispatchSignalLabel(missingEndpoint.signal), '待第三方回推地址');
});

test('database source observability helpers normalize redacted source summary', () => {
  const integration = normalizeIntegrationSummary({
    integration_id: 'hy-sql-source',
    integration_kind: 'source',
    provider: 'mysql',
    config_summary: {
      database_source: {
        kind: 'mysql',
        database: 'hy_sql',
        connection_env: 'THIRD_PARTY_HY_SQL_DATABASE_URL',
        default_dataset_id: '018f0000-0000-7000-9000-000000000001',
        system_user_id: 'system-hy-sql',
        tenant_external_id: 'tenant-hy',
        bot_external_id: 'bot-v3',
        dataset_external_ids: ['hy-sql-main', 'hy-sql-reporting'],
        read_only: true,
        latest_analysis_status: 'analysis_ready',
        latest_sync_status: 'completed',
        table_count: 3,
        tables: ['bi_traffic_area', 'bi_order_day', 'bi_user_region'],
      },
    },
    drift_summary: {
      latest_sync_status: 'completed',
      failed_sync_count: 0,
      database_dataset_readiness: {
        signal: 'partial_ready',
        default_dataset_id: '018f0000-0000-7000-9000-000000000001',
        document_count: 128,
        indexed_document_count: 120,
        failed_document_count: 1,
        processing_document_count: 7,
        chunk_count: 480,
        indexed_chunk_count: 460,
        latest_document_updated_at: '2026-05-22T01:00:00Z',
      },
    },
  });

  const source = databaseSourceSummary(integration);
  assert.equal(source.configured, true);
  assert.equal(source.valid, true);
  assert.equal(source.kind, 'mysql');
  assert.equal(source.database, 'hy_sql');
  assert.equal(source.defaultDatasetId, '018f0000-0000-7000-9000-000000000001');
  assert.equal(source.systemUserId, 'system-hy-sql');
  assert.equal(source.tenantExternalId, 'tenant-hy');
  assert.equal(source.botExternalId, 'bot-v3');
  assert.deepEqual(source.datasetExternalIds, ['hy-sql-main', 'hy-sql-reporting']);
  assert.equal(source.readOnly, true);
  assert.equal(source.tableCount, 3);
  assert.deepEqual(source.tables, ['bi_traffic_area', 'bi_order_day', 'bi_user_region']);

  const metrics = databaseSourceMetrics(integration);
  assert.deepEqual(metrics.map((metric) => metric.label), [
    '只读状态',
    '问答就绪',
    '系统用户',
    '第三方范围',
    '稳定分组',
    '数据库',
    '连接引用',
    '默认数据集',
    '表数量',
    '最近同步',
    '失败同步',
  ]);
  assert.equal(metrics.find((metric) => metric.label === '只读状态').value, '只读可用');
  assert.equal(metrics.find((metric) => metric.label === '系统用户').value, 'system-hy-sql');
  assert.equal(metrics.find((metric) => metric.label === '第三方范围').value, 'tenant tenant-hy · bot bot-v3');
  assert.equal(metrics.find((metric) => metric.label === '稳定分组').value, 'hy-sql-main, hy-sql-reporting');
  assert.equal(metrics.find((metric) => metric.label === '问答就绪').value, '部分可问');
  assert.equal(metrics.find((metric) => metric.label === '最近同步').value, 'completed');
  const readiness = databaseSourceReadiness(integration);
  assert.equal(readiness.signal, 'partial_ready');
  assert.equal(readiness.label, '部分可问');
  assert.equal(readiness.documentCount, 128);
  assert.equal(readiness.indexedChunkCount, 460);
  assert.equal(readiness.latestDocumentUpdatedAt, '2026-05-22T01:00:00Z');
  const readOnly = databaseSourceReadOnlyStatus(integration);
  assert.equal(readOnly.label, '只读可用');
  assert.equal(readOnly.sourceId, 'hy-sql-source');
  assert.equal(readOnly.systemUserId, 'system-hy-sql');
  assert.deepEqual(readOnly.datasetExternalIds, ['hy-sql-main', 'hy-sql-reporting']);

  const tablePreview = databaseSourceTablePreview(integration, 2);
  assert.deepEqual(tablePreview.tables, ['bi_traffic_area', 'bi_order_day']);
  assert.equal(tablePreview.hiddenCount, 1);

  const syncRuns = databaseSourceSyncRuns([
    normalizeAuditItem({
      item_type: 'sync',
      status: 'completed',
      created_at: '2026-05-22T01:00:00Z',
      summary: {
        sync_kind: 'full',
        counts: {
          documents_ingested: 128,
          acl_snapshot_count: 12,
          enqueued_task_count: 1,
          ingest_table_counts: [{
            table: 'bi_traffic_area',
            documents_ingested: 128,
            chunks_ingested: 480,
          }],
        },
      },
    }),
    normalizeAuditItem({
      item_type: 'action',
      status: 'external_action_succeeded',
    }),
  ]);
  assert.deepEqual(syncRuns, [{
    status: 'completed',
    syncKind: 'full',
    updatedAt: '2026-05-22T01:00:00Z',
    failureKind: '',
    lastError: '',
    failedTaskKey: '',
    documentCount: 128,
    rowCount: 128,
    skippedRowCount: 0,
    failedRowCount: 0,
    rowFailureSamples: [],
    aclSnapshotCount: 12,
    enqueuedTaskCount: 1,
    checkpointSummary: {
      configured: false,
      hasCheckpoint: false,
      cursorPresent: false,
      topLevelIncrementalPresent: false,
      tableCount: 0,
      tableCheckpoints: [],
      label: '无检查点',
    },
    tableCounts: [{
      table: 'bi_traffic_area',
      documentCount: 128,
      rowCount: 128,
      chunkCount: 480,
      skippedRowCount: 0,
      failedRowCount: 0,
    }],
  }]);
});

test('database source observability helpers keep unconfigured sources quiet', () => {
  const integration = normalizeIntegrationSummary({
    integration_id: 'plain-source',
    integration_kind: 'source',
    config_summary: {
      database_source: {
        configured: false,
      },
    },
  });

  const source = databaseSourceSummary(integration);
  assert.equal(source.configured, false);
  assert.deepEqual(databaseSourceMetrics(integration), []);
  assert.deepEqual(databaseSourceTablePreview(integration), { tables: [], hiddenCount: 0 });
  assert.equal(databaseSourceReadOnlyStatus(integration).label, '未挂接');
});

test('database source read-only status separates unsynced, running, and operator states', () => {
  const baseIntegration = normalizeIntegrationSummary({
    integration_id: 'hy-sql-read-only',
    integration_kind: 'source',
    config_summary: {
      database_source: {
        kind: 'mysql',
        database: 'hy_sql',
        connection_env: 'THIRD_PARTY_HY_SQL_DATABASE_URL',
        dataset_external_id: 'hy-sql-main',
        read_only: true,
      },
    },
  });

  assert.equal(databaseSourceReadOnlyStatus(baseIntegration, {
    status: {
      sync_readiness: { signal: 'no_sync' },
    },
  }).label, '未同步');

  assert.equal(databaseSourceReadOnlyStatus(baseIntegration, {
    status: {
      sync_readiness: { signal: 'sync_running', latest_status: 'running' },
    },
  }).label, '分析中');

  const operatorRequired = databaseSourceReadOnlyStatus(baseIntegration, {
    status: {
      config_valid: true,
      sync_readiness: {
        signal: 'sync_failed',
        last_error: 'failed with database_url=postgres://secret at /srv/private/source.sql',
      },
    },
  });
  assert.equal(operatorRequired.label, '需要 operator 处理');
  assert.equal(operatorRequired.recentError.includes('database_url=[redacted]'), true);
  assert.equal(operatorRequired.recentError.includes('[redacted:path]'), true);
  assert(!operatorRequired.recentError.includes('postgres://secret'));
});

test('database source status helper normalizes selected-only detail payload', () => {
  const status = normalizeDatabaseSourceStatus({
    source_id: 'hy-sql-source',
    status: {
      config_valid: true,
      dataset: {
        dataset_id: '018f0000-0000-7000-9000-000000000001',
        key: 'hy-sql',
        title: 'HY SQL',
        dataset_external_id: 'hy-sql-main',
        is_default: false,
      },
      datasets: [{
        dataset_id: '018f0000-0000-7000-9000-000000000001',
        key: 'hy-sql',
        title: 'HY SQL',
        dataset_external_id: 'hy-sql-main',
        is_default: false,
        readiness: {
          signal: 'ready',
          document_count: 2,
          indexed_document_count: 2,
          indexed_chunk_count: 8,
        },
        retrieval_evidence_count: 2,
      }],
      dataset_readiness: {
        signal: 'ready',
        document_count: 2,
        indexed_document_count: 2,
        indexed_chunk_count: 8,
      },
      table_readiness: [{
        table: 'bi_traffic_area',
        signal: 'ready',
        document_count: 2,
        indexed_document_count: 2,
        indexed_chunk_count: 8,
      }, {
        table: 'empty_table',
        signal: 'no_documents',
      }],
      semantic_profile: {
        kind: 'mysql',
        database: 'hy_sql',
        table_count: 1,
        column_count: 12,
        metric_count: 3,
        dimension_count: 4,
        time_dimension_count: 1,
        report_suggestion_count: 2,
        tables: [{
          table: 'bi_traffic_area',
          column_count: 12,
          approximate_row_count: 320,
          metric_count: 3,
          dimension_count: 4,
          time_dimension_count: 1,
          mapping_confidence: 92,
        }],
      },
      recent_sync_runs: [{
        sync_run_id: 'run-001',
        sync_kind: 'full',
        status: 'succeeded',
        row_failure_groups: [{
          table: 'bi_traffic_area',
          reason: 'mapped row has empty identity columns',
          sample_count: 1,
          reported_failed_row_count: 1,
          first_row_index: 3,
          sample_source_primary_keys: ['42'],
        }],
        counts: {
          documents_ingested: 2,
          row_count: 2,
          enqueued_task_count: 6,
          skipped_row_count: 0,
          failed_row_count: 1,
          row_failure_samples: [{
            table: 'bi_traffic_area',
            row_index: 3,
            source_primary_key: '42',
            reason: 'mapped row has empty identity columns',
          }],
          ingest_table_counts: [{
            table: 'bi_traffic_area',
            documents_ingested: 2,
            row_count: 2,
            chunks_ingested: 8,
            skipped_row_count: 0,
            failed_row_count: 1,
          }],
        },
        checkpoint: {
          workflow_stage: 'completed',
          workflow_status: 'succeeded',
        },
        checkpoint_summary: {
          has_checkpoint: true,
          table_count: 1,
          cursor_present: true,
          table_checkpoints: [{
            table: 'bi_traffic_area',
            updated_after_present: true,
            last_id_present: true,
            version_after_present: false,
          }],
        },
        updated_at: '2026-05-22T02:00:00Z',
      }],
      sync_readiness: {
        signal: 'ready',
        dataset_signal: 'ready',
        has_sync_run: true,
        latest_status: 'succeeded',
        workflow_stage: 'completed',
        workflow_status: 'succeeded',
        document_count: 2,
        row_count: 2,
        chunk_count: 8,
        skipped_row_count: 0,
        failed_row_count: 1,
        enqueued_task_count: 6,
        row_failure_groups: [{
          table: 'bi_traffic_area',
          reason: 'mapped row has empty identity columns',
          sample_count: 1,
          reported_failed_row_count: 1,
          first_row_index: 3,
          sample_source_primary_keys: ['42'],
        }],
        row_failure_samples: [{
          table: 'bi_traffic_area',
          row_index: 3,
          source_primary_key: '42',
          reason: 'mapped row has empty identity columns',
        }],
        table_counts: [{
          table: 'bi_traffic_area',
          document_count: 2,
          row_count: 2,
          chunk_count: 8,
          skipped_row_count: 0,
          failed_row_count: 1,
        }],
        checkpoint_summary: {
          has_checkpoint: true,
          table_count: 1,
          cursor_present: true,
          table_checkpoints: [{
            table: 'bi_traffic_area',
            updated_after_present: true,
            last_id_present: true,
          }],
        },
        updated_at: '2026-05-22T02:00:00Z',
      },
      health_findings: {
        signal: 'attention',
        blocking_count: 0,
        warning_count: 3,
        info_count: 0,
        items: [{
          severity: 'warning',
          code: 'mapped_tables_without_documents',
          title: '映射表暂无入库文档',
          message: '部分已映射表还没有同步出可问答文档。',
          count: 1,
        }, {
          severity: 'warning',
          code: 'row_conversion_failures',
          title: '存在行级转换失败',
          message: '部分数据库行未能转换为文档，观测页已保留少量失败样本。',
          count: 1,
        }, {
          severity: 'warning',
          code: 'semantic_profile_table_gap',
          title: '语义画像缺少映射表',
          message: '已映射表未出现在当前语义画像中，可能需要重新画像并应用配置：empty_table',
          table: 'empty_table',
          count: 1,
        }],
      },
    },
  });

  assert.equal(status.loaded, true);
  assert.equal(status.dataset.title, 'HY SQL');
  assert.equal(status.dataset.datasetExternalId, 'hy-sql-main');
  assert.equal(status.datasets.length, 1);
  assert.equal(status.datasets[0].datasetExternalId, 'hy-sql-main');
  assert.equal(status.datasets[0].readiness.signal, 'ready');
  assert.equal(status.datasets[0].documentCount, 2);
  assert.equal(status.datasets[0].indexedChunkCount, 8);
  assert.equal(status.datasets[0].retrievalEvidenceCount, 2);
  assert.equal(status.datasetReadiness.label, '可问');
  assert.equal(status.syncReadiness.label, '可问');
  assert.equal(status.syncReadiness.workflowStage, 'completed');
  assert.equal(status.syncReadiness.enqueuedTaskCount, 6);
  assert.equal(status.syncReadiness.rowCount, 2);
  assert.equal(status.syncReadiness.skippedRowCount, 0);
  assert.equal(status.syncReadiness.failedRowCount, 1);
  assert.deepEqual(status.syncReadiness.rowFailureSamples, [{
    table: 'bi_traffic_area',
    rowIndex: 3,
    reason: 'mapped row has empty identity columns',
    sourcePrimaryKey: '42',
  }]);
  assert.deepEqual(status.syncReadiness.rowFailureGroups, [{
    table: 'bi_traffic_area',
    reason: 'mapped row has empty identity columns',
    sampleCount: 1,
    reportedFailedRowCount: 1,
    firstRowIndex: 3,
    sampleSourcePrimaryKeys: ['42'],
  }]);
  assert.equal(status.syncReadiness.checkpointSummary.label, '表检查点 1 · 游标已隐藏');
  assert.equal(status.syncReadiness.checkpointSummary.tableCheckpoints[0].updatedAfterPresent, true);
  assert.equal(status.healthFindings.signal, 'attention');
  assert.equal(status.healthFindings.label, '需关注');
  assert.equal(status.healthFindings.warningCount, 3);
  assert.equal(status.healthFindings.items[0].code, 'mapped_tables_without_documents');
  assert.equal(status.healthFindings.items[1].count, 1);
  assert.equal(status.healthFindings.items[2].table, 'empty_table');
  assert.equal(status.semanticProfile.configured, true);
  assert.equal(status.semanticProfile.tableCount, 1);
  assert.equal(status.semanticProfile.metricCount, 3);
  assert.equal(status.semanticProfile.dimensionCount, 4);
  assert.equal(status.semanticProfile.reportSuggestionCount, 2);
  assert.equal(status.semanticProfile.tables[0].mappingConfidence, 92);
  assert.deepEqual(status.syncReadiness.tableCounts, [{
    table: 'bi_traffic_area',
    documentCount: 2,
    rowCount: 2,
    chunkCount: 8,
    skippedRowCount: 0,
    failedRowCount: 1,
  }]);
  assert.equal(status.tableReadiness[0].table, 'bi_traffic_area');
  assert.equal(status.tableReadiness[1].label, '未入库');
  assert.equal(status.recentSyncRuns[0].syncRunId, 'run-001');
  assert.equal(status.recentSyncRuns[0].workflowStage, 'completed');
  assert.equal(status.recentSyncRuns[0].enqueuedTaskCount, 6);
  assert.equal(status.recentSyncRuns[0].rowCount, 2);
  assert.equal(status.recentSyncRuns[0].rowFailureSamples[0].sourcePrimaryKey, '42');
  assert.equal(status.recentSyncRuns[0].rowFailureGroups[0].reportedFailedRowCount, 1);
  assert.equal(status.recentSyncRuns[0].checkpointSummary.cursorPresent, true);
  assert.equal(status.recentSyncRuns[0].checkpointSummary.tableCount, 1);
  assert.deepEqual(status.recentSyncRuns[0].tableCounts, [{
    table: 'bi_traffic_area',
    documentCount: 2,
    rowCount: 2,
    chunkCount: 8,
    skippedRowCount: 0,
    failedRowCount: 1,
  }]);
});

test('driftSignalLabel covers source recovery states', () => {
  assert.equal(driftSignalLabel('acl_missing'), 'ACL 缺失');
  assert.equal(driftSignalLabel('acl_stale'), 'ACL 过期');
  assert.equal(driftSignalLabel('sync_failed'), '同步失败');
  assert.equal(driftSignalLabel('sync_recovering'), '恢复中');
});

test('databaseSourceHealthSignalLabel covers selected-source health states', () => {
  assert.equal(databaseSourceHealthSignalLabel('ok'), '健康');
  assert.equal(databaseSourceHealthSignalLabel('attention'), '需关注');
  assert.equal(databaseSourceHealthSignalLabel('blocking'), '阻断');
  assert.equal(databaseSourceHealthSignalLabel('in_progress'), '处理中');
});

test('databaseSourceReadOnlyStatusLabel covers read-only database states', () => {
  assert.equal(databaseSourceReadOnlyStatusLabel('read_only_ready'), '只读可用');
  assert.equal(databaseSourceReadOnlyStatusLabel('attached'), '已挂接');
  assert.equal(databaseSourceReadOnlyStatusLabel('not_synced'), '未同步');
  assert.equal(databaseSourceReadOnlyStatusLabel('analyzing'), '分析中');
  assert.equal(databaseSourceReadOnlyStatusLabel('operator_required'), '需要 operator 处理');
  assert.equal(databaseSourceReadOnlyStatusLabel('not_attached'), '未挂接');
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

test('normalizeExternalConversationTest keeps conversation test fields readable', () => {
  const item = normalizeExternalConversationTest({
    event_id: 'evt-1',
    integration_id: 'generic-chat-main',
    integration_display_name: 'Generic Chat',
    platform: 'generic_chat',
    conversation_external_id: 'conv-001',
    sender_external_id: 'user-001',
    message_external_id: 'msg-001',
    direction: 'inbound',
    assistant_run_id: 'run-001',
    assistant_status: 'completed',
    assistant_event: 'assistant_run.external_channel_model_reply_completed',
    question_text: 'DataMax 怎么解析 docx？',
    answer_text: '调用 documents/parse，等到 indexed 后即可用于问答。',
    duration_ms: 2345,
    payload_summary: {
      text_chars: 5,
      output_format: 'markdown_table',
    },
  });

  assert.equal(item.integrationDisplayName, 'Generic Chat');
  assert.equal(item.conversationExternalId, 'conv-001');
  assert.equal(item.senderExternalId, 'user-001');
  assert.equal(item.assistantRunId, 'run-001');
  assert.equal(item.payloadSummary.output_format, 'markdown_table');
  assert.equal(item.questionText, 'DataMax 怎么解析 docx？');
  assert.equal(item.answerText, '调用 documents/parse，等到 indexed 后即可用于问答。');
  assert.equal(item.durationMs, 2345);
  assert.equal(formatExternalConversationDuration(item.durationMs), '2.3s');
  assert.equal(formatExternalConversationDuration(null), '未完成');
  assert.equal(externalConversationStatusLabel(item.assistantStatus), '已回复');
  assert.equal(externalConversationStatusLabel('failed'), '失败');
  assert.equal(externalConversationStatusLabel('no_run'), '未建运行');
});

test('normalizeExternalConversationTimeline keeps event progress and artifact links readable', () => {
  const timeline = normalizeExternalConversationTimeline({
    event_id: 'evt-1',
    integration_id: 'generic-chat-main',
    integration_display_name: 'Generic Chat',
    platform: 'generic_chat',
    conversation_external_id: 'conv-001',
    message_external_id: 'msg-001',
    assistant_run_id: 'run-001',
    events: [{
      sequence_no: 20,
      event_name: 'assistant_run.external_channel_static_page_queued',
      phase: 'artifact',
      status: 'queued',
      display_text: '页面生成任务已进入队列。',
      artifact_links: ['https://v3.elepcloud.com/generated-artifacts/demo/index.html', ''],
      payload_summary: { schema: 'v3.external_channel.sse.v1' },
      debug_payload: { status: 'queued' },
      created_at: '2026-06-02T08:00:00Z',
    }],
  });

  assert.equal(timeline.integrationDisplayName, 'Generic Chat');
  assert.equal(timeline.assistantRunId, 'run-001');
  assert.equal(timeline.events.length, 1);
  assert.equal(timeline.events[0].sequenceNo, 20);
  assert.equal(timeline.events[0].displayText, '页面生成任务已进入队列。');
  assert.deepEqual(timeline.events[0].artifactLinks, [
    'https://v3.elepcloud.com/generated-artifacts/demo/index.html',
  ]);
  assert.equal(timeline.events[0].debugPayload.status, 'queued');
});

test('auditItemTypeLabel covers search evidence items', () => {
  assert.equal(auditItemTypeLabel('message'), '消息');
  assert.equal(auditItemTypeLabel('action'), '动作');
  assert.equal(auditItemTypeLabel('search_evidence'), '搜索证据');
  assert.equal(auditItemTypeLabel('outbound_reply'), '回复回推');
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
  assert.equal(
    controlResultLabel(normalizeControlResult({
      accepted: true,
      action: 'configure_reply_dispatch',
      status: 'reply_dispatch_ready',
    })),
    '助手消息回推配置已保存',
  );
  assert.equal(
    controlResultLabel(normalizeControlResult({
      accepted: true,
      action: 'configure_reply_dispatch',
      status: 'reply_dispatch_cleared',
    })),
    '助手消息回推配置已清空',
  );
});

test('codex executor helpers expose poll retry and remote task id', () => {
  const listItem = normalizeCodexExecutorTask({
    id: 'exec-001',
    kind: 'codex_host_task_workflow',
    status: 'running',
    stage: 'run_codex_host_task',
    updated_at: '2026-05-27T09:40:00Z',
  });
  assert.equal(listItem.updatedAt, '2026-05-27T09:40:00Z');
  assert.equal(workflowStatusLabel('claimed'), '执行中');

  const task = normalizeWorkflowTask({
    id: 'task-001',
    status: 'queued',
    task_key: 'run_codex_host_task',
    attempt: 2,
    max_attempts: 3,
    available_at: '2026-05-27T09:41:00Z',
    error: 'Cloudflare Codex task timed out after 1800000ms; task_id=cf-001 at /Users/private/run.log token=secret-value',
    payload: {
      cloudflare_orchestrator: {
        task_id: 'cf-001',
        runtime_target_id: 'cloudflare',
      },
    },
  });
  assert.equal(task.cloudflareTaskId, 'cf-001');
  assert.equal(task.maxAttempts, 3);
  assert.equal(task.error.includes('[redacted:path]'), true);
  assert.equal(task.error.includes('token=[redacted]'), true);
  assert(!task.error.includes('/Users/private'));
  assert(!task.error.includes('secret-value'));

  const summary = codexExecutorInspectSummary({
    execution: {
      id: 'exec-001',
      status: 'running',
      stage: 'run_codex_host_task',
    },
    workflow_tasks: [task],
    artifact_manifests: [
      {
        schema: 'v3.output_artifact_manifest',
        schema_version: 1,
        artifact_type: 'static_page',
        artifact_kind: 'generated_artifact',
        primary_url: 'https://v3.elepcloud.com/generated-artifacts/demo/index.html',
        links: [
          { rel: 'public', url: 'https://v3.elepcloud.com/generated-artifacts/demo/index.html' },
          { rel: 'ppt', url: 'https://v3.elepcloud.com/generated-artifacts/demo/report.ppt' },
        ],
        safety: {
          credentials_exposed: false,
          raw_logs_exposed: false,
        },
      },
      {
        schema: 'legacy',
        schema_version: 1,
        artifact_type: 'raw_log',
      },
    ],
  });
  assert.equal(summary.poll_retry.active, true);
  assert.equal(summary.poll_retry.cloudflare_task_id, 'cf-001');
  assert.equal(summary.latest_task.attempt, 2);
  assert.equal(summary.artifact_manifests.length, 1);
  assert.equal(summary.artifact_manifests[0].artifactType, 'static_page');
  assert.equal(summary.artifact_manifests[0].primaryUrl, 'https://v3.elepcloud.com/generated-artifacts/demo/index.html');
  assert.deepEqual(summary.artifact_manifests[0].artifactLinks, [
    { rel: 'primary', url: 'https://v3.elepcloud.com/generated-artifacts/demo/index.html' },
    { rel: 'ppt', url: 'https://v3.elepcloud.com/generated-artifacts/demo/report.ppt' },
  ]);
});

test('workflow queue stats helpers normalize logical queue counts', () => {
  const stats = normalizeWorkflowQueueStats({
    generated_at: '2026-05-30T12:00:00Z',
    execution_count: 5,
    task_count: 8,
    queues: [
      {
        logical_queue: 'static_page_publish',
        physical_queues: ['codex_host'],
        task_count: 3,
        queued: 2,
        running: 1,
        retrying: 2,
        failed: 0,
        next_available_at: '2026-05-30T12:00:30Z',
        finished_duration_p50_ms: 42_000,
        finished_duration_p95_ms: 180_000,
        succeeded_duration_p50_ms: 40_000,
        succeeded_duration_p95_ms: 120_000,
        task_keys: [
          {
            logical_task_key: 'poll_static_page_publish',
            physical_task_keys: ['run_codex_host_task'],
            task_count: 2,
            queued: 2,
            retrying: 2,
            finished_duration_p95_ms: 180_000,
            succeeded_duration_p95_ms: 120_000,
          },
        ],
      },
    ],
  });

  assert.equal(stats.executionCount, 5);
  assert.equal(stats.taskCount, 8);
  assert.equal(stats.queues[0].logicalQueue, 'static_page_publish');
  assert.equal(stats.queues[0].retrying, 2);
  assert.equal(stats.queues[0].finishedDurationP50Ms, 42_000);
  assert.equal(stats.queues[0].finishedDurationP95Ms, 180_000);
  assert.equal(stats.queues[0].succeededDurationP50Ms, 40_000);
  assert.equal(stats.queues[0].succeededDurationP95Ms, 120_000);
  assert.equal(stats.queues[0].taskKeys[0].logicalTaskKey, 'poll_static_page_publish');
  assert.equal(stats.queues[0].taskKeys[0].finishedDurationP95Ms, 180_000);
  assert.equal(stats.queues[0].taskKeys[0].succeededDurationP95Ms, 120_000);
  assert.equal(formatWorkflowDuration(stats.queues[0].finishedDurationP50Ms), '42s');
  assert.equal(formatWorkflowDuration(null), '无完成样本');
  assert.equal(workflowQueueLabel('static_page_publish'), '页面发布');
  assert.equal(workflowTaskKeyLabel('poll_static_page_publish'), '轮询页面发布');
});

test('document processing diagnostics normalize dedup, parse, enrichment, and latest task state', () => {
  const diagnostic = normalizeDocumentProcessingDiagnostic({
    document_id: 'doc-001',
    external_id: 'bi_traffic_area:1',
    dataset_ids: ['ds-001', 'ds-001', 'ds-002'],
    canonical_document_id: 'doc-canonical',
    dedup_state: 'duplicate',
    parse_status: 'parsed',
    index_status: 'indexed',
    enrichment_counts: {
      succeeded: 2,
    },
    latest_task: {
      task_id: 'task-001',
      task_key: 'fact_index_v2',
      status: 'succeeded',
      attempt: 1,
      max_attempts: 2,
      updated_at: '2026-06-07T03:00:00Z',
    },
    waiting_reason: 'duplicate_uses_canonical',
    title: 'should not be included',
    object_key: '/srv/aiv3/private/customer.md',
    raw_content: 'should not be included',
    provider_payload: { secret: 'should not be included' },
  });

  assert.equal(diagnostic.documentId, 'doc-001');
  assert.equal(diagnostic.externalId, 'bi_traffic_area:1');
  assert.deepEqual(diagnostic.datasetIds, ['ds-001', 'ds-002']);
  assert.equal(diagnostic.canonicalDocumentId, 'doc-canonical');
  assert.equal(diagnostic.dedupState, 'duplicate');
  assert.equal(diagnostic.dedupLabel, '重复归档');
  assert.equal(diagnostic.parseLabel, '已完成');
  assert.equal(diagnostic.indexLabel, '已索引');
  assert.equal(diagnostic.enrichmentStatus, 'duplicate');
  assert.equal(diagnostic.latestTask.id, 'task-001');
  assert.equal(diagnostic.latestTaskLabel, 'fact_index_v2 · 成功');
  assert.equal(documentProcessingStatusLabel('parse_or_index_pending'), '解析/索引等待');

  const rendered = JSON.stringify(diagnostic);
  assert(!rendered.includes('should not be included'));
  assert(!rendered.includes('/srv/aiv3/private/customer.md'));
});

test('document processing diagnostics redact paths, URLs, bearer tokens, and secret assignments', () => {
  const [diagnostic] = normalizeDocumentProcessingDiagnostics([{
    documentId: 'doc-redacted',
    dedupState: 'canonical',
    parseStatus: 'failed',
    indexStatus: 'failed',
    enrichmentStatus: 'blocked',
    failureSummary: 'failed at /Users/manslive01/private/a.md from https://private.example/a?token=raw Bearer abcdefghijklmnop api_key=secret-value v3in_private_token',
    latestTask: {
      id: 'task-redacted',
      status: 'failed',
      error: 'C:\\Users\\soulzyn\\secret.txt cookie=session-secret',
    },
  }]);

  assert.equal(diagnostic.tone, 'critical');
  assert.equal(diagnostic.failureSummary.includes('[redacted:path]'), true);
  assert.equal(diagnostic.failureSummary.includes('[redacted:url]'), true);
  assert.equal(diagnostic.failureSummary.includes('Bearer [redacted:token]'), true);
  assert.equal(diagnostic.failureSummary.includes('api_key=[redacted]'), true);
  assert.equal(diagnostic.failureSummary.includes('[redacted:token]'), true);
  assert.equal(diagnostic.latestTask.error.includes('[redacted:path]'), true);
  assert.equal(diagnostic.latestTask.error.includes('cookie=[redacted]'), true);
  const rendered = JSON.stringify(diagnostic);
  assert(!rendered.includes('manslive01/private'));
  assert(!rendered.includes('private.example'));
  assert(!rendered.includes('session-secret'));
  assert(!rendered.includes('v3in_private_token'));
});

test('buildOperationsSummary groups sanitized operator counters', () => {
  const integrations = [
    normalizeIntegrationSummary({
      integration_id: 'generic-chat-main',
      integration_kind: 'channel',
      provider: 'generic_chat',
      health_status: 'healthy',
      action_summary: {
        waiting_result_count: 1,
      },
      artifact_summary: {
        signal: 'artifact_confirmation_pending',
      },
      config_summary: {
        inbound_auth_configured: true,
        inbound_auth_mode: 'bearer',
        inbound_bearer_token: 'v3in_should_not_escape',
      },
    }),
    normalizeIntegrationSummary({
      integration_id: 'hy-sql-traffic-area',
      integration_kind: 'source',
      provider: 'mysql',
      drift_summary: {
        failed_sync_count: 1,
        database_dataset_readiness: {
          signal: 'ready',
          document_count: 384,
          indexed_document_count: 384,
        },
        document_diagnostics: [{
          document_id: 'doc-database-001',
          external_id: 'bi_traffic_area:1',
          dataset_ids: ['ds-database'],
          canonical_document_id: 'doc-database-001',
          dedup_state: 'canonical',
          parse_status: 'parsed',
          index_status: 'indexed',
          enrichment_status: 'succeeded',
          latest_task: {
            task_id: 'task-database-001',
            task_key: 'fact_index_v2',
            status: 'succeeded',
            updated_at: '2026-06-07T03:00:00Z',
          },
          failure_summary: 'should redact https://private.example/doc token=secret',
          object_key: '/Volumes/private/customer.json',
        }],
      },
      config_summary: {
        database_source: {
          kind: 'mysql',
          database: 'ops',
          connection_env: 'MYSQL_PASSWORD_SHOULD_NOT_ESCAPE',
          table_count: 6,
        },
      },
    }),
  ];
  const workflowQueueStats = normalizeWorkflowQueueStats({
    generated_at: '2026-06-06T06:00:00Z',
    execution_count: 8,
    task_count: 12,
    queues: [
      {
        logical_queue: 'static_page_publish',
        task_count: 4,
        queued: 1,
        running: 1,
        retrying: 0,
        failed: 0,
        succeeded_duration_p95_ms: 120_000,
        task_keys: [{ logical_task_key: 'poll_static_page_publish', queued: 1 }],
      },
      {
        logical_queue: 'document_enrichment',
        task_count: 2,
        queued: 2,
        failed: 0,
        task_keys: [{ logical_task_key: 'fact_index_v2', queued: 2 }],
      },
      {
        logical_queue: 'codex_fixed_task',
        task_count: 1,
        queued: 1,
        failed: 0,
        task_keys: [{ logical_task_key: 'submit_codex_fixed_task', queued: 1 }],
      },
    ],
  });
  const summary = buildOperationsSummary({
    integrations,
    workflowQueueStats,
    modelGatewayStatus: {
      lanes: [{
        lane: 'assistant_chat',
        routing_mode: 'database_primary',
        active_count: 2,
        queued_count: 1,
        max_concurrency: 20,
        profile_count: 2,
        active_profile_count: 2,
      }],
      providers: [{
        lane: 'assistant_chat',
        profile_id: 'rightcode-default',
        runtime_failure_count: 0,
      }],
      runtime: {
        externalChannel: {
          activeConversations: 3,
        },
      },
    },
    codexExecutorTasks: [
      { kind: 'answer_quality_autofix', stage: 'queued' },
    ],
  });

  const cardsByKey = Object.fromEntries(summary.cards.map((card) => [card.key, card]));
  assert.equal(cardsByKey.ordinary_chat.value, '1');
  assert.equal(cardsByKey.model_lane.value, '2/20');
  assert.equal(cardsByKey.workflow_backlog.value, '5');
  assert.equal(cardsByKey.static_report.value, '2');
  assert.equal(cardsByKey.data_ingestion.value, '1/1');
  assert.equal(cardsByKey.document_enrichment.value, '2');
  assert.equal(cardsByKey.document_enrichment.detail.includes('文档诊断 1'), true);
  assert.equal(cardsByKey.low_quality.value, '1');
  assert.equal(cardsByKey.low_quality.detail.includes('不拦截正常回复'), true);
  assert.equal(summary.documentDiagnostics.length, 1);
  assert.equal(summary.documentDiagnostics[0].documentId, 'doc-database-001');
  assert.equal(summary.documentDiagnostics[0].dedupState, 'canonical');
  assert.equal(summary.documentDiagnostics[0].indexStatus, 'indexed');
  assert.equal(summary.documentDiagnostics[0].enrichmentStatus, 'succeeded');
  assert.equal(summary.documentDiagnostics[0].failureSummary.includes('[redacted:url]'), true);
  assert.equal(summary.documentDiagnostics[0].failureSummary.includes('token=[redacted]'), true);
  const rendered = JSON.stringify(summary);
  assert(!rendered.includes('v3in_should_not_escape'));
  assert(!rendered.includes('MYSQL_PASSWORD_SHOULD_NOT_ESCAPE'));
  assert(!rendered.includes('/Volumes/private/customer.json'));
  assert(!rendered.includes('private.example'));
  assert(!rendered.includes('token=secret'));
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
