'use client';

import StaticPagePlanningCanvas from './StaticPagePlanningCanvas';

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
      <StaticPagePlanningCanvas draft={draft} onApplyOperation={onApplyOperation} />

      <div className="static-page-planning-hint">
        <strong>桌面规划模式</strong>
        <span>这里只保留可拖动框架和模块编辑。要让模型整体改版，直接在底部对话框输入要求。</span>
      </div>
    </div>
  );
}
