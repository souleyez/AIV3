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
      '按模板生成报表：document_template_skill',
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
  const signal = String(raw.signal || 'unknown').toLowerCase();
  return {
    configured: Boolean(raw.signal),
    signal,
    label: databaseSourceReadinessLabel(signal),
    defaultDatasetId: String(raw.default_dataset_id || raw.defaultDatasetId || ''),
    documentCount: numberOrZero(raw.document_count ?? raw.documentCount),
    indexedDocumentCount: numberOrZero(raw.indexed_document_count ?? raw.indexedDocumentCount),
    failedDocumentCount: numberOrZero(raw.failed_document_count ?? raw.failedDocumentCount),
    processingDocumentCount: numberOrZero(raw.processing_document_count ?? raw.processingDocumentCount),
    chunkCount: numberOrZero(raw.chunk_count ?? raw.chunkCount),
    indexedChunkCount: numberOrZero(raw.indexed_chunk_count ?? raw.indexedChunkCount),
    latestDocumentUpdatedAt: raw.latest_document_updated_at || raw.latestDocumentUpdatedAt || null,
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
        documentCount: numberOrZero(counts.document_count),
        aclSnapshotCount: numberOrZero(counts.acl_snapshot_count),
        enqueuedTaskCount: numberOrZero(counts.enqueued_task_count),
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
