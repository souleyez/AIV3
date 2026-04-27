'use client';

import { startTransition, useEffect, useMemo, useRef, useState } from 'react';
import ChatPanel from './components/ChatPanel';
import HomeMobileShell from './components/HomeMobileShell';
import HomeWorkspaceToolbar from './components/HomeWorkspaceToolbar';
import InsightPanel from './components/InsightPanel';
import Sidebar from './components/Sidebar';
import { buildAssistantStartupBriefing, formatStartupBriefingForModel } from './lib/assistant-startup-briefing';
import { planAssistantScope, selectPlannerDatasetId } from './lib/scope-planner';
import {
  applyStaticPageOperation,
  applyStaticPageOperations,
  buildInitialStaticPageDraft,
  buildMockStaticPagePreview,
  buildStaticPageImagePayload,
  interpretStaticPagePrompt,
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
const LOCAL_CHAT_STORAGE_KEY = 'aidp-v3-local-chat-messages';
const LOCAL_ACTIVITY_STORAGE_KEY = 'aidp-v3-local-activity-events';
const LOCAL_THREAD_ID_STORAGE_KEY = 'aidp-v3-local-thread-id';
const LOCAL_ASSISTANT_RUN_ID_STORAGE_KEY = 'aidp-v3-local-assistant-run-id';
const LOCAL_SECRET_BINDING_IDS_STORAGE_KEY = 'aidp-v3-secret-binding-ids';
const LOCAL_SECRET_VALUE_STORAGE_KEY = 'aidp-v3-local-secret-value';
const STATIC_PAGE_QUEUE_MESSAGE = '资源正在排队，可以联系商务开通高级用户跳过等待。';

function readLocalThreadId() {
  if (typeof window === 'undefined') {
    return 'server-render-thread';
  }
  try {
    const existing = window.localStorage.getItem(LOCAL_THREAD_ID_STORAGE_KEY);
    if (existing) {
      return existing;
    }
    const next = globalThis.crypto?.randomUUID?.() || `thread-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    window.localStorage.setItem(LOCAL_THREAD_ID_STORAGE_KEY, next);
    return next;
  } catch {
    return 'browser-thread-unavailable';
  }
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
    ...options,
    headers: {
      Accept: 'application/json',
      ...(options.body && !isFormData ? { 'Content-Type': 'application/json' } : {}),
      ...(secretBindingIds ? { 'X-AI-Data-Platform-Secret-Binding-Ids': secretBindingIds } : {}),
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

function firstDatasetIdFromScope(scope) {
  const datasets = Array.isArray(scope?.datasets)
    ? scope.datasets
    : Array.isArray(scope?.selected)
      ? scope.selected
      : [];
  return datasets.find(Boolean) || '';
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
  const [datasets, setDatasets] = useState([]);
  const [reportPlans, setReportPlans] = useState([]);
  const [publishedReports, setPublishedReports] = useState([]);
  const [selectedDatasetId, setSelectedDatasetId] = useState(null);
  const [selectedReportPlanId, setSelectedReportPlanId] = useState(null);
  const [sessions, setSessions] = useState([]);
  const [outputs, setOutputs] = useState([]);
  const [reportRenderOutputs, setReportRenderOutputs] = useState([]);
  const [reportAstVersions, setReportAstVersions] = useState([]);
  const [publishedReportDetail, setPublishedReportDetail] = useState(null);
  const [selectedSessionId, setSelectedSessionId] = useState(null);
  const [composingNewSession, setComposingNewSession] = useState(false);
  const [messages, setMessages] = useState([]);
  const [localMessages, setLocalMessages] = useState([]);
  const [input, setInput] = useState('');
  const [datasetDraft, setDatasetDraft] = useState({ key: '', title: '', secret: '' });
  const [localSecretDraft, setLocalSecretDraft] = useState('');
  const [activeSecretCount, setActiveSecretCount] = useState(0);
  const [reportSurface, setReportSurface] = useState('pc');
  const [publishNote, setPublishNote] = useState('');
  const [mobileViewport, setMobileViewport] = useState(false);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const [mobilePanel, setMobilePanel] = useState('chat');
  const [bootstrapping, setBootstrapping] = useState(true);
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [messageLoading, setMessageLoading] = useState(false);
  const [reportDetailLoading, setReportDetailLoading] = useState(false);
  const [creatingDataset, setCreatingDataset] = useState(false);
  const [resolvingSecret, setResolvingSecret] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [uploadingFiles, setUploadingFiles] = useState(false);
  const [reportEntryBusy, setReportEntryBusy] = useState(false);
  const [reportActionBusy, setReportActionBusy] = useState('');
  const [banner, setBanner] = useState('');
  const [error, setError] = useState('');
  const [staticPageDrafts, setStaticPageDrafts] = useState({});
  const [activeStaticPageDraftId, setActiveStaticPageDraftId] = useState(null);
  const [scopePlan, setScopePlan] = useState({ candidates: [], hint: '' });
  const [activityEvents, setActivityEvents] = useState([]);
  const [lastAssistantRunId, setLastAssistantRunId] = useState('');

  const datasetLoadIdRef = useRef(0);
  const messageLoadIdRef = useRef(0);
  const reportDetailLoadIdRef = useRef(0);
  const fileInputRef = useRef(null);

  const selectedDataset = useMemo(
    () => datasets.find((dataset) => dataset.id === selectedDatasetId) || null,
    [datasets, selectedDatasetId],
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
  const visibleMessages = useMemo(
    () => (selectedDatasetId || selectedSessionId ? messages : localMessages),
    [localMessages, messages, selectedDatasetId, selectedSessionId],
  );
  const assistantStartupBriefing = useMemo(
    () => buildAssistantStartupBriefing({
      datasets,
      reportPlans,
      publishedReports,
      latestMessages: visibleMessages,
      activityEvents,
      selectedDataset,
    }),
    [activityEvents, datasets, publishedReports, reportPlans, selectedDataset, visibleMessages],
  );
  const toolbarSourceItems = useMemo(
    () => (selectedDataset ? [{ name: selectedDataset.title, status: 'healthy' }] : []),
    [selectedDataset],
  );

  function promptRequestsStaticPage(prompt) {
    return /静态页|静态页面|页面规划|一页|生成页面|落地页/.test(String(prompt || ''));
  }

  function buildStaticPageConversationSummary(prompt = '', options = {}) {
    const draftDataset = options.dataset || selectedDataset;
    const draftSession = options.session || selectedSession;
    const sourceMessages = options.messages || visibleMessages;
    const latestAssistantMessage = [...sourceMessages].reverse().find((message) => message.role === 'assistant');
    const latestMessage = latestAssistantMessage || sourceMessages[sourceMessages.length - 1];
    const summaryParts = [
      draftDataset ? `数据集：${draftDataset.title}` : '未选数据集，按普通对话意图规划。',
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
      },
    });
    const renderOutput = response?.render_output;
    const rendered = applyStaticPageOperation(draft, {
      type: 'request_final_render',
      finalPage: {
        status: 'rendered',
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
      status: 'rendered',
      finalPage: rendered.finalPage,
    };
    replaceStaticPageDraft(baseDraft.id, finalDraft);
    setBanner('最终静态页已按确认效果图生成。');
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

  async function refreshCatalog(options = {}) {
    const { preferredDatasetId = null, silent = false } = options;
    if (!silent) {
      setBootstrapping(true);
    }

    try {
      const [datasetItems, planItems, reportItems] = await Promise.all([
        fetchJson('/api/v3/datasets'),
        fetchJson('/api/v3/report-plans'),
        fetchJson('/api/v3/published-reports'),
      ]);

      const nextDatasets = sortDatasets(datasetItems);
      const nextReportPlans = Array.isArray(planItems) ? planItems : [];
      const nextPublishedReports = sortByDateDesc(reportItems, 'updated_at');

      startTransition(() => {
        setDatasets(nextDatasets);
        setReportPlans(nextReportPlans);
        setPublishedReports(nextPublishedReports);
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
      const [renderOutputItems, astVersionItems, publishedDetail] = await Promise.all([
        fetchJson(`/api/v3/report-plans/${planId}/render-outputs`),
        fetchJson(`/api/v3/report-plans/${planId}/ast-versions`),
        fetchPlanPublishedReport(planId),
      ]);

      if (reportDetailLoadIdRef.current !== loadId) {
        return;
      }

      startTransition(() => {
        setReportRenderOutputs(sortByDateDesc(renderOutputItems, 'created_at'));
        setReportAstVersions(sortByDateDesc(astVersionItems, 'created_at'));
        setPublishedReportDetail(publishedDetail);
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
    const key = datasetDraft.key.trim();
    const title = datasetDraft.title.trim();
    const secret = String(datasetDraft.secret || '').trim();

    if (!key || !title) {
      setError('新建数据集至少需要 key 和标题。');
      return;
    }

    setCreatingDataset(true);
    try {
      const fingerprint = secret ? await fingerprintLocalSecret(secret) : '';
      const dataset = await fetchJson('/api/v3/datasets', {
        method: 'POST',
        body: {
          key,
          title,
          ...(fingerprint ? { secret_fingerprint: fingerprint, secret_label: `local-${key}` } : {}),
        },
      });
      let nextDataset = dataset;
      let secretNote = '';
      if (secret && dataset.secret_binding_ids?.length) {
        const nextBindingIds = [...readLocalSecretBindingIds(), ...dataset.secret_binding_ids];
        writeLocalSecretState(secret, nextBindingIds);
        setActiveSecretCount([...new Set(nextBindingIds)].length);
        secretNote = ' 已绑定本地密钥，后续请求会优先带当前密钥。';
      }
      setDatasetDraft({ key: '', title: '', secret: '' });
      setBanner(
        `已创建数据集 ${nextDataset.title}。${secretNote || (nextDataset.access_warning ? ` ${nextDataset.access_warning}。` : '')}`,
      );
      await refreshCatalog({ preferredDatasetId: nextDataset.id, silent: true });
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
  }) {
    const messages = localMessages
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
        scope_candidates: nextScopePlan.candidates,
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
          selectedDatasetId,
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
      const targetDataset = selectedDataset?.id
        ? selectedDataset
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
      if (targetDataset?.id) {
        setSelectedDatasetId(targetDataset.id);
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
      if (targetDataset?.id || selectedDatasetId) {
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
      conversationMemory: visibleMessages,
    });
    setScopePlan(nextScopePlan);
    const plannedDatasetId = selectedDatasetId || selectPlannerDatasetId(nextScopePlan);
    const effectiveDatasetId = plannedDatasetId || '';
    const effectiveDataset = datasets.find((dataset) => dataset.id === effectiveDatasetId) || null;

    if (effectiveDatasetId && effectiveDatasetId !== selectedDatasetId) {
      setSelectedDatasetId(effectiveDatasetId);
    }

    let pendingStaticPageDraft = null;
    if (promptRequestsStaticPage(prompt)) {
      pendingStaticPageDraft = handleStartStaticPageDraft({
        oneClick: /一键|直接|马上|立即|跳过/.test(prompt),
        prompt,
        datasetId: effectiveDatasetId,
        dataset: effectiveDataset,
      });
    } else if (activeStaticPageDraft && /调整|修改|换成|改成|突出|减少|增加|放大|缩小|移动|排序|风格|老板|高层|风险|柱状图|折线图|环图|看板|精简/.test(prompt)) {
      pendingStaticPageDraft = handleApplyStaticPagePrompt(prompt);
    }

    if (!effectiveDatasetId) {
      setSubmitting(true);
      try {
        const userMessage = createLocalMessage('user', prompt);
        const briefing = buildAssistantStartupBriefing({
          datasets,
          reportPlans,
          publishedReports,
          latestMessages: [...localMessages, userMessage],
          activityEvents,
          selectedDataset: null,
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
          const backendCandidates = Array.isArray(responsePayload?.scope_candidates)
            ? responsePayload.scope_candidates
            : [];
          if (backendCandidates.length) {
            setScopePlan({
              candidates: backendCandidates,
              hint: scopeHintFromCandidates(backendCandidates),
            });
          }
          const backendDatasetId = firstDatasetIdFromScope(responsePayload?.selected_scope);
          if (backendDatasetId) {
            await refreshCatalog({ preferredDatasetId: backendDatasetId, silent: true });
          }
          usedBackendAssistantRun = Boolean(assistantContent);
        } catch (assistantRunError) {
          assistantContent = [
            '已进入普通聊天模式；当前没有锁定数据集，所以不会强行检索资料。',
            formatStartupBriefingForModel(briefing),
            nextScopePlan.hint ? `供料判断：${nextScopePlan.hint}。你也可以在左侧取消或改选。` : '供料判断：暂未命中具体数据集。',
            `AssistantRun 暂不可用：${assistantRunError instanceof Error ? assistantRunError.message : '请求失败'}。`,
          ].join('\n\n');
        }

        const assistantMessage = createLocalMessage('assistant', assistantContent);
        setLocalMessages((current) => [...current, userMessage, assistantMessage].slice(-40));
        rememberLocalUserStatement(userMessage, assistantRunId);
        setInput('');
        setComposingNewSession(false);
        setBanner(
          usedBackendAssistantRun
            ? usedAssistantRunContinue
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

    setSubmitting(true);
    try {
      const response = selectedSessionId && selectedDatasetId
        ? await fetchJson(`/api/v3/chat-sessions/${selectedSessionId}/turns`, {
            method: 'POST',
            body: { prompt },
          })
        : await fetchJson(`/api/v3/datasets/${effectiveDatasetId}/chat-sessions`, {
            method: 'POST',
            body: { prompt },
          });
      const started = await startWorkflowExecution(response.workflow_execution?.id);
      const sessionTitle = response.chat_session?.title || '当前会话';

      setInput('');
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
    setBanner(
      selectedDatasetId
        ? '已切换为新会话输入；下一次发送会创建独立 chat_session。'
        : '已开始新的普通聊天；本地缓存只保留当前浏览器的轻量记录。',
    );
    setError('');
    setComposingNewSession(true);
    setSelectedSessionId(null);
    setMessages([]);
    setLastAssistantRunId('');
    if (!selectedDatasetId) {
      setLocalMessages([]);
    }
    setMobilePanel('chat');
  }

  function handleStartStaticPageDraft(options = {}) {
    const {
      oneClick = false,
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
    setBanner(oneClick ? '已按 AI 理解创建静态页草稿，并进入效果图排队。' : '已创建静态页草稿，下一步会展示页面规划。');
    setError('');
    setMobilePanel('insights');
    if (assistantRunId) {
      syncStaticPageDraftCreate(draft, { assistantRunId, prompt });
    }
    return draft;
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

    if (operation.type === 'request_final_render') {
      if (draft.backendDraftId) {
        createBackendStaticPageRender(draft, operation).catch((syncError) => {
          setBanner(`静态页已先在本地生成模拟版；后端渲染暂不可用：${syncError instanceof Error ? syncError.message : '请求失败'}。`);
        });
      }
      return draft;
    }

    syncStaticPageDraftOperations(activeStaticPageDraft, draft, [operation]);
    return draft;
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
      ]);
    } catch (retryError) {
      setError(retryError instanceof Error ? retryError.message : '重试 workflow 失败');
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
  }, [selectedDatasetId]);

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
    if (!selectedDatasetId) {
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
  }, [selectedDatasetId, selectedSessionId, composingNewSession]);

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
    if (!selectedReportPlanId) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      refreshReportDetail(selectedReportPlanId, { silent: true });
    }, REPORT_DETAIL_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [selectedReportPlanId]);

  const stats = {
    sessions: sessions.length,
    outputs: outputs.length,
    plans: datasetReportPlans.length,
    published: datasetPublishedReports.length,
  };
  const sidebarProps = {
    datasets,
    selectedDatasetId,
    selectedDataset,
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
    onSelectDataset: (datasetId) => {
      setBanner('');
      setError('');
      setComposingNewSession(false);
      setSelectedDatasetId(datasetId);
      setMobileSidebarOpen(false);
      setMobilePanel('chat');
    },
    onClearDatasetSelection: () => {
      setBanner('已切回普通聊天；没有选中数据集时不会强行检索。');
      setError('');
      setComposingNewSession(false);
      setSelectedDatasetId(null);
      setSelectedSessionId(null);
      setScopePlan({ candidates: [], hint: '' });
      setMobileSidebarOpen(false);
      setMobilePanel('chat');
    },
    creatingDataset,
    stats,
    loading: bootstrapping || workspaceLoading,
    mobileOpen: mobileSidebarOpen,
    onClose: () => setMobileSidebarOpen(false),
    scopePlan,
  };
  const chatPanelProps = {
    dataset: selectedDataset,
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
    onOpenStaticPageBuilder: () => setMobilePanel('insights'),
    startupBriefing: assistantStartupBriefing,
    scopePlan,
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
    onSelectSession: (sessionId) => {
      setComposingNewSession(false);
      setSelectedSessionId(sessionId);
      setMobilePanel('chat');
    },
    onSelectReportPlan: setSelectedReportPlanId,
    onReportSurfaceChange: setReportSurface,
    onPublishNoteChange: setPublishNote,
    onContinueReportPlan: handleContinueReportPlan,
    onRequestReportRender: handleRequestReportRender,
    onPublishReport: handlePublishReport,
    onRetryWorkflowExecution: handleRetryWorkflowExecution,
    onRefreshReportDetail: () => {
      if (selectedReportPlanId) {
        refreshReportDetail(selectedReportPlanId);
      }
    },
    staticPageDraft: activeStaticPageDraft,
    onStartStaticPageDraft: handleStartStaticPageDraft,
    onApplyStaticPageOperation: handleApplyStaticPageOperation,
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
          selectedDataset={selectedDataset}
          stats={stats}
          loading={bootstrapping || workspaceLoading}
          banner={banner}
          error={error}
          staticPageDraft={activeStaticPageDraft}
          onApplyStaticPageOperation={handleApplyStaticPageOperation}
          onApplyStaticPagePrompt={handleApplyStaticPagePrompt}
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
          selectedDataset={selectedDataset}
          stats={stats}
          loading={bootstrapping}
          workspaceLoading={workspaceLoading}
          sourceItems={toolbarSourceItems}
        />

        {banner ? <div className="page-banner success-banner">{banner}</div> : null}
        {error ? <div className="page-banner error-banner">{error}</div> : null}

        <section className={`workspace-grid homepage-workspace mobile-panel-${mobilePanel}`}>
          <ChatPanel {...chatPanelProps} />
          <InsightPanel {...insightPanelProps} />
        </section>
      </main>
    </div>
  );
}
