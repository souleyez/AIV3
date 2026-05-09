'use client';

import StaticPageMobileModuleList from './StaticPageMobileModuleList';

const STYLE_LABELS = {
  'decision-brief': '高层决策简报',
  'client-delivery': '客户交付报告',
  'data-command': '数据运营看板',
};

export default function StaticPageMobileBuilder({
  draft,
  onApplyOperation,
  onReorderModules,
  onRequestPreview,
  onBackToChat,
}) {
  if (!draft) {
    return (
      <section className="static-page-mobile-builder empty">
        <div className="static-page-mobile-builder-head">
          <button type="button" className="ghost-btn compact-action-btn" onClick={onBackToChat}>
            返回
          </button>
          <strong>静态页构建</strong>
        </div>
        <div className="static-page-mobile-empty">
          <strong>还没有静态页草稿</strong>
          <p>回到对话，说“生成静态页”，或点击一键生成后会在这里构建。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="static-page-mobile-builder">
      <div className="static-page-mobile-builder-head">
        <button type="button" className="ghost-btn compact-action-btn" onClick={onBackToChat}>
          返回对话
        </button>
        <strong>静态页构建</strong>
        <button type="button" className="primary-btn compact-action-btn" onClick={onRequestPreview}>
          效果图
        </button>
      </div>

      <div className="static-page-mobile-preview">
        <span>{STYLE_LABELS[draft.styleDirection] || draft.styleDirection}</span>
        <strong>{draft.objective}</strong>
        <div className="static-page-mobile-preview-bars">
          {draft.modules.map((module) => (
            <i key={module.id} style={{ width: `${Math.max(36, Math.min(100, (module.layout?.w || 4) * 8))}%` }} />
          ))}
        </div>
      </div>

      <div className="static-page-mobile-section-title">
        <strong>模块顺序</strong>
        <span>手机端支持上下排序，也可展开模块微调标题、内容、数据和图表；整体修改继续在底部对话框输入。</span>
      </div>

      <StaticPageMobileModuleList
        draft={draft}
        onReorder={onReorderModules}
        onApplyOperation={onApplyOperation}
      />
    </section>
  );
}
