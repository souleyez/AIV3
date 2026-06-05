'use client';

import { useState } from 'react';
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
  downloadUrlArtifact,
  hasDataQualitySummary,
  staticPageHtmlDownloadHref,
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
  preview_ready: '效果图已生成',
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

function staticPageFinalPageUrl(draft) {
  const finalPage = draft?.finalPage || {};
  const manifest = finalPage.assetManifest || finalPage.asset_manifest || {};
  return finalPage.publicUrl
    || finalPage.public_url
    || finalPage.generatedArtifactUrl
    || finalPage.generated_artifact_url
    || finalPage.htmlPreviewUrl
    || finalPage.html_preview_url
    || finalPage.htmlDownloadUrl
    || finalPage.html_download_url
    || finalPage.downloadUrl
    || finalPage.download_url
    || manifest.publicUrl
    || manifest.public_url
    || manifest.generatedArtifactUrl
    || manifest.generated_artifact_url
    || manifest.artifactPublicUrl
    || manifest.artifact_public_url
    || manifest.publish_result?.public_url
    || manifest.publishResult?.publicUrl
    || manifest.output?.artifact_public_url
    || '';
}

function staticPageFinalManifest(draft) {
  const finalPage = draft?.finalPage || {};
  return finalPage.assetManifest || finalPage.asset_manifest || {};
}

function staticPageOfficialTitle(draft) {
  const finalPage = draft?.finalPage || {};
  const manifest = staticPageFinalManifest(draft);
  return manifest.reportTitle
    || manifest.report_title
    || manifest.displayTitle
    || manifest.display_title
    || manifest.title
    || finalPage.reportTitle
    || finalPage.report_title
    || finalPage.displayTitle
    || finalPage.display_title
    || '';
}

function staticPageArtifactSiblingUrl(draft, fileName) {
  const finalPageUrl = staticPageFinalPageUrl(draft);
  if (!finalPageUrl) return '';
  try {
    const url = new URL(finalPageUrl, typeof window !== 'undefined' ? window.location.origin : 'https://v3.elepcloud.com');
    url.search = '';
    url.hash = '';
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts[parts.length - 1]?.toLowerCase() === 'index.html') {
      parts.pop();
    }
    parts.push(fileName);
    url.pathname = `/${parts.join('/')}`;
    if (typeof window !== 'undefined' && url.origin === window.location.origin) {
      return `${url.pathname}${url.search}${url.hash}`;
    }
    return url.toString();
  } catch {
    return '';
  }
}

function staticPageExportUrl(draft, keys, fallbackFileName = '') {
  const finalPage = draft?.finalPage || {};
  const manifest = staticPageFinalManifest(draft);
  const containers = [finalPage, manifest, manifest.exportPackage, manifest.export_package].filter(Boolean);
  for (const container of containers) {
    for (const key of keys) {
      const value = String(container?.[key] || '').trim();
      if (value) return value;
    }
  }
  const exports = finalPage.downloadExports || finalPage.download_exports || manifest.downloadExports || manifest.download_exports || [];
  if (Array.isArray(exports)) {
    for (const item of exports) {
      const kind = String(item?.kind || item?.format || '').toLowerCase();
      const label = String(item?.label || '').toLowerCase();
      if (keys.some((key) => kind.includes(key.toLowerCase()) || label.includes(key.toLowerCase()))) {
        const value = String(item?.url || item?.href || '').trim();
        if (value) return value;
      }
    }
  }
  return fallbackFileName ? staticPageArtifactSiblingUrl(draft, fallbackFileName) : '';
}

function downloadStaticPageHtmlFromShelf(draft) {
  const downloadHref = draft?.finalPage?.htmlDownloadUrl || draft?.finalPage?.html_download_url || '';
  if (downloadHref && downloadUrlArtifact({ href: downloadHref })) {
    return;
  }
  const html = buildStaticPageStandaloneHtml(draft, draft?.finalPage?.html || '');
  if (!html) return;
  downloadTextArtifact({
    content: html,
    filename: staticPageHtmlFilename(draft),
    mime: 'text/html;charset=utf-8',
  });
}

function openStaticPageFromShelf(draft) {
  const href = staticPageHtmlDownloadHref(staticPageFinalPageUrl(draft));
  if (href && typeof window !== 'undefined') {
    window.open(href, '_blank', 'noopener,noreferrer');
    return true;
  }
  const html = buildStaticPageStandaloneHtml(draft, draft?.finalPage?.html || '');
  if (!html || typeof window === 'undefined') return false;
  const url = URL.createObjectURL(new Blob([html], { type: 'text/html;charset=utf-8' }));
  window.open(url, '_blank', 'noopener,noreferrer');
  window.setTimeout(() => URL.revokeObjectURL(url), 60000);
  return true;
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
    templateId: result.manifest.templateId,
    payload: result.manifest.payload,
    provenance: result.manifest.provenance,
  };
}

function artifactKindLabel(kind) {
  const labels = {
    table_data: '表格数据',
    report_ppt: '导出PPT',
    report_markdown: '文本下载（MD）',
    pptx: '下载PPTX',
    final_deliverables_manifest: '交付清单',
    published_deliverable_manifest: '发布清单',
    published_version_history: '版本历史',
    extraction_artifacts_manifest: '产物索引',
    transcript_text: '原文',
    source_text: '来源文档',
    ppt_outline: 'PPT大纲',
    slide_notes: '讲稿备注',
    video_slides_markdown: 'Markdown讲义',
    subtitle_page_map: '字幕对页',
    timestamp_map: '时间映射',
    contact_sheet_html: '接触表',
    ppt_keep_list_template: 'Keep-list',
    selected_slides_manifest: '选页清单',
    pptx_build_plan: '构建计划',
  };
  return labels[kind] || formatSnakeCaseLabel(kind || '文件');
}

function staticPagePublishedSiblingPath(path, fileName) {
  const value = String(path || '').trim();
  if (!value) return '';
  try {
    const url = new URL(value, 'https://v3.elepcloud.com');
    url.search = '';
    url.hash = '';
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts[parts.length - 1]?.toLowerCase() === 'index.html') {
      parts.pop();
    }
    parts.push(fileName);
    url.pathname = `/${parts.join('/')}`;
    if (value.startsWith('/')) return url.pathname;
    return url.toString();
  } catch {
    return '';
  }
}

function htmlArtifactGeneratedFiles(artifact) {
  const result = normalizeHtmlArtifactManifest(artifact);
  if (result.rejected) return [];
  const generatedArtifacts = result.manifest.payload?.generatedArtifacts || result.manifest.payload?.generated_artifacts || {};
  const files = Array.isArray(generatedArtifacts.files) ? generatedArtifacts.files : [];
  const sourceRunId = result.manifest.provenance?.sourceRunId || '';
  const localThreadId = result.manifest.payload?.localThreadId || result.manifest.payload?.local_thread_id || '';
  const normalizedFiles = files.map((file, index) => {
    const kind = file?.artifactKind || file?.artifact_kind || '';
    const path = file?.path || file?.uri || '';
    const params = new URLSearchParams();
    if (sourceRunId) {
      params.set('assistant_run_id', sourceRunId);
    } else if (localThreadId) {
      params.set('local_thread_id', localThreadId);
    }
    return {
      index,
      kind,
      label: artifactKindLabel(kind),
      path,
      downloadable: Boolean(file?.path && params.toString()),
      url: params.toString()
        ? `/api/v3/html-artifacts/${encodeURIComponent(result.manifest.id)}/files/${index}?${params.toString()}`
        : '',
    };
  });
  if (result.manifest.templateId === 'static_page_published_preview') {
    const payload = result.manifest.payload || {};
    const previewPath = payload.previewPath || payload.preview_path || '';
    const virtualFiles = [
      ['table_data', 'table-data.csv'],
      ['report_ppt', 'report.ppt'],
      ['report_markdown', 'report.md'],
    ].map(([kind, fileName], index) => {
      const url = staticPagePublishedSiblingPath(previewPath, fileName);
      return {
        index: `published-${index}`,
        kind,
        label: artifactKindLabel(kind),
        path: url,
        downloadable: Boolean(url),
        url,
      };
    }).filter((file) => file.downloadable);
    return [...virtualFiles, ...normalizedFiles];
  }
  return normalizedFiles;
}

function preferredHtmlArtifactDownloads(artifact) {
  const priority = [
    'table_data',
    'report_ppt',
    'report_markdown',
    'pptx',
    'video_slides_markdown',
    'final_deliverables_manifest',
    'published_deliverable_manifest',
    'published_version_history',
    'extraction_artifacts_manifest',
    'slide_notes',
    'subtitle_page_map',
    'ppt_outline',
    'transcript_text',
  ];
  const files = htmlArtifactGeneratedFiles(artifact).filter((file) => file.downloadable);
  return files
    .sort((left, right) => {
      const leftIndex = priority.indexOf(left.kind);
      const rightIndex = priority.indexOf(right.kind);
      return (leftIndex === -1 ? 99 : leftIndex) - (rightIndex === -1 ? 99 : rightIndex);
    })
    .slice(0, 5);
}

function videoDeliverablePackage(payload = {}) {
  return payload?.deliverablePackage || payload?.deliverable_package || {};
}

function videoDeliverablePackageStatus(payload = {}) {
  const deliverablePackage = videoDeliverablePackage(payload);
  const lifecycle = deliverablePackage.lifecycleState || deliverablePackage.lifecycle_state || '';
  const immutable = deliverablePackage.immutableVersion || deliverablePackage.immutable_version;
  const publishable = deliverablePackage.publishable === true;
  const missing = deliverablePackage.missingRequiredFileKinds || deliverablePackage.missing_required_file_kinds || [];
  if (immutable) return '已发布版本';
  if (publishable || lifecycle === 'downloadable_not_published') return '可下载未发布';
  if (Array.isArray(missing) && missing.length > 0) return `交付包缺${missing.length}项`;
  if (lifecycle === 'not_ready') return '交付包未完成';
  return '';
}

function htmlArtifactProjectStage(summary) {
  const status = summary.payload?.deliverableStatus || summary.payload?.deliverable_status || {};
  const state = status.state || summary.payload?.parseStatus || summary.payload?.parse_status || summary.meta;
  if (summary.templateId === 'video_extraction_summary') {
    const packageStatus = videoDeliverablePackageStatus(summary.payload);
    return {
      label: '视频/PPT',
      status: packageStatus || (state === 'final_pptx_ready' ? 'PPTX已就绪' : formatSnakeCaseLabel(state || '处理中')),
    };
  }
  return {
    label: summary.sourceLabel || '产物',
    status: formatSnakeCaseLabel(state || summary.meta || '已生成'),
  };
}

function htmlArtifactBrief(summary) {
  const status = summary.payload?.deliverableStatus || summary.payload?.deliverable_status || {};
  if (summary.templateId === 'video_extraction_summary') {
    const packageStatus = videoDeliverablePackageStatus(summary.payload);
    const packageSuffix = packageStatus ? ` · ${packageStatus}` : '';
    const completionFollowUp = summary.payload?.completionFollowUp || summary.payload?.completion_follow_up || {};
    const notification = completionFollowUp.userNotification || completionFollowUp.user_notification || {};
    const completionAudit = summary.payload?.completionAudit || summary.payload?.completion_audit || {};
    const auditProviderFailures = Number(
      completionAudit.providerFailureCount
        ?? completionAudit.provider_failure_count
        ?? (completionAudit.providerFailures || completionAudit.provider_failures || []).length
        ?? 0,
    );
    const auditWarningCount = Number(completionAudit.warningCount ?? completionAudit.warning_count ?? 0);
    const auditSuffix = auditProviderFailures > 0
      ? ` · 审计：${auditProviderFailures} 个提供方需复核`
      : auditWarningCount > 0
        ? ` · 审计：${auditWarningCount} 条质量提示`
        : completionAudit.kind
          ? ' · 审计已脱敏'
          : '';
    if (notification.userVisible || notification.user_visible) {
      const modelSuffix = (completionFollowUp.modelFollowUp || completionFollowUp.model_follow_up)?.required
        ? ' · 模型接手待回复'
        : '';
      return `${notification.title || notification.message || summary.subtitle || summary.meta}${modelSuffix}${packageSuffix}${auditSuffix}`;
    }
    const parts = [
      status.hasPptx || status.has_pptx ? 'PPTX ready' : 'PPTX waiting',
      (completionFollowUp.modelFollowUp || completionFollowUp.model_follow_up)?.required ? 'model turn requested' : '',
      status.hasVideoSlidesMarkdown || status.has_video_slides_markdown ? 'Markdown deck ready' : '',
      status.hasFinalDeliverablesManifest || status.has_final_deliverables_manifest ? 'deliverable manifest ready' : '',
      status.hasPublishedVersionHistory || status.has_published_version_history ? 'version history ready' : '',
      status.hasExtractionArtifactsManifest || status.has_extraction_artifacts_manifest ? 'artifact index ready' : '',
      status.hasSlideNotes || status.has_slide_notes ? 'slide notes ready' : '',
      status.hasSubtitlePageMap || status.has_subtitle_page_map ? 'subtitle map ready' : '',
      packageStatus,
      auditSuffix.trim().replace(/^·\s*/, ''),
    ].filter(Boolean);
    return parts.join(' · ') || summary.subtitle || summary.meta;
  }
  return summary.subtitle || summary.meta;
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

function staticPageProjectStage(draft) {
  if (!draft) {
    return { key: 'template', index: 0, label: '模板规划', status: '等待创建' };
  }
  const finalStatus = draft?.finalPage?.status || '';
  const previewStatus = draft?.previewContract?.status || draft?.imageJob?.status || '';
  const hasFinalStage = Boolean(finalStatus)
    || draft.status === 'rendered'
    || draft.status === 'rendering';
  if (hasFinalStage) {
    return {
      key: 'static',
      index: 2,
      label: '静态页',
      status: STATIC_PAGE_STATUS_LABELS[finalStatus] || STATIC_PAGE_STATUS_LABELS[draft.status] || formatSnakeCaseLabel(finalStatus || draft.status),
    };
  }
  const hasEffectStage = Boolean(draft.previewImage)
    || Boolean(draft.imageJob?.id)
    || ['queued', 'running', 'preview_ready', 'effect_confirmed'].includes(draft.status)
    || ['queued', 'running', 'preview_ready', 'confirmed', 'stale'].includes(previewStatus);
  if (hasEffectStage) {
    return {
      key: 'effect',
      index: 1,
      label: '效果图',
      status: STATIC_PAGE_STATUS_LABELS[previewStatus] || STATIC_PAGE_STATUS_LABELS[draft.status] || formatSnakeCaseLabel(previewStatus || draft.status),
    };
  }
  return {
    key: 'template',
    index: 0,
    label: '模板规划',
    status: draft.modules?.length ? `${draft.modules.length} 个模块` : '规划中',
  };
}

function staticPageProjectTitle(draft) {
  return truncateText(staticPageOfficialTitle(draft) || draft?.objective || draft?.title || '静态页项目', 38);
}

function staticPageProjectCanExport(draft) {
  const finalStatus = draft?.finalPage?.status || '';
  return finalStatus === 'rendered' || finalStatus === 'mock_ready' || draft?.status === 'rendered';
}

function safeExportId(draft) {
  return String(draft?.id || 'static-page')
    .trim()
    .replace(/[^a-zA-Z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64) || 'static-page';
}

function escapeHtml(value) {
  return String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function csvCell(value) {
  const text = String(value ?? '').replace(/\r?\n/g, ' ');
  return /[",\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function downloadStaticPageTable(draft) {
  const exportUrl = staticPageExportUrl(
    draft,
    ['table_data_url', 'tableDataUrl', 'table_data', 'csv_url', 'csvUrl', 'csv', '表格数据'],
    'table-data.csv',
  );
  if (exportUrl && downloadUrlArtifact({ href: exportUrl })) {
    return;
  }
  const rows = [
    ['阶段', '模块', '标题', '内容', '数据来源', '可视化'],
    ...(draft?.modules || []).map((module) => [
      staticPageProjectStage(draft).label,
      module.id,
      module.title,
      module.content,
      module.dataBinding?.label || module.dataBinding?.type || '',
      module.visualization?.label || module.visualization?.type || '',
    ]),
  ];
  downloadTextArtifact({
    content: `\uFEFF${rows.map((row) => row.map(csvCell).join(',')).join('\n')}`,
    filename: `static-page-${safeExportId(draft)}-modules.csv`,
    mime: 'text/csv;charset=utf-8',
  });
}

function downloadStaticPagePpt(draft) {
  const exportUrl = staticPageExportUrl(
    draft,
    ['ppt_download_url', 'pptDownloadUrl', 'ppt_url', 'pptUrl', 'ppt', '导出PPT'],
    'report.ppt',
  );
  if (exportUrl && downloadUrlArtifact({ href: exportUrl })) {
    return;
  }
  const modules = draft?.modules || [];
  const slideSections = modules.map((module, index) => `
    <section class="slide">
      <p class="eyebrow">${index + 1} / ${modules.length || 1} · ${escapeHtml(module.visualization?.label || module.role || '模块')}</p>
      <h2>${escapeHtml(module.title || '未命名模块')}</h2>
      <p>${escapeHtml(module.content || '')}</p>
      <footer>${escapeHtml(module.dataBinding?.label || '数据待绑定')}</footer>
    </section>
  `).join('');
  const html = `<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <title>${escapeHtml(staticPageProjectTitle(draft))}</title>
  <style>
    body { margin: 0; font-family: Aptos, Calibri, sans-serif; color: #102033; background: #f8fafc; }
    .slide { width: 960px; min-height: 540px; box-sizing: border-box; padding: 72px 82px; page-break-after: always; background: linear-gradient(135deg, #ffffff, #eef6ff); }
    .cover { background: linear-gradient(135deg, #0f172a, #2563eb); color: white; }
    .eyebrow { margin: 0 0 22px; color: #38bdf8; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; }
    h1, h2 { margin: 0 0 22px; font-size: 44px; line-height: 1.08; }
    p { font-size: 23px; line-height: 1.52; }
    footer { margin-top: 42px; color: #64748b; font-size: 18px; }
  </style>
</head>
<body>
  <section class="slide cover">
    <p class="eyebrow">DataMax Static Page Export</p>
    <h1>${escapeHtml(staticPageProjectTitle(draft))}</h1>
    <p>${escapeHtml(draft?.modelSummary || '由静态页项目导出的演示稿。')}</p>
  </section>
  ${slideSections}
</body>
</html>`;
  downloadTextArtifact({
    content: html,
    filename: `static-page-${safeExportId(draft)}.ppt`,
    mime: 'application/vnd.ms-powerpoint;charset=utf-8',
  });
}

function downloadStaticPageMarkdown(draft) {
  const exportUrl = staticPageExportUrl(
    draft,
    ['markdown_download_url', 'markdownDownloadUrl', 'text_download_url', 'textDownloadUrl', 'markdown', 'md', '文本下载'],
    'report.md',
  );
  if (exportUrl && downloadUrlArtifact({ href: exportUrl })) {
    return;
  }
  const title = staticPageOfficialTitle(draft) || draft?.objective || draft?.title || 'DataMax 经营分析报表';
  const modules = draft?.modules || [];
  const lines = [
    `# ${title}`,
    '',
    draft?.modelSummary || '页面已生成，可在主站内预览并继续通过对话修改。',
    '',
    '## 模块',
    '',
    ...modules.map((module, index) => [
      `### ${index + 1}. ${module.title || module.id || '未命名模块'}`,
      '',
      module.content || '',
      '',
      `- 数据来源：${module.dataBinding?.label || module.dataBinding?.type || '数据待绑定'}`,
      `- 可视化：${module.visualization?.label || module.visualization?.type || '页面模块'}`,
      '',
    ].join('\n')),
  ];
  downloadTextArtifact({
    content: lines.join('\n'),
    filename: `static-page-${safeExportId(draft)}.md`,
    mime: 'text/markdown;charset=utf-8',
  });
}

function ExecutionObservationCard({ progress }) {
  const steps = Array.isArray(progress?.steps) ? progress.steps : [];
  const traceSteps = Array.isArray(progress?.traceSteps) ? progress.traceSteps : [];
  return (
    <section className="card insight-card right-observation-card">
      <SectionHeader
        title="本次执行观测"
        subtitle={steps.length || traceSteps.length ? '模型怎样检索、深读、调用工具会收在这里' : '本轮暂时没有执行条目'}
      />
      {steps.length ? (
        <div className="right-observation-steps">
          {steps.map((step, index) => (
            <div className="right-observation-step" key={`${step.label}-${index}`}>
              <strong>{step.label}</strong>
              <span>
                {step.message || formatSnakeCaseLabel(step.status)}
                {step.suppliedCount !== null ? ` · 供料 ${step.suppliedCount}` : ''}
                {step.detailTargetCount ? ` · 深读 ${step.detailTargetCount}` : ''}
                {step.returnedCount !== null ? ` · 返回 ${step.returnedCount}` : ''}
                {step.deniedCount ? ` · 拒绝 ${step.deniedCount}` : ''}
              </span>
            </div>
          ))}
        </div>
      ) : <EmptySection text="发送问题后，本轮供料、深读和工具闭环会在这里显示。" />}
      {traceSteps.length ? (
        <div className="right-observation-trace">
          {traceSteps.map((step, index) => (
            <span className={`message-chip ${step.status === 'failed' ? 'danger' : step.status === 'completed' ? 'green' : 'neutral'}`} key={`${step.actionType}-${index}`}>
              {formatSnakeCaseLabel(step.actionType)}
              {step.returnedCount ? ` · ${step.returnedCount}` : ''}
              {step.durationMs !== null ? ` · ${step.durationMs}ms` : ''}
            </span>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function GeneratedProjectCard({
  draft,
  active,
  open,
  copied,
  onSelect,
  onPreview,
  onDelete,
  onRevert,
  onCopyLink,
}) {
  const stage = staticPageProjectStage(draft);
  const exportable = staticPageProjectCanExport(draft);
  const finalPageUrl = staticPageFinalPageUrl(draft);
  const canOpenFinalPage = Boolean(finalPageUrl || buildStaticPageStandaloneHtml(draft, draft?.finalPage?.html || ''));
  const staleReason = staticPageIsPreviewStale(draft)
    ? staticPageFinalRenderBlockReason(draft)
    : '';
  const updatedAt = staticPageUpdatedAt(draft);
  const summary = truncateText(
    staleReason || draft?.finalPage?.notice || draft?.modelSummary || '模板规划、效果图、静态页会按阶段推进。',
    58,
  );
  const handleSelect = () => {
    if (staticPageIsRendered(draft) && canOpenFinalPage && onPreview) {
      onPreview();
      return;
    }
    onSelect?.();
  };
  return (
    <article className={`generated-project-card ${active ? 'active' : ''}`.trim()}>
      <button type="button" className="generated-project-main" onClick={handleSelect}>
        <div className="generated-project-title-row">
          <strong>{staticPageProjectTitle(draft)}</strong>
          <time dateTime={updatedAt || undefined}>{updatedAt ? formatRelativeTime(updatedAt) : '刚刚'}</time>
        </div>
        <div className="generated-project-brief-row">
          <span>{summary}</span>
          <em>{stage.label} · {stage.status}</em>
        </div>
      </button>
      {active ? (
        <div className="generated-project-actions" aria-label={`${open ? '已打开' : '已选中'}项目操作`}>
          {stage.index > 0 ? (
            <button type="button" className="ghost-btn compact-action-btn" onClick={onRevert}>
              退回上阶段
            </button>
          ) : null}
          {stage.key === 'static' ? (
            <>
              {canOpenFinalPage ? (
                <button type="button" className="primary-btn compact-action-btn" onClick={onPreview || (() => openStaticPageFromShelf(draft))}>
                  预览页面
                </button>
              ) : null}
              {finalPageUrl ? (
                <button type="button" className="ghost-btn compact-action-btn" onClick={() => openStaticPageFromShelf(draft)}>
                  新窗口
                </button>
              ) : null}
              <button type="button" className="ghost-btn compact-action-btn" disabled={!exportable} onClick={() => downloadStaticPageHtmlFromShelf(draft)}>
                HTML
              </button>
              <button type="button" className="ghost-btn compact-action-btn" disabled={!exportable} onClick={() => downloadStaticPagePackageFromShelf(draft)}>
                ZIP
              </button>
              <button type="button" className="ghost-btn compact-action-btn" disabled={!exportable} onClick={() => downloadStaticPagePpt(draft)}>
                导出PPT
              </button>
              <button type="button" className="ghost-btn compact-action-btn" disabled={!exportable} onClick={() => downloadStaticPageTable(draft)}>
                表格数据
              </button>
              <button type="button" className="ghost-btn compact-action-btn" disabled={!exportable} onClick={() => downloadStaticPageMarkdown(draft)}>
                文本下载（MD）
              </button>
              <button type="button" className="ghost-btn compact-action-btn" onClick={onCopyLink}>
                {copied ? '已复制' : '复制链接'}
              </button>
            </>
          ) : null}
          <button type="button" className="ghost-btn compact-action-btn danger-action" onClick={onDelete}>
            删除
          </button>
        </div>
      ) : null}
    </article>
  );
}

function GeneratedHtmlArtifactCard({
  artifact,
  active,
  onSelect,
}) {
  const summary = htmlArtifactSummary(artifact);
  const stage = htmlArtifactProjectStage(summary);
  const downloads = preferredHtmlArtifactDownloads(artifact);
  return (
    <article className={`generated-project-card html-artifact-project-card ${active ? 'active' : ''}`.trim()}>
      <button
        type="button"
        className="generated-project-main"
        disabled={summary.rejected}
        onClick={summary.rejected ? undefined : onSelect}
      >
        <div className="generated-project-title-row">
          <strong>{truncateText(summary.title, 38)}</strong>
          <time dateTime={summary.createdAt || undefined}>{summary.createdAt ? formatRelativeTime(summary.createdAt) : '刚刚'}</time>
        </div>
        <div className="generated-project-brief-row">
          <span>{truncateText(htmlArtifactBrief(summary), 58)}</span>
          <em>{stage.label} · {stage.status}</em>
        </div>
      </button>
      {active ? (
        <div className="generated-project-actions" aria-label="产物操作">
          <button type="button" className="ghost-btn compact-action-btn" disabled={summary.rejected} onClick={onSelect}>
            打开详情
          </button>
          {downloads.map((file) => (
            <a
              key={`${file.index}-${file.kind}`}
              className="ghost-btn compact-action-btn artifact-download-link"
              href={file.url}
              download
            >
              {file.label}
            </a>
          ))}
        </div>
      ) : null}
    </article>
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
  onPreviewStaticPageDraft,
  onDeleteStaticPageDraft,
  onRevertStaticPageStage,
  onRefreshStaticPageDrafts,
  staticPageEditorOpen = false,
  assistantRunProgress,
  showExecutionObservability = false,
  htmlArtifacts = [],
  activeHtmlArtifactId,
  onSelectHtmlArtifact,
}) {
  const [copiedProjectId, setCopiedProjectId] = useState('');
  const shelfHtmlArtifacts = htmlArtifacts.filter((artifact) => {
    const templateId = artifact?.templateId || artifact?.template_id;
    return templateId !== 'static_page_planning_handoff'
      && templateId !== 'static_page_published_preview'
      && templateId !== 'static_page_data_quality_report';
  });
  const resultCount = staticPageDrafts.length + shelfHtmlArtifacts.length;

  async function copyProjectLink(draft) {
    const finalPageUrl = staticPageHtmlDownloadHref(staticPageFinalPageUrl(draft));
    const link = finalPageUrl
      ? finalPageUrl.startsWith('/') && typeof window !== 'undefined'
        ? `${window.location.origin}${finalPageUrl}`
        : finalPageUrl
      : (typeof window === 'undefined'
      ? `#static-page-${draft.id}`
      : `${window.location.origin}${window.location.pathname}#static-page-${draft.id}`);
    try {
      await navigator.clipboard.writeText(link);
      setCopiedProjectId(draft.id);
      window.setTimeout(() => setCopiedProjectId(''), 1600);
    } catch {
      setCopiedProjectId('');
    }
  }

  return (
    <aside className="insight-panel">
      {showExecutionObservability ? <ExecutionObservationCard progress={assistantRunProgress} /> : null}

      <section className="card insight-card right-results-card">
        <SectionHeader
          title="生成项目"
          subtitle={resultCount ? `${resultCount} 个项目，每个项目一张卡` : '从聊天或“页面”按钮创建项目'}
        />
        <div className="generated-project-list">
          {staticPageDrafts.map((draft) => {
            const active = staticPageDraft?.id === draft.id;
            return (
              <GeneratedProjectCard
                key={draft.id}
                active={active}
                open={active && staticPageEditorOpen}
                copied={copiedProjectId === draft.id}
                draft={draft}
                onClick={() => onSelectStaticPageDraft?.(draft.id)}
                onSelect={() => onSelectStaticPageDraft?.(draft.id)}
                onPreview={() => onPreviewStaticPageDraft?.(draft.id)}
                onDelete={() => onDeleteStaticPageDraft?.(draft.id)}
                onRevert={() => onRevertStaticPageStage?.(draft.id)}
                onCopyLink={() => copyProjectLink(draft)}
              />
            );
          })}
          {shelfHtmlArtifacts.map((artifact) => {
            const artifactId = artifact?.id || artifact?.artifact_id;
            return (
              <GeneratedHtmlArtifactCard
                key={artifactId}
                active={activeHtmlArtifactId === artifactId}
                artifact={artifact}
                onSelect={() => onSelectHtmlArtifact?.(artifactId)}
              />
            );
          })}
          {!resultCount ? <EmptySection text="暂时还没有生成项目。通过对话发起后会出现在这里。" /> : null}
        </div>
        <button type="button" className="ghost-btn compact-action-btn" onClick={onRefreshStaticPageDrafts}>
          刷新项目
        </button>
      </section>
    </aside>
  );
}
