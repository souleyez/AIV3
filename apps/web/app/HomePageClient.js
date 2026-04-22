'use client';

import { startTransition, useEffect, useMemo, useRef, useState } from 'react';
import ChatPanel from './components/ChatPanel';
import InsightPanel from './components/InsightPanel';
import Sidebar from './components/Sidebar';

const DATASET_POLL_INTERVAL_MS = 5000;
const MESSAGE_POLL_INTERVAL_MS = 3000;
const CATALOG_POLL_INTERVAL_MS = 12000;

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
  const [sessions, setSessions] = useState([]);
  const [outputs, setOutputs] = useState([]);
  const [selectedSessionId, setSelectedSessionId] = useState(null);
  const [messages, setMessages] = useState([]);
  const [input, setInput] = useState('');
  const [datasetDraft, setDatasetDraft] = useState({ key: '', title: '' });
  const [bootstrapping, setBootstrapping] = useState(true);
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [messageLoading, setMessageLoading] = useState(false);
  const [creatingDataset, setCreatingDataset] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [reportEntryBusy, setReportEntryBusy] = useState(false);
  const [banner, setBanner] = useState('');
  const [error, setError] = useState('');

  const datasetLoadIdRef = useRef(0);
  const messageLoadIdRef = useRef(0);

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
    const { preferredSessionId = null, silent = false } = options;
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

  async function handleCreateSession() {
    const prompt = input.trim();

    if (!selectedDatasetId || !prompt) {
      return;
    }

    setSubmitting(true);
    try {
      const response = await fetchJson(`/api/v3/datasets/${selectedDatasetId}/chat-sessions`, {
        method: 'POST',
        body: { prompt },
      });

      setInput('');
      setBanner(`已启动新会话 ${response.chat_session.title}。`);
      await refreshWorkspace(selectedDatasetId, {
        preferredSessionId: response.chat_session.id,
        silent: true,
      });
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : '发起会话失败');
    } finally {
      setSubmitting(false);
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
        setBanner(`已进入报告服务，生成 report_plan ${response.report_plan.id}。`);
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

  useEffect(() => {
    refreshCatalog();
  }, []);

  useEffect(() => {
    if (!selectedDatasetId) {
      startTransition(() => {
        setSessions([]);
        setOutputs([]);
        setSelectedSessionId(null);
        setMessages([]);
      });
      return;
    }

    startTransition(() => {
      setSessions([]);
      setOutputs([]);
      setSelectedSessionId(null);
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
    if (!selectedDatasetId) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      refreshWorkspace(selectedDatasetId, {
        preferredSessionId: selectedSessionId,
        silent: true,
      });
    }, DATASET_POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [selectedDatasetId, selectedSessionId]);

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
          setSelectedDatasetId(datasetId);
        }}
        creatingDataset={creatingDataset}
        stats={stats}
        loading={bootstrapping || workspaceLoading}
      />

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

        {banner ? <div className="page-banner success-banner">{banner}</div> : null}
        {error ? <div className="page-banner error-banner">{error}</div> : null}

        <section className="workspace-grid homepage-workspace">
          <ChatPanel
            dataset={selectedDataset}
            session={selectedSession}
            messages={messages}
            messageLoading={messageLoading}
            input={input}
            onInputChange={setInput}
            onSubmit={handleCreateSession}
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
            onSelectSession={setSelectedSessionId}
          />
        </section>
      </main>
    </div>
  );
}
