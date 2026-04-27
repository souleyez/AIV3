'use client';

import { useState } from 'react';
import ChatPanel from './ChatPanel';
import InsightPanel from './InsightPanel';
import Sidebar from './Sidebar';

export default function HomeMobileShell({
  sidebarProps,
  chatPanelProps,
  insightPanelProps,
  selectedDataset,
  stats,
  loading,
  banner,
  error,
}) {
  const [datasetOpen, setDatasetOpen] = useState(false);
  const [resultsOpen, setResultsOpen] = useState(false);

  return (
    <div className="mobile-home-shell">
      <header className="mobile-home-topbar">
        <button
          type="button"
          className="mobile-home-topbar-chip"
          onClick={() => setDatasetOpen(true)}
        >
          数据集
        </button>
        <strong className="mobile-home-topbar-brand">AI智能助手</strong>
        <button
          type="button"
          className="mobile-home-topbar-chip"
          onClick={() => setResultsOpen(true)}
          disabled={!selectedDataset}
        >
          结果
        </button>
      </header>

      <div className="mobile-home-status-strip">
        <span>{selectedDataset ? selectedDataset.title : '选择数据集后开始'}</span>
        <strong>{loading ? '同步中' : `会话 ${stats.sessions} · 报告 ${stats.plans}`}</strong>
      </div>

      {banner ? <div className="page-banner success-banner mobile-home-banner">{banner}</div> : null}
      {error ? <div className="page-banner error-banner mobile-home-banner">{error}</div> : null}

      <main className="mobile-home-stage">
        <ChatPanel
          {...chatPanelProps}
          panelClassName="chat-panel-mobile-home"
        />
      </main>

      <nav className="mobile-home-bottom-nav" aria-label="移动端工作区">
        <button type="button" onClick={() => setDatasetOpen(true)}>
          <span>数据集</span>
          <strong>{selectedDataset ? '已选' : '未选'}</strong>
        </button>
        <button type="button" className="active">
          <span>对话</span>
          <strong>当前</strong>
        </button>
        <button type="button" onClick={() => setResultsOpen(true)} disabled={!selectedDataset}>
          <span>结果</span>
          <strong>{stats.published}</strong>
        </button>
      </nav>

      {datasetOpen ? (
        <>
          <button
            type="button"
            className="mobile-drawer-backdrop mobile-home-overlay"
            aria-label="关闭数据集侧栏"
            onClick={() => setDatasetOpen(false)}
          />
          <Sidebar
            {...sidebarProps}
            mobileOpen
            onClose={() => setDatasetOpen(false)}
          />
        </>
      ) : null}

      {resultsOpen ? (
        <>
          <button
            type="button"
            className="mobile-drawer-backdrop mobile-home-overlay"
            aria-label="关闭结果侧栏"
            onClick={() => setResultsOpen(false)}
          />
          <aside className="mobile-home-results-drawer">
            <div className="mobile-home-drawer-head">
              <div>
                <strong>上下文 / 报告</strong>
                <span>{selectedDataset ? selectedDataset.title : '暂无数据集'}</span>
              </div>
              <button type="button" className="ghost-btn compact-action-btn" onClick={() => setResultsOpen(false)}>
                收起
              </button>
            </div>
            <InsightPanel {...insightPanelProps} />
          </aside>
        </>
      ) : null}
    </div>
  );
}
