'use client';

const PAGE_LINKS = [
  { key: 'home', label: '首页' },
  { key: 'datasets', label: '数据集' },
  { key: 'sources', label: '数据源' },
  { key: 'members', label: '成员' },
  { key: 'audit', label: '审计' },
];

function StatusCount({ children, tone = 'neutral' }) {
  return <span className={`library-tab-count status-${tone}`}>{children}</span>;
}

function ToolbarLine({ label, value }) {
  return (
    <div className="home-toolbar-model-line">
      <strong>{label}</strong>
      <span>{value}</span>
    </div>
  );
}

export default function HomeWorkspaceToolbar({
  activePage = 'home',
  onPageChange,
  selectedDataset,
  stats,
  loading,
  workspaceLoading,
  documents = [],
  accountAuth,
}) {
  const healthTone = loading || workspaceLoading ? 'warning' : 'healthy';
  const healthText = loading || workspaceLoading ? '同步中' : '正常';
  const accountStatus = accountAuth?.statusSummary || {
    label: '未登录',
    detail: '可普通聊天；私密数据集需要邮箱或本地密钥',
    signedIn: false,
  };
  const accountTone = accountStatus.signedIn ? 'healthy' : 'warning';
  const busy = Boolean(accountAuth?.busy);
  const emailDraft = accountAuth?.emailDraft || '';
  const codeDraft = accountAuth?.codeDraft || '';
  const newKeyDraft = accountAuth?.newKeyDraft || '';

  return (
    <header className="card home-toolbar">
      <div className="home-toolbar-left">
        <button
          type="button"
          className="home-toolbar-brand"
          aria-label="智能助手首页"
          onClick={() => onPageChange?.('home')}
        >
          <span className="home-toolbar-brand-mark">AI</span>
          <span className="home-toolbar-brand-name">智能助手</span>
        </button>
        <nav className="home-toolbar-nav" aria-label="主要页面目录">
          {PAGE_LINKS.map((item) => (
            <button
              key={item.key}
              type="button"
              className={`home-toolbar-nav-link ${activePage === item.key ? 'active' : ''}`.trim()}
              onClick={() => onPageChange?.(item.key)}
            >
              {item.label}
            </button>
          ))}
        </nav>
      </div>

      <div className="home-toolbar-right">
        <div className="home-toolbar-flyout">
          <button type="button" className="ghost-btn home-toolbar-flyout-trigger">
            系统状态
            <StatusCount tone={healthTone}>{healthText}</StatusCount>
          </button>
          <div className="home-toolbar-flyout-panel">
            <div className="home-toolbar-flyout-title">系统状态</div>
            <ToolbarLine label="工作区" value={loading || workspaceLoading ? '正在同步' : '运行正常'} />
            <ToolbarLine label="当前供料" value={selectedDataset?.title || '普通聊天'} />
            <ToolbarLine label="会话" value={stats.sessions} />
            <ToolbarLine label="资料输出" value={stats.outputs} />
            <ToolbarLine label="报告计划" value={stats.plans} />
            <ToolbarLine label="文档" value={documents.length} />
          </div>
        </div>

        <div className="home-toolbar-flyout">
          <button type="button" className="ghost-btn home-toolbar-flyout-trigger">
            登录状态
            <StatusCount tone={accountTone}>{accountStatus.signedIn ? '已登录' : '未登录'}</StatusCount>
          </button>
          <div className="home-toolbar-flyout-panel toolbar-account-panel">
            <div className="home-toolbar-flyout-title">登录状态</div>
            <div className="home-toolbar-model-warning toolbar-account-summary">
              <strong>{accountStatus.label}</strong>
              <span>{accountStatus.detail}</span>
            </div>
            <label className="toolbar-account-field">
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
            <div className="toolbar-account-actions">
              <button
                className="ghost-btn compact-action-btn"
                type="button"
                onClick={accountAuth?.onSendEmailCode}
                disabled={busy || !emailDraft.trim()}
              >
                发验证码
              </button>
              <button
                className="ghost-btn compact-action-btn"
                type="button"
                onClick={accountAuth?.onLoginWithKey}
                disabled={busy || !emailDraft.trim()}
              >
                密钥登录
              </button>
            </div>
            <label className="toolbar-account-field">
              <span>验证码</span>
              <div className="toolbar-account-code">
                <input
                  value={codeDraft}
                  onChange={(event) => accountAuth?.onCodeDraftChange?.(event.target.value)}
                  placeholder="6 位数字"
                  disabled={busy}
                  inputMode="numeric"
                  autoComplete="one-time-code"
                />
                <button
                  className="ghost-btn compact-action-btn"
                  type="button"
                  onClick={accountAuth?.onVerifyEmailCode}
                  disabled={busy || !codeDraft.trim()}
                >
                  验证
                </button>
              </div>
            </label>
            {accountStatus.signedIn ? (
              <>
                <label className="toolbar-account-field">
                  <span>新本地密钥</span>
                  <div className="toolbar-account-code">
                    <input
                      value={newKeyDraft}
                      onChange={(event) => accountAuth?.onNewKeyDraftChange?.(event.target.value)}
                      placeholder="设置新密钥"
                      disabled={busy}
                      type="password"
                      autoComplete="new-password"
                    />
                    <button
                      className="ghost-btn compact-action-btn"
                      type="button"
                      onClick={accountAuth?.onRotateLocalKey}
                      disabled={busy || !newKeyDraft.trim()}
                    >
                      设置
                    </button>
                  </div>
                </label>
                <div className="toolbar-account-actions">
                  <button className="ghost-btn compact-action-btn" type="button" onClick={accountAuth?.onClaimLocalData} disabled={busy}>
                    认领数据
                  </button>
                  <button className="ghost-btn compact-action-btn" type="button" onClick={accountAuth?.onLogout} disabled={busy}>
                    退出
                  </button>
                </div>
              </>
            ) : null}
            {accountAuth?.message ? <p className="toolbar-account-message">{accountAuth.message}</p> : null}
          </div>
        </div>

        <div className="home-toolbar-flyout">
          <button type="button" className="ghost-btn home-toolbar-flyout-trigger">
            模型代理
            <StatusCount tone="critical">待接入</StatusCount>
          </button>
          <div className="home-toolbar-flyout-panel">
            <div className="home-toolbar-flyout-title">模型代理</div>
            <div className="home-toolbar-model-warning status-critical">
              <strong>本地模型代理待联通</strong>
              <span>这里后续承载 Codex Host、MinMax、GPT Image 2 和其他模型 API 的统一状态。</span>
            </div>
            <ToolbarLine label="执行内核" value="Codex Host 规划中" />
            <ToolbarLine label="图像通道" value="Cloudflare 队列" />
          </div>
        </div>
      </div>
    </header>
  );
}
