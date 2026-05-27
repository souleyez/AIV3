export const DEFAULT_THIRD_PARTY_API_BASE_URL = 'https://v3.elepcloud.com';

export const EXTERNAL_AUDIT_FILTERS = [
  { key: 'all', label: '全部', params: {} },
  { key: 'actions', label: '动作', params: { itemType: 'action' } },
  { key: 'search', label: '搜索证据', params: { itemType: 'search_evidence' } },
  { key: 'callbacks', label: '结果回调', params: { itemType: 'action', actionState: 'result_callback' } },
  { key: 'waiting', label: '待结果', params: { itemType: 'action', actionState: 'waiting_result' } },
  { key: 'failed', label: '失败/阻断', params: { itemType: 'action', actionState: 'failed' } },
];

export const EXTERNAL_INTEGRATION_MODES = [
  {
    key: 'standard_bot',
    title: '标准机器人模式',
    status: '可选',
    summary: '飞书、Lark、企业微信等平台事件进入 V3 适配器，再转为统一外部通道事件。',
    docs: [
      'docs/integrations/third-party-integration-api.zh-CN.html',
      'docs/integrations/third-party-integration-api.zh-CN.md',
    ],
    documentLinks: [
      {
        key: 'complete-third-party-html',
        label: '完整文档 HTML',
        href: '/external-integrations/third-party-integration-api.zh-CN.html',
        kind: 'html',
        target: '_blank',
      },
      {
        key: 'complete-third-party-md',
        label: '完整文档 MD 下载',
        href: '/external-integrations/third-party-integration-api.zh-CN.md',
        kind: 'markdown',
        download: true,
      },
    ],
    checklist: [
      '平台事件验签与消息解密',
      '外部用户、会话和机器人 ID 映射',
      '统一消息事件写入 V3 通道',
      '文档源、文档解析和解析详情查询',
      '默认提示词、输出格式和本轮 skill 策略',
      '模板文档解析和模板生成产物',
      '用户确认、动作派发和结果回调',
      '审计列表、失败阻断和 trace 导出',
    ],
    guardrail: '按平台官方验签；不要把平台 token 当作 V3 出站派发凭证。',
  },
  {
    key: 'pure_third_party',
    title: '纯第三方最简版',
    status: '常规接入',
    summary: '只保留文档解析、聊天同步、按模板生成报表三段最小接口，适合第三方 AI 小上下文直接读取。',
    docs: [
      'docs/integrations/pure-third-party-integration-guide.zh-CN.html',
      'docs/integrations/pure-third-party-integration-guide.zh-CN.md',
      'docs/integrations/third-party-integration-api.zh-CN.md',
    ],
    documentLinks: [
      {
        key: 'pure-third-party-html',
        label: '最简版 HTML',
        href: '/external-integrations/pure-third-party-integration-guide.zh-CN.html',
        kind: 'html',
        target: '_blank',
      },
      {
        key: 'pure-third-party-md',
        label: '最简版 MD 下载',
        href: '/external-integrations/pure-third-party-integration-guide.zh-CN.md',
        kind: 'markdown',
        download: true,
      },
      {
        key: 'pure-complete-third-party-html',
        label: '完整文档 HTML',
        href: '/external-integrations/third-party-integration-api.zh-CN.html',
        kind: 'html',
        target: '_blank',
      },
      {
        key: 'pure-complete-third-party-md',
        label: '完整文档 MD 下载',
        href: '/external-integrations/third-party-integration-api.zh-CN.md',
        kind: 'markdown',
        download: true,
      },
    ],
    checklist: [
      '文档解析：parse 与 parse-detail',
      '聊天同步：用户 ID、会话 ID、消息 ID',
      '回答控制：default_prompt、output_format、render_mode',
      '本轮文档范围：source_id 和 document_external_ids',
      '模板列表：模板 ID、版本、输出类型',
      '模板解析：模板作为普通文档入库',
      '按模板生成产物：artifact_type + template',
      '产物查询、预览和下载链接',
    ],
    guardrail: '凭证线下交付；观测页只展示模式和文档路径，不展示 token、客户 endpoint 或密钥。',
  },
];

export function thirdPartyApiBaseUrl() {
  const configured = process.env.NEXT_PUBLIC_THIRD_PARTY_API_BASE_URL || DEFAULT_THIRD_PARTY_API_BASE_URL;
  return configured.endsWith('/') ? configured.slice(0, -1) : configured;
}

export function buildThirdPartyApiUrl(pathname) {
  const path = pathname.startsWith('/') ? pathname : `/${pathname}`;
  return `${thirdPartyApiBaseUrl()}${path}`;
}

export function buildExternalAuditQuery(filter = {}) {
  const params = new URLSearchParams();
  const itemType = filter.itemType || filter.item_type;
  const actionState = filter.actionState || filter.action_state;
  const actionId = filter.actionId || filter.action_id;
  const limit = filter.limit;
  if (itemType && itemType !== 'all') {
    params.set('item_type', itemType);
  }
  if (actionState && actionState !== 'all') {
    params.set('action_state', actionState);
  }
  if (actionId) {
    params.set('action_id', actionId);
  }
  if (Number.isFinite(Number(limit)) && Number(limit) > 0) {
    params.set('limit', String(Math.floor(Number(limit))));
  }
  const query = params.toString();
  return query ? `?${query}` : '';
}

export function buildExternalActionPermalink({
  baseUrl = DEFAULT_THIRD_PARTY_API_BASE_URL,
  pathname = '/external-integrations',
  integrationId = '',
  auditFilterKey = '',
  actionId = '',
} = {}) {
  const url = new URL(pathname || '/external-integrations', baseUrl || DEFAULT_THIRD_PARTY_API_BASE_URL);
  if (integrationId) {
    url.searchParams.set('integration_id', integrationId);
  }
  if (auditFilterKey && auditFilterKey !== 'all') {
    url.searchParams.set('audit_filter', auditFilterKey);
  }
  if (actionId) {
    url.searchParams.set('action_id', actionId);
  }
  return `${url.origin}${url.pathname}${url.search}`;
}

export function buildExternalActionTrace({ integration = {}, action = {}, generatedAt = new Date().toISOString() } = {}) {
  return {
    report_type: 'external_action_trace',
    generated_at: generatedAt,
    integration: {
      id: integration.id || '',
      kind: integration.kind || '',
      provider: integration.provider || '',
      display_name: integration.displayName || '',
      health_status: integration.healthStatus || '',
      action_signal: integration.actionSignal || '',
      artifact_signal: integration.artifactSignal || '',
      drift_signal: integration.driftSignal || '',
    },
    action: {
      item_type: action.itemType || '',
      action_id: action.actionId || '',
      status: action.status || '',
      failure_kind: action.failureKind || '',
      assistant_run_id: action.assistantRunId || '',
      updated_at: action.createdAt || null,
      summary: action.summary && typeof action.summary === 'object' ? action.summary : {},
    },
    redaction: {
      raw_third_party_payload_included: false,
      raw_callback_message_included: false,
      source: 'v3_external_integration_audit_summary',
    },
  };
}

export function buildDatabaseSourceStatusExport({
  integration = {},
  status = {},
  generatedAt = new Date().toISOString(),
} = {}) {
  const source = databaseSourceSummary(integration);
  const normalized = status?.loaded ? status : normalizeDatabaseSourceStatus(status);
  return {
    report_type: 'database_source_status_summary',
    generated_at: generatedAt,
    source: {
      id: integration.id || '',
      kind: integration.kind || '',
      provider: integration.provider || '',
      display_name: integration.displayName || '',
      health_status: integration.healthStatus || '',
      drift_signal: integration.driftSignal || '',
    },
    database_source: {
      configured: source.configured,
      valid: source.valid,
      kind: source.kind,
      database: source.database,
      connection_env: source.connectionEnv,
      default_dataset_id: source.defaultDatasetId,
      table_count: source.tableCount,
      tables: source.tables,
    },
    dataset: normalized.dataset || {},
    dataset_readiness: normalized.datasetReadiness || {},
    sync_readiness: {
      ...(normalized.syncReadiness || {}),
      rowFailureGroups: normalized.syncReadiness?.rowFailureGroups || [],
      rowFailureSamples: normalized.syncReadiness?.rowFailureSamples || [],
      tableCounts: normalized.syncReadiness?.tableCounts || [],
    },
    semantic_profile: normalized.semanticProfile || {},
    health_findings: normalized.healthFindings || {},
    table_readiness: normalized.tableReadiness || [],
    recent_sync_runs: (normalized.recentSyncRuns || []).map((run) => ({
      syncRunId: run.syncRunId || '',
      syncKind: run.syncKind || '',
      status: run.status || '',
      failureKind: run.failureKind || '',
      lastError: run.lastError || '',
      failedTaskKey: run.failedTaskKey || '',
      documentCount: run.documentCount || 0,
      rowCount: run.rowCount || 0,
      skippedRowCount: run.skippedRowCount || 0,
      failedRowCount: run.failedRowCount || 0,
      enqueuedTaskCount: run.enqueuedTaskCount || 0,
      tableCounts: run.tableCounts || [],
      rowFailureGroups: run.rowFailureGroups || [],
      checkpointSummary: run.checkpointSummary || {},
      workflowStage: run.workflowStage || '',
      workflowStatus: run.workflowStatus || '',
      updatedAt: run.updatedAt || null,
    })),
    redaction: {
      raw_database_credentials_included: false,
      raw_sql_included: false,
      raw_source_cursors_included: false,
      source: 'v3_selected_database_source_status',
    },
  };
}

export function databaseSourceStatusExportFilename(integration = {}) {
  const sourceId = String(integration.id || integration.source_id || 'database-source')
    .replace(/[^a-zA-Z0-9._-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80) || 'database-source';
  return `${sourceId}-database-status.json`;
}

export function externalActionTraceFilename(action = {}) {
  const actionId = String(action.actionId || action.action_id || 'external-action')
    .replace(/[^a-zA-Z0-9._-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80) || 'external-action';
  return `${actionId}-trace.json`;
}

export function normalizeIntegrationSummary(raw = {}) {
  const pending = numberOrZero(raw.pending_action_count);
  const blocked = numberOrZero(raw.blocked_action_count);
  const failed = numberOrZero(raw.failed_action_count);
  const dispatched = numberOrZero(raw.dispatched_action_count);
  const driftSummary = raw.drift_summary && typeof raw.drift_summary === 'object' ? raw.drift_summary : {};
  const drift = driftSignal(driftSummary);
  const artifactSummary = raw.artifact_summary && typeof raw.artifact_summary === 'object' ? raw.artifact_summary : {};
  const artifact = artifactSignal(artifactSummary);
  const searchSummary = raw.search_summary && typeof raw.search_summary === 'object' ? raw.search_summary : {};
  const search = searchEvidenceSignal(searchSummary);
  const actionSummary = raw.action_summary && typeof raw.action_summary === 'object' ? raw.action_summary : {};
  const action = actionSignal(actionSummary);
  return {
    id: String(raw.integration_id || ''),
    kind: String(raw.integration_kind || 'unknown'),
    displayName: String(raw.display_name || raw.integration_id || '未命名集成'),
    provider: String(raw.provider || 'unknown'),
    status: String(raw.status || 'unknown'),
    healthStatus: String(raw.health_status || 'unknown'),
    lastEventAt: raw.last_event_at || null,
    lastSyncAt: raw.last_sync_at || null,
    lastSuccessAt: raw.last_success_at || null,
    lastFailureAt: raw.last_failure_at || null,
    disabledAt: raw.disabled_at || null,
    pendingActionCount: pending,
    blockedActionCount: blocked,
    failedActionCount: failed,
    dispatchedActionCount: dispatched,
    latestActionAt: raw.latest_action_at || null,
    actionSummary,
    actionSignal: action,
    configSummary: raw.config_summary && typeof raw.config_summary === 'object' ? raw.config_summary : {},
    searchSummary,
    searchSignal: search,
    searchEvidenceRequiredCount: numberOrZero(searchSummary.required_count),
    driftSummary,
    driftSignal: drift,
    artifactSummary,
    artifactSignal: artifact,
    signal: integrationSignal({
      pending,
      blocked,
      failed,
      dispatched,
      actionSignal: action,
      healthStatus: raw.health_status,
      status: raw.status,
    }),
  };
}

export function normalizeAuditItem(raw = {}) {
  return {
    itemType: String(raw.item_type || 'event'),
    createdAt: raw.created_at || null,
    assistantRunId: raw.assistant_run_id || null,
    actionId: raw.action_id || null,
    status: raw.status || null,
    failureKind: raw.failure_kind || null,
    summary: raw.summary && typeof raw.summary === 'object' ? raw.summary : {},
  };
}

export function normalizeExternalConversationTest(raw = {}) {
  const payloadSummary = raw.payload_summary && typeof raw.payload_summary === 'object'
    ? raw.payload_summary
    : {};
  const questionText = conversationDisplayText(
    raw.question_text
      || raw.question
      || raw.user_text
      || raw.prompt
      || payloadSummary.question_text
      || payloadSummary.text,
  );
  const answerText = conversationDisplayText(
    raw.answer_text
      || raw.answer
      || raw.reply_text
      || raw.response_text
      || raw.assistant_answer_excerpt
      || payloadSummary.answer_text
      || payloadSummary.assistant_answer_excerpt,
  );
  return {
    eventId: String(raw.event_id || ''),
    integrationId: String(raw.integration_id || ''),
    integrationDisplayName: String(raw.integration_display_name || raw.integration_id || '未命名集成'),
    platform: String(raw.platform || 'unknown'),
    conversationExternalId: String(raw.conversation_external_id || ''),
    senderExternalId: raw.sender_external_id ? String(raw.sender_external_id) : '',
    messageExternalId: String(raw.message_external_id || ''),
    direction: String(raw.direction || 'inbound'),
    assistantRunId: raw.assistant_run_id ? String(raw.assistant_run_id) : '',
    assistantStatus: String(raw.assistant_status || 'unknown'),
    assistantEvent: raw.assistant_event ? String(raw.assistant_event) : '',
    questionText,
    answerText,
    durationMs: normalizeConversationDuration(raw.duration_ms ?? raw.latency_ms ?? raw.elapsed_ms),
    createdAt: raw.created_at || null,
    assistantUpdatedAt: raw.assistant_updated_at || null,
    payloadSummary,
  };
}

export function databaseSourceSummary(integration = {}) {
  const configSummary = integration?.configSummary && typeof integration.configSummary === 'object'
    ? integration.configSummary
    : integration?.config_summary && typeof integration.config_summary === 'object'
      ? integration.config_summary
      : {};
  const raw = configSummary.database_source && typeof configSummary.database_source === 'object'
    ? configSummary.database_source
    : configSummary.databaseSource && typeof configSummary.databaseSource === 'object'
      ? configSummary.databaseSource
    : {};
  const tables = Array.isArray(raw.tables)
    ? raw.tables.map((table) => String(table || '').trim()).filter(Boolean)
    : [];
  const configured = raw.configured !== false && Boolean(
    raw.kind
      || raw.database
      || raw.connection_env
      || raw.connectionEnv
      || tables.length
      || Number(raw.table_count || raw.tableCount) > 0,
  );
  if (!configured) {
    return {
      configured: false,
      valid: true,
      kind: '',
      database: '',
      connectionEnv: '',
      defaultDatasetId: '',
      tableCount: 0,
      tables: [],
      error: '',
    };
  }
  return {
    configured: true,
    valid: raw.valid !== false,
    kind: String(raw.kind || 'database'),
    database: String(raw.database || ''),
    connectionEnv: String(raw.connection_env || raw.connectionEnv || ''),
    defaultDatasetId: String(raw.default_dataset_id || raw.defaultDatasetId || ''),
    tableCount: numberOrZero(raw.table_count ?? raw.tableCount) || tables.length,
    tables,
    error: String(raw.error || ''),
  };
}

export function databaseSourceMetrics(integration = {}) {
  const source = databaseSourceSummary(integration);
  if (!source.configured) {
    return [];
  }
  const driftSummary = integration?.driftSummary && typeof integration.driftSummary === 'object'
    ? integration.driftSummary
    : integration?.drift_summary && typeof integration.drift_summary === 'object'
      ? integration.drift_summary
      : {};
  const readiness = databaseSourceReadiness(integration);
  return [
    { label: '问答就绪', value: readiness.label },
    { label: '数据库', value: source.database || '未配置' },
    { label: '连接引用', value: source.connectionEnv || '未配置' },
    { label: '默认数据集', value: readiness.defaultDatasetId || source.defaultDatasetId || '未绑定' },
    { label: '表数量', value: source.tableCount },
    { label: '最近同步', value: driftSummary.latest_sync_status || '无记录' },
    { label: '失败同步', value: numberOrZero(driftSummary.failed_sync_count) },
  ];
}

export function databaseSourceReadiness(integration = {}) {
  const driftSummary = integration?.driftSummary && typeof integration.driftSummary === 'object'
    ? integration.driftSummary
    : integration?.drift_summary && typeof integration.drift_summary === 'object'
      ? integration.drift_summary
      : {};
  const raw = driftSummary.database_dataset_readiness && typeof driftSummary.database_dataset_readiness === 'object'
    ? driftSummary.database_dataset_readiness
    : {};
  return normalizeDatabaseReadiness(raw);
}

export function normalizeDatabaseSourceStatus(raw = {}) {
  const status = raw?.status && typeof raw.status === 'object' ? raw.status : raw;
  const dataset = status?.dataset && typeof status.dataset === 'object' ? status.dataset : {};
  const datasets = Array.isArray(status?.datasets)
    ? status.datasets.map((item) => ({
      datasetId: String(item?.dataset_id || item?.datasetId || ''),
      key: String(item?.key || ''),
      title: String(item?.title || ''),
      lifecycle: String(item?.lifecycle || ''),
      datasetExternalId: String(item?.dataset_external_id || item?.datasetExternalId || ''),
      isDefault: item?.is_default === true || item?.isDefault === true,
      updatedAt: item?.updated_at || item?.updatedAt || null,
    })).filter((item) => item.datasetId)
    : [];
  const tableReadiness = Array.isArray(status?.table_readiness)
    ? status.table_readiness.map((table) => ({
      ...normalizeDatabaseReadiness(table),
      table: String(table?.table || ''),
    })).filter((table) => table.table)
    : [];
  const recentSyncRuns = Array.isArray(status?.recent_sync_runs)
    ? status.recent_sync_runs.map((run) => {
      const counts = run?.counts && typeof run.counts === 'object' ? run.counts : {};
      const checkpoint = run?.checkpoint && typeof run.checkpoint === 'object' ? run.checkpoint : {};
      const checkpointSummary = normalizeDatabaseCheckpointSummary(run?.checkpoint_summary || run?.checkpointSummary || {});
      return {
        syncRunId: String(run?.sync_run_id || run?.syncRunId || ''),
        syncKind: String(run?.sync_kind || run?.syncKind || ''),
        status: String(run?.status || 'unknown'),
        failureKind: run?.failure_kind || run?.failureKind || '',
        lastError: String(counts.last_error || counts.lastError || ''),
        failedTaskKey: String(counts.failed_task_key || counts.failedTaskKey || ''),
        documentCount: numberOrZero(
          counts.documents_ingested
          ?? counts.documentsIngested
          ?? counts.content_document_count
          ?? counts.contentDocumentCount
          ?? counts.metadata_document_count
          ?? counts.metadataDocumentCount
          ?? counts.document_count
          ?? counts.documentCount,
        ),
        rowCount: numberOrZero(
          counts.row_count
          ?? counts.rowCount
          ?? counts.content_row_count
          ?? counts.contentRowCount
          ?? counts.metadata_row_count
          ?? counts.metadataRowCount
          ?? counts.documents_ingested
          ?? counts.documentsIngested
          ?? counts.content_document_count
          ?? counts.contentDocumentCount
          ?? counts.metadata_document_count
          ?? counts.metadataDocumentCount
          ?? counts.document_count
          ?? counts.documentCount,
        ),
        skippedRowCount: numberOrZero(counts.skipped_row_count ?? counts.skippedRowCount),
        failedRowCount: numberOrZero(counts.failed_row_count ?? counts.failedRowCount),
        rowFailureSamples: normalizeDatabaseRowFailureSamples(counts.row_failure_samples || counts.rowFailureSamples),
        rowFailureGroups: normalizeDatabaseRowFailureGroups(
          run?.row_failure_groups
          || run?.rowFailureGroups
          || counts.row_failure_groups
          || counts.rowFailureGroups,
        ),
        aclSnapshotCount: numberOrZero(counts.acl_snapshot_count ?? counts.aclSnapshotCount),
        enqueuedTaskCount: numberOrZero(counts.enqueued_task_count ?? counts.enqueuedTaskCount),
        tableCounts: normalizeDatabaseSyncTableCounts(counts),
        checkpointSummary,
        workflowStage: String(checkpoint.workflow_stage || checkpoint.workflowStage || ''),
        workflowStatus: String(checkpoint.workflow_status || checkpoint.workflowStatus || ''),
        createdAt: run?.created_at || run?.createdAt || null,
        updatedAt: run?.updated_at || run?.updatedAt || null,
      };
    })
    : [];
  const syncReadiness = normalizeDatabaseSyncReadiness(status?.sync_readiness || status?.syncReadiness || {});
  return {
    loaded: Boolean(raw?.status || raw?.dataset_readiness || raw?.table_readiness),
    configValid: status?.config_valid !== false,
    configError: String(status?.config_error || ''),
    dataset: {
      datasetId: String(dataset.dataset_id || dataset.datasetId || ''),
      key: String(dataset.key || ''),
      title: String(dataset.title || ''),
      lifecycle: String(dataset.lifecycle || ''),
      datasetExternalId: String(dataset.dataset_external_id || dataset.datasetExternalId || ''),
      isDefault: dataset?.is_default === true || dataset?.isDefault === true,
      updatedAt: dataset.updated_at || dataset.updatedAt || null,
    },
    datasets,
    datasetReadiness: normalizeDatabaseReadiness(status?.dataset_readiness || {}),
    syncReadiness,
    semanticProfile: normalizeDatabaseSemanticProfile(status?.semantic_profile || status?.semanticProfile || {}),
    healthFindings: normalizeDatabaseHealthFindings(status?.health_findings || status?.healthFindings || {}),
    tableReadiness,
    recentSyncRuns,
    generatedAt: status?.generated_at || status?.generatedAt || null,
  };
}

function normalizeDatabaseHealthFindings(raw = {}) {
  const items = Array.isArray(raw?.items)
    ? raw.items.map((item) => ({
      severity: String(item?.severity || 'info').toLowerCase(),
      code: String(item?.code || ''),
      title: String(item?.title || ''),
      message: String(item?.message || ''),
      table: String(item?.table || ''),
      count: numberOrZero(item?.count),
    })).filter((item) => item.code || item.title || item.message)
    : [];
  const signal = String(raw?.signal || (items.length ? 'attention' : 'ok')).toLowerCase();
  return {
    configured: Boolean(raw?.signal || items.length),
    signal,
    label: databaseSourceHealthSignalLabel(signal),
    blockingCount: numberOrZero(raw?.blocking_count ?? raw?.blockingCount ?? items.filter((item) => item.severity === 'error').length),
    warningCount: numberOrZero(raw?.warning_count ?? raw?.warningCount ?? items.filter((item) => item.severity === 'warning').length),
    infoCount: numberOrZero(raw?.info_count ?? raw?.infoCount ?? items.filter((item) => item.severity === 'info').length),
    items: items.slice(0, 8),
  };
}

export function databaseSourceHealthSignalLabel(signal) {
  switch (String(signal || '').toLowerCase()) {
    case 'ok':
      return '健康';
    case 'attention':
      return '需关注';
    case 'blocking':
      return '阻断';
    case 'in_progress':
      return '处理中';
    default:
      return '未知';
  }
}

function normalizeDatabaseSemanticProfile(raw = {}) {
  const tables = Array.isArray(raw?.tables)
    ? raw.tables.map((table) => ({
      table: String(table?.table || table?.name || ''),
      columnCount: numberOrZero(table?.column_count ?? table?.columnCount),
      approximateRowCount: numberOrZero(table?.approximate_row_count ?? table?.approximateRowCount),
      dimensionCount: numberOrZero(table?.dimension_count ?? table?.dimensionCount),
      metricCount: numberOrZero(table?.metric_count ?? table?.metricCount),
      timeDimensionCount: numberOrZero(table?.time_dimension_count ?? table?.timeDimensionCount),
      entityColumnCount: numberOrZero(table?.entity_column_count ?? table?.entityColumnCount),
      textColumnCount: numberOrZero(table?.text_column_count ?? table?.textColumnCount),
      suggestedQuestionCount: numberOrZero(table?.suggested_question_count ?? table?.suggestedQuestionCount),
      suggestedVisualizationCount: numberOrZero(table?.suggested_visualization_count ?? table?.suggestedVisualizationCount),
      mappingConfidence: numberOrZero(table?.mapping_confidence ?? table?.mappingConfidence),
    })).filter((table) => table.table)
    : [];
  return {
    configured: Boolean(raw?.table_count ?? raw?.tableCount ?? tables.length),
    kind: String(raw?.kind || ''),
    database: String(raw?.database || ''),
    tableCount: numberOrZero(raw?.table_count ?? raw?.tableCount ?? tables.length),
    columnCount: numberOrZero(raw?.column_count ?? raw?.columnCount),
    metricCount: numberOrZero(raw?.metric_count ?? raw?.metricCount),
    dimensionCount: numberOrZero(raw?.dimension_count ?? raw?.dimensionCount),
    timeDimensionCount: numberOrZero(raw?.time_dimension_count ?? raw?.timeDimensionCount),
    entityColumnCount: numberOrZero(raw?.entity_column_count ?? raw?.entityColumnCount),
    textColumnCount: numberOrZero(raw?.text_column_count ?? raw?.textColumnCount),
    reportSuggestionCount: numberOrZero(raw?.report_suggestion_count ?? raw?.reportSuggestionCount),
    tables,
  };
}

function normalizeDatabaseSyncReadiness(raw = {}) {
  const signal = String(raw?.signal || 'unknown').toLowerCase();
  return {
    configured: Boolean(raw?.signal),
    signal,
    label: databaseSourceSyncReadinessLabel(signal),
    datasetSignal: String(raw?.dataset_signal || raw?.datasetSignal || '').toLowerCase(),
    hasSyncRun: Boolean(raw?.has_sync_run ?? raw?.hasSyncRun),
    latestStatus: String(raw?.latest_status || raw?.latestStatus || ''),
    workflowStage: String(raw?.workflow_stage || raw?.workflowStage || ''),
    workflowStatus: String(raw?.workflow_status || raw?.workflowStatus || ''),
    failureKind: String(raw?.failure_kind || raw?.failureKind || ''),
    lastError: String(raw?.last_error || raw?.lastError || ''),
    failedTaskKey: String(raw?.failed_task_key || raw?.failedTaskKey || ''),
    documentCount: numberOrZero(raw?.document_count ?? raw?.documentCount),
    rowCount: numberOrZero(raw?.row_count ?? raw?.rowCount ?? raw?.document_count ?? raw?.documentCount),
    chunkCount: numberOrZero(raw?.chunk_count ?? raw?.chunkCount),
    skippedRowCount: numberOrZero(raw?.skipped_row_count ?? raw?.skippedRowCount),
    failedRowCount: numberOrZero(raw?.failed_row_count ?? raw?.failedRowCount),
    rowFailureSamples: normalizeDatabaseRowFailureSamples(raw?.row_failure_samples || raw?.rowFailureSamples),
    rowFailureGroups: normalizeDatabaseRowFailureGroups(raw?.row_failure_groups || raw?.rowFailureGroups),
    enqueuedTaskCount: numberOrZero(raw?.enqueued_task_count ?? raw?.enqueuedTaskCount),
    tableCounts: normalizeDatabaseSyncTableCountRows(raw?.table_counts || raw?.tableCounts),
    checkpointSummary: normalizeDatabaseCheckpointSummary(raw?.checkpoint_summary || raw?.checkpointSummary || {}),
    updatedAt: raw?.updated_at || raw?.updatedAt || null,
  };
}

function normalizeDatabaseCheckpointSummary(raw = {}) {
  const tableCheckpoints = Array.isArray(raw?.table_checkpoints || raw?.tableCheckpoints)
    ? (raw.table_checkpoints || raw.tableCheckpoints).map((row) => ({
      table: String(row?.table || ''),
      updatedAfterPresent: Boolean(row?.updated_after_present ?? row?.updatedAfterPresent),
      lastIdPresent: Boolean(row?.last_id_present ?? row?.lastIdPresent),
      versionAfterPresent: Boolean(row?.version_after_present ?? row?.versionAfterPresent),
    })).filter((row) => row.table)
    : [];
  const hasCheckpoint = Boolean(raw?.has_checkpoint ?? raw?.hasCheckpoint);
  const cursorPresent = Boolean(raw?.cursor_present ?? raw?.cursorPresent);
  const topLevelIncrementalPresent = Boolean(raw?.top_level_incremental_present ?? raw?.topLevelIncrementalPresent);
  const tableCount = numberOrZero(raw?.table_count ?? raw?.tableCount ?? tableCheckpoints.length);
  const label = databaseCheckpointSummaryLabel({
    hasCheckpoint,
    cursorPresent,
    topLevelIncrementalPresent,
    tableCount,
  });
  return {
    configured: hasCheckpoint || cursorPresent || topLevelIncrementalPresent || tableCount > 0,
    hasCheckpoint,
    cursorPresent,
    topLevelIncrementalPresent,
    tableCount,
    tableCheckpoints,
    label,
  };
}

function databaseCheckpointSummaryLabel(summary) {
  if (!summary.hasCheckpoint) {
    return '无检查点';
  }
  const parts = [];
  if (summary.tableCount > 0) {
    parts.push(`表检查点 ${summary.tableCount}`);
  }
  if (summary.topLevelIncrementalPresent) {
    parts.push('增量条件');
  }
  if (summary.cursorPresent) {
    parts.push('游标已隐藏');
  }
  return parts.length ? parts.join(' · ') : '检查点已记录';
}

function normalizeDatabaseRowFailureSamples(rows = []) {
  return (Array.isArray(rows) ? rows : []).map((row) => ({
    table: String(row?.table || ''),
    rowIndex: numberOrZero(row?.row_index ?? row?.rowIndex),
    reason: String(row?.reason || ''),
    sourcePrimaryKey: String(row?.source_primary_key || row?.sourcePrimaryKey || ''),
  })).filter((row) => row.table).slice(0, 8);
}

function normalizeDatabaseRowFailureGroups(rows = []) {
  return (Array.isArray(rows) ? rows : []).map((row) => ({
    table: String(row?.table || ''),
    reason: String(row?.reason || 'row_conversion_failed'),
    sampleCount: numberOrZero(row?.sample_count ?? row?.sampleCount),
    reportedFailedRowCount: numberOrZero(row?.reported_failed_row_count ?? row?.reportedFailedRowCount),
    firstRowIndex: numberOrZero(row?.first_row_index ?? row?.firstRowIndex),
    sampleSourcePrimaryKeys: Array.isArray(row?.sample_source_primary_keys || row?.sampleSourcePrimaryKeys)
      ? (row.sample_source_primary_keys || row.sampleSourcePrimaryKeys)
        .map((value) => String(value || '').trim())
        .filter(Boolean)
        .slice(0, 3)
      : [],
  })).filter((row) => row.table).slice(0, 8);
}

function normalizeDatabaseSyncTableCounts(counts = {}) {
  const candidates = [
    counts.ingest_table_counts,
    counts.ingestTableCounts,
    counts.content_table_counts,
    counts.contentTableCounts,
    counts.metadata_table_counts,
    counts.metadataTableCounts,
  ];
  const rows = candidates.find((candidate) => Array.isArray(candidate)) || [];
  return normalizeDatabaseSyncTableCountRows(rows);
}

function normalizeDatabaseSyncTableCountRows(rows = []) {
  return (Array.isArray(rows) ? rows : []).map((row) => ({
    table: String(row?.table || ''),
    documentCount: numberOrZero(
      row?.documents_ingested
      ?? row?.documentsIngested
      ?? row?.content_document_count
      ?? row?.contentDocumentCount
      ?? row?.metadata_document_count
      ?? row?.metadataDocumentCount
      ?? row?.document_count
      ?? row?.documentCount,
    ),
    rowCount: numberOrZero(
      row?.row_count
      ?? row?.rowCount
      ?? row?.documents_ingested
      ?? row?.documentsIngested
      ?? row?.content_document_count
      ?? row?.contentDocumentCount
      ?? row?.metadata_document_count
      ?? row?.metadataDocumentCount
      ?? row?.document_count
      ?? row?.documentCount,
    ),
    chunkCount: numberOrZero(
      row?.chunks_ingested
      ?? row?.chunksIngested
      ?? row?.chunk_count
      ?? row?.chunkCount,
    ),
    skippedRowCount: numberOrZero(row?.skipped_row_count ?? row?.skippedRowCount),
    failedRowCount: numberOrZero(row?.failed_row_count ?? row?.failedRowCount),
  })).filter((row) => row.table);
}

function normalizeDatabaseReadiness(raw = {}) {
  const signal = String(raw?.signal || 'unknown').toLowerCase();
  return {
    configured: Boolean(raw?.signal),
    signal,
    label: databaseSourceReadinessLabel(signal),
    defaultDatasetId: String(raw?.default_dataset_id || raw?.defaultDatasetId || ''),
    documentCount: numberOrZero(raw?.document_count ?? raw?.documentCount),
    indexedDocumentCount: numberOrZero(raw?.indexed_document_count ?? raw?.indexedDocumentCount),
    failedDocumentCount: numberOrZero(raw?.failed_document_count ?? raw?.failedDocumentCount),
    processingDocumentCount: numberOrZero(raw?.processing_document_count ?? raw?.processingDocumentCount),
    chunkCount: numberOrZero(raw?.chunk_count ?? raw?.chunkCount),
    indexedChunkCount: numberOrZero(raw?.indexed_chunk_count ?? raw?.indexedChunkCount),
    latestDocumentUpdatedAt: raw?.latest_document_updated_at || raw?.latestDocumentUpdatedAt || null,
  };
}

export function databaseSourceReadinessLabel(signal) {
  switch (String(signal || '').toLowerCase()) {
    case 'ready':
      return '可问';
    case 'partial_ready':
      return '部分可问';
    case 'processing':
      return '处理中';
    case 'failed':
      return '失败';
    case 'no_documents':
      return '未入库';
    default:
      return '未知';
  }
}

export function databaseSourceSyncReadinessLabel(signal) {
  switch (String(signal || '').toLowerCase()) {
    case 'ready':
      return '可问';
    case 'partial_ready':
      return '部分可问';
    case 'sync_running':
      return '同步中';
    case 'sync_queued':
      return '排队中';
    case 'sync_failed':
      return '同步失败';
    case 'indexing':
      return '索引中';
    case 'index_failed':
      return '索引失败';
    case 'synced_no_documents':
      return '已同步无文档';
    case 'no_sync':
      return '未同步';
    case 'no_documents':
      return '未入库';
    default:
      return '未知';
  }
}

export function databaseSourceTablePreview(integration = {}, limit = 8) {
  const source = databaseSourceSummary(integration);
  const max = Math.max(0, Math.floor(Number(limit) || 0));
  const tables = source.tables.slice(0, max);
  return {
    tables,
    hiddenCount: Math.max(0, source.tableCount - tables.length),
  };
}

export function databaseSourceSyncRuns(auditItems = [], limit = 3) {
  const max = Math.max(0, Math.floor(Number(limit) || 0));
  return (Array.isArray(auditItems) ? auditItems : [])
    .filter((item) => String(item?.itemType || item?.item_type || '').toLowerCase() === 'sync')
    .slice(0, max)
    .map((item) => {
      const summary = item.summary && typeof item.summary === 'object' ? item.summary : {};
      const counts = summary.counts && typeof summary.counts === 'object' ? summary.counts : {};
      return {
        status: String(item.status || summary.status || 'unknown'),
        syncKind: String(summary.sync_kind || ''),
        updatedAt: item.createdAt || item.created_at || null,
        failureKind: item.failureKind || item.failure_kind || '',
        lastError: String(counts.last_error || counts.lastError || ''),
        failedTaskKey: String(counts.failed_task_key || counts.failedTaskKey || ''),
        documentCount: numberOrZero(
          counts.documents_ingested
          ?? counts.documentsIngested
          ?? counts.content_document_count
          ?? counts.contentDocumentCount
          ?? counts.metadata_document_count
          ?? counts.metadataDocumentCount
          ?? counts.document_count,
        ),
        rowCount: numberOrZero(
          counts.row_count
          ?? counts.rowCount
          ?? counts.content_row_count
          ?? counts.contentRowCount
          ?? counts.metadata_row_count
          ?? counts.metadataRowCount
          ?? counts.documents_ingested
          ?? counts.documentsIngested
          ?? counts.content_document_count
          ?? counts.contentDocumentCount
          ?? counts.metadata_document_count
          ?? counts.metadataDocumentCount
          ?? counts.document_count
          ?? counts.documentCount,
        ),
        skippedRowCount: numberOrZero(counts.skipped_row_count ?? counts.skippedRowCount),
        failedRowCount: numberOrZero(counts.failed_row_count ?? counts.failedRowCount),
        rowFailureSamples: normalizeDatabaseRowFailureSamples(counts.row_failure_samples || counts.rowFailureSamples),
        aclSnapshotCount: numberOrZero(counts.acl_snapshot_count),
        enqueuedTaskCount: numberOrZero(counts.enqueued_task_count),
        tableCounts: normalizeDatabaseSyncTableCounts(counts),
        checkpointSummary: normalizeDatabaseCheckpointSummary(summary.checkpoint_summary || summary.checkpointSummary || {}),
      };
    });
}

export function externalConversationStatusLabel(status) {
  switch (String(status || '').toLowerCase()) {
    case 'completed':
      return '已回复';
    case 'running':
      return '处理中';
    case 'failed':
      return '失败';
    case 'rejected':
      return '已拒绝';
    case 'unavailable':
      return '无可用回复';
    case 'no_run':
      return '未建运行';
    default:
      return '未知';
  }
}

export function formatExternalConversationDuration(value) {
  const ms = normalizeConversationDuration(value);
  if (ms === null) {
    return '未完成';
  }
  if (ms < 1000) {
    return `${ms}ms`;
  }
  if (ms < 60_000) {
    return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)}s`;
  }
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.round((ms % 60_000) / 1000);
  return seconds ? `${minutes}m ${seconds}s` : `${minutes}m`;
}

export function normalizeWorkflowStatusKey(status) {
  return String(status || '')
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[\s-]+/g, '_')
    .toLowerCase();
}

export function workflowStatusLabel(status) {
  const normalized = normalizeWorkflowStatusKey(status);
  const labels = {
    pending: '等待',
    running: '运行中',
    queued: '队列中',
    claimed: '执行中',
    succeeded: '成功',
    failed: '失败',
    cancelled: '取消',
    dead_lettered: '死信',
  };
  return labels[normalized] || status || '未知';
}

export function workflowStatusClass(status) {
  const normalized = normalizeWorkflowStatusKey(status);
  return `external-conversation-status external-executor-status-${normalized || 'unknown'}`;
}

export function normalizeWorkflowTask(item = {}) {
  const payload = item.payload && typeof item.payload === 'object' ? item.payload : {};
  const cloudflare = payload.cloudflare_orchestrator && typeof payload.cloudflare_orchestrator === 'object'
    ? payload.cloudflare_orchestrator
    : {};
  return {
    id: item.id || '',
    status: item.status || '',
    queue: item.queue || '',
    taskKey: item.task_key || item.taskKey || '',
    attempt: Number(item.attempt || 0),
    maxAttempts: Number(item.max_attempts || item.maxAttempts || 0),
    availableAt: item.available_at || item.availableAt || null,
    claimedAt: item.claimed_at || item.claimedAt || null,
    finishedAt: item.finished_at || item.finishedAt || null,
    updatedAt: item.updated_at || item.updatedAt || item.available_at || null,
    error: item.error || '',
    cloudflareTaskId: item.cloudflareTaskId || cloudflare.task_id || cloudflare.taskId || '',
    cloudflareStatus: item.cloudflareStatus || cloudflare.status || '',
    cloudflareRuntimeTargetId: item.cloudflareRuntimeTargetId || cloudflare.runtime_target_id || cloudflare.runtimeTargetId || '',
  };
}

export function normalizeArtifactManifest(item = {}) {
  const manifest = item && typeof item === 'object' ? item : {};
  const links = Array.isArray(manifest.links)
    ? manifest.links
      .map((link) => ({
        rel: link?.rel || '',
        url: link?.url || '',
      }))
      .filter((link) => link.url)
    : [];
  const refs = manifest.refs && typeof manifest.refs === 'object' ? manifest.refs : {};
  const safety = manifest.safety && typeof manifest.safety === 'object' ? manifest.safety : {};
  return {
    schema: manifest.schema || '',
    schemaVersion: Number(manifest.schema_version || manifest.schemaVersion || 0),
    artifactType: manifest.artifact_type || manifest.artifactType || '',
    artifactKind: manifest.artifact_kind || manifest.artifactKind || '',
    primaryUrl: manifest.primary_url || manifest.primaryUrl || '',
    links,
    refs,
    safety,
  };
}

export function normalizeArtifactManifests(value = []) {
  const items = Array.isArray(value) ? value : [];
  return items
    .map(normalizeArtifactManifest)
    .filter((manifest) => (
      manifest.schema === 'v3.output_artifact_manifest'
      && manifest.schemaVersion === 1
      && (manifest.artifactType || manifest.artifactKind)
    ));
}

export function normalizeCodexExecutorTask(item = {}) {
  return {
    id: item.id || '',
    kind: item.kind || '',
    status: item.status || '',
    stage: item.stage || '',
    updatedAt: item.updated_at || item.updatedAt || null,
  };
}

function latestWorkflowTask(tasks) {
  return [...tasks].sort((left, right) => {
    const leftTime = Date.parse(left.updatedAt || left.availableAt || 0) || 0;
    const rightTime = Date.parse(right.updatedAt || right.availableAt || 0) || 0;
    return rightTime - leftTime;
  })[0] || null;
}

function taskLooksLikePollRetry(task) {
  const error = String(task?.error || '');
  return normalizeWorkflowStatusKey(task?.status) === 'queued'
    && error.includes('Cloudflare Codex task timed out after');
}

export function codexExecutorInspectSummary(detail = {}) {
  const execution = detail.execution || {};
  const runtime = detail.execution_scope_runtime || detail.executionScopeRuntime || {};
  const prettySummaries = Array.isArray(detail.pretty_summaries)
    ? detail.pretty_summaries
    : Array.isArray(detail.prettySummaries)
      ? detail.prettySummaries
      : [];
  const artifactManifests = normalizeArtifactManifests(
    Array.isArray(detail.artifact_manifests)
      ? detail.artifact_manifests
      : detail.artifactManifests,
  );
  const workflowTasks = (Array.isArray(detail.workflow_tasks)
    ? detail.workflow_tasks
    : Array.isArray(detail.workflowTasks)
      ? detail.workflowTasks
      : []
  ).map(normalizeWorkflowTask);
  const latestTask = latestWorkflowTask(workflowTasks);
  const pollRetryTask = workflowTasks.find(taskLooksLikePollRetry) || null;
  return {
    execution: {
      id: execution.id || '',
      kind: execution.kind || '',
      status: execution.status || '',
      stage: execution.stage || '',
      updated_at: execution.updated_at || execution.updatedAt || null,
    },
    runtime: {
      status: runtime.status || '',
      latest_finish_reason: runtime.latest_finish_reason || runtime.latestFinishReason || '',
      invocation_count: runtime.invocation_count || runtime.invocationCount || 0,
      tool_execution_count: runtime.tool_execution_count || runtime.toolExecutionCount || 0,
    },
    workflow_tasks: workflowTasks,
    latest_task: latestTask,
    poll_retry: pollRetryTask
      ? {
        active: true,
        task_id: pollRetryTask.id,
        cloudflare_task_id: pollRetryTask.cloudflareTaskId,
        attempt: pollRetryTask.attempt,
        max_attempts: pollRetryTask.maxAttempts,
        next_available_at: pollRetryTask.availableAt,
        error: pollRetryTask.error,
      }
      : { active: false },
    pretty_summaries: prettySummaries,
    artifact_manifests: artifactManifests,
    model_facing: detail.model_facing || detail.modelFacing || null,
  };
}

export function auditItemTypeLabel(itemType) {
  switch (String(itemType || '').toLowerCase()) {
    case 'message':
      return '消息';
    case 'action':
      return '动作';
    case 'search_evidence':
      return '搜索证据';
    case 'sync':
      return '同步';
    default:
      return '事件';
  }
}

export function normalizeControlResult(raw = {}) {
  return {
    accepted: Boolean(raw.accepted),
    integrationId: String(raw.integration_id || ''),
    integrationKind: String(raw.integration_kind || 'unknown'),
    action: String(raw.action || 'control'),
    status: String(raw.status || 'unknown'),
    message: String(raw.message || ''),
    affectedActionCount: numberOrZero(raw.affected_action_count),
    syncRunId: raw.sync_run_id || null,
    enqueuedTaskCount: Array.isArray(raw.enqueued_tasks) ? raw.enqueued_tasks.length : 0,
  };
}

export function controlResultLabel(result) {
  if (!result?.accepted) {
    return '操作未受理';
  }
  if (result.action === 'retry' && result.syncRunId) {
    return `同步已入队：${result.syncRunId}`;
  }
  if (result.action === 'retry') {
    return result.affectedActionCount > 0
      ? `已入队 ${result.affectedActionCount} 个动作重试`
      : '暂无可重试动作';
  }
  if (result.action === 'disable') {
    return '集成已停用';
  }
  if (result.action === 'rotate_secret') {
    return '密钥轮换请求已记录';
  }
  return result.message || '操作已受理';
}

export function integrationSignal(integration = {}) {
  if (String(integration.status || '').toLowerCase() === 'disabled') {
    return 'disabled';
  }
  if (integration.actionSignal === 'result_failed') {
    return 'failed';
  }
  if (numberOrZero(integration.failed) > 0 || String(integration.healthStatus || '').toLowerCase() === 'failed') {
    return 'failed';
  }
  if (numberOrZero(integration.blocked) > 0) {
    return 'blocked';
  }
  if (numberOrZero(integration.pending) > 0) {
    return 'pending';
  }
  if (numberOrZero(integration.dispatched) > 0 || String(integration.healthStatus || '').toLowerCase() === 'healthy') {
    return 'healthy';
  }
  return 'unknown';
}

export function driftSignal(driftSummary = {}) {
  return String(driftSummary.signal || 'unknown').toLowerCase();
}

export function driftSignalLabel(signal) {
  switch (signal) {
    case 'ok':
      return '治理正常';
    case 'identity_mapping_gap':
      return '身份待映射';
    case 'disabled_principals':
      return '用户停用';
    case 'acl_missing':
      return 'ACL 缺失';
    case 'acl_stale':
      return 'ACL 过期';
    case 'sync_failed':
      return '同步失败';
    case 'sync_recovering':
      return '恢复中';
    default:
      return '未知';
  }
}

export function artifactSignal(artifactSummary = {}) {
  return String(artifactSummary.signal || 'none').toLowerCase();
}

export function searchEvidenceSignal(searchSummary = {}) {
  return String(searchSummary.signal || 'none').toLowerCase();
}

export function searchEvidenceSignalLabel(signal) {
  switch (signal) {
    case 'search_evidence_required':
      return '搜索待供料';
    case 'none':
      return '无需搜索';
    default:
      return '未知';
  }
}

export function artifactSignalLabel(signal) {
  switch (signal) {
    case 'artifact_confirmation_pending':
      return '撤销待确认';
    case 'artifact_failed':
      return '产物失败';
    case 'artifact_blocked':
      return '产物阻断';
    case 'artifact_revoked':
      return '已撤销';
    case 'artifact_published':
      return '已发布';
    case 'artifact_observed':
      return '有产物动作';
    case 'none':
      return '暂无产物';
    default:
      return '未知';
  }
}

export function actionSignal(actionSummary = {}) {
  return String(actionSummary.signal || 'none').toLowerCase();
}

export function actionSignalLabel(signal) {
  switch (signal) {
    case 'result_failed':
      return '结果失败';
    case 'dispatch_blocked':
      return '派发阻断';
    case 'dispatch_failed':
      return '派发失败';
    case 'confirmation_pending':
      return '确认待处理';
    case 'waiting_result':
      return '等待结果';
    case 'result_running':
      return '结果处理中';
    case 'result_succeeded':
      return '结果成功';
    case 'action_observed':
      return '有动作记录';
    case 'none':
      return '暂无动作';
    default:
      return '未知';
  }
}

export function numberOrZero(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
}

function conversationDisplayText(value) {
  const text = String(value || '').trim();
  if (!text || text === '[redacted]') {
    return '';
  }
  return text;
}

function normalizeConversationDuration(value) {
  if (value === null || value === undefined || value === '') {
    return null;
  }
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? Math.round(number) : null;
}

export function formatObservationTime(value) {
  if (!value) {
    return '无记录';
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return '无记录';
  }
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(date).replace(/\//g, '-');
}

export function latestIntegrationActivity(integration) {
  return integration.lastFailureAt
    || integration.latestActionAt
    || integration.searchSummary?.latest_required_at
    || integration.artifactSummary?.latest_artifact_action_at
    || integration.lastSuccessAt
    || integration.lastEventAt
    || integration.lastSyncAt
    || integration.disabledAt
    || null;
}

export function signalLabel(signal) {
  switch (signal) {
    case 'healthy':
      return '正常';
    case 'pending':
      return '待确认';
    case 'blocked':
      return '已阻断';
    case 'failed':
      return '失败';
    case 'disabled':
      return '停用';
    default:
      return '未知';
  }
}
