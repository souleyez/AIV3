export const DEFAULT_THIRD_PARTY_API_BASE_URL = 'https://v3.elepcloud.com';

export const EXTERNAL_AUDIT_FILTERS = [
  { key: 'all', label: '全部', params: {} },
  { key: 'actions', label: '动作', params: { itemType: 'action' } },
  { key: 'search', label: '搜索证据', params: { itemType: 'search_evidence' } },
  { key: 'outbound_replies', label: '回复回推', params: { itemType: 'outbound_reply' } },
  { key: 'callbacks', label: '结果回调', params: { itemType: 'action', actionState: 'result_callback' } },
  { key: 'waiting', label: '待结果', params: { itemType: 'action', actionState: 'waiting_result' } },
  { key: 'failed', label: '失败/阻断', params: { itemType: 'action', actionState: 'failed' } },
];

export const EXTERNAL_INTEGRATION_MODES = [
  {
    key: 'standard_bot',
    title: '标准机器人模式',
    status: '可选',
    summary: '飞书、Lark、企业微信等平台事件进入 DataMax 适配器，再转为统一外部通道事件。',
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
      '统一消息事件写入 DataMax 通道',
      '文档源、文档解析和解析详情查询',
      '默认提示词、输出格式和本轮 skill 策略',
      '模板文档解析和模板生成产物',
      '用户确认、动作派发和结果回调',
      '审计列表、失败阻断和 trace 导出',
    ],
    guardrail: '按平台官方验签；不要把平台 token 当作 DataMax 出站派发凭证。',
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
  {
    key: 'database_integration',
    title: '数据库对接',
    status: '联调接入',
    summary: '适合类似 8 服务器现有数据库源的接入：DataMax 托管密钥和表映射，数据库行同步入数据集后用于问答和静态页报表。',
    docs: [
      'docs/integrations/third-party-integration-api.zh-CN.html#section-26',
      'docs/integrations/third-party-integration-api.zh-CN.md',
    ],
    documentLinks: [
      {
        key: 'database-complete-third-party-html',
        label: '完整 API（含数据库）',
        href: '/external-integrations/third-party-integration-api.zh-CN.html#section-26',
        kind: 'html',
        target: '_blank',
      },
      {
        key: 'database-complete-third-party-md',
        label: '完整 API MD 下载',
        href: '/external-integrations/third-party-integration-api.zh-CN.md',
        kind: 'markdown',
        download: true,
      },
    ],
    checklist: [
      '数据库源配置：source_id、密钥引用、库名和表白名单',
      '联调验证：连接测试、schema 扫描、表预览',
      '语义画像：识别时间、指标、维度、实体和文本字段',
      '同步入库：数据库行清洗为 DataMax 数据集证据',
      '状态查询：同步状态、数据集可用性、行转换失败',
      '聊天使用：传同步后的 dataset_external_ids',
      '报表生成：基于数据库数据生成 Image2 和静态页',
      '安全边界：不传密码、不执行任意 SQL、不暴露原始表 dump',
    ],
    guardrail: '数据库密码只放服务端密钥或密钥绑定；第三方聊天接口只传数据集范围，不传 SQL 和连接串。',
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
  const readOnlyStatus = databaseSourceReadOnlyStatus(integration, normalized);
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
    read_only_status: readOnlyStatus,
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
  const configSummary = raw.config_summary && typeof raw.config_summary === 'object' ? raw.config_summary : {};
  const documentDiagnostics = normalizeDocumentProcessingDiagnostics([
    ...arrayFromAnyKey(raw, ['document_diagnostics', 'documentDiagnostics']),
    ...arrayFromAnyKey(configSummary, ['document_diagnostics', 'documentDiagnostics']),
    ...arrayFromAnyKey(driftSummary, ['document_diagnostics', 'documentDiagnostics']),
  ]);
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
    configSummary,
    inboundAuth: inboundAuthSummary(configSummary),
    searchSummary,
    searchSignal: search,
    searchEvidenceRequiredCount: numberOrZero(searchSummary.required_count),
    driftSummary,
    driftSignal: drift,
    artifactSummary,
    artifactSignal: artifact,
    documentDiagnostics,
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

export function inboundAuthSummary(configSummary = {}) {
  const temporaryAccess = configSummary.temporary_access && typeof configSummary.temporary_access === 'object'
    ? configSummary.temporary_access
    : configSummary.temporaryAccess && typeof configSummary.temporaryAccess === 'object'
      ? configSummary.temporaryAccess
      : {};
  return {
    configured: Boolean(configSummary.inbound_auth_configured ?? configSummary.inboundAuthConfigured),
    mode: String(configSummary.inbound_auth_mode || configSummary.inboundAuthMode || 'none'),
    temporary: Boolean(temporaryAccess.temporary),
    expired: Boolean(temporaryAccess.expired),
    expiresAt: temporaryAccess.expires_at || temporaryAccess.expiresAt || null,
    tokenRotatedAt: temporaryAccess.token_rotated_at || temporaryAccess.tokenRotatedAt || null,
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

export function normalizeExternalConversationTimeline(raw = {}) {
  return {
    eventId: String(raw.event_id || ''),
    integrationId: String(raw.integration_id || ''),
    integrationDisplayName: String(raw.integration_display_name || raw.integration_id || '未命名集成'),
    platform: String(raw.platform || 'unknown'),
    conversationExternalId: String(raw.conversation_external_id || ''),
    messageExternalId: String(raw.message_external_id || ''),
    direction: String(raw.direction || 'inbound'),
    assistantRunId: raw.assistant_run_id ? String(raw.assistant_run_id) : '',
    events: Array.isArray(raw.events)
      ? raw.events.map(normalizeExternalConversationTimelineEvent)
      : [],
  };
}

export function normalizeExternalConversationTimelineEvent(raw = {}) {
  const artifactLinks = Array.isArray(raw.artifact_links)
    ? raw.artifact_links.map((link) => String(link || '').trim()).filter(Boolean)
    : [];
  return {
    sequenceNo: Number.isFinite(Number(raw.sequence_no)) ? Number(raw.sequence_no) : 0,
    eventName: String(raw.event_name || ''),
    phase: conversationDisplayText(raw.phase),
    status: conversationDisplayText(raw.status),
    displayText: conversationDisplayText(raw.display_text),
    artifactLinks,
    payloadSummary: raw.payload_summary && typeof raw.payload_summary === 'object'
      ? raw.payload_summary
      : {},
    debugPayload: raw.debug_payload && typeof raw.debug_payload === 'object'
      ? raw.debug_payload
      : null,
    createdAt: raw.created_at || null,
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
  const datasetExternalIds = uniqueStringArray([
    ...arrayValue(raw.dataset_external_ids || raw.datasetExternalIds),
    raw.dataset_external_id,
    raw.datasetExternalId,
    raw.default_dataset_external_id,
    raw.defaultDatasetExternalId,
    ...arrayValue(configSummary.dataset_external_ids || configSummary.datasetExternalIds),
    configSummary.dataset_external_id,
    configSummary.datasetExternalId,
  ]);
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
      sourceId: String(integration.id || integration.source_id || ''),
      systemUserId: '',
      tenantExternalId: '',
      botExternalId: '',
      datasetExternalIds: [],
      dataSourceExists: false,
      databaseExists: false,
      readOnly: false,
      latestAnalysisStatus: '',
      latestSyncStatus: '',
      recentError: '',
      error: '',
    };
  }
  const productionWriteAllowed = raw.production_write_allowed ?? raw.productionWriteAllowed;
  const readOnlyValue = raw.read_only ?? raw.readOnly ?? raw.read_only_attached ?? raw.readOnlyAttached;
  return {
    configured: true,
    valid: raw.valid !== false,
    kind: String(raw.kind || 'database'),
    database: String(raw.database || ''),
    connectionEnv: String(raw.connection_env || raw.connectionEnv || ''),
    defaultDatasetId: String(raw.default_dataset_id || raw.defaultDatasetId || ''),
    tableCount: numberOrZero(raw.table_count ?? raw.tableCount) || tables.length,
    tables,
    sourceId: String(raw.source_id || raw.sourceId || integration.id || integration.source_id || ''),
    systemUserId: String(raw.system_user_id || raw.systemUserId || raw.owner_user_id || raw.ownerUserId || ''),
    tenantExternalId: String(raw.tenant_external_id || raw.tenantExternalId || configSummary.tenant_external_id || configSummary.tenantExternalId || ''),
    botExternalId: String(raw.bot_external_id || raw.botExternalId || configSummary.bot_external_id || configSummary.botExternalId || ''),
    datasetExternalIds,
    dataSourceExists: Boolean(raw.data_source_exists ?? raw.dataSourceExists ?? raw.source_exists ?? raw.sourceExists ?? true),
    databaseExists: Boolean(raw.database_exists ?? raw.databaseExists ?? raw.database),
    readOnly: readOnlyValue === undefined ? productionWriteAllowed !== true : Boolean(readOnlyValue),
    latestAnalysisStatus: String(raw.latest_analysis_status || raw.latestAnalysisStatus || ''),
    latestSyncStatus: String(raw.latest_sync_status || raw.latestSyncStatus || ''),
    recentError: redactDatabaseStatusText(raw.recent_error || raw.recentError || raw.last_error || raw.lastError || raw.error || ''),
    error: redactDatabaseStatusText(raw.error || ''),
  };
}

function arrayValue(value) {
  if (Array.isArray(value)) {
    return value;
  }
  return value ? [value] : [];
}

function redactDatabaseStatusText(value, maxLength = 180) {
  let text = String(value || '').trim();
  if (!text) {
    return '';
  }
  text = text
    .replace(/https?:\/\/[^\s<>"']+/gi, '[redacted:url]')
    .replace(/\/(?:Users|Volumes|srv|tmp|var|private|home)\/[^\s<>"']+/g, '[redacted:path]')
    .replace(/\b[A-Za-z]:\\[^\s<>"']+/g, '[redacted:path]')
    .replace(/\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/gi, 'Bearer [redacted:token]')
    .replace(/\bv3in_[A-Za-z0-9._-]+/g, '[redacted:token]')
    .replace(/\b(cookie|token|secret|password|api[_-]?key|connection[_-]?url|database[_-]?url)\s*[:=]\s*[^\s,;]+/gi, '$1=[redacted]');
  return text.length > maxLength ? `${text.slice(0, maxLength - 1)}…` : text;
}

export function outboundReplyDispatchSummary(integration = {}) {
  const configSummary = integration?.configSummary && typeof integration.configSummary === 'object'
    ? integration.configSummary
    : integration?.config_summary && typeof integration.config_summary === 'object'
      ? integration.config_summary
      : {};
  const raw = configSummary.outbound_reply_dispatch && typeof configSummary.outbound_reply_dispatch === 'object'
    ? configSummary.outbound_reply_dispatch
    : configSummary.outboundReplyDispatch && typeof configSummary.outboundReplyDispatch === 'object'
      ? configSummary.outboundReplyDispatch
      : {};
  const endpointConfigured = Boolean(
    raw.endpoint_configured
      ?? raw.endpointConfigured
      ?? configSummary.reply_dispatch_endpoint_configured
      ?? configSummary.replyDispatchEndpointConfigured,
  );
  const authMode = String(raw.auth_mode || raw.authMode || configSummary.reply_dispatch_auth_mode || 'none');
  const authConfigured = Boolean(
    raw.auth_configured
      ?? raw.authConfigured
      ?? (authMode && authMode !== 'none'),
  );
  const ready = Boolean(raw.ready ?? raw.configured ?? (endpointConfigured && authConfigured));
  const authSource = String(raw.auth_source || raw.authSource || 'none');
  const endpointHost = String(raw.endpoint_host || raw.endpointHost || '');
  const actionAuthFallbackAvailable = Boolean(
    raw.action_auth_fallback_available
      ?? raw.actionAuthFallbackAvailable
      ?? false,
  );
  const signal = ready
    ? 'ready'
    : endpointConfigured
      ? 'missing_auth'
      : 'missing_endpoint';
  return {
    configured: endpointConfigured || authConfigured,
    ready,
    signal,
    endpointConfigured,
    endpointHost,
    authConfigured,
    authMode,
    authSource,
    actionAuthFallbackAvailable,
  };
}

export function outboundReplyDispatchSignalLabel(signal) {
  switch (String(signal || '').toLowerCase()) {
    case 'ready':
      return '可主动回推';
    case 'missing_auth':
      return '待配置鉴权';
    case 'missing_endpoint':
      return '待第三方回推地址';
    default:
      return '未配置';
  }
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
  const readOnly = databaseSourceReadOnlyStatus(integration);
  const thirdPartyScope = [
    source.tenantExternalId ? `tenant ${source.tenantExternalId}` : '',
    source.botExternalId ? `bot ${source.botExternalId}` : '',
  ].filter(Boolean).join(' · ');
  return [
    { label: '只读状态', value: readOnly.label },
    { label: '问答就绪', value: readiness.label },
    { label: '系统用户', value: source.systemUserId || '未配置' },
    { label: '第三方范围', value: thirdPartyScope || '未配置' },
    { label: '稳定分组', value: source.datasetExternalIds.length ? source.datasetExternalIds.join(', ') : '未配置' },
    { label: '数据库', value: source.database || '未配置' },
    { label: '连接引用', value: source.connectionEnv || '未配置' },
    { label: '默认数据集', value: readiness.defaultDatasetId || source.defaultDatasetId || '未绑定' },
    { label: '表数量', value: source.tableCount },
    { label: '最近同步', value: source.latestSyncStatus || driftSummary.latest_sync_status || '无记录' },
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

export function databaseSourceReadOnlyStatus(integration = {}, status = null) {
  const source = databaseSourceSummary(integration);
  const normalized = status?.loaded
    ? status
    : status
      ? normalizeDatabaseSourceStatus(status)
      : null;
  const driftSummary = integration?.driftSummary && typeof integration.driftSummary === 'object'
    ? integration.driftSummary
    : integration?.drift_summary && typeof integration.drift_summary === 'object'
      ? integration.drift_summary
      : {};
  const datasetExternalIds = uniqueStringArray([
    ...source.datasetExternalIds,
    normalized?.dataset?.datasetExternalId,
    ...(normalized?.datasets || []).map((dataset) => dataset.datasetExternalId),
  ]);
  const datasetReadiness = normalized?.datasetReadiness?.configured
    ? normalized.datasetReadiness
    : databaseSourceReadiness(integration);
  const syncReadiness = normalized?.syncReadiness || {};
  const healthFindings = normalized?.healthFindings || {};
  const latestSyncStatus = syncReadiness.latestStatus || source.latestSyncStatus || driftSummary.latest_sync_status || '';
  const latestAnalysisStatus = source.latestAnalysisStatus || driftSummary.latest_analysis_status || '';
  const recentError = redactDatabaseStatusText(
    source.recentError
    || normalized?.configError
    || syncReadiness.lastError
    || syncReadiness.failureKind
    || healthFindings.items?.find((item) => item.severity === 'error' || item.severity === 'warning')?.message
    || '',
  );
  const syncFailure = ['sync_failed', 'index_failed'].includes(syncReadiness.signal)
    || Boolean(syncReadiness.lastError || syncReadiness.failureKind || normalized?.configError);
  let signal = 'attached';
  if (!source.configured) {
    signal = 'not_attached';
  } else if (
    !source.valid
    || normalized?.configValid === false
    || healthFindings.signal === 'blocking'
    || syncFailure
  ) {
    signal = 'operator_required';
  } else if (
    ['sync_running', 'sync_queued', 'indexing'].includes(syncReadiness.signal)
    || /queued|running|processing|in_progress/i.test(latestAnalysisStatus)
  ) {
    signal = 'analyzing';
  } else if (['ready', 'partial_ready'].includes(datasetReadiness.signal) && source.readOnly) {
    signal = 'read_only_ready';
  } else if (
    ['no_sync', 'synced_no_documents', 'no_documents'].includes(syncReadiness.signal)
    || (!latestSyncStatus && source.configured)
  ) {
    signal = 'not_synced';
  } else if (!source.readOnly) {
    signal = 'operator_required';
  }
  return {
    configured: source.configured,
    signal,
    label: databaseSourceReadOnlyStatusLabel(signal),
    sourceId: source.sourceId,
    systemUserId: source.systemUserId,
    tenantExternalId: source.tenantExternalId,
    botExternalId: source.botExternalId,
    datasetExternalIds,
    dataSourceExists: source.dataSourceExists,
    databaseExists: source.databaseExists,
    readOnlyAttached: source.configured && source.readOnly,
    latestSyncStatus,
    latestAnalysisStatus,
    recentError,
  };
}

export function databaseSourceReadOnlyStatusLabel(signal) {
  switch (String(signal || '').toLowerCase()) {
    case 'read_only_ready':
      return '只读可用';
    case 'attached':
      return '已挂接';
    case 'not_synced':
      return '未同步';
    case 'analyzing':
      return '分析中';
    case 'operator_required':
      return '需要 operator 处理';
    case 'not_attached':
      return '未挂接';
    default:
      return '未知';
  }
}

export function normalizeDatabaseSourceStatus(raw = {}) {
  const status = raw?.status && typeof raw.status === 'object' ? raw.status : raw;
  const dataset = status?.dataset && typeof status.dataset === 'object' ? status.dataset : {};
  const datasets = Array.isArray(status?.datasets)
    ? status.datasets.map((item) => {
      const readiness = normalizeDatabaseReadiness(item?.readiness || item || {});
      return {
        datasetId: String(item?.dataset_id || item?.datasetId || ''),
        key: String(item?.key || ''),
        title: String(item?.title || ''),
        lifecycle: String(item?.lifecycle || ''),
        datasetExternalId: String(item?.dataset_external_id || item?.datasetExternalId || ''),
        isDefault: item?.is_default === true || item?.isDefault === true,
        updatedAt: item?.updated_at || item?.updatedAt || null,
        readiness,
        documentCount: readiness.documentCount,
        indexedDocumentCount: readiness.indexedDocumentCount,
        failedDocumentCount: readiness.failedDocumentCount,
        processingDocumentCount: readiness.processingDocumentCount,
        chunkCount: readiness.chunkCount,
        indexedChunkCount: readiness.indexedChunkCount,
        latestDocumentUpdatedAt: readiness.latestDocumentUpdatedAt,
        retrievalEvidenceCount: numberOrZero(item?.retrieval_evidence_count ?? item?.retrievalEvidenceCount),
      };
    }).filter((item) => item.datasetId)
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
    sourceId: String(raw?.source_id || raw?.sourceId || status?.source_id || status?.sourceId || ''),
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

export function formatWorkflowDuration(value) {
  const ms = normalizeConversationDuration(value);
  if (ms === null) {
    return '无完成样本';
  }
  return formatExternalConversationDuration(ms);
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
  const imageOrchestrator = payload.static_page_image_orchestrator && typeof payload.static_page_image_orchestrator === 'object'
    ? payload.static_page_image_orchestrator
    : {};
  const safeError = redactDatabaseStatusText(
    item.error
      || item.error_message
      || item.errorMessage
      || item.failure_reason
      || item.failureReason
      || item.reason
      || '',
    240,
  );
  return {
    id: item.id || '',
    status: item.status || '',
    queue: item.queue || '',
    taskKey: item.task_key || item.taskKey || '',
    logicalQueue: item.logical_queue || item.logicalQueue || payload.logical_queue || '',
    logicalTaskKey: item.logical_task_key || item.logicalTaskKey || payload.logical_task_key || '',
    attempt: Number(item.attempt || 0),
    maxAttempts: Number(item.max_attempts || item.maxAttempts || 0),
    availableAt: item.available_at || item.availableAt || null,
    nextPollAt: item.next_poll_at || item.nextPollAt || cloudflare.next_poll_at || imageOrchestrator.next_poll_at || null,
    claimedAt: item.claimed_at || item.claimedAt || null,
    finishedAt: item.finished_at || item.finishedAt || null,
    updatedAt: item.updated_at || item.updatedAt || item.available_at || null,
    error: safeError,
    failureReason: safeError,
    cloudflareTaskId: item.remote_task_id || item.remoteTaskId || item.cloudflareTaskId || cloudflare.task_id || cloudflare.taskId || imageOrchestrator.task_id || imageOrchestrator.taskId || '',
    cloudflareStatus: item.cloudflareStatus || cloudflare.status || '',
    cloudflareRuntimeTargetId: item.cloudflareRuntimeTargetId || cloudflare.runtime_target_id || cloudflare.runtimeTargetId || '',
  };
}

function normalizeWorkflowQueueSummary(item = {}) {
  const taskKeys = Array.isArray(item.task_keys)
    ? item.task_keys
    : Array.isArray(item.taskKeys)
      ? item.taskKeys
      : [];
  return {
    logicalQueue: item.logical_queue || item.logicalQueue || '',
    physicalQueues: Array.isArray(item.physical_queues)
      ? item.physical_queues
      : Array.isArray(item.physicalQueues)
        ? item.physicalQueues
        : [],
    taskCount: numberOrZero(item.task_count ?? item.taskCount),
    queued: numberOrZero(item.queued),
    running: numberOrZero(item.running),
    retrying: numberOrZero(item.retrying),
    succeeded: numberOrZero(item.succeeded),
    failed: numberOrZero(item.failed),
    cancelled: numberOrZero(item.cancelled),
    deadLettered: numberOrZero(item.dead_lettered ?? item.deadLettered),
    nextAvailableAt: item.next_available_at || item.nextAvailableAt || null,
    finishedDurationP50Ms: normalizeConversationDuration(
      item.finished_duration_p50_ms ?? item.finishedDurationP50Ms,
    ),
    finishedDurationP95Ms: normalizeConversationDuration(
      item.finished_duration_p95_ms ?? item.finishedDurationP95Ms,
    ),
    succeededDurationP50Ms: normalizeConversationDuration(
      item.succeeded_duration_p50_ms ?? item.succeededDurationP50Ms,
    ),
    succeededDurationP95Ms: normalizeConversationDuration(
      item.succeeded_duration_p95_ms ?? item.succeededDurationP95Ms,
    ),
    taskKeys: taskKeys.map((taskKey) => ({
      logicalTaskKey: taskKey.logical_task_key || taskKey.logicalTaskKey || '',
      physicalTaskKeys: Array.isArray(taskKey.physical_task_keys)
        ? taskKey.physical_task_keys
        : Array.isArray(taskKey.physicalTaskKeys)
          ? taskKey.physicalTaskKeys
          : [],
      taskCount: numberOrZero(taskKey.task_count ?? taskKey.taskCount),
      queued: numberOrZero(taskKey.queued),
      running: numberOrZero(taskKey.running),
      retrying: numberOrZero(taskKey.retrying),
      succeeded: numberOrZero(taskKey.succeeded),
      failed: numberOrZero(taskKey.failed),
      cancelled: numberOrZero(taskKey.cancelled),
      deadLettered: numberOrZero(taskKey.dead_lettered ?? taskKey.deadLettered),
      nextAvailableAt: taskKey.next_available_at || taskKey.nextAvailableAt || null,
      finishedDurationP50Ms: normalizeConversationDuration(
        taskKey.finished_duration_p50_ms ?? taskKey.finishedDurationP50Ms,
      ),
      finishedDurationP95Ms: normalizeConversationDuration(
        taskKey.finished_duration_p95_ms ?? taskKey.finishedDurationP95Ms,
      ),
      succeededDurationP50Ms: normalizeConversationDuration(
        taskKey.succeeded_duration_p50_ms ?? taskKey.succeededDurationP50Ms,
      ),
      succeededDurationP95Ms: normalizeConversationDuration(
        taskKey.succeeded_duration_p95_ms ?? taskKey.succeededDurationP95Ms,
      ),
    })),
  };
}

export function normalizeWorkflowQueueStats(raw = {}) {
  const queues = Array.isArray(raw.queues) ? raw.queues : [];
  return {
    generatedAt: raw.generated_at || raw.generatedAt || null,
    executionCount: numberOrZero(raw.execution_count ?? raw.executionCount),
    taskCount: numberOrZero(raw.task_count ?? raw.taskCount),
    queues: queues.map(normalizeWorkflowQueueSummary),
  };
}

function valueFromAnyKey(source = {}, keys = []) {
  if (!source || typeof source !== 'object') {
    return undefined;
  }
  for (const key of keys) {
    if (
      Object.prototype.hasOwnProperty.call(source, key)
      && source[key] !== undefined
      && source[key] !== null
    ) {
      return source[key];
    }
  }
  return undefined;
}

function objectFromAnyKey(source = {}, keys = []) {
  const value = valueFromAnyKey(source, keys);
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function arrayFromAnyKey(source = {}, keys = []) {
  const value = valueFromAnyKey(source, keys);
  return Array.isArray(value) ? value : [];
}

function stringFromAnyKey(source = {}, keys = []) {
  const value = valueFromAnyKey(source, keys);
  if (value === null || value === undefined) {
    return '';
  }
  return String(value).trim();
}

function normalizeDiagnosticStatus(value, fallback = 'unknown') {
  const normalized = String(value || '')
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[\s-]+/g, '_')
    .toLowerCase()
    .trim();
  return normalized || fallback;
}

function uniqueStringArray(value = []) {
  const rawItems = Array.isArray(value) ? value : [value];
  const seen = new Set();
  const items = [];
  for (const item of rawItems) {
    const normalized = String(item || '').trim();
    if (!normalized || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    items.push(normalized);
  }
  return items;
}

function redactDocumentDiagnosticText(value, maxLength = 180) {
  let text = String(value || '').trim();
  if (!text) {
    return '';
  }
  text = text
    .replace(/https?:\/\/[^\s<>"']+/gi, '[redacted:url]')
    .replace(/\/(?:Users|Volumes|srv|tmp|var|private|home)\/[^\s<>"']+/g, '[redacted:path]')
    .replace(/\b[A-Za-z]:\\[^\s<>"']+/g, '[redacted:path]')
    .replace(/\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/gi, 'Bearer [redacted:token]')
    .replace(/\bv3in_[A-Za-z0-9._-]+/g, '[redacted:token]')
    .replace(/\b(cookie|token|secret|password|api[_-]?key)\s*[:=]\s*[^\s,;]+/gi, '$1=[redacted]');
  return text.length > maxLength ? `${text.slice(0, maxLength - 1)}…` : text;
}

function normalizeStatusCountMap(value = {}) {
  const source = value && typeof value === 'object' && !Array.isArray(value) ? value : {};
  return Object.fromEntries(
    Object.entries(source)
      .map(([key, count]) => [normalizeDiagnosticStatus(key), numberOrZero(count)])
      .filter(([key, count]) => key && count > 0)
      .slice(0, 20),
  );
}

function enrichmentCountsFromRuns(runs = []) {
  return runs.reduce((counts, run) => {
    const status = normalizeDiagnosticStatus(run?.status);
    counts[status] = numberOrZero(counts[status]) + 1;
    return counts;
  }, {});
}

function normalizeDocumentLatestTask(raw = {}) {
  const latestTask = raw && typeof raw === 'object' && !Array.isArray(raw) ? raw : null;
  if (!latestTask || !Object.keys(latestTask).length) {
    return null;
  }
  return normalizeWorkflowTask({
    id: latestTask.id || latestTask.task_id || latestTask.taskId || '',
    ...latestTask,
    task_key: latestTask.task_key || latestTask.taskKey || '',
    logical_queue: latestTask.logical_queue || latestTask.logicalQueue || '',
    logical_task_key: latestTask.logical_task_key || latestTask.logicalTaskKey || '',
    max_attempts: latestTask.max_attempts || latestTask.maxAttempts || 0,
    available_at: latestTask.available_at || latestTask.availableAt || null,
    claimed_at: latestTask.claimed_at || latestTask.claimedAt || null,
    finished_at: latestTask.finished_at || latestTask.finishedAt || null,
    updated_at: latestTask.updated_at || latestTask.updatedAt || latestTask.available_at || null,
    error: redactDocumentDiagnosticText(latestTask.error || latestTask.error_message || latestTask.errorMessage || ''),
  });
}

function deriveEnrichmentStatus({ dedupState = 'unknown', counts = {}, explicit = '' } = {}) {
  if (explicit) {
    return normalizeDiagnosticStatus(explicit);
  }
  if (dedupState === 'duplicate') {
    return 'duplicate';
  }
  if (numberOrZero(counts.failed) || numberOrZero(counts.dead_lettered)) {
    return 'blocked';
  }
  if (numberOrZero(counts.running) || numberOrZero(counts.claimed)) {
    return 'running';
  }
  if (numberOrZero(counts.pending) || numberOrZero(counts.queued)) {
    return 'waiting';
  }
  if (numberOrZero(counts.succeeded)) {
    return 'succeeded';
  }
  return 'not_started';
}

function documentDiagnosticTone(diagnostic = {}) {
  const statuses = [
    diagnostic.parseStatus,
    diagnostic.indexStatus,
    diagnostic.enrichmentStatus,
    normalizeWorkflowStatusKey(diagnostic.latestTask?.status),
  ];
  if (
    diagnostic.blockedReason
    || diagnostic.failureSummary
    || statuses.some((status) => ['failed', 'blocked', 'dead_lettered', 'cancelled'].includes(status))
  ) {
    return 'critical';
  }
  if (statuses.some((status) => ['pending', 'queued', 'waiting', 'running', 'claimed', 'processing'].includes(status))) {
    return 'warning';
  }
  if (diagnostic.dedupState === 'duplicate') {
    return 'neutral';
  }
  if (diagnostic.indexStatus === 'indexed' && ['succeeded', 'duplicate'].includes(diagnostic.enrichmentStatus)) {
    return 'healthy';
  }
  return 'neutral';
}

function dedupStateLabel(state) {
  switch (state) {
    case 'canonical':
      return 'Canonical';
    case 'duplicate':
      return '重复归档';
    default:
      return '去重未知';
  }
}

function dedupStateTone(state) {
  switch (state) {
    case 'canonical':
      return 'healthy';
    case 'duplicate':
      return 'neutral';
    default:
      return 'warning';
  }
}

export function documentProcessingStatusLabel(status) {
  switch (normalizeDiagnosticStatus(status)) {
    case 'canonical':
      return 'Canonical';
    case 'duplicate':
      return '重复';
    case 'indexed':
      return '已索引';
    case 'extracted':
    case 'parsed':
    case 'succeeded':
      return '已完成';
    case 'partial_ready':
    case 'enriched_partial':
      return '部分完成';
    case 'received':
    case 'pending':
    case 'queued':
    case 'waiting':
      return '等待中';
    case 'running':
    case 'claimed':
    case 'processing':
      return '处理中';
    case 'not_started':
      return '未开始';
    case 'blocked':
      return '已阻断';
    case 'failed':
    case 'dead_lettered':
      return '失败';
    case 'parse_or_index_pending':
      return '解析/索引等待';
    case 'enrichment_waiting':
      return '深化等待';
    case 'duplicate_uses_canonical':
      return '使用 canonical';
    case 'parse_failed':
      return '解析失败';
    case 'enrichment_failed':
      return '深化失败';
    case 'unknown':
      return '未知';
    default:
      return status || '未知';
  }
}

export function normalizeDocumentProcessingDiagnostic(raw = {}) {
  const source = raw && typeof raw === 'object' && !Array.isArray(raw) ? raw : {};
  const document = objectFromAnyKey(source, ['document']);
  const parseState = objectFromAnyKey(source, ['parse_state', 'parseState']);
  const workflow = objectFromAnyKey(source, ['workflow']);
  const runs = [
    ...arrayFromAnyKey(source, ['enrichment_runs', 'enrichmentRuns']),
    ...arrayFromAnyKey(source, ['document_enrichment_runs', 'documentEnrichmentRuns']),
  ];
  const runCounts = enrichmentCountsFromRuns(runs);
  const explicitCounts = normalizeStatusCountMap(
    valueFromAnyKey(source, ['enrichment_counts', 'enrichmentCounts', 'enrichment_status_counts', 'enrichmentStatusCounts']),
  );
  const enrichmentCounts = Object.keys(explicitCounts).length ? explicitCounts : runCounts;
  const dedupState = normalizeDiagnosticStatus(
    stringFromAnyKey(source, ['dedup_state', 'dedupState'])
      || stringFromAnyKey(document, ['dedup_state', 'dedupState']),
  );
  const latestTask = normalizeDocumentLatestTask(
    valueFromAnyKey(source, ['latest_task', 'latestTask'])
      || valueFromAnyKey(workflow, ['latest_task', 'latestTask'])
      || {},
  );
  const parseStatus = normalizeDiagnosticStatus(
    stringFromAnyKey(source, ['parse_status', 'parseStatus'])
      || stringFromAnyKey(parseState, ['parse_status', 'parseStatus', 'model_status', 'modelStatus'])
      || stringFromAnyKey(document, ['parse_status', 'parseStatus'])
      || stringFromAnyKey(source, ['lifecycle'])
      || stringFromAnyKey(document, ['lifecycle']),
  );
  const indexStatus = normalizeDiagnosticStatus(
    stringFromAnyKey(source, ['index_status', 'indexStatus'])
      || stringFromAnyKey(source, ['retrieval_status', 'retrievalStatus'])
      || (numberOrZero(parseState.retrieval_evidence_count ?? parseState.retrievalEvidenceCount) > 0 ? 'indexed' : ''),
  );
  const enrichmentStatus = deriveEnrichmentStatus({
    dedupState,
    counts: enrichmentCounts,
    explicit: stringFromAnyKey(source, ['enrichment_status', 'enrichmentStatus']),
  });
  const diagnostic = {
    documentId: stringFromAnyKey(source, ['document_id', 'documentId'])
      || stringFromAnyKey(document, ['id', 'document_id', 'documentId']),
    externalId: stringFromAnyKey(source, ['external_id', 'externalId', 'document_external_id', 'documentExternalId'])
      || stringFromAnyKey(document, ['external_id', 'externalId', 'document_external_id', 'documentExternalId']),
    datasetIds: uniqueStringArray(
      valueFromAnyKey(source, ['dataset_ids', 'datasetIds'])
      || valueFromAnyKey(document, ['dataset_ids', 'datasetIds'])
      || stringFromAnyKey(source, ['dataset_id', 'datasetId'])
      || stringFromAnyKey(document, ['dataset_id', 'datasetId']),
    ),
    canonicalDocumentId: stringFromAnyKey(source, ['canonical_document_id', 'canonicalDocumentId'])
      || stringFromAnyKey(document, ['canonical_document_id', 'canonicalDocumentId']),
    dedupState,
    dedupLabel: dedupStateLabel(dedupState),
    dedupTone: dedupStateTone(dedupState),
    parseStatus,
    parseLabel: documentProcessingStatusLabel(parseStatus),
    indexStatus,
    indexLabel: documentProcessingStatusLabel(indexStatus),
    enrichmentStatus,
    enrichmentLabel: documentProcessingStatusLabel(enrichmentStatus),
    latestTask,
    latestTaskLabel: latestTask
      ? `${workflowTaskKeyLabel(latestTask.logicalTaskKey || latestTask.taskKey)} · ${workflowStatusLabel(latestTask.status)}`
      : '',
    failureSummary: redactDocumentDiagnosticText(
      stringFromAnyKey(source, ['failure_summary', 'failureSummary', 'last_error', 'lastError', 'error_message', 'errorMessage'])
      || latestTask?.error
      || '',
    ),
    blockedReason: redactDocumentDiagnosticText(stringFromAnyKey(source, ['blocked_reason', 'blockedReason'])),
    waitingReason: redactDocumentDiagnosticText(stringFromAnyKey(source, ['waiting_reason', 'waitingReason'])),
    enrichmentCounts,
    updatedAt: stringFromAnyKey(source, ['updated_at', 'updatedAt'])
      || stringFromAnyKey(document, ['updated_at', 'updatedAt'])
      || latestTask?.updatedAt
      || null,
    redaction: {
      rawContentIncluded: false,
      rawDocumentPathIncluded: false,
      rawProviderPayloadIncluded: false,
    },
  };
  diagnostic.tone = documentDiagnosticTone(diagnostic);
  return diagnostic;
}

export function normalizeDocumentProcessingDiagnostics(value = []) {
  return (Array.isArray(value) ? value : [])
    .map(normalizeDocumentProcessingDiagnostic)
    .filter((item) => (
      item.documentId
      || item.externalId
      || item.canonicalDocumentId
      || item.parseStatus !== 'unknown'
      || item.enrichmentStatus !== 'not_started'
    ));
}

function queueTotal(queue, keys = ['queued', 'running', 'retrying']) {
  return keys.reduce((sum, key) => sum + numberOrZero(queue?.[key]), 0);
}

function queueMatches(queue = {}, matchers = []) {
  const logicalQueue = String(queue.logicalQueue || '').toLowerCase();
  const physicalQueues = Array.isArray(queue.physicalQueues)
    ? queue.physicalQueues.map((item) => String(item || '').toLowerCase())
    : [];
  const taskKeys = Array.isArray(queue.taskKeys)
    ? queue.taskKeys.flatMap((taskKey) => [
      taskKey.logicalTaskKey,
      ...(Array.isArray(taskKey.physicalTaskKeys) ? taskKey.physicalTaskKeys : []),
    ]).map((item) => String(item || '').toLowerCase())
    : [];
  const haystack = [logicalQueue, ...physicalQueues, ...taskKeys].join(' ');
  return matchers.some((matcher) => haystack.includes(String(matcher || '').toLowerCase()));
}

function summarizeQueueGroup(stats = {}, matchers = []) {
  const queues = Array.isArray(stats?.queues) ? stats.queues : [];
  const matched = queues.filter((queue) => queueMatches(queue, matchers));
  const active = matched.reduce((sum, queue) => sum + queueTotal(queue), 0);
  const failed = matched.reduce((sum, queue) => sum + queueTotal(queue, ['failed', 'deadLettered']), 0);
  const taskCount = matched.reduce((sum, queue) => sum + numberOrZero(queue.taskCount), 0);
  const p95Values = matched
    .map((queue) => queue.succeededDurationP95Ms ?? queue.finishedDurationP95Ms)
    .filter((value) => value !== null && value !== undefined);
  return {
    queueCount: matched.length,
    taskCount,
    queued: matched.reduce((sum, queue) => sum + numberOrZero(queue.queued), 0),
    running: matched.reduce((sum, queue) => sum + numberOrZero(queue.running), 0),
    retrying: matched.reduce((sum, queue) => sum + numberOrZero(queue.retrying), 0),
    active,
    failed,
    p95Ms: p95Values.length ? Math.max(...p95Values) : null,
  };
}

function operationsTone({ active = 0, failed = 0, warning = false, ready = true } = {}) {
  if (!ready || failed > 0) return 'critical';
  if (warning || active > 0) return 'warning';
  return 'healthy';
}

function modelLaneSummary(modelGatewayStatus = null) {
  const lanes = Array.isArray(modelGatewayStatus?.lanes) ? modelGatewayStatus.lanes : [];
  const providers = Array.isArray(modelGatewayStatus?.providers) ? modelGatewayStatus.providers : [];
  const lane = lanes.find((item) => item.lane === 'assistant_chat') || lanes[0] || null;
  if (!lane) {
    return {
      loaded: false,
      value: '待读取',
      detail: '模型池状态需 operator 会话',
      active: 0,
      queued: 0,
      maxConcurrency: null,
      activeProfileCount: 0,
      failureCount: 0,
    };
  }
  const laneName = String(lane.lane || '').trim();
  const routingMode = String(lane.routingMode || lane.routing_mode || 'routing');
  const active = numberOrZero(lane.activeCount ?? lane.active_count ?? lane.active);
  const queued = numberOrZero(lane.queuedCount ?? lane.queued_count ?? lane.queued);
  const maxConcurrency = lane.maxConcurrency ?? lane.max_concurrency ?? null;
  const activeProfileCount = numberOrZero(lane.activeProfileCount ?? lane.active_profile_count);
  const profileCount = numberOrZero(lane.profileCount ?? lane.profile_count);
  const laneProviders = providers.filter((provider) => String(provider.lane || '') === laneName);
  const failureCount = laneProviders.reduce((sum, provider) => (
    sum
    + numberOrZero(provider.runtimeFailureCount ?? provider.runtime_failure_count)
    + numberOrZero(provider.runtimeTimeoutCount ?? provider.runtime_timeout_count)
    + numberOrZero(provider.runtimeRateLimitCount ?? provider.runtime_rate_limit_count)
  ), 0);
  return {
    loaded: true,
    value: `${active}/${maxConcurrency ?? '-'}`,
    detail: `${routingMode} · active profiles ${activeProfileCount}/${profileCount}`,
    active,
    queued,
    maxConcurrency,
    activeProfileCount,
    failureCount,
  };
}

function card(key, label, value, detail, tone = 'neutral', meta = {}) {
  return {
    key,
    label,
    value,
    detail,
    tone,
    ...meta,
  };
}

export function buildOperationsSummary({
  integrations = [],
  workflowQueueStats = null,
  modelGatewayStatus = null,
  codexExecutorTasks = [],
  documentDiagnostics = [],
} = {}) {
  const normalizedIntegrations = Array.isArray(integrations) ? integrations : [];
  const normalizedDocumentDiagnostics = normalizeDocumentProcessingDiagnostics([
    ...(Array.isArray(documentDiagnostics) ? documentDiagnostics : []),
    ...normalizedIntegrations.flatMap((item) => (
      Array.isArray(item.documentDiagnostics) ? item.documentDiagnostics : []
    )),
  ]);
  const stats = workflowQueueStats?.queues
    ? workflowQueueStats
    : normalizeWorkflowQueueStats(workflowQueueStats || {});
  const queueStatsLoaded = Boolean(workflowQueueStats?.queues || workflowQueueStats?.generatedAt || workflowQueueStats?.generated_at);
  const channels = normalizedIntegrations.filter((item) => item.kind === 'channel');
  const sources = normalizedIntegrations.filter((item) => item.kind === 'source');
  const databaseSources = normalizedIntegrations.filter((item) => databaseSourceSummary(item).configured);
  const readyDatabaseSources = databaseSources.filter((item) => {
    const readiness = databaseSourceReadiness(item);
    return readiness.signal === 'ready' || readiness.signal === 'partial_ready';
  });
  const databaseFailures = normalizedIntegrations.reduce((sum, item) => (
    sum + numberOrZero(item.driftSummary?.failed_sync_count)
  ), 0);
  const actionFailures = normalizedIntegrations.reduce((sum, item) => (
    sum + numberOrZero(item.blockedActionCount) + numberOrZero(item.failedActionCount)
  ), 0);
  const artifactIssues = normalizedIntegrations.filter((item) => (
    ['artifact_confirmation_pending', 'artifact_blocked', 'artifact_failed'].includes(item.artifactSignal)
  )).length;
  const waitingResults = normalizedIntegrations.reduce((sum, item) => (
    sum + numberOrZero(item.actionSummary?.waiting_result_count)
  ), 0);
  const searchRequired = normalizedIntegrations.reduce((sum, item) => (
    sum + numberOrZero(item.searchSummary?.required_count)
  ), 0);
  const staticReportQueue = summarizeQueueGroup(stats, [
    'static_page',
    'report',
    'product_image_generation',
  ]);
  const enrichmentQueue = summarizeQueueGroup(stats, [
    'document_enrichment',
    'fact_index',
    'enrichment',
  ]);
  const fixedTaskQueue = summarizeQueueGroup(stats, [
    'codex_fixed_task',
    'answer_quality',
  ]);
  const lowQualityTaskCount = (Array.isArray(codexExecutorTasks) ? codexExecutorTasks : [])
    .filter((task) => {
      const text = `${task.kind || ''} ${task.stage || ''}`.toLowerCase();
      return text.includes('answer_quality') || text.includes('autofix');
    }).length;
  const model = modelLaneSummary(modelGatewayStatus);
  const externalChannelRuntime = modelGatewayStatus?.runtime?.externalChannel
    || modelGatewayStatus?.runtime?.external_channel
    || {};
  const externalActiveConversations = numberOrZero(
    externalChannelRuntime.activeConversations
    ?? externalChannelRuntime.active_conversations,
  );
  const activeWorkflowBacklog = stats.queues.reduce((sum, queue) => sum + queueTotal(queue), 0);
  const workflowFailures = stats.queues.reduce((sum, queue) => sum + queueTotal(queue, ['failed', 'deadLettered']), 0);

  return {
    generatedAt: new Date().toISOString(),
    queueStatsLoaded,
    modelStatusLoaded: model.loaded,
    cards: [
      card(
        'ordinary_chat',
        '普通对话',
        `${channels.length}`,
        `通道 ${channels.length} · 当前并发 ${externalActiveConversations || '待读取'} · 动作异常 ${actionFailures}`,
        operationsTone({ failed: actionFailures }),
      ),
      card(
        'model_lane',
        '模型通道',
        model.value,
        model.detail,
        operationsTone({
          ready: model.loaded && model.activeProfileCount > 0,
          active: model.queued,
          failed: model.failureCount,
          warning: model.maxConcurrency !== null && model.active >= model.maxConcurrency,
        }),
      ),
      card(
        'workflow_backlog',
        '工作流积压',
        queueStatsLoaded ? `${activeWorkflowBacklog}` : '待读取',
        queueStatsLoaded
          ? `队列 ${stats.queues.length} · 失败 ${workflowFailures}`
          : '只读取汇总队列，不读取任务详情',
        queueStatsLoaded ? operationsTone({ active: activeWorkflowBacklog, failed: workflowFailures }) : 'neutral',
      ),
      card(
        'static_report',
        '报表/静态页',
        queueStatsLoaded ? `${staticReportQueue.active}` : '待读取',
        queueStatsLoaded
          ? `运行 ${staticReportQueue.running} · 排队 ${staticReportQueue.queued} · P95 ${formatWorkflowDuration(staticReportQueue.p95Ms)}`
          : '打开或刷新运营状态后显示',
        queueStatsLoaded ? operationsTone({ active: staticReportQueue.active, failed: staticReportQueue.failed }) : 'neutral',
      ),
      card(
        'template_reuse',
        '模板/产物',
        `${artifactIssues}`,
        `产物异常 ${artifactIssues} · 待结果 ${waitingResults} · 网页证据 ${searchRequired}`,
        operationsTone({ active: waitingResults + searchRequired, failed: artifactIssues }),
      ),
      card(
        'data_ingestion',
        '数据接入',
        `${readyDatabaseSources.length}/${databaseSources.length}`,
        `数据库源 ${databaseSources.length} · 同步失败 ${databaseFailures} · 文档源 ${sources.length}`,
        operationsTone({ failed: databaseFailures, warning: databaseSources.length > readyDatabaseSources.length }),
      ),
      card(
        'document_enrichment',
        '文档深化',
        queueStatsLoaded ? `${enrichmentQueue.active}` : '待读取',
        queueStatsLoaded
          ? `队列 ${enrichmentQueue.queueCount} · 失败 ${enrichmentQueue.failed} · 文档诊断 ${normalizedDocumentDiagnostics.length}`
          : '后台 enrichment 仅显示汇总',
        queueStatsLoaded ? operationsTone({ active: enrichmentQueue.active, failed: enrichmentQueue.failed }) : 'neutral',
      ),
      card(
        'low_quality',
        '低质量恢复',
        queueStatsLoaded ? `${Math.max(fixedTaskQueue.active, lowQualityTaskCount)}` : '待读取',
        queueStatsLoaded
          ? `固定任务积压 ${fixedTaskQueue.active} · 已加载低质任务 ${lowQualityTaskCount} · 不拦截正常回复`
          : '仅统计固定任务/低质量队列',
        queueStatsLoaded ? operationsTone({ active: fixedTaskQueue.active + lowQualityTaskCount, failed: fixedTaskQueue.failed }) : 'neutral',
      ),
    ],
    documentDiagnostics: normalizedDocumentDiagnostics,
  };
}

export function workflowQueueLabel(value) {
  const labels = {
    static_page_image_preview: '可视化预览',
    static_page_publish: '页面发布',
    product_image_generation: '商品图',
    codex_fixed_task: '通用 Codex',
    static_page_render: '静态页渲染',
    document_enrichment: '文档深化',
    report: '报表',
    report_plan: '报表规划',
    report_render: '报表渲染',
    external_source: '数据接入',
    ingest: '文档入库',
  };
  return labels[value] || value || '未分组';
}

export function workflowTaskKeyLabel(value) {
  const labels = {
    submit_static_page_image_preview: '提交可视化',
    poll_static_page_image_preview: '轮询可视化',
    submit_static_page_publish: '提交页面发布',
    poll_static_page_publish: '轮询页面发布',
    submit_codex_fixed_task: '提交通用任务',
    poll_codex_fixed_task: '轮询通用任务',
    render_static_page: '渲染静态页',
  };
  return labels[value] || value || '未分组';
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
  const primaryUrl = manifest.primary_url || manifest.primaryUrl || '';
  const artifactLinks = normalizeArtifactManifestLinks(primaryUrl, links);
  return {
    schema: manifest.schema || '',
    schemaVersion: Number(manifest.schema_version || manifest.schemaVersion || 0),
    artifactType: manifest.artifact_type || manifest.artifactType || '',
    artifactKind: manifest.artifact_kind || manifest.artifactKind || '',
    primaryUrl,
    links,
    artifactLinks,
    refs,
    safety,
  };
}

function normalizeArtifactManifestLinks(primaryUrl = '', links = []) {
  const seen = new Set();
  const items = [];
  const add = (rel, url) => {
    const normalizedUrl = String(url || '').trim();
    if (!normalizedUrl || seen.has(normalizedUrl)) return;
    seen.add(normalizedUrl);
    items.push({
      rel: String(rel || '').trim() || 'link',
      url: normalizedUrl,
    });
  };
  add('primary', primaryUrl);
  links.forEach((link) => add(link.rel, link.url));
  return items;
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
  const status = normalizeWorkflowStatusKey(task?.status);
  return normalizeWorkflowStatusKey(task?.status) === 'queued'
    && (
      task?.nextPollAt
      || task?.cloudflareTaskId
      || task?.logicalTaskKey?.startsWith('poll_')
      || error.includes('Cloudflare Codex task timed out after')
      || error.includes('Cloudflare Codex task still running')
      || error.includes('Cloudflare Codex task submitted and pending')
      || task?.cloudflareStatus === 'cloudflare_orchestrator_pending'
      || task?.cloudflareStatus === 'cloudflare_orchestrator_submitted'
    )
    && status === 'queued';
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
        next_available_at: pollRetryTask.nextPollAt || pollRetryTask.availableAt,
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
    case 'outbound_reply':
      return '回复回推';
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
    inboundBearerToken: raw.inbound_bearer_token ? String(raw.inbound_bearer_token) : '',
    tokenExpiresAt: raw.token_expires_at || null,
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
  if (result.action === 'enable') {
    return '集成已启用';
  }
  if (result.action === 'create_channel') {
    return '第三方通道已创建，token 仅本次展示';
  }
  if (result.action === 'rotate_token') {
    return '入站 token 已轮换，token 仅本次展示';
  }
  if (result.action === 'rotate_secret') {
    return '密钥轮换请求已记录';
  }
  if (result.action === 'configure_reply_dispatch') {
    return result.status === 'reply_dispatch_cleared'
      ? '助手消息回推配置已清空'
      : '助手消息回推配置已保存';
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
