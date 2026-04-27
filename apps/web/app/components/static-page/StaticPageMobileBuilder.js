'use client';

import StaticPageMobileModuleList from './StaticPageMobileModuleList';
import StaticPageStyleDirectionPicker from './StaticPageStyleDirectionPicker';

const STYLE_LABELS = {
  'decision-brief': '高层决策简报',
  'client-delivery': '客户交付报告',
  'data-command': '数据运营看板',
};

export default function StaticPageMobileBuilder({
  draft,
  intent,
  onIntentChange,
  onSubmitIntent,
  onReorderModules,
  onChangeStyleDirection,
  onOneClick,
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
        <button type="button" className="primary-btn compact-action-btn" onClick={onOneClick}>
          一键出图
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

      <div className="static-page-mobile-model-summary">
        <span>模型理解</span>
        <p>{draft.modelSummary}</p>
      </div>

      <StaticPageStyleDirectionPicker
        compact
        value={draft.styleDirection}
        onChange={onChangeStyleDirection}
      />

      <div className="static-page-mobile-intent">
        <label htmlFor="static-page-mobile-intent">告诉模型怎么改</label>
        <textarea
          id="static-page-mobile-intent"
          value={intent}
          onChange={(event) => onIntentChange?.(event.target.value)}
          placeholder="例如：把风险放到前面，减少文字，整体更像给老板看的"
        />
        <button type="button" className="ghost-btn" onClick={onSubmitIntent} disabled={!intent?.trim()}>
          按意图刷新规划
        </button>
      </div>

      <div className="static-page-mobile-section-title">
        <strong>模块顺序</strong>
        <span>手机端只做上下排序，大小由桌面或模型处理。</span>
      </div>

      <StaticPageMobileModuleList draft={draft} onReorder={onReorderModules} />
    </section>
  );
}
