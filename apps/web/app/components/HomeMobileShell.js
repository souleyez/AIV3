'use client';

import { useEffect, useState } from 'react';
import ChatPanel from './ChatPanel';
import InsightPanel from './InsightPanel';
import Sidebar from './Sidebar';
import StaticPageMobileBuilder from './static-page/StaticPageMobileBuilder';

export default function HomeMobileShell({
  sidebarProps,
  chatPanelProps,
  insightPanelProps,
  selectedDataset,
  stats,
  loading,
  banner,
  error,
  staticPageDraft,
  staticPageEditorOpen,
  onStaticPageEditorOpenChange,
  onApplyStaticPageOperation,
}) {
  const [datasetOpen, setDatasetOpen] = useState(false);
  const [resultsOpen, setResultsOpen] = useState(false);
  const [surface, setSurface] = useState('chat');

  function handleStartStaticPageDraft(options = {}) {
    const draft = chatPanelProps.onStartStaticPageDraft?.(options);
    onStaticPageEditorOpenChange?.(true);
    setSurface('static-page');
    return draft;
  }

  function handleBackToChat() {
    onStaticPageEditorOpenChange?.(false);
    setSurface('chat');
  }

  function handleRequestPreviewFromBuilder() {
    chatPanelProps.onStaticPagePrimaryAction?.();
    onStaticPageEditorOpenChange?.(false);
    setSurface('chat');
  }

  useEffect(() => {
    if (staticPageEditorOpen && staticPageDraft && surface !== 'static-page') {
      setSurface('static-page');
    }
  }, [staticPageDraft, staticPageEditorOpen, surface]);

  useEffect(() => {
    if (!staticPageEditorOpen && surface === 'static-page' && staticPageDraft?.imageJob?.status === 'preview_ready') {
      setSurface('chat');
    }
  }, [staticPageDraft?.imageJob?.status, staticPageEditorOpen, surface]);

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
        <span>{selectedDataset ? selectedDataset.title : '普通聊天 · 未选数据集'}</span>
        <strong>{loading ? '同步中' : `会话 ${stats.sessions} · 报告 ${stats.plans}`}</strong>
      </div>

      {banner ? <div className="page-banner success-banner mobile-home-banner">{banner}</div> : null}
      {error ? <div className="page-banner error-banner mobile-home-banner">{error}</div> : null}

      <main className={`mobile-home-stage mobile-home-stage-${surface}`}>
        {surface === 'static-page' ? (
          <StaticPageMobileBuilder
            draft={staticPageDraft}
            onApplyOperation={onApplyStaticPageOperation}
            onReorderModules={(order) => onApplyStaticPageOperation?.({ type: 'reorder_modules', order })}
            onRequestPreview={handleRequestPreviewFromBuilder}
            onBackToChat={handleBackToChat}
          />
        ) : (
          <ChatPanel
            {...chatPanelProps}
            panelClassName="chat-panel-mobile-home"
            onStartStaticPageDraft={handleStartStaticPageDraft}
            onOpenStaticPageBuilder={() => {
              onStaticPageEditorOpenChange?.(true);
              setSurface('static-page');
            }}
            showStaticPageWorkspace={false}
          />
        )}
      </main>

      <nav className="mobile-home-bottom-nav" aria-label="移动端工作区">
        <button type="button" onClick={() => setDatasetOpen(true)}>
          <span>数据集</span>
          <strong>{selectedDataset ? '已选' : '未选'}</strong>
        </button>
        <button type="button" className={surface === 'chat' ? 'active' : ''} onClick={() => setSurface('chat')}>
          <span>对话</span>
          <strong>{surface === 'chat' ? '当前' : '返回'}</strong>
        </button>
        <button
          type="button"
          className={surface === 'static-page' ? 'active' : ''}
          onClick={() => {
            onStaticPageEditorOpenChange?.(true);
            setSurface('static-page');
          }}
          disabled={!staticPageDraft}
        >
          <span>静态页</span>
          <strong>{staticPageDraft ? '构建' : '待生成'}</strong>
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
