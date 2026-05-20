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
      'docs/integrations/third-party-integration-api.md',
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
    guardrail: '按平台官方验签；不要把平台 token 当作 V3 出站派发凭证。',
  },
  {
    key: 'pure_third_party',
    title: '纯第三方简单版',
    status: '常规接入',
    summary: '第三方自建页面、资料源、用户 ID、会话 ID、skill、模板和业务系统接入 V3，由 V3 统一解析、索引、问答、产物和动作治理。',
    docs: [
      'docs/integrations/pure-third-party-integration-guide.zh-CN.html',
      'docs/integrations/pure-third-party-integration-guide.zh-CN.md',
      'docs/integrations/third-party-document-parse-first-phase.zh-CN.md',
      'docs/integrations/third-party-document-parse-first-phase-sendable.zh-CN.html',
      'docs/integrations/third-party-document-parse-first-phase-sendable.zh-CN.md',
      'docs/integrations/third-party-document-parse-request.sample.json',
      'docs/integrations/third-party-chat-with-document-ids.sample.json',
      'docs/integrations/third-party-source-parameter-card.zh-CN.md',
      'docs/integrations/third-party-source-parameter-card.sample.json',
      'docs/integrations/third-party-source-sync-request.sample.json',
      'docs/integrations/third-party-integration-api.zh-CN.md',
    ],
    documentLinks: [
      {
        key: 'pure-third-party-html',
        label: '简单版 HTML',
        href: '/external-integrations/pure-third-party-integration-guide.zh-CN.html',
        kind: 'html',
        target: '_blank',
      },
      {
        key: 'pure-third-party-md',
        label: '简单版 MD 下载',
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
    guardrail: '凭证线下交付；观测页只展示模式和文档路径，不展示 token、客户 endpoint 或密钥。',
  },
  {
    key: 'edge_local_data_plane',
    title: 'V3 Edge / Local Data Plane',
    status: '新增可选模式',
    summary: '资料、索引、页面和回答尽量留在第三方服务器，V3 只提供能力控制面和脱敏观测。',
    docs: [
      'docs/integrations/v3-edge-local-data-plane-mode.zh-CN.md',
      'docs/integrations/v3-edge-local-data-plane.sample.json',
    ],
    guardrail: '适用于客户要求资料和页面尽量留在本地的场景；浏览器不得直连 V3，也不得持有 V3 token。',
  },
  {
    key: 'handoff_package',
    title: '交接清单与校验',
    status: '按需生成',
    summary: '用于客户沙箱联调前检查 HTTPS、派发鉴权、回调、资料权限样例和脱敏要求。',
    docs: [
      'docs/integrations/third-party-handoff.sample.json',
      'tools/validate-external-handoff.mjs',
    ],
    guardrail: '清单只写交付方式和配置状态，不写明文 token、signing secret、password、private key。',
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
