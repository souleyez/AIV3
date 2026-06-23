import { formatSnakeCaseLabel } from './formatters.js';
import { normalizeHtmlArtifactManifest } from './html-artifact-manifest.js';
import { isCustomerFacingHtmlArtifact, staticPageRenderedUrlFromDraft, staticPageSafePreviewPath } from './html-artifact-utils.js';

const TASK_STATUSES = new Set([
  'queued',
  'running',
  'retrying',
  'needs_review',
  'published',
  'completed',
  'failed',
  'cancelled',
]);

const STATUS_LABELS = {
  queued: '已排队',
  running: '执行中',
  retrying: '重试中',
  needs_review: '待确认',
  published: '已发布',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消',
};

const STATUS_RANK = {
  published: 90,
  completed: 80,
  needs_review: 70,
  retrying: 60,
  running: 50,
  queued: 40,
  failed: 30,
  cancelled: 20,
};

const FILE_KIND_LABELS = {
  html: '页面',
  static_page: '页面',
  preview_image: '效果图',
  table_data: '表格数据',
  report_ppt: '导出PPT',
  report_markdown: '文本下载（MD）',
  zip: 'ZIP',
  pptx: '下载PPTX',
  markdown: 'Markdown',
  csv: 'CSV',
  client_html: 'HTML',
};

const ACTIVE_STATUSES = new Set(['queued', 'running', 'retrying', 'needs_review']);
const ARTIFACT_PRODUCING_CODEX_TASKS = new Set([
  'customer_artifact_request',
  'generated_static_page_edit',
  'generated_static_page_publish',
]);

function compactText(value, limit = 180) {
  return String(value || '').trim().replace(/\s+/g, ' ').slice(0, limit);
}

function firstText(values = [], limit = 180) {
  for (const value of values) {
    const text = compactText(value, limit);
    if (text) return text;
  }
  return '';
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function asObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function stableId(prefix, ...parts) {
  const raw = parts.map((part) => compactText(part, 120)).find(Boolean) || prefix;
  return `${prefix}:${raw.replace(/[^a-zA-Z0-9_-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 140) || 'item'}`;
}

export function normalizeArtifactTaskStatus(value, fallback = 'running') {
  const raw = compactText(value, 80).toLowerCase();
  if (!raw) return TASK_STATUSES.has(fallback) ? fallback : 'running';
  if (TASK_STATUSES.has(raw)) return raw;
  if (['pending', 'scheduled', 'accepted', 'created', 'not_started'].includes(raw)) return 'queued';
  if (['active', 'processing', 'in_progress', 'executing', 'rendering', 'planning', 'draft', 'planned'].includes(raw)) return 'running';
  if (['waiting', 'wait', 'poll_retry', 'retry_wait', 'fallback_started', 'exec_fallback_started'].includes(raw)) return 'retrying';
  if (['preview_ready', 'effect_confirmed', 'confirmed', 'blocked', 'rejected', 'needs_human', 'requires_confirmation'].includes(raw)) return 'needs_review';
  if (['rendered', 'available', 'done', 'success', 'succeeded', 'ready'].includes(raw)) return 'completed';
  if (['published', 'public', 'live'].includes(raw)) return 'published';
  if (['error', 'errored', 'failure'].includes(raw)) return 'failed';
  if (['canceled'].includes(raw)) return 'cancelled';
  return TASK_STATUSES.has(fallback) ? fallback : 'running';
}

export function artifactTaskStatusLabel(status) {
  return STATUS_LABELS[normalizeArtifactTaskStatus(status)] || formatSnakeCaseLabel(status);
}

function fileKindLabel(kind) {
  return FILE_KIND_LABELS[kind] || formatSnakeCaseLabel(kind || 'file');
}

function siblingUrl(baseUrl, fileName) {
  const value = compactText(baseUrl, 500);
  if (!value || !fileName) return '';
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
    return value.startsWith('/') ? url.pathname : url.toString();
  } catch {
    return '';
  }
}

function createFile({
  id,
  kind = 'file',
  label,
  url = '',
  path = '',
  mimeType = '',
  source = '',
  selectMode = '',
}) {
  const fileUrl = compactText(url, 800);
  const filePath = compactText(path, 800);
  if (!fileUrl && !filePath) return null;
  const fileId = compactText(id, 180)
    || compactText(`${kind}:${fileUrl || filePath || label || source}`, 180);
  if (!fileId) return null;
  return {
    id: fileId,
    kind,
    label: label || fileKindLabel(kind),
    url: fileUrl,
    path: filePath,
    mimeType: compactText(mimeType, 120),
    source: compactText(source, 80),
    canOpen: Boolean(fileUrl) && ['html', 'static_page', 'preview_image', 'client_html'].includes(kind),
    canDownload: Boolean(fileUrl || filePath),
    selectMode: selectMode || (['html', 'static_page', 'client_html'].includes(kind) ? 'edit' : 'download'),
  };
}

function uniqueFiles(files = []) {
  const seen = new Set();
  return asArray(files).filter(Boolean).filter((file) => {
    const locator = compactText(file.url || file.path, 800);
    const key = locator
      ? `locator:${locator}`
      : file.id || `${file.kind}:${file.label}`;
    if (!key || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function uniqueRefs(refs = []) {
  const seen = new Set();
  return asArray(refs).filter(Boolean).map((ref) => ({
    kind: compactText(ref.kind || 'ref', 60),
    id: compactText(ref.id || ref.value || '', 160),
    label: compactText(ref.label || ref.title || ref.id || '', 120),
  })).filter((ref) => {
    const key = `${ref.kind}:${ref.id || ref.label}`;
    if (!ref.id && !ref.label) return false;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function primaryFileId(files = []) {
  const priority = ['html', 'static_page', 'client_html', 'preview_image', 'report_ppt', 'report_markdown', 'table_data', 'zip'];
  const found = [...files].sort((left, right) => {
    const leftIndex = priority.indexOf(left.kind);
    const rightIndex = priority.indexOf(right.kind);
    return (leftIndex === -1 ? 99 : leftIndex) - (rightIndex === -1 ? 99 : rightIndex);
  })[0];
  return found?.id || '';
}

function cardStatusFromDraft(draft) {
  const finalPage = draft?.finalPage || {};
  const raw = finalPage.status || draft?.status || draft?.backendStatus || draft?.backend_status || '';
  const status = normalizeArtifactTaskStatus(raw, 'running');
  if (status === 'completed' && staticPageRenderedUrlFromDraft(draft)) return 'published';
  return status;
}

function staticPageTitle(draft) {
  const finalPage = draft?.finalPage || {};
  const manifest = finalPage.assetManifest || finalPage.asset_manifest || {};
  return firstText([
    manifest.reportTitle,
    manifest.report_title,
    manifest.displayTitle,
    manifest.display_title,
    manifest.title,
    finalPage.reportTitle,
    finalPage.report_title,
    draft?.title,
    draft?.objective,
    draft?.prompt,
  ], 120) || '静态页任务';
}

function staticPageSummary(draft) {
  const finalPage = draft?.finalPage || {};
  return firstText([
    finalPage.notice,
    draft?.modelSummary,
    draft?.model_summary,
    draft?.summary,
    draft?.source?.missingEvidence?.message,
    draft?.source?.missing_evidence?.message,
  ], 180) || '页面规划、可视化、HTML 生成和发布会在任务卡内持续推进。';
}

function staticPageUpdatedAt(draft) {
  return firstText([
    draft?.backendUpdatedAt,
    draft?.updated_at,
    draft?.updatedAt,
    draft?.created_at,
    draft?.createdAt,
  ], 80);
}

function staticPageWorkflowExecutionId(draft) {
  const finalPage = draft?.finalPage || {};
  const manifest = finalPage.assetManifest || finalPage.asset_manifest || {};
  const workflow = manifest.workflow || {};
  return firstText([
    finalPage.workflowExecutionId,
    finalPage.workflow_execution_id,
    finalPage.renderWorkflowExecutionId,
    finalPage.render_workflow_execution_id,
    workflow.executionId,
    workflow.execution_id,
    workflow.workflowExecutionId,
    manifest.workflowExecutionId,
    manifest.workflow_execution_id,
  ], 120);
}

function staticPageRenderOutputId(draft) {
  const finalPage = draft?.finalPage || {};
  const manifest = finalPage.assetManifest || finalPage.asset_manifest || {};
  return firstText([
    finalPage.renderOutputId,
    finalPage.render_output_id,
    manifest.renderOutputId,
    manifest.render_output_id,
  ], 120);
}

function staticPageFiles(draft) {
  const finalPage = draft?.finalPage || {};
  const manifest = finalPage.assetManifest || finalPage.asset_manifest || {};
  const pageUrl = staticPageRenderedUrlFromDraft(draft);
  const imageUrl = firstText([
    draft?.imageJob?.publicUrl,
    draft?.imageJob?.public_url,
    draft?.imageJob?.previewUrl,
    draft?.imageJob?.preview_url,
    draft?.imageJob?.url,
    finalPage.imageUrl,
    finalPage.image_url,
    manifest.imageUrl,
    manifest.image_url,
    manifest.previewImageUrl,
    manifest.preview_image_url,
  ], 800);
  const exportPackage = asObject(manifest.exportPackage || manifest.export_package);
  return uniqueFiles([
    pageUrl ? createFile({
      id: `static-html:${pageUrl}`,
      kind: 'html',
      label: 'index.html',
      url: pageUrl,
      path: pageUrl,
      source: 'static_page_draft',
      selectMode: 'edit',
    }) : null,
    imageUrl ? createFile({
      id: `static-image:${imageUrl}`,
      kind: 'preview_image',
      label: '效果图',
      url: imageUrl,
      path: imageUrl,
      source: 'static_page_draft',
      selectMode: 'preview',
    }) : null,
    createFile({
      id: `static-csv:${pageUrl}`,
      kind: 'table_data',
      label: 'table-data.csv',
      url: firstText([manifest.dataUrl, manifest.data_url, exportPackage.dataUrl, exportPackage.data_url], 800) || siblingUrl(pageUrl, 'table-data.csv'),
      source: 'static_page_draft',
    }),
    createFile({
      id: `static-ppt:${pageUrl}`,
      kind: 'report_ppt',
      label: 'report.ppt',
      url: firstText([manifest.pptUrl, manifest.ppt_url, exportPackage.pptUrl, exportPackage.ppt_url], 800) || siblingUrl(pageUrl, 'report.ppt'),
      source: 'static_page_draft',
    }),
    createFile({
      id: `static-md:${pageUrl}`,
      kind: 'report_markdown',
      label: 'report.md',
      url: firstText([manifest.markdownUrl, manifest.markdown_url, exportPackage.markdownUrl, exportPackage.markdown_url], 800) || siblingUrl(pageUrl, 'report.md'),
      source: 'static_page_draft',
    }),
    createFile({
      id: `static-zip:${pageUrl}`,
      kind: 'zip',
      label: 'ZIP',
      url: firstText([manifest.zipUrl, manifest.zip_url, exportPackage.zipUrl, exportPackage.zip_url], 800) || siblingUrl(pageUrl, 'report.zip'),
      source: 'static_page_draft',
    }),
  ]);
}

function stageStatus(done, active, failed = false) {
  if (failed) return 'failed';
  if (done) return 'completed';
  if (active) return 'running';
  return 'queued';
}

function staticPageStages(draft, cardStatus, files = []) {
  const finalStatus = draft?.finalPage?.status || draft?.status || '';
  const hasImage = Boolean(draft?.imageJob?.id || draft?.imageJob?.status || files.some((file) => file.kind === 'preview_image'));
  const hasHtml = Boolean(files.some((file) => file.kind === 'html'));
  const failed = cardStatus === 'failed';
  return [
    { key: 'plan', label: '计划', status: stageStatus(true, false) },
    { key: 'data', label: '数据补充', status: stageStatus(Boolean(draft?.dataSnapshot || draft?.source || draft?.source_refs), !hasImage && !hasHtml && !failed) },
    { key: 'image2', label: 'Image2 设计', status: stageStatus(hasImage || hasHtml, ['queued', 'preview_ready', 'effect_confirmed'].includes(finalStatus), failed && !hasHtml) },
    { key: 'html', label: 'HTML 生成', status: stageStatus(hasHtml, ['queued', 'rendering'].includes(finalStatus), failed && !hasHtml) },
    { key: 'publish', label: '发布', status: stageStatus(cardStatus === 'published', cardStatus === 'completed') },
    { key: 'files', label: '文件就绪', status: stageStatus(files.length > 0, ACTIVE_STATUSES.has(cardStatus)) },
  ];
}

function buildStaticPageDraftCard(draft) {
  if (!draft) return null;
  const files = staticPageFiles(draft);
  const status = cardStatusFromDraft(draft);
  const draftId = draft.backendDraftId || draft.backend_draft_id || draft.id || staticPageRenderedUrlFromDraft(draft);
  const renderOutputId = staticPageRenderOutputId(draft);
  const workflowExecutionId = staticPageWorkflowExecutionId(draft);
  return {
    id: stableId('static_page_draft', draftId || renderOutputId || workflowExecutionId),
    kind: 'static_page_draft',
    title: staticPageTitle(draft),
    status,
    statusLabel: artifactTaskStatusLabel(status),
    phase: files.length ? '文件就绪' : status === 'failed' ? '生成失败' : '生成推进中',
    updatedAt: staticPageUpdatedAt(draft),
    summary: staticPageSummary(draft),
    detail: {
      stages: staticPageStages(draft, status, files),
      retryableReason: firstText([draft?.finalPage?.failureReason, draft?.finalPage?.failure_reason, draft?.failureReason, draft?.failure_reason], 200),
      nextAction: status === 'failed' ? '可重试或在对话里说明修改要求。' : '可继续等待、打开详情或在对话里要求修改报表。',
      workflowExecutionId,
      renderOutputId,
    },
    sourceRefs: uniqueRefs([
      draft.id ? { kind: 'static_page_draft', id: draft.id, label: '本地草稿' } : null,
      draft.backendDraftId ? { kind: 'static_page_draft', id: draft.backendDraftId, label: '后端草稿' } : null,
      renderOutputId ? { kind: 'static_page_render_output', id: renderOutputId, label: 'Render Output' } : null,
      workflowExecutionId ? { kind: 'workflow_execution', id: workflowExecutionId, label: 'Workflow' } : null,
    ]),
    files,
    primaryFileId: primaryFileId(files),
    canOpen: files.some((file) => file.canOpen),
    canEdit: status === 'published' || files.some((file) => file.kind === 'html'),
    canRetry: status === 'failed' && Boolean(workflowExecutionId),
    canCancel: ['queued', 'running', 'retrying'].includes(status) && Boolean(workflowExecutionId),
    raw: { staticPageDraft: draft },
    dedupeKeys: [
      draft.id ? `draft:${draft.id}` : '',
      draft.backendDraftId ? `draft:${draft.backendDraftId}` : '',
      renderOutputId ? `render:${renderOutputId}` : '',
      workflowExecutionId ? `workflow:${workflowExecutionId}` : '',
      staticPageRenderedUrlFromDraft(draft) ? `url:${staticPageRenderedUrlFromDraft(draft)}` : '',
    ].filter(Boolean),
  };
}

function reportPlanTitle(plan, published) {
  return firstText([
    published?.title,
    published?.name,
    plan?.title,
    plan?.name,
    plan?.objective,
  ], 120) || '报表任务';
}

function publishedReportUrl(report) {
  return firstText([
    report?.public_url,
    report?.publicUrl,
    report?.generated_artifact_url,
    report?.generatedArtifactUrl,
    report?.url,
  ], 800);
}

function buildReportPlanCard(plan, publishedReports = []) {
  if (!plan) return null;
  const published = asArray(publishedReports).find((report) => {
    const planId = report?.report_plan_id || report?.reportPlanId || report?.plan_id || report?.planId || '';
    return planId && planId === plan.id;
  });
  const url = publishedReportUrl(published);
  const rawStatus = published ? 'published' : plan.status || plan.lifecycle || '';
  const status = normalizeArtifactTaskStatus(rawStatus, 'running');
  const files = uniqueFiles([
    url ? createFile({
      id: `report-html:${url}`,
      kind: 'html',
      label: '报表页面',
      url,
      path: url,
      source: 'published_report',
      selectMode: 'edit',
    }) : null,
  ]);
  const id = plan.id || published?.id || reportPlanTitle(plan, published);
  return {
    id: stableId('report_plan', id),
    kind: 'report_plan',
    title: reportPlanTitle(plan, published),
    status,
    statusLabel: artifactTaskStatusLabel(status),
    phase: published ? '发布版本' : '报表规划',
    updatedAt: firstText([published?.updated_at, published?.updatedAt, published?.created_at, plan?.updated_at, plan?.created_at], 80),
    summary: firstText([published?.summary, published?.description, plan?.objective, plan?.summary], 180) || '选择后可继续规划、渲染或发布这个报表。',
    detail: {
      stages: [
        { key: 'plan', label: '计划', status: 'completed' },
        { key: 'render', label: '渲染输出', status: published ? 'completed' : 'running' },
        { key: 'publish', label: '发布', status: published ? 'completed' : 'queued' },
        { key: 'files', label: '文件就绪', status: files.length ? 'completed' : 'queued' },
      ],
      nextAction: published ? '可打开报表或在对话里要求修改。' : '可继续规划或启动渲染。',
    },
    sourceRefs: uniqueRefs([
      plan.id ? { kind: 'report_plan', id: plan.id, label: 'Report Plan' } : null,
      published?.id ? { kind: 'published_report', id: published.id, label: 'Published Report' } : null,
    ]),
    files,
    primaryFileId: primaryFileId(files),
    canOpen: Boolean(url),
    canEdit: Boolean(plan.id || url),
    canRetry: false,
    canCancel: false,
    raw: { reportPlan: plan, publishedReport: published || null },
    dedupeKeys: [
      plan.id ? `report_plan:${plan.id}` : '',
      published?.id ? `published_report:${published.id}` : '',
      url ? `url:${url}` : '',
    ].filter(Boolean),
  };
}

function buildPublishedReportCard(report) {
  if (!report) return null;
  const url = publishedReportUrl(report);
  const id = report.id || report.report_id || report.reportId || url || report.title;
  const files = uniqueFiles([
    url ? createFile({
      id: `published-report-html:${url}`,
      kind: 'html',
      label: '报表页面',
      url,
      path: url,
      source: 'published_report',
      selectMode: 'edit',
    }) : null,
  ]);
  return {
    id: stableId('published_report', id),
    kind: 'published_report',
    title: firstText([report.title, report.name, report.report_title, report.reportTitle], 120) || '已发布报表',
    status: 'published',
    statusLabel: artifactTaskStatusLabel('published'),
    phase: '发布版本',
    updatedAt: firstText([report.updated_at, report.updatedAt, report.created_at, report.createdAt], 80),
    summary: firstText([report.summary, report.description, report.publish_note, report.publishNote], 180) || '已发布报表，可打开查看或继续修改。',
    detail: {
      stages: [
        { key: 'plan', label: '计划', status: 'completed' },
        { key: 'render', label: '渲染输出', status: 'completed' },
        { key: 'publish', label: '发布', status: 'completed' },
        { key: 'files', label: '文件就绪', status: files.length ? 'completed' : 'queued' },
      ],
      nextAction: '可打开报表或在对话里要求修改。',
    },
    sourceRefs: uniqueRefs([
      id ? { kind: 'published_report', id, label: 'Published Report' } : null,
    ]),
    files,
    primaryFileId: primaryFileId(files),
    canOpen: Boolean(url),
    canEdit: Boolean(url),
    canRetry: false,
    canCancel: false,
    raw: { publishedReport: report },
    dedupeKeys: [
      id ? `published_report:${id}` : '',
      report.report_plan_id ? `report_plan:${report.report_plan_id}` : '',
      report.reportPlanId ? `report_plan:${report.reportPlanId}` : '',
      url ? `url:${url}` : '',
    ].filter(Boolean),
  };
}

function htmlArtifactGeneratedFiles(artifact, manifest) {
  const generatedArtifacts = manifest.payload?.generatedArtifacts || manifest.payload?.generated_artifacts || {};
  const files = asArray(generatedArtifacts.files);
  const sourceRunId = manifest.provenance?.sourceRunId || '';
  const localThreadId = manifest.payload?.localThreadId || manifest.payload?.local_thread_id || '';
  const normalized = files.map((file, index) => {
    const kind = compactText(file?.artifactKind || file?.artifact_kind || file?.kind || 'file', 80);
    const path = compactText(file?.path || file?.uri || '', 800);
    const params = new URLSearchParams();
    if (sourceRunId) {
      params.set('assistant_run_id', sourceRunId);
    } else if (localThreadId) {
      params.set('local_thread_id', localThreadId);
    }
    const url = params.toString()
      ? `/api/v3/html-artifacts/${encodeURIComponent(manifest.id)}/files/${index}?${params.toString()}`
      : compactText(file?.url || file?.publicUrl || file?.public_url || '', 800);
    return createFile({
      id: `html-artifact:${manifest.id}:${index}`,
      kind,
      label: fileKindLabel(kind),
      url,
      path,
      mimeType: file?.mimeType || file?.mime_type || '',
      source: 'html_artifact',
      selectMode: kind === 'html' ? 'edit' : 'download',
    });
  });
  if (manifest.templateId === 'static_page_published_preview') {
    const previewPath = manifest.payload?.previewPath || manifest.payload?.preview_path || '';
    return uniqueFiles([
      previewPath ? createFile({
        id: `html-artifact-preview:${manifest.id}`,
        kind: 'html',
        label: 'index.html',
        url: previewPath,
        path: previewPath,
        source: 'html_artifact',
        selectMode: 'edit',
      }) : null,
      createFile({
        id: `html-artifact-csv:${manifest.id}`,
        kind: 'table_data',
        label: 'table-data.csv',
        url: siblingUrl(previewPath, 'table-data.csv'),
        source: 'html_artifact',
      }),
      createFile({
        id: `html-artifact-ppt:${manifest.id}`,
        kind: 'report_ppt',
        label: 'report.ppt',
        url: siblingUrl(previewPath, 'report.ppt'),
        source: 'html_artifact',
      }),
      createFile({
        id: `html-artifact-md:${manifest.id}`,
        kind: 'report_markdown',
        label: 'report.md',
        url: siblingUrl(previewPath, 'report.md'),
        source: 'html_artifact',
      }),
      ...normalized,
    ]);
  }
  return uniqueFiles(normalized);
}

function buildHtmlArtifactCard(artifact) {
  if (!artifact) return null;
  if (!isCustomerFacingHtmlArtifact(artifact)) return null;
  const normalized = normalizeHtmlArtifactManifest(artifact);
  if (normalized.rejected) {
    const id = normalized.sourceId || artifact.id || artifact.artifact_id || 'rejected-html-artifact';
    return {
      id: stableId('html_artifact', id),
      kind: 'html_artifact',
      title: artifact.title || '已拦截 HTML 产物',
      status: 'failed',
      statusLabel: artifactTaskStatusLabel('failed'),
      phase: '安全拦截',
      updatedAt: artifact.createdAt || artifact.created_at || '',
      summary: normalized.reason || 'manifest 未通过安全规则。',
      detail: { stages: [{ key: 'validate', label: '校验', status: 'failed' }], retryableReason: normalized.reason },
      sourceRefs: uniqueRefs([{ kind: 'html_artifact', id, label: 'HTML Artifact' }]),
      files: [],
      primaryFileId: '',
      canOpen: false,
      canEdit: false,
      canRetry: false,
      canCancel: false,
      raw: { htmlArtifact: artifact },
      dedupeKeys: [`artifact:${id}`],
    };
  }
  const manifest = normalized.manifest;
  const status = manifest.templateId === 'static_page_published_preview'
    ? 'published'
    : normalizeArtifactTaskStatus(manifest.payload?.status || manifest.payload?.deliverableStatus?.state || 'completed', 'completed');
  const files = htmlArtifactGeneratedFiles(artifact, manifest);
  const ownerScope = manifest.ownerScope || {};
  const renderOutputId = manifest.payload?.renderOutputId || manifest.payload?.render_output_id || '';
  const workflowExecutionId = manifest.payload?.workflowExecutionId || manifest.payload?.workflow_execution_id || '';
  const previewPath = manifest.payload?.previewPath || manifest.payload?.preview_path || '';
  return {
    id: stableId('html_artifact', manifest.id),
    kind: 'html_artifact',
    title: manifest.title || 'HTML 产物',
    status,
    statusLabel: artifactTaskStatusLabel(status),
    phase: manifest.templateLabel || '产物',
    updatedAt: manifest.createdAt,
    summary: firstText([
      manifest.payload?.summary,
      manifest.provenance?.reason,
      manifest.templateLabel,
    ], 180) || '产物已生成，可打开查看或继续处理。',
    detail: {
      stages: [
        { key: 'validate', label: '校验', status: 'completed' },
        { key: 'publish', label: '发布', status: status === 'published' ? 'completed' : 'queued' },
        { key: 'files', label: '文件就绪', status: files.length ? 'completed' : 'queued' },
      ],
      workflowExecutionId,
      renderOutputId,
      nextAction: '可打开详情，或在对话里要求继续修改。',
    },
    sourceRefs: uniqueRefs([
      { kind: 'html_artifact', id: manifest.id, label: 'HTML Artifact' },
      ownerScope?.id ? { kind: ownerScope.type || 'owner', id: ownerScope.id, label: ownerScope.type || 'Owner' } : null,
      renderOutputId ? { kind: 'static_page_render_output', id: renderOutputId, label: 'Render Output' } : null,
      workflowExecutionId ? { kind: 'workflow_execution', id: workflowExecutionId, label: 'Workflow' } : null,
    ]),
    files,
    primaryFileId: primaryFileId(files),
    canOpen: files.some((file) => file.canOpen) || Boolean(staticPageSafePreviewPath(previewPath)),
    canEdit: manifest.interactionMode !== 'read_only' || manifest.templateId === 'static_page_published_preview',
    canRetry: false,
    canCancel: false,
    raw: { htmlArtifact: artifact },
    dedupeKeys: [
      `artifact:${manifest.id}`,
      ownerScope?.type === 'static_page_draft' && ownerScope.id ? `draft:${ownerScope.id}` : '',
      renderOutputId ? `render:${renderOutputId}` : '',
      workflowExecutionId ? `workflow:${workflowExecutionId}` : '',
      previewPath ? `url:${previewPath}` : '',
    ].filter(Boolean),
  };
}

function buildCodexTaskCard(task) {
  if (!task) return null;
  if (!ARTIFACT_PRODUCING_CODEX_TASKS.has(compactText(task.capability, 80))
    && !ARTIFACT_PRODUCING_CODEX_TASKS.has(compactText(task.route, 80))) {
    return null;
  }
  const workflowExecutionId = task.workflowExecutionId || task.workflow_execution_id || '';
  const status = normalizeArtifactTaskStatus(task.status, 'running');
  return {
    id: stableId('codex_task', workflowExecutionId || task.id || task.title),
    kind: 'codex_task',
    title: task.title || 'Codex 执行任务',
    status,
    statusLabel: task.statusLabel || artifactTaskStatusLabel(status),
    phase: task.route ? formatSnakeCaseLabel(task.route) : '受控执行',
    updatedAt: task.createdAt || task.created_at || '',
    summary: task.resultSummary?.summary || task.summary || '客户任务已进入受控执行链路。',
    detail: {
      stages: [
        { key: 'queued', label: '排队', status: ['queued', 'running', 'retrying', 'completed', 'published'].includes(status) ? 'completed' : status },
        { key: 'execute', label: '执行', status: ['completed', 'published'].includes(status) ? 'completed' : status },
        { key: 'files', label: '文件就绪', status: ['completed', 'published'].includes(status) ? 'completed' : 'queued' },
      ],
      retryableReason: task.retryable ? '任务标记为可重试。' : '',
      nextAction: status === 'failed' ? '可根据失败原因重试或补充要求。' : '等待执行器写回结果。',
      workflowExecutionId,
      permissionScope: task.permissionScope || '',
      resultItems: [
        ...asArray(task.resultSummary?.findings).map((text) => ({ label: '结论', text })),
        ...asArray(task.resultSummary?.recommendedNextActions).map((text) => ({ label: '建议', text })),
        ...asArray(task.resultSummary?.warnings).map((text) => ({ label: '提示', text })),
      ].slice(0, 4),
    },
    sourceRefs: uniqueRefs([
      task.id ? { kind: 'codex_task', id: task.id, label: 'Codex Task' } : null,
      workflowExecutionId ? { kind: 'workflow_execution', id: workflowExecutionId, label: 'Workflow' } : null,
    ]),
    files: [],
    primaryFileId: '',
    canOpen: false,
    canEdit: task.capability === 'generated_static_page_edit',
    canRetry: status === 'failed' && task.retryable === true,
    canCancel: ['queued', 'running', 'retrying'].includes(status) && Boolean(workflowExecutionId),
    raw: { codexCustomerTask: task },
    dedupeKeys: [
      task.id ? `codex_task:${task.id}` : '',
      workflowExecutionId ? `workflow:${workflowExecutionId}` : '',
    ].filter(Boolean),
  };
}

function buildCodexArtifactCard(bundle) {
  if (!bundle) return null;
  const workflowExecutionId = bundle.workflowExecutionId || bundle.workflow_execution_id || '';
  const status = bundle.published || bundle.primaryUrl ? 'published' : bundle.requiresPublishValidation ? 'needs_review' : 'completed';
  const files = uniqueFiles([
    bundle.primaryUrl ? createFile({
      id: `codex-primary:${bundle.primaryUrl}`,
      kind: 'html',
      label: '打开产物',
      url: bundle.primaryUrl,
      path: bundle.primaryUrl,
      source: 'codex_customer_artifact',
      selectMode: 'edit',
    }) : null,
    ...asArray(bundle.files).map((file, index) => createFile({
      id: `codex-file:${bundle.id || workflowExecutionId}:${index}:${file.path || file.publicUrl}`,
      kind: file.kind || 'file',
      label: file.title || fileKindLabel(file.kind || 'file'),
      url: file.publicUrl || file.public_url || '',
      path: file.path || '',
      mimeType: file.mimeType || file.mime_type || '',
      source: 'codex_customer_artifact',
      selectMode: file.publicUrl ? 'download' : 'inspect',
    })),
  ]);
  return {
    id: stableId('codex_artifact', bundle.id || workflowExecutionId || bundle.primaryUrl || bundle.title),
    kind: 'codex_artifact',
    title: bundle.title || 'Codex 产物',
    status,
    statusLabel: bundle.statusLabel || artifactTaskStatusLabel(status),
    phase: bundle.published ? '已发布' : '发布校验',
    updatedAt: bundle.createdAt || bundle.created_at || '',
    summary: bundle.summary || `${files.length} 个文件`,
    detail: {
      stages: [
        { key: 'execute', label: '执行', status: 'completed' },
        { key: 'publish', label: '发布', status: status === 'published' ? 'completed' : 'running' },
        { key: 'files', label: '文件就绪', status: files.length ? 'completed' : 'queued' },
      ],
      nextAction: status === 'published' ? '可打开产物或继续要求修改。' : '等待发布校验生成公开链接。',
      workflowExecutionId,
      manifestPath: bundle.manifestPath || '',
    },
    sourceRefs: uniqueRefs([
      bundle.id ? { kind: 'codex_artifact', id: bundle.id, label: 'Codex Artifact' } : null,
      workflowExecutionId ? { kind: 'workflow_execution', id: workflowExecutionId, label: 'Workflow' } : null,
    ]),
    files,
    primaryFileId: primaryFileId(files),
    canOpen: files.some((file) => file.canOpen),
    canEdit: Boolean(bundle.primaryUrl),
    canRetry: false,
    canCancel: false,
    raw: { codexCustomerArtifact: bundle },
    dedupeKeys: [
      bundle.id ? `codex_artifact:${bundle.id}` : '',
      workflowExecutionId ? `workflow:${workflowExecutionId}` : '',
      bundle.primaryUrl ? `url:${bundle.primaryUrl}` : '',
    ].filter(Boolean),
  };
}

function clientArtifactFileKind(file = {}) {
  const role = compactText(file.role, 80);
  const contentType = compactText(file.content_type || file.contentType, 120).toLowerCase();
  const filename = compactText(file.filename || file.name, 180).toLowerCase();
  if (role === 'primary_html' || contentType.includes('html') || filename.endsWith('.html')) return 'client_html';
  if (contentType.includes('markdown') || filename.endsWith('.md')) return 'report_markdown';
  if (contentType.includes('csv') || filename.endsWith('.csv')) return 'csv';
  if (contentType.includes('zip') || filename.endsWith('.zip')) return 'zip';
  return role || 'file';
}

function clientArtifactDownloadUrl(file = {}) {
  const raw = compactText(file.download_url || file.downloadUrl || '', 800);
  if (!raw) return '';
  if (raw.startsWith('/v1/')) {
    return `/api/v3/${raw.slice('/v1/'.length)}`;
  }
  return raw;
}

function clientArtifactPreviewUrl(file = {}) {
  const raw = compactText(file.preview_url || file.previewUrl || '', 800);
  if (!raw) return '';
  if (raw.startsWith('/v1/')) {
    return `/api/v3/${raw.slice('/v1/'.length)}`;
  }
  return raw;
}

function clientArtifactPublicUrl(file = {}) {
  return compactText(file.public_url || file.publicUrl || '', 800);
}

function clientArtifactFiles(artifact) {
  const manifest = asObject(artifact?.manifest);
  const manifestFiles = asArray(manifest.files);
  const recordFiles = asArray(artifact?.files);
  const files = recordFiles.length ? recordFiles : manifestFiles;
  return uniqueFiles(files.map((file, index) => {
    const filename = compactText(file.filename || file.name || `file-${index + 1}`, 180);
    const publicUrl = clientArtifactPublicUrl(file);
    const previewUrl = clientArtifactPreviewUrl(file);
    return createFile({
      id: `client-artifact-file:${artifact?.artifact_id || artifact?.artifactId || artifact?.id}:${index}:${filename}`,
      kind: clientArtifactFileKind(file),
      label: filename,
      url: publicUrl || previewUrl || clientArtifactDownloadUrl(file),
      path: filename,
      mimeType: file.content_type || file.contentType || '',
      source: 'v3_client_artifact',
      selectMode: publicUrl ? 'edit' : (previewUrl ? 'inspect' : 'download'),
    });
  }));
}

function buildClientArtifactCard(artifact) {
  if (!artifact) return null;
  const artifactId = artifact.artifact_id || artifact.artifactId || artifact.id || '';
  const manifest = asObject(artifact.manifest);
  const taskId = artifact.task_id || artifact.taskId || manifest.task_id || manifest.taskId || '';
  const rawStatus = artifact.status === 'received' ? 'completed' : artifact.status;
  const status = normalizeArtifactTaskStatus(rawStatus, 'completed');
  const published = status === 'published';
  const files = clientArtifactFiles(artifact);
  const datasetIds = asArray(artifact.dataset_ids || artifact.datasetIds || manifest.dataset_ids || manifest.datasetIds);
  const assetLibraryIds = asArray(artifact.asset_library_ids || artifact.assetLibraryIds || manifest.asset_library_ids || manifest.assetLibraryIds);
  return {
    id: stableId('v3_client_artifact', artifactId || taskId || artifact.title),
    kind: 'v3_client_artifact',
    title: firstText([artifact.title, manifest.title], 120) || '客户端上传产物',
    status,
    statusLabel: artifactTaskStatusLabel(status),
    phase: published ? 'V3 已发布' : 'V3 已接收',
    updatedAt: artifact.updated_at || artifact.updatedAt || artifact.created_at || artifact.createdAt || manifest.created_at || manifest.createdAt || '',
    summary: firstText([
      artifact.summary,
      manifest.metadata?.summary,
      `${files.length} 个文件已回传到 V3`,
    ], 180),
    detail: {
      stages: [
        { key: 'upload', label: '上传', status: 'completed' },
        { key: 'attach', label: '关联', status: (datasetIds.length || assetLibraryIds.length) ? 'completed' : 'queued' },
        { key: 'publish', label: '发布', status: published ? 'completed' : 'queued' },
        { key: 'files', label: '文件就绪', status: files.length ? 'completed' : 'queued' },
      ],
      nextAction: published
        ? '已进入 V3 企业产物表；如已公开发布，可直接打开页面链接。'
        : '可在 V3 继续关联、解析或走发布链路。',
      taskId,
    },
    sourceRefs: uniqueRefs([
      artifactId ? { kind: 'v3_client_artifact', id: artifactId, label: 'Client Artifact' } : null,
      taskId ? { kind: 'client_task', id: taskId, label: 'Client Task' } : null,
      ...datasetIds.map((id) => ({ kind: 'dataset', id, label: id })),
      ...assetLibraryIds.map((id) => ({ kind: 'asset_library', id, label: id })),
    ]),
    files,
    primaryFileId: primaryFileId(files),
    canOpen: published && files.some((file) => file.kind === 'client_html' && file.url),
    canEdit: false,
    canRetry: status === 'failed',
    canCancel: false,
    raw: { clientArtifact: artifact },
    dedupeKeys: [
      artifactId ? `v3_client_artifact:${artifactId}` : '',
      taskId ? `client_task:${taskId}` : '',
    ].filter(Boolean),
  };
}

function mergeRaw(existingRaw = {}, incomingRaw = {}) {
  return Object.fromEntries(Object.entries({
    ...existingRaw,
    ...incomingRaw,
  }).filter(([, value]) => value !== undefined && value !== null));
}

function mergeCards(existing, incoming) {
  const existingRank = STATUS_RANK[existing.status] || 0;
  const incomingRank = STATUS_RANK[incoming.status] || 0;
  const incomingNewer = String(incoming.updatedAt || '').localeCompare(String(existing.updatedAt || '')) > 0;
  const lead = incomingRank > existingRank || (incomingRank === existingRank && incomingNewer) ? incoming : existing;
  const other = lead === incoming ? existing : incoming;
  const files = uniqueFiles([...lead.files, ...other.files]);
  const sourceRefs = uniqueRefs([...lead.sourceRefs, ...other.sourceRefs]);
  return {
    ...other,
    ...lead,
    files,
    primaryFileId: primaryFileId(files),
    sourceRefs,
    summary: lead.summary || other.summary,
    detail: {
      ...other.detail,
      ...lead.detail,
      stages: lead.detail?.stages?.length ? lead.detail.stages : other.detail?.stages || [],
    },
    canOpen: lead.canOpen || other.canOpen || files.some((file) => file.canOpen),
    canEdit: lead.canEdit || other.canEdit,
    canRetry: lead.canRetry || other.canRetry,
    canCancel: lead.canCancel || other.canCancel,
    raw: mergeRaw(other.raw, lead.raw),
    dedupeKeys: Array.from(new Set([...(other.dedupeKeys || []), ...(lead.dedupeKeys || [])])),
  };
}

function buildTaskCardsFromInputs({
  reportPlans = [],
  publishedReports = [],
  staticPageDrafts = [],
  htmlArtifacts = [],
  codexCustomerTasks = [],
  codexCustomerArtifacts = [],
  clientArtifacts = [],
} = {}) {
  const planIds = new Set(asArray(reportPlans).map((plan) => plan?.id).filter(Boolean));
  return [
    ...asArray(reportPlans).map((plan) => buildReportPlanCard(plan, publishedReports)),
    ...asArray(publishedReports)
      .filter((report) => {
        const planId = report?.report_plan_id || report?.reportPlanId || report?.plan_id || report?.planId || '';
        return !planId || !planIds.has(planId);
      })
      .map(buildPublishedReportCard),
    ...asArray(staticPageDrafts).map(buildStaticPageDraftCard),
    ...asArray(htmlArtifacts).map(buildHtmlArtifactCard),
    ...asArray(codexCustomerTasks).map(buildCodexTaskCard),
    ...asArray(codexCustomerArtifacts).map(buildCodexArtifactCard),
    ...asArray(clientArtifacts).map(buildClientArtifactCard),
  ].filter(Boolean);
}

export function buildArtifactTaskCards(inputs = {}) {
  const cards = [];
  const keyToIndex = new Map();

  buildTaskCardsFromInputs(inputs).forEach((card) => {
    const keys = Array.from(new Set([card.id, ...(card.dedupeKeys || [])].filter(Boolean)));
    const existingIndex = keys
      .map((key) => keyToIndex.get(key))
      .find((index) => Number.isInteger(index));
    if (Number.isInteger(existingIndex)) {
      cards[existingIndex] = mergeCards(cards[existingIndex], card);
      keys.concat(cards[existingIndex].dedupeKeys || []).forEach((key) => keyToIndex.set(key, existingIndex));
      return;
    }
    const nextIndex = cards.length;
    cards.push(card);
    keys.forEach((key) => keyToIndex.set(key, nextIndex));
  });

  return cards
    .map((card) => {
      const files = uniqueFiles(card.files || []);
      return {
        ...card,
        status: normalizeArtifactTaskStatus(card.status),
        statusLabel: card.statusLabel || artifactTaskStatusLabel(card.status),
        files,
        primaryFileId: card.primaryFileId || primaryFileId(files),
        canOpen: card.canOpen || files.some((file) => file.canOpen),
        canEdit: Boolean(card.canEdit),
        canRetry: Boolean(card.canRetry),
        canCancel: Boolean(card.canCancel),
      };
    })
    .sort((left, right) => {
      const statusDelta = (ACTIVE_STATUSES.has(right.status) ? 1 : 0) - (ACTIVE_STATUSES.has(left.status) ? 1 : 0);
      if (statusDelta) return statusDelta;
      return String(right.updatedAt || '').localeCompare(String(left.updatedAt || ''));
    });
}
