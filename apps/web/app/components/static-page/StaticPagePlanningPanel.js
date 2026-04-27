'use client';

import StaticPageEffectPreview from './StaticPageEffectPreview';
import StaticPageFinalRender from './StaticPageFinalRender';
import StaticPagePlanningCanvas from './StaticPagePlanningCanvas';
import StaticPageStyleDirectionPicker from './StaticPageStyleDirectionPicker';

const STYLE_LABELS = {
  'decision-brief': '高层决策简报',
  'client-delivery': '客户交付报告',
  'data-command': '数据运营看板',
};

export default function StaticPagePlanningPanel({
  draft,
  onStartDraft,
  onApplyOperation,
}) {
  if (!draft) {
    return (
      <div className="static-page-planning-empty">
        <strong>还没有静态页规划</strong>
        <p>从对话里说“生成静态页”，或直接点下面按钮，系统会先生成一版模块规划。</p>
        <button type="button" className="primary-btn" onClick={() => onStartDraft?.({ oneClick: false })}>
          开始规划静态页
        </button>
      </div>
    );
  }

  return (
    <div className="static-page-planning-panel">
      <div className="static-page-planning-summary">
        <div>
          <span>目标</span>
          <strong>{draft.objective}</strong>
        </div>
        <div>
          <span>风格</span>
          <strong>{STYLE_LABELS[draft.styleDirection] || draft.styleDirection}</strong>
        </div>
        <div>
          <span>状态</span>
          <strong>{draft.status}</strong>
        </div>
      </div>

      <div className="static-page-model-summary">
        <span>模型理解</span>
        <p>{draft.modelSummary}</p>
      </div>

      <StaticPageStyleDirectionPicker
        value={draft.styleDirection}
        onChange={(styleDirection) => onApplyOperation?.({ type: 'change_style_direction', styleDirection })}
      />

      <StaticPageEffectPreview draft={draft} onApplyOperation={onApplyOperation} />

      <StaticPageFinalRender draft={draft} onApplyOperation={onApplyOperation} />

      <StaticPagePlanningCanvas draft={draft} onApplyOperation={onApplyOperation} />

      <div className="static-page-planning-hint">
        <strong>桌面规划模式</strong>
        <span>拖动模块标题条调整位置，拖右下角调整大小；手机端后续会使用纵向构建模式。</span>
      </div>
    </div>
  );
}
