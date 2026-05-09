'use client';

function DatasetCreateForm({
  draft,
  creating,
  onChange,
  onSubmit,
}) {
  return (
    <form
      className="side-card side-form"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <div className="card-title">新建数据集</div>
      <label className="side-form-field">
        <span>Key</span>
        <input
          value={draft.key}
          onChange={(event) => onChange('key', event.target.value)}
          placeholder="例如 sales_briefing"
          disabled={creating}
        />
      </label>
      <label className="side-form-field">
        <span>标题</span>
        <input
          value={draft.title}
          onChange={(event) => onChange('title', event.target.value)}
          placeholder="例如 销售简报知识集"
          disabled={creating}
        />
      </label>
      <label className="side-form-field">
        <span>本地密钥</span>
        <input
          value={draft.secret || ''}
          onChange={(event) => onChange('secret', event.target.value)}
          placeholder="可选，填写后创建私密数据集"
          disabled={creating}
          type="password"
        />
      </label>
      <button className="primary-btn side-form-submit" type="submit" disabled={creating}>
        {creating ? '创建中...' : '创建并切换'}
      </button>
    </form>
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
  selectedDataset,
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
      </div>

      <DatasetCreateForm
        draft={datasetDraft}
        creating={creatingDataset}
        onChange={onDatasetDraftChange}
        onSubmit={onCreateDataset}
      />

      <section className="side-card">
        <div className="card-title">可选数据集</div>
        <div className="dataset-list">
          <button
            type="button"
            className={`dataset-item ordinary-chat-item ${selectedDatasetId ? '' : 'active'}`}
            onClick={onClearDatasetSelection}
            disabled={loading && !selectedDatasetId}
          >
            <span className="dataset-item-title">普通聊天</span>
            <span className="dataset-item-meta">未锁定数据集 · 命中资料意图后预选</span>
          </button>
          {datasets.length ? (
            datasets.map((dataset) => {
              const active = dataset.id === selectedDatasetId;
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
                    {preselected ? <small>预选</small> : null}
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
      </section>
    </aside>
  );
}
