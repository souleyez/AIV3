'use client';

import {
  buildStaticPageFinalRenderPayload,
  canRequestStaticPageDirectHtml,
  canRequestStaticPageFinalRender,
  staticPageDirectHtmlBlockReason,
  staticPageFinalRenderBlockReason,
} from '../../lib/static-page-draft';
import {
  buildStaticPageStandaloneHtml,
  dataQualityModulesFromManifest,
  dataQualitySummaryFromManifest,
  downloadStaticPageExportZip,
  downloadTextArtifact,
  downloadUrlArtifact,
  hasDataQualitySummary,
  staticPageHtmlFilename,
} from '../../lib/static-page-export-package';
import StaticPageChartPreview from './StaticPageChartPreview';

const STYLE_LABELS = {
  'decision-brief': '高层决策简报',
  'client-delivery': '客户交付报告',
  'data-command': '数据运营看板',
};

const FINAL_STATUS_LABELS = {
  queued: '后台排队',
  rendering: '后台生成中',
  rendered: '后端已生成',
  failed: '生成失败',
  cancelled: '已取消',
  mock_ready: '本地模拟',
};

const PENDING_FINAL_STATUSES = new Set(['queued', 'rendering']);

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

function finalPageStatus(draft) {
  if (draft?.finalPage?.status) {
    return draft.finalPage.status;
  }
  if (draft?.status === 'rendering') {
    return 'rendering';
  }
  if (draft?.status === 'rendered') {
    return 'rendered';
  }
  return '';
}

function finalPageManifest(draft) {
  return draft?.finalPage?.assetManifest && typeof draft.finalPage.assetManifest === 'object'
    ? draft.finalPage.assetManifest
    : {};
}

function workflowExecutionIdFromManifest(manifest) {
  return manifest?.workflow?.executionId
    || manifest?.workflow?.execution_id
    || manifest?.workflow?.workflowExecutionId
    || manifest?.workflow_execution_id
    || manifest?.workflowExecutionId
    || '';
}

function qualityStatusLabel(status) {
  if (status === 'confirmed') return '已确认';
  if (status === 'partial') return '部分确认';
  if (status === 'missing') return '缺失';
  return '待确认';
}

function chartRuntimeLabel(runtime) {
  return runtime === 'echarts' ? 'ECharts' : '基础静态';
}

function downloadStaticPageExportPackage(draft, payload, backendHtml) {
  if (!draft || !payload) {
    return;
  }
  downloadStaticPageExportZip(draft, payload, backendHtml);
}

function downloadStaticPageHtml(draft, backendHtml) {
  const downloadHref = draft?.finalPage?.htmlDownloadUrl || draft?.finalPage?.html_download_url || '';
  if (downloadHref && downloadUrlArtifact({ href: downloadHref })) {
    return;
  }
  const html = buildStaticPageStandaloneHtml(draft, backendHtml);
  if (!html) {
    return;
  }
  downloadTextArtifact({
    content: html,
    filename: staticPageHtmlFilename(draft),
    mime: 'text/html;charset=utf-8',
  });
}

function FinalRenderStatusCard({
  draft,
  finalStatus,
  workflowExecutionId,
  canRequestRender,
  onApplyOperation,
  onRetryWorkflow,
  onCancelWorkflow,
  onRefreshDraft,
}) {
  const manifest = finalPageManifest(draft);
  const pending = PENDING_FINAL_STATUSES.has(finalStatus);
  const failed = finalStatus === 'failed';
  const cancelled = finalStatus === 'cancelled';
  if (!pending && !failed && !cancelled) {
    return null;
  }

  const title = pending
    ? '资源正在排队制作'
    : failed
      ? '后台生成失败'
      : '已取消本次后台生成';
  const detail = pending
    ? manifest.queue_copy || '最终静态页正在后台制作，可以继续聊天。资源排队时可联系商务开通高级用户跳过等待。'
    : failed
      ? manifest.workflow?.lastError?.message || manifest.workflow?.lastError || '可以直接重试，或调整模块后重新生成。'
      : manifest.workflow?.cancelReason || '可以按当前可视化重新发起最终静态页生成。';

  return (
    <div className={`static-page-final-status-card ${failed ? 'failed' : cancelled ? 'cancelled' : 'pending'}`}>
      <div>
        <strong>{title}</strong>
        <p>{String(detail)}</p>
      </div>
      <div className="static-page-final-status-meta">
        <span>{FINAL_STATUS_LABELS[finalStatus] || finalStatus}</span>
        {workflowExecutionId ? <code>workflow {workflowExecutionId.slice(0, 8)}</code> : null}
      </div>
      <div className="static-page-final-actions">
        {pending && workflowExecutionId ? (
          <button type="button" className="ghost-btn compact-action-btn" onClick={() => onCancelWorkflow?.(workflowExecutionId)}>
            取消生成
          </button>
        ) : null}
        {failed && workflowExecutionId ? (
          <button type="button" className="ghost-btn compact-action-btn" onClick={() => onRetryWorkflow?.(workflowExecutionId)}>
            重试 workflow
          </button>
        ) : null}
        {(cancelled || failed) && canRequestRender ? (
          <button type="button" className="primary-btn compact-action-btn" onClick={() => onApplyOperation?.({ type: 'request_final_render' })}>
            重新生成
          </button>
        ) : null}
        <button type="button" className="ghost-btn compact-action-btn" onClick={() => onRefreshDraft?.(draft.backendDraftId)}>
          刷新状态
        </button>
      </div>
    </div>
  );
}

function FinalRenderDataQuality({ manifest }) {
  const summary = dataQualitySummaryFromManifest(manifest);
  const modules = dataQualityModulesFromManifest(manifest);
  if (!hasDataQualitySummary(summary)) {
    return null;
  }

  const attentionCount = summary.attentionModules || summary.partialModules + summary.missingModules;
  const readyLabel = attentionCount > 0 ? '仍有模块需要确认' : '数据已全部确认';
  const visibleModules = modules
    .filter((module) => attentionCount > 0
      ? module?.dataQualityStatus !== 'confirmed'
      : true)
    .slice(0, 4);
  return (
    <div className={`static-page-final-quality ${attentionCount > 0 ? 'attention' : 'ready'}`}>
      <div>
        <span>数据质量</span>
        <strong>{readyLabel}</strong>
      </div>
      <div className="static-page-final-quality-grid">
        <span>已确认 {summary.confirmedModules}</span>
        <span>部分 {summary.partialModules}</span>
        <span>缺失 {summary.missingModules}</span>
      </div>
      {visibleModules.length ? (
        <div className="static-page-final-quality-modules">
          {visibleModules.map((module) => (
            <span key={module.moduleId || module.title}>
              <b>{module.title || module.moduleId || '未命名模块'}</b>
              <em>
                {qualityStatusLabel(module.dataQualityStatus)} · {chartRuntimeLabel(module.chartRuntime)}
                {module.fallback ? ' · 静态回退' : ''}
              </em>
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export default function StaticPageFinalRender({
  draft,
  onApplyOperation,
  onRetryWorkflow,
  onCancelWorkflow,
  onRefreshDraft,
  compact = false,
}) {
  const canRequestRender = canRequestStaticPageFinalRender(draft);
  const blockReason = staticPageFinalRenderBlockReason(draft);
  const canRequestDirectHtml = canRequestStaticPageDirectHtml(draft);
  const directHtmlBlockReason = staticPageDirectHtmlBlockReason(draft);
  const finalStatus = finalPageStatus(draft);
  const hasFinalPage = ['mock_ready', 'rendered', 'queued', 'rendering', 'failed', 'cancelled'].includes(finalStatus)
    || draft?.status === 'rendering'
    || draft?.status === 'rendered';
  const manifest = finalPageManifest(draft);
  const workflowExecutionId = workflowExecutionIdFromManifest(manifest);
  const backendHtml = typeof draft?.finalPage?.html === 'string' ? draft.finalPage.html : '';
  const htmlDownloadUrl = draft?.finalPage?.htmlDownloadUrl || draft?.finalPage?.html_download_url || '';
  const payload = draft ? buildStaticPageFinalRenderPayload(draft) : null;
  const canShowRenderedPage = finalStatus === 'rendered' || finalStatus === 'mock_ready';
  const canDownloadHtml = finalStatus === 'rendered' && Boolean(backendHtml || htmlDownloadUrl);
  const canDownloadPackage = finalStatus === 'rendered';

  if (!draft) return null;

  if (!canRequestRender && !hasFinalPage) {
    return (
      <section className={`static-page-final-render locked${compact ? ' compact' : ''}`}>
        <div className="static-page-final-head">
          <span>最终静态页</span>
          <strong>等待可视化</strong>
        </div>
        <p>{blockReason || '先生成可视化，再按效果制作可交付静态页。'}</p>
        <button
          type="button"
          className="ghost-btn compact-action-btn"
          disabled={!canRequestDirectHtml}
          title={canRequestDirectHtml ? '' : directHtmlBlockReason}
          onClick={() => onApplyOperation?.({ type: 'request_final_render', directHtml: true })}
        >
          快速生成 HTML
        </button>
      </section>
    );
  }

  return (
    <section className={`static-page-final-render${compact ? ' compact' : ''}`}>
      <div className="static-page-final-head">
        <span>最终静态页</span>
        <strong>{FINAL_STATUS_LABELS[finalStatus] || (hasFinalPage ? '已提交' : '可生成')}</strong>
      </div>

      {!hasFinalPage ? (
        <div className="static-page-final-actions">
          <button
            type="button"
            className="primary-btn compact-action-btn"
            onClick={() => onApplyOperation?.({ type: 'request_final_render' })}
          >
            按效果制作静态页
          </button>
          <button
            type="button"
            className="ghost-btn compact-action-btn"
            disabled={!canRequestDirectHtml}
            title={canRequestDirectHtml ? '' : directHtmlBlockReason}
            onClick={() => onApplyOperation?.({ type: 'request_final_render', directHtml: true })}
          >
            快速生成 HTML
          </button>
        </div>
      ) : null}

      {hasFinalPage ? (
        <>
          <FinalRenderStatusCard
            draft={draft}
            finalStatus={finalStatus}
            workflowExecutionId={workflowExecutionId}
            canRequestRender={canRequestRender}
            onApplyOperation={onApplyOperation}
            onRetryWorkflow={onRetryWorkflow}
            onCancelWorkflow={onCancelWorkflow}
            onRefreshDraft={onRefreshDraft}
          />
          <FinalRenderDataQuality manifest={manifest} />

          {canShowRenderedPage && backendHtml ? (
            <iframe
              className="static-page-final-frame"
              title="后端生成的静态页预览"
              srcDoc={backendHtml}
              sandbox=""
            />
          ) : canShowRenderedPage ? (
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
                    <StaticPageChartPreview module={module} compact={compact} />
                  </article>
                ))}
              </div>
            </div>
          ) : null}

          {canDownloadPackage ? (
            <div className="static-page-final-actions">
              {canDownloadHtml ? (
                <button
                  type="button"
                  className="ghost-btn compact-action-btn"
                  onClick={() => downloadStaticPageHtml(draft, backendHtml)}
                >
                  下载 index.html
                </button>
              ) : null}
              <button
                type="button"
                className="ghost-btn compact-action-btn"
                onClick={() => downloadStaticPageExportPackage(draft, payload, backendHtml)}
              >
                下载交付包 ZIP
              </button>
            </div>
          ) : null}

          <p className="static-page-final-note">
            {finalStatus === 'rendered'
              ? draft.finalPage?.directHtml
                ? '后端 renderer 已按快速 HTML 模式生成静态页。'
                : '后端 renderer 已按可视化和模块规划生成静态页。'
              : PENDING_FINAL_STATUSES.has(finalStatus)
                ? '后台生成不会阻塞当前对话；完成后会在右侧成品栏保留。'
                : finalStatus === 'mock_ready'
                  ? '当前结果来自前端 mock；真实数据图表会在后端渲染完成后替换。'
                  : '可刷新状态、重试 workflow，或按当前规划重新发起最终生成。'}
          </p>
        </>
      ) : null}
    </section>
  );
}
