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

const INTERNAL_HTML_ARTIFACT_TEMPLATE_IDS = new Set([
  'static_page_planning_handoff',
  'static_page_data_quality_report',
  'codex_execution_report',
  'wechat_video_login_handoff',
]);

function htmlArtifactTemplateId(artifact = {}) {
  return String(
    artifact.templateId
      || artifact.template_id
      || artifact.manifest?.templateId
      || artifact.manifest?.template_id
      || '',
  ).trim();
}

export function isCustomerFacingHtmlArtifact(artifact) {
  if (!artifact) return false;
  return !INTERNAL_HTML_ARTIFACT_TEMPLATE_IDS.has(htmlArtifactTemplateId(artifact));
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

export function findStaticPageDraftByAnyId(draftId, draftsById = {}, draftItems = []) {
  if (!draftId) return null;
  return draftsById?.[draftId]
    || (Array.isArray(draftItems)
      ? draftItems.find((draft) => draft?.id === draftId || draft?.backendDraftId === draftId)
      : null)
    || null;
}

export function htmlArtifactOwnerScope(artifact) {
  return artifact?.ownerScope || artifact?.owner_scope || {};
}

export function findPublishedStaticPageArtifactForDraft(draft, artifacts = []) {
  if (!draft) return null;
  const ownerIds = new Set([draft.id, draft.backendDraftId].filter(Boolean));
  return (Array.isArray(artifacts) ? artifacts : []).find((artifact) => {
    const templateId = artifact?.templateId || artifact?.template_id;
    if (templateId !== 'static_page_published_preview') return false;
    const ownerScope = htmlArtifactOwnerScope(artifact);
    return ownerScope?.type === 'static_page_draft' && ownerIds.has(ownerScope.id);
  }) || null;
}

export function buildCurrentAssistantArtifact({
  activeStaticPageDraft = null,
  activeHtmlArtifact = null,
  draftsById = {},
  draftItems = [],
} = {}) {
  if (activeStaticPageDraft) return activeStaticPageDraft;
  if (!activeHtmlArtifact) return null;

  const ownerScope = htmlArtifactOwnerScope(activeHtmlArtifact);
  const ownerDraftId = ownerScope?.type === 'static_page_draft'
    ? String(ownerScope.id || '').trim()
    : '';
  if (!ownerDraftId) {
    return activeHtmlArtifact;
  }

  const draft = findStaticPageDraftByAnyId(ownerDraftId, draftsById, draftItems);
  if (draft) return draft;

  const payload = activeHtmlArtifact.payload || activeHtmlArtifact.manifest?.payload || {};
  const publicUrl = payload.publicUrl
    || payload.public_url
    || payload.previewPath
    || payload.preview_path
    || '';
  const renderOutputId = payload.renderOutputId || payload.render_output_id || '';
  const title = payload.reportTitle
    || payload.report_title
    || activeHtmlArtifact.title
    || activeHtmlArtifact.name
    || '当前报表';

  return {
    kind: 'static_page_draft',
    type: 'static_page_draft',
    id: ownerDraftId,
    backendDraftId: ownerDraftId,
    title,
    status: payload.status || activeHtmlArtifact.status || 'rendered',
    finalPage: {
      status: payload.status || 'rendered',
      publicUrl,
      renderOutputId,
      assetManifest: {
        reportTitle: title,
      },
    },
    source: {
      htmlArtifactId: activeHtmlArtifact.id || activeHtmlArtifact.artifact_id || '',
      templateId: activeHtmlArtifact.templateId || activeHtmlArtifact.template_id || '',
      sourceType: activeHtmlArtifact.sourceType || activeHtmlArtifact.source_type || '',
    },
  };
}

function firstObjectValue(...values) {
  for (const value of values) {
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      return value;
    }
    if (Array.isArray(value)) {
      const found = value.find((item) => item && typeof item === 'object' && !Array.isArray(item));
      if (found) return found;
    }
  }
  return null;
}

export function compactStaticPageTemplateReference(draft) {
  const reference = firstObjectValue(
    draft?.templateReference,
    draft?.template_reference,
    draft?.designReferences,
    draft?.design_references,
    draft?.source?.templateReference,
    draft?.source?.template_reference,
    draft?.source?.templateReferences,
    draft?.source?.template_references,
  );
  if (!reference) return null;
  const providerPolicy = reference.providerPolicy || reference.provider_policy || {};
  return {
    source: reference.source || 'html-anything',
    templateId: reference.templateId || reference.template_id || reference.id || '',
    label: reference.label || reference.name || '',
    importPolicy: reference.importPolicy || reference.import_policy || '',
    styleDirection: reference.styleDirection || reference.style_direction || '',
    designIntent: reference.designIntent || reference.design_intent || '',
    promptHints: Array.isArray(reference.promptHints || reference.prompt_hints)
      ? (reference.promptHints || reference.prompt_hints).slice(0, 6)
      : [],
    forbiddenOutput: Array.isArray(providerPolicy.forbiddenOutput || providerPolicy.forbidden_output)
      ? (providerPolicy.forbiddenOutput || providerPolicy.forbidden_output).slice(0, 8)
      : [],
  };
}

export function compactStaticPageMissingEvidence(draft) {
  const missingEvidence = firstObjectValue(
    draft?.missingEvidence,
    draft?.missing_evidence,
    draft?.source?.missingEvidence,
    draft?.source?.missing_evidence,
  );
  if (!missingEvidence) return null;
  return {
    status: missingEvidence.status || 'unknown',
    items: Array.isArray(missingEvidence.items)
      ? missingEvidence.items.slice(0, 8).map((item) => ({
        code: item?.code || '',
        message: item?.message || '',
        recommendedAction: item?.recommendedAction || item?.recommended_action || '',
        detailTargetCount: item?.detailTargetCount || item?.detail_target_count || null,
      }))
      : [],
  };
}

function pushCompactText(out, value, limit = 80) {
  if (typeof value === 'string') {
    const text = value.trim().replace(/\s+/g, ' ').slice(0, limit);
    if (text && !out.includes(text)) {
      out.push(text);
    }
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item) => pushCompactText(out, item, limit));
  }
}

export function compactStaticPageStructureSignals(draft) {
  const raw = firstObjectValue(
    draft?.structureSignals,
    draft?.structure_signals,
    draft?.dataSnapshot?.structureSignals,
    draft?.dataSnapshot?.structure_signals,
    draft?.source?.structureSignals,
    draft?.source?.structure_signals,
  );
  if (!raw) return null;
  const sectionTitleHints = [];
  pushCompactText(sectionTitleHints, raw.sectionTitleHints || raw.section_title_hints);
  const fieldCandidates = Array.isArray(raw.fieldCandidates || raw.field_candidates)
    ? (raw.fieldCandidates || raw.field_candidates).slice(0, 4).map((candidate) => {
      pushCompactText(sectionTitleHints, candidate?.sectionTitleHints || candidate?.section_title_hints);
      return {
        sourceId: candidate?.sourceId || candidate?.source_id || '',
        fieldPath: candidate?.fieldPath || candidate?.field_path || '',
        label: candidate?.label || '',
        kind: candidate?.kind || '',
        sectionTitleHints: Array.isArray(candidate?.sectionTitleHints || candidate?.section_title_hints)
          ? (candidate.sectionTitleHints || candidate.section_title_hints).slice(0, 8)
          : [],
      };
    })
    : [];
  const boundModules = Array.isArray(raw.boundModules || raw.bound_modules)
    ? (raw.boundModules || raw.bound_modules).slice(0, 8).map((module) => ({
      moduleId: module?.moduleId || module?.module_id || '',
      title: module?.title || '',
      fieldPath: module?.fieldPath || module?.field_path || '',
      bindingQualityStatus: module?.bindingQualityStatus || module?.binding_quality_status || module?.status || '',
    }))
    : [];
  if (!sectionTitleHints.length && !fieldCandidates.length && !boundModules.length) {
    return null;
  }
  return {
    status: raw.status || (sectionTitleHints.length ? 'available' : 'none'),
    policy: raw.policy || 'source_structure_only_no_body_no_sample_rows',
    sectionTitleHints: sectionTitleHints.slice(0, 12),
    fieldCandidates,
    boundModules,
  };
}

export function buildStaticPagePlanningHtmlArtifact(draft) {
  if (!draft) return null;
  const id = draft.backendDraftId || draft.id || 'local-static-page-draft';
  const templateReference = compactStaticPageTemplateReference(draft);
  const missingEvidence = compactStaticPageMissingEvidence(draft);
  const structureSignals = compactStaticPageStructureSignals(draft);
  return {
    kind: 'html_artifact',
    version: 1,
    id: `html-static-page-handoff-${id}`,
    title: `${draft.objective || draft.title || '静态页规划'} · 交接`,
    sourceType: 'static_page',
    templateId: 'static_page_planning_handoff',
    interactionMode: 'read_only',
    ownerScope: {
      type: 'static_page_draft',
      id,
    },
    dataRefs: [
      draft.datasetId ? { kind: 'dataset', id: draft.datasetId, label: '选中数据集' } : null,
      draft.sessionId ? { kind: 'chat_session', id: draft.sessionId, label: '关联会话' } : null,
      draft.backendDraftId ? { kind: 'static_page_draft', id: draft.backendDraftId, label: '后端草稿' } : null,
    ].filter(Boolean),
    provenance: {
      producer: 'v3-static-page-workspace',
      reason: 'static page planning handoff',
      sourceRunId: draft.assistantRunId || draft.source?.assistantRunId || '',
    },
    createdAt: draft.backendUpdatedAt || draft.updated_at || draft.updatedAt || draft.created_at || new Date(0).toISOString(),
    payload: {
      objective: draft.objective || draft.title || '静态页规划',
      templateReference,
      evidenceSummary: draft.templateEvidenceSummary || draft.template_evidence_summary || draft.source?.templateEvidenceSummary || null,
      missingEvidence,
      structureSignals,
      visualBridge: {
        providerLane: 'gpt-image-2-cloudflare-queue',
        role: 'effect_preview_reference_only',
        rule: '可视化只锁定视觉方向和确认指纹；最终 HTML 由 Draft JSON、DataSnapshot、VisualSpec 和 renderer 生成。',
        status: draft.previewContract?.status || draft.imageJob?.status || 'not_requested',
        imageJobStatus: draft.imageJob?.status || 'not_requested',
        imageJobId: draft.imageJob?.id || '',
        previewAssetKey: draft.previewImage?.assetKey || draft.previewContract?.assetKey || '',
        draftFingerprint: draft.previewContract?.draftFingerprint || '',
        styleDirection: draft.styleDirection || '',
        renderModel: draft.renderSpec?.componentModel || '',
        finalRenderStatus: draft.finalPage?.status || 'not_requested',
      },
      modules: (Array.isArray(draft.modules) ? draft.modules : []).map((module) => ({
        id: module.id,
        title: module.title,
        content: module.content,
        dataBinding: module.dataBinding?.label || module.dataBinding?.fieldPath || '待绑定',
        visualizationType: module.visualizationType,
        layout: module.layout,
        dataQuality: module.dataQuality || module.dataQualityStatus || '',
      })),
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
