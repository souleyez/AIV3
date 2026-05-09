'use client';

import { useState } from 'react';

function DatasetCreateButton({
  creating,
  signedIn,
  draft,
  onDraftChange,
  onCreate,
}) {
  const [open, setOpen] = useState(false);
  const title = draft?.title || '';

  return (
    <div className="dataset-create-control">
      <button
        className="dataset-create-plus"
        type="button"
        onClick={() => setOpen((current) => !current)}
        disabled={creating}
        aria-label={signedIn ? '按当前登录用户新建数据集' : '新建本机公开数据集'}
        title={signedIn ? '按当前登录用户新建数据集' : '未登录时新建本机公开数据集'}
      >
        {creating ? '...' : '+'}
      </button>
      {open ? (
        <form
          className="dataset-create-popover"
          onSubmit={(event) => {
            event.preventDefault();
            if (!title.trim() || creating) {
              return;
            }
            onCreate?.();
            setOpen(false);
          }}
        >
          <label>
            <span>数据集名称</span>
            <input
              value={title}
              onChange={(event) => onDraftChange?.('title', event.target.value)}
              placeholder="例如：客服问答"
              disabled={creating}
              autoFocus
            />
          </label>
          <p>{signedIn ? '创建后归属当前账号。' : '未登录创建为本机公开数据集。'}</p>
          <div className="dataset-create-actions">
            <button
              className="ghost-btn compact-action-btn"
              type="button"
              onClick={() => {
                onDraftChange?.('title', '');
                setOpen(false);
              }}
              disabled={creating}
            >
              取消
            </button>
            <button
              className="primary-btn compact-action-btn"
              type="submit"
              disabled={creating || !title.trim()}
            >
              确认
            </button>
          </div>
        </form>
      ) : null}
    </div>
  );
}

function AccountPanel({ accountAuth }) {
  const status = accountAuth?.statusSummary || {
    label: '未登录',
    detail: '可普通聊天；私密数据集需要邮箱或本地密钥',
    signedIn: false,
  };
  const busy = Boolean(accountAuth?.busy);
  const emailDraft = accountAuth?.emailDraft || '';
  const codeDraft = accountAuth?.codeDraft || '';
  const newKeyDraft = accountAuth?.newKeyDraft || '';

  return (
    <section className="side-card side-form account-card">
      <div className="account-status-row">
        <div>
          <div className="card-title account-card-title">账号 / 密钥</div>
          <strong>{status.label}</strong>
          <p>{status.detail}</p>
        </div>
        {status.signedIn ? (
          <button
            className="ghost-btn compact-action-btn"
            type="button"
            onClick={accountAuth?.onLogout}
            disabled={busy}
          >
            退出
          </button>
        ) : null}
      </div>
      <label className="side-form-field">
        <span>邮箱</span>
        <input
          value={emailDraft}
          onChange={(event) => accountAuth?.onEmailDraftChange?.(event.target.value)}
          placeholder="用于找回密钥和区分私密数据"
          disabled={busy}
          type="email"
          autoComplete="email"
        />
      </label>
      <div className="account-action-grid">
        <button
          className="ghost-btn"
          type="button"
          onClick={accountAuth?.onSendEmailCode}
          disabled={busy || !emailDraft.trim()}
        >
          发验证码
        </button>
        <button
          className="primary-btn"
          type="button"
          onClick={accountAuth?.onLoginWithKey}
          disabled={busy || !emailDraft.trim()}
        >
          邮箱密钥登录
        </button>
      </div>
      {status.signedIn ? (
        <>
          <label className="side-form-field account-code-field">
            <span>新本地密钥</span>
            <div className="account-code-row">
              <input
                value={newKeyDraft}
                onChange={(event) => accountAuth?.onNewKeyDraftChange?.(event.target.value)}
                placeholder="设置未来使用的新密钥"
                disabled={busy}
                type="password"
                autoComplete="new-password"
              />
              <button
                className="ghost-btn"
                type="button"
                onClick={accountAuth?.onRotateLocalKey}
                disabled={busy || !newKeyDraft.trim()}
              >
                设置
              </button>
            </div>
          </label>
          <button
            className="ghost-btn side-form-submit"
            type="button"
            onClick={accountAuth?.onClaimLocalData}
            disabled={busy}
          >
            认领本地密钥数据
          </button>
        </>
      ) : null}
      <label className="side-form-field account-code-field">
        <span>验证码</span>
        <div className="account-code-row">
          <input
            value={codeDraft}
            onChange={(event) => accountAuth?.onCodeDraftChange?.(event.target.value)}
            placeholder="6 位数字"
            disabled={busy}
            inputMode="numeric"
            autoComplete="one-time-code"
          />
          <button
            className="ghost-btn"
            type="button"
            onClick={accountAuth?.onVerifyEmailCode}
            disabled={busy || !codeDraft.trim()}
          >
            验证登录
          </button>
        </div>
      </label>
      {accountAuth?.message ? <p className="side-form-hint account-message">{accountAuth.message}</p> : null}
    </section>
  );
}

function LocalSecretPanel({
  secretDraft,
  selectedDataset,
  activeSecretCount,
  resolving,
  onSecretDraftChange,
  onResolveSecret,
  onBindSelectedDatasetSecret,
  onClearSecret,
}) {
  const bindLabel = selectedDataset?.visibility === 'private'
    ? '增加当前数据集密钥'
    : '绑定当前数据集为私密';

  return (
    <form
      className="side-card side-form"
      onSubmit={(event) => {
        event.preventDefault();
        onResolveSecret();
      }}
    >
      <div className="card-title">本地密钥</div>
      <label className="side-form-field">
        <span>当前终端密钥</span>
        <input
          value={secretDraft}
          onChange={(event) => onSecretDraftChange(event.target.value)}
          placeholder="输入密钥以解锁私密数据集"
          disabled={resolving}
          type="password"
        />
      </label>
      <button className="primary-btn side-form-submit" type="submit" disabled={resolving}>
        {resolving ? '解锁中...' : '解锁数据集'}
      </button>
      <button
        className="ghost-btn side-form-submit"
        type="button"
        onClick={onBindSelectedDatasetSecret}
        disabled={resolving || !selectedDataset}
      >
        {bindLabel}
      </button>
      <button className="ghost-btn side-form-submit" type="button" onClick={onClearSecret} disabled={resolving}>
        清除本地密钥
      </button>
      <p className="side-form-hint">
        已启用 {activeSecretCount} 个本地绑定。
        {selectedDataset ? ` 当前选中：${selectedDataset.title}。` : ' 选中数据集后可绑定为私密。'}
        密钥只缓存在当前浏览器。
      </p>
    </form>
  );
}

export default function Sidebar({
  datasets,
  selectedDatasetId,
  selectedDatasetIds = [],
  selectedDataset,
  selectedDatasets = [],
  datasetDraft,
  onDatasetDraftChange,
  onCreateDataset,
  localSecretDraft,
  activeSecretCount,
  resolvingSecret,
  onLocalSecretDraftChange,
  onResolveLocalSecret,
  onBindSelectedDatasetSecret,
  onClearLocalSecret,
  onSelectDataset,
  onClearDatasetSelection,
  creatingDataset,
  stats,
  loading,
  mobileOpen = false,
  onClose,
  scopePlan,
  accountAuth,
}) {
  const scopeCandidateIds = new Set(
    (scopePlan?.candidates || [])
      .filter((candidate) => candidate.type === 'dataset')
      .map((candidate) => candidate.id),
  );
  const selectedIdSet = new Set(selectedDatasetIds.length ? selectedDatasetIds : selectedDatasetId ? [selectedDatasetId] : []);

  return (
    <aside className={`sidebar ${mobileOpen ? 'open' : ''}`}>
      <div className="sidebar-mobile-head">
        <strong>数据集工作台</strong>
        <button type="button" className="ghost-btn compact-action-btn" onClick={onClose}>
          关闭
        </button>
      </div>

      <div className="brand">
        <div className="brand-logo">AI</div>
        <div>
          <h1>数据集</h1>
          <p>左侧只负责供料范围</p>
        </div>
        <DatasetCreateButton
          creating={creatingDataset}
          signedIn={Boolean(accountAuth?.statusSummary?.signedIn)}
          draft={datasetDraft}
          onDraftChange={onDatasetDraftChange}
          onCreate={onCreateDataset}
        />
      </div>

      <section className="side-card">
        <div className="card-title">可选数据集</div>
        <div className="dataset-list">
          <button
            type="button"
            className={`dataset-item ordinary-chat-item ${selectedIdSet.size ? '' : 'active'}`}
            onClick={onClearDatasetSelection}
            disabled={loading && !selectedIdSet.size}
          >
            <span className="dataset-item-title">普通聊天</span>
            <span className="dataset-item-meta">清空供料范围 · 命中资料意图后可预选</span>
          </button>
          {datasets.length ? (
            datasets.map((dataset) => {
              const active = selectedIdSet.has(dataset.id);
              const preselected = !active && scopeCandidateIds.has(dataset.id);
              return (
                <button
                  key={dataset.id}
                  type="button"
                  className={`dataset-item ${active ? 'active' : ''} ${preselected ? 'preselected' : ''}`.trim()}
                  onClick={() => onSelectDataset(dataset.id)}
                  disabled={loading && !active}
                >
                  <span className="dataset-item-title">
                    {dataset.title}
                    {active ? <small>已选</small> : preselected ? <small>预选</small> : null}
                  </span>
                  <span className="dataset-item-meta">
                    {dataset.key} · {dataset.visibility === 'private' ? '私密' : '公开'} · {dataset.lifecycle}
                  </span>
                </button>
              );
            })
          ) : (
            <div className="side-card-empty">
              还没有数据集。可以先普通聊天，也可以在上面创建一个公开数据集。
            </div>
          )}
        </div>
        {selectedDatasets.length ? (
          <p className="side-form-hint selected-scope-hint">
            已选 {selectedDatasets.length} 个供料范围：{selectedDatasets.map((dataset) => dataset.title).join('、')}。
          </p>
        ) : null}
      </section>
    </aside>
  );
}
