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
  normalizeAccountEmail,
  normalizeVerificationCode,
  summarizeAccountState,
  validateAccountEmail,
} from './lib/account-auth';
import { buildAssistantStartupBriefing, formatStartupBriefingForModel } from './lib/assistant-startup-briefing';
import { planAssistantScope, selectPlannerDatasetIds } from './lib/scope-planner';
import {
  applyStaticPageOperation,
  applyStaticPageOperations,
  buildInitialStaticPageDraft,
  buildMockStaticPagePreview,
  buildStaticPageImagePayload,
  canRequestStaticPageFinalRender,
  interpretStaticPagePrompt,
  staticPageFinalRenderBlockReason,
} from './lib/static-page-draft';
import {
  buildLocalUploadObjectKey,
  buildUploadDatasetPayload,
  classifyUploadTarget,
  inferUploadMediaKind,
  isPublicUploadClassification,
  summarizeUploadClassification,
} from './lib/upload-classifier';

const DATASET_POLL_INTERVAL_MS = 5000;
const MESSAGE_POLL_INTERVAL_MS = 3000;
const CATALOG_POLL_INTERVAL_MS = 12000;
const REPORT_DETAIL_POLL_INTERVAL_MS = 6000;
const STATIC_PAGE_SHELF_POLL_INTERVAL_MS = 12000;
const STATIC_PAGE_ACTIVE_JOB_POLL_INTERVAL_MS = 4000;
const STATIC_PAGE_ACTIVE_RENDER_POLL_INTERVAL_MS = 4000;
const LOCAL_CHAT_STORAGE_KEY = 'aidp-v3-local-chat-messages';
const LOCAL_ACTIVITY_STORAGE_KEY = 'aidp-v3-local-activity-events';
const LOCAL_THREAD_ID_STORAGE_KEY = 'aidp-v3-local-thread-id';
const LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY = 'aidp-v3-local-assistant-run-id';
const LOCAL_SECRET_BINDING_IDS_STORAGE_KEY = 'aidp-v3-secret-binding-ids';
const LOCAL_SECRET_VALUE_STORAGE_KEY = 'aidp-v3-local-secret-value';
const LOCAL_ACCOUNT_EMAIL_STORAGE_KEY = 'aidp-v3-account-email';
const STATIC_PAGE_QUEUE_MESSAGE = '资源正在排队，可以联系商务开通高级用户跳过等待。';
const ASSISTANT_RUN_PROGRESS_LIMIT = 8;
const ASSISTANT_RUN_TRACE_LIMIT = 6;

function compactConversationSummary(value, maxLength = 28) {
  const text = String(value || '').replace(/\s+/g, ' ').trim();
  if (!text) {
    return '新对话';
  }
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
}

function formatConversationTitleTime(value = new Date()) {
  const date = value instanceof Date ? value : new Date(value);
  const safeDate = Number.isNaN(date.getTime()) ? new Date() : date;
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(safeDate).replace(/\//g, '-');
}

function buildDefaultConversationTitle(prompt, startedAt = new Date()) {
  return `${formatConversationTitleTime(startedAt)} · ${compactConversationSummary(prompt)}`;
}

function limitAssistantRunText(value, maxLength = 80) {
  const text = String(value || '').trim();
  if (!text) {
    return '';
  }
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
}

function sanitizeAssistantRunTrailStep(step) {
  if (!step || typeof step !== 'object') {
    return null;
  }
  const label = limitAssistantRunText(step.label || step.react_action || step.status, 44);
  if (!label) {
    return null;
  }
  return {
    label,
    status: limitAssistantRunText(step.status || 'completed', 24),
    message: limitAssistantRunText(step.safe_message || step.message || step.hint || '', 72),
    suppliedCount: Number.isFinite(Number(step.supplied_count)) ? Number(step.supplied_count) : null,
    detailTargetCount: Number.isFinite(Number(step.detail_target_count)) ? Number(step.detail_target_count) : null,
    returnedCount: Number.isFinite(Number(step.returned_count ?? step.item_count)) ? Number(step.returned_count ?? step.item_count) : null,
    deniedCount: Number.isFinite(Number(step.denied_count)) ? Number(step.denied_count) : null,
    stepCount: Number.isFinite(Number(step.step_count)) ? Number(step.step_count) : null,
    reactStep: Number.isFinite(Number(step.react_step)) ? Number(step.react_step) : null,
  };
}

function sanitizeAssistantRunTraceStep(step) {
  if (!step || typeof step !== 'object') {
    return null;
  }
  const actionType = limitAssistantRunText(step.action_type || 'react_action', 40);
  if (!actionType) {
    return null;
  }
  return {
    actionType,
    status: limitAssistantRunText(step.status || 'unknown', 24),
    message: limitAssistantRunText(step.safe_message || '', 72),
    deniedCount: Number.isFinite(Number(step.denied_count)) ? Number(step.denied_count) : 0,
    returnedCount: Number.isFinite(Number(step.returned_count)) ? Number(step.returned_count) : 0,
    detailTargetCount: Number.isFinite(Number(step.detail_target_count)) ? Number(step.detail_target_count) : 0,
    durationMs: Number.isFinite(Number(step.duration_ms)) ? Number(step.duration_ms) : null,
  };
}

function buildAssistantRunProgress(response, continued = false) {
  const run = response?.run || {};
  const runtime = response?.runtime || run.runtime || {};
  const trail = Array.isArray(response?.execution_trail)
    ? response.execution_trail
    : Array.isArray(run.execution_trail)
      ? run.execution_trail
      : [];
  const trace = Array.isArray(runtime?.react_trace?.steps) ? runtime.react_trace.steps : [];
  const steps = trail
    .map(sanitizeAssistantRunTrailStep)
    .filter(Boolean)
    .slice(-ASSISTANT_RUN_PROGRESS_LIMIT);
  const traceSteps = trace
    .map(sanitizeAssistantRunTraceStep)
    .filter(Boolean)
    .slice(-ASSISTANT_RUN_TRACE_LIMIT);

  if (!steps.length && !traceSteps.length) {
    return null;
  }
  return {
    runId: response?.assistant_run_id || run.id || '',
    continued,
    steps,
    traceSteps,
  };
}

function readLocalThreadId() {
  if (typeof window === 'undefined') {
    return 'server-render-thread';
  }
  try {
    const existing = window.localStorage.getItem(LOCAL_THREAD_ID_STORAGE_KEY);
    if (existing) {
      return existing;
    }
    const next = createLocalThreadId();
    window.localStorage.setItem(LOCAL_THREAD_ID_STORAGE_KEY, next);
    return next;
  } catch {
    return 'browser-thread-unavailable';
  }
}

function createLocalThreadId() {
  return globalThis.crypto?.randomUUID?.() || `thread-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function writeLocalThreadId(threadId) {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(LOCAL_THREAD_ID_STORAGE_KEY, threadId);
  } catch {
    // The browser cache is a convenience; AssistantRun can still use the current in-memory thread.
  }
}

function buildAutoDatasetIdentity(existingCount = 0) {
  const now = new Date();
  const safeStamp = Number.isFinite(now.getTime()) ? now.getTime().toString(36) : String(Date.now());
  const randomPart = Math.random().toString(36).slice(2, 7);
  const titleTime = now.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
  return {
    key: `dataset-${safeStamp}-${randomPart}`,
    title: `新数据集 ${existingCount + 1} · ${titleTime}`,
  };
}

function readLocalSecretBindingIdsHeader() {
  if (typeof window === 'undefined') {
    return '';
  }
  try {
    const raw = window.localStorage.getItem(LOCAL_SECRET_BINDING_IDS_STORAGE_KEY) || '';
    return raw
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean)
      .join(',');
  } catch {
    return '';
  }
}

function readLocalSecretBindingIds() {
  return readLocalSecretBindingIdsHeader()
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function writeLocalSecretState(secretValue, secretBindingIds) {
  if (typeof window === 'undefined') {
    return;
  }
  const normalizedBindingIds = [...new Set((secretBindingIds || []).filter(Boolean))];
  window.localStorage.setItem(LOCAL_SECRET_BINDING_IDS_STORAGE_KEY, normalizedBindingIds.join(','));
  if (secretValue) {
    window.localStorage.setItem(LOCAL_SECRET_VALUE_STORAGE_KEY, secretValue);
  }
}

function clearLocalSecretState() {
  if (typeof window === 'undefined') {
    return;
  }
  window.localStorage.removeItem(LOCAL_SECRET_BINDING_IDS_STORAGE_KEY);
  window.localStorage.removeItem(LOCAL_SECRET_VALUE_STORAGE_KEY);
}

function readLocalSecretValue() {
  if (typeof window === 'undefined') {
    return '';
  }
  try {
    return window.localStorage.getItem(LOCAL_SECRET_VALUE_STORAGE_KEY) || '';
  } catch {
    return '';
  }
}

function readLocalAccountEmail() {
  if (typeof window === 'undefined') {
    return '';
  }
  try {
    return window.localStorage.getItem(LOCAL_ACCOUNT_EMAIL_STORAGE_KEY) || '';
  } catch {
    return '';
  }
}

function writeLocalAccountEmail(email) {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    const normalized = normalizeAccountEmail(email);
    if (normalized) {
      window.localStorage.setItem(LOCAL_ACCOUNT_EMAIL_STORAGE_KEY, normalized);
    } else {
      window.localStorage.removeItem(LOCAL_ACCOUNT_EMAIL_STORAGE_KEY);
    }
  } catch {
    // Account email is a convenience cache; auth is still cookie based.
  }
}

async function fingerprintLocalSecret(secretValue) {
  const normalized = String(secretValue || '').trim();
  if (!normalized) {
    return '';
  }
  if (!globalThis.crypto?.subtle) {
    throw new Error('当前浏览器不支持本地密钥指纹计算。');
  }
  const bytes = new TextEncoder().encode(normalized);
  const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
}

async function fetchJson(url, options = {}) {
  const isFormData = typeof FormData !== 'undefined' && options.body instanceof FormData;
  const secretBindingIds = readLocalSecretBindingIdsHeader();
  const response = await fetch(url, {
    cache: 'no-store',
    credentials: 'include',
    ...options,
    headers: {
      Accept: 'application/json',
      ...(options.body && !isFormData ? { 'Content-Type': 'application/json' } : {}),
      ...(secretBindingIds ? { 'X-AI-Data-Platform-Secret-Binding-Ids': secretBindingIds } : {}),
      'X-AI-Data-Platform-Local-Thread-Id': readLocalThreadId(),
      ...(options.headers || {}),
    },
    body: options.body && !isFormData && typeof options.body !== 'string'
      ? JSON.stringify(options.body)
      : options.body,
  });

  const contentType = response.headers.get('content-type') || '';
  const payload = contentType.includes('application/json')
    ? await response.json()
    : await response.text();

  if (!response.ok) {
    const message = typeof payload === 'string'
      ? payload
      : payload?.message || payload?.payload?.message || payload?.error || `Request failed: ${response.status}`;
    throw new Error(message);
  }

  return payload;
}

function sortByDateDesc(items, fieldName) {
  return [...(Array.isArray(items) ? items : [])].sort((left, right) => {
    const leftValue = new Date(left?.[fieldName] || 0).getTime();
    const rightValue = new Date(right?.[fieldName] || 0).getTime();
    return rightValue - leftValue;
  });
}

function sortDatasets(items) {
  return [...(Array.isArray(items) ? items : [])].sort((left, right) =>
    String(left?.title || left?.key || '').localeCompare(String(right?.title || right?.key || ''), 'zh-CN'),
  );
}

function normalizeDatasetIds(ids) {
  return [...new Set((Array.isArray(ids) ? ids : [ids])
    .map((id) => String(id || '').trim())
    .filter(Boolean))];
}

function sameDatasetIds(left, right) {
  const leftIds = normalizeDatasetIds(left);
  const rightIds = normalizeDatasetIds(right);
  return leftIds.length === rightIds.length && leftIds.every((id, index) => id === rightIds[index]);
}

function sortStaticPageDrafts(items) {
  return [...(Array.isArray(items) ? items : [])].sort((left, right) => {
    const leftValue = new Date(left?.backendUpdatedAt || left?.updated_at || left?.updatedAt || left?.created_at || 0).getTime();
    const rightValue = new Date(right?.backendUpdatedAt || right?.updated_at || right?.updatedAt || right?.created_at || 0).getTime();
    return rightValue - leftValue;
  });
}

function buildStaticPagePlanningHtmlArtifact(draft) {
  if (!draft) return null;
  const id = draft.backendDraftId || draft.id || 'local-static-page-draft';
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
      visualBridge: {
        providerLane: 'gpt-image-2-cloudflare-queue',
        role: 'effect_preview_reference_only',
        rule: '效果图只锁定视觉方向和确认指纹；最终 HTML 由 Draft JSON、DataSnapshot、VisualSpec 和 renderer 生成。',
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

function firstDatasetIdFromScope(scope) {
  return datasetIdsFromScope(scope)[0] || '';
}

function scopeHintFromCandidates(candidates) {
  const labels = (Array.isArray(candidates) ? candidates : [])
    .map((candidate) => candidate?.label)
    .filter(Boolean)
    .slice(0, 3);
  return labels.length ? `可能相关：${labels.join('、')}` : '';
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

function readLocalAssistantRunId() {
  if (typeof window === 'undefined') {
    return '';
  }
  try {
    return window.localStorage.getItem(LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY) || '';
  } catch {
    return '';
  }
}

function writeLocalAssistantRunId(runId) {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    if (runId) {
      window.localStorage.setItem(LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY, runId);
    } else {
      window.localStorage.removeItem(LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY);
    }
  } catch {
    // AssistantRun id is a convenience cache; chat still works without it.
  }
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
  const [documentSearch, setDocumentSearch] = useState('');
  const [selectedDocumentId, setSelectedDocumentId] = useState('');
  const [selectedDocumentDetail, setSelectedDocumentDetail] = useState(null);

  const datasetLoadIdRef = useRef(0);
  const messageLoadIdRef = useRef(0);
  const reportDetailLoadIdRef = useRef(0);
  const fileInputRef = useRef(null);

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
  const selectedSession = useMemo(
    () => sessions.find((session) => session.id === selectedSessionId) || null,
    [sessions, selectedSessionId],
  );
  const datasetPublishedReports = useMemo(
    () => publishedReports.filter((report) => report.dataset_id === selectedDatasetId),
    [publishedReports, selectedDatasetId],
  );
  const datasetReportPlans = useMemo(
    () => reportPlans.filter((plan) => plan.dataset_id === selectedDatasetId),
    [reportPlans, selectedDatasetId],
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
  const htmlArtifacts = useMemo(
    () => mergeHtmlArtifacts(
      backendHtmlArtifacts,
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
  const assistantStartupBriefing = useMemo(
    () => buildAssistantStartupBriefing({
      datasets,
      reportPlans,
      publishedReports,
      latestMessages: visibleMessages,
      activityEvents,
      selectedDataset,
      selectedDatasets,
      activeStaticPageDraft,
      staticPageDrafts: staticPageDraftItems,
    }),
    [activityEvents, activeStaticPageDraft, datasets, publishedReports, reportPlans, selectedDataset, selectedDatasets, staticPageDraftItems, visibleMessages],
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

  function promptRequestsStaticPage(prompt) {
    return /静态页|静态页面|页面规划|一页|生成页面|落地页/.test(String(prompt || ''));
  }

  function buildStaticPageConversationSummary(prompt = '', options = {}) {
    const draftDataset = options.dataset || selectedDataset;
    const draftDatasets = Array.isArray(options.datasets) && options.datasets.length
      ? options.datasets
      : selectedDatasets;
    const draftSession = options.session || selectedSession;
    const sourceMessages = options.messages || visibleMessages;
    const latestAssistantMessage = [...sourceMessages].reverse().find((message) => message.role === 'assistant');
    const latestMessage = latestAssistantMessage || sourceMessages[sourceMessages.length - 1];
    const summaryParts = [
      draftDatasets.length
        ? `数据集：${draftDatasets.map((dataset) => dataset.title || dataset.key).join('、')}`
        : draftDataset ? `数据集：${draftDataset.title}` : '未选数据集，按普通对话意图规划。',
      draftSession ? `会话：${draftSession.title}` : '',
      latestMessage?.content ? `最近内容：${latestMessage.content}` : '',
      prompt ? `用户要求：${prompt}` : '',
    ].filter(Boolean);
    return summaryParts.join('\n');
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
      source: draft?.source || {},
    };
  }

  function mergeBackendStaticPageDraft(localDraft, backendDraft) {
    const payload = backendDraft?.draft_payload && typeof backendDraft.draft_payload === 'object'
      ? backendDraft.draft_payload
      : {};
    const merged = {
      ...localDraft,
      ...payload,
      id: backendDraft?.id || localDraft.id,
      localDraftId: localDraft.localDraftId || localDraft.id,
      backendDraftId: backendDraft?.id || localDraft.backendDraftId || '',
      assistantRunId: backendDraft?.assistant_run_id || localDraft.assistantRunId || '',
      backendStatus: backendDraft?.status || localDraft.backendStatus || '',
      backendUpdatedAt: backendDraft?.updated_at || localDraft.backendUpdatedAt || '',
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
          || (status === 'preview_ready' ? '效果图已生成，等待客户确认。' : STATIC_PAGE_QUEUE_MESSAGE),
      },
      previewImage: imageJob.preview_asset_key
        ? buildConfirmedStaticPagePreview(draft, imageJob, draft.previewImage)
        : draft.previewImage,
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
    const response = await fetchJson(`/api/v3/assistant-runs/${assistantRunId}/static-page-drafts`, {
      method: 'POST',
      body: {
        title: localDraft?.objective || '静态页草稿',
        prompt,
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
        setBanner(`静态页草稿已同步；效果图队列暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
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

  async function refreshStaticPageDraftShelf(options = {}) {
    const { silent = true } = options;
    const query = new URLSearchParams({
      local_thread_id: readLocalThreadId(),
      limit: '12',
    });
    try {
      const backendDrafts = await fetchJson(`/api/v3/static-page-drafts?${query.toString()}`);
      const hydratedDrafts = await Promise.all(
        (Array.isArray(backendDrafts) ? backendDrafts : []).map((item) => hydrateBackendStaticPageDraft(item)),
      );
      setStaticPageDrafts((current) => {
        const next = { ...current };
        hydratedDrafts.forEach((draft) => {
          if (draft?.id) {
            next[draft.id] = draft;
          }
        });
        return next;
      });
      if (!silent) {
        setBanner(hydratedDrafts.length ? `已刷新 ${hydratedDrafts.length} 个静态页草稿/成品。` : '当前终端还没有静态页草稿。');
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
        }),
      },
    });
    const imageJob = response?.image_job;
    const latestDraft = staticPageDrafts[baseDraft.id] || staticPageDrafts[baseDraft.backendDraftId] || baseDraft;
    const draft = replaceDraftWithOperation(latestDraft, staticPageImageJobQueueOperation(imageJob, operation));
    setBanner(`效果图任务已进入资源队列，当前前方约 ${imageJob?.queue_position ?? 1} 个任务。`);
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
      throw new Error('效果图任务不存在。');
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
    setBanner('效果图已确认，下一步可以按效果制作静态页。');
    return { imageJob: response?.image_job, draft };
  }

  async function createBackendStaticPageRender(baseDraft, operation = {}) {
    if (!baseDraft?.backendDraftId) {
      throw new Error('静态页草稿还没有同步到后端。');
    }
    let draft = baseDraft;
    let imageJobId = isBackendStaticPageImageJobId(draft.imageJob?.id) ? draft.imageJob.id : '';
    if (draft.imageJob?.status !== 'confirmed') {
      const confirmed = await confirmBackendStaticPagePreview(draft, operation);
      draft = confirmed.draft;
      imageJobId = confirmed.imageJob?.id || draft.imageJob?.id || imageJobId;
    }
    const response = await fetchJson(`/api/v3/static-page-drafts/${draft.backendDraftId}/renders`, {
      method: 'POST',
      body: {
        image_job_id: imageJobId || null,
        background: true,
      },
    });
    const renderOutput = response?.render_output;
    const renderStatus = renderOutput?.status || 'queued';
    const rendered = applyStaticPageOperation(draft, {
      type: 'request_final_render',
      finalPage: {
        status: renderStatus,
        renderer: 'platform-api-static-page-renderer',
        renderOutputId: renderOutput?.id || '',
        imageJobId: renderOutput?.image_job_id || imageJobId || null,
        assetManifest: renderOutput?.asset_manifest || {},
        html: renderOutput?.html || '',
      },
    });
    const merged = mergeBackendStaticPageDraft(rendered, response?.draft);
    const finalDraft = {
      ...merged,
      status: renderStatus === 'rendered' ? 'rendered' : 'rendering',
      finalPage: rendered.finalPage,
    };
    replaceStaticPageDraft(baseDraft.id, finalDraft);
    setBanner(renderStatus === 'rendered'
      ? '最终静态页已按确认效果图生成。'
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
  }) {
    const messages = visibleMessages
      .slice(-12)
      .map((message) => ({ role: message.role, content: message.content }));
    if (continueRunId && promptRequestsAssistantContinue(prompt)) {
      try {
        const continued = await fetchJson(`/api/v3/assistant-runs/${continueRunId}/continue`, {
          method: 'POST',
          body: {
            prompt,
            max_steps: 3,
            current_artifact: activeStaticPageDraft || null,
            messages,
          },
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

    const created = await fetchJson('/api/v3/assistant-runs', {
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
    });
    return {
      response: created,
      assistantRunId: created?.assistant_run_id || '',
      assistantContent: created?.assistant_message?.content || '',
      continued: false,
    };
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
    });
    const ingestResponse = await fetchJson(`/api/v3/documents/${registered.document.id}/ingest`, {
      method: 'POST',
    });
    const started = await startWorkflowExecution(ingestResponse.workflow_execution?.id);
    return {
      file,
      document: registered.document,
      workflowExecution: ingestResponse.workflow_execution,
      started,
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

    try {
      let availableDatasets = datasets;
      const uploadResults = [];
      const createdDatasetTitles = [];
      const savedFiles = await saveFilesForLocalIngest(files);

      if (savedFiles.length !== files.length) {
        throw new Error('上传文件保存数量不一致，已停止登记。');
      }

      for (const [index, file] of files.entries()) {
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
        .map((result) => result.started?.enqueued_tasks?.[0]?.id)
        .filter(Boolean)
        .slice(0, 2)
        .join('、');

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
          workflowLabel ? `解析任务已入队：${workflowLabel}。` : '解析任务已提交。',
          publicWarning,
        ].filter(Boolean).join(' '),
      );
      await refreshCatalog({ preferredDatasetId: targetDataset?.id || selectedDatasetId, silent: true });
      if (activePage !== 'home' && (targetDataset?.id || selectedDatasetId)) {
        await refreshWorkspace(targetDataset?.id || selectedDatasetId, { silent: true, preserveNewSessionDraft: true });
      }
    } catch (uploadError) {
      setError(uploadError instanceof Error ? uploadError.message : '上传登记失败');
    } finally {
      setUploadingFiles(false);
    }
  }

  async function handleSubmitMessage() {
    const prompt = input.trim();

    if (!prompt) {
      return;
    }

    const nextScopePlan = planAssistantScope({
      prompt,
      datasets,
      selectedDatasetId,
      selectedDatasetIds,
      conversationMemory: visibleMessages,
      activeStaticPageDraft,
    });
    setScopePlan(nextScopePlan);
    const plannedDatasetIds = selectPlannerDatasetIds(nextScopePlan);
    const effectiveDatasetIds = normalizeDatasetIds([...selectedDatasetIds, ...plannedDatasetIds]);
    const effectiveDatasets = effectiveDatasetIds
      .map((datasetId) => datasets.find((dataset) => dataset.id === datasetId))
      .filter(Boolean);
    const effectiveDatasetId = effectiveDatasetIds[0] || '';
    const effectiveDataset = effectiveDatasets[0] || null;

    if (!sameDatasetIds(effectiveDatasetIds, selectedDatasetIds)) {
      setSelectedDatasetIds(effectiveDatasetIds);
      setSelectedDatasetId(effectiveDatasetId || null);
    }

    let pendingStaticPageDraft = null;
    const staticPageCreateRequested = promptRequestsStaticPage(prompt);
    const staticPageEditRequested = Boolean(activeStaticPageDraft && promptRequestsStaticPageEdit(prompt));
    const backendStaticPageEditRequested = Boolean(staticPageEditRequested && activeStaticPageDraft?.backendDraftId && lastAssistantRunId);
    const shouldUseAssistantRun = true;
    if (staticPageCreateRequested) {
      pendingStaticPageDraft = handleStartStaticPageDraft({
        oneClick: /一键|直接|马上|立即|跳过/.test(prompt),
        openEditor: false,
        prompt,
        datasetId: effectiveDatasetId,
        dataset: effectiveDataset,
        assistantRunId: '',
      });
    } else if (staticPageEditRequested && !backendStaticPageEditRequested) {
      pendingStaticPageDraft = handleApplyStaticPagePrompt(prompt);
    }

    if (shouldUseAssistantRun) {
      setSubmitting(true);
      try {
        if (!selectedSessionId && !draftSessionTitle.trim()) {
          setDraftSessionTitle(buildDefaultConversationTitle(prompt, draftSessionStartedAt));
        }
        const userMessage = createLocalMessage('user', prompt);
        const assistantSelectedScope = buildAssistantRunSelectedScope(effectiveDatasetIds, nextScopePlan);
        const briefing = buildAssistantStartupBriefing({
          datasets,
          reportPlans,
          publishedReports,
          latestMessages: [...localMessages, userMessage],
          activityEvents,
          selectedDataset: effectiveDataset,
          selectedDatasets: effectiveDatasets,
          activeStaticPageDraft: pendingStaticPageDraft || activeStaticPageDraft,
          staticPageDrafts: staticPageDraftItems,
        });
        let assistantContent = '';
        let usedBackendAssistantRun = false;
        let assistantRunId = '';
        let usedAssistantRunContinue = false;
        try {
          const assistantRun = await requestOrdinaryAssistantRun({
            prompt,
            userMessage,
            briefing,
            nextScopePlan,
            continueRunId: lastAssistantRunId,
            selectedScope: assistantSelectedScope,
          });
          assistantRunId = assistantRun.assistantRunId || '';
          assistantContent = assistantRun.assistantContent || '';
          usedAssistantRunContinue = assistantRun.continued;
          if (assistantRunId) {
            setLastAssistantRunId(assistantRunId);
            if (pendingStaticPageDraft && !pendingStaticPageDraft.backendDraftId) {
              syncStaticPageDraftCreate(pendingStaticPageDraft, { assistantRunId, prompt });
            }
          }
          const responsePayload = assistantRun.response || {};
          setAssistantRunProgress(buildAssistantRunProgress(responsePayload, usedAssistantRunContinue));
          const backendCandidates = Array.isArray(responsePayload?.scope_candidates)
            ? responsePayload.scope_candidates
            : [];
          if (backendCandidates.length) {
            setScopePlan({
              candidates: backendCandidates,
              hint: scopeHintFromCandidates(backendCandidates),
            });
          }
          const backendDatasetIds = datasetIdsFromScope(responsePayload?.selected_scope);
          if (backendDatasetIds.length) {
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
          if (backendStaticPageEditRequested) {
            pendingStaticPageDraft = handleApplyStaticPagePrompt(prompt);
          }
          const scopeDescription = effectiveDatasets.length
            ? `当前供料范围：${effectiveDatasets.map((dataset) => dataset.title || dataset.key || '当前数据集').join('、')}，本地兜底会继续优先使用这些范围。`
            : '当前没有锁定数据集，所以不会强行检索资料。';
          assistantContent = [
            `已进入普通聊天模式；${scopeDescription}`,
            formatStartupBriefingForModel(briefing),
            nextScopePlan.hint ? `供料判断：${nextScopePlan.hint}。你也可以在左侧取消或改选。` : '供料判断：暂未命中具体数据集。',
            `AssistantRun 暂不可用：${assistantRunError instanceof Error ? assistantRunError.message : '请求失败'}。`,
          ].join('\n\n');
        }

        const assistantMessage = createLocalMessage('assistant', assistantContent);
        if (selectedSessionId) {
          setSelectedSessionId(null);
        }
        setLocalMessages((current) => [
          ...(selectedSessionId ? visibleMessages : current),
          userMessage,
          assistantMessage,
        ].slice(-40));
        rememberLocalUserStatement(userMessage, assistantRunId);
        setInput('');
        setComposingNewSession(false);
        setBanner(
          usedBackendAssistantRun
            ? staticPageCreateRequested
              ? '已正常完成本轮对话，并准备好静态页草稿；如果需要编辑，点消息末尾的“进入静态页工作台”。'
              : backendStaticPageEditRequested && usedAssistantRunContinue
              ? '已让模型在当前静态页草稿上继续执行；记录只缓存在当前浏览器。'
              : usedAssistantRunContinue
                ? '已在同一个 AssistantRun 上继续执行；记录只缓存在当前浏览器。'
              : '已通过 AssistantRun 返回普通聊天；记录只缓存在当前浏览器。'
            : 'AssistantRun 暂不可用，已用本地占位回复保留这轮普通聊天。',
        );
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

  function handleStartNewConversation() {
    writeLocalThreadId(createLocalThreadId());
    setDraftSessionStartedAt(new Date().toISOString());
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
    setMobilePanel('chat');
  }

  function handleSelectConversation(sessionId) {
    if (!sessionId) {
      return;
    }
    if (sessionId === 'draft') {
      if (selectedSessionId) {
        handleStartNewConversation();
      }
      return;
    }
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
      assistantRunId = lastAssistantRunId,
    } = options;
    const draftDatasetId = datasetId || '';

    const baseDraft = buildInitialStaticPageDraft({
      datasetId: draftDatasetId,
      sessionId: selectedSessionId,
      conversationSummary: buildStaticPageConversationSummary(prompt, {
        dataset,
        datasets: selectedDatasets,
        messages: visibleMessages,
      }),
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
    setBanner(openEditor
      ? (oneClick ? '已按 AI 理解创建静态页草稿，并进入效果图排队。' : '已创建静态页草稿，下一步会展示页面规划。')
      : '已准备静态页草稿；当前对话不会中断，需要时点击“进入静态页工作台”。');
    setError('');
    setMobilePanel('chat');
    if (assistantRunId) {
      syncStaticPageDraftCreate(draft, { assistantRunId, prompt });
    }
    return draft;
  }

  function handleSelectStaticPageDraft(draftId) {
    const draft = staticPageDrafts[draftId];
    if (!draft) {
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
      summary: hasFinalStage ? '已退回效果图阶段继续修改。' : '已退回模板规划阶段继续修改。',
    });
    setBanner(hasFinalStage ? '已退回效果图阶段，可调整后重新制作静态页。' : '已退回模板规划阶段，可继续修改模板和模块。');
    setMobilePanel('chat');
    return nextDraft;
  }

  async function handleDeleteStaticPageDraft(draftId) {
    const draft = staticPageDrafts[draftId];
    if (!draft) {
      return;
    }
    if (typeof window !== 'undefined' && !window.confirm('删除这个生成项目？删除后右侧列表将不再展示。')) {
      return;
    }
    setStaticPageDrafts((current) => {
      const next = { ...current };
      delete next[draftId];
      return next;
    });
    if (activeStaticPageDraftId === draftId) {
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
        setBanner(`项目已先从本地列表移除；后端归档暂不可用：${deleteError instanceof Error ? deleteError.message : '请求失败'}。`);
        return;
      }
    }
    setBanner('生成项目已删除。');
  }

  function handleSelectHtmlArtifact(artifactId) {
    setActiveHtmlArtifactId(artifactId);
    setStaticPageEditorOpen(false);
    setBanner('已打开安全 HTML 产物；内容在沙箱中展示，不会执行任意外部脚本。');
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
      setBanner(`HTML 产物动作已提交并同步到 V3：${eventData?.type || 'unknown'}。`);
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

    if (operation.type === 'request_final_render' && !canRequestStaticPageFinalRender(activeStaticPageDraft)) {
      setBanner(staticPageFinalRenderBlockReason(activeStaticPageDraft) || '需要先确认当前效果图，再制作最终静态页。');
      return activeStaticPageDraft;
    }

    if (operation.type === 'request_final_render' && activeStaticPageDraft.backendDraftId) {
      const optimisticDraft = replaceDraftWithOperation(activeStaticPageDraft, {
        ...operation,
        finalPage: operation.finalPage || {
          status: 'queued',
          renderer: 'platform-api-static-page-renderer',
          renderOutputId: '',
          imageJobId: activeStaticPageDraft.imageJob?.id || null,
          assetManifest: {
            status: 'queued',
            queue_copy: '最终静态页正在后台制作，可以继续聊天或修改其他内容。',
          },
        },
      });
      createBackendStaticPageRender(optimisticDraft, operation).catch((syncError) => {
        setBanner(`静态页已先进入本地后台状态；后端渲染暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
      });
      return optimisticDraft;
    }

    const draft = replaceDraftWithOperation(activeStaticPageDraft, operation);

    if (operation.type === 'queue_image_job') {
      if (activeStaticPageDraft.backendDraftId) {
        createBackendStaticPageImageJob(activeStaticPageDraft, operation).catch((syncError) => {
          setBanner(`效果图已先进入本地排队；后端队列暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
        });
      }
      return draft;
    }

    if (operation.type === 'confirm_preview') {
      if (draft.backendDraftId) {
        confirmBackendStaticPagePreview(draft, operation).catch((syncError) => {
          setBanner(`效果图已先在本地确认；后端确认暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
        });
      }
      return draft;
    }

    syncStaticPageDraftOperations(activeStaticPageDraft, draft, [operation]);
    return draft;
  }

  async function handleStaticPagePrimaryAction() {
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
      setBanner('静态页已经生成；如果要重做，先回到模块编辑修改内容，再重新发起效果图。');
      return draft;
    }

    if (['queued', 'running'].includes(jobStatus)) {
      setStaticPageEditorOpen(false);
      setBanner('效果图正在生成中；资源返回后会停在主聊天区等待确认。');
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
        if (draft.previewContract?.status === 'confirmed' && !canRequestStaticPageFinalRender(draft)) {
          setBanner(staticPageFinalRenderBlockReason(draft) || '效果图确认状态缺少资源，请重新发起效果图。');
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
          : replaceDraftWithOperation(draft, {
              type: 'confirm_preview',
              previewImage: draft.previewImage || buildMockStaticPagePreview(draft),
            });
        const renderedDraft = replaceDraftWithOperation(confirmedDraft, { type: 'request_final_render' });
        setStaticPageEditorOpen(false);
        setMobilePanel('chat');
        setBanner('已按确认效果图生成本地静态页模拟结果；接入后端时会进入正式后台渲染。');
        return renderedDraft;
      }

      const queueOperation = {
        type: 'queue_image_job',
        queueMessage: STATIC_PAGE_QUEUE_MESSAGE,
      };

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
      setBanner('当前草稿尚未同步到后端，已生成本地模拟效果图；正式运行会进入 Cloudflare/Codex 生图队列。');
      return previewDraft;
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : '静态页生成动作失败');
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
    setActiveSecretCount(readLocalSecretBindingIds().length);
    setLastAssistantRunId(readLocalAssistantRunId());
  }, []);

  useEffect(() => {
    writeLocalAssistantRunId(lastAssistantRunId);
  }, [lastAssistantRunId]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    try {
      const raw = window.localStorage.getItem(LOCAL_CHAT_STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : [];
      if (Array.isArray(parsed)) {
        setLocalMessages(parsed.slice(-40));
      }
    } catch {
      setLocalMessages([]);
    }
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    try {
      window.localStorage.setItem(LOCAL_CHAT_STORAGE_KEY, JSON.stringify(localMessages.slice(-40)));
    } catch {
      // Ignore cache write failures; chat can still continue in memory.
    }
  }, [localMessages]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    try {
      const raw = window.localStorage.getItem(LOCAL_ACTIVITY_STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : [];
      if (Array.isArray(parsed)) {
        setActivityEvents(parsed.slice(0, 20));
      }
    } catch {
      setActivityEvents([]);
    }
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    try {
      window.localStorage.setItem(LOCAL_ACTIVITY_STORAGE_KEY, JSON.stringify(activityEvents.slice(0, 20)));
    } catch {
      // Activity cache is only a local briefing hint; ignore write failures.
    }
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
    if (!['datasets', 'sources'].includes(activePage)) {
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
    setBanner('效果图已生成，已回到主聊天区。满意就点“效果图——生成页面”；不满意就回到模块编辑。');
  }, [activeStaticPageDraft?.id, activeStaticPageDraft?.imageJob?.status, staticPageEditorOpen]);

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
    if (!selectedReportPlanId) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      refreshReportDetail(selectedReportPlanId, { silent: true });
    }, REPORT_DETAIL_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [selectedReportPlanId]);

  function handleToggleDatasetSelection(datasetId) {
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
      setBanner('已清空供料范围；后续对话会先按普通聊天处理，命中资料意图时再预选。');
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
    onSelectReportPlan: setSelectedReportPlanId,
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
    staticPageDraft: activeStaticPageDraft,
    staticPageDrafts: staticPageDraftItems,
    onSelectStaticPageDraft: handleSelectStaticPageDraft,
    onDeleteStaticPageDraft: handleDeleteStaticPageDraft,
    onRevertStaticPageStage: handleRevertStaticPageStage,
    onRefreshStaticPageDrafts: () => refreshStaticPageDraftShelf({ silent: false }),
    staticPageEditorOpen,
    assistantRunProgress,
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
    onSelectDocument: setSelectedDocumentId,
    selectedDocumentDetail,
    documentDetailLoading,
    onRefreshDocuments: () => refreshDocuments({ silent: false }),
    onUpdateDataset: handleUpdateDataset,
    onArchiveDataset: handleArchiveDataset,
    datasetActionBusy,
    onUpdateDocument: handleUpdateDocument,
    onArchiveDocuments: handleArchiveDocuments,
    documentActionBusy,
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
          sessions={sessions}
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
          sessions={sessions}
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

        {banner ? <div className="page-banner success-banner">{banner}</div> : null}
        {error ? <div className="page-banner error-banner">{error}</div> : null}

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
