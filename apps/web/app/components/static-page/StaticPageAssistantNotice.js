'use client';

export default function StaticPageAssistantNotice({
  draft,
  onOpenBuilder,
  onOneClick,
}) {
  if (!draft) {
    return null;
  }

  return (
    <div className="static-page-assistant-notice" role="note">
      <div className="static-page-notice-copy">
        <span>静态页草稿</span>
        <strong>{draft.objective}</strong>
        <p>{draft.modelSummary}</p>
      </div>
      <div className="static-page-notice-meta">
        <span>{draft.modules.length} 个模块</span>
        <span>{draft.styleDirection}</span>
        <span>{draft.status}</span>
      </div>
      <div className="static-page-notice-actions">
        <button type="button" className="ghost-btn compact-action-btn" onClick={onOpenBuilder}>
          查看规划
        </button>
        <button type="button" className="primary-btn compact-action-btn" onClick={onOneClick}>
          一键生成
        </button>
      </div>
    </div>
  );
}
