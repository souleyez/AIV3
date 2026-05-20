'use client';

import { useEffect, useMemo, useState } from 'react';
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
  EXTERNAL_AUDIT_FILTERS,
  EXTERNAL_INTEGRATION_MODES,
  externalActionTraceFilename,
  formatObservationTime,
  latestIntegrationActivity,
  normalizeControlResult,
  normalizeAuditItem,
  normalizeIntegrationSummary,
  numberOrZero,
  searchEvidenceSignalLabel,
  signalLabel,
  thirdPartyApiBaseUrl,
} from '../lib/external-integrations';

const REFRESH_INTERVAL_MS = 15000;

async function fetchJson(pathname, options = {}) {
  const headers = { accept: 'application/json', ...(options.headers || {}) };
  let body = options.body;
  if (body && typeof body === 'object' && !(body instanceof FormData)) {
    headers['content-type'] = 'application/json';
    body = JSON.stringify(body);
  }
  const response = await fetch(pathname, {
    method: options.method || 'GET',
    cache: 'no-store',
    headers,
    body,
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload?.message || payload?.error || `请求失败 ${response.status}`);
  }
  return payload;
}

function metricTotal(integrations, key) {
  return integrations.reduce((sum, item) => sum + Number(item[key] || 0), 0);
}

function statusClass(signal) {
  return `external-status external-status-${signal || 'unknown'}`;
}

function driftClass(signal) {
  return `external-drift-pill external-drift-${signal || 'unknown'}`;
}

function artifactClass(signal) {
  return `external-artifact-pill external-artifact-${signal || 'none'}`;
}

function governanceMetrics(integration) {
  const summary = integration?.driftSummary || {};
  if (integration?.kind === 'source') {
    return [
      { label: '治理信号', value: driftSignalLabel(integration.driftSignal) },
      { label: 'ACL 快照', value: numberOrZero(summary.acl_snapshot_count) },
      { label: '过期快照', value: numberOrZero(summary.stale_acl_snapshot_count) },
      { label: '同步失败', value: numberOrZero(summary.failed_sync_count) },
      { label: '最近 ACL', value: formatObservationTime(summary.latest_acl_captured_at) },
      { label: '同步状态', value: summary.latest_sync_status || '无记录' },
    ];
  }
  return [
    { label: '治理信号', value: driftSignalLabel(integration?.driftSignal) },
    { label: '未映射用户', value: numberOrZero(summary.unmapped_principal_count) },
    { label: '停用用户', value: numberOrZero(summary.disabled_principal_count) },
    { label: '最近用户更新', value: formatObservationTime(summary.latest_principal_updated_at) },
  ];
}

function artifactMetrics(integration) {
  const summary = integration?.artifactSummary || {};
  return [
    { label: '产物信号', value: artifactSignalLabel(integration?.artifactSignal) },
    { label: '状态查询', value: numberOrZero(summary.status_action_count) },
    { label: '发布动作', value: numberOrZero(summary.publish_action_count) },
    { label: '撤销动作', value: numberOrZero(summary.revoke_action_count) },
    { label: '待确认', value: numberOrZero(summary.pending_confirmation_count) },
    { label: '阻断/失败', value: numberOrZero(summary.blocked_count) + numberOrZero(summary.failed_count) },
    { label: '已发布', value: numberOrZero(summary.published_count) },
    { label: '已撤销', value: numberOrZero(summary.revoked_count) },
    { label: '最近产物动作', value: formatObservationTime(summary.latest_artifact_action_at) },
  ];
}

function actionLifecycleMetrics(integration) {
  const summary = integration?.actionSummary || {};
  return [
    { label: '动作信号', value: actionSignalLabel(integration?.actionSignal) },
    { label: '总动作', value: numberOrZero(summary.total_action_count) },
    { label: '待结果', value: numberOrZero(summary.waiting_result_count) },
    { label: '已回调', value: numberOrZero(summary.result_callback_count) },
    { label: '成功结果', value: numberOrZero(summary.result_succeeded_count) },
    { label: '失败结果', value: numberOrZero(summary.result_failed_count) },
    { label: '最近回调', value: formatObservationTime(summary.latest_result_callback_at) },
    { label: '最近动作', value: formatObservationTime(summary.latest_action_at) },
  ];
}

function searchEvidenceMetrics(integration) {
  const summary = integration?.searchSummary || {};
  return [
    { label: '搜索信号', value: searchEvidenceSignalLabel(integration?.searchSignal) },
    { label: '待供料', value: numberOrZero(summary.required_count) },
    { label: '最近搜索请求', value: formatObservationTime(summary.latest_required_at) },
  ];
}

function JsonPreview({ value }) {
  const text = useMemo(() => JSON.stringify(value || {}, null, 2), [value]);
  return <pre className="external-json-preview">{text}</pre>;
}

function readInitialUrlState() {
  if (typeof window === 'undefined') {
    return {};
  }
  const params = new URLSearchParams(window.location.search);
  const requestedFilter = params.get('audit_filter') || '';
  return {
    integrationId: params.get('integration_id') || '',
    auditFilterKey: EXTERNAL_AUDIT_FILTERS.some((filter) => filter.key === requestedFilter)
      ? requestedFilter
      : '',
    actionId: params.get('action_id') || '',
  };
}

function replaceUrlState({ integrationId, auditFilterKey, actionId }) {
  if (typeof window === 'undefined') {
    return;
  }
  const nextUrl = buildExternalActionPermalink({
    baseUrl: window.location.origin,
    pathname: window.location.pathname,
    integrationId,
    auditFilterKey,
    actionId,
  });
  window.history.replaceState({}, '', nextUrl);
}

export default function ExternalIntegrationsPageClient() {
  const [integrations, setIntegrations] = useState([]);
  const [selectedId, setSelectedId] = useState('');
  const [auditItems, setAuditItems] = useState([]);
  const [auditFilterKey, setAuditFilterKey] = useState('all');
  const [selectedActionId, setSelectedActionId] = useState('');
  const [actionDetail, setActionDetail] = useState(null);
  const [loading, setLoading] = useState(true);
  const [auditLoading, setAuditLoading] = useState(false);
  const [actionDetailLoading, setActionDetailLoading] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [controlBusy, setControlBusy] = useState('');
  const [updatedAt, setUpdatedAt] = useState(null);

  async function loadIntegrations({ silent = false } = {}) {
    if (!silent) {
      setLoading(true);
    }
    setError('');
    try {
      const payload = await fetchJson('/api/v3/external/integrations');
      const next = Array.isArray(payload?.integrations)
        ? payload.integrations.map(normalizeIntegrationSummary)
        : [];
      setIntegrations(next);
      setSelectedId((current) => {
        if (!current) {
          return next[0]?.id || '';
        }
        return next.some((item) => item.id === current) ? current : next[0]?.id || '';
      });
      setUpdatedAt(new Date().toISOString());
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : '外部集成观测数据不可用');
    } finally {
      setLoading(false);
    }
  }

  async function loadAudit(integrationId, filterKey = auditFilterKey) {
    if (!integrationId) {
      setAuditItems([]);
      return;
    }
    setAuditLoading(true);
    try {
      const filter = EXTERNAL_AUDIT_FILTERS.find((item) => item.key === filterKey) || EXTERNAL_AUDIT_FILTERS[0];
      const payload = await fetchJson(
        `/api/v3/external/integrations/${encodeURIComponent(integrationId)}/audit${buildExternalAuditQuery(filter.params)}`,
      );
      setAuditItems(Array.isArray(payload?.items) ? payload.items.map(normalizeAuditItem) : []);
    } catch {
      setAuditItems([]);
    } finally {
      setAuditLoading(false);
    }
  }

  async function loadActionDetail(integrationId, actionId) {
    if (!integrationId || !actionId) {
      setActionDetail(null);
      return;
    }
    setActionDetailLoading(true);
    try {
      const payload = await fetchJson(
        `/api/v3/external/integrations/${encodeURIComponent(integrationId)}/audit${buildExternalAuditQuery({
          itemType: 'action',
          actionId,
          limit: 1,
        })}`,
      );
      const next = Array.isArray(payload?.items) ? payload.items.map(normalizeAuditItem) : [];
      setActionDetail(next[0] || null);
    } catch {
      setActionDetail(null);
    } finally {
      setActionDetailLoading(false);
    }
  }

  async function handleControl(action) {
    if (!selected || controlBusy) {
      return;
    }
    const endpoint = action === 'rotate_secret' ? 'rotate-secret' : action;
    const busyKey = `${action}:${selected.id}`;
    setControlBusy(busyKey);
    setError('');
    setNotice('');
    try {
      const result = normalizeControlResult(await fetchJson(
        `/api/v3/external/integrations/${encodeURIComponent(selected.id)}/${endpoint}`,
        {
          method: 'POST',
          body: {
            reason: `web_external_integrations_${action}`,
            sync_kind: selected.kind === 'source' && action === 'retry' ? 'incremental' : undefined,
          },
        },
      ));
      setNotice(controlResultLabel(result));
      await loadIntegrations({ silent: true });
      await loadAudit(selected.id, auditFilterKey);
    } catch (controlError) {
      setError(controlError instanceof Error ? controlError.message : '外部集成操作失败');
    } finally {
      setControlBusy('');
    }
  }

  function selectIntegration(integrationId) {
    setSelectedId(integrationId);
    setSelectedActionId('');
    setActionDetail(null);
  }

  function selectAuditFilter(filterKey) {
    setAuditFilterKey(filterKey);
    setSelectedActionId('');
    setActionDetail(null);
  }

  async function copyActionPermalink() {
    if (!selectedActionId || typeof window === 'undefined') {
      return;
    }
    const permalink = buildExternalActionPermalink({
      baseUrl: window.location.origin,
      pathname: window.location.pathname,
      integrationId: selectedId,
      auditFilterKey,
      actionId: selectedActionId,
    });
    try {
      await window.navigator.clipboard.writeText(permalink);
      setNotice('动作定位链接已复制');
    } catch {
      setError('复制动作定位链接失败');
    }
  }

  function exportActionTrace() {
    if (!selected || !actionDetail || typeof window === 'undefined') {
      return;
    }
    const trace = buildExternalActionTrace({ integration: selected, action: actionDetail });
    const blob = new Blob([`${JSON.stringify(trace, null, 2)}\n`], { type: 'application/json' });
    const url = window.URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = externalActionTraceFilename(actionDetail);
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    window.URL.revokeObjectURL(url);
    setNotice('脱敏 trace 已导出');
  }

  useEffect(() => {
    const initialUrlState = readInitialUrlState();
    if (initialUrlState.integrationId) {
      setSelectedId(initialUrlState.integrationId);
    }
    if (initialUrlState.auditFilterKey) {
      setAuditFilterKey(initialUrlState.auditFilterKey);
    }
    if (initialUrlState.actionId) {
      setSelectedActionId(initialUrlState.actionId);
    }
    loadIntegrations();
    const timer = window.setInterval(() => {
      loadIntegrations({ silent: true });
    }, REFRESH_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    loadAudit(selectedId, auditFilterKey);
  }, [selectedId, auditFilterKey]);

  useEffect(() => {
    loadActionDetail(selectedId, selectedActionId);
  }, [selectedId, selectedActionId]);

  useEffect(() => {
    replaceUrlState({ integrationId: selectedId, auditFilterKey, actionId: selectedActionId });
  }, [selectedId, auditFilterKey, selectedActionId]);

  const selected = integrations.find((item) => item.id === selectedId) || integrations[0] || null;
  const totals = {
    pending: metricTotal(integrations, 'pendingActionCount'),
    blocked: metricTotal(integrations, 'blockedActionCount'),
    failed: metricTotal(integrations, 'failedActionCount'),
    dispatched: metricTotal(integrations, 'dispatchedActionCount'),
    driftIssues: integrations.filter((item) => !['ok', 'unknown'].includes(item.driftSignal)).length,
    artifactIssues: integrations.filter((item) => ['artifact_confirmation_pending', 'artifact_blocked', 'artifact_failed'].includes(item.artifactSignal)).length,
    resultCallbacks: integrations.reduce((sum, item) => sum + numberOrZero(item.actionSummary?.result_callback_count), 0),
    waitingResults: integrations.reduce((sum, item) => sum + numberOrZero(item.actionSummary?.waiting_result_count), 0),
    searchEvidenceRequired: integrations.reduce((sum, item) => sum + numberOrZero(item.searchSummary?.required_count), 0),
  };

  return (
    <main className="external-observability-shell">
      <section className="external-hero-band">
        <div>
          <p className="external-kicker">V3 External Integrations</p>
          <h1>外部集成观测</h1>
          <div className="external-domain-line">
            <span>默认接口域名</span>
            <code>{thirdPartyApiBaseUrl()}</code>
          </div>
        </div>
        <div className="external-hero-metrics" aria-label="外部动作概览">
          <div>
            <strong>{integrations.length}</strong>
            <span>集成</span>
          </div>
          <div>
            <strong>{totals.pending}</strong>
            <span>待确认</span>
          </div>
          <div>
            <strong>{totals.blocked}</strong>
            <span>阻断</span>
          </div>
          <div>
            <strong>{totals.failed}</strong>
            <span>失败</span>
          </div>
          <div>
            <strong>{totals.waitingResults}</strong>
            <span>待结果</span>
          </div>
          <div>
            <strong>{totals.resultCallbacks}</strong>
            <span>已回调</span>
          </div>
          <div>
            <strong>{totals.searchEvidenceRequired}</strong>
            <span>搜索待供料</span>
          </div>
        </div>
      </section>

      <section className="external-api-band" aria-label="第三方接口">
        <div>
          <span>聊天事件</span>
          <code>{buildThirdPartyApiUrl('/v1/external/channels/{connection_id}/events')}</code>
        </div>
        <div>
          <span>流式聊天事件</span>
          <code>{buildThirdPartyApiUrl('/v1/external/channels/{connection_id}/events/stream')}</code>
        </div>
        <div>
          <span>动作确认</span>
          <code>{buildThirdPartyApiUrl('/v1/external/channels/{connection_id}/confirmations')}</code>
        </div>
        <div>
          <span>结果回调</span>
          <code>{buildThirdPartyApiUrl('/v1/external/channels/{connection_id}/actions/{action_id}/result')}</code>
        </div>
        <div>
          <span>观测列表</span>
          <code>{buildThirdPartyApiUrl('/v1/external/integrations')}</code>
        </div>
      </section>

      <section className="external-mode-band" aria-label="对接方式与文档">
        <div className="external-mode-head">
          <div>
            <p className="external-kicker">Integration Playbooks</p>
            <h2>对接方式与文档</h2>
          </div>
          <p>
            面向所有客户的公开说明入口。凭证、客户 endpoint、真实权限样例和生产网络参数请通过线下交付。
          </p>
        </div>
        <div className="external-mode-grid">
          {EXTERNAL_INTEGRATION_MODES.map((mode) => (
            <article className="external-mode-card" key={mode.key}>
              <div className="external-mode-card-head">
                <h3>{mode.title}</h3>
                <span>{mode.status}</span>
              </div>
              <p>{mode.summary}</p>
              <div className="external-mode-docs" aria-label={`${mode.title} 文档路径`}>
                {mode.docs.map((docPath) => (
                  <code key={docPath}>{docPath}</code>
                ))}
              </div>
              {Array.isArray(mode.documentLinks) && mode.documentLinks.length ? (
                <div className="external-mode-actions" aria-label={`${mode.title} 可打开文档`}>
                  {mode.documentLinks.map((link) => (
                    <a
                      key={link.key || link.href}
                      className={`external-mode-link external-mode-link-${link.kind || 'doc'}`}
                      href={link.href}
                      target={link.download ? undefined : link.target || '_blank'}
                      rel={link.download ? undefined : 'noopener noreferrer'}
                      download={link.download || undefined}
                    >
                      {link.label}
                    </a>
                  ))}
                </div>
              ) : null}
              {Array.isArray(mode.checklist) && mode.checklist.length ? (
                <div className="external-mode-checklist" aria-label={`${mode.title} 完整对接清单`}>
                  <strong>完整对接清单</strong>
                  <ul>
                    {mode.checklist.map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ul>
                </div>
              ) : null}
              <small>{mode.guardrail}</small>
            </article>
          ))}
        </div>
      </section>

      {error ? <div className="external-error-line">{error}</div> : null}
      {notice ? <div className="external-notice-line">{notice}</div> : null}

      <section className="external-dashboard-grid">
        <div className="external-panel external-list-panel">
          <div className="external-panel-head">
            <div>
              <h2>连接状态</h2>
              <p>{loading ? '刷新中' : `更新 ${formatObservationTime(updatedAt)}`}</p>
            </div>
            <button type="button" className="external-refresh-button" onClick={() => loadIntegrations()}>
              刷新
            </button>
          </div>

          <div className="external-integration-table">
            <div className="external-table-row external-table-head">
              <span>集成</span>
              <span>状态</span>
              <span>治理</span>
              <span>动作</span>
              <span>最近活动</span>
            </div>
            {integrations.map((integration) => (
              <button
                type="button"
                key={`${integration.kind}:${integration.id}`}
                className={`external-table-row external-table-button${selected?.id === integration.id ? ' is-selected' : ''}`}
                onClick={() => selectIntegration(integration.id)}
              >
                <span>
                  <strong>{integration.displayName}</strong>
                  <small>{integration.kind} / {integration.provider}</small>
                </span>
                <span className={statusClass(integration.signal)}>{signalLabel(integration.signal)}</span>
                <span className={driftClass(integration.driftSignal)}>
                  {driftSignalLabel(integration.driftSignal)}
                </span>
                <span className="external-action-counts">
                  <b>{integration.dispatchedActionCount}</b>
                  <b>{integration.blockedActionCount}</b>
                  <b>{integration.failedActionCount}</b>
                </span>
                <span>{formatObservationTime(latestIntegrationActivity(integration))}</span>
              </button>
            ))}
            {!loading && !integrations.length ? (
              <div className="external-empty-state">暂无外部集成记录</div>
            ) : null}
          </div>
        </div>

        <div className="external-panel external-detail-panel">
          <div className="external-panel-head">
            <div>
              <h2>{selected?.displayName || '集成详情'}</h2>
              <p>{selected ? `${selected.kind} / ${selected.provider}` : '无选中项'}</p>
            </div>
            {selected ? (
              <div className="external-detail-head-signals">
                <span className={statusClass(selected.signal)}>{signalLabel(selected.signal)}</span>
                <span className={driftClass(selected.driftSignal)}>{driftSignalLabel(selected.driftSignal)}</span>
                {selected.kind === 'channel' ? (
                  <>
                    <span className={statusClass(selected.actionSignal)}>{actionSignalLabel(selected.actionSignal)}</span>
                    <span className={artifactClass(selected.artifactSignal)}>
                      {artifactSignalLabel(selected.artifactSignal)}
                    </span>
                  </>
                ) : null}
              </div>
            ) : null}
          </div>

          {selected ? (
            <>
              <div className="external-control-row" aria-label="外部集成管理操作">
                <button
                  type="button"
                  className="external-control-button"
                  disabled={Boolean(controlBusy)}
                  onClick={() => handleControl('retry')}
                >
                  {controlBusy === `retry:${selected.id}` ? '重试中' : selected.kind === 'source' ? '重试同步' : '重试动作'}
                </button>
                <button
                  type="button"
                  className="external-control-button"
                  disabled={Boolean(controlBusy)}
                  onClick={() => handleControl('rotate_secret')}
                >
                  {controlBusy === `rotate_secret:${selected.id}` ? '记录中' : '标记轮换'}
                </button>
                <button
                  type="button"
                  className="external-control-button external-control-danger"
                  disabled={Boolean(controlBusy) || selected.signal === 'disabled'}
                  onClick={() => handleControl('disable')}
                >
                  {controlBusy === `disable:${selected.id}` ? '停用中' : '停用'}
                </button>
              </div>
              <div className="external-detail-strip">
                <div>
                  <span>健康</span>
                  <strong>{selected.healthStatus}</strong>
                </div>
                <div>
                  <span>状态</span>
                  <strong>{selected.status}</strong>
                </div>
                <div>
                  <span>最近成功</span>
                  <strong>{formatObservationTime(selected.lastSuccessAt)}</strong>
                </div>
                <div>
                  <span>最近失败</span>
                  <strong>{formatObservationTime(selected.lastFailureAt)}</strong>
                </div>
              </div>
              <div className="external-detail-strip external-governance-strip">
                {governanceMetrics(selected).map((metric) => (
                  <div key={metric.label}>
                    <span>{metric.label}</span>
                    <strong>{metric.value}</strong>
                  </div>
                ))}
              </div>
              {selected.kind === 'channel' ? (
                <>
                  <div className="external-detail-strip external-action-strip">
                    {actionLifecycleMetrics(selected).map((metric) => (
                      <div key={metric.label}>
                        <span>{metric.label}</span>
                        <strong>{metric.value}</strong>
                      </div>
                    ))}
                  </div>
                  <div className="external-detail-strip external-search-strip">
                    {searchEvidenceMetrics(selected).map((metric) => (
                      <div key={metric.label}>
                        <span>{metric.label}</span>
                        <strong>{metric.value}</strong>
                      </div>
                    ))}
                  </div>
                  <div className="external-detail-strip external-artifact-strip">
                    {artifactMetrics(selected).map((metric) => (
                      <div key={metric.label}>
                        <span>{metric.label}</span>
                        <strong>{metric.value}</strong>
                      </div>
                    ))}
                  </div>
                </>
              ) : null}
              <JsonPreview value={selected.configSummary} />
            </>
          ) : (
            <div className="external-empty-state">选择一个集成查看详情</div>
          )}
        </div>

        <div className="external-panel external-audit-panel">
          <div className="external-panel-head">
            <div>
              <h2>审计时间线</h2>
              <p>{auditLoading ? '读取中' : `${auditItems.length} 条记录`}</p>
            </div>
          </div>
          <div className="external-audit-filters" aria-label="审计筛选">
            {EXTERNAL_AUDIT_FILTERS.map((filter) => (
              <button
                type="button"
                key={filter.key}
                className={auditFilterKey === filter.key ? 'is-selected' : ''}
                onClick={() => selectAuditFilter(filter.key)}
              >
                {filter.label}
              </button>
            ))}
          </div>
          {selectedActionId ? (
            <div className="external-action-detail">
              <div className="external-action-detail-head">
                <div>
                  <span>动作详情</span>
                  <strong>{selectedActionId}</strong>
                </div>
                <button type="button" onClick={() => setSelectedActionId('')}>
                  关闭
                </button>
                <div className="external-action-detail-actions">
                  <button type="button" onClick={copyActionPermalink}>
                    复制链接
                  </button>
                  <button type="button" disabled={!actionDetail} onClick={exportActionTrace}>
                    导出 trace
                  </button>
                </div>
              </div>
              {actionDetailLoading ? (
                <div className="external-empty-state">读取动作详情</div>
              ) : actionDetail ? (
                <>
                  <div className="external-action-detail-strip">
                    <div>
                      <span>状态</span>
                      <strong>{actionDetail.status || 'recorded'}</strong>
                    </div>
                    <div>
                      <span>失败原因</span>
                      <strong>{actionDetail.failureKind || '无'}</strong>
                    </div>
                    <div>
                      <span>AssistantRun</span>
                      <strong>{actionDetail.assistantRunId || '无记录'}</strong>
                    </div>
                    <div>
                      <span>更新时间</span>
                      <strong>{formatObservationTime(actionDetail.createdAt)}</strong>
                    </div>
                  </div>
                  <JsonPreview value={actionDetail.summary} />
                </>
              ) : (
                <div className="external-empty-state">未找到该动作详情</div>
              )}
            </div>
          ) : null}
          <div className="external-audit-list">
            {auditItems.map((item, index) => (
              <article
                className={`external-audit-item${selectedActionId && selectedActionId === item.actionId ? ' is-selected' : ''}`}
                key={`${item.itemType}:${item.actionId || item.createdAt || index}`}
              >
                <div>
                  <span className="external-audit-type">{auditItemTypeLabel(item.itemType)}</span>
                  <strong>{item.status || item.failureKind || 'recorded'}</strong>
                  <small>{formatObservationTime(item.createdAt)}</small>
                  {item.itemType === 'action' && item.actionId ? (
                    <button
                      type="button"
                      className="external-audit-detail-button"
                      onClick={() => setSelectedActionId(item.actionId)}
                    >
                      详情
                    </button>
                  ) : null}
                </div>
                <JsonPreview value={item.summary} />
              </article>
            ))}
            {!auditLoading && !auditItems.length ? (
              <div className="external-empty-state">暂无审计记录</div>
            ) : null}
          </div>
        </div>
      </section>
    </main>
  );
}
