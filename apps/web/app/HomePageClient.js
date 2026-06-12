'use client';

import { startTransition, useEffect, useMemo, useRef, useState } from 'react';
import ChatPanel from './components/ChatPanel';
import HomeMobileShell from './components/HomeMobileShell';
import HomeWorkspaceToolbar from './components/HomeWorkspaceToolbar';
import InsightPanel from './components/InsightPanel';
import Sidebar from './components/Sidebar';
import WorkspaceDirectoryPanel from './components/WorkspaceDirectoryPanel';
import {
  buildClaimLocalDataPayload,
  buildDeviceFingerprint,
  buildKeyLoginPayload,
  buildRotateLocalKeyPayload,
  buildStartEmailAuthPayload,
  buildVerifyEmailAuthPayload,
  normalizeVerificationCode,
  summarizeAccountState,
  validateAccountEmail,
} from './lib/account-auth';
import {
  assistantRunErrorRunId,
  assistantRunFailureMessage,
  staticPagePreviewGateErrorMessage,
} from './lib/api-error';
import { buildAssistantRunProgress } from './lib/assistant-run-progress';
import { buildAssistantStartupBriefing } from './lib/assistant-startup-briefing';
import {
  appendArtifactLinkText,
  assistantRunStreamArtifactLink,
  assistantRunStreamDisplayText,
} from './lib/assistant-stream-content';
import { formatRelativeTime } from './lib/formatters';
import { fetchJson, fetchSseJson } from './lib/home-api-client';
import {
  isTerminalCodexCustomerTaskStatus,
  mergeCodexCustomerArtifactBundles,
  mergeCodexCustomerTasks,
  normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse,
  normalizeCodexCustomerTasksFromAssistantRunResponse,
  promptMayUseCustomerCodex,
} from './lib/codex-customer-artifacts';
import { buildDefaultConversationTitle } from './lib/conversation-title';
import { buildAutoDatasetIdentity } from './lib/dataset-identity';
import {
  documentDatasetIds,
  filterRecordsByDatasetIds,
  normalizeDatasetIds,
  sameDatasetIds,
  sortByDateDesc,
  sortDatasets,
  staticPageDraftDatasetIds,
} from './lib/dataset-record-scope';
import {
  clearLocalSecretState,
  readLocalAccountEmail,
  readLocalSecretBindingIds,
  readLocalSecretValue,
  writeLocalAccountEmail,
  writeLocalSecretState,
} from './lib/local-account-state';
import { readLocalActivityEvents, writeLocalActivityEvents } from './lib/local-activity-events';
import {
  createLocalThreadId,
  readLocalAssistantRunId,
  readLocalThreadId,
  writeLocalAssistantRunId,
  writeLocalThreadId,
} from './lib/local-browser-state';
import { fingerprintLocalSecret } from './lib/local-secret-fingerprint';
import { planAssistantScope, selectPlannerDatasetIds } from './lib/scope-planner';
import {
  applyStaticPageOperation,
  applyStaticPageOperations,
  buildInitialStaticPageDraft,
  buildMockStaticPagePreview,
  buildStaticPageImagePayload,
  buildStaticPageImagePromptText,
  canRequestStaticPageDirectHtml,
  canRequestStaticPageFinalRender,
  interpretStaticPagePrompt,
  staticPageDirectHtmlBlockReason,
  staticPageFinalRenderBlockReason,
  staticPagePreviewBlockReason,
} from './lib/static-page-draft';
import {
  buildLocalUploadObjectKey,
  buildUploadDatasetPayload,
  classifyUploadTarget,
  inferUploadMediaKind,
  isPublicUploadClassification,
  summarizeUploadClassification,
} from './lib/upload-classifier';
import {
  isLocalChatSessionOptionId,
  localChatSessionOptionId,
  localThreadIdFromSessionOptionId,
  readLocalChatMessages,
  readLocalChatSessions,
  shouldPersistLocalChatSession,
  upsertLocalChatSession,
  writeLocalChatMessages,
  writeLocalChatSessions,
} from './lib/local-chat-sessions';

const DATASET_POLL_INTERVAL_MS = 5000;
const MESSAGE_POLL_INTERVAL_MS = 3000;
const CATALOG_POLL_INTERVAL_MS = 12000;
const REPORT_DETAIL_POLL_INTERVAL_MS = 6000;
const STATIC_PAGE_SHELF_POLL_INTERVAL_MS = 12000;
const STATIC_PAGE_ACTIVE_JOB_POLL_INTERVAL_MS = 4000;
const STATIC_PAGE_ACTIVE_RENDER_POLL_INTERVAL_MS = 4000;
const ASSISTANT_RUN_CUSTOMER_CODEX_POLL_INTERVAL_MS = 2500;
const ASSISTANT_RUN_CUSTOMER_CODEX_POLL_ATTEMPTS = 8;
const LOCAL_UPLOAD_TIMEOUT_MS = 180000;
const UPLOAD_REGISTRATION_TIMEOUT_MS = 60000;
const STATIC_PAGE_QUEUE_MESSAGE = '资源正在排队，可以联系商务开通高级用户跳过等待。';

function wait(ms) {
  return new Promise((resolve) => {
    globalThis.setTimeout(resolve, ms);
  });
}

function reportPlanIdFromPublished(report) {
  return String(
    report?.report_plan_id
      || report?.reportPlanId
      || report?.plan_id
      || report?.planId
      || '',
  ).trim();
}

function publishedReportForPlan(plan, publishedReports = []) {
  if (!plan?.id) return null;
  return (Array.isArray(publishedReports) ? publishedReports : []).find((report) =>
    reportPlanIdFromPublished(report) === plan.id,
  ) || null;
}

function addArtifactUrlContainers(containers, seen, value) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || seen.has(value)) {
    return;
  }
  seen.add(value);
  containers.push(value);
}

function firstArtifactUrlFromObject(value) {
  const containers = [];
  const seen = new Set();
  addArtifactUrlContainers(containers, seen, value);
  for (let index = 0; index < containers.length; index += 1) {
    const container = containers[index];
    [
      container.asset_manifest,
      container.assetManifest,
      container.current_version,
      container.currentVersion,
      container.version,
      container.report,
      container.artifact,
      container.output,
      container.finalPage,
      container.final_page,
      container.publish_result,
      container.publishResult,
      container.template_reference,
      container.templateReference,
      container.relaxed_template_match,
      container.relaxedTemplateMatch,
    ].forEach((candidate) => addArtifactUrlContainers(containers, seen, candidate));
  }

  const urlKeys = [
    'public_url',
    'publicUrl',
    'primary_url',
    'primaryUrl',
    'generated_artifact_url',
    'generatedArtifactUrl',
    'artifact_public_url',
    'artifactPublicUrl',
    'html_preview_url',
    'htmlPreviewUrl',
    'html_download_url',
    'htmlDownloadUrl',
    'download_url',
    'downloadUrl',
    'baseline_public_url',
    'baselinePublicUrl',
    'template_url',
    'templateUrl',
    'url',
    'href',
  ];
  for (const container of containers) {
    for (const key of urlKeys) {
      const text = String(container?.[key] || '').trim();
      if (/^(https?:\/\/|\/)/i.test(text)) {
        return text;
      }
    }
    const artifactLinks = container?.artifact_links || container?.artifactLinks;
    if (Array.isArray(artifactLinks)) {
      const link = artifactLinks.map((item) => String(item || '').trim()).find((item) => /^(https?:\/\/|\/)/i.test(item));
      if (link) return link;
    }
    const links = container?.links;
    if (Array.isArray(links)) {
      const link = links
        .map((item) => String(item?.url || item?.href || item || '').trim())
        .find((item) => /^(https?:\/\/|\/)/i.test(item));
      if (link) return link;
    }
  }
  return '';
}

function reportTemplateTitle(candidate, fallback = '当前数据集报表模板') {
  const plan = candidate?.plan;
  const published = candidate?.published;
  const detail = candidate?.detail;
  const draft = candidate?.draft || candidate?.staticPageDraft || candidate?.staticDraft;
  const finalPage = draft?.finalPage || {};
  const manifest = finalPage.assetManifest || finalPage.asset_manifest || {};
  return detail?.current_version?.asset_manifest?.report_title
    || detail?.current_version?.asset_manifest?.reportTitle
    || detail?.currentVersion?.assetManifest?.reportTitle
    || published?.title
    || published?.report_title
    || published?.reportTitle
    || plan?.title
    || plan?.objective
    || manifest.reportTitle
    || manifest.report_title
    || manifest.displayTitle
    || manifest.display_title
    || manifest.title
    || finalPage.reportTitle
    || finalPage.report_title
    || finalPage.displayTitle
    || finalPage.display_title
    || draft?.title
    || draft?.objective
    || fallback;
}

function reportTemplateCandidateId(candidate) {
  const draft = candidate?.draft || candidate?.staticPageDraft || candidate?.staticDraft;
  return String(
    candidate?.plan?.id
      || candidate?.published?.id
      || candidate?.published?.report_id
      || candidate?.published?.reportId
      || draft?.backendDraftId
      || draft?.backendId
      || draft?.id
      || reportTemplateTitle(candidate),
  ).trim();
}

function reportTemplateUrl(candidate) {
  return firstArtifactUrlFromObject(candidate?.detail)
    || firstArtifactUrlFromObject(candidate?.published)
    || firstArtifactUrlFromObject(candidate?.plan)
    || firstArtifactUrlFromObject(candidate?.draft || candidate?.staticPageDraft || candidate?.staticDraft)
    || '';
}

function staticPageDraftArtifactKey(draft) {
  return String(
    draft?.source_refs?.artifact_stability?.dataset_artifact_key
      || draft?.source_refs?.artifact_stability?.datasetArtifactKey
      || draft?.sourceRefs?.artifactStability?.datasetArtifactKey
      || draft?.sourceRefs?.artifact_stability?.dataset_artifact_key
      || draft?.artifact_stability?.dataset_artifact_key
      || draft?.artifactStability?.datasetArtifactKey
      || draft?.draft_payload?.artifact_stability?.dataset_artifact_key
      || draft?.draft_payload?.artifactStability?.datasetArtifactKey
      || draft?.draftPayload?.artifactStability?.datasetArtifactKey
      || draft?.draftPayload?.artifact_stability?.dataset_artifact_key
      || draft?.source_refs?.dataset_artifact_key
      || draft?.sourceRefs?.datasetArtifactKey
      || draft?.dataset_artifact_key
      || draft?.datasetArtifactKey
      || '',
  ).trim();
}

function staticPageDraftBaselineStatus(draft) {
  return String(
    draft?.source_refs?.artifact_stability?.baseline_status
      || draft?.source_refs?.artifact_stability?.baselineStatus
      || draft?.sourceRefs?.artifactStability?.baselineStatus
      || draft?.sourceRefs?.artifact_stability?.baseline_status
      || draft?.artifact_stability?.baseline_status
      || draft?.artifactStability?.baselineStatus
      || draft?.draft_payload?.artifact_stability?.baseline_status
      || draft?.draft_payload?.artifactStability?.baselineStatus
      || draft?.draftPayload?.artifactStability?.baselineStatus
      || draft?.draftPayload?.artifact_stability?.baseline_status
      || draft?.finalPage?.baselineStatus
      || draft?.finalPage?.baseline_status
      || '',
  ).trim().toLowerCase();
}

function normalizeReportShelfDefaultValue(value) {
  if (typeof value === 'boolean') {
    return value;
  }
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase();
    if (['true', '1', 'yes', 'default', 'accepted', 'enabled'].includes(normalized)) {
      return true;
    }
    if (['false', '0', 'no', 'not_default', 'non_default', 'retired', 'disabled'].includes(normalized)) {
      return false;
    }
  }
  return null;
}

function mergeReportShelfDefaultEntries(target, value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return target;
  }
  Object.entries(value).forEach(([datasetId, enabled]) => {
    const normalizedDatasetId = String(datasetId || '').trim();
    const normalizedValue = normalizeReportShelfDefaultValue(enabled);
    if (normalizedDatasetId && normalizedValue !== null) {
      target[normalizedDatasetId] = normalizedValue;
    }
  });
  return target;
}

function staticPageDraftReportShelfDefaults(draft) {
  const defaults = {};
  [
    draft?.source_refs?.artifact_stability?.report_shelf_defaults,
    draft?.source_refs?.artifact_stability?.reportShelfDefaults,
    draft?.sourceRefs?.artifactStability?.reportShelfDefaults,
    draft?.sourceRefs?.artifact_stability?.report_shelf_defaults,
    draft?.source_refs?.report_shelf_defaults,
    draft?.sourceRefs?.reportShelfDefaults,
    draft?.artifact_stability?.report_shelf_defaults,
    draft?.artifactStability?.reportShelfDefaults,
    draft?.draft_payload?.artifact_stability?.report_shelf_defaults,
    draft?.draft_payload?.artifactStability?.reportShelfDefaults,
    draft?.draftPayload?.artifactStability?.reportShelfDefaults,
    draft?.draftPayload?.artifact_stability?.report_shelf_defaults,
    draft?.reportShelfDefaults,
    draft?.report_shelf_defaults,
  ].forEach((value) => mergeReportShelfDefaultEntries(defaults, value));
  return defaults;
}

function staticPageDraftIsDefaultForDatasetIds(draft, datasetIds) {
  const ids = normalizeDatasetIds(datasetIds);
  if (!draft || !ids.length) {
    return false;
  }
  const overrides = staticPageDraftReportShelfDefaults(draft);
  const relatedIdSet = new Set(staticPageDraftDatasetIds(draft));
  return ids.some((datasetId) => {
    if (Object.prototype.hasOwnProperty.call(overrides, datasetId)) {
      return overrides[datasetId] === true;
    }
    return staticPageDraftBaselineStatus(draft) !== 'retired' && relatedIdSet.has(datasetId);
  });
}

function staticPageDraftHasAnyReportShelfDefault(draft) {
  if (!draft) {
    return false;
  }
  const overrides = staticPageDraftReportShelfDefaults(draft);
  if (Object.values(overrides).some((enabled) => enabled === true)) {
    return true;
  }
  if (staticPageDraftBaselineStatus(draft) === 'retired') {
    return false;
  }
  return staticPageDraftDatasetIds(draft).some((datasetId) => overrides[datasetId] !== false);
}

function reportShelfDefaultTargetDatasetIds(draft, selectedDatasetIds) {
  const selectedIds = normalizeDatasetIds(selectedDatasetIds);
  if (selectedIds.length) {
    return selectedIds;
  }
  return staticPageDraftDatasetIds(draft);
}

function withStaticPageDraftReportShelfDefaults(draft, datasetIds, enabled) {
  const targetDatasetIds = normalizeDatasetIds(datasetIds);
  if (!draft || !targetDatasetIds.length) {
    return draft;
  }
  const nextDefaults = {
    ...staticPageDraftReportShelfDefaults(draft),
  };
  targetDatasetIds.forEach((datasetId) => {
    nextDefaults[datasetId] = Boolean(enabled);
  });

  const sourceRefs = {
    ...(draft.source_refs || draft.sourceRefs || {}),
  };
  const sourceStability = {
    ...(sourceRefs.artifact_stability || sourceRefs.artifactStability || {}),
    report_shelf_defaults: nextDefaults,
    reportShelfDefaults: nextDefaults,
    report_shelf_defaults_updated_at: new Date().toISOString(),
  };
  if (enabled) {
    sourceStability.baseline_status = 'accepted';
    sourceStability.baselineStatus = 'accepted';
  }
  sourceRefs.artifact_stability = sourceStability;
  sourceRefs.artifactStability = sourceStability;

  return {
    ...draft,
    source_refs: sourceRefs,
    sourceRefs,
    reportShelfDefaults: nextDefaults,
    report_shelf_defaults: nextDefaults,
  };
}

function staticPageDraftIsDataReportArtifact(draft) {
  return /template:data-report(?:\||$)/.test(staticPageDraftArtifactKey(draft));
}

function shouldAnnounceStaticPageRendered(draft) {
  return !staticPageDraftIsDataReportArtifact(draft);
}

function isReusableStaticPageReportDraft(draft) {
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

function isVisibleReportShelfStaticPageDraft(draft) {
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

function findReusableReportTemplate(reportPlans = [], publishedReports = [], staticPageDrafts = []) {
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

function sortStaticPageDrafts(items) {
  return [...(Array.isArray(items) ? items : [])].sort((left, right) => {
    const leftValue = new Date(left?.backendUpdatedAt || left?.updated_at || left?.updatedAt || left?.created_at || 0).getTime();
    const rightValue = new Date(right?.backendUpdatedAt || right?.updated_at || right?.updatedAt || right?.created_at || 0).getTime();
    return rightValue - leftValue;
  });
}

function staticPageDraftDiscoveryId(draft) {
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

function staticPageDraftAsyncSnapshot(draft) {
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

function staticPageDraftReadyForAutoRender(draft) {
  const snapshot = staticPageDraftAsyncSnapshot(draft);
  return Boolean(
    draft?.backendDraftId
      && snapshot.previewReady
      && snapshot.previewAssetKey
      && !snapshot.renderInProgress
      && !snapshot.rendered,
  );
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

function compactStaticPageTemplateReference(draft) {
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

function compactStaticPageMissingEvidence(draft) {
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

function compactStaticPageStructureSignals(draft) {
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

function staticPageRenderedUrlFromDraft(draftOrOutput) {
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

function staticPageSafePreviewPath(value) {
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

function safeHtmlArtifactIdSegment(value, fallback = 'item') {
  return String(value || fallback)
    .trim()
    .replace(/[^a-zA-Z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 96) || fallback;
}

function staticPagePublishedArtifactOwnerId(draft) {
  return draft?.backendDraftId || draft?.id || '';
}

function buildStaticPagePublishedHtmlArtifact(draft) {
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

function buildStaticPagePlanningHtmlArtifact(draft) {
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

function firstReportAssetPath(assetManifest = {}) {
  if (!assetManifest || typeof assetManifest !== 'object') return '';
  if (typeof assetManifest.path === 'string' && assetManifest.path.trim()) return assetManifest.path.trim();
  const assets = Array.isArray(assetManifest.assets) ? assetManifest.assets : [];
  const firstAsset = assets.find((asset) => asset && typeof asset.path === 'string' && asset.path.trim());
  return firstAsset?.path?.trim() || '';
}

function reportAssetKind(assetManifest = {}) {
  if (!assetManifest || typeof assetManifest !== 'object') return '';
  if (typeof assetManifest.kind === 'string' && assetManifest.kind.trim()) return assetManifest.kind.trim();
  const path = firstReportAssetPath(assetManifest).toLowerCase();
  if (path.endsWith('.html')) return 'html';
  if (path.endsWith('.pdf')) return 'pdf';
  if (path.endsWith('.json')) return 'json';
  return path ? 'asset' : '';
}

function buildReportRenderHtmlArtifact(output, plan) {
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

function mergeHtmlArtifacts(...groups) {
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

function isReportRenderHtmlArtifact(artifact) {
  return (artifact?.templateId || artifact?.template_id) === 'report_render_summary';
}

function replaceReportRenderHtmlArtifacts(existingArtifacts, reportArtifacts) {
  return mergeHtmlArtifacts(
    (Array.isArray(existingArtifacts) ? existingArtifacts : []).filter((artifact) => !isReportRenderHtmlArtifact(artifact)),
    Array.isArray(reportArtifacts) ? reportArtifacts : [],
  );
}

function datasetIdsFromScope(scope) {
  const datasets = Array.isArray(scope?.datasets)
    ? scope.datasets
    : Array.isArray(scope?.selected)
      ? scope.selected
      : [];
  return normalizeDatasetIds(
    datasets.map((item) => {
      if (typeof item === 'string') {
        return item;
      }
      return item?.id || item?.dataset_id || item?.datasetId || '';
    }),
  );
}

function uiDatasetIdsFromBackendScope(scope) {
  const policy = scope?.dataset_scope_policy
    || scope?.datasetScopePolicy
    || scope?.supply_policy?.candidatePolicy
    || scope?.supplyPolicy?.candidatePolicy;
  if (policy === 'all_visible_datasets_with_preselection_priority') {
    return normalizeDatasetIds(scope?.preferred_dataset_ids || scope?.preferredDatasetIds || []);
  }
  return datasetIdsFromScope(scope);
}

function firstDatasetIdFromScope(scope) {
  return datasetIdsFromScope(scope)[0] || '';
}

function scopeHintFromCandidates(candidates) {
  const labels = (Array.isArray(candidates) ? candidates : [])
    .map((candidate) => candidate?.label)
    .filter(Boolean)
    .slice(0, 3);
  return labels.length ? `已选中：${labels.join('、')}` : '';
}

function createLocalMessage(role, content) {
  return {
    id: `local-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    role,
    content,
    created_at: new Date().toISOString(),
  };
}

function promptRequestsAssistantContinue(prompt) {
  return /继续|接着|下一步|刚才|上面|之前|这个|那版|修改|调整|改成|换成|按计划|照这个|沿用|再来|继续吧/.test(String(prompt || ''));
}

function promptRequestsStaticPageEdit(prompt) {
  return /继续|接着|下一步|刚才|上面|之前|这个|那版|草稿|标题|文案|内容|数据|图表|布局|模块|调整|修改|改|换|突出|减少|增加|放大|缩小|移动|排序|风格|确认|效果图|导出|老板|高层|风险|柱状图|折线图|环图|看板|精简/.test(String(prompt || ''));
}

function promptRejectsStaticPageOutput(prompt) {
  const text = String(prompt || '');
  const compact = text.replace(/\s+/g, '');
  if (!compact) {
    return false;
  }
  const asksExistingTemplateDelivery = /(?:已有|现有|旧|原|上次|之前|模板|复用).{0,16}(?:链接|地址|页面|报表|模板|产物)|(?:链接|地址).{0,16}(?:已有|现有|模板|复用|原页面|旧页面)/.test(compact);
  if (asksExistingTemplateDelivery) {
    return false;
  }
  const negativeLead = /(?:不要|别|不用|无需|不需要|禁止|避免|先别|不要再)(?:生成|制作|创建|输出|发布|渲染|做|做成|给|提供|返回|出|产出)?(?:任何|新的|新)?(?:静态页|静态页面|页面|网页|html|HTML|可视化页|报表|看板|链接|页面链接|报表链接)/.test(compact);
  const negativeTail = /(?:静态页|静态页面|页面|网页|html|HTML|可视化页|报表|看板|链接|页面链接|报表链接)(?:也)?(?:不要|别|不用|无需|不需要|禁止|避免)(?:生成|制作|创建|输出|发布|渲染|做|做成|给|提供|返回|出|产出)?/.test(compact);
  return negativeLead || negativeTail;
}

export default function HomePageClient() {
  const [activePage, setActivePage] = useState('home');
  const [datasets, setDatasets] = useState([]);
  const [documents, setDocuments] = useState([]);
  const [reportPlans, setReportPlans] = useState([]);
  const [publishedReports, setPublishedReports] = useState([]);
  const [selectedDatasetId, setSelectedDatasetId] = useState(null);
  const [selectedDatasetIds, setSelectedDatasetIds] = useState([]);
  const [selectedReportPlanId, setSelectedReportPlanId] = useState(null);
  const [sessions, setSessions] = useState([]);
  const [outputs, setOutputs] = useState([]);
  const [reportRenderOutputs, setReportRenderOutputs] = useState([]);
  const [reportAstVersions, setReportAstVersions] = useState([]);
  const [publishedReportDetail, setPublishedReportDetail] = useState(null);
  const [selectedSessionId, setSelectedSessionId] = useState(null);
  const [composingNewSession, setComposingNewSession] = useState(false);
  const [draftSessionStartedAt, setDraftSessionStartedAt] = useState(() => new Date().toISOString());
  const [draftSessionTitle, setDraftSessionTitle] = useState('');
  const [messages, setMessages] = useState([]);
  const [localMessages, setLocalMessages] = useState([]);
  const [localThreadId, setLocalThreadId] = useState('');
  const [localChatSessions, setLocalChatSessions] = useState([]);
  const [localChatStorageReady, setLocalChatStorageReady] = useState(false);
  const [input, setInput] = useState('');
  const [datasetDraft, setDatasetDraft] = useState({ key: '', title: '', secret: '' });
  const [localSecretDraft, setLocalSecretDraft] = useState('');
  const [activeSecretCount, setActiveSecretCount] = useState(0);
  const [accountEmailDraft, setAccountEmailDraft] = useState('');
  const [accountCodeDraft, setAccountCodeDraft] = useState('');
  const [accountNewKeyDraft, setAccountNewKeyDraft] = useState('');
  const [authSession, setAuthSession] = useState({ user: null, session: null });
  const [authChallenge, setAuthChallenge] = useState(null);
  const [authBusy, setAuthBusy] = useState(false);
  const [authMessage, setAuthMessage] = useState('');
  const [reportSurface, setReportSurface] = useState('pc');
  const [publishNote, setPublishNote] = useState('');
  const [mobileViewport, setMobileViewport] = useState(false);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const [mobilePanel, setMobilePanel] = useState('chat');
  const [bootstrapping, setBootstrapping] = useState(true);
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [messageLoading, setMessageLoading] = useState(false);
  const [documentsLoading, setDocumentsLoading] = useState(false);
  const [documentDetailLoading, setDocumentDetailLoading] = useState(false);
  const [reportDetailLoading, setReportDetailLoading] = useState(false);
  const [creatingDataset, setCreatingDataset] = useState(false);
  const [resolvingSecret, setResolvingSecret] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [uploadingFiles, setUploadingFiles] = useState(false);
  const [reportEntryBusy, setReportEntryBusy] = useState(false);
  const [reportActionBusy, setReportActionBusy] = useState('');
  const [datasetActionBusy, setDatasetActionBusy] = useState('');
  const [documentActionBusy, setDocumentActionBusy] = useState('');
  const [banner, setBanner] = useState('');
  const [error, setError] = useState('');
  const [staticPageDrafts, setStaticPageDrafts] = useState({});
  const [activeStaticPageDraftId, setActiveStaticPageDraftId] = useState(null);
  const [staticPageEditorOpen, setStaticPageEditorOpen] = useState(false);
  const [staticPageActionBusy, setStaticPageActionBusy] = useState(false);
  const [backendHtmlArtifacts, setBackendHtmlArtifacts] = useState([]);
  const [activeHtmlArtifactId, setActiveHtmlArtifactId] = useState(null);
  const [scopePlan, setScopePlan] = useState({ candidates: [], hint: '' });
  const [activityEvents, setActivityEvents] = useState([]);
  const [lastAssistantRunId, setLastAssistantRunId] = useState('');
  const [assistantRunProgress, setAssistantRunProgress] = useState(null);
  const [codexCustomerTasks, setCodexCustomerTasks] = useState([]);
  const [codexCustomerArtifacts, setCodexCustomerArtifacts] = useState([]);
  const [documentSearch, setDocumentSearch] = useState('');
  const [selectedDocumentId, setSelectedDocumentId] = useState('');
  const [selectedDocumentDetail, setSelectedDocumentDetail] = useState(null);

  const datasetLoadIdRef = useRef(0);
  const messageLoadIdRef = useRef(0);
  const reportDetailLoadIdRef = useRef(0);
  const fileInputRef = useRef(null);
  const staticPageAutoRenderKeysRef = useRef(new Set());
  const staticPageProgressMessageKeysRef = useRef(new Set());
  const staticPageDraftStatusRef = useRef(new Map());
  const assistantRunCustomerCodexPollRef = useRef(0);
  const codexCustomerChatMessageKeysRef = useRef(new Set());
  const uiNoticeMessageKeysRef = useRef(new Set());

  const selectedDataset = useMemo(
    () => datasets.find((dataset) => dataset.id === selectedDatasetId) || null,
    [datasets, selectedDatasetId],
  );
  const selectedDatasets = useMemo(
    () => selectedDatasetIds
      .map((datasetId) => datasets.find((dataset) => dataset.id === datasetId))
      .filter(Boolean),
    [datasets, selectedDatasetIds],
  );
  const selectedDocument = useMemo(
    () => selectedDocumentDetail?.document
      || documents.find((document) => document.id === selectedDocumentId)
      || null,
    [documents, selectedDocumentDetail, selectedDocumentId],
  );
  const selectedSession = useMemo(
    () => sessions.find((session) => session.id === selectedSessionId) || null,
    [sessions, selectedSessionId],
  );
  const reportShelfDatasetIds = useMemo(
    () => normalizeDatasetIds(selectedDatasetIds.length ? selectedDatasetIds : selectedDatasetId ? [selectedDatasetId] : []),
    [selectedDatasetId, selectedDatasetIds],
  );
  const reportShelfVisibleDatasetIds = useMemo(
    () => normalizeDatasetIds(datasets.map((dataset) => dataset.id)),
    [datasets],
  );
  const reportShelfFetchDatasetIds = useMemo(
    () => normalizeDatasetIds([...reportShelfVisibleDatasetIds, ...reportShelfDatasetIds]),
    [reportShelfVisibleDatasetIds, reportShelfDatasetIds],
  );
  const datasetPublishedReports = useMemo(
    () => filterRecordsByDatasetIds(publishedReports, reportShelfDatasetIds),
    [publishedReports, reportShelfDatasetIds],
  );
  const datasetReportPlans = useMemo(
    () => filterRecordsByDatasetIds(reportPlans, reportShelfDatasetIds),
    [reportPlans, reportShelfDatasetIds],
  );
  const selectedReportPlan = useMemo(
    () => datasetReportPlans.find((plan) => plan.id === selectedReportPlanId) || null,
    [datasetReportPlans, selectedReportPlanId],
  );
  const activeStaticPageDraft = useMemo(
    () => staticPageDrafts[activeStaticPageDraftId] || null,
    [activeStaticPageDraftId, staticPageDrafts],
  );
  const staticPageDraftItems = useMemo(
    () => sortStaticPageDrafts(Object.values(staticPageDrafts).filter((draft) => {
      const status = String(draft?.status || draft?.backendStatus || '').toLowerCase();
      return status !== 'archived';
    })),
    [staticPageDrafts],
  );
  const reportShelfStaticPageDraftItems = useMemo(
    () => {
      return staticPageDraftItems
        .filter((draft) => isVisibleReportShelfStaticPageDraft(draft))
        .map((draft) => {
          const relatedDatasetIds = staticPageDraftDatasetIds(draft);
          const isDefaultForSelectedDataset = staticPageDraftIsDefaultForDatasetIds(draft, reportShelfDatasetIds);
          return {
            ...draft,
            reportShelfDefault: isDefaultForSelectedDataset,
            reportShelfRelatedDatasetIds: relatedDatasetIds,
          };
        })
        .sort((left, right) => {
          if (left.reportShelfDefault !== right.reportShelfDefault) {
            return left.reportShelfDefault ? -1 : 1;
          }
          const leftValue = new Date(left?.backendUpdatedAt || left?.updated_at || left?.updatedAt || left?.created_at || 0).getTime();
          const rightValue = new Date(right?.backendUpdatedAt || right?.updated_at || right?.updatedAt || right?.created_at || 0).getTime();
          return rightValue - leftValue;
        });
    },
    [reportShelfDatasetIds, staticPageDraftItems],
  );
  const htmlArtifacts = useMemo(
    () => mergeHtmlArtifacts(
      backendHtmlArtifacts,
      staticPageDraftItems.map(buildStaticPagePublishedHtmlArtifact).filter(Boolean),
      staticPageDraftItems.map(buildStaticPagePlanningHtmlArtifact).filter(Boolean),
      reportRenderOutputs.map((output) => buildReportRenderHtmlArtifact(output, selectedReportPlan)).filter(Boolean),
    ),
    [backendHtmlArtifacts, staticPageDraftItems, reportRenderOutputs, selectedReportPlan],
  );
  const activeHtmlArtifact = useMemo(
    () => htmlArtifacts.find((artifact) => artifact.id === activeHtmlArtifactId) || null,
    [activeHtmlArtifactId, htmlArtifacts],
  );
  const visibleMessages = useMemo(
    () => (selectedSessionId ? messages : localMessages),
    [localMessages, messages, selectedSessionId],
  );
  const currentConversationTitle = useMemo(() => {
    if (selectedSession) {
      return selectedSession.title || '当前对话';
    }
    if (draftSessionTitle.trim()) {
      return draftSessionTitle.trim();
    }
    const firstUserMessage = visibleMessages.find((message) => message.role === 'user')?.content || '';
    return buildDefaultConversationTitle(input || firstUserMessage || '新对话', draftSessionStartedAt);
  }, [draftSessionStartedAt, draftSessionTitle, input, selectedSession, visibleMessages]);
  const conversationMenuSessions = useMemo(() => {
    const localSessionOptions = localChatSessions
      .filter((session) => selectedSessionId || session.id !== localThreadId)
      .map((session) => ({
        id: localChatSessionOptionId(session.id),
        title: session.title || '本地对话',
        updated_at: session.updatedAt,
        meta: session.updatedAt ? `本地 · ${formatRelativeTime(session.updatedAt)}` : '本地对话',
        localOnly: true,
      }));
    return [
      ...localSessionOptions,
      ...sessions,
    ];
  }, [localChatSessions, localThreadId, selectedSessionId, sessions]);
  const assistantStartupBriefing = useMemo(
    () => buildAssistantStartupBriefing({
      datasets,
      documents,
      reportPlans,
      publishedReports,
      latestMessages: visibleMessages,
      activityEvents,
      selectedDataset,
      selectedDatasets,
      activeStaticPageDraft: isReusableStaticPageReportDraft(activeStaticPageDraft) ? activeStaticPageDraft : null,
      staticPageDrafts: reportShelfStaticPageDraftItems,
    }),
    [activityEvents, activeStaticPageDraft, reportShelfStaticPageDraftItems, datasets, documents, publishedReports, reportPlans, selectedDataset, selectedDatasets, visibleMessages],
  );
  const toolbarSourceItems = useMemo(
    () => selectedDatasets.map((dataset) => ({ name: dataset.title, status: 'healthy' })),
    [selectedDatasets],
  );
  const accountStatusSummary = useMemo(
    () => summarizeAccountState({
      user: authSession.user,
      session: authSession.session,
      activeSecretCount,
    }),
    [activeSecretCount, authSession],
  );

  function handlePageChange(nextPage) {
    setActivePage(nextPage);
  }

  function handleOpenDocumentPage(documentId) {
    if (!documentId) {
      return;
    }
    const document = documents.find((item) => item.id === documentId);
    const datasetIds = documentDatasetIds(document);
    if (datasetIds.length) {
      setSelectedDatasetId(datasetIds[0]);
      setSelectedDatasetIds(datasetIds);
    }
    setSelectedDocumentId(documentId);
    setActivePage('document-detail');
  }

  function handleFocusDocumentMembership(documentId) {
    if (!documentId) {
      setSelectedDocumentId('');
      setSelectedDocumentDetail(null);
      return;
    }
    const document = documents.find((item) => item.id === documentId);
    const datasetIds = documentDatasetIds(document);
    if (datasetIds.length) {
      setSelectedDatasetId(datasetIds[0]);
      setSelectedDatasetIds(datasetIds);
    }
    setSelectedDocumentId(documentId);
  }

  function handleClearDocumentSelection() {
    setSelectedDocumentId('');
    setSelectedDocumentDetail(null);
  }

  function promptRequestsCodexForward(prompt) {
    return /^cc(?:$|[\s:：,，.。;；-])/i.test(String(prompt || '').trimStart());
  }

  function promptRequestsStaticPage(prompt) {
    const text = String(prompt || '');
    const compact = text.replace(/\s+/g, '');
    if (promptRequestsCodexForward(text)) {
      return false;
    }
    if (promptRejectsStaticPageOutput(text)) {
      return false;
    }
    const hasCreateAction = /生成|制作|创建|输出|发布|渲染|出页面|出报表|做成|做个|做一个|做一份|改成|修改|调整/.test(compact);
    if (/是什么意思|什么含义|怎么计算|如何计算|为什么|口径|有哪些问题|什么问题/.test(compact) && !hasCreateAction) {
      return false;
    }
    if (/静态页|静态页面|页面规划|一页|生成页面|落地页|效果图|网页|html|HTML|可视化页|报表|看板/.test(text)) {
      return true;
    }
    const hasDataAnalysisTopic = /经营分析|经营工作分析|数据分析|业务分析|综合分析|多维分析/.test(compact)
      && /数据|数据集|数据库|指标|经营|销售|客流|租金|门店|店铺|品牌|收入|风险|趋势|排行|排名|明细|汇总|统计|新百|新世界/.test(compact);
    const hasAnalysisReportAction = /做一下|做一做|帮我|请|输出|整理|生成|制作|汇总|全面|完整|详细|系统|多维|多角度|图表|可视化|报告|报表|看板|清单|明细/.test(compact);
    if (hasDataAnalysisTopic && hasAnalysisReportAction) {
      return true;
    }
    const hasRiskIdentificationTopic = /风险识别/.test(compact) && !/风险识别系统/.test(compact);
    const hasBusinessReportTopic = /取高|经营状况|经营情况|经营状态|经营健康度|销售缺口|销售额缺口|需要助推|需助推|助推门店|门店助推|销售统计|门店统计|品牌统计|经营统计|风险门店|风险店铺/.test(compact)
      || hasRiskIdentificationTopic;
    const hasReportAction = /看看|看一下|查看|查一下|哪些|列|列出|统计|汇总|排行|排名|最新|本月|五月|5月|取高|经营状况|经营情况|销售缺口|销售额缺口|需要助推/.test(compact);
    return hasBusinessReportTopic && hasReportAction;
  }

  function buildStaticPageConversationSummary(prompt = '', options = {}) {
    const draftDataset = options.dataset || selectedDataset;
    const draftDatasets = Array.isArray(options.datasets) && options.datasets.length
      ? options.datasets
      : selectedDatasets;
    const draftSession = options.session || selectedSession;
    const sourceMessages = options.messages || visibleMessages;
    const draftDatasetIds = new Set([
      ...draftDatasets.map((dataset) => dataset.id).filter(Boolean),
      draftDataset?.id,
    ].filter(Boolean));
    const relatedDocuments = documents
      .filter((document) => documentDatasetIds(document).some((datasetId) => draftDatasetIds.has(datasetId)))
      .slice(0, 8);
    const recentMessages = sourceMessages
      .slice(-6)
      .map((message) => {
        const role = message.role === 'assistant' ? '助手' : '用户';
        return `${role}: ${String(message.content || '').replace(/\s+/g, ' ').trim().slice(0, 180)}`;
      })
      .filter((line) => !/^(助手|用户):\s*$/.test(line));
    const datasetLine = draftDatasets.length
      ? draftDatasets.map((dataset) => {
          const count = dataset.document_count ?? dataset.documentCount ?? dataset.documents_count ?? dataset.documentsCount;
          const status = dataset.parse_status_summary || dataset.parseStatusSummary || dataset.content_type_summary || dataset.contentTypeSummary || '';
          return `${dataset.title || dataset.key}${count ? `（${count}文档）` : ''}${status ? `/${status}` : ''}`;
        }).join('、')
      : draftDataset ? `${draftDataset.title || draftDataset.key}` : '未选数据集，按普通对话意图规划。';
    const documentLine = relatedDocuments
      .map((document) => document.title || document.name || document.filename || document.id)
      .filter(Boolean)
      .slice(0, 8)
      .join('、');
    const summaryParts = [
      '# 静态页规划输入',
      prompt ? `用户要求：${prompt}` : '',
      `数据集：${datasetLine}`,
      draftSession ? `会话：${draftSession.title}` : '',
      documentLine ? `文档线索：${documentLine}` : '',
      recentMessages.length ? `上下文：${recentMessages.join(' / ')}` : '',
    ].filter(Boolean);
    return summaryParts.join('\n');
  }

  function buildStaticPageContextFieldCandidates(options = {}) {
    const draftDataset = options.dataset || selectedDataset;
    const draftDatasets = Array.isArray(options.datasets) && options.datasets.length
      ? options.datasets
      : selectedDatasets;
    const datasetIds = new Set([
      ...draftDatasets.map((dataset) => dataset.id).filter(Boolean),
      draftDataset?.id,
    ].filter(Boolean));
    const hints = [];
    draftDatasets.forEach((dataset) => {
      if (dataset?.title || dataset?.key) hints.push(dataset.title || dataset.key);
    });
    if (!draftDatasets.length && (draftDataset?.title || draftDataset?.key)) {
      hints.push(draftDataset.title || draftDataset.key);
    }
    documents
      .filter((document) => documentDatasetIds(document).some((datasetId) => datasetIds.has(datasetId)))
      .slice(0, 10)
      .forEach((document) => {
        const title = document.title || document.name || document.filename;
        if (title && !hints.includes(title)) hints.push(title);
      });
    if (!hints.length) return [];
    return [{
      sourceId: 'evidence',
      fieldPath: 'retrieval.section_title_hints',
      label: '对话和文档标题线索',
      kind: 'section_titles',
      confidence: 0.52,
      sectionTitleHints: hints.slice(0, 12),
    }];
  }

  function buildStaticPageDraftSelectedScope(draft) {
    if (draft?.datasetId) {
      return {
        mode: 'selected_datasets',
        selected: [{ type: 'dataset', id: draft.datasetId }],
        datasets: [{ type: 'dataset', id: draft.datasetId }],
      };
    }
    return {
      mode: 'ordinary_chat',
      selected: [],
    };
  }

  function buildAssistantRunSelectedScope(datasetIds, scopePlan) {
    const scopeDatasetIds = normalizeDatasetIds(datasetIds);
    const conversationMemory = (scopePlan?.candidates || []).some((candidate) => candidate.type === 'conversation_memory')
      ? ['local-thread']
      : [];
    const intent = scopePlan?.intent || scopePlan?.supplyStrategy?.intent || 'ordinary_chat';
    const supplyPolicy = scopePlan?.supplyStrategy || {
      intent,
      historyPolicy: conversationMemory.length ? 'intent_gated_selected' : 'intent_gated',
      retrievalPolicy: scopeDatasetIds.length ? 'standard' : 'not_requested',
      preferDetail: false,
      noFakeData: true,
    };
    if (scopeDatasetIds.length) {
      return {
        mode: 'user_selected',
        datasets: scopeDatasetIds,
        selected: scopeDatasetIds.map((datasetId) => ({ type: 'dataset', id: datasetId })),
        conversation_memory: conversationMemory,
        intent,
        supply_policy: supplyPolicy,
      };
    }
    return {
      mode: 'ordinary_chat',
      datasets: [],
      selected: [],
      conversation_memory: conversationMemory,
      intent,
      supply_policy: supplyPolicy,
    };
  }

  function buildStaticPageDraftSourceRefs(draft) {
    return {
      local_thread_id: readLocalThreadId(),
      local_draft_id: draft?.localDraftId || draft?.id || '',
      dataset_id: draft?.datasetId || null,
      chat_session_id: draft?.sessionId || null,
      source: 'local_chat_static_page_image2_pipeline',
      client_source: draft?.source || {},
      auto_publish_generated_artifact: true,
      effect_image_confirmation_required: false,
      continue_to_publish_after_effect_image: true,
      fixed_task_template_id: 'static_page_image2_data_publish',
      customer_preview_delivery: 'stream_event_or_status_card',
    };
  }

  function appendStaticPageProgressMessage(key, content, options = {}) {
    const stableKey = `static-page:${key}`;
    setLocalMessages((current) => {
      if (staticPageProgressMessageKeysRef.current.has(stableKey)
        || current.some((message) => message?.metadata?.key === stableKey)) {
        return current;
      }
      staticPageProgressMessageKeysRef.current.add(stableKey);
      return [
        ...current,
        {
          ...createLocalMessage('assistant', content),
          metadata: {
            source: 'static_page_progress',
            key: stableKey,
            final: Boolean(options.final),
          },
        },
      ].slice(-40);
    });
  }

  function staticPagePreviewProgressContent(draft, snapshot = {}) {
    const previewUrl = String(
      snapshot.previewAssetKey
        || draft?.previewImage?.assetKey
        || draft?.previewContract?.assetKey
        || '',
    ).trim();
    const link = /^(https?:\/\/|\/)/i.test(previewUrl)
      ? `：[打开设计图](${previewUrl})`
      : '';
    return `设计图已生成${link}。DataMax 正在继续读取视觉稿并制作最终页面。`;
  }

  function markdownLabel(value, fallback = '产物') {
    return String(value || fallback)
      .replace(/[\r\n]+/g, ' ')
      .replace(/\]/g, '\\]')
      .trim()
      .slice(0, 80) || fallback;
  }

  function codexCustomerBundleChatContent(bundle) {
    if (!bundle) return '';
    const lines = [
      `Codex 产物：${bundle.title || '客户产物'}`,
    ];
    if (bundle.summary) {
      lines.push(bundle.summary);
    }
    const seenUrls = new Set();
    if (bundle.primaryUrl) {
      seenUrls.add(bundle.primaryUrl);
      lines.push(`[打开产物](${bundle.primaryUrl})`);
    }
    const fileLines = (Array.isArray(bundle.files) ? bundle.files : [])
      .slice(0, 6)
      .map((file) => {
        const title = markdownLabel(file?.title || file?.path, '文件');
        if (file?.publicUrl && !seenUrls.has(file.publicUrl)) {
          seenUrls.add(file.publicUrl);
          return `- [${title}](${file.publicUrl})`;
        }
        const path = file?.path && file.path !== file?.title ? `：${file.path}` : '';
        return `- ${title}${path}`;
      });
    if (fileLines.length) {
      lines.push('文件：', ...fileLines);
    }
    if (!bundle.published && bundle.requiresPublishValidation) {
      lines.push('产物已进入 DataMax 发布校验，公开链接生成后会继续回传。');
    }
    return lines.filter(Boolean).join('\n');
  }

  function codexCustomerTaskChatContent(task) {
    if (!task || !isTerminalCodexCustomerTaskStatus(task.status)) return '';
    const result = task.resultSummary;
    if (result) {
      const lines = [
        `Codex 回复：${result.title || task.title || '执行结果'}`,
        result.summary || task.summary || '',
      ];
      if (Array.isArray(result.findings) && result.findings.length) {
        lines.push('要点：', ...result.findings.slice(0, 5).map((item) => `- ${item}`));
      }
      if (Array.isArray(result.recommendedNextActions) && result.recommendedNextActions.length) {
        lines.push('下一步：', ...result.recommendedNextActions.slice(0, 4).map((item) => `- ${item}`));
      }
      if (Array.isArray(result.warnings) && result.warnings.length) {
        lines.push('注意：', ...result.warnings.slice(0, 3).map((item) => `- ${item}`));
      }
      return lines.filter(Boolean).join('\n');
    }
    if (task.status === 'completed') return '';
    return `${task.title || 'Codex 执行'}${task.statusLabel ? `（${task.statusLabel}）` : ''}：${task.summary || '任务已结束。'}`;
  }

  function appendCodexCustomerChatUpdates(bundles = [], tasks = []) {
    const nextMessages = [];
    bundles.forEach((bundle) => {
      const content = codexCustomerBundleChatContent(bundle);
      const key = `codex-artifact:${bundle?.id || content}`;
      if (content && key) {
        nextMessages.push({ key, content, source: 'codex_customer_artifact' });
      }
    });
    tasks.forEach((task) => {
      const content = codexCustomerTaskChatContent(task);
      const key = `codex-task:${task?.id || task?.workflowExecutionId || content}:${task?.status || 'terminal'}`;
      if (content && key) {
        nextMessages.push({ key, content, source: 'codex_customer_task' });
      }
    });
    if (!nextMessages.length) return;
    setLocalMessages((current) => {
      const appended = [];
      nextMessages.forEach((message) => {
        const stableKey = message.key;
        if (
          codexCustomerChatMessageKeysRef.current.has(stableKey)
          || current.some((item) => item?.metadata?.key === stableKey)
        ) {
          return;
        }
        codexCustomerChatMessageKeysRef.current.add(stableKey);
        appended.push({
          ...createLocalMessage('assistant', message.content),
          metadata: {
            source: message.source,
            key: stableKey,
          },
        });
      });
      return appended.length ? [...current, ...appended].slice(-40) : current;
    });
  }

  function appendUiNoticeMessage(kind, content) {
    const text = String(content || '').trim();
    if (!text) return;
    if (text === '已发送，助手正在后台处理；你可以继续输入。') {
      return;
    }
    const displayContent = kind === 'error' ? `提示：${text}` : text;
    const stableKey = `ui-notice:${kind}:${displayContent.replace(/\s+/g, ' ')}`;
    setLocalMessages((current) => {
      if (
        uiNoticeMessageKeysRef.current.has(stableKey)
        || current.some((message) => message?.metadata?.key === stableKey)
      ) {
        return current;
      }
      uiNoticeMessageKeysRef.current.add(stableKey);
      return [
        ...current,
        {
          ...createLocalMessage('assistant', displayContent),
          metadata: {
            source: 'ui_notice',
            tone: kind,
            key: stableKey,
          },
        },
      ].slice(-40);
    });
  }

  function staticPageRenderedUrl(draftOrOutput) {
    return staticPageRenderedUrlFromDraft(draftOrOutput);
  }

  function mergeBackendStaticPageDraft(localDraft, backendDraft) {
    const payload = backendDraft?.draft_payload && typeof backendDraft.draft_payload === 'object'
      ? backendDraft.draft_payload
      : {};
    const matchedDatasetIds = [
      ...new Set([
        ...(Array.isArray(localDraft?.matchedDatasetIds) ? localDraft.matchedDatasetIds : []),
        ...(Array.isArray(localDraft?.matched_dataset_ids) ? localDraft.matched_dataset_ids : []),
        ...(Array.isArray(payload?.matchedDatasetIds) ? payload.matchedDatasetIds : []),
        ...(Array.isArray(payload?.matched_dataset_ids) ? payload.matched_dataset_ids : []),
        ...(Array.isArray(backendDraft?.matchedDatasetIds) ? backendDraft.matchedDatasetIds : []),
        ...(Array.isArray(backendDraft?.matched_dataset_ids) ? backendDraft.matched_dataset_ids : []),
      ]),
    ];
    const merged = {
      ...localDraft,
      ...payload,
      id: backendDraft?.id || localDraft.id,
      localDraftId: localDraft.localDraftId || localDraft.id,
      backendDraftId: backendDraft?.id || localDraft.backendDraftId || '',
      assistantRunId: backendDraft?.assistant_run_id || localDraft.assistantRunId || '',
      backendStatus: backendDraft?.status || localDraft.backendStatus || '',
      backendUpdatedAt: backendDraft?.updated_at || localDraft.backendUpdatedAt || '',
      source_refs: backendDraft?.source_refs || localDraft.source_refs || payload.source_refs || null,
      sourceRefs: backendDraft?.source_refs || localDraft.sourceRefs || payload.sourceRefs || payload.source_refs || null,
      draft_payload: backendDraft?.draft_payload || localDraft.draft_payload || payload,
      draftPayload: backendDraft?.draft_payload || localDraft.draftPayload || payload,
      matchedDatasetIds,
    };
    if (backendDraft?.status === 'rendered' && merged.finalPage?.status === 'rendered') {
      merged.status = 'rendered';
    }
    if (backendDraft?.status === 'confirmed' && merged.status !== 'rendered') {
      merged.status = 'effect_confirmed';
    }
    return merged;
  }

  function normalizeBackendStaticPageDraft(backendDraft) {
    const payload = backendDraft?.draft_payload && typeof backendDraft.draft_payload === 'object'
      ? backendDraft.draft_payload
      : {};
    return mergeBackendStaticPageDraft({
      ...payload,
      id: payload.id || backendDraft?.id,
      localDraftId: payload.localDraftId || backendDraft?.source_refs?.local_draft_id || backendDraft?.id,
    }, backendDraft);
  }

  function mergeStaticPageRenderOutput(draft, renderOutput) {
    if (!draft || !renderOutput) {
      return draft;
    }
    const renderStatus = renderOutput.status || draft.finalPage?.status || 'rendered';
    const nextStatus = renderStatus === 'rendered'
      ? 'rendered'
      : ['queued', 'rendering'].includes(renderStatus)
        ? 'rendering'
        : draft.status;
    return {
      ...draft,
      status: nextStatus,
      finalPage: {
        ...(draft.finalPage || {}),
        status: renderStatus,
        renderer: 'platform-api-static-page-renderer',
        renderOutputId: renderOutput.id,
        imageJobId: renderOutput.image_job_id || draft.finalPage?.imageJobId || null,
        assetManifest: renderOutput.asset_manifest || draft.finalPage?.assetManifest || {},
        html: renderOutput.html || draft.finalPage?.html || '',
        htmlPreviewUrl: renderOutput.html_preview_url || renderOutput.htmlPreviewUrl || draft.finalPage?.htmlPreviewUrl || '',
        htmlDownloadUrl: renderOutput.html_download_url || renderOutput.htmlDownloadUrl || draft.finalPage?.htmlDownloadUrl || '',
        directHtml: Boolean(draft.finalPage?.directHtml || renderOutput.asset_manifest?.directHtml || renderOutput.asset_manifest?.direct_html),
      },
    };
  }

  function mergeStaticPageImageJob(draft, imageJob) {
    if (!draft || !imageJob) {
      return draft;
    }
    const status = imageJob.status === 'confirmed' ? 'confirmed' : imageJob.status;
    const nextStatus = (() => {
      if (draft.status === 'rendered' || draft.status === 'effect_confirmed') {
        return draft.status;
      }
      if (status === 'preview_ready') {
        return 'preview_ready';
      }
      if (status === 'confirmed') {
        return 'effect_confirmed';
      }
      if (status === 'queued' || status === 'running') {
        return 'queued';
      }
      if (status === 'failed') {
        return 'planning';
      }
      return draft.status;
    })();
    return {
      ...draft,
      status: nextStatus,
      imageJob: {
        ...(draft.imageJob || {}),
        id: imageJob.id,
        status,
        queuePosition: imageJob.queue_position ?? null,
        queueMessage: imageJob.failure_reason
          || (status === 'preview_ready' ? '可视化已生成，将自动继续制作页面。' : STATIC_PAGE_QUEUE_MESSAGE),
      },
      previewImage: imageJob.preview_asset_key
        ? buildConfirmedStaticPagePreview(draft, imageJob, draft.previewImage)
        : draft.previewImage,
      previewContract: imageJob.preview_asset_key && status === 'preview_ready'
        ? {
            ...(draft.previewContract || {}),
            status: 'preview_ready',
            imageJobId: imageJob.id,
            assetKey: imageJob.preview_asset_key,
            queuePosition: null,
          }
        : imageJob.preview_asset_key && status === 'confirmed'
          ? {
              ...(draft.previewContract || {}),
              status: 'confirmed',
              imageJobId: imageJob.id,
              assetKey: imageJob.preview_asset_key,
              queuePosition: null,
            }
          : draft.previewContract,
    };
  }

  function isBackendStaticPageImageJobId(jobId) {
    return Boolean(jobId) && !String(jobId).startsWith('mock-image-job-');
  }

  function staticPageImageJobQueueOperation(imageJob, fallback = {}) {
    return {
      type: 'queue_image_job',
      jobId: imageJob?.id || fallback.jobId || fallback.id,
      queuePosition: imageJob?.queue_position ?? imageJob?.queuePosition ?? fallback.queuePosition ?? null,
      queueMessage: fallback.queueMessage || STATIC_PAGE_QUEUE_MESSAGE,
    };
  }

  function staticPageOperationIsPromptOnly(operation = {}) {
    const payload = operation.imagePromptPayload || operation.image_prompt_payload || {};
    return Boolean(
      operation.promptOnly
        || operation.prompt_only
        || payload.promptOnly
        || payload.prompt_only,
    );
  }

  function buildPromptOnlyStaticPageQueueOperation(draft, operation = {}) {
    const prompt = String(
      operation.prompt
        || operation.promptText
        || operation.prompt_text
        || buildStaticPageImagePromptText(draft),
    ).trim();
    const providedPayload = operation.imagePromptPayload || operation.image_prompt_payload;
    const basePayload = providedPayload && typeof providedPayload === 'object' && !Array.isArray(providedPayload)
      ? providedPayload
      : buildStaticPageImagePayload(draft, {
        oneClick: Boolean(operation.oneClick || draft?.source?.oneClick),
        promptText: prompt,
        promptOnly: true,
      });
    const imagePromptPayload = {
      ...basePayload,
      prompt,
      promptText: prompt,
      prompt_text: prompt,
      promptOnly: true,
      prompt_only: true,
    };
    return {
      ...operation,
      type: 'queue_image_job',
      prompt,
      promptText: prompt,
      prompt_text: prompt,
      promptOnly: true,
      prompt_only: true,
      queueMessage: operation.queueMessage || STATIC_PAGE_QUEUE_MESSAGE,
      imagePromptPayload,
    };
  }

  function buildConfirmedStaticPagePreview(draft, imageJob, fallbackPreview = null) {
    return {
      ...(fallbackPreview || buildMockStaticPagePreview(draft)),
      kind: 'static-page-effect-preview',
      assetKey: imageJob?.preview_asset_key || fallbackPreview?.assetKey || `static-page-previews/${imageJob?.id || draft.id}.json`,
      imageJobId: imageJob?.id || draft?.imageJob?.id || null,
    };
  }

  function replaceDraftWithOperation(baseDraft, operation) {
    const draft = applyStaticPageOperation(baseDraft, operation);
    setStaticPageDrafts((current) => ({
      ...current,
      [draft.id]: draft,
    }));
    setActiveStaticPageDraftId(draft.id);
    return draft;
  }

  function replaceStaticPageDraft(previousId, draft) {
    setStaticPageDrafts((current) => {
      const next = { ...current };
      if (previousId && previousId !== draft.id) {
        delete next[previousId];
      }
      next[draft.id] = draft;
      return next;
    });
    setActiveStaticPageDraftId(draft.id);
  }

  async function createBackendStaticPageDraft(localDraft, { assistantRunId, prompt = '' } = {}) {
    if (!assistantRunId || localDraft?.backendDraftId) {
      return localDraft;
    }
    const templateReferenceId = localDraft?.templateReferenceId
      || localDraft?.designReferences?.[0]?.templateId
      || localDraft?.source?.templateReferences?.[0]?.templateId
      || null;
    const response = await fetchJson(`/api/v3/assistant-runs/${assistantRunId}/static-page-drafts`, {
      method: 'POST',
      body: {
        title: localDraft?.objective || '静态页草稿',
        prompt,
        template_reference_id: templateReferenceId,
        selected_scope: localDraft?.datasetId ? buildStaticPageDraftSelectedScope(localDraft) : null,
        source_refs: buildStaticPageDraftSourceRefs(localDraft),
        draft_payload: {
          ...localDraft,
          assistantRunId,
        },
      },
    });
    const draft = mergeBackendStaticPageDraft(localDraft, response?.draft);
    replaceStaticPageDraft(localDraft.id, draft);
    if (draft?.imageJob?.status === 'queued' && !isBackendStaticPageImageJobId(draft.imageJob.id)) {
      createBackendStaticPageImageJob(draft, { prompt, oneClick: true }).catch((syncError) => {
        setBanner(`静态页草稿已同步；可视化队列暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
      });
    }
    return draft;
  }

  function syncStaticPageDraftCreate(localDraft, options = {}) {
    createBackendStaticPageDraft(localDraft, options).catch((syncError) => {
      setBanner(`静态页草稿已保存在本地；后端草稿同步暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
    });
  }

  function syncStaticPageDraftOperations(previousDraft, nextDraft, operations = [], options = {}) {
    const backendDraftId = previousDraft?.backendDraftId || nextDraft?.backendDraftId;
    if (!backendDraftId) {
      return;
    }
    fetchJson(`/api/v3/static-page-drafts/${backendDraftId}/operations`, {
      method: 'POST',
      body: {
        prompt: options.prompt || null,
        summary: options.summary || null,
        operations,
        draft_payload: nextDraft,
      },
    }).then((response) => {
      const draft = mergeBackendStaticPageDraft(nextDraft, response?.draft);
      replaceStaticPageDraft(nextDraft.id, draft);
    }).catch((syncError) => {
      setBanner(`静态页修改已先保存在本地；后端操作同步暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
    });
  }

  function syncStaticPageDraftIntent(baseDraft, prompt) {
    if (!baseDraft?.backendDraftId) {
      return;
    }
    fetchJson(`/api/v3/static-page-drafts/${baseDraft.backendDraftId}/intent`, {
      method: 'POST',
      body: {
        prompt,
        draft_payload: baseDraft,
        messages: visibleMessages.slice(-8).map((message) => ({
          role: message.role,
          content: message.content,
        })),
      },
    }).then((response) => {
      const draft = mergeBackendStaticPageDraft(baseDraft, response?.draft);
      replaceStaticPageDraft(baseDraft.id, draft);
      setBanner(`已按后端意图解释刷新静态页规划：${response?.summary || '修改已应用'}。`);
    }).catch((syncError) => {
      setBanner(`静态页修改已先保存在本地；后端意图解释暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
    });
  }

  async function hydrateBackendStaticPageDraft(backendDraft) {
    let draft = normalizeBackendStaticPageDraft(backendDraft);
    const shouldLoadJobs = draft?.backendDraftId && (draft.imageJob?.id || ['queued', 'preview_ready', 'effect_confirmed'].includes(draft.status));
    const shouldLoadRenders = Boolean(draft?.backendDraftId);

    const [imageJobs, renderOutputs] = await Promise.all([
      shouldLoadJobs
        ? fetchJson(`/api/v3/static-page-drafts/${draft.backendDraftId}/image-jobs`).catch(() => [])
        : Promise.resolve([]),
      shouldLoadRenders
        ? fetchJson(`/api/v3/static-page-drafts/${draft.backendDraftId}/renders`).catch(() => [])
        : Promise.resolve([]),
    ]);

    const latestJob = Array.isArray(imageJobs) ? imageJobs[0] : null;
    const latestRender = Array.isArray(renderOutputs) ? renderOutputs[0] : null;
    draft = mergeStaticPageImageJob(draft, latestJob);
    draft = mergeStaticPageRenderOutput(draft, latestRender);
    return draft;
  }

  function upsertStaticPageDraftBatch(drafts = []) {
    const normalizedDrafts = (Array.isArray(drafts) ? drafts : [])
      .map((draft) => (draft?.draft_payload ? normalizeBackendStaticPageDraft(draft) : draft))
      .filter((draft) => draft?.id);
    if (!normalizedDrafts.length) {
      return;
    }
    setStaticPageDrafts((current) => {
      const next = { ...current };
      normalizedDrafts.forEach((draft) => {
        const existing = next[draft.id] || next[draft.backendDraftId] || {};
        const matchedDatasetIds = normalizeDatasetIds([
          ...(Array.isArray(existing.matchedDatasetIds) ? existing.matchedDatasetIds : []),
          ...(Array.isArray(existing.matched_dataset_ids) ? existing.matched_dataset_ids : []),
          ...(Array.isArray(draft.matchedDatasetIds) ? draft.matchedDatasetIds : []),
          ...(Array.isArray(draft.matched_dataset_ids) ? draft.matched_dataset_ids : []),
        ]);
        next[draft.id] = {
          ...existing,
          ...draft,
          finalPage: {
            ...(existing.finalPage || {}),
            ...(draft.finalPage || {}),
          },
          imageJob: draft.imageJob || existing.imageJob,
          previewContract: draft.previewContract || existing.previewContract,
          matchedDatasetIds,
        };
      });
      return next;
    });
  }

  async function refreshStaticPageDraftShelf(options = {}) {
    const { silent = true, datasetIds = reportShelfFetchDatasetIds } = options;
    const queries = [];
    const localThreadId = readLocalThreadId();
    const priorityDatasetIds = normalizeDatasetIds(reportShelfDatasetIds);
    const targetDatasetIds = normalizeDatasetIds([
      ...priorityDatasetIds,
      ...(Array.isArray(datasetIds) ? datasetIds : [datasetIds]),
      ...reportShelfVisibleDatasetIds,
    ]);
    const orderedDatasetIds = [
      ...priorityDatasetIds,
      ...targetDatasetIds.filter((datasetId) => !priorityDatasetIds.includes(datasetId)),
    ];
    queries.push({
      query: new URLSearchParams({
        visible_templates: 'true',
        limit: '60',
      }),
      datasetId: '',
    });
    orderedDatasetIds.forEach((datasetId) => {
      queries.push({
        query: new URLSearchParams({
          dataset_id: datasetId,
          limit: '12',
        }),
        datasetId,
      });
    });
    if (localThreadId) {
      queries.push({
        query: new URLSearchParams({
          local_thread_id: localThreadId,
          limit: '12',
        }),
        datasetId: '',
      });
    }
    try {
      const draftGroups = await Promise.all(
        queries.map(({ query, datasetId }) => fetchJson(`/api/v3/static-page-drafts?${query.toString()}`, { timeoutMs: 18000 })
          .then((items) => {
            const drafts = (Array.isArray(items) ? items : []).map((draft) => ({
              ...draft,
              matchedDatasetIds: normalizeDatasetIds([
                ...(Array.isArray(draft?.matchedDatasetIds) ? draft.matchedDatasetIds : []),
                datasetId,
              ]),
            }));
            upsertStaticPageDraftBatch(drafts);
            return { items: drafts, datasetId };
          })
          .catch(() => ({ items: [], datasetId }))),
      );
      const draftById = new Map();
      draftGroups.forEach(({ items, datasetId }) => {
        (Array.isArray(items) ? items : []).forEach((draft) => {
          const id = draft?.id || draft?.backendDraftId || draft?.backend_draft_id;
          if (!id) return;
          const existing = draftById.get(id) || {};
          const matchedDatasetIds = new Set([
            ...(Array.isArray(existing.matchedDatasetIds) ? existing.matchedDatasetIds : []),
            ...(Array.isArray(draft?.matchedDatasetIds) ? draft.matchedDatasetIds : []),
          ]);
          if (datasetId) {
            matchedDatasetIds.add(datasetId);
          }
          draftById.set(id, {
            ...existing,
            ...draft,
            matchedDatasetIds: [...matchedDatasetIds],
          });
        });
      });
      const backendDrafts = [...draftById.values()];
      Promise.all(
        (Array.isArray(backendDrafts) ? backendDrafts : []).map((item) => hydrateBackendStaticPageDraft(item)),
      ).then((hydratedDrafts) => {
        upsertStaticPageDraftBatch(hydratedDrafts);
      }).catch(() => {});
      if (!silent) {
        setBanner(backendDrafts.length ? `已刷新 ${backendDrafts.length} 个静态页草稿/成品。` : '当前终端还没有静态页草稿。');
      }
    } catch (shelfError) {
      if (!silent) {
        setBanner(`静态页草稿架暂不可用：${shelfError instanceof Error ? shelfError.message : '请求失败'}。`);
      }
    }
  }

  async function refreshHtmlArtifacts(options = {}) {
    const { silent = true } = options;
    const query = new URLSearchParams({
      local_thread_id: readLocalThreadId(),
      limit: '50',
    });
    try {
      const artifacts = await fetchJson(`/api/v3/html-artifacts?${query.toString()}`);
      setBackendHtmlArtifacts(Array.isArray(artifacts) ? artifacts : []);
      if (!silent) {
        setBanner(artifacts?.length ? `已刷新 ${artifacts.length} 个 HTML 产物。` : '当前终端还没有后端 HTML 产物。');
      }
    } catch (artifactError) {
      if (!silent) {
        setBanner(`HTML 产物暂不可用：${artifactError instanceof Error ? artifactError.message : '请求失败'}。`);
      }
    }
  }

  async function refreshBackendStaticPageDraft(backendDraftId, options = {}) {
    const { silent = true } = options;
    if (!backendDraftId) {
      return null;
    }
    try {
      const backendDraft = await fetchJson(`/api/v3/static-page-drafts/${backendDraftId}`);
      const hydratedDraft = await hydrateBackendStaticPageDraft(backendDraft);
      if (hydratedDraft?.id) {
        setStaticPageDrafts((current) => ({
          ...current,
          [hydratedDraft.id]: hydratedDraft,
        }));
      }
      return hydratedDraft;
    } catch (refreshError) {
      if (!silent) {
        setBanner(`静态页状态刷新暂不可用：${refreshError instanceof Error ? refreshError.message : '请求失败'}。`);
      }
      return null;
    }
  }

  async function createBackendStaticPageImageJob(baseDraft, operation = {}) {
    if (!baseDraft?.backendDraftId) {
      throw new Error('静态页草稿还没有同步到后端。');
    }
    const response = await fetchJson(`/api/v3/static-page-drafts/${baseDraft.backendDraftId}/image-jobs`, {
      method: 'POST',
      body: {
        prompt: operation.prompt || null,
        image_prompt_payload: operation.imagePromptPayload || buildStaticPageImagePayload(baseDraft, {
          oneClick: Boolean(operation.oneClick),
          promptText: operation.prompt || operation.promptText || operation.prompt_text || '',
          promptOnly: staticPageOperationIsPromptOnly(operation),
        }),
      },
    });
    const imageJob = response?.image_job;
    const latestDraft = staticPageDrafts[baseDraft.id] || staticPageDrafts[baseDraft.backendDraftId] || baseDraft;
    const draft = replaceDraftWithOperation(latestDraft, staticPageImageJobQueueOperation(imageJob, operation));
    appendStaticPageProgressMessage(
      `${baseDraft.id}:image-queued:${imageJob?.id || 'pending'}`,
      '已确认生图文案，可视化任务已入队。可视化只作为预览，完成后会自动继续生成静态页。',
    );
    setBanner(`可视化任务已进入资源队列，当前前方约 ${imageJob?.queue_position ?? 1} 个任务。`);
    return { imageJob, draft };
  }

  async function ensureBackendStaticPageImageJob(baseDraft, operation = {}) {
    if (isBackendStaticPageImageJobId(baseDraft?.imageJob?.id)) {
      return {
        imageJob: {
          id: baseDraft.imageJob.id,
          queue_position: baseDraft.imageJob.queuePosition ?? null,
          preview_asset_key: baseDraft.previewImage?.assetKey || null,
        },
        draft: baseDraft,
      };
    }
    return createBackendStaticPageImageJob(baseDraft, operation);
  }

  async function confirmBackendStaticPagePreview(baseDraft, operation = {}) {
    if (!baseDraft?.backendDraftId) {
      throw new Error('静态页草稿还没有同步到后端。');
    }
    const ensured = await ensureBackendStaticPageImageJob(baseDraft, operation);
    const imageJobId = ensured.imageJob?.id;
    if (!imageJobId) {
      throw new Error('可视化任务不存在。');
    }
    const draftWithJob = ensured.draft || baseDraft;
    const preview = operation.previewImage || draftWithJob.previewImage || buildMockStaticPagePreview(draftWithJob);
    const response = await fetchJson(`/api/v3/static-page-image-jobs/${imageJobId}/confirm`, {
      method: 'POST',
      body: {
        preview_asset_key: preview?.assetKey || null,
      },
    });
    const confirmedPreview = buildConfirmedStaticPagePreview(draftWithJob, response?.image_job, preview);
    const localConfirmed = applyStaticPageOperation(draftWithJob, {
      type: 'confirm_preview',
      previewImage: confirmedPreview,
      imageJobStatus: 'confirmed',
    });
    const merged = mergeBackendStaticPageDraft(localConfirmed, response?.draft);
    const draft = {
      ...merged,
      status: merged.status === 'rendered' ? 'rendered' : 'effect_confirmed',
      previewImage: confirmedPreview,
      imageJob: {
        ...localConfirmed.imageJob,
        id: response?.image_job?.id || imageJobId,
        status: 'confirmed',
        queuePosition: null,
        queueMessage: STATIC_PAGE_QUEUE_MESSAGE,
      },
    };
    replaceStaticPageDraft(baseDraft.id, draft);
    setBanner('可视化已确认，下一步可以按效果制作静态页。');
    return { imageJob: response?.image_job, draft };
  }

  async function createBackendStaticPageRender(baseDraft, operation = {}) {
    if (!baseDraft?.backendDraftId) {
      throw new Error('静态页草稿还没有同步到后端。');
    }
    const directHtml = Boolean(operation.directHtml || operation.finalPage?.directHtml);
    let draft = baseDraft;
    let imageJobId = isBackendStaticPageImageJobId(draft.imageJob?.id) ? draft.imageJob.id : '';
    if (!directHtml) {
      if (!imageJobId) {
        const ensured = await ensureBackendStaticPageImageJob(draft, operation);
        draft = ensured.draft || draft;
        imageJobId = ensured.imageJob?.id || draft.imageJob?.id || imageJobId;
      }
      if (!canRequestStaticPageFinalRender(draft)) {
        throw new Error(staticPageFinalRenderBlockReason(draft) || '可视化还未准备好。');
      }
    }
    appendStaticPageProgressMessage(
      `${draft.id}:render-requested:${directHtml ? 'direct-html' : imageJobId || 'image-ready'}`,
      directHtml
        ? '已进入快速 HTML 制作，DataMax 会先产出一个可打开的页面版本。'
        : '可视化已接上，正在把视觉稿和数据绑定为可访问静态页。',
    );
    const response = await fetchJson(`/api/v3/static-page-drafts/${draft.backendDraftId}/renders`, {
      method: 'POST',
      body: {
        image_job_id: directHtml ? null : imageJobId || null,
        background: !directHtml,
        direct_html: directHtml,
      },
    });
    const renderOutput = response?.render_output;
    const renderStatus = renderOutput?.status || 'queued';
    const rendered = applyStaticPageOperation(draft, {
      type: 'request_final_render',
      directHtml,
      finalPage: {
        status: renderStatus,
        renderer: 'platform-api-static-page-renderer',
        renderOutputId: renderOutput?.id || '',
        imageJobId: directHtml ? null : renderOutput?.image_job_id || imageJobId || null,
        assetManifest: renderOutput?.asset_manifest || {},
        html: renderOutput?.html || '',
        htmlPreviewUrl: renderOutput?.html_preview_url || renderOutput?.htmlPreviewUrl || '',
        htmlDownloadUrl: renderOutput?.html_download_url || renderOutput?.htmlDownloadUrl || '',
        directHtml,
      },
    });
    const merged = mergeBackendStaticPageDraft(rendered, response?.draft);
    const finalDraft = {
      ...merged,
      status: renderStatus === 'rendered' ? 'rendered' : 'rendering',
      finalPage: rendered.finalPage,
    };
    replaceStaticPageDraft(baseDraft.id, finalDraft);
    if (renderStatus === 'rendered' && shouldAnnounceStaticPageRendered(finalDraft)) {
      const finalUrl = staticPageRenderedUrl(renderOutput || finalDraft);
      appendStaticPageProgressMessage(
        `${draft.id}:rendered:${renderOutput?.id || finalUrl || 'ready'}`,
        finalUrl
          ? `报表页面已生成：${finalUrl}`
          : '报表页面已生成。',
        { final: true },
      );
    }
    setBanner(renderStatus === 'rendered'
      ? directHtml
        ? '快速 HTML 已生成，可以下载 index.html。'
        : '最终静态页已按可视化生成。'
      : '最终静态页已进入后台制作队列，可以继续聊天；完成后会保存在右侧成品栏。');
    if (typeof window !== 'undefined') {
      window.setTimeout(() => {
        refreshBackendStaticPageDraft(finalDraft.backendDraftId, { silent: true });
      }, 2400);
    }
    return { renderOutput, draft: finalDraft };
  }

  async function fetchPlanPublishedReport(planId) {
    try {
      return await fetchJson(`/api/v3/report-plans/${planId}/published-report`);
    } catch (loadError) {
      const message = loadError instanceof Error ? loadError.message : '';
      if (message.includes('published report') || message.includes('Published report')) {
        return null;
      }
      throw loadError;
    }
  }

  async function startWorkflowExecution(executionId) {
    if (!executionId) {
      return null;
    }

    return fetchJson(`/api/v3/workflow-executions/${executionId}/start`, {
      method: 'POST',
    });
  }

  async function refreshAuthSession(options = {}) {
    const { silent = false } = options;
    try {
      const response = await fetchJson('/api/v3/auth/session');
      const nextSession = {
        user: response?.user || null,
        session: response?.session || null,
      };
      setAuthSession(nextSession);
      if (nextSession.user?.email) {
        setAccountEmailDraft(nextSession.user.email);
        writeLocalAccountEmail(nextSession.user.email);
      }
      if (!silent) {
        setAuthMessage(nextSession.user?.email ? '已恢复当前账号会话。' : '当前未登录账号。');
      }
      return nextSession;
    } catch (sessionError) {
      if (!silent) {
        setAuthMessage(sessionError instanceof Error ? sessionError.message : '账号状态读取失败');
      }
      return { user: null, session: null };
    }
  }

  async function handleSendEmailCode() {
    const { email, valid } = validateAccountEmail(accountEmailDraft);
    if (!valid) {
      setError('请输入有效邮箱。');
      return;
    }

    setAuthBusy(true);
    setError('');
    try {
      const response = await fetchJson('/api/v3/auth/email/start', {
        method: 'POST',
        body: buildStartEmailAuthPayload(email, 'login', buildDeviceFingerprint()),
      });
      writeLocalAccountEmail(response.email || email);
      setAccountEmailDraft(response.email || email);
      setAuthChallenge(response);
      setAuthMessage(
        response.resend_after_seconds
          ? `验证码已发送；${response.resend_after_seconds} 秒内会复用本次验证码。`
          : '验证码已发送，请在邮箱里查看。',
      );
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : '发送验证码失败');
    } finally {
      setAuthBusy(false);
    }
  }

  async function handleVerifyEmailCode() {
    const { email, valid } = validateAccountEmail(accountEmailDraft);
    const code = normalizeVerificationCode(accountCodeDraft);
    if (!valid) {
      setError('请输入有效邮箱。');
      return;
    }
    if (!code) {
      setError('请输入验证码。');
      return;
    }

    setAuthBusy(true);
    setError('');
    try {
      const response = await fetchJson('/api/v3/auth/email/verify', {
        method: 'POST',
        body: buildVerifyEmailAuthPayload(
          email,
          code,
          authChallenge?.purpose || 'login',
          buildDeviceFingerprint(),
        ),
      });
      setAuthSession({ user: response.user, session: response.session });
      writeLocalAccountEmail(response.user?.email || email);
      setAccountEmailDraft(response.user?.email || email);
      setAccountCodeDraft('');
      setAuthChallenge(null);
      setAuthMessage('邮箱已登录；后续数据集、机器人和产物会跟随当前账号。');
      await Promise.all([
        refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true }),
        refreshStaticPageDraftShelf({ silent: true }),
        refreshHtmlArtifacts({ silent: true }),
      ]);
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : '验证码登录失败');
    } finally {
      setAuthBusy(false);
    }
  }

  async function handleLoginWithLocalKey() {
    const { email, valid } = validateAccountEmail(accountEmailDraft);
    const localKey = String(localSecretDraft || readLocalSecretValue()).trim();
    if (!valid) {
      setError('请输入有效邮箱。');
      return;
    }
    if (!localKey) {
      setError('请在本地密钥框输入密钥。');
      return;
    }

    setAuthBusy(true);
    setResolvingSecret(true);
    setError('');
    try {
      const response = await fetchJson('/api/v3/auth/key/login', {
        method: 'POST',
        body: buildKeyLoginPayload(email, localKey, buildDeviceFingerprint()),
      });
      const bindingIds = response.active_secret_binding_ids || [];
      writeLocalSecretState(localKey, bindingIds);
      setActiveSecretCount(bindingIds.length);
      setLocalSecretDraft('');
      setAuthSession({ user: response.user, session: response.session });
      writeLocalAccountEmail(response.user?.email || email);
      setAccountEmailDraft(response.user?.email || email);
      setAuthMessage(`邮箱密钥已登录，已启用 ${bindingIds.length} 个本地绑定。`);
      await Promise.all([
        refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true }),
        refreshStaticPageDraftShelf({ silent: true }),
        refreshHtmlArtifacts({ silent: true }),
      ]);
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : '邮箱密钥登录失败');
    } finally {
      setAuthBusy(false);
      setResolvingSecret(false);
    }
  }

  async function handleClaimLocalData() {
    const localKey = String(localSecretDraft || readLocalSecretValue()).trim();
    if (!authSession.user?.email) {
      setError('请先登录邮箱账号，再认领本地密钥数据。');
      return;
    }
    if (!localKey) {
      setError('请在本地密钥框输入要认领的旧密钥。');
      return;
    }

    setAuthBusy(true);
    setResolvingSecret(true);
    setError('');
    try {
      const fingerprint = await fingerprintLocalSecret(localKey);
      const response = await fetchJson('/api/v3/auth/claim-local-data', {
        method: 'POST',
        body: buildClaimLocalDataPayload(fingerprint),
      });
      const bindingIds = response.active_secret_binding_ids || [];
      writeLocalSecretState(localKey, bindingIds);
      setActiveSecretCount(bindingIds.length);
      setLocalSecretDraft('');
      const claimedCount = response.claimed_datasets?.length || 0;
      const skippedCount = response.skipped_owned_dataset_count || 0;
      setAuthMessage(
        claimedCount
          ? `已认领 ${claimedCount} 个本地密钥数据集${skippedCount ? `，跳过 ${skippedCount} 个已有归属的数据集` : ''}。`
          : '没有找到可认领的旧本地密钥数据集。',
      );
      await refreshCatalog({
        preferredDatasetId: response.claimed_datasets?.[0]?.id || selectedDatasetId,
        silent: true,
      });
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : '认领本地数据失败');
    } finally {
      setAuthBusy(false);
      setResolvingSecret(false);
    }
  }

  async function handleRotateLocalKey() {
    const newLocalKey = String(accountNewKeyDraft || '').trim();
    if (!authSession.user?.email) {
      setError('请先登录邮箱账号，再设置新本地密钥。');
      return;
    }
    if (!newLocalKey) {
      setError('请输入新的本地密钥。');
      return;
    }

    setAuthBusy(true);
    setResolvingSecret(true);
    setError('');
    try {
      const response = await fetchJson('/api/v3/auth/key/rotate', {
        method: 'POST',
        body: buildRotateLocalKeyPayload(newLocalKey, buildDeviceFingerprint()),
      });
      const bindingIds = response.active_secret_binding_ids || [];
      writeLocalSecretState(newLocalKey, bindingIds);
      setActiveSecretCount(bindingIds.length);
      setLocalSecretDraft('');
      setAccountNewKeyDraft('');
      if (response.user) {
        setAuthSession((current) => ({ ...current, user: response.user }));
      }
      setAuthMessage(
        bindingIds.length
          ? `已设置新本地密钥，并启用 ${bindingIds.length} 个匹配绑定。`
          : '已设置新本地密钥。旧密钥数据不会自动迁移，需要用旧密钥执行认领。',
      );
      await refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true });
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : '设置新本地密钥失败');
    } finally {
      setAuthBusy(false);
      setResolvingSecret(false);
    }
  }

  async function handleLogoutAccount() {
    setAuthBusy(true);
    setError('');
    try {
      await fetchJson('/api/v3/auth/logout', {
        method: 'POST',
      });
      clearLocalSecretState();
      setActiveSecretCount(0);
      setLocalSecretDraft('');
      setAuthSession({ user: null, session: null });
      setSelectedDatasetId(null);
      setSelectedDatasetIds([]);
      setAuthMessage('已退出账号，并清除当前浏览器的本地私密绑定。');
      await Promise.all([
        refreshCatalog({ silent: true }),
        refreshStaticPageDraftShelf({ silent: true }),
        refreshHtmlArtifacts({ silent: true }),
      ]);
    } catch (authError) {
      setError(authError instanceof Error ? authError.message : '退出账号失败');
    } finally {
      setAuthBusy(false);
    }
  }

  async function refreshCatalog(options = {}) {
    const { preferredDatasetId = null, silent = false } = options;
    if (!silent) {
      setBootstrapping(true);
    }

    try {
      const [datasetItems, planItems, reportItems, documentItems] = await Promise.all([
        fetchJson('/api/v3/datasets'),
        fetchJson('/api/v3/report-plans'),
        fetchJson('/api/v3/published-reports'),
        fetchJson('/api/v3/documents'),
      ]);

      const nextDatasets = sortDatasets(datasetItems);
      const nextReportPlans = Array.isArray(planItems) ? planItems : [];
      const nextPublishedReports = sortByDateDesc(reportItems, 'updated_at');
      const nextDocuments = Array.isArray(documentItems) ? sortByDateDesc(documentItems, 'updated_at') : [];
      const nextDatasetIdSet = new Set(nextDatasets.map((item) => item.id).filter(Boolean));

      startTransition(() => {
        setDatasets(nextDatasets);
        setReportPlans(nextReportPlans);
        setPublishedReports(nextPublishedReports);
        setDocuments(nextDocuments);
        setSelectedDatasetIds((current) => {
          const retained = normalizeDatasetIds(current).filter((datasetId) => nextDatasetIdSet.has(datasetId));
          if (preferredDatasetId && nextDatasetIdSet.has(preferredDatasetId)) {
            return normalizeDatasetIds([preferredDatasetId, ...retained]);
          }
          return retained;
        });
        setSelectedDatasetId((current) => {
          if (preferredDatasetId && nextDatasets.some((item) => item.id === preferredDatasetId)) {
            return preferredDatasetId;
          }
          if (current && nextDatasets.some((item) => item.id === current)) {
            return current;
          }
          return null;
        });
      });
      setError('');
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : '目录加载失败');
    } finally {
      if (!silent) {
        setBootstrapping(false);
      }
    }
  }

  async function refreshDocuments(options = {}) {
    const { silent = false } = options;
    if (!silent) {
      setDocumentsLoading(true);
    }

    try {
      const documentItems = await fetchJson('/api/v3/documents');
      setDocuments(Array.isArray(documentItems) ? sortByDateDesc(documentItems, 'updated_at') : []);
      setError('');
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : '文档列表加载失败');
    } finally {
      if (!silent) {
        setDocumentsLoading(false);
      }
    }
  }

  async function refreshDocumentDetail(documentId) {
    if (!documentId) {
      setSelectedDocumentDetail(null);
      return;
    }

    setDocumentDetailLoading(true);
    try {
      const detail = await fetchJson(`/api/v3/documents/${documentId}/detail`);
      setSelectedDocumentDetail(detail);
      setError('');
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : '文档解析详情加载失败');
    } finally {
      setDocumentDetailLoading(false);
    }
  }

  async function handleUpdateDataset(datasetId, updates) {
    if (!datasetId) {
      return;
    }
    setDatasetActionBusy(datasetId);
    try {
      const payload = {};
      if (typeof updates.title === 'string') payload.title = updates.title;
      if (typeof updates.description === 'string') payload.description = updates.description;
      if (typeof updates.lifecycle === 'string') payload.lifecycle = updates.lifecycle;
      const updated = await fetchJson(`/api/v3/datasets/${datasetId}`, {
        method: 'PATCH',
        body: JSON.stringify(payload),
      });
      setBanner(updated.lifecycle === 'archived' || updated.lifecycle === 'Archived'
        ? '数据集已归档。'
        : '数据集已更新。');
      if (payload.lifecycle === 'archived') {
        setSelectedDatasetId(null);
        setSelectedDatasetIds((current) => normalizeDatasetIds(current).filter((item) => item !== datasetId));
      }
      await refreshCatalog({
        preferredDatasetId: payload.lifecycle === 'archived' ? null : datasetId,
        silent: true,
      });
      setError('');
    } catch (updateError) {
      setError(updateError instanceof Error ? updateError.message : '数据集更新失败');
    } finally {
      setDatasetActionBusy('');
    }
  }

  async function handleArchiveDataset(datasetId) {
    await handleUpdateDataset(datasetId, { lifecycle: 'archived' });
  }

  async function handleUpdateDocument(documentId, updates) {
    if (!documentId) {
      return;
    }
    setDocumentActionBusy(documentId);
    try {
      const payload = {};
      if (typeof updates.title === 'string') payload.title = updates.title;
      if (typeof updates.lifecycle === 'string') payload.lifecycle = updates.lifecycle;
      await fetchJson(`/api/v3/documents/${documentId}`, {
        method: 'PATCH',
        body: JSON.stringify(payload),
      });
      setBanner(payload.lifecycle === 'archived' ? '文档已归档。' : '文档已更新。');
      await refreshDocuments({ silent: true });
      if (payload.lifecycle === 'archived') {
        setSelectedDocumentId((current) => (current === documentId ? '' : current));
        setSelectedDocumentDetail((current) => (
          current?.document?.id === documentId ? null : current
        ));
      } else {
        await refreshDocumentDetail(documentId);
      }
      setError('');
    } catch (updateError) {
      setError(updateError instanceof Error ? updateError.message : '文档更新失败');
    } finally {
      setDocumentActionBusy('');
    }
  }

  async function handleArchiveDocuments(documentIds) {
    const ids = [...new Set((documentIds || []).filter(Boolean))];
    if (!ids.length) {
      return;
    }
    setDocumentActionBusy('batch');
    try {
      await Promise.all(ids.map((documentId) => fetchJson(`/api/v3/documents/${documentId}`, {
        method: 'PATCH',
        body: JSON.stringify({ lifecycle: 'archived' }),
      })));
      setBanner(ids.length === 1 ? '文档已归档。' : `已归档 ${ids.length} 个文档。`);
      await refreshDocuments({ silent: true });
      if (ids.includes(selectedDocumentId)) {
        setSelectedDocumentId('');
        setSelectedDocumentDetail(null);
      }
      setError('');
    } catch (updateError) {
      setError(updateError instanceof Error ? updateError.message : '文档批量归档失败');
    } finally {
      setDocumentActionBusy('');
    }
  }

  async function refreshWorkspace(datasetId, options = {}) {
    const { preferredSessionId = null, silent = false, preserveNewSessionDraft = false } = options;
    const loadId = datasetLoadIdRef.current + 1;
    datasetLoadIdRef.current = loadId;

    if (!silent) {
      setWorkspaceLoading(true);
    }

    try {
      const [sessionItems, outputItems] = await Promise.all([
        fetchJson(`/api/v3/datasets/${datasetId}/chat-sessions`),
        fetchJson(`/api/v3/datasets/${datasetId}/outputs`),
      ]);

      if (datasetLoadIdRef.current !== loadId) {
        return;
      }

      const nextSessions = sortByDateDesc(sessionItems, 'updated_at');
      const nextOutputs = sortByDateDesc(outputItems, 'created_at');

      startTransition(() => {
        setSessions(nextSessions);
        setOutputs(nextOutputs);
        setSelectedSessionId((current) => {
          if (preferredSessionId && nextSessions.some((item) => item.id === preferredSessionId)) {
            return preferredSessionId;
          }
          if (current && nextSessions.some((item) => item.id === current)) {
            return current;
          }
          if (preserveNewSessionDraft) {
            return null;
          }
          return nextSessions[0]?.id || null;
        });
      });
      setError('');
    } catch (loadError) {
      if (datasetLoadIdRef.current !== loadId) {
        return;
      }
      setError(loadError instanceof Error ? loadError.message : '数据集工作区加载失败');
    } finally {
      if (datasetLoadIdRef.current === loadId && !silent) {
        setWorkspaceLoading(false);
      }
    }
  }

  async function refreshMessages(sessionId, options = {}) {
    const { silent = false } = options;
    const loadId = messageLoadIdRef.current + 1;
    messageLoadIdRef.current = loadId;

    if (!silent) {
      setMessageLoading(true);
    }

    try {
      const nextMessages = await fetchJson(`/api/v3/chat-sessions/${sessionId}/messages`);
      if (messageLoadIdRef.current !== loadId) {
        return;
      }
      startTransition(() => {
        setMessages(Array.isArray(nextMessages) ? nextMessages : []);
      });
      setError('');
    } catch (loadError) {
      if (messageLoadIdRef.current !== loadId) {
        return;
      }
      setError(loadError instanceof Error ? loadError.message : '会话消息加载失败');
    } finally {
      if (messageLoadIdRef.current === loadId && !silent) {
        setMessageLoading(false);
      }
    }
  }

  async function refreshReportDetail(planId, options = {}) {
    const { silent = false } = options;
    const loadId = reportDetailLoadIdRef.current + 1;
    reportDetailLoadIdRef.current = loadId;

    if (!silent) {
      setReportDetailLoading(true);
    }

    try {
      const [renderOutputItems, astVersionItems, publishedDetail, reportHtmlArtifacts] = await Promise.all([
        fetchJson(`/api/v3/report-plans/${planId}/render-outputs`),
        fetchJson(`/api/v3/report-plans/${planId}/ast-versions`),
        fetchPlanPublishedReport(planId),
        fetchJson(`/api/v3/html-artifacts?${new URLSearchParams({
          report_plan_id: planId,
          limit: '30',
        }).toString()}`).catch(() => []),
      ]);

      if (reportDetailLoadIdRef.current !== loadId) {
        return;
      }

      startTransition(() => {
        setReportRenderOutputs(sortByDateDesc(renderOutputItems, 'created_at'));
        setReportAstVersions(sortByDateDesc(astVersionItems, 'created_at'));
        setPublishedReportDetail(publishedDetail);
        setBackendHtmlArtifacts((current) => replaceReportRenderHtmlArtifacts(current, reportHtmlArtifacts));
      });
      setError('');
    } catch (loadError) {
      if (reportDetailLoadIdRef.current !== loadId) {
        return;
      }
      setError(loadError instanceof Error ? loadError.message : '报告详情加载失败');
    } finally {
      if (reportDetailLoadIdRef.current === loadId && !silent) {
        setReportDetailLoading(false);
      }
    }
  }

  async function handleCreateDataset() {
    const autoIdentity = buildAutoDatasetIdentity(datasets.length);
    const requestedTitle = datasetDraft.title.trim();
    if (!requestedTitle) {
      setError('请输入数据集名称后确认创建。');
      return;
    }
    const key = (datasetDraft.key.trim() || autoIdentity.key).toLowerCase();
    const title = requestedTitle || autoIdentity.title;
    const secret = String(datasetDraft.secret || '').trim();
    const signedIn = Boolean(authSession.user);

    setCreatingDataset(true);
    try {
      const fingerprint = secret ? await fingerprintLocalSecret(secret) : '';
      const dataset = await fetchJson('/api/v3/datasets', {
        method: 'POST',
        body: {
          key,
          title,
          description: signedIn ? '登录用户创建的数据集' : '未登录终端创建的本机公开数据集',
          ...(!signedIn ? { local_only: true, local_thread_id: readLocalThreadId() } : {}),
          ...(fingerprint ? { secret_fingerprint: fingerprint, secret_label: `local-${key}` } : {}),
        },
      });
      let secretNote = '';
      if (secret && dataset.secret_binding_ids?.length) {
        const nextBindingIds = [...readLocalSecretBindingIds(), ...dataset.secret_binding_ids];
        writeLocalSecretState(secret, nextBindingIds);
        setActiveSecretCount([...new Set(nextBindingIds)].length);
        secretNote = ' 已绑定本地密钥，后续请求会优先带当前密钥。';
      }
      setDatasetDraft({ key: '', title: '', secret: '' });
      setBanner(
        signedIn
          ? `已按当前登录用户创建数据集 ${dataset.title}。${secretNote}`
          : `已创建本机公开数据集 ${dataset.title}，仅当前浏览器默认可见。`,
      );
      if (dataset.id) {
        setSelectedDatasetIds((current) => normalizeDatasetIds([dataset.id, ...current]));
        setSelectedDatasetId(dataset.id);
      }
      await refreshCatalog({ preferredDatasetId: dataset.id, silent: true });
    } catch (createError) {
      setError(createError instanceof Error ? createError.message : '创建数据集失败');
    } finally {
      setCreatingDataset(false);
    }
  }

  async function handleResolveLocalSecret() {
    const secret = String(localSecretDraft || '').trim();
    if (!secret) {
      setError('请输入本地密钥。');
      return;
    }

    setResolvingSecret(true);
    setError('');
    try {
      const fingerprint = await fingerprintLocalSecret(secret);
      const response = await fetchJson('/api/v3/dataset-secret-bindings/resolve', {
        method: 'POST',
        body: { fingerprint },
      });
      const bindingIds = response.secret_binding_ids || [];
      if (!bindingIds.length) {
        clearLocalSecretState();
        setActiveSecretCount(0);
        setBanner('未找到匹配的私密数据集。');
        await refreshCatalog({ silent: true });
        return;
      }
      writeLocalSecretState(secret, bindingIds);
      setActiveSecretCount(bindingIds.length);
      setLocalSecretDraft('');
      const unlockedTitles = (response.datasets || []).map((dataset) => dataset.title).join('、');
      setBanner(`已解锁 ${bindingIds.length} 个本地绑定${unlockedTitles ? `：${unlockedTitles}` : ''}。`);
      await refreshCatalog({ preferredDatasetId: response.datasets?.[0]?.id || selectedDatasetId, silent: true });
    } catch (resolveError) {
      setError(resolveError instanceof Error ? resolveError.message : '解锁本地密钥失败');
    } finally {
      setResolvingSecret(false);
    }
  }

  async function handleBindSelectedDatasetSecret() {
    const secret = String(localSecretDraft || '').trim();
    if (!selectedDataset?.id) {
      setError('请先选择要绑定为私密的数据集。');
      return;
    }
    if (!secret) {
      setError('请输入本地密钥。');
      return;
    }

    setResolvingSecret(true);
    setError('');
    try {
      const fingerprint = await fingerprintLocalSecret(secret);
      const response = await fetchJson('/api/v3/dataset-secret-bindings', {
        method: 'POST',
        body: {
          dataset_id: selectedDataset.id,
          fingerprint,
          label: `local-${selectedDataset.key || selectedDataset.title || 'dataset'}`,
        },
      });
      const bindingIds = response.active_secret_binding_ids || response.dataset?.secret_binding_ids || [];
      writeLocalSecretState(secret, bindingIds);
      setActiveSecretCount(bindingIds.length);
      setLocalSecretDraft('');
      setBanner(`已将 ${response.dataset?.title || selectedDataset.title} 绑定为私密数据集。`);
      await refreshCatalog({ preferredDatasetId: response.dataset?.id || selectedDataset.id, silent: true });
    } catch (bindError) {
      setError(bindError instanceof Error ? bindError.message : '绑定本地密钥失败');
    } finally {
      setResolvingSecret(false);
    }
  }

  async function handleClearLocalSecret() {
    clearLocalSecretState();
    setActiveSecretCount(0);
    setLocalSecretDraft('');
    setSelectedDatasetId(null);
    setSelectedDatasetIds([]);
    setBanner('已清除当前浏览器的本地密钥。');
    await refreshCatalog({ silent: true });
  }

  function rememberLocalUserStatement(message, assistantRunId = '') {
    const summary = String(message?.content || '').trim();
    if (!summary) {
      return;
    }
    fetchJson('/api/v3/conversation-memory-items', {
      method: 'POST',
      body: {
        local_thread_id: readLocalThreadId(),
        role: 'user',
        item_kind: 'user_statement',
        summary,
        source_message_refs: [message.id].filter(Boolean),
        artifact_refs: [],
        metadata: {
          source: 'browser_local_chat',
          assistant_run_id: assistantRunId || null,
        },
      },
    }).catch(() => {
      // Conversation memory is best-effort; chat must not wait on background supply indexing.
    });
  }

  async function requestOrdinaryAssistantRun({
    prompt,
    userMessage,
    briefing,
    nextScopePlan,
    continueRunId = '',
    selectedScope = null,
    onDelta = null,
    onEvent = null,
  }) {
    const messages = visibleMessages
      .slice(-12)
      .map((message) => ({ role: message.role, content: message.content }));
    if (continueRunId && promptRequestsAssistantContinue(prompt)) {
      try {
        const continued = await fetchSseJson(`/api/v3/assistant-runs/${continueRunId}/continue/stream`, {
          method: 'POST',
          body: {
            prompt,
            max_steps: 3,
            current_artifact: activeStaticPageDraft || null,
            messages,
          },
        }, {
          onDelta,
          onEvent,
        });
        return {
          response: continued,
          assistantRunId: continued?.run?.id || continueRunId,
          assistantContent: continued?.assistant_message?.content || '',
          continued: true,
        };
      } catch (continueError) {
        setLastAssistantRunId('');
      }
    }

    const created = await fetchSseJson('/api/v3/assistant-runs/stream', {
      method: 'POST',
      body: {
        prompt,
        local_thread_id: readLocalThreadId(),
        startup_briefing: briefing,
        selected_scope: selectedScope,
        scope_candidates: nextScopePlan.candidates,
        current_artifact: activeStaticPageDraft || null,
        messages: [...messages, { role: userMessage.role, content: userMessage.content }].slice(-12),
      },
    }, {
      onDelta,
      onEvent,
    });
    return {
      response: created,
      assistantRunId: created?.assistant_run_id || '',
      assistantContent: created?.assistant_message?.content || '',
      continued: false,
    };
  }

  function mergeAssistantRunCustomerCodexState(response) {
    const bundles = normalizeCodexCustomerArtifactBundlesFromAssistantRunResponse(response);
    if (bundles.length) {
      setCodexCustomerArtifacts((current) => mergeCodexCustomerArtifactBundles(current, bundles));
    }
    const tasks = normalizeCodexCustomerTasksFromAssistantRunResponse(response);
    if (tasks.length) {
      setCodexCustomerTasks((current) => mergeCodexCustomerTasks(current, tasks));
    }
    appendCodexCustomerChatUpdates(bundles, tasks);
    return { bundles, tasks };
  }

  function hasTerminalCustomerCodexTask(tasks = []) {
    return tasks.some((task) => isTerminalCodexCustomerTaskStatus(task?.status));
  }

  async function pollAssistantRunCustomerCodexState(assistantRunId, pollKey) {
    for (let attempt = 0; attempt < ASSISTANT_RUN_CUSTOMER_CODEX_POLL_ATTEMPTS; attempt += 1) {
      if (assistantRunCustomerCodexPollRef.current !== pollKey) {
        return;
      }
      if (attempt > 0) {
        await wait(ASSISTANT_RUN_CUSTOMER_CODEX_POLL_INTERVAL_MS);
      }
      if (assistantRunCustomerCodexPollRef.current !== pollKey) {
        return;
      }
      try {
        const detail = await fetchJson(`/api/v3/assistant-runs/${assistantRunId}`, {
          timeoutMs: 15000,
        });
        const { bundles, tasks } = mergeAssistantRunCustomerCodexState(detail);
        if (bundles.length || hasTerminalCustomerCodexTask(tasks)) {
          return;
        }
      } catch {
        return;
      }
    }
  }

  function startAssistantRunCustomerCodexPolling(assistantRunId, prompt, responsePayload) {
    const initial = mergeAssistantRunCustomerCodexState(responsePayload);
    if (
      !assistantRunId
      || (
        !initial.bundles.length
        && !initial.tasks.length
        && !promptMayUseCustomerCodex(prompt)
      )
    ) {
      return;
    }
    const pollKey = assistantRunCustomerCodexPollRef.current + 1;
    assistantRunCustomerCodexPollRef.current = pollKey;
    void pollAssistantRunCustomerCodexState(assistantRunId, pollKey);
  }

  function findDatasetForUploadPayload(items, payload) {
    return (Array.isArray(items) ? items : []).find((dataset) =>
      dataset?.key === payload.key || dataset?.title === payload.title,
    ) || null;
  }

  async function ensureUploadTargetDataset(classification, availableDatasets) {
    if (classification?.dataset?.id) {
      return { dataset: classification.dataset, created: false };
    }

    const payload = buildUploadDatasetPayload(classification);
    const existingDataset = findDatasetForUploadPayload(availableDatasets, payload);
    if (existingDataset?.id) {
      return { dataset: existingDataset, created: false };
    }

    try {
      const createdDataset = await fetchJson('/api/v3/datasets', {
        method: 'POST',
        body: payload,
      });
      return { dataset: createdDataset, created: true };
    } catch (createError) {
      const latestDatasets = await fetchJson('/api/v3/datasets');
      const fallbackDataset = findDatasetForUploadPayload(latestDatasets, payload);
      if (fallbackDataset?.id) {
        return { dataset: fallbackDataset, created: false };
      }
      throw createError;
    }
  }

  async function saveFilesForLocalIngest(files) {
    const formData = new FormData();
    files.forEach((file) => formData.append('files', file, file.name));
    const response = await fetchJson('/api/v3/local-document-uploads', {
      method: 'POST',
      body: formData,
      timeoutMs: LOCAL_UPLOAD_TIMEOUT_MS,
    });
    return Array.isArray(response?.files) ? response.files : [];
  }

  async function registerAndIngestUploadedFile(file, savedFile, targetDataset, classification, index) {
    const registered = await fetchJson('/api/v3/documents', {
      method: 'POST',
      body: {
        dataset_id: targetDataset.id,
        title: file.name || `上传文件 ${index + 1}`,
        object_key: savedFile?.object_key || buildLocalUploadObjectKey(file, Date.now() + index),
        content_type: savedFile?.content_type || file.type || 'application/octet-stream',
        secret_binding_ids: readLocalSecretBindingIds(),
        metadata: {
          initial_classification: {
            dataset_id: targetDataset.id,
            dataset_title: targetDataset.title,
            confidence: classification?.confidence || 'none',
            source: classification?.source || 'unknown',
            reason: classification?.reason || '',
            media_kind: inferUploadMediaKind(file) || undefined,
          },
          processing_policy: {
            foreground_allowed: ['save_file', 'preclassify', 'register_document', 'enqueue_ingest'],
            background_required: ['parse_content', 'vlm_enrichment', 'media_transcription', 'indexing', 'report_supply'],
          },
          parse_state: {
            stage: 'queued',
            user_blocking: false,
          },
        },
      },
      timeoutMs: UPLOAD_REGISTRATION_TIMEOUT_MS,
    });
    const ingestResponse = await fetchJson(`/api/v3/documents/${registered.document.id}/ingest`, {
      method: 'POST',
      timeoutMs: UPLOAD_REGISTRATION_TIMEOUT_MS,
    });
    return {
      file,
      document: registered.document,
      childDocuments: ingestResponse.child_documents || ingestResponse.childDocuments || [],
      workflowExecution: ingestResponse.workflow_execution,
      childWorkflowExecutions: ingestResponse.child_workflow_executions || ingestResponse.childWorkflowExecutions || [],
      started: ingestResponse.workflow_execution || null,
      targetDataset,
      classification,
    };
  }

  function handleUploadButtonClick() {
    if (uploadingFiles) {
      return;
    }
    fileInputRef.current?.click();
  }

  async function handleUploadFiles(event) {
    const files = Array.from(event.target.files || []);
    event.target.value = '';
    if (!files.length) {
      return;
    }

    setUploadingFiles(true);
    setError('');
    setBanner(`正在保存 ${files.length} 个文件到本地临时区...`);

    try {
      let availableDatasets = datasets;
      const uploadResults = [];
      const createdDatasetTitles = [];
      const savedFiles = await saveFilesForLocalIngest(files);

      if (savedFiles.length !== files.length) {
        throw new Error('上传文件保存数量不一致，已停止登记。');
      }

      for (const [index, file] of files.entries()) {
        setBanner(`正在登记并提交解析：${file.name || `上传文件 ${index + 1}`}（${index + 1}/${files.length}）...`);
        const classification = classifyUploadTarget({
          file,
          datasets: availableDatasets,
          selectedDatasetId: selectedDatasetIds[0] || selectedDatasetId,
        });
        const { dataset: targetDataset, created } = await ensureUploadTargetDataset(classification, availableDatasets);
        if (created) {
          createdDatasetTitles.push(targetDataset.title);
          availableDatasets = sortDatasets([...availableDatasets, targetDataset]);
        }
        const result = await registerAndIngestUploadedFile(file, savedFiles[index], targetDataset, classification, index);
        uploadResults.push(result);
      }

      const firstResult = uploadResults[0];
      const targetDataset = selectedDatasets[0]?.id
        ? selectedDatasets[0]
        : firstResult?.targetDataset || null;
      const uniqueTargets = [...new Map(uploadResults.map((result) => [result.targetDataset.id, result.targetDataset])).values()];
      const summary = summarizeUploadClassification({
        fileCount: files.length,
        datasetTitle: uniqueTargets.length === 1 ? uniqueTargets[0].title : uniqueTargets.map((dataset) => dataset.title).join('、'),
        classification: firstResult?.classification,
      });
      const publicWarning = uploadResults.some((result) => isPublicUploadClassification(result.classification))
        ? ' 未选私密数据集时当前按公开/default 数据集处理。'
        : '';
      const workflowLabel = uploadResults
        .flatMap((result) => [
          result.started?.enqueued_tasks?.[0]?.id,
          ...((result.childWorkflowExecutions || []).map((execution) => execution?.enqueued_tasks?.[0]?.id)),
        ])
        .filter(Boolean)
        .slice(0, 2)
        .join('、');
      const expandedZipChildCount = uploadResults
        .reduce((count, result) => count + (Array.isArray(result.childDocuments) ? result.childDocuments.length : 0), 0);

      setActivityEvents((current) => [
        {
          id: `activity-${Date.now()}`,
          kind: 'upload_event',
          summary: `上传分类：${files.map((file) => file.name).join('、')} -> ${uniqueTargets.map((dataset) => dataset.title).join('、')}`,
          created_at: new Date().toISOString(),
          dataset_ids: uniqueTargets.map((dataset) => dataset.id),
        },
        ...current,
      ].slice(0, 20));
      setScopePlan({
        candidates: uniqueTargets.map((dataset) => ({
          type: 'dataset',
          id: dataset.id,
          label: dataset.title,
          confidence: 'high',
          reason: '上传文件已自动归类到该数据集',
          source: 'upload_classified',
        })),
        hint: `上传已归类：${uniqueTargets.map((dataset) => dataset.title).join('、')}`,
      });
      if (uniqueTargets.length) {
        const uploadedDatasetIds = uniqueTargets.map((dataset) => dataset.id);
        setSelectedDatasetIds((current) => normalizeDatasetIds([...current, ...uploadedDatasetIds]));
        setSelectedDatasetId((current) => current || uploadedDatasetIds[0] || null);
      }
      setBanner(
        [
          summary,
          createdDatasetTitles.length ? `已补建默认公开数据集：${createdDatasetTitles.join('、')}。` : '',
          expandedZipChildCount ? `已从压缩包展开 ${expandedZipChildCount} 个子文档。` : '',
          workflowLabel ? `解析任务已入队：${workflowLabel}。` : '解析任务已提交。',
          publicWarning,
        ].filter(Boolean).join(' '),
      );
      await refreshCatalog({ preferredDatasetId: targetDataset?.id || selectedDatasetId, silent: true });
      if (activePage !== 'home' && (targetDataset?.id || selectedDatasetId)) {
        await refreshWorkspace(targetDataset?.id || selectedDatasetId, { silent: true, preserveNewSessionDraft: true });
      }
    } catch (uploadError) {
      const message = uploadError instanceof Error ? uploadError.message : '上传登记失败';
      setError(message);
      setBanner(`上传未完成：${message}`);
    } finally {
      setUploadingFiles(false);
    }
  }

  async function handleSubmitMessage() {
    const prompt = input.trim();

    if (!prompt) {
      return;
    }

    const userMessage = createLocalMessage('user', prompt);
    const baseMessages = selectedSessionId ? visibleMessages : localMessages;
    const wasSelectedSessionId = Boolean(selectedSessionId);

    if (!selectedSessionId && !draftSessionTitle.trim()) {
      setDraftSessionTitle(buildDefaultConversationTitle(prompt, draftSessionStartedAt));
    }
    if (wasSelectedSessionId) {
      setSelectedSessionId(null);
    }
    setLocalMessages((current) => [
      ...(wasSelectedSessionId ? baseMessages : current),
      userMessage,
    ].slice(-40));
    setInput('');
    setComposingNewSession(false);
    setSubmitting(false);
    setError('');
    setBanner('已发送，助手正在后台处理；你可以继续输入。');

    const codexForwardRequested = promptRequestsCodexForward(prompt);
    const nextScopePlan = planAssistantScope({
      prompt,
      datasets,
      documents,
      selectedDatasetId,
      selectedDatasetIds,
      conversationMemory: [...visibleMessages, userMessage],
      activeStaticPageDraft,
    });
    setScopePlan(nextScopePlan);
    const plannedDatasetIds = codexForwardRequested ? [] : selectPlannerDatasetIds(nextScopePlan);
    const currentSelectedDatasetIds = normalizeDatasetIds(selectedDatasetIds);
    const effectiveDatasetIds = currentSelectedDatasetIds.length
      ? currentSelectedDatasetIds
      : plannedDatasetIds;
    const effectiveDatasets = effectiveDatasetIds
      .map((datasetId) => datasets.find((dataset) => dataset.id === datasetId))
      .filter(Boolean);
    const effectiveDatasetId = effectiveDatasetIds[0] || '';
    const effectiveDataset = effectiveDatasets[0] || null;

    if (!codexForwardRequested && !sameDatasetIds(effectiveDatasetIds, selectedDatasetIds)) {
      setSelectedDatasetIds(effectiveDatasetIds);
      setSelectedDatasetId(effectiveDatasetId || null);
    }

    let pendingStaticPageDraft = null;
    const staticPageCreateRequested = promptRequestsStaticPage(prompt);
    const staticPageEditRequested = Boolean(activeStaticPageDraft && promptRequestsStaticPageEdit(prompt));
    const backendStaticPageEditRequested = Boolean(staticPageEditRequested && activeStaticPageDraft?.backendDraftId && lastAssistantRunId);
    const effectiveReportPlans = filterRecordsByDatasetIds(reportPlans, effectiveDatasetIds);
    const effectivePublishedReports = filterRecordsByDatasetIds(publishedReports, effectiveDatasetIds);
    const effectiveStaticPageDrafts = filterRecordsByDatasetIds(staticPageDraftItems, effectiveDatasetIds, staticPageDraftDatasetIds)
      .filter((draft) => isReusableStaticPageReportDraft(draft));
    const reusableReportTemplate = staticPageCreateRequested && !staticPageEditRequested
      ? findReusableReportTemplate(effectiveReportPlans, effectivePublishedReports, effectiveStaticPageDrafts)
      : null;
    if (reusableReportTemplate?.plan?.id) {
      setSelectedReportPlanId(reusableReportTemplate.plan.id);
    }
    const shouldUseAssistantRun = true;
    if (staticPageCreateRequested && !reusableReportTemplate) {
      pendingStaticPageDraft = handleStartStaticPageDraft({
        oneClick: true,
        openEditor: false,
        prompt,
        datasetId: effectiveDatasetId,
        dataset: effectiveDataset,
        datasets: effectiveDatasets,
        assistantRunId: '',
        announce: false,
      });
    } else if (staticPageEditRequested && !backendStaticPageEditRequested) {
      pendingStaticPageDraft = handleApplyStaticPagePrompt(prompt);
    }

    if (shouldUseAssistantRun) {
      try {
        const assistantSelectedScope = buildAssistantRunSelectedScope(effectiveDatasetIds, nextScopePlan);
        const briefingActiveStaticPageDraft = pendingStaticPageDraft
          || (staticPageEditRequested
            ? activeStaticPageDraft
            : isReusableStaticPageReportDraft(activeStaticPageDraft)
              ? activeStaticPageDraft
              : null);
        const briefing = buildAssistantStartupBriefing({
          datasets,
          documents,
          reportPlans,
          publishedReports,
          latestMessages: [...visibleMessages, userMessage],
          activityEvents,
          selectedDataset: effectiveDataset,
          selectedDatasets: effectiveDatasets,
          activeStaticPageDraft: briefingActiveStaticPageDraft,
          staticPageDrafts: effectiveStaticPageDrafts,
        });
        let assistantContent = '';
        let streamedAssistantContent = '';
        let streamStatusText = '正在生成回复...';
        let streamArtifactLink = '';
        let usedBackendAssistantRun = false;
        let assistantRunId = '';
        let usedAssistantRunContinue = false;
        const assistantMessage = createLocalMessage('assistant', '正在生成回复...');
        const updateAssistantStreamMessage = () => {
          const baseContent = streamedAssistantContent || streamStatusText || '正在生成回复...';
          const nextContent = appendArtifactLinkText(baseContent, streamArtifactLink);
          setLocalMessages((current) => current.map((message) =>
            message.id === assistantMessage.id
              ? { ...message, content: nextContent }
              : message,
          ));
        };
        setLocalMessages((current) => [
          ...current,
          assistantMessage,
        ].slice(-40));
        try {
          const assistantRun = await requestOrdinaryAssistantRun({
            prompt,
            userMessage,
            briefing,
            nextScopePlan,
            continueRunId: lastAssistantRunId,
            selectedScope: assistantSelectedScope,
            onDelta: (delta) => {
              streamedAssistantContent += delta;
              updateAssistantStreamMessage();
            },
            onEvent: (eventName, payload) => {
              const displayText = assistantRunStreamDisplayText(eventName, payload);
              if (displayText && !streamedAssistantContent) {
                streamStatusText = displayText;
              }
              const artifactLink = assistantRunStreamArtifactLink(eventName, payload);
              if (artifactLink) {
                streamArtifactLink = artifactLink;
              }
              if (displayText || artifactLink) {
                updateAssistantStreamMessage();
              }
            },
          });
          assistantRunId = assistantRun.assistantRunId || '';
          assistantContent = assistantRun.assistantContent || streamedAssistantContent || '';
          usedAssistantRunContinue = assistantRun.continued;
          if (assistantRunId) {
            setLastAssistantRunId(assistantRunId);
            if (pendingStaticPageDraft && !pendingStaticPageDraft.backendDraftId) {
              syncStaticPageDraftCreate(pendingStaticPageDraft, { assistantRunId, prompt });
            }
          }
          const responsePayload = assistantRun.response || {};
          setAssistantRunProgress(buildAssistantRunProgress(responsePayload, usedAssistantRunContinue));
          startAssistantRunCustomerCodexPolling(assistantRunId, prompt, responsePayload);
          const backendCandidates = Array.isArray(responsePayload?.scope_candidates)
            ? responsePayload.scope_candidates
            : [];
          if (backendCandidates.length) {
            setScopePlan({
              candidates: backendCandidates,
              hint: scopeHintFromCandidates(backendCandidates),
            });
          }
          const backendCandidateDatasetIds = effectiveDatasetIds.length
            ? []
            : selectPlannerDatasetIds({ candidates: backendCandidates });
          const backendDatasetIds = normalizeDatasetIds([
            ...(effectiveDatasetIds.length ? [] : uiDatasetIdsFromBackendScope(responsePayload?.selected_scope)),
            ...backendCandidateDatasetIds,
          ]);
          if (!codexForwardRequested && backendDatasetIds.length) {
            setSelectedDatasetIds((current) => normalizeDatasetIds([...current, ...backendDatasetIds]));
            setSelectedDatasetId((current) => current || backendDatasetIds[0]);
            await refreshCatalog({ preferredDatasetId: backendDatasetIds[0], silent: true });
          }
          if (activeStaticPageDraft?.backendDraftId) {
            await refreshBackendStaticPageDraft(activeStaticPageDraft.backendDraftId, { silent: true });
          }
          await refreshHtmlArtifacts({ silent: true });
          usedBackendAssistantRun = Boolean(assistantContent);
        } catch (assistantRunError) {
          setAssistantRunProgress(null);
          assistantRunId = assistantRunErrorRunId(assistantRunError) || assistantRunId;
          if (assistantRunId) {
            setLastAssistantRunId(assistantRunId);
          }
          if (backendStaticPageEditRequested) {
            pendingStaticPageDraft = handleApplyStaticPagePrompt(prompt);
          }
          assistantContent = assistantRunFailureMessage(assistantRunError);
        }

        const finalAssistantContent = appendArtifactLinkText(
          assistantContent || streamedAssistantContent || streamStatusText || '已完成，但本轮没有返回文本。',
          streamArtifactLink,
        );
        setLocalMessages((current) => current.map((message) =>
          message.id === assistantMessage.id
            ? {
                ...message,
                content: finalAssistantContent,
              }
            : message,
        ).slice(-40));
        rememberLocalUserStatement(userMessage, assistantRunId);
        let completionBanner = '';
        if (!usedBackendAssistantRun) {
          completionBanner = 'AssistantRun 本轮回复失败；用户消息已保留，详情见助手消息。';
        }
        setBanner(completionBanner);
        setError('');
      } finally {
        setSubmitting(false);
      }
      return;
    }

    setAssistantRunProgress(null);
    setSubmitting(true);
    try {
      const response = selectedSessionId && selectedDatasetId
        ? await fetchJson(`/api/v3/chat-sessions/${selectedSessionId}/turns`, {
            method: 'POST',
            body: { prompt },
          })
        : await fetchJson(`/api/v3/datasets/${effectiveDatasetId}/chat-sessions`, {
            method: 'POST',
            body: {
              prompt,
              title: draftSessionTitle.trim() || buildDefaultConversationTitle(prompt, draftSessionStartedAt),
              local_thread_id: readLocalThreadId(),
            },
          });
      const started = await startWorkflowExecution(response.workflow_execution?.id);
      const sessionTitle = response.chat_session?.title || '当前会话';

      setInput('');
      setDraftSessionTitle('');
      setComposingNewSession(false);
      setBanner(
        selectedSessionId
          ? `已追加到会话 ${sessionTitle}，任务 ${started?.enqueued_tasks?.[0]?.id || '已入队'}。`
          : `已启动新会话 ${sessionTitle}，任务 ${started?.enqueued_tasks?.[0]?.id || '已入队'}。`,
      );
      await Promise.all([
        refreshWorkspace(effectiveDatasetId, {
          preferredSessionId: response.chat_session.id,
          silent: true,
        }),
        refreshMessages(response.chat_session.id, { silent: true }),
      ]);
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : '提交问题失败');
    } finally {
      setSubmitting(false);
    }
  }

  function buildCurrentLocalChatSessionSnapshot(overrides = {}) {
    const threadId = overrides.threadId || localThreadId || readLocalThreadId();
    const snapshotMessages = Array.isArray(overrides.messages) ? overrides.messages : localMessages;
    const title = String(overrides.title || currentConversationTitle || '').trim();
    const now = new Date().toISOString();
    return {
      id: threadId,
      title: title || buildDefaultConversationTitle(
        snapshotMessages.find((message) => message.role === 'user')?.content || '新对话',
        draftSessionStartedAt,
      ),
      messages: snapshotMessages,
      startedAt: overrides.startedAt || draftSessionStartedAt || now,
      updatedAt: overrides.updatedAt || now,
      assistantRunId: overrides.assistantRunId ?? lastAssistantRunId,
    };
  }

  function persistCurrentLocalConversation(overrides = {}) {
    if (selectedSessionId) {
      return null;
    }
    const snapshot = buildCurrentLocalChatSessionSnapshot(overrides);
    if (!shouldPersistLocalChatSession({
      messages: snapshot.messages,
      assistantRunId: snapshot.assistantRunId,
      title: snapshot.title,
    })) {
      return null;
    }
    setLocalChatSessions((current) => {
      const next = upsertLocalChatSession(current, snapshot);
      writeLocalChatSessions(next);
      return next;
    });
    return snapshot;
  }

  function loadLocalConversation(sessionId) {
    const threadId = localThreadIdFromSessionOptionId(sessionId);
    if (!threadId) {
      return false;
    }
    if (!selectedSessionId && threadId !== localThreadId) {
      persistCurrentLocalConversation();
    }
    const localSession = localChatSessions.find((session) => session.id === threadId);
    if (!localSession) {
      setError('本地对话缓存未找到，可能已被浏览器清理。');
      return true;
    }
    writeLocalThreadId(threadId);
    setLocalThreadId(threadId);
    setDraftSessionStartedAt(localSession.startedAt || new Date().toISOString());
    setDraftSessionTitle(localSession.title || '');
    setComposingNewSession(false);
    setSelectedSessionId(null);
    setMessages([]);
    setLocalMessages((localSession.messages || []).slice(-40));
    setLastAssistantRunId(localSession.assistantRunId || '');
    setAssistantRunProgress(null);
    setCodexCustomerTasks([]);
    setCodexCustomerArtifacts([]);
    assistantRunCustomerCodexPollRef.current += 1;
    setMobilePanel('chat');
    setError('');
    return true;
  }

  function handleStartNewConversation() {
    persistCurrentLocalConversation();
    const nextThreadId = createLocalThreadId();
    const startedAt = new Date().toISOString();
    writeLocalThreadId(nextThreadId);
    setLocalThreadId(nextThreadId);
    setDraftSessionStartedAt(startedAt);
    setDraftSessionTitle('');
    setBanner(
      selectedDatasetIds.length
        ? '已新建对话；已选数据集仍作为优先供料范围，不会切换成别的会话。'
        : '已新建普通对话；未选数据集时按普通模型聊天处理。',
    );
    setError('');
    setComposingNewSession(true);
    setSelectedSessionId(null);
    setMessages([]);
    setLocalMessages([]);
    setLastAssistantRunId('');
    setAssistantRunProgress(null);
    setCodexCustomerTasks([]);
    setCodexCustomerArtifacts([]);
    assistantRunCustomerCodexPollRef.current += 1;
    setMobilePanel('chat');
  }

  function handleSelectConversation(sessionId) {
    if (!sessionId) {
      return;
    }
    if (isLocalChatSessionOptionId(sessionId)) {
      loadLocalConversation(sessionId);
      return;
    }
    if (sessionId === 'draft') {
      if (selectedSessionId) {
        handleStartNewConversation();
      }
      return;
    }
    persistCurrentLocalConversation();
    setComposingNewSession(false);
    setSelectedSessionId(sessionId);
    setMobilePanel('chat');
  }

  async function handleRenameConversation(sessionId, title) {
    const nextTitle = String(title || '').trim();
    if (!nextTitle) {
      return;
    }
    if (!sessionId || sessionId === 'draft') {
      setDraftSessionTitle(nextTitle);
      return;
    }

    try {
      const response = await fetchJson(`/api/v3/chat-sessions/${sessionId}`, {
        method: 'PATCH',
        body: { title: nextTitle },
      });
      const updatedSession = response?.chat_session;
      if (updatedSession?.id) {
        setSessions((current) => current.map((session) => (
          session.id === updatedSession.id ? updatedSession : session
        )));
      }
      setError('');
    } catch (renameError) {
      setError(renameError instanceof Error ? renameError.message : '对话改名失败');
    }
  }

  function handleStartStaticPageDraft(options = {}) {
    const {
      oneClick = false,
      openEditor = !oneClick,
      prompt = '',
      datasetId = selectedDatasetId,
      dataset = selectedDataset,
      datasets: draftDatasets = selectedDatasets,
      assistantRunId = lastAssistantRunId,
      announce = true,
    } = options;
    const draftDatasetId = datasetId || '';
    const conversationSummary = buildStaticPageConversationSummary(prompt, {
      dataset,
      datasets: draftDatasets,
      messages: visibleMessages,
    });
    const fieldCandidates = buildStaticPageContextFieldCandidates({
      dataset,
      datasets: draftDatasets,
    });

    const baseDraft = buildInitialStaticPageDraft({
      datasetId: draftDatasetId,
      sessionId: selectedSessionId,
      templateIntent: prompt,
      templateReferenceFallbackId: oneClick ? 'data-report' : null,
      conversationSummary,
      fieldCandidates,
    });
    const draft = oneClick
      ? applyStaticPageOperation(baseDraft, {
          type: 'queue_image_job',
          queueMessage: '已按 AI 理解跳过手工调整，资源正在排队，可以联系商务开通高级用户跳过等待。',
        })
      : baseDraft;

    setStaticPageDrafts((current) => ({
      ...current,
      [draft.id]: draft,
    }));
    setActiveStaticPageDraftId(draft.id);
    setStaticPageEditorOpen(Boolean(openEditor));
    setActiveHtmlArtifactId(null);
    if (announce) {
      setBanner(openEditor
        ? (oneClick ? '已按 AI 理解创建静态页草稿，并进入可视化排队。' : '已创建静态页草稿，下一步会展示页面规划。')
        : '已准备静态页草稿；当前对话不会中断，需要时点击“进入静态页工作台”。');
    }
    setError('');
    setMobilePanel('chat');
    if (assistantRunId) {
      syncStaticPageDraftCreate(draft, { assistantRunId, prompt });
    }
    return draft;
  }

  function staticPageDraftByAnyId(draftId) {
    if (!draftId) return null;
    return staticPageDrafts[draftId]
      || staticPageDraftItems.find((draft) => draft?.id === draftId || draft?.backendDraftId === draftId)
      || null;
  }

  function htmlArtifactOwnerScope(artifact) {
    return artifact?.ownerScope || artifact?.owner_scope || {};
  }

  function publishedStaticPageArtifactForDraft(draft) {
    if (!draft) return null;
    const ownerIds = new Set([draft.id, draft.backendDraftId].filter(Boolean));
    return htmlArtifacts.find((artifact) => {
      const templateId = artifact?.templateId || artifact?.template_id;
      if (templateId !== 'static_page_published_preview') return false;
      const ownerScope = htmlArtifactOwnerScope(artifact);
      return ownerScope?.type === 'static_page_draft' && ownerIds.has(ownerScope.id);
    }) || null;
  }

  function handlePreviewStaticPageDraft(draftId) {
    const draft = staticPageDraftByAnyId(draftId);
    if (!draft) {
      return;
    }
    setActiveStaticPageDraftId(draft.id);
    setStaticPageEditorOpen(false);
    setMobilePanel('chat');
    const artifact = publishedStaticPageArtifactForDraft(draft);
    if (artifact?.id) {
      setActiveHtmlArtifactId(artifact.id);
      setBanner('已在主站打开静态页预览；后续直接在对话里说修改要求，会沿用这个项目。');
      return;
    }
    setActiveHtmlArtifactId(null);
    setBanner('静态页已生成，但当前链接不是主站可内嵌预览路径；右侧项目卡仍会保留，可复制链接或新窗口查看。');
  }

  function handleSelectStaticPageDraft(draftId) {
    const draft = staticPageDraftByAnyId(draftId);
    if (!draft) {
      return;
    }
    if (draft.finalPage?.status === 'rendered' || draft.status === 'rendered') {
      handlePreviewStaticPageDraft(draft.id);
      return;
    }
    if (activeStaticPageDraftId === draftId && staticPageEditorOpen) {
      setStaticPageEditorOpen(false);
      setActiveHtmlArtifactId(null);
      setBanner('已退出项目，回到聊天记录。右侧项目卡可再次进入。');
      setMobilePanel('chat');
      return;
    }
    setActiveStaticPageDraftId(draftId);
    setStaticPageEditorOpen(true);
    setActiveHtmlArtifactId(null);
    setBanner(draft.status === 'rendered' ? '已打开已生成静态页，可继续在对话框提出修改。' : '已打开静态页草稿，可继续规划或生成。');
    setMobilePanel('chat');
  }

  function appendReportShelfSelectionMessage(title, metadata = {}) {
    const cleanTitle = String(title || '当前').replace(/^静态页[：:]\s*/, '').trim() || '当前';
    const reportName = /报表|报告|看板|页面/.test(cleanTitle) ? cleanTitle : `${cleanTitle}报表`;
    const content = `已选中「${reportName}」，你可以继续修改。`;
    setLocalMessages((current) => {
      const last = current[current.length - 1];
      if (last?.metadata?.source === 'report_shelf_selection' && last?.content === content) {
        return current;
      }
      return [
        ...current,
        {
          ...createLocalMessage('assistant', content),
          metadata: {
            source: 'report_shelf_selection',
            ...metadata,
          },
        },
      ].slice(-40);
    });
  }

  function handleSelectReportPlanFromShelf(reportPlanId) {
    setSelectedReportPlanId(reportPlanId);
    const plan = datasetReportPlans.find((item) => item.id === reportPlanId) || null;
    const published = publishedReportForPlan(plan, datasetPublishedReports);
    appendReportShelfSelectionMessage(reportTemplateTitle({ plan, published }, '当前'), {
      reportPlanId,
    });
    setMobilePanel('chat');
  }

  function handleSelectPublishedReportFromShelf(reportId, title) {
    appendReportShelfSelectionMessage(title || '当前', {
      publishedReportId: reportId,
    });
    setMobilePanel('chat');
  }

  function handleSelectReportShelfStaticPageDraft(draftId) {
    const draft = staticPageDraftByAnyId(draftId);
    if (!draft) {
      return;
    }
    setActiveStaticPageDraftId(draft.id);
    setStaticPageEditorOpen(false);
    setActiveHtmlArtifactId(null);
    setMobilePanel('chat');
    const title = reportTemplateTitle({ draft }, '当前');
    appendReportShelfSelectionMessage(title, {
      draftId: draft.id,
    });
  }

  function handleOpenStaticPageDraft(draftId) {
    const draft = staticPageDraftByAnyId(draftId);
    if (!draft) {
      return;
    }
    const finalUrl = staticPageRenderedUrl(draft);
    setActiveStaticPageDraftId(draft.id);
    setStaticPageEditorOpen(false);
    setMobilePanel('chat');
    if (finalUrl && typeof window !== 'undefined') {
      window.open(finalUrl, '_blank', 'noopener,noreferrer');
      return;
    }
    if (typeof window !== 'undefined') {
      window.alert('这个报表还没有可直接打开的公开链接。');
    }
  }

  function handleCloseStaticPageDraft() {
    setStaticPageEditorOpen(false);
    setBanner('已返回聊天记录；右侧静态页成品架可随时重新打开草稿或成品。');
  }

  function handleRevertStaticPageStage(draftId) {
    const draft = staticPageDrafts[draftId];
    if (!draft) {
      return null;
    }
    const finalStatus = draft.finalPage?.status || '';
    const hasFinalStage = Boolean(finalStatus) || draft.status === 'rendered' || draft.status === 'rendering';
    const hasEffectStage = Boolean(draft.previewImage)
      || Boolean(draft.imageJob?.id)
      || ['queued', 'running', 'preview_ready', 'effect_confirmed'].includes(draft.status)
      || ['queued', 'running', 'preview_ready', 'confirmed', 'stale'].includes(draft.previewContract?.status || '');
    const operation = hasFinalStage
      ? { type: 'reset_final_render' }
      : hasEffectStage
        ? { type: 'reset_image_job' }
        : null;
    if (!operation) {
      setActiveStaticPageDraftId(draftId);
      setStaticPageEditorOpen(true);
      setActiveHtmlArtifactId(null);
      setBanner('当前已经在模板规划阶段，可直接继续修改。');
      return draft;
    }
    const nextDraft = applyStaticPageOperation(draft, operation);
    setStaticPageDrafts((current) => ({
      ...current,
      [nextDraft.id]: nextDraft,
    }));
    setActiveStaticPageDraftId(nextDraft.id);
    setStaticPageEditorOpen(true);
    setActiveHtmlArtifactId(null);
    syncStaticPageDraftOperations(draft, nextDraft, [operation], {
      summary: hasFinalStage ? '已退回可视化阶段继续修改。' : '已退回模板规划阶段继续修改。',
    });
    setBanner(hasFinalStage ? '已退回可视化阶段，可调整后重新制作静态页。' : '已退回模板规划阶段，可继续修改模板和模块。');
    setMobilePanel('chat');
    return nextDraft;
  }

  async function handleDeleteStaticPageDraft(draftId) {
    const draft = staticPageDraftByAnyId(draftId);
    if (!draft) {
      return;
    }
    if (staticPageDraftHasAnyReportShelfDefault(draft)) {
      appendUiNoticeMessage('warning', '请先取消这个报表在相关数据集里的默认，再删除。');
      return;
    }
    if (typeof window !== 'undefined' && !window.confirm('删除这个生成项目？删除后右侧列表将不再展示。')) {
      return;
    }
    setStaticPageDrafts((current) => {
      const next = { ...current };
      delete next[draft.id];
      return next;
    });
    if (activeStaticPageDraftId === draft.id) {
      setActiveStaticPageDraftId(null);
      setStaticPageEditorOpen(false);
    }
    setActiveHtmlArtifactId(null);
    if (draft.backendDraftId) {
      try {
        await fetchJson(`/api/v3/static-page-drafts/${draft.backendDraftId}`, {
          method: 'PATCH',
          body: JSON.stringify({
            status: 'archived',
            draft_payload: {
              ...draft,
              status: 'archived',
              archivedAt: new Date().toISOString(),
            },
          }),
        });
      } catch (deleteError) {
        if (typeof window !== 'undefined') {
          window.alert(`项目已先从本地列表移除；后端归档暂不可用：${deleteError instanceof Error ? deleteError.message : '请求失败'}。`);
        }
        return;
      }
    }
  }

  async function syncStaticPageDraftSourceRefs(nextDraft, failureLabel) {
    if (!nextDraft?.backendDraftId) {
      return;
    }
    try {
      const response = await fetchJson(`/api/v3/static-page-drafts/${nextDraft.backendDraftId}`, {
        method: 'PATCH',
        body: {
          source_refs: nextDraft.source_refs,
        },
      });
      const normalized = response?.draft ? normalizeBackendStaticPageDraft(response.draft) : null;
      if (normalized?.id) {
        setStaticPageDrafts((current) => ({
          ...current,
          [normalized.id]: normalized,
        }));
      }
    } catch (syncError) {
      if (typeof window !== 'undefined') {
        window.alert(`${failureLabel}；后端同步失败：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
      }
    }
  }

  async function handleSetDefaultStaticPageTemplate(draftId) {
    const draft = staticPageDraftByAnyId(draftId);
    if (!draft) {
      return;
    }
    const targetDatasetIds = reportShelfDefaultTargetDatasetIds(draft, reportShelfDatasetIds);
    if (!targetDatasetIds.length) {
      appendUiNoticeMessage('warning', '请先选择一个数据集，再设置默认报表。');
      return;
    }
    const nextDraft = withStaticPageDraftReportShelfDefaults(draft, targetDatasetIds, true);
    setStaticPageDrafts((current) => ({
      ...current,
      [nextDraft.id]: nextDraft,
    }));
    await syncStaticPageDraftSourceRefs(nextDraft, '已先在当前页面设置默认');
  }

  async function handleCancelDefaultStaticPageTemplate(draftId) {
    const draft = staticPageDraftByAnyId(draftId);
    if (!draft) {
      return;
    }
    const targetDatasetIds = reportShelfDefaultTargetDatasetIds(draft, reportShelfDatasetIds);
    if (!targetDatasetIds.length) {
      appendUiNoticeMessage('warning', '请先选择一个数据集，再取消默认报表。');
      return;
    }
    const nextDraft = withStaticPageDraftReportShelfDefaults(draft, targetDatasetIds, false);
    setStaticPageDrafts((current) => ({
      ...current,
      [nextDraft.id]: nextDraft,
    }));
    await syncStaticPageDraftSourceRefs(nextDraft, '已先在当前页面取消默认');
  }

  function handleSelectHtmlArtifact(artifactId) {
    const artifact = htmlArtifacts.find((item) => (item?.id || item?.artifact_id) === artifactId);
    const ownerScope = htmlArtifactOwnerScope(artifact);
    if (ownerScope?.type === 'static_page_draft' && ownerScope.id) {
      const draft = staticPageDraftByAnyId(ownerScope.id);
      if (draft?.id) {
        setActiveStaticPageDraftId(draft.id);
      }
    }
    setActiveHtmlArtifactId(artifactId);
    setStaticPageEditorOpen(false);
    setBanner('已打开主站内 HTML/静态页预览；需要调整时继续在对话里说修改要求。');
    setMobilePanel('chat');
  }

  function handleCloseHtmlArtifact() {
    setActiveHtmlArtifactId(null);
    setBanner('已关闭 HTML 产物预览。');
  }

  async function handleHtmlArtifactEvent(eventData, manifest) {
    const artifactId = manifest?.id || eventData?.artifactId || activeHtmlArtifactId;
    if (!artifactId) {
      setBanner('HTML 产物动作缺少产物编号，已拦截。');
      return;
    }
    try {
      await fetchJson(`/api/v3/html-artifacts/${encodeURIComponent(artifactId)}/events`, {
        method: 'POST',
        body: {
          assistant_run_id: manifest?.provenance?.sourceRunId || activeHtmlArtifact?.provenance?.source_run_id || null,
          local_thread_id: readLocalThreadId(),
          event_type: eventData?.type || '',
          payload: eventData?.payload || {},
        },
      });
      await refreshHtmlArtifacts({ silent: true });
      const ownerScope = manifest?.ownerScope || activeHtmlArtifact?.ownerScope || activeHtmlArtifact?.owner_scope;
      if (ownerScope?.type === 'static_page_draft' && ownerScope.id) {
        await refreshBackendStaticPageDraft(ownerScope.id, { silent: true });
      }
      setBanner(`HTML 产物动作已提交并同步到 DataMax：${eventData?.type || 'unknown'}。`);
    } catch (submitError) {
      setBanner(`HTML 产物动作已拦截：${submitError instanceof Error ? submitError.message : '提交失败'}。`);
    }
  }

  function handleApplyStaticPagePrompt(prompt) {
    if (!activeStaticPageDraft) {
      return handleStartStaticPageDraft({ prompt });
    }

    const interpretation = interpretStaticPagePrompt(activeStaticPageDraft, prompt);
    const draft = applyStaticPageOperations(activeStaticPageDraft, interpretation.operations);
    setStaticPageDrafts((current) => ({
      ...current,
      [draft.id]: draft,
    }));
    setActiveStaticPageDraftId(draft.id);
    setBanner(`已按模型理解刷新静态页规划：${interpretation.summary}`);
    if (activeStaticPageDraft.backendDraftId) {
      syncStaticPageDraftIntent(activeStaticPageDraft, prompt);
    }
    return draft;
  }

  function handleApplyStaticPageOperation(operation) {
    if (!activeStaticPageDraft || !operation?.type) {
      return null;
    }

    const directHtmlRender = Boolean(operation.directHtml || operation.finalPage?.directHtml);
    if (
      operation.type === 'request_final_render'
      && !directHtmlRender
      && !canRequestStaticPageFinalRender(activeStaticPageDraft)
    ) {
      setBanner(staticPageFinalRenderBlockReason(activeStaticPageDraft) || '需要先确认当前可视化，再制作最终静态页。');
      return activeStaticPageDraft;
    }

    if (
      operation.type === 'request_final_render'
      && directHtmlRender
      && !canRequestStaticPageDirectHtml(activeStaticPageDraft)
    ) {
      setBanner(staticPageDirectHtmlBlockReason(activeStaticPageDraft) || '需要先补齐模块数据，再生成 HTML。');
      return activeStaticPageDraft;
    }

    if (operation.type === 'request_final_render' && activeStaticPageDraft.backendDraftId) {
      const optimisticDraft = replaceDraftWithOperation(activeStaticPageDraft, {
        ...operation,
        finalPage: operation.finalPage || {
          status: 'queued',
          renderer: 'platform-api-static-page-renderer',
          renderOutputId: '',
          imageJobId: directHtmlRender ? null : activeStaticPageDraft.imageJob?.id || null,
          directHtml: directHtmlRender,
          assetManifest: {
            status: 'queued',
            queue_copy: directHtmlRender
              ? '快速 HTML 正在生成，完成后可直接下载 index.html。'
              : '最终静态页正在后台制作，可以继续聊天或修改其他内容。',
          },
        },
      });
      createBackendStaticPageRender(optimisticDraft, operation).catch((syncError) => {
        replaceStaticPageDraft(activeStaticPageDraft.id, activeStaticPageDraft);
        setBanner('');
        setError(`静态页未进入后台制作：${staticPagePreviewGateErrorMessage(syncError, '后端渲染暂不可用')}。`);
      });
      return optimisticDraft;
    }

    if (operation.type === 'queue_image_job') {
      const queueOperation = staticPageOperationIsPromptOnly(operation)
        ? buildPromptOnlyStaticPageQueueOperation(activeStaticPageDraft, operation)
        : operation;
      const blockReason = staticPageOperationIsPromptOnly(queueOperation)
        ? ''
        : staticPagePreviewBlockReason(activeStaticPageDraft);
      if (blockReason) {
        setBanner('');
        setError(blockReason);
        setStaticPageEditorOpen(true);
        return activeStaticPageDraft;
      }
      setError('');
      if (activeStaticPageDraft.backendDraftId) {
        setStaticPageActionBusy(true);
        createBackendStaticPageImageJob(activeStaticPageDraft, queueOperation)
          .then(() => {
            setStaticPageEditorOpen(false);
            setMobilePanel('chat');
          })
          .catch((syncError) => {
            setBanner('');
            setError(`可视化未入队：${staticPagePreviewGateErrorMessage(syncError)}。`);
          })
          .finally(() => setStaticPageActionBusy(false));
        return activeStaticPageDraft;
      }
      return replaceDraftWithOperation(activeStaticPageDraft, queueOperation);
    }

    const draft = replaceDraftWithOperation(activeStaticPageDraft, operation);

    if (operation.type === 'confirm_preview') {
      if (draft.backendDraftId) {
        confirmBackendStaticPagePreview(draft, operation).catch((syncError) => {
          setBanner(`可视化已先在本地确认；后端确认暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
        });
      }
      return draft;
    }

    syncStaticPageDraftOperations(activeStaticPageDraft, draft, [operation]);
    return draft;
  }

  async function handleStaticPagePrimaryAction(operation = {}) {
    if (staticPageActionBusy) {
      return activeStaticPageDraft;
    }

    if (!activeStaticPageDraft) {
      const draft = handleStartStaticPageDraft({ oneClick: false });
      setStaticPageEditorOpen(true);
      return draft;
    }

    const draft = activeStaticPageDraft;
    const previewStale = draft.previewContract?.status === 'stale' || draft.imageJob?.status === 'stale';
    const jobStatus = previewStale ? 'stale' : (draft.imageJob?.status || draft.previewContract?.status || 'idle');
    const finalStatus = draft.finalPage?.status || '';

    if (finalStatus === 'rendered' || draft.status === 'rendered') {
      setStaticPageEditorOpen(false);
      const finalUrl = staticPageRenderedUrl(draft);
      if (finalUrl && typeof window !== 'undefined') {
        window.open(finalUrl, '_blank', 'noopener,noreferrer');
        setBanner('静态页已打开；需要调整时继续在对话里提出即可。');
      } else {
        setBanner('静态页已经生成；需要调整时继续在对话里提出即可。');
      }
      return draft;
    }

    if (['queued', 'running'].includes(jobStatus)) {
      setStaticPageEditorOpen(false);
      setBanner('可视化正在生成中；资源返回后会自动继续制作页面。');
      return draft;
    }

    if (['queued', 'rendering'].includes(finalStatus)) {
      setStaticPageEditorOpen(false);
      setBanner('最终静态页正在后台制作；完成后会进入右侧生成结果。');
      return draft;
    }

    setStaticPageActionBusy(true);
    setError('');

    try {
      const canContinueToRender = jobStatus === 'preview_ready'
        || draft.status === 'preview_ready'
        || draft.status === 'effect_confirmed'
        || draft.previewContract?.status === 'confirmed';

      if (canContinueToRender) {
        if (!canRequestStaticPageFinalRender(draft)) {
          setBanner(staticPageFinalRenderBlockReason(draft) || '可视化资源缺失，请重新发起可视化。');
          return draft;
        }

        if (draft.backendDraftId) {
          const { draft: renderDraft } = await createBackendStaticPageRender(draft, {
            previewImage: draft.previewImage || buildMockStaticPagePreview(draft),
          });
          setStaticPageEditorOpen(false);
          setMobilePanel('chat');
          return renderDraft;
        }

        const confirmedDraft = draft.previewContract?.status === 'confirmed'
          ? draft
          : draft;
        const renderedDraft = replaceDraftWithOperation(confirmedDraft, { type: 'request_final_render' });
        setStaticPageEditorOpen(false);
        setMobilePanel('chat');
        setBanner('已按可视化生成本地静态页模拟结果；接入后端时会进入正式后台渲染。');
        return renderedDraft;
      }

      const queueOperation = buildPromptOnlyStaticPageQueueOperation(draft, operation);
      const blockReason = staticPageOperationIsPromptOnly(queueOperation)
        ? ''
        : staticPagePreviewBlockReason(draft);
      if (blockReason) {
        setBanner('');
        setError(blockReason);
        setStaticPageEditorOpen(true);
        return draft;
      }

      if (draft.backendDraftId) {
        const { draft: queuedDraft } = await createBackendStaticPageImageJob(draft, queueOperation);
        setStaticPageEditorOpen(false);
        setMobilePanel('chat');
        return queuedDraft;
      }

      const queuedDraft = replaceDraftWithOperation(draft, queueOperation);
      const previewDraft = replaceDraftWithOperation(queuedDraft, { type: 'mark_preview_ready' });
      setStaticPageEditorOpen(false);
      setMobilePanel('chat');
      setBanner('当前草稿尚未同步到后端，已生成本地模拟可视化；正式运行会进入后台生成队列。');
      return previewDraft;
    } catch (actionError) {
      setError(staticPagePreviewGateErrorMessage(actionError, '静态页生成动作失败'));
      return draft;
    } finally {
      setStaticPageActionBusy(false);
    }
  }

  async function handleResolveReportEntry(action) {
    if (!selectedSessionId) {
      return;
    }

    const reportEntry = selectedSession?.session_manifest_view?.report_entry;
    setReportEntryBusy(true);

    try {
      const response = await fetchJson(`/api/v3/chat-sessions/${selectedSessionId}/report-entry`, {
        method: 'POST',
        body: {
          action,
          title: reportEntry?.suggested_title || null,
          objective: reportEntry?.suggested_objective || null,
        },
      });

      if (action === 'enter_report_service' && response.report_plan) {
        const started = await startWorkflowExecution(response.workflow_execution?.id);
        setSelectedReportPlanId(response.report_plan.id);
        setMobilePanel('insights');
        setBanner(`已进入报告服务，生成 report_plan ${response.report_plan.id}，任务 ${started?.enqueued_tasks?.[0]?.id || '已入队'}。`);
      } else {
        setBanner('已保持资料服务，这条分流记录会保留在 session manifest 里。');
      }

      await Promise.all([
        refreshWorkspace(selectedDatasetId, {
          preferredSessionId: selectedSessionId,
          silent: true,
        }),
        refreshMessages(selectedSessionId, { silent: true }),
        refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true }),
      ]);
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : '处理报告分流失败');
    } finally {
      setReportEntryBusy(false);
    }
  }

  async function handleContinueReportPlan() {
    if (!selectedReportPlanId) {
      return;
    }

    setReportActionBusy('continue');
    setError('');
    try {
      const response = await fetchJson(`/api/v3/report-plans/${selectedReportPlanId}/continue`, {
        method: 'POST',
      });
      const started = await startWorkflowExecution(response.workflow_execution?.id);
      setBanner(`已请求继续规划：workflow ${response.workflow_execution.id}，任务 ${started?.enqueued_tasks?.[0]?.id || '已入队'}。`);
      await Promise.all([
        refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true }),
        refreshReportDetail(selectedReportPlanId, { silent: true }),
      ]);
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : '继续报告规划失败');
    } finally {
      setReportActionBusy('');
    }
  }

  async function handleRequestReportRender() {
    if (!selectedReportPlanId) {
      return;
    }

    setReportActionBusy('render');
    setError('');
    try {
      const response = await fetchJson(`/api/v3/report-plans/${selectedReportPlanId}/renders`, {
        method: 'POST',
        body: { surface: reportSurface },
      });
      const started = await startWorkflowExecution(response.workflow_execution?.id);
      setBanner(`已请求 ${response.surface} 渲染：workflow ${response.workflow_execution.id}，任务 ${started?.enqueued_tasks?.[0]?.id || '已入队'}。`);
      await Promise.all([
        refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true }),
        refreshReportDetail(selectedReportPlanId, { silent: true }),
      ]);
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : '请求报告渲染失败');
    } finally {
      setReportActionBusy('');
    }
  }

  async function handlePublishReport() {
    if (!selectedReportPlanId) {
      return;
    }

    setReportActionBusy('publish');
    setError('');
    try {
      const response = await fetchJson(`/api/v3/report-plans/${selectedReportPlanId}/publish`, {
        method: 'POST',
        body: {
          surface: reportSurface,
          publish_note: publishNote.trim() || null,
        },
      });
      setPublishNote('');
      setBanner(`已发布 ${response.version.surface} v${response.version.version_no}：${response.report.slug}。`);
      await Promise.all([
        refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true }),
        refreshReportDetail(selectedReportPlanId, { silent: true }),
      ]);
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : '发布报告失败');
    } finally {
      setReportActionBusy('');
    }
  }

  async function handleRetryWorkflowExecution(executionId) {
    if (!executionId) {
      return;
    }

    setReportActionBusy(`retry:${executionId}`);
    setError('');
    try {
      const response = await fetchJson(`/api/v3/workflow-executions/${executionId}/retry`, {
        method: 'POST',
        body: { reason: 'web_report_control_retry' },
      });
      const restart = response.restart_transition || response.retry_transition;
      setBanner(`已请求重试 workflow ${restart.execution.id}，任务 ${restart.enqueued_tasks?.[0]?.id || '已入队'}。`);
      await Promise.all([
        refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true }),
        selectedReportPlanId ? refreshReportDetail(selectedReportPlanId, { silent: true }) : Promise.resolve(),
        refreshStaticPageDraftShelf({ silent: true }),
        refreshHtmlArtifacts({ silent: true }),
        activeStaticPageDraft?.backendDraftId
          ? refreshBackendStaticPageDraft(activeStaticPageDraft.backendDraftId, { silent: true })
          : Promise.resolve(),
      ]);
    } catch (retryError) {
      setError(retryError instanceof Error ? retryError.message : '重试 workflow 失败');
    } finally {
      setReportActionBusy('');
    }
  }

  async function handleCancelWorkflowExecution(executionId) {
    if (!executionId) {
      return;
    }

    setReportActionBusy(`cancel:${executionId}`);
    setError('');
    try {
      const transition = await fetchJson(`/api/v3/workflow-executions/${executionId}/signals`, {
        method: 'POST',
        body: {
          kind: 'cancel_requested',
          reason: 'web_static_page_render_cancel',
        },
      });
      setBanner(`已请求取消 workflow ${transition.execution?.id || executionId}。`);
      await Promise.all([
        refreshCatalog({ preferredDatasetId: selectedDatasetId, silent: true }),
        selectedReportPlanId ? refreshReportDetail(selectedReportPlanId, { silent: true }) : Promise.resolve(),
        refreshStaticPageDraftShelf({ silent: true }),
        refreshHtmlArtifacts({ silent: true }),
        activeStaticPageDraft?.backendDraftId
          ? refreshBackendStaticPageDraft(activeStaticPageDraft.backendDraftId, { silent: true })
          : Promise.resolve(),
      ]);
    } catch (cancelError) {
      setError(cancelError instanceof Error ? cancelError.message : '取消 workflow 失败');
    } finally {
      setReportActionBusy('');
    }
  }

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return undefined;
    }

    const mediaQuery = window.matchMedia('(max-width: 960px)');
    const syncViewport = () => setMobileViewport(mediaQuery.matches);
    syncViewport();
    mediaQuery.addEventListener('change', syncViewport);
    return () => mediaQuery.removeEventListener('change', syncViewport);
  }, []);

  useEffect(() => {
    refreshCatalog();
  }, []);

  useEffect(() => {
    const cachedEmail = readLocalAccountEmail();
    if (cachedEmail) {
      setAccountEmailDraft(cachedEmail);
    }
    refreshAuthSession({ silent: true });
  }, []);

  useEffect(() => {
    refreshStaticPageDraftShelf({ silent: true });
    refreshHtmlArtifacts({ silent: true });
  }, []);

  useEffect(() => {
    refreshStaticPageDraftShelf({ silent: true });
  }, [reportShelfFetchDatasetIds.join('|')]);

  useEffect(() => {
    setActiveSecretCount(readLocalSecretBindingIds().length);
    setLastAssistantRunId(readLocalAssistantRunId());
  }, []);

  useEffect(() => {
    writeLocalAssistantRunId(lastAssistantRunId);
  }, [lastAssistantRunId]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      setLocalChatStorageReady(true);
      return;
    }
    try {
      const threadId = readLocalThreadId();
      setLocalThreadId(threadId);
      const storedLocalSessions = readLocalChatSessions();
      setLocalChatSessions(storedLocalSessions);
      const currentLocalSession = storedLocalSessions.find((session) => session.id === threadId);
      if (currentLocalSession) {
        setLocalMessages((currentLocalSession.messages || []).slice(-40));
        setDraftSessionStartedAt(currentLocalSession.startedAt || new Date().toISOString());
        setDraftSessionTitle(currentLocalSession.title || '');
        if (currentLocalSession.assistantRunId) {
          setLastAssistantRunId(currentLocalSession.assistantRunId);
        }
      } else {
        setLocalMessages(readLocalChatMessages());
      }
    } catch {
      setLocalMessages([]);
    } finally {
      setLocalChatStorageReady(true);
    }
  }, []);

  useEffect(() => {
    appendUiNoticeMessage('error', error);
  }, [error]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    if (!localChatStorageReady) {
      return;
    }
    writeLocalChatMessages(localMessages);
    if (!selectedSessionId) {
      const snapshot = buildCurrentLocalChatSessionSnapshot({ threadId: localThreadId || readLocalThreadId() });
      if (shouldPersistLocalChatSession({
        messages: snapshot.messages,
        assistantRunId: snapshot.assistantRunId,
        title: snapshot.title,
      })) {
        setLocalChatSessions((current) => {
          const next = upsertLocalChatSession(current, snapshot);
          writeLocalChatSessions(next);
          return next;
        });
      }
    }
  }, [
    currentConversationTitle,
    draftSessionStartedAt,
    lastAssistantRunId,
    localChatStorageReady,
    localMessages,
    localThreadId,
    selectedSessionId,
  ]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    setActivityEvents(readLocalActivityEvents());
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    writeLocalActivityEvents(activityEvents);
  }, [activityEvents]);

  useEffect(() => {
    if (activePage === 'home') {
      return;
    }
    if (!selectedDatasetId) {
      startTransition(() => {
        setSessions([]);
        setOutputs([]);
        setSelectedReportPlanId(null);
        setSelectedSessionId(null);
        setComposingNewSession(false);
        setMessages([]);
        setActiveStaticPageDraftId(null);
      });
      return;
    }

    startTransition(() => {
      setSessions([]);
      setOutputs([]);
      setSelectedReportPlanId(null);
      setSelectedSessionId(null);
      setComposingNewSession(false);
      setMessages([]);
      setActiveStaticPageDraftId(null);
    });

    refreshWorkspace(selectedDatasetId);
  }, [activePage, selectedDatasetId]);

  useEffect(() => {
    if (!selectedDocumentId) {
      setSelectedDocumentDetail(null);
      return;
    }
    refreshDocumentDetail(selectedDocumentId);
  }, [selectedDocumentId]);

  useEffect(() => {
    if (!['datasets', 'sources', 'document-detail'].includes(activePage)) {
      return;
    }
    refreshDocuments({ silent: true });
  }, [activePage]);

  useEffect(() => {
    if (!selectedSessionId) {
      setMessages([]);
      return;
    }
    refreshMessages(selectedSessionId);
  }, [selectedSessionId]);

  useEffect(() => {
    setSelectedReportPlanId((current) => {
      if (current && datasetReportPlans.some((plan) => plan.id === current)) {
        return current;
      }
      return datasetReportPlans[0]?.id || null;
    });
  }, [datasetReportPlans]);

  useEffect(() => {
    if (!selectedReportPlanId) {
      startTransition(() => {
        setReportRenderOutputs([]);
        setReportAstVersions([]);
        setPublishedReportDetail(null);
      });
      return;
    }

    refreshReportDetail(selectedReportPlanId);
  }, [selectedReportPlanId]);

  useEffect(() => {
    if (!selectedDatasetId || activePage === 'home') {
      return undefined;
    }

    const timer = window.setInterval(() => {
      refreshWorkspace(selectedDatasetId, {
        preferredSessionId: selectedSessionId,
        preserveNewSessionDraft: composingNewSession,
        silent: true,
      });
    }, DATASET_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [activePage, selectedDatasetId, selectedSessionId, composingNewSession]);

  useEffect(() => {
    if (!selectedSessionId) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      refreshMessages(selectedSessionId, { silent: true });
    }, MESSAGE_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [selectedSessionId]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      refreshCatalog({
        preferredDatasetId: selectedDatasetId,
        silent: true,
      });
    }, CATALOG_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [selectedDatasetId]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      refreshStaticPageDraftShelf({ silent: true });
      refreshHtmlArtifacts({ silent: true });
    }, STATIC_PAGE_SHELF_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const backendDraftId = activeStaticPageDraft?.backendDraftId;
    const jobStatus = activeStaticPageDraft?.imageJob?.status;
    if (!backendDraftId || !['queued', 'running'].includes(jobStatus)) {
      return undefined;
    }

    refreshBackendStaticPageDraft(backendDraftId, { silent: true });
    const timer = window.setInterval(() => {
      refreshBackendStaticPageDraft(backendDraftId, { silent: true });
    }, STATIC_PAGE_ACTIVE_JOB_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [activeStaticPageDraft?.backendDraftId, activeStaticPageDraft?.imageJob?.status]);

  useEffect(() => {
    if (!staticPageEditorOpen || activeStaticPageDraft?.imageJob?.status !== 'preview_ready') {
      return;
    }
    setStaticPageEditorOpen(false);
    setMobilePanel('chat');
    setBanner('可视化已生成，正在自动继续制作静态页。');
    appendStaticPageProgressMessage(
      `${activeStaticPageDraft.id}:preview-ready:${activeStaticPageDraft.previewImage?.assetKey || activeStaticPageDraft.previewContract?.assetKey || 'ready'}`,
      staticPagePreviewProgressContent(activeStaticPageDraft),
    );
  }, [activeStaticPageDraft?.id, activeStaticPageDraft?.imageJob?.status, staticPageEditorOpen]);

  useEffect(() => {
    const draft = activeStaticPageDraft;
    const backendDraftId = draft?.backendDraftId;
    const jobId = draft?.imageJob?.id || draft?.previewContract?.imageJobId || '';
    const jobStatus = draft?.imageJob?.status || draft?.previewContract?.status || '';
    const previewAssetKey = draft?.previewImage?.assetKey || draft?.previewContract?.assetKey || '';
    const finalStatus = draft?.finalPage?.status || '';

    if (
      !backendDraftId
      || jobStatus !== 'preview_ready'
      || !previewAssetKey
      || staticPageActionBusy
      || ['queued', 'rendering', 'rendered', 'mock_ready'].includes(finalStatus)
    ) {
      return;
    }

    const autoRenderKey = `${backendDraftId}:${jobId}:${previewAssetKey}`;
    if (staticPageAutoRenderKeysRef.current.has(autoRenderKey)) {
      return;
    }

    if (!canRequestStaticPageFinalRender(draft)) {
      setBanner(staticPageFinalRenderBlockReason(draft) || '可视化已生成，但最终页面生成条件还未满足。');
      return;
    }

    staticPageAutoRenderKeysRef.current.add(autoRenderKey);
    setStaticPageActionBusy(true);
    appendStaticPageProgressMessage(
      `${draft.id}:auto-render:${jobId || previewAssetKey}`,
      '已自动进入静态页制作，不需要再确认可视化。完成后会直接给出页面链接。',
    );
    const optimisticDraft = replaceDraftWithOperation(draft, {
      type: 'request_final_render',
      finalPage: {
        status: 'queued',
        renderer: 'platform-api-static-page-renderer',
        renderOutputId: '',
        imageJobId: jobId || null,
        directHtml: false,
        assetManifest: {
          status: 'queued',
          queue_copy: '最终静态页正在后台制作，可以继续聊天或修改其他内容。',
        },
      },
    });
    createBackendStaticPageRender(optimisticDraft, {
      previewImage: draft.previewImage || buildMockStaticPagePreview(draft),
    }).catch((syncError) => {
      staticPageAutoRenderKeysRef.current.delete(autoRenderKey);
      replaceStaticPageDraft(draft.id, draft);
      setBanner('');
      setError(`静态页未进入后台制作：${staticPagePreviewGateErrorMessage(syncError, '后端渲染暂不可用')}。`);
    }).finally(() => setStaticPageActionBusy(false));
  }, [
    activeStaticPageDraft?.backendDraftId,
    activeStaticPageDraft?.imageJob?.id,
    activeStaticPageDraft?.imageJob?.status,
    activeStaticPageDraft?.previewImage?.assetKey,
    activeStaticPageDraft?.previewContract?.assetKey,
    activeStaticPageDraft?.finalPage?.status,
    staticPageActionBusy,
  ]);

  useEffect(() => {
    if (staticPageActionBusy) {
      return;
    }
    const draft = staticPageDraftItems.find((candidate) => {
      if (!staticPageDraftReadyForAutoRender(candidate)) {
        return false;
      }
      const discoveryId = staticPageDraftDiscoveryId(candidate);
      const previous = discoveryId ? staticPageDraftStatusRef.current.get(discoveryId) : null;
      return Boolean(previous && !previous.previewReady);
    });
    if (!draft) {
      return;
    }
    const snapshot = staticPageDraftAsyncSnapshot(draft);
    const autoRenderKey = `${draft.backendDraftId}:${snapshot.jobId || snapshot.previewAssetKey}:${snapshot.previewAssetKey}`;
    if (staticPageAutoRenderKeysRef.current.has(autoRenderKey)) {
      return;
    }
    if (!canRequestStaticPageFinalRender(draft)) {
      return;
    }

    staticPageAutoRenderKeysRef.current.add(autoRenderKey);
    setStaticPageActionBusy(true);
    appendStaticPageProgressMessage(
      `${draft.id}:auto-render:${snapshot.jobId || snapshot.previewAssetKey}`,
      '后台发现可视化已生成，正在自动续接静态页制作。完成后会直接给出页面链接。',
    );
    createBackendStaticPageRender(draft, {
      previewImage: draft.previewImage || buildMockStaticPagePreview(draft),
    }).catch((syncError) => {
      setError(`静态页未进入后台制作：${staticPagePreviewGateErrorMessage(syncError, '后端渲染暂不可用')}。`);
    }).finally(() => setStaticPageActionBusy(false));
  }, [staticPageDraftItems, staticPageActionBusy]);

  useEffect(() => {
    const backendDraftId = activeStaticPageDraft?.backendDraftId;
    const renderStatus = activeStaticPageDraft?.finalPage?.status;
    if (!backendDraftId || !['queued', 'rendering'].includes(renderStatus)) {
      return undefined;
    }

    refreshBackendStaticPageDraft(backendDraftId, { silent: true });
    const timer = window.setInterval(() => {
      refreshBackendStaticPageDraft(backendDraftId, { silent: true });
    }, STATIC_PAGE_ACTIVE_RENDER_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [activeStaticPageDraft?.backendDraftId, activeStaticPageDraft?.finalPage?.status]);

  useEffect(() => {
    const draft = activeStaticPageDraft;
    if (!draft || draft.finalPage?.status !== 'rendered') {
      return;
    }
    if (!shouldAnnounceStaticPageRendered(draft)) {
      return;
    }
    const finalUrl = staticPageRenderedUrl(draft);
    appendStaticPageProgressMessage(
      `${draft.id}:rendered:${draft.finalPage?.renderOutputId || finalUrl || 'ready'}`,
      finalUrl
        ? `报表页面已生成：${finalUrl}`
        : '报表页面已生成。',
      { final: true },
    );
    const artifact = publishedStaticPageArtifactForDraft(draft);
    if (artifact?.id && activeHtmlArtifactId !== artifact.id) {
      setActiveHtmlArtifactId(artifact.id);
      setStaticPageEditorOpen(false);
      setMobilePanel('chat');
    }
  }, [
    activeStaticPageDraft?.id,
    activeStaticPageDraft?.finalPage?.status,
    activeStaticPageDraft?.finalPage?.renderOutputId,
    activeStaticPageDraft?.finalPage?.htmlPreviewUrl,
    activeStaticPageDraft?.finalPage?.htmlDownloadUrl,
    activeHtmlArtifactId,
    htmlArtifacts,
  ]);

  useEffect(() => {
    const previous = staticPageDraftStatusRef.current;
    const next = new Map();
    const previewTransitions = [];
    const renderedTransitions = [];

    staticPageDraftItems.forEach((draft) => {
      const discoveryId = staticPageDraftDiscoveryId(draft);
      if (!discoveryId) {
        return;
      }
      const snapshot = staticPageDraftAsyncSnapshot(draft);
      next.set(discoveryId, snapshot);
      const before = previous.get(discoveryId);
      if (!before) {
        return;
      }
      if (snapshot.previewReady && !before.previewReady && !snapshot.renderInProgress && !snapshot.rendered) {
        previewTransitions.push({ draft, snapshot });
      }
      if (
        snapshot.rendered
        && (
          !before.rendered
          || before.renderOutputId !== snapshot.renderOutputId
          || before.finalUrl !== snapshot.finalUrl
        )
      ) {
        renderedTransitions.push({ draft, snapshot });
      }
    });

    staticPageDraftStatusRef.current = next;
    if (!previous.size) {
      return;
    }

    const preview = previewTransitions[0];
    if (preview?.draft) {
      appendStaticPageProgressMessage(
        `${preview.draft.id}:preview-ready:${preview.snapshot.previewAssetKey || 'ready'}`,
        staticPagePreviewProgressContent(preview.draft, preview.snapshot),
      );
    }

    const rendered = renderedTransitions[0];
    if (!rendered?.draft) {
      return;
    }
    if (!shouldAnnounceStaticPageRendered(rendered.draft)) {
      return;
    }
    const finalUrl = rendered.snapshot.finalUrl;
    appendStaticPageProgressMessage(
      `${rendered.draft.id}:rendered:${rendered.snapshot.renderOutputId || finalUrl || 'ready'}`,
      finalUrl
        ? `报表页面已生成：${finalUrl}`
        : '报表页面已生成。',
      { final: true },
    );
    if (staticPageEditorOpen) {
      return;
    }
    const artifact = publishedStaticPageArtifactForDraft(rendered.draft);
    setActiveStaticPageDraftId(rendered.draft.id);
    setMobilePanel('chat');
    if (artifact?.id && activeHtmlArtifactId !== artifact.id) {
      setActiveHtmlArtifactId(artifact.id);
    }
  }, [staticPageDraftItems, activeHtmlArtifactId, htmlArtifacts, staticPageEditorOpen]);

  useEffect(() => {
    if (!selectedReportPlanId) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      refreshReportDetail(selectedReportPlanId, { silent: true });
    }, REPORT_DETAIL_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [selectedReportPlanId]);

  async function handleToggleDocumentDatasetMembership(datasetId) {
    if (!selectedDocumentId || !datasetId) {
      return false;
    }
    const documentDatasetIdList = documentDatasetIds(selectedDocument);
    const currentIds = documentDatasetIdList.length
      ? documentDatasetIdList
      : normalizeDatasetIds(selectedDatasetIds);
    const active = currentIds.includes(datasetId);
    setBanner('');
    setError('');
    setDocumentActionBusy(selectedDocumentId);
    try {
      const response = await fetchJson(
        `/api/v3/documents/${selectedDocumentId}/dataset-memberships/${datasetId}`,
        { method: active ? 'DELETE' : 'PUT' },
      );
      const nextIds = normalizeDatasetIds(response?.dataset_ids || response?.datasetIds || response?.document?.dataset_ids || response?.document?.datasetIds || []);
      if (nextIds.length) {
        setSelectedDatasetIds(nextIds);
        setSelectedDatasetId(nextIds[0]);
      }
      if (response?.document) {
        setDocuments((current) => current.map((document) => (
          document.id === response.document.id ? { ...document, ...response.document } : document
        )));
        setSelectedDocumentDetail((current) => (
          current?.document?.id === response.document.id
            ? { ...current, document: { ...current.document, ...response.document } }
            : current
        ));
      }
      setBanner(active ? '已将文档移出该数据集。' : '已将文档加入该数据集。');
      await refreshDocuments({ silent: true });
      await refreshDocumentDetail(selectedDocumentId);
      return true;
    } catch (membershipError) {
      setError(membershipError instanceof Error ? membershipError.message : '文档数据集归属更新失败');
      return false;
    } finally {
      setDocumentActionBusy('');
    }
  }

  async function handleToggleDatasetSelection(datasetId) {
    if (selectedDocumentId) {
      await handleToggleDocumentDatasetMembership(datasetId);
      return;
    }
    setBanner('');
    setError('');
    setComposingNewSession(false);
    setAssistantRunProgress(null);
    const currentIds = normalizeDatasetIds(selectedDatasetIds);
    const nextIds = currentIds.includes(datasetId)
      ? currentIds.filter((item) => item !== datasetId)
      : [...currentIds, datasetId];
    setSelectedDatasetIds(nextIds);
    setSelectedDatasetId(nextIds[0] || null);
    refreshStaticPageDraftShelf({ silent: true, datasetIds: nextIds });
    setMobileSidebarOpen(false);
    setMobilePanel('chat');
  }

  const stats = {
    sessions: sessions.length,
    outputs: outputs.length,
    plans: datasetReportPlans.length,
    published: datasetPublishedReports.length,
  };
  const sidebarProps = {
    datasets,
    selectedDatasetId,
    selectedDatasetIds,
    selectedDataset,
    selectedDatasets,
    selectedDocument,
    datasetDraft,
    onDatasetDraftChange: (field, value) =>
      setDatasetDraft((current) => ({ ...current, [field]: value })),
    onCreateDataset: handleCreateDataset,
    localSecretDraft,
    activeSecretCount,
    resolvingSecret,
    onLocalSecretDraftChange: setLocalSecretDraft,
    onResolveLocalSecret: handleResolveLocalSecret,
    onBindSelectedDatasetSecret: handleBindSelectedDatasetSecret,
    onClearLocalSecret: handleClearLocalSecret,
    onSelectDataset: handleToggleDatasetSelection,
    onClearDatasetSelection: () => {
      setBanner('已清空供料范围；后续对话会先按普通聊天处理，命中资料意图时会自动选中相关数据集。');
      setError('');
      setComposingNewSession(false);
      setSelectedDatasetId(null);
      setSelectedDatasetIds([]);
      setScopePlan({ candidates: [], hint: '' });
      setAssistantRunProgress(null);
      setMobileSidebarOpen(false);
      setMobilePanel('chat');
    },
    creatingDataset,
    stats,
    loading: bootstrapping || workspaceLoading,
    mobileOpen: mobileSidebarOpen,
    onClose: () => setMobileSidebarOpen(false),
    scopePlan,
    accountAuth: {
      emailDraft: accountEmailDraft,
      codeDraft: accountCodeDraft,
      newKeyDraft: accountNewKeyDraft,
      statusSummary: accountStatusSummary,
      busy: authBusy,
      message: authMessage,
      onEmailDraftChange: (value) => {
        setAccountEmailDraft(value);
        setAuthMessage('');
      },
      onCodeDraftChange: (value) => setAccountCodeDraft(normalizeVerificationCode(value)),
      onNewKeyDraftChange: (value) => setAccountNewKeyDraft(value),
      onSendEmailCode: handleSendEmailCode,
      onVerifyEmailCode: handleVerifyEmailCode,
      onLoginWithKey: handleLoginWithLocalKey,
      onClaimLocalData: handleClaimLocalData,
      onRotateLocalKey: handleRotateLocalKey,
      onLogout: handleLogoutAccount,
    },
  };
  const chatPanelProps = {
    dataset: selectedDataset,
    selectedDatasets,
    session: selectedSession,
    messages: visibleMessages,
    messageLoading,
    input,
    onInputChange: setInput,
    onSubmit: handleSubmitMessage,
    onStartNewConversation: handleStartNewConversation,
    submitting: submitting || uploadingFiles,
    uploadingFiles,
    onUploadClick: handleUploadButtonClick,
    reportEntryBusy,
    onResolveReportEntry: handleResolveReportEntry,
    staticPageDraft: activeStaticPageDraft,
    onStartStaticPageDraft: handleStartStaticPageDraft,
    onApplyStaticPageOperation: handleApplyStaticPageOperation,
    onStaticPagePrimaryAction: handleStaticPagePrimaryAction,
    staticPageActionBusy,
    onApplyStaticPagePrompt: handleApplyStaticPagePrompt,
    onRetryWorkflowExecution: handleRetryWorkflowExecution,
    onCancelWorkflowExecution: handleCancelWorkflowExecution,
    onRefreshStaticPageDraft: (backendDraftId) => refreshBackendStaticPageDraft(backendDraftId, { silent: false }),
    onOpenStaticPageBuilder: () => {
      if (activeStaticPageDraft) {
        setStaticPageEditorOpen(true);
        setActiveHtmlArtifactId(null);
        setMobilePanel('chat');
      }
    },
    onCloseStaticPageDraft: handleCloseStaticPageDraft,
    showStaticPageWorkspace: staticPageEditorOpen,
    startupBriefing: assistantStartupBriefing,
    scopePlan,
    assistantRunProgress,
    htmlArtifact: activeHtmlArtifact,
    onCloseHtmlArtifact: handleCloseHtmlArtifact,
    onHtmlArtifactEvent: handleHtmlArtifactEvent,
  };
  const insightPanelProps = {
    dataset: selectedDataset,
    sessions,
    selectedSessionId,
    outputs,
    reportPlans: datasetReportPlans,
    publishedReports: datasetPublishedReports,
    selectedReportPlanId,
    selectedReportPlan,
    reportRenderOutputs,
    reportAstVersions,
    publishedReportDetail,
    reportDetailLoading,
    reportActionBusy,
    reportSurface,
    publishNote,
    onSelectSession: handleSelectConversation,
    onSelectReportPlan: handleSelectReportPlanFromShelf,
    onSelectPublishedReport: handleSelectPublishedReportFromShelf,
    onReportSurfaceChange: setReportSurface,
    onPublishNoteChange: setPublishNote,
    onContinueReportPlan: handleContinueReportPlan,
    onRequestReportRender: handleRequestReportRender,
    onPublishReport: handlePublishReport,
    onRetryWorkflowExecution: handleRetryWorkflowExecution,
    onCancelWorkflowExecution: handleCancelWorkflowExecution,
    onRefreshReportDetail: () => {
      if (selectedReportPlanId) {
        refreshReportDetail(selectedReportPlanId);
      }
    },
    onRefreshReports: () => refreshCatalog({
      preferredDatasetId: selectedDatasetIds[0] || selectedDatasetId,
      silent: true,
    }),
    staticPageDraft: activeStaticPageDraft,
    staticPageDrafts: reportShelfStaticPageDraftItems,
    onSelectStaticPageDraft: handleSelectReportShelfStaticPageDraft,
    onPreviewStaticPageDraft: handlePreviewStaticPageDraft,
    onOpenStaticPageDraft: handleOpenStaticPageDraft,
    onSetDefaultStaticPageTemplate: handleSetDefaultStaticPageTemplate,
    onCancelDefaultStaticPageTemplate: handleCancelDefaultStaticPageTemplate,
    onDeleteStaticPageDraft: handleDeleteStaticPageDraft,
    onRevertStaticPageStage: handleRevertStaticPageStage,
    onRefreshStaticPageDrafts: () => refreshStaticPageDraftShelf({ silent: true }),
    staticPageEditorOpen,
    assistantRunProgress,
    codexCustomerTasks,
    codexCustomerArtifacts,
    htmlArtifacts,
    activeHtmlArtifactId,
    onSelectHtmlArtifact: handleSelectHtmlArtifact,
  };
  const directoryPanelProps = {
    activePage,
    datasets,
    selectedDatasetId,
    selectedDatasetIds,
    onSelectDataset: sidebarProps.onSelectDataset,
    onClearDatasetSelection: sidebarProps.onClearDatasetSelection,
    datasetDraft,
    onDatasetDraftChange: sidebarProps.onDatasetDraftChange,
    onCreateDataset: handleCreateDataset,
    creatingDataset,
    documents,
    documentsLoading,
    documentSearch,
    onDocumentSearchChange: setDocumentSearch,
    selectedDocumentId,
    onFocusDocumentMembership: handleFocusDocumentMembership,
    onClearDocumentSelection: handleClearDocumentSelection,
    onOpenDocumentPage: handleOpenDocumentPage,
    onBackToDatasets: () => {
      handleClearDocumentSelection();
      setActivePage('datasets');
    },
    selectedDocumentDetail,
    documentDetailLoading,
    onRefreshDocuments: () => refreshDocuments({ silent: false }),
    onUpdateDataset: handleUpdateDataset,
    onArchiveDataset: handleArchiveDataset,
    datasetActionBusy,
    onUpdateDocument: handleUpdateDocument,
    onArchiveDocuments: handleArchiveDocuments,
    documentActionBusy,
    onToggleDocumentDatasetMembership: handleToggleDocumentDatasetMembership,
    stats,
    accountStatusSummary,
    activityEvents,
    htmlArtifacts,
  };
  const uploadInput = (
    <input
      ref={fileInputRef}
      className="hidden-file-input"
      type="file"
      multiple
      onChange={handleUploadFiles}
      aria-label="上传文件并自动分类"
    />
  );

  if (mobileViewport) {
    return (
      <>
        <HomeMobileShell
          sidebarProps={sidebarProps}
          chatPanelProps={chatPanelProps}
          insightPanelProps={insightPanelProps}
          sessions={conversationMenuSessions}
          selectedSessionId={selectedSessionId}
          currentConversationTitle={currentConversationTitle}
          composingNewSession={composingNewSession}
          accountAuth={sidebarProps.accountAuth}
          onSelectSession={handleSelectConversation}
          onStartNewConversation={handleStartNewConversation}
          selectedDataset={selectedDataset}
          selectedDatasets={selectedDatasets}
          stats={stats}
          loading={bootstrapping || workspaceLoading}
          banner={banner}
          error={error}
          staticPageDraft={activeStaticPageDraft}
          staticPageEditorOpen={staticPageEditorOpen}
          onStaticPageEditorOpenChange={setStaticPageEditorOpen}
          onApplyStaticPageOperation={handleApplyStaticPageOperation}
        />
        {uploadInput}
      </>
    );
  }

  return (
    <div className="app-shell assistant-shell">
      <Sidebar {...sidebarProps} />
      {uploadInput}
      {mobileSidebarOpen ? (
        <button
          type="button"
          className="mobile-drawer-backdrop"
          aria-label="关闭数据集侧栏"
          onClick={() => setMobileSidebarOpen(false)}
        />
      ) : null}

      <main className="main-panel main-panel-home">
        <HomeWorkspaceToolbar
          activePage={activePage}
          onPageChange={handlePageChange}
          selectedDataset={selectedDataset}
          selectedDatasets={selectedDatasets}
          stats={stats}
          sessions={conversationMenuSessions}
          selectedSession={selectedSession}
          selectedSessionId={selectedSessionId}
          currentConversationTitle={currentConversationTitle}
          composingNewSession={composingNewSession}
          loading={bootstrapping}
          workspaceLoading={workspaceLoading}
          documents={documents}
          sourceItems={toolbarSourceItems}
          accountAuth={sidebarProps.accountAuth}
          onStartNewConversation={handleStartNewConversation}
          onSelectSession={handleSelectConversation}
          onRenameConversation={handleRenameConversation}
        />

        <section
          className={`workspace-grid homepage-workspace ${activePage === 'home' ? '' : 'page-directory-workspace'} mobile-panel-${mobilePanel}`.trim()}
        >
          {activePage === 'home' ? (
            <>
              <ChatPanel {...chatPanelProps} />
              <InsightPanel {...insightPanelProps} />
            </>
          ) : (
            <WorkspaceDirectoryPanel {...directoryPanelProps} />
          )}
        </section>
      </main>
    </div>
  );
}
