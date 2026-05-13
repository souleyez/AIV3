export const DEFAULT_THIRD_PARTY_API_BASE_URL = 'https://v3.elepcloud.com';

export function thirdPartyApiBaseUrl() {
  const configured = process.env.NEXT_PUBLIC_THIRD_PARTY_API_BASE_URL || DEFAULT_THIRD_PARTY_API_BASE_URL;
  return configured.endsWith('/') ? configured.slice(0, -1) : configured;
}

export function buildThirdPartyApiUrl(pathname) {
  const path = pathname.startsWith('/') ? pathname : `/${pathname}`;
  return `${thirdPartyApiBaseUrl()}${path}`;
}

export function normalizeIntegrationSummary(raw = {}) {
  const pending = numberOrZero(raw.pending_action_count);
  const blocked = numberOrZero(raw.blocked_action_count);
  const failed = numberOrZero(raw.failed_action_count);
  const dispatched = numberOrZero(raw.dispatched_action_count);
  const driftSummary = raw.drift_summary && typeof raw.drift_summary === 'object' ? raw.drift_summary : {};
  const drift = driftSignal(driftSummary);
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
    configSummary: raw.config_summary && typeof raw.config_summary === 'object' ? raw.config_summary : {},
    driftSummary,
    driftSignal: drift,
    signal: integrationSignal({ pending, blocked, failed, dispatched, healthStatus: raw.health_status, status: raw.status }),
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
