'use client';

import { useEffect, useMemo, useState } from 'react';
import {
  buildThirdPartyApiUrl,
  controlResultLabel,
  driftSignalLabel,
  formatObservationTime,
  latestIntegrationActivity,
  normalizeControlResult,
  normalizeAuditItem,
  normalizeIntegrationSummary,
  numberOrZero,
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

function JsonPreview({ value }) {
  const text = useMemo(() => JSON.stringify(value || {}, null, 2), [value]);
  return <pre className="external-json-preview">{text}</pre>;
}

export default function ExternalIntegrationsPageClient() {
  const [integrations, setIntegrations] = useState([]);
  const [selectedId, setSelectedId] = useState('');
  const [auditItems, setAuditItems] = useState([]);
  const [loading, setLoading] = useState(true);
  const [auditLoading, setAuditLoading] = useState(false);
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
      setSelectedId((current) => current || next[0]?.id || '');
      setUpdatedAt(new Date().toISOString());
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : '外部集成观测数据不可用');
    } finally {
      setLoading(false);
    }
  }

  async function loadAudit(integrationId) {
    if (!integrationId) {
      setAuditItems([]);
      return;
    }
    setAuditLoading(true);
    try {
      const payload = await fetchJson(`/api/v3/external/integrations/${encodeURIComponent(integrationId)}/audit`);
      setAuditItems(Array.isArray(payload?.items) ? payload.items.map(normalizeAuditItem) : []);
    } catch {
      setAuditItems([]);
    } finally {
      setAuditLoading(false);
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
      await loadAudit(selected.id);
    } catch (controlError) {
      setError(controlError instanceof Error ? controlError.message : '外部集成操作失败');
    } finally {
      setControlBusy('');
    }
  }

  useEffect(() => {
    loadIntegrations();
    const timer = window.setInterval(() => {
      loadIntegrations({ silent: true });
    }, REFRESH_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    loadAudit(selectedId);
  }, [selectedId]);

  const selected = integrations.find((item) => item.id === selectedId) || integrations[0] || null;
  const totals = {
    pending: metricTotal(integrations, 'pendingActionCount'),
    blocked: metricTotal(integrations, 'blockedActionCount'),
    failed: metricTotal(integrations, 'failedActionCount'),
    dispatched: metricTotal(integrations, 'dispatchedActionCount'),
    driftIssues: integrations.filter((item) => !['ok', 'unknown'].includes(item.driftSignal)).length,
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
            <strong>{totals.driftIssues}</strong>
            <span>治理告警</span>
          </div>
        </div>
      </section>

      <section className="external-api-band" aria-label="第三方接口">
        <div>
          <span>聊天事件</span>
          <code>{buildThirdPartyApiUrl('/v1/external/channels/{connection_id}/events')}</code>
        </div>
        <div>
          <span>动作确认</span>
          <code>{buildThirdPartyApiUrl('/v1/external/channels/{connection_id}/confirmations')}</code>
        </div>
        <div>
          <span>观测列表</span>
          <code>{buildThirdPartyApiUrl('/v1/external/integrations')}</code>
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
                onClick={() => setSelectedId(integration.id)}
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
          <div className="external-audit-list">
            {auditItems.map((item, index) => (
              <article className="external-audit-item" key={`${item.itemType}:${item.actionId || item.createdAt || index}`}>
                <div>
                  <span className="external-audit-type">{item.itemType}</span>
                  <strong>{item.status || item.failureKind || 'recorded'}</strong>
                  <small>{formatObservationTime(item.createdAt)}</small>
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
