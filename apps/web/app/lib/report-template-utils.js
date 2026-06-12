import { createLocalMessage } from './local-chat-sessions.js';

export function reportPlanIdFromPublished(report) {
  return String(
    report?.report_plan_id
      || report?.reportPlanId
      || report?.plan_id
      || report?.planId
      || '',
  ).trim();
}

export function publishedReportForPlan(plan, publishedReports = []) {
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

export function firstArtifactUrlFromObject(value) {
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

export function reportTemplateTitle(candidate, fallback = '当前数据集报表模板') {
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

export function reportTemplateCandidateId(candidate) {
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

export function reportTemplateUrl(candidate) {
  return firstArtifactUrlFromObject(candidate?.detail)
    || firstArtifactUrlFromObject(candidate?.published)
    || firstArtifactUrlFromObject(candidate?.plan)
    || firstArtifactUrlFromObject(candidate?.draft || candidate?.staticPageDraft || candidate?.staticDraft)
    || '';
}

export function reportShelfSelectionContent(title) {
  const cleanTitle = String(title || '当前').replace(/^静态页[：:]\s*/, '').trim() || '当前';
  const reportName = /报表|报告|看板|页面/.test(cleanTitle) ? cleanTitle : `${cleanTitle}报表`;
  return `已选中「${reportName}」，你可以继续修改。`;
}

export function buildReportShelfSelectionLocalMessage(title, metadata = {}, options = {}) {
  const messageFactory = typeof options.messageFactory === 'function'
    ? options.messageFactory
    : createLocalMessage;
  const content = reportShelfSelectionContent(title);
  return {
    ...messageFactory('assistant', content),
    metadata: {
      source: 'report_shelf_selection',
      ...metadata,
    },
  };
}
