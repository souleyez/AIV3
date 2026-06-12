export function staticPageRenderedUrlFromDraft(draftOrOutput) {
  const finalPage = draftOrOutput?.finalPage || draftOrOutput || {};
  return finalPage.publicUrl
    || finalPage.public_url
    || finalPage.generatedArtifactUrl
    || finalPage.generated_artifact_url
    || finalPage.html_preview_url
    || finalPage.htmlPreviewUrl
    || finalPage.html_download_url
    || finalPage.htmlDownloadUrl
    || finalPage.download_url
    || finalPage.downloadUrl
    || finalPage.asset_manifest?.public_url
    || finalPage.assetManifest?.public_url
    || finalPage.assetManifest?.publicUrl
    || finalPage.asset_manifest?.generated_artifact_url
    || finalPage.assetManifest?.generatedArtifactUrl
    || '';
}

export function staticPageSafePreviewPath(value) {
  const raw = String(value || '').trim();
  if (!raw) return '';
  const toAllowedPath = (url) => {
    const path = `${url.pathname || '/'}${url.search || ''}${url.hash || ''}`;
    if (path.startsWith('/generated-artifacts/')) return path;
    if (/^\/v1\/static-page-render-outputs\/[^/]+\/preview(?:[?#].*)?$/.test(path)) return path;
    if (/^\/v1\/external\/channels\/[^/]+\/static-page-renders\/[^/]+\/preview(?:[?#].*)?$/.test(path)) return path;
    return '';
  };
  if (raw.startsWith('/') && !raw.startsWith('//')) {
    return toAllowedPath({ pathname: raw.split(/[?#]/)[0], search: raw.match(/\?[^#]*/)?.[0] || '', hash: raw.match(/#.*$/)?.[0] || '' });
  }
  if (!/^https?:\/\//i.test(raw)) return '';
  try {
    const url = new URL(raw);
    if (typeof window !== 'undefined' && url.origin !== window.location.origin) {
      return '';
    }
    return toAllowedPath(url);
  } catch {
    return '';
  }
}

export function safeHtmlArtifactIdSegment(value, fallback = 'item') {
  return String(value || fallback)
    .trim()
    .replace(/[^a-zA-Z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 96) || fallback;
}

export function staticPagePublishedArtifactOwnerId(draft) {
  return draft?.backendDraftId || draft?.id || '';
}

export function buildStaticPagePublishedHtmlArtifact(draft) {
  const finalStatus = draft?.finalPage?.status || draft?.status || '';
  if (finalStatus !== 'rendered') return null;
  const previewPath = staticPageSafePreviewPath(staticPageRenderedUrlFromDraft(draft));
  if (!previewPath) return null;
  const finalPage = draft.finalPage || {};
  const assetManifest = finalPage.assetManifest || finalPage.asset_manifest || {};
  const reportTitle = assetManifest.reportTitle
    || assetManifest.report_title
    || assetManifest.displayTitle
    || assetManifest.display_title
    || assetManifest.title
    || finalPage.reportTitle
    || finalPage.report_title
    || finalPage.displayTitle
    || finalPage.display_title
    || draft.objective
    || draft.title
    || '静态页';
  const ownerId = staticPagePublishedArtifactOwnerId(draft);
  const renderOutputId = finalPage.renderOutputId || finalPage.render_output_id || '';
  const dataPath = staticPageSafePreviewPath(assetManifest.dataUrl || assetManifest.data_url || '');
  const snapshotPath = staticPageSafePreviewPath(assetManifest.dataSnapshotUrl || assetManifest.data_snapshot_url || '');
  return {
    kind: 'html_artifact',
    version: 1,
    id: `html-static-page-published-${safeHtmlArtifactIdSegment(ownerId || draft.id)}-${safeHtmlArtifactIdSegment(renderOutputId || previewPath, 'page')}`,
    title: `${reportTitle} · 成品`,
    sourceType: 'static_page',
    templateId: 'static_page_published_preview',
    interactionMode: 'read_only',
    ownerScope: {
      type: 'static_page_draft',
      id: ownerId || draft.id,
    },
    dataRefs: [
      draft.id ? { kind: 'local_static_page_draft', id: draft.id, label: '本地项目' } : null,
      draft.backendDraftId ? { kind: 'static_page_draft', id: draft.backendDraftId, label: '后端草稿' } : null,
      renderOutputId ? { kind: 'static_page_render_output', id: renderOutputId, label: 'Render Output' } : null,
    ].filter(Boolean),
    provenance: {
      producer: finalPage.renderer || 'v3-static-page-publisher',
      reason: 'published static page preview',
      sourceRunId: draft.assistantRunId || draft.source?.assistantRunId || '',
    },
    createdAt: draft.backendUpdatedAt || draft.updated_at || draft.updatedAt || draft.created_at || new Date(0).toISOString(),
    payload: {
      status: 'rendered',
      draftId: draft.id || '',
      backendDraftId: draft.backendDraftId || '',
      renderOutputId,
      previewPath,
      dataPath,
      snapshotPath,
      summary: draft.modelSummary || finalPage.notice || '页面已生成，可在主站内预览并继续通过对话修改。',
      reportTitle,
      report_title: reportTitle,
    },
  };
}

export function firstReportAssetPath(assetManifest = {}) {
  if (!assetManifest || typeof assetManifest !== 'object') return '';
  if (typeof assetManifest.path === 'string' && assetManifest.path.trim()) return assetManifest.path.trim();
  const assets = Array.isArray(assetManifest.assets) ? assetManifest.assets : [];
  const firstAsset = assets.find((asset) => asset && typeof asset.path === 'string' && asset.path.trim());
  return firstAsset?.path?.trim() || '';
}

export function reportAssetKind(assetManifest = {}) {
  if (!assetManifest || typeof assetManifest !== 'object') return '';
  if (typeof assetManifest.kind === 'string' && assetManifest.kind.trim()) return assetManifest.kind.trim();
  const path = firstReportAssetPath(assetManifest).toLowerCase();
  if (path.endsWith('.html')) return 'html';
  if (path.endsWith('.pdf')) return 'pdf';
  if (path.endsWith('.json')) return 'json';
  return path ? 'asset' : '';
}

export function buildReportRenderHtmlArtifact(output, plan) {
  if (!output || !plan) return null;
  const id = output.id || output.report_render_output_id;
  if (!id) return null;
  const assetPath = firstReportAssetPath(output.asset_manifest);
  const publishable = output.status === 'rendered' && Boolean(assetPath);
  return {
    kind: 'html_artifact',
    version: 1,
    id: `html-report-render-${id}`,
    title: `${plan.title || '报告'} · 渲染摘要`,
    sourceType: 'report',
    templateId: 'report_render_summary',
    interactionMode: 'read_only',
    ownerScope: {
      type: 'report_render_output',
      id,
    },
    dataRefs: [
      { kind: 'report_plan', id: output.plan_id || plan.id, label: 'Report Plan' },
      { kind: 'report_render_output', id, label: 'Render Output' },
      output.execution_id ? { kind: 'workflow_execution', id: output.execution_id, label: 'Workflow' } : null,
    ].filter(Boolean),
    provenance: {
      producer: 'v3-report-runtime',
      reason: 'report render output summary',
      sourceRunId: '',
    },
    createdAt: output.created_at || new Date(0).toISOString(),
    payload: {
      reportTitle: plan.title || '报告',
      objective: plan.objective || '',
      surface: output.surface || 'pc',
      status: output.status || 'unknown',
      publishable,
      assetPath,
      assetKind: reportAssetKind(output.asset_manifest),
      reportPlanId: output.plan_id || plan.id || '',
      reportRenderOutputId: id,
      workflowExecutionId: output.execution_id || '',
      astVersionId: output.ast_version_id || '',
      modelFacing: output.model_facing || null,
      serviceHandoff: output.service_handoff || plan.service_handoff || null,
      warnings: publishable
        ? []
        : [{
          title: output.status === 'failed' ? '渲染失败' : '尚不可发布',
          detail: output.status === 'failed'
            ? '需要重试渲染或检查 report-render-worker 写回的 asset manifest。'
            : '报告还没有可发布资产路径，先等待渲染完成或重新发起渲染。',
        }],
    },
  };
}

export function mergeHtmlArtifacts(...groups) {
  const artifactMap = new Map();
  groups.flat().filter(Boolean).forEach((artifact) => {
    const id = artifact.id || artifact.artifact_id;
    if (!id || artifactMap.has(id)) return;
    artifactMap.set(id, artifact);
  });
  return Array.from(artifactMap.values()).sort((left, right) => {
    const leftValue = new Date(left?.createdAt || left?.created_at || 0).getTime();
    const rightValue = new Date(right?.createdAt || right?.created_at || 0).getTime();
    return rightValue - leftValue;
  });
}

export function isReportRenderHtmlArtifact(artifact) {
  return (artifact?.templateId || artifact?.template_id) === 'report_render_summary';
}

export function replaceReportRenderHtmlArtifacts(existingArtifacts, reportArtifacts) {
  return mergeHtmlArtifacts(
    (Array.isArray(existingArtifacts) ? existingArtifacts : []).filter((artifact) => !isReportRenderHtmlArtifact(artifact)),
    Array.isArray(reportArtifacts) ? reportArtifacts : [],
  );
}
