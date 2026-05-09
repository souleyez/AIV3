'use client';

import { useState } from 'react';
import StaticPageEffectPreview from './StaticPageEffectPreview';
import StaticPageFinalRender from './StaticPageFinalRender';
import StaticPagePlanningCanvas from './StaticPagePlanningCanvas';
import StaticPageStyleDirectionPicker from './StaticPageStyleDirectionPicker';

const STYLE_LABELS = {
  'decision-brief': '高层决策简报',
  'client-delivery': '客户交付报告',
  'data-command': '数据运营看板',
};

const PREVIEW_STATUS_LABELS = {
  not_requested: '未生成',
  queued: '排队中',
  running: '生成中',
  preview_ready: '待确认',
  confirmed: '已确认',
  stale: '需重生成',
};

export default function StaticPagePlanningPanel({
  draft,
  onStartDraft,
  onApplyOperation,
  onApplyPrompt,
  onRetryWorkflow,
  onCancelWorkflow,
  onRefreshDraft,
}) {
  const [intent, setIntent] = useState('');

  function submitIntent() {
    const prompt = intent.trim();
    if (!prompt) return;
    onApplyPrompt?.(prompt);
    setIntent('');
  }

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

  const echartsModuleCount = (draft.modules || [])
    .filter((module) => module.visualization?.chartRuntime === 'echarts')
    .length;
  const deterministicModuleCount = Math.max(0, (draft.modules || []).length - echartsModuleCount);
  const previewStatus = draft.previewContract?.status || draft.imageJob?.status || 'not_requested';
  const finalStatus = draft.finalPage?.status || '未生成';

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
          <span>效果图</span>
          <strong>{PREVIEW_STATUS_LABELS[previewStatus] || previewStatus}</strong>
        </div>
        <div>
          <span>成品</span>
          <strong>{finalStatus}</strong>
        </div>
        <div>
          <span>图表运行</span>
          <strong>基础 {deterministicModuleCount} · ECharts {echartsModuleCount}</strong>
        </div>
      </div>

      <div className="static-page-model-summary">
        <span>模型理解</span>
        <p>{draft.modelSummary}</p>
      </div>

      <div className="static-page-intent-card">
        <label htmlFor="static-page-desktop-intent">告诉模型怎么改</label>
        <div className="static-page-intent-input">
          <textarea
            id="static-page-desktop-intent"
            value={intent}
            rows={3}
            placeholder="例如：把风险放到前面，趋势图换成柱状图，文字再短一点，整体更像给老板看的"
            onChange={(event) => setIntent(event.target.value)}
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
                event.preventDefault();
                submitIntent();
              }
            }}
          />
          <button
            type="button"
            className="primary-btn compact-action-btn"
            onClick={submitIntent}
            disabled={!intent.trim()}
          >
            按意图刷新规划
          </button>
        </div>
        <span>模型会先理解你的意图，再调整模块、图表、数据绑定和风格；直接编辑仍可随时微调。</span>
      </div>

      <StaticPageStyleDirectionPicker
        value={draft.styleDirection}
        onChange={(styleDirection) => onApplyOperation?.({ type: 'change_style_direction', styleDirection })}
      />

      <StaticPageEffectPreview draft={draft} onApplyOperation={onApplyOperation} />

      <StaticPageFinalRender
        draft={draft}
        onApplyOperation={onApplyOperation}
        onRetryWorkflow={onRetryWorkflow}
        onCancelWorkflow={onCancelWorkflow}
        onRefreshDraft={onRefreshDraft}
      />

      <StaticPagePlanningCanvas draft={draft} onApplyOperation={onApplyOperation} />

      <div className="static-page-planning-hint">
        <strong>桌面规划模式</strong>
        <span>拖动模块标题条调整位置，拖右下角调整大小；展开“微调模块”可改标题、内容、数据说明和图表。</span>
      </div>
    </div>
  );
}
