'use client';

import { useEffect, useMemo, useState } from 'react';
import {
  buildThirdPartyApiUrl,
  formatObservationTime,
  latestIntegrationActivity,
  normalizeAuditItem,
  normalizeIntegrationSummary,
  signalLabel,
  thirdPartyApiBaseUrl,
} from '../lib/external-integrations';

const REFRESH_INTERVAL_MS = 15000;

async function fetchJson(pathname) {
  const response = await fetch(pathname, {
    cache: 'no-store',
    headers: { accept: 'application/json' },
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
            {selected ? <span className={statusClass(selected.signal)}>{signalLabel(selected.signal)}</span> : null}
          </div>

          {selected ? (
            <>
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
