'use client';

import { formatDateTime, formatRelativeTime, formatSnakeCaseLabel, truncateText } from '../lib/formatters';
import {
  buildStaticPageFinalRenderPayload,
  staticPageFinalRenderBlockReason,
} from '../lib/static-page-draft';
import {
  buildStaticPageStandaloneHtml,
  dataQualityModulesFromManifest,
  dataQualitySummaryFromManifest,
  downloadStaticPageExportZip,
  downloadTextArtifact,
  hasDataQualitySummary,
  staticPageHtmlFilename,
} from '../lib/static-page-export-package';
import { normalizeHtmlArtifactManifest } from '../lib/html-artifact-manifest';

const SURFACE_LABELS = {
  pc: 'PC',
  mobile: 'Mobile',
};

const STATIC_PAGE_STATUS_LABELS = {
  planning: '规划中',
  queued: '效果图排队',
  preview_ready: '待确认效果图',
  effect_confirmed: '效果图已确认',
  rendering: '生成中',
  failed: '生成失败',
  cancelled: '已取消',
  rendered: '已生成',
  draft: '草稿',
  planned: '已规划',
  confirmed: '已确认',
  stale: '规划已变更',
};

function SectionHeader({ title, subtitle }) {
  return (
    <div className="insight-section-head">
      <h4>{title}</h4>
      <span>{subtitle}</span>
    </div>
  );
}

function EmptySection({ text }) {
  return <div className="insight-empty">{text}</div>;
}

function manifestAssets(manifest) {
  return Array.isArray(manifest?.assets) ? manifest.assets : [];
}

function firstAssetPath(manifest) {
  const assets = manifestAssets(manifest);
  const htmlAsset = assets.find((asset) => asset?.kind === 'html' && asset?.path);
  return htmlAsset?.path || assets.find((asset) => asset?.path)?.path || manifest?.path || manifest?.manifest_key || '';
}

function hasPublishableAsset(output) {
  return output?.status === 'rendered' && Boolean(firstAssetPath(output.asset_manifest));
}

function latestOutputForSurface(renderOutputs, surface) {
  return renderOutputs.find((output) => output.surface === surface) || null;
}

function canContinuePlan(plan) {
  return plan?.status === 'draft' && !plan.current_ast_version_id;
}

function canRenderPlan(plan) {
  return Boolean(plan?.current_ast_version_id);
}

function staticPageStatusLabel(draft) {
  if (staticPageIsPreviewStale(draft)) {
    return STATIC_PAGE_STATUS_LABELS.stale;
  }
  const renderStatus = draft?.finalPage?.status;
  return STATIC_PAGE_STATUS_LABELS[renderStatus]
    || STATIC_PAGE_STATUS_LABELS[draft?.status]
    || STATIC_PAGE_STATUS_LABELS[draft?.backendStatus]
    || formatSnakeCaseLabel(renderStatus || draft?.status || draft?.backendStatus || 'draft');
}

function staticPageUpdatedAt(draft) {
  return draft?.backendUpdatedAt || draft?.updated_at || draft?.updatedAt || draft?.created_at || '';
}

function staticPageWorkflowExecutionId(draft) {
  const manifest = draft?.finalPage?.assetManifest || {};
  return manifest?.workflow?.executionId
    || manifest?.workflow?.execution_id
    || manifest?.workflow?.workflowExecutionId
    || manifest?.workflow_execution_id
    || manifest?.workflowExecutionId
    || '';
}

function staticPageDataQualitySummary(draft) {
  return dataQualitySummaryFromManifest(draft?.finalPage?.assetManifest || {});
}

function staticPageDataQualityModules(draft) {
  return dataQualityModulesFromManifest(draft?.finalPage?.assetManifest || {});
}

function staticPageQualityStatusLabel(status) {
  if (status === 'confirmed') return '已确认';
  if (status === 'partial') return '部分';
  if (status === 'missing') return '缺失';
  return '待确认';
}

function staticPageChartRuntimeLabel(runtime) {
  return runtime === 'echarts' ? 'ECharts' : '静态';
}

function staticPageRenderAction(draft) {
  if (staticPageIsPreviewStale(draft)) {
    return null;
  }
  const status = draft?.finalPage?.status || '';
  const executionId = staticPageWorkflowExecutionId(draft);
  if (!executionId) {
    return null;
  }
  if (status === 'failed') {
    return { label: '重试', kind: 'retry', executionId };
  }
  if (status === 'queued' || status === 'rendering') {
    return { label: '取消', kind: 'cancel', executionId };
  }
  return null;
}

function staticPageIsRendered(draft) {
  return !staticPageIsPreviewStale(draft) && (draft?.finalPage?.status === 'rendered' || draft?.status === 'rendered');
}

function staticPageIsPreviewStale(draft) {
  return draft?.previewContract?.status === 'stale' || draft?.imageJob?.status === 'stale';
}

function downloadStaticPageHtmlFromShelf(draft) {
  const html = buildStaticPageStandaloneHtml(draft, draft?.finalPage?.html || '');
  if (!html) return;
  downloadTextArtifact({
    content: html,
    filename: staticPageHtmlFilename(draft),
    mime: 'text/html;charset=utf-8',
  });
}

function downloadStaticPagePackageFromShelf(draft) {
  const payload = buildStaticPageFinalRenderPayload(draft);
  const html = buildStaticPageStandaloneHtml(draft, draft?.finalPage?.html || '');
  downloadStaticPageExportZip(draft, payload, html);
}

function htmlArtifactSummary(artifact) {
  const result = normalizeHtmlArtifactManifest(artifact);
  if (result.rejected) {
    return {
      id: result.sourceId || artifact?.id || 'rejected-html-artifact',
      rejected: true,
      title: artifact?.title || '已拦截 HTML 产物',
      subtitle: result.reason || 'manifest 未通过安全规则',
      meta: 'blocked',
      createdAt: artifact?.createdAt || artifact?.created_at || '',
    };
  }
  return {
    id: result.manifest.id,
    rejected: false,
    title: result.manifest.title,
    subtitle: result.manifest.provenance.reason || result.manifest.templateLabel,
    meta: `${result.manifest.templateLabel} · ${result.manifest.interactionMode}`,
    sourceLabel: result.manifest.sourceLabel,
    createdAt: result.manifest.createdAt,
  };
}

function buildReportControlNotice({
  plan,
  surface,
  latestSurfaceOutput,
  latestPublishableOutput,
  publishedDetail,
  actionBusy,
}) {
  if (!plan) return null;

  if (actionBusy) {
    return {
      tone: 'info',
      title: 'Host action 正在执行',
      detail: '控制台会自动轮询详情；如果 worker 写回了新 AST、render output 或发布版本，这里会同步刷新。',
    };
  }

  if (!plan.current_ast_version_id) {
    return {
      tone: 'warn',
      title: '还不能渲染',
      detail: '当前 report plan 还没有 AST 版本，先继续规划，让 worker 写回可渲染结构。',
    };
  }

  if (latestSurfaceOutput?.status === 'failed') {
    return {
      tone: 'danger',
      title: `${SURFACE_LABELS[surface] || surface} 最近一次渲染失败`,
      detail: `可直接重试 workflow ${truncateText(latestSurfaceOutput.execution_id, 18)}，或进入 runtime.inspect 查看失败细节。`,
    };
  }

  if (latestSurfaceOutput?.status === 'rendered' && !firstAssetPath(latestSurfaceOutput.asset_manifest)) {
    return {
      tone: 'warn',
      title: '渲染已完成但缺少资产路径',
      detail: '这个 render output 不能发布。需要重试渲染，或检查 report-render-worker 的 asset manifest 写回。',
    };
  }

  if (latestPublishableOutput) {
    const currentVersion = publishedDetail?.current_version;
    return {
      tone: currentVersion ? 'success' : 'ready',
      title: currentVersion ? '已有发布版本' : '可以发布',
      detail: currentVersion
        ? `当前版本是 v${currentVersion.version_no}；如需覆盖交付状态，可以发布新的 ${SURFACE_LABELS[surface] || surface} 版本。`
        : `已有可发布的 ${SURFACE_LABELS[surface] || surface} 渲染资产，确认后可发布为 published report。`,
    };
  }

  return {
    tone: 'info',
    title: '可以启动渲染',
    detail: `AST 已就绪，但当前还没有可发布的 ${SURFACE_LABELS[surface] || surface} render output。`,
  };
}

function ReportPlanDetail({
  plan,
  renderOutputs,
  astVersions,
  publishedDetail,
  loading,
  actionBusy,
  surface,
  publishNote,
  onSurfaceChange,
  onPublishNoteChange,
  onContinue,
  onRender,
  onPublish,
  onRetryWorkflowExecution,
  onRefresh,
}) {
  if (!plan) {
    return <EmptySection text="选择一个 report plan 后，这里会显示 AST、渲染输出和发布版本。" />;
  }

  const latestSurfaceOutput = latestOutputForSurface(renderOutputs, surface);
  const latestPublishableOutput = renderOutputs.find(
    (output) => output.surface === surface && hasPublishableAsset(output),
  );
  const notice = buildReportControlNotice({
    plan,
    surface,
    latestSurfaceOutput,
    latestPublishableOutput,
    publishedDetail,
    actionBusy,
  });
  const disableActions = Boolean(actionBusy);

  return (
    <div className="report-control">
      <div className="report-control-grid">
        <div>
          <span>Plan</span>
          <strong>{truncateText(plan.id, 18)}</strong>
        </div>
        <div>
          <span>Status</span>
          <strong>{formatSnakeCaseLabel(plan.status)}</strong>
        </div>
        <div>
          <span>AST</span>
          <strong>{plan.current_ast_version_id ? truncateText(plan.current_ast_version_id, 18) : '未生成'}</strong>
        </div>
        <div>
          <span>Recommended</span>
          <strong>{plan.model_facing?.recommended_tool_key || 'report.plan'}</strong>
        </div>
      </div>

      <div className="report-objective-box">
        <span>目标</span>
        <p>{plan.objective}</p>
      </div>

      <div className="surface-toggle">
        {['pc', 'mobile'].map((item) => (
          <button
            key={item}
            type="button"
            className={surface === item ? 'active' : ''}
            onClick={() => onSurfaceChange(item)}
          >
            {SURFACE_LABELS[item]}
          </button>
        ))}
      </div>

      {notice ? (
        <div className={`report-status-callout ${notice.tone}`.trim()}>
          <strong>{notice.title}</strong>
          <span>{notice.detail}</span>
        </div>
      ) : null}

      <div className="insight-action-row">
        <button
          type="button"
          className="ghost-btn"
          disabled={disableActions || !canContinuePlan(plan)}
          onClick={onContinue}
        >
          {actionBusy === 'continue' ? '规划中...' : '继续规划'}
        </button>
        <button
          type="button"
          className="primary-btn"
          disabled={disableActions || !canRenderPlan(plan)}
          onClick={onRender}
        >
          {actionBusy === 'render' ? '渲染中...' : `渲染 ${SURFACE_LABELS[surface]}`}
        </button>
        <button type="button" className="ghost-btn" disabled={loading || disableActions} onClick={onRefresh}>
          {loading ? '刷新中...' : '刷新详情'}
        </button>
      </div>

      <div className="publish-box">
        <input
          value={publishNote}
          onChange={(event) => onPublishNoteChange(event.target.value)}
          placeholder="发布备注，可留空"
          disabled={disableActions}
        />
        <button
          type="button"
          className="primary-btn"
          disabled={disableActions || !latestPublishableOutput}
          onClick={onPublish}
        >
          {actionBusy === 'publish' ? '发布中...' : `发布 ${SURFACE_LABELS[surface]}`}
        </button>
      </div>
      {!latestPublishableOutput ? (
        <p className="report-hint">当前 surface 还没有可发布的 rendered output，先启动渲染并等待 worker 写回。</p>
      ) : null}

      <div className="report-detail-columns">
        <div className="report-detail-block">
          <strong>AST 版本</strong>
          {astVersions.length ? (
            <div className="mini-list">
              {astVersions.map((version) => (
                <div className="mini-row" key={version.id}>
                  <span>v{version.version_no}</span>
                  <em>{formatDateTime(version.created_at)}</em>
                </div>
              ))}
            </div>
          ) : (
            <EmptySection text="暂无 AST version。" />
          )}
        </div>

        <div className="report-detail-block">
          <strong>渲染输出</strong>
          {renderOutputs.length ? (
            <div className="mini-list">
              {renderOutputs.map((output) => {
                const assetPath = firstAssetPath(output.asset_manifest);
                const failed = output.status === 'failed';
                const retryBusy = actionBusy === `retry:${output.execution_id}`;
                return (
                  <div className={`mini-row multi report-output-row ${failed ? 'failed' : ''}`.trim()} key={output.id}>
                    <div className="report-output-copy">
                      <span>
                        {SURFACE_LABELS[output.surface] || output.surface} · {formatSnakeCaseLabel(output.status)}
                      </span>
                      <em>{truncateText(assetPath || output.id, 42)}</em>
                      <small>
                        {output.model_facing?.recommended_tool_key || (failed ? 'workflow.retry' : 'report.publish')}
                        {' · '}
                        workflow {truncateText(output.execution_id, 14)}
                      </small>
                    </div>
                    {failed ? (
                      <button
                        type="button"
                        className="ghost-btn compact-action-btn report-row-action"
                        disabled={disableActions}
                        onClick={() => onRetryWorkflowExecution(output.execution_id)}
                      >
                        {retryBusy ? '重试中...' : '重试'}
                      </button>
                    ) : null}
                  </div>
                );
              })}
            </div>
          ) : (
            <EmptySection text="暂无 render output。" />
          )}
        </div>
      </div>

      <div className="report-detail-block">
        <strong>发布版本</strong>
        {publishedDetail?.versions?.length ? (
          <div className="mini-list">
            {publishedDetail.versions.map((version) => (
              <div className="mini-row multi" key={version.id}>
                <span>
                  v{version.version_no} · {SURFACE_LABELS[version.surface] || version.surface}
                </span>
                <em>{truncateText(firstAssetPath(version.asset_manifest) || version.id, 54)}</em>
              </div>
            ))}
          </div>
        ) : (
          <EmptySection text="这个 plan 还没有 published report。" />
        )}
      </div>
    </div>
  );
}

function staticPageStageRows(draft) {
  const previewStatus = draft?.previewContract?.status || draft?.imageJob?.status || 'not_requested';
  const finalStatus = draft?.finalPage?.status || draft?.status || 'draft';
  return [
    ['需求理解', draft ? '已建立目标' : '等待用户提出页面需求'],
    ['模块规划', draft?.modules?.length ? `${draft.modules.length} 个模块` : '等待生成规划'],
    ['框架编辑', draft ? '主区域拖拽与模块微调' : '未进入'],
    ['效果图', STATIC_PAGE_STATUS_LABELS[previewStatus] || formatSnakeCaseLabel(previewStatus)],
    ['静态页生成', STATIC_PAGE_STATUS_LABELS[finalStatus] || formatSnakeCaseLabel(finalStatus)],
  ];
}

function ResultTextLink({ title, meta, detail, onClick, active = false }) {
  const Component = onClick ? 'button' : 'article';
  return (
    <Component
      type={onClick ? 'button' : undefined}
      className={`result-text-link ${active ? 'active' : ''}`.trim()}
      onClick={onClick}
    >
      <strong>{title}</strong>
      <span>{meta}</span>
      {detail ? <p>{detail}</p> : null}
    </Component>
  );
}

export default function InsightPanel({
  dataset,
  sessions,
  selectedSessionId,
  outputs,
  reportPlans,
  publishedReports,
  selectedReportPlanId,
  selectedReportPlan,
  reportRenderOutputs,
  reportAstVersions,
  publishedReportDetail,
  reportDetailLoading,
  reportActionBusy,
  reportSurface,
  publishNote,
  onSelectSession,
  onSelectReportPlan,
  onReportSurfaceChange,
  onPublishNoteChange,
  onContinueReportPlan,
  onRequestReportRender,
  onPublishReport,
  onRetryWorkflowExecution,
  onCancelWorkflowExecution,
  onRefreshReportDetail,
  staticPageDraft,
  staticPageDrafts = [],
  onSelectStaticPageDraft,
  onRefreshStaticPageDrafts,
  htmlArtifacts = [],
  activeHtmlArtifactId,
  onSelectHtmlArtifact,
}) {
  const activeHtmlSummaryId = activeHtmlArtifactId || '';
  const resultCount = staticPageDrafts.length + htmlArtifacts.length + reportPlans.length + publishedReports.length;

  return (
    <aside className="insight-panel">
      <section className="card insight-card right-brief-card">
        <SectionHeader
          title="项目任务"
          subtitle={dataset ? `当前供料：${dataset.title}` : '普通聊天 / 自动判断资料范围'}
        />
        <div className="right-brief-lines">
          <p>当前任务：问答、资料供料、静态页生成、报告产物都从底部对话输入框发起。</p>
          <p>可用动作：发送问题、上传资料、进入页面生成；模型负责理解意图，系统只提供工具和状态。</p>
          <p>页面原则：主区域编辑当前任务，右侧只显示任务阶段和结果索引。</p>
        </div>
        <div className="right-stage-list">
          {staticPageStageRows(staticPageDraft).map(([stage, status], index) => (
            <div className="right-stage-row" key={stage}>
              <span>{index + 1}. {stage}</span>
              <strong>{status}</strong>
            </div>
          ))}
        </div>
        {sessions.length ? (
          <div className="right-brief-links">
            <span>最近会话</span>
            {sessions.slice(0, 3).map((session) => (
              <button
                key={session.id}
                type="button"
                className={session.id === selectedSessionId ? 'active' : ''}
                onClick={() => onSelectSession?.(session.id)}
              >
                {truncateText(session.title, 28)}
              </button>
            ))}
          </div>
        ) : null}
      </section>

      <section className="card insight-card right-results-card">
        <SectionHeader
          title="生成结果"
          subtitle={resultCount ? `${resultCount} 个可打开结果` : '静态页、HTML 产物、报告会显示在这里'}
        />
        <div className="right-result-list">
          {staticPageDrafts.map((draft) => {
            const active = staticPageDraft?.id === draft.id;
            const staleReason = staticPageIsPreviewStale(draft)
              ? staticPageFinalRenderBlockReason(draft)
              : '';
            return (
              <ResultTextLink
                key={draft.id}
                active={active}
                title={truncateText(draft.objective || draft.title || '静态页草稿', 34)}
                meta={`静态页 · ${staticPageStatusLabel(draft)} · ${draft.modules?.length || 0} 模块`}
                detail={truncateText(staleReason || draft.finalPage?.notice || draft.modelSummary || '', 80)}
                onClick={() => onSelectStaticPageDraft?.(draft.id)}
              />
            );
          })}

          {htmlArtifacts.map((artifact) => {
            const summary = htmlArtifactSummary(artifact);
            return (
              <ResultTextLink
                key={summary.id}
                active={activeHtmlSummaryId === summary.id}
                title={truncateText(summary.title, 34)}
                meta={`HTML 产物 · ${summary.rejected ? '已拦截' : summary.sourceLabel || summary.meta}`}
                detail={truncateText(summary.subtitle, 80)}
                onClick={() => onSelectHtmlArtifact?.(summary.id)}
              />
            );
          })}

          {reportPlans.map((plan) => (
            <ResultTextLink
              key={plan.id}
              active={plan.id === selectedReportPlanId}
              title={truncateText(plan.title, 34)}
              meta={`报告计划 · ${formatSnakeCaseLabel(plan.status)}`}
              detail={truncateText(plan.objective, 80)}
              onClick={() => onSelectReportPlan?.(plan.id)}
            />
          ))}

          {publishedReports.map((report) => (
            <ResultTextLink
              key={report.id}
              active={report.plan_id === selectedReportPlanId}
              title={truncateText(report.slug, 34)}
              meta={`已发布 · ${formatDateTime(report.updated_at)}`}
              detail={`report ${truncateText(report.id, 16)}`}
              onClick={() => onSelectReportPlan?.(report.plan_id)}
            />
          ))}

          {!resultCount ? <EmptySection text="暂时还没有生成结果。通过底部“页面”或对话发起后会出现在这里。" /> : null}
        </div>
        <button type="button" className="ghost-btn compact-action-btn" onClick={onRefreshStaticPageDrafts}>
          刷新结果
        </button>
      </section>
    </aside>
  );
}
