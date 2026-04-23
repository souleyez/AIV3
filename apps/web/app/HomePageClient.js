'use client';

import { startTransition, useEffect, useMemo, useRef, useState } from 'react';
import ChatPanel from './components/ChatPanel';
import InsightPanel from './components/InsightPanel';
import Sidebar from './components/Sidebar';

const DATASET_POLL_INTERVAL_MS = 5000;
const MESSAGE_POLL_INTERVAL_MS = 3000;
const CATALOG_POLL_INTERVAL_MS = 12000;
const REPORT_DETAIL_POLL_INTERVAL_MS = 6000;

async function fetchJson(url, options = {}) {
  const response = await fetch(url, {
    cache: 'no-store',
    ...options,
    headers: {
      Accept: 'application/json',
      ...(options.body ? { 'Content-Type': 'application/json' } : {}),
      ...(options.headers || {}),
    },
    body: options.body && typeof options.body !== 'string' ? JSON.stringify(options.body) : options.body,
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
  const [input, setInput] = useState('');
  const [datasetDraft, setDatasetDraft] = useState({ key: '', title: '' });
  const [reportSurface, setReportSurface] = useState('pc');
  const [publishNote, setPublishNote] = useState('');
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const [mobilePanel, setMobilePanel] = useState('chat');
  const [bootstrapping, setBootstrapping] = useState(true);
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [messageLoading, setMessageLoading] = useState(false);
  const [reportDetailLoading, setReportDetailLoading] = useState(false);
  const [creatingDataset, setCreatingDataset] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [reportEntryBusy, setReportEntryBusy] = useState(false);
  const [reportActionBusy, setReportActionBusy] = useState('');
  const [banner, setBanner] = useState('');
  const [error, setError] = useState('');

  const datasetLoadIdRef = useRef(0);
  const messageLoadIdRef = useRef(0);
  const reportDetailLoadIdRef = useRef(0);

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
          return nextDatasets[0]?.id || null;
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

    if (!key || !title) {
      setError('新建数据集至少需要 key 和标题。');
      return;
    }

    setCreatingDataset(true);
    try {
      const dataset = await fetchJson('/api/v3/datasets', {
        method: 'POST',
        body: { key, title },
      });
      setDatasetDraft({ key: '', title: '' });
      setBanner(`已创建数据集 ${dataset.title}。`);
      await refreshCatalog({ preferredDatasetId: dataset.id, silent: true });
    } catch (createError) {
      setError(createError instanceof Error ? createError.message : '创建数据集失败');
    } finally {
      setCreatingDataset(false);
    }
  }

  async function handleSubmitMessage() {
    const prompt = input.trim();

    if (!selectedDatasetId || !prompt) {
      return;
    }

    setSubmitting(true);
    try {
      const response = selectedSessionId
        ? await fetchJson(`/api/v3/chat-sessions/${selectedSessionId}/turns`, {
            method: 'POST',
            body: { prompt },
          })
        : await fetchJson(`/api/v3/datasets/${selectedDatasetId}/chat-sessions`, {
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
        refreshWorkspace(selectedDatasetId, {
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
    if (!selectedDatasetId) {
      return;
    }

    setBanner('已切换为新会话输入；下一次发送会创建独立 chat_session。');
    setError('');
    setComposingNewSession(true);
    setSelectedSessionId(null);
    setMessages([]);
    setMobilePanel('chat');
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

  useEffect(() => {
    refreshCatalog();
  }, []);

  useEffect(() => {
    if (!selectedDatasetId) {
      startTransition(() => {
        setSessions([]);
        setOutputs([]);
        setSelectedReportPlanId(null);
        setSelectedSessionId(null);
        setComposingNewSession(false);
        setMessages([]);
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

  return (
    <div className="app-shell">
      <Sidebar
        datasets={datasets}
        selectedDatasetId={selectedDatasetId}
        selectedDataset={selectedDataset}
        datasetDraft={datasetDraft}
        onDatasetDraftChange={(field, value) =>
          setDatasetDraft((current) => ({ ...current, [field]: value }))
        }
        onCreateDataset={handleCreateDataset}
        onSelectDataset={(datasetId) => {
          setBanner('');
          setError('');
          setComposingNewSession(false);
          setSelectedDatasetId(datasetId);
          setMobileSidebarOpen(false);
          setMobilePanel('chat');
        }}
        creatingDataset={creatingDataset}
        stats={stats}
        loading={bootstrapping || workspaceLoading}
        mobileOpen={mobileSidebarOpen}
        onClose={() => setMobileSidebarOpen(false)}
      />
      {mobileSidebarOpen ? (
        <button
          type="button"
          className="mobile-drawer-backdrop"
          aria-label="关闭数据集侧栏"
          onClick={() => setMobileSidebarOpen(false)}
        />
      ) : null}

      <main className="main-panel main-panel-home">
        <header className="topbar">
          <div className="topbar-title-row">
            <h2>智能助手</h2>
            <span className="topbar-inline-note">
              沿用老版工作台壳子，但控制器改成直接消费 V3 host surface。左侧数据集切换已接通。
            </span>
          </div>
          <div className="topbar-actions">
            <div className="topbar-summary-card">
              <span>Dataset</span>
              <strong>{selectedDataset ? `${selectedDataset.title} (${selectedDataset.key})` : '未选择'}</strong>
            </div>
            <div className="topbar-summary-card">
              <span>Status</span>
              <strong>{workspaceLoading ? '加载中' : '就绪'}</strong>
            </div>
          </div>
        </header>

        <div className="mobile-shell-controls" aria-label="移动端工作区切换">
          <button
            type="button"
            className="mobile-shell-button wide"
            onClick={() => setMobileSidebarOpen(true)}
          >
            <span>数据集</span>
            <strong>{selectedDataset ? selectedDataset.title : '选择数据集'}</strong>
          </button>
          <button
            type="button"
            className={`mobile-shell-button ${mobilePanel === 'chat' ? 'active' : ''}`}
            onClick={() => setMobilePanel('chat')}
          >
            对话
          </button>
          <button
            type="button"
            className={`mobile-shell-button ${mobilePanel === 'insights' ? 'active' : ''}`}
            onClick={() => setMobilePanel('insights')}
          >
            上下文 / 报告
          </button>
        </div>

        {banner ? <div className="page-banner success-banner">{banner}</div> : null}
        {error ? <div className="page-banner error-banner">{error}</div> : null}

        <section className={`workspace-grid homepage-workspace mobile-panel-${mobilePanel}`}>
          <ChatPanel
            dataset={selectedDataset}
            session={selectedSession}
            messages={messages}
            messageLoading={messageLoading}
            input={input}
            onInputChange={setInput}
            onSubmit={handleSubmitMessage}
            onStartNewConversation={handleStartNewConversation}
            submitting={submitting}
            reportEntryBusy={reportEntryBusy}
            onResolveReportEntry={handleResolveReportEntry}
          />
          <InsightPanel
            dataset={selectedDataset}
            sessions={sessions}
            selectedSessionId={selectedSessionId}
            outputs={outputs}
            reportPlans={datasetReportPlans}
            publishedReports={datasetPublishedReports}
            selectedReportPlanId={selectedReportPlanId}
            selectedReportPlan={selectedReportPlan}
            reportRenderOutputs={reportRenderOutputs}
            reportAstVersions={reportAstVersions}
            publishedReportDetail={publishedReportDetail}
            reportDetailLoading={reportDetailLoading}
            reportActionBusy={reportActionBusy}
            reportSurface={reportSurface}
            publishNote={publishNote}
            onSelectSession={(sessionId) => {
              setComposingNewSession(false);
              setSelectedSessionId(sessionId);
              setMobilePanel('chat');
            }}
            onSelectReportPlan={setSelectedReportPlanId}
            onReportSurfaceChange={setReportSurface}
            onPublishNoteChange={setPublishNote}
            onContinueReportPlan={handleContinueReportPlan}
            onRequestReportRender={handleRequestReportRender}
            onPublishReport={handlePublishReport}
            onRefreshReportDetail={() => {
              if (selectedReportPlanId) {
                refreshReportDetail(selectedReportPlanId);
              }
            }}
          />
        </section>
      </main>
    </div>
  );
}
