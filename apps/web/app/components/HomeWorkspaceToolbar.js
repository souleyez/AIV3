'use client';

const DESKTOP_NAV_LINKS = [
  { label: '智能会话', href: '/', active: true },
  { label: '数据集', href: '#datasets' },
  { label: '采集源', href: '#sources' },
  { label: '静态页', href: '#static-pages' },
  { label: '成员', href: '#members' },
  { label: '审计', href: '#audit' },
];

function StatusCount({ children, tone = 'neutral' }) {
  return <span className={`library-tab-count status-${tone}`}>{children}</span>;
}

export default function HomeWorkspaceToolbar({
  selectedDataset,
  stats,
  loading,
  workspaceLoading,
  sourceItems = [],
}) {
  const healthTone = loading || workspaceLoading ? 'warning' : 'healthy';
  const healthText = loading || workspaceLoading ? '同步中' : '正常';
  const connectedSourceCount = selectedDataset ? Math.max(sourceItems.length, 1) : 0;

  return (
    <header className="card home-toolbar">
      <div className="home-toolbar-left">
        <a href="/" className="home-toolbar-brand" aria-label="智能助手首页">
          <span className="home-toolbar-brand-mark">AI</span>
          <span className="home-toolbar-brand-name">智能助手</span>
        </a>
        <nav className="home-toolbar-nav" aria-label="桌面导航">
          {DESKTOP_NAV_LINKS.map((item) => (
            <a
              key={item.label}
              href={item.href}
              className={`home-toolbar-nav-link ${item.active ? 'active' : ''}`.trim()}
            >
              {item.label}
            </a>
          ))}
        </nav>
      </div>

      <div className="home-toolbar-right">
        <div className="home-toolbar-flyout">
          <button type="button" className="ghost-btn home-toolbar-flyout-trigger">
            系统健康
            <StatusCount tone={healthTone}>{healthText}</StatusCount>
          </button>
          <div className="home-toolbar-flyout-panel">
            <div className="home-toolbar-flyout-title">系统健康</div>
            <div className="home-toolbar-model-line">
              <strong>当前状态</strong>
              <span>{loading || workspaceLoading ? '正在同步 V3 工作区' : '运行正常'}</span>
            </div>
            <div className="home-toolbar-model-line">
              <strong>会话</strong>
              <span>{stats.sessions}</span>
            </div>
            <div className="home-toolbar-model-line">
              <strong>资料输出</strong>
              <span>{stats.outputs}</span>
            </div>
            <div className="home-toolbar-model-line">
              <strong>报告计划</strong>
              <span>{stats.plans}</span>
            </div>
          </div>
        </div>

        <div className="home-toolbar-flyout">
          <button type="button" className="ghost-btn home-toolbar-flyout-trigger">
            已连接数据源
            <StatusCount>{connectedSourceCount}</StatusCount>
          </button>
          <div className="home-toolbar-flyout-panel">
            <div className="home-toolbar-flyout-title">已连接数据源</div>
            {selectedDataset ? (
              <div className="home-toolbar-source-list">
                <div className="home-toolbar-source-item">
                  <span className="dot healthy"></span>
                  <span>{selectedDataset.title}</span>
                </div>
                <div className="home-toolbar-model-line">
                  <strong>Key</strong>
                  <span>{selectedDataset.key}</span>
                </div>
                <div className="home-toolbar-model-line">
                  <strong>生命周期</strong>
                  <span>{selectedDataset.lifecycle || 'active'}</span>
                </div>
              </div>
            ) : (
              <div className="home-toolbar-empty">先在左侧选择数据集。</div>
            )}
          </div>
        </div>

        <div className="home-toolbar-flyout">
          <button type="button" className="ghost-btn home-toolbar-flyout-trigger">
            模型连接
            <StatusCount tone="critical">未连通</StatusCount>
          </button>
          <div className="home-toolbar-flyout-panel">
            <div className="home-toolbar-flyout-title">模型连接</div>
            <div className="home-toolbar-model-warning status-critical">
              <strong>模型配置待接入</strong>
              <span>当前 V3 页面先保留原版入口样式，后续接入真实模型状态。</span>
            </div>
            <button type="button" className="ghost-btn compact-action-btn">
              输入密钥
            </button>
          </div>
        </div>
      </div>
    </header>
  );
}
