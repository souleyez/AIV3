'use client';

import { buildStaticPageFinalRenderPayload } from '../../lib/static-page-draft';

const STYLE_LABELS = {
  'decision-brief': '高层决策简报',
  'client-delivery': '客户交付报告',
  'data-command': '数据运营看板',
};

function orderedModules(draft, compact) {
  if (!compact) {
    return [...draft.modules].sort((left, right) => {
      const yDiff = Number(left.layout?.y || 0) - Number(right.layout?.y || 0);
      if (yDiff !== 0) return yDiff;
      return Number(left.layout?.x || 0) - Number(right.layout?.x || 0);
    });
  }

  const byId = new Map(draft.modules.map((module) => [module.id, module]));
  return (draft.mobileOrder || []).map((id) => byId.get(id)).filter(Boolean);
}

function visualizationLabel(module) {
  return module.visualization?.label || module.visualization?.type || '图表占位';
}

export default function StaticPageFinalRender({
  draft,
  onApplyOperation,
  compact = false,
}) {
  const canRequestRender = draft?.status === 'effect_confirmed';
  const finalStatus = draft?.finalPage?.status || '';
  const hasFinalPage = ['mock_ready', 'rendered'].includes(finalStatus)
    || draft?.status === 'rendering'
    || draft?.status === 'rendered';
  const backendHtml = typeof draft?.finalPage?.html === 'string' ? draft.finalPage.html : '';
  const payload = draft ? buildStaticPageFinalRenderPayload(draft) : null;

  if (!draft) return null;

  if (!canRequestRender && !hasFinalPage) {
    return (
      <section className={`static-page-final-render locked${compact ? ' compact' : ''}`}>
        <div className="static-page-final-head">
          <span>最终静态页</span>
          <strong>等待效果图确认</strong>
        </div>
        <p>先确认效果图，再按效果制作可交付静态页。</p>
      </section>
    );
  }

  return (
    <section className={`static-page-final-render${compact ? ' compact' : ''}`}>
      <div className="static-page-final-head">
        <span>最终静态页</span>
        <strong>{finalStatus === 'rendered' ? '后端已生成' : hasFinalPage ? '本地模拟已生成' : '可生成'}</strong>
      </div>

      {!hasFinalPage ? (
        <button
          type="button"
          className="primary-btn compact-action-btn"
          onClick={() => onApplyOperation?.({ type: 'request_final_render' })}
        >
          按效果制作静态页
        </button>
      ) : null}

      {hasFinalPage ? (
        <>
          {backendHtml ? (
            <iframe
              className="static-page-final-frame"
              title="后端生成的静态页预览"
              srcDoc={backendHtml}
              sandbox=""
            />
          ) : (
            <div className={`static-page-final-sheet ${payload.styleDirection}`}>
              <div className="static-page-final-cover">
                <span>{STYLE_LABELS[payload.styleDirection] || payload.styleDirection}</span>
                <strong>{draft.objective}</strong>
                <p>{draft.modelSummary}</p>
              </div>
              <div className="static-page-final-modules">
                {orderedModules(draft, compact).map((module) => (
                  <article key={module.id} className="static-page-final-module">
                    <span>{visualizationLabel(module)}</span>
                    <strong>{module.title}</strong>
                    <p>{module.content}</p>
                    <em>{module.dataBinding?.label || '数据待绑定'}</em>
                  </article>
                ))}
              </div>
            </div>
          )}
          <p className="static-page-final-note">
            {finalStatus === 'rendered'
              ? '后端 renderer 已按确认效果图和模块规划生成静态页。'
              : '后端 renderer 尚未接入，当前结果来自前端 mock；真实数据图表会在数据绑定完成后渲染。'}
          </p>
        </>
      ) : null}
    </section>
  );
}
