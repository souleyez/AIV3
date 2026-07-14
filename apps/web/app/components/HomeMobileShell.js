'use client';

import { useEffect, useState } from 'react';
import ChatPanel from './ChatPanel';
import InsightPanel from './InsightPanel';
import Sidebar from './Sidebar';
import StaticPageMobileBuilder from './static-page/StaticPageMobileBuilder';
import { formatRelativeTime, truncateText } from '../lib/formatters';

function MobileConversationMenu({
  open,
  sessions = [],
  selectedSessionId = '',
  currentConversationTitle = '当前对话',
  composingNewSession = false,
  onToggle,
  onSelectSession,
  onStartNewConversation,
}) {
  const activeConversationId = selectedSessionId || 'draft';
  const conversationOptions = [
    {
      id: 'draft',
      title: currentConversationTitle || '当前对话',
      meta: composingNewSession ? '当前草稿' : '本地对话',
    },
    ...sessions.map((session) => ({
      id: session.id,
      title: session.title || '未命名对话',
      meta: session.meta || (session.updated_at ? formatRelativeTime(session.updated_at) : '最近更新'),
    })),
  ];

  return (
    <div className="mobile-conversation-menu">
      <button
        type="button"
        className="mobile-home-conversation-trigger"
        onClick={onToggle}
        aria-expanded={open}
      >
        <span>当前对话</span>
        <strong>{truncateText(currentConversationTitle || '当前对话', 18)}</strong>
      </button>
      {open ? (
        <div className="mobile-top-dropdown mobile-conversation-dropdown">
          <div className="mobile-top-dropdown-head">
            <strong>对话列表</strong>
            <button type="button" className="ghost-btn compact-action-btn" onClick={onStartNewConversation}>
              新建
            </button>
          </div>
          <div className="mobile-conversation-list">
            {conversationOptions.map((session) => (
              <button
                key={session.id}
                type="button"
                className={`mobile-conversation-option ${session.id === activeConversationId ? 'active' : ''}`.trim()}
                onClick={() => {
                  onSelectSession?.(session.id);
                  onToggle?.(false);
                }}
              >
                <strong>{truncateText(session.title, 34)}</strong>
                <span>{session.meta}</span>
              </button>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function MobileAccountMenu({ open, accountAuth, onToggle }) {
  const status = accountAuth?.statusSummary || {
    label: '未登录',
    detail: '可普通聊天；私密数据集需要邮箱或本地密钥',
    signedIn: false,
  };
  const busy = Boolean(accountAuth?.busy);
  const emailDraft = accountAuth?.emailDraft || '';
  const codeDraft = accountAuth?.codeDraft || '';

  return (
    <div className="mobile-account-menu">
      <button
        type="button"
        className="mobile-home-topbar-chip mobile-login-chip"
        onClick={onToggle}
        aria-expanded={open}
      >
        {status.signedIn ? '账号' : '登录'}
      </button>
      {open ? (
        <div className="mobile-top-dropdown mobile-account-dropdown">
          <div className="mobile-account-summary">
            <strong>{status.label}</strong>
            <span>{status.detail}</span>
          </div>
          <label className="mobile-account-field">
            <span>邮箱</span>
            <input
              value={emailDraft}
              onChange={(event) => accountAuth?.onEmailDraftChange?.(event.target.value)}
              placeholder="登录邮箱"
              disabled={busy}
              type="email"
              autoComplete="email"
            />
          </label>
          <div className="mobile-account-actions">
            <button className="ghost-btn compact-action-btn" type="button" onClick={accountAuth?.onSendEmailCode} disabled={busy || !emailDraft.trim()}>
              发验证码
            </button>
            <button className="ghost-btn compact-action-btn" type="button" onClick={accountAuth?.onLoginWithKey} disabled={busy || !emailDraft.trim()}>
              密钥登录
            </button>
          </div>
          <label className="mobile-account-field">
            <span>验证码</span>
            <div className="mobile-account-code">
              <input
                value={codeDraft}
                onChange={(event) => accountAuth?.onCodeDraftChange?.(event.target.value)}
                placeholder="6 位数字"
                disabled={busy}
                inputMode="numeric"
                autoComplete="one-time-code"
              />
              <button className="primary-btn compact-action-btn" type="button" onClick={accountAuth?.onVerifyEmailCode} disabled={busy || !codeDraft.trim()}>
                验证
              </button>
            </div>
          </label>
          {status.signedIn ? (
            <button className="ghost-btn compact-action-btn" type="button" onClick={accountAuth?.onLogout} disabled={busy}>
              退出账号
            </button>
          ) : null}
          {accountAuth?.message ? <p className="mobile-account-message">{accountAuth.message}</p> : null}
        </div>
      ) : null}
    </div>
  );
}

export default function HomeMobileShell({
  sidebarProps,
  chatPanelProps,
  insightPanelProps,
  sessions = [],
  selectedSessionId = '',
  currentConversationTitle = '',
  composingNewSession = false,
  accountAuth,
  onSelectSession,
  onStartNewConversation,
  selectedDataset,
  selectedDatasets = [],
  stats,
  loading,
  banner,
  error,
  staticPageDraft,
  staticPageEditorOpen,
  onStaticPageEditorOpenChange,
  onApplyStaticPageOperation,
  onOpenDatasetUnderstanding,
}) {
  const [datasetOpen, setDatasetOpen] = useState(false);
  const [resultsOpen, setResultsOpen] = useState(false);
  const [conversationOpen, setConversationOpen] = useState(false);
  const [accountOpen, setAccountOpen] = useState(false);
  const [surface, setSurface] = useState('chat');
  const selectedScope = selectedDatasets.length ? selectedDatasets : selectedDataset ? [selectedDataset] : [];
  const selectedScopeLabel = selectedScope.map((dataset) => dataset.title || dataset.key).filter(Boolean).join('、');

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

  function handleRequestPreviewFromBuilder(operation) {
    if (operation) {
      onApplyStaticPageOperation?.(operation);
    } else {
      chatPanelProps.onStaticPagePrimaryAction?.();
    }
    onStaticPageEditorOpenChange?.(false);
    setSurface('chat');
  }

  const visibleHtmlArtifactCount = Array.isArray(insightPanelProps?.htmlArtifacts)
    ? insightPanelProps.htmlArtifacts.filter((artifact) => {
      const templateId = artifact?.templateId || artifact?.template_id;
      return templateId !== 'static_page_planning_handoff'
        && templateId !== 'static_page_data_quality_report';
    }).length
    : 0;
  const reportItemCount =
    (Number(stats.plans) || 0)
    + (Number(stats.published) || 0)
    + visibleHtmlArtifactCount
    + (Array.isArray(insightPanelProps?.staticPageDrafts) ? insightPanelProps.staticPageDrafts.length : 0);

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
          onClick={() => {
            setDatasetOpen(true);
            setConversationOpen(false);
            setAccountOpen(false);
          }}
        >
          数据集
        </button>
        <MobileConversationMenu
          open={conversationOpen}
          sessions={sessions}
          selectedSessionId={selectedSessionId}
          currentConversationTitle={currentConversationTitle}
          composingNewSession={composingNewSession}
          onToggle={(nextOpen) => {
            const resolved = typeof nextOpen === 'boolean' ? nextOpen : !conversationOpen;
            setConversationOpen(resolved);
            if (resolved) setAccountOpen(false);
          }}
          onSelectSession={onSelectSession}
          onStartNewConversation={onStartNewConversation}
        />
        <MobileAccountMenu
          open={accountOpen}
          accountAuth={accountAuth}
          onToggle={() => {
            setAccountOpen((current) => !current);
            setConversationOpen(false);
          }}
        />
      </header>

      <div className="mobile-home-status-strip">
        <span>{selectedScopeLabel || '普通聊天 · 未选数据集'}</span>
        {selectedScope.length ? (
          <button
            type="button"
            className="mobile-understanding-button"
            onClick={onOpenDatasetUnderstanding}
          >
            理解图谱
          </button>
        ) : null}
        <button
          type="button"
          className="mobile-report-summary-button"
          onClick={() => {
            setResultsOpen(true);
            setDatasetOpen(false);
            setConversationOpen(false);
            setAccountOpen(false);
          }}
        >
          {loading ? '同步中' : `会话 ${stats.sessions} · 报告 ${reportItemCount || stats.plans || 0}`}
        </button>
      </div>

      <main className={`mobile-home-stage mobile-home-stage-${surface}`}>
        {surface === 'static-page' ? (
          <StaticPageMobileBuilder
            draft={staticPageDraft}
            onStartDraft={handleStartStaticPageDraft}
            onRequestPreview={handleRequestPreviewFromBuilder}
            onBackToChat={handleBackToChat}
            busy={chatPanelProps.staticPageActionBusy}
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
                <strong>报告与产物</strong>
                <span>{selectedScopeLabel || '草稿、报告和静态页产物'}</span>
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
