'use client';

import { useEffect, useMemo, useState } from 'react';
import {
  createModelGatewayProfile,
  disableModelGatewayProfile,
  fetchModelGatewayPresets,
  fetchModelGatewayProfiles,
  fetchModelGatewayStatus,
  modelGatewayProfileStatusSummary,
  testModelGatewayProfile,
  updateModelGatewayProfile,
} from '../lib/model-gateway';

const DEFAULT_DRAFT = {
  profileId: '',
  displayName: '',
  lane: 'assistant_chat',
  providerId: '',
  modelId: '',
  baseUrl: '',
  apiPath: '/v1/chat/completions',
  wireApi: 'openai-compatible',
  authMode: 'env_key',
  authEnvKeyName: '',
  recommendedPreset: '',
  maxConcurrency: '',
  rpmLimit: '',
  tpmLimit: '',
  timeoutMs: '',
  priority: '',
  enabled: true,
};

function numberText(value) {
  return value === null || value === undefined ? '' : String(value);
}

function draftFromPreset(preset, current = DEFAULT_DRAFT) {
  if (!preset) return { ...current };
  return {
    ...current,
    lane: preset.lane || current.lane,
    providerId: preset.providerId || current.providerId,
    modelId: preset.modelId || current.modelId,
    baseUrl: preset.baseUrl || current.baseUrl,
    apiPath: preset.apiPath || current.apiPath,
    wireApi: preset.wireApi || current.wireApi,
    recommendedPreset: preset.id || current.recommendedPreset,
    maxConcurrency: numberText(preset.maxConcurrency),
    rpmLimit: numberText(preset.rpmLimit),
    tpmLimit: numberText(preset.tpmLimit),
    timeoutMs: numberText(preset.timeoutMs),
    priority: numberText(preset.priority),
  };
}

function draftFromProfile(profile) {
  return {
    profileId: profile.profileId,
    displayName: profile.displayName,
    lane: profile.lane,
    providerId: profile.providerId,
    modelId: profile.modelId,
    baseUrl: profile.baseUrl,
    apiPath: profile.apiPath,
    wireApi: profile.wireApi,
    authMode: profile.authMode,
    authEnvKeyName: profile.authEnvKeyName,
    recommendedPreset: profile.recommendedPreset,
    maxConcurrency: numberText(profile.maxConcurrency),
    rpmLimit: numberText(profile.rpmLimit),
    tpmLimit: numberText(profile.tpmLimit),
    timeoutMs: numberText(profile.timeoutMs),
    priority: numberText(profile.priority),
    enabled: profile.enabled,
  };
}

function MetricPill({ label, value }) {
  return (
    <span className="model-pool-pill">
      <small>{label}</small>
      <strong>{value === null || value === undefined || value === '' ? '-' : value}</strong>
    </span>
  );
}

function formatLatency(value) {
  return value === null || value === undefined ? '' : `${value}ms`;
}

function formatPercent(value) {
  return value === null || value === undefined ? '' : `${value}%`;
}

function ModelPoolRuntimeSummary({ status, loading }) {
  const lane = (status.lanes || []).find((item) => item.lane === 'assistant_chat') || (status.lanes || [])[0] || {};
  const providers = status.providers || [];
  const openCircuitCount = providers.filter((provider) => provider.circuitOpen || provider.circuitState === 'open').length;
  const activeCount = providers.reduce((total, provider) => total + (provider.activeCount || 0), 0);
  const queuedCount = providers.reduce((total, provider) => total + (provider.queuedCount || 0), 0);
  const p95Values = providers.map((provider) => provider.p95LatencyMs).filter((value) => value !== null && value !== undefined);
  const maxP95 = p95Values.length ? Math.max(...p95Values) : null;
  const canaryLabel = lane.canaryPercent === null || lane.canaryPercent === undefined ? '' : `${lane.canaryPercent}%`;

  return (
    <div className="model-pool-runtime-strip">
      <MetricPill label="路由模式" value={lane.routingMode || (loading ? '同步中' : '')} />
      <MetricPill label="灰度" value={canaryLabel} />
      <MetricPill label="Lane 并发" value={`${lane.activeCount || activeCount}/${lane.maxConcurrency || '-'}`} />
      <MetricPill label="队列" value={`${lane.queuedCount || queuedCount}/${lane.queueLimit ?? '-'}`} />
      <MetricPill label="熔断" value={openCircuitCount} />
      <MetricPill label="P95" value={formatLatency(maxP95)} />
    </div>
  );
}

function ProfileCard({ profile, status, onEdit, onDisable, onTest, testing }) {
  const summary = modelGatewayProfileStatusSummary(profile, status);
  const providerStatus = summary.providerStatus || {};
  return (
    <article className="model-pool-profile-card">
      <div className="model-pool-profile-main">
        <div>
          <span className={`model-pool-status status-${summary.tone}`}>{summary.label}</span>
          <h3>{profile.displayName || profile.profileId}</h3>
          <p>{profile.profileId} · {profile.providerId}/{profile.modelId}</p>
        </div>
        <div className="model-pool-profile-actions">
          <button type="button" className="ghost-btn compact-action-btn" onClick={() => onTest(profile.profileId)} disabled={testing}>
            {testing ? '检查中' : '测试'}
          </button>
          <button type="button" className="ghost-btn compact-action-btn" onClick={() => onEdit(profile)}>
            编辑
          </button>
          <button type="button" className="ghost-btn compact-action-btn danger-action" disabled={!profile.enabled} onClick={() => onDisable(profile.profileId)}>
            停用
          </button>
        </div>
      </div>
      <div className="model-pool-metrics">
        <MetricPill label="Lane" value={profile.lane} />
        <MetricPill label="运行" value={`${providerStatus.activeCount || 0}/${profile.maxConcurrency || providerStatus.maxConcurrency || '-'}`} />
        <MetricPill label="排队" value={providerStatus.queuedCount || 0} />
        <MetricPill label="分钟请求" value={`${providerStatus.minuteRequestCount || 0}/${profile.rpmLimit || providerStatus.rpmLimit || '-'}`} />
        <MetricPill label="分钟Token" value={`${providerStatus.minuteTokenCount || 0}/${profile.tpmLimit || providerStatus.tpmLimit || '-'}`} />
        <MetricPill label="P95" value={formatLatency(providerStatus.p95LatencyMs)} />
        <MetricPill label="质量" value={formatPercent(providerStatus.qualityScore)} />
        <MetricPill label="格式" value={formatPercent(providerStatus.formatPassRate)} />
        <MetricPill label="修复率" value={formatPercent(providerStatus.repairRate)} />
        <MetricPill label="Shadow" value={providerStatus.shadowEvalCount || 0} />
        <MetricPill label="观测限流" value={providerStatus.wouldThrottleCount || 0} />
        <MetricPill label="失败" value={providerStatus.runtimeFailureCount || providerStatus.failureCount || 0} />
        <MetricPill label="超时" value={profile.timeoutMs ? `${profile.timeoutMs}ms` : ''} />
        <MetricPill label="优先级" value={profile.priority} />
        <MetricPill label="请求" value={providerStatus.requestCount || 0} />
        <MetricPill label="最近失败" value={providerStatus.lastFailureReason} />
        <MetricPill label="密钥" value={profile.hasSecret ? profile.authEnvKeyName : '未配置'} />
      </div>
    </article>
  );
}

export default function ModelPoolPanel({ accountStatusSummary }) {
  const signedIn = Boolean(accountStatusSummary?.signedIn);
  const [presets, setPresets] = useState([]);
  const [profiles, setProfiles] = useState([]);
  const [status, setStatus] = useState({ lanes: [], providers: [] });
  const [draft, setDraft] = useState(DEFAULT_DRAFT);
  const [editingProfileId, setEditingProfileId] = useState('');
  const [loading, setLoading] = useState(false);
  const [statusLoading, setStatusLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testingProfileId, setTestingProfileId] = useState('');
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');

  const selectedPreset = useMemo(
    () => presets.find((preset) => preset.id === draft.recommendedPreset) || null,
    [draft.recommendedPreset, presets],
  );

  async function loadModelPool() {
    if (!signedIn) return;
    setLoading(true);
    setStatusLoading(true);
    setError('');
    try {
      const [nextPresets, nextProfiles, nextStatus] = await Promise.all([
        fetchModelGatewayPresets(),
        fetchModelGatewayProfiles(),
        fetchModelGatewayStatus(),
      ]);
      setPresets(nextPresets);
      setProfiles(nextProfiles);
      setStatus(nextStatus);
      if (!draft.recommendedPreset && nextPresets.length) {
        setDraft((current) => draftFromPreset(nextPresets[0], current));
      }
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : '模型池读取失败');
    } finally {
      setLoading(false);
      setStatusLoading(false);
    }
  }

  useEffect(() => {
    loadModelPool();
  }, [signedIn]);

  useEffect(() => {
    if (!signedIn) return undefined;
    let cancelled = false;
    const interval = window.setInterval(async () => {
      try {
        const nextStatus = await fetchModelGatewayStatus();
        if (!cancelled) {
          setStatus(nextStatus);
        }
      } catch (_nextError) {
        // Keep the latest visible status; manual refresh surfaces request failures.
      }
    }, 15000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [signedIn]);

  function updateDraft(field, value) {
    setDraft((current) => ({ ...current, [field]: value }));
  }

  function startCreate() {
    const preset = presets[0] || null;
    setEditingProfileId('');
    setDraft(draftFromPreset(preset, {
      ...DEFAULT_DRAFT,
      profileId: '',
      displayName: '',
      enabled: true,
    }));
    setMessage('');
    setError('');
  }

  function startEdit(profile) {
    setEditingProfileId(profile.profileId);
    setDraft(draftFromProfile(profile));
    setMessage('');
    setError('');
  }

  async function submitProfile(event) {
    event.preventDefault();
    setSaving(true);
    setError('');
    setMessage('');
    try {
      if (editingProfileId) {
        await updateModelGatewayProfile(editingProfileId, draft);
        setMessage('模型配置已更新。');
      } else {
        await createModelGatewayProfile(draft);
        setMessage('模型配置已创建。');
      }
      await loadModelPool();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : '保存失败');
    } finally {
      setSaving(false);
    }
  }

  async function handleDisable(profileId) {
    setError('');
    setMessage('');
    try {
      await disableModelGatewayProfile(profileId);
      setMessage('模型配置已停用。');
      await loadModelPool();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : '停用失败');
    }
  }

  async function handleTest(profileId) {
    setTestingProfileId(profileId);
    setError('');
    setMessage('');
    try {
      const result = await testModelGatewayProfile(profileId);
      setMessage(result?.message || '连接检查完成。');
      const nextStatus = await fetchModelGatewayStatus();
      setStatus(nextStatus);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : '测试失败');
    } finally {
      setTestingProfileId('');
    }
  }

  if (!signedIn) {
    return (
      <div className="model-pool-login-state">
        <strong>需要登录主系统</strong>
        <span>模型池配置属于运维面，登录后可添加 profile、查看密钥绑定状态和执行连接检查。</span>
      </div>
    );
  }

  return (
    <div className="model-pool-layout">
      <section className="directory-card model-pool-list-card">
        <div className="directory-section-head">
          <div>
            <h3>模型配置</h3>
            <p>{loading ? '正在同步模型池...' : `${profiles.length} 个 profile`}</p>
          </div>
          <div className="model-pool-head-actions">
            <button type="button" className="ghost-btn compact-action-btn" onClick={loadModelPool} disabled={loading}>
              刷新
            </button>
            <button type="button" className="primary-btn compact-action-btn" onClick={startCreate}>
              添加模型
            </button>
          </div>
        </div>
        <ModelPoolRuntimeSummary status={status} loading={statusLoading || loading} />
        <div className="model-pool-profile-list">
          {profiles.length ? profiles.map((profile) => (
            <ProfileCard
              key={profile.profileId}
              profile={profile}
              status={status}
              onEdit={startEdit}
              onDisable={handleDisable}
              onTest={handleTest}
              testing={testingProfileId === profile.profileId}
            />
          )) : (
            <div className="directory-empty">暂无模型 profile。先从右侧选择 preset 创建一个配置。</div>
          )}
        </div>
      </section>

      <section className="directory-card model-pool-editor-card">
        <div className="directory-section-head">
          <div>
            <h3>{editingProfileId ? '编辑 profile' : '添加 profile'}</h3>
            <p>{selectedPreset ? `${selectedPreset.providerId}/${selectedPreset.modelId}` : '选择 preset 后自动填入推荐值'}</p>
          </div>
        </div>
        <form className="model-pool-form" onSubmit={submitProfile}>
          <label>
            <span>推荐 preset</span>
            <select
              value={draft.recommendedPreset}
              onChange={(event) => {
                const preset = presets.find((item) => item.id === event.target.value);
                setDraft((current) => draftFromPreset(preset, current));
              }}
            >
              <option value="">自定义</option>
              {presets.map((preset) => (
                <option key={preset.id} value={preset.id}>{preset.label}</option>
              ))}
            </select>
          </label>
          <div className="model-pool-form-grid">
            <label>
              <span>Profile ID</span>
              <input value={draft.profileId} onChange={(event) => updateDraft('profileId', event.target.value)} disabled={Boolean(editingProfileId)} placeholder="openclaw-main" />
            </label>
            <label>
              <span>显示名称</span>
              <input value={draft.displayName} onChange={(event) => updateDraft('displayName', event.target.value)} placeholder="OpenClaw 主通道" />
            </label>
            <label>
              <span>Lane</span>
              <input value={draft.lane} onChange={(event) => updateDraft('lane', event.target.value)} />
            </label>
            <label>
              <span>Provider</span>
              <input value={draft.providerId} onChange={(event) => updateDraft('providerId', event.target.value)} placeholder="openclaw" />
            </label>
            <label>
              <span>Model</span>
              <input value={draft.modelId} onChange={(event) => updateDraft('modelId', event.target.value)} placeholder="default" />
            </label>
            <label>
              <span>Wire API</span>
              <input value={draft.wireApi} onChange={(event) => updateDraft('wireApi', event.target.value)} />
            </label>
            <label>
              <span>Base URL</span>
              <input value={draft.baseUrl} onChange={(event) => updateDraft('baseUrl', event.target.value)} placeholder="https://example.com" />
            </label>
            <label>
              <span>API Path</span>
              <input value={draft.apiPath} onChange={(event) => updateDraft('apiPath', event.target.value)} />
            </label>
            <label>
              <span>Auth Env</span>
              <input value={draft.authEnvKeyName} onChange={(event) => updateDraft('authEnvKeyName', event.target.value)} placeholder="OPENCLAW_API_KEY" />
            </label>
            <label>
              <span>并发</span>
              <input type="number" min="1" value={draft.maxConcurrency} onChange={(event) => updateDraft('maxConcurrency', event.target.value)} />
            </label>
            <label>
              <span>RPM</span>
              <input type="number" min="1" value={draft.rpmLimit} onChange={(event) => updateDraft('rpmLimit', event.target.value)} />
            </label>
            <label>
              <span>TPM</span>
              <input type="number" min="1" value={draft.tpmLimit} onChange={(event) => updateDraft('tpmLimit', event.target.value)} />
            </label>
            <label>
              <span>超时 ms</span>
              <input type="number" min="1" value={draft.timeoutMs} onChange={(event) => updateDraft('timeoutMs', event.target.value)} />
            </label>
            <label>
              <span>优先级</span>
              <input type="number" value={draft.priority} onChange={(event) => updateDraft('priority', event.target.value)} />
            </label>
          </div>
          <label className="model-pool-toggle">
            <input
              type="checkbox"
              checked={draft.enabled}
              onChange={(event) => updateDraft('enabled', event.target.checked)}
            />
            <span>启用配置</span>
          </label>
          {selectedPreset ? (
            <div className="model-pool-preset-preview">
              <MetricPill label="推荐并发" value={selectedPreset.maxConcurrency} />
              <MetricPill label="推荐超时" value={`${selectedPreset.timeoutMs}ms`} />
              <MetricPill label="推荐优先级" value={selectedPreset.priority} />
            </div>
          ) : null}
          <div className="model-pool-form-actions">
            <button type="button" className="ghost-btn compact-action-btn" onClick={startCreate}>
              清空
            </button>
            <button type="submit" className="primary-btn compact-action-btn" disabled={saving || !draft.profileId.trim() || !draft.displayName.trim()}>
              {saving ? '保存中' : editingProfileId ? '保存修改' : '创建 profile'}
            </button>
          </div>
        </form>
        {message ? <p className="model-pool-message success">{message}</p> : null}
        {error ? <p className="model-pool-message error">{error}</p> : null}
      </section>
    </div>
  );
}
