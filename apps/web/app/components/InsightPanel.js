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
  return (
    <aside className="insight-panel">
      <section className="card insight-card">
        <SectionHeader
          title="会话"
          subtitle={dataset ? `${dataset.title} 下的最近会话` : '选择数据集后展示'}
        />
        <div className="insight-list">
          {sessions.length ? (
            sessions.map((session) => {
              const active = session.id === selectedSessionId;
              const reportEntryState = session.session_manifest_view?.report_entry?.state;
              return (
                <button
                  key={session.id}
                  type="button"
                  className={`insight-item ${active ? 'active' : ''}`}
                  onClick={() => onSelectSession(session.id)}
                >
                  <div className="insight-item-head">
                    <strong>{session.title}</strong>
                    <span>{formatRelativeTime(session.updated_at)}</span>
                  </div>
                  <p>{truncateText(session.latest_assistant_message?.content || session.session_manifest_view?.initial_prompt || '', 88)}</p>
                  <div className="insight-meta-row">
                    <span>{reportEntryState || 'not_applicable'}</span>
                    <span>{session.model_facing?.recommended_tool_key || 'chat_session'}</span>
                  </div>
                </button>
              );
            })
          ) : (
            <EmptySection text="当前数据集还没有会话。" />
          )}
        </div>
      </section>

      <section className="card insight-card">
        <SectionHeader
          title="HTML 产物"
          subtitle={htmlArtifacts.length ? `${htmlArtifacts.length} 个安全产物` : '报告/交接/审查'}
        />
        <div className="insight-list">
          {htmlArtifacts.length ? (
            htmlArtifacts.map((artifact) => {
              const summary = htmlArtifactSummary(artifact);
              const active = activeHtmlArtifactId === summary.id;
              return (
                <button
                  key={summary.id}
                  type="button"
                  className={`insight-item ${active ? 'active' : ''} ${summary.rejected ? 'blocked' : ''}`.trim()}
                  onClick={() => onSelectHtmlArtifact?.(summary.id)}
                >
                  <div className="insight-item-head">
                    <strong>{truncateText(summary.title, 30)}</strong>
                    <span>{summary.createdAt ? formatRelativeTime(summary.createdAt) : summary.sourceLabel || 'artifact'}</span>
                  </div>
                  <p>{truncateText(summary.subtitle, 96)}</p>
                  <div className="insight-meta-row">
                    <span>{summary.rejected ? '已拦截' : summary.sourceLabel}</span>
                    <span>{summary.meta}</span>
                  </div>
                </button>
              );
            })
          ) : (
            <EmptySection text="Codex 执行报告、静态页规划交接、代码审查摘要会在这里显示。" />
          )}
        </div>
      </section>

      <section className="card insight-card">
        <SectionHeader
          title="静态页成品"
          subtitle={staticPageDrafts.length ? `${staticPageDrafts.length} 个草稿/成品` : '当前终端暂无记录'}
        />
        <div className="insight-list">
          {staticPageDrafts.length ? (
            staticPageDrafts.map((draft) => {
              const active = staticPageDraft?.id === draft.id;
              const action = staticPageRenderAction(draft);
              const rendered = staticPageIsRendered(draft);
              const hasHtml = Boolean(buildStaticPageStandaloneHtml(draft, draft?.finalPage?.html || ''));
              const staleReason = staticPageIsPreviewStale(draft)
                ? staticPageFinalRenderBlockReason(draft)
                : '';
              const dataQuality = staticPageDataQualitySummary(draft);
              const hasDataQuality = hasDataQualitySummary(dataQuality);
              const needsDataAttention = dataQuality.attentionModules > 0
                || dataQuality.partialModules > 0
                || dataQuality.missingModules > 0;
              const qualityHighlights = staticPageDataQualityModules(draft)
                .filter((module) => module?.dataQualityStatus !== 'confirmed' || module?.fallback)
                .slice(0, 2);
              return (
                <div
                  key={draft.id}
                  className={`insight-item ${active ? 'active' : ''}`}
                >
                  <button
                    type="button"
                    className="insight-item-main"
                    onClick={() => onSelectStaticPageDraft?.(draft.id)}
                  >
                    <div className="insight-item-head">
                      <strong>{truncateText(draft.objective || draft.title || '静态页草稿', 30)}</strong>
                      <span>{formatRelativeTime(staticPageUpdatedAt(draft))}</span>
                    </div>
                    <p>{truncateText(staleReason || draft.modelSummary || draft.finalPage?.notice || '点击后在当前页面继续规划或查看生成结果。', 96)}</p>
                    <div className="insight-meta-row">
                      <span>{staticPageStatusLabel(draft)}</span>
                      <span>{draft.modules?.length || 0} 个模块</span>
                      {staticPageWorkflowExecutionId(draft) ? (
                        <span>workflow {truncateText(staticPageWorkflowExecutionId(draft), 12)}</span>
                      ) : null}
                      {hasDataQuality ? (
                        <span className={`insight-quality ${needsDataAttention ? 'attention' : 'ready'}`}>
                          数据 {dataQuality.confirmedModules}/{dataQuality.partialModules}/{dataQuality.missingModules}
                        </span>
                      ) : null}
                    </div>
                    {qualityHighlights.length ? (
                      <div className="insight-quality-detail">
                        {qualityHighlights.map((module) => (
                          <span key={module.moduleId || module.title}>
                            {truncateText(module.title || module.moduleId || '未命名模块', 14)}
                            {' · '}
                            {staticPageQualityStatusLabel(module.dataQualityStatus)}
                            {' · '}
                            {staticPageChartRuntimeLabel(module.chartRuntime)}
                            {module.fallback ? '回退' : ''}
                          </span>
                        ))}
                      </div>
                    ) : null}
                  </button>
                  {action || rendered ? (
                    <div className="insight-item-actions">
                      {action ? (
                        <button
                          type="button"
                          className="ghost-btn compact-action-btn"
                          onClick={() => {
                            if (action.kind === 'retry') {
                              onRetryWorkflowExecution?.(action.executionId);
                            } else {
                              onCancelWorkflowExecution?.(action.executionId);
                            }
                          }}
                        >
                          {action.label}后台生成
                        </button>
                      ) : null}
                      {rendered && hasHtml ? (
                        <button
                          type="button"
                          className="ghost-btn compact-action-btn"
                          onClick={() => downloadStaticPageHtmlFromShelf(draft)}
                        >
                          下载 HTML
                        </button>
                      ) : null}
                      {rendered ? (
                        <button
                          type="button"
                          className="ghost-btn compact-action-btn"
                          onClick={() => downloadStaticPagePackageFromShelf(draft)}
                        >
                          交付包
                        </button>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              );
            })
          ) : (
            <EmptySection text="对话生成静态页后，会保存在这里。" />
          )}
        </div>
        <button type="button" className="ghost-btn compact-action-btn" onClick={onRefreshStaticPageDrafts}>
          刷新草稿架
        </button>
      </section>

      <section className="card insight-card">
        <SectionHeader
          title="资料输出"
          subtitle="数据集级 material output 回看"
        />
        <div className="insight-list">
          {outputs.length ? (
            outputs.map((output) => (
              <article className="insight-item static" key={output.id}>
                <div className="insight-item-head">
                  <strong>{truncateText(output.prompt, 28)}</strong>
                  <span>{formatRelativeTime(output.created_at)}</span>
                </div>
                <p>{truncateText(output.output_text, 110) || '等待 worker 写回输出。'}</p>
                <div className="insight-meta-row">
                  <span>证据 {output.retrieval_evidence_ids?.length || 0}</span>
                  <span>工具 {output.tool_executions?.length || 0}</span>
                </div>
              </article>
            ))
          ) : (
            <EmptySection text="当前数据集还没有资料输出。" />
          )}
        </div>
      </section>

      <section className="card insight-card">
        <SectionHeader
          title="报告计划"
          subtitle="选择 plan 后可继续规划、渲染和发布"
        />
        <div className="insight-list">
          {reportPlans.length ? (
            reportPlans.map((plan) => {
              const active = plan.id === selectedReportPlanId;
              return (
                <button
                  type="button"
                  className={`insight-item ${active ? 'active' : ''}`}
                  key={plan.id}
                  onClick={() => onSelectReportPlan(plan.id)}
                >
                  <div className="insight-item-head">
                    <strong>{plan.title}</strong>
                    <span>{formatSnakeCaseLabel(plan.status)}</span>
                  </div>
                  <p>{truncateText(plan.objective, 110)}</p>
                  <div className="insight-meta-row">
                    <span>{plan.model_facing?.recommended_tool_key || 'report.plan'}</span>
                    <span>{plan.current_ast_version_id ? '已有 AST 版本' : '尚无 AST 版本'}</span>
                  </div>
                </button>
              );
            })
          ) : (
            <EmptySection text="当前数据集下还没有 report plan。" />
          )}
        </div>
      </section>

      <section className="card insight-card report-control-card">
        <SectionHeader
          title="报告服务控制台"
          subtitle={selectedReportPlan ? selectedReportPlan.title : '等待选择 report plan'}
        />
        <ReportPlanDetail
          plan={selectedReportPlan}
          renderOutputs={reportRenderOutputs}
          astVersions={reportAstVersions}
          publishedDetail={publishedReportDetail}
          loading={reportDetailLoading}
          actionBusy={reportActionBusy}
          surface={reportSurface}
          publishNote={publishNote}
          onSurfaceChange={onReportSurfaceChange}
          onPublishNoteChange={onPublishNoteChange}
          onContinue={onContinueReportPlan}
          onRender={onRequestReportRender}
          onPublish={onPublishReport}
          onRetryWorkflowExecution={onRetryWorkflowExecution}
          onRefresh={onRefreshReportDetail}
        />
      </section>

      <section className="card insight-card">
        <SectionHeader
          title="已发布"
          subtitle="published reports 聚合视图"
        />
        <div className="insight-list">
          {publishedReports.length ? (
            publishedReports.map((report) => (
              <button
                type="button"
                className={`insight-item ${report.plan_id === selectedReportPlanId ? 'active' : ''}`}
                key={report.id}
                onClick={() => onSelectReportPlan(report.plan_id)}
              >
                <div className="insight-item-head">
                  <strong>{report.slug}</strong>
                  <span>{formatDateTime(report.updated_at)}</span>
                </div>
                <p>plan_id: {truncateText(report.plan_id, 24)}</p>
                <div className="insight-meta-row">
                  <span>report_id {truncateText(report.id, 12)}</span>
                  <span>{report.current_version_id ? '有当前版本' : '尚未设当前版本'}</span>
                </div>
              </button>
            ))
          ) : (
            <EmptySection text="这个数据集下还没有 published report。" />
          )}
        </div>
      </section>
    </aside>
  );
}
