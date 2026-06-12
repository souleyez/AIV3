import { staticPageRenderedUrlFromDraft } from './html-artifact-utils.js';
import { publishedReportForPlan } from './report-template-utils.js';
import {
  staticPageDraftArtifactKey,
  staticPageDraftBaselineStatus,
} from './static-page-report-shelf.js';

export function staticPageDraftIsDataReportArtifact(draft) {
  return /template:data-report(?:\||$)/.test(staticPageDraftArtifactKey(draft));
}

export function shouldAnnounceStaticPageRendered(draft) {
  return !staticPageDraftIsDataReportArtifact(draft);
}

export function staticPageDraftAsyncSnapshot(draft) {
  const jobStatus = String(draft?.imageJob?.status || draft?.previewContract?.status || '').toLowerCase();
  const draftStatus = String(draft?.status || '').toLowerCase();
  const finalStatus = String(draft?.finalPage?.status || '').toLowerCase();
  const previewAssetKey = String(draft?.previewImage?.assetKey || draft?.previewContract?.assetKey || '').trim();
  const jobId = String(draft?.imageJob?.id || draft?.previewContract?.imageJobId || '').trim();
  const renderOutputId = String(draft?.finalPage?.renderOutputId || draft?.finalPage?.render_output_id || '').trim();
  const finalUrl = staticPageRenderedUrlFromDraft(draft);
  return {
    jobStatus,
    draftStatus,
    finalStatus,
    previewAssetKey,
    jobId,
    renderOutputId,
    finalUrl,
    previewReady: jobStatus === 'preview_ready' || draftStatus === 'preview_ready',
    renderInProgress: finalStatus === 'queued' || finalStatus === 'rendering',
    rendered: finalStatus === 'rendered' || draftStatus === 'rendered',
  };
}

export function isReusableStaticPageReportDraft(draft) {
  if (!draft) {
    return false;
  }
  const stale = draft?.previewContract?.status === 'stale' || draft?.imageJob?.status === 'stale';
  const snapshot = staticPageDraftAsyncSnapshot(draft);
  if (staticPageDraftBaselineStatus(draft) === 'retired') {
    return false;
  }
  if (staticPageDraftIsDataReportArtifact(draft)) {
    return false;
  }
  const artifactKey = staticPageDraftArtifactKey(draft);
  const highQualityTemplate = /template:generated-static-page/.test(artifactKey)
    || /template:dashboard/.test(artifactKey)
    || /复用默认模板|按这个模板|新百经营分析月报/.test(artifactKey);
  return Boolean(!stale && snapshot.rendered && snapshot.finalUrl && highQualityTemplate);
}

export function isVisibleReportShelfStaticPageDraft(draft) {
  if (!draft) {
    return false;
  }
  const stale = draft?.previewContract?.status === 'stale' || draft?.imageJob?.status === 'stale';
  const snapshot = staticPageDraftAsyncSnapshot(draft);
  return Boolean(
    !stale
      && snapshot.rendered
      && snapshot.finalUrl,
  );
}

export function findReusableReportTemplate(reportPlans = [], publishedReports = [], staticPageDrafts = []) {
  const plans = Array.isArray(reportPlans) ? reportPlans : [];
  const published = Array.isArray(publishedReports) ? publishedReports : [];
  const staticDrafts = Array.isArray(staticPageDrafts) ? staticPageDrafts : [];
  const planWithPublished = plans
    .map((plan) => ({ plan, published: publishedReportForPlan(plan, published) }))
    .find((candidate) => candidate.published);
  if (planWithPublished) {
    return planWithPublished;
  }
  const publishedOnly = published.find(Boolean);
  if (publishedOnly) {
    return { plan: null, published: publishedOnly };
  }
  const renderedStaticPage = staticDrafts.find((draft) => isReusableStaticPageReportDraft(draft));
  if (renderedStaticPage) {
    return { plan: null, published: null, draft: renderedStaticPage };
  }
  const plannedTemplate = plans.find((plan) => (
    plan?.current_ast_version_id
      || plan?.currentAstVersionId
      || ['planned', 'rendered', 'published'].includes(String(plan?.status || '').toLowerCase())
  ));
  return plannedTemplate ? { plan: plannedTemplate, published: null } : null;
}

export function sortStaticPageDrafts(items) {
  return [...(Array.isArray(items) ? items : [])].sort((left, right) => {
    const leftValue = new Date(left?.backendUpdatedAt || left?.updated_at || left?.updatedAt || left?.created_at || 0).getTime();
    const rightValue = new Date(right?.backendUpdatedAt || right?.updated_at || right?.updatedAt || right?.created_at || 0).getTime();
    return rightValue - leftValue;
  });
}

export function staticPageDraftDiscoveryId(draft) {
  return String(
    draft?.localDraftId
      || draft?.local_draft_id
      || draft?.source?.localDraftId
      || draft?.source?.local_draft_id
      || draft?.source_refs?.local_draft_id
      || draft?.id
      || draft?.backendDraftId
      || '',
  ).trim();
}

export function staticPageDraftReadyForAutoRender(draft) {
  const snapshot = staticPageDraftAsyncSnapshot(draft);
  return Boolean(
    draft?.backendDraftId
      && snapshot.previewReady
      && snapshot.previewAssetKey
      && !snapshot.renderInProgress
      && !snapshot.rendered,
  );
}
