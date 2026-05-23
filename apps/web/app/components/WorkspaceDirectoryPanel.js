'use client';

import { useEffect, useState } from 'react';
import {
  fetchDatabaseSourceOptions,
  fetchDatabaseSourceStatus,
  inspectDatabaseSourceSchema,
  profileDatabaseSource,
  startDatabaseSourceSync,
  testDatabaseSourceConnection,
} from '../lib/database-source';
import { buildDocumentDetailViewModel, chunkSectionHints } from '../lib/document-detail-view';
import { formatDateTime, formatRelativeTime, formatSnakeCaseLabel, truncateText } from '../lib/formatters';
import ModelPoolPanel from './ModelPoolPanel';

const PAGE_COPY = {
  datasets: {
    title: '数据集',
    subtitle: '数据集列表、文档列表和基础批量管理；点击文档名称进入原文与解析详情。',
  },
  'document-detail': {
    title: '文档详情',
    subtitle: '查看文档原文、解析切片、检索证据，并可在同一数据集内切换前后文档。',
  },
  sources: {
    title: '数据源',
    subtitle: '所有采集来源按文件、网页、音视频和未分类入口归档。',
  },
  members: {
    title: '成员',
    subtitle: '用户、机器人和第三方页面管理入口。',
  },
  audit: {
    title: '审计',
    subtitle: '运行观测、工作流状态、产物和本终端操作记录。',
  },
  'model-pool': {
    title: '模型池',
    subtitle: '统一配置模型 API、并发额度、健康状态和故障切换策略。',
  },
};

function datasetTitle(datasetId, datasets = []) {
  return datasets.find((dataset) => dataset.id === datasetId)?.title || '未知数据集';
}

function documentKind(contentType = '') {
  const lower = String(contentType || '').toLowerCase();
  if (lower.startsWith('audio/') || lower.startsWith('video/')) return '音视频';
  if (lower.includes('html') || lower.includes('url') || lower.includes('web')) return '网页采集';
  if (lower.includes('pdf') || lower.includes('word') || lower.includes('document') || lower.startsWith('text/')) return '文档';
  if (lower.includes('sheet') || lower.includes('excel') || lower.includes('csv')) return '表格';
  if (lower.startsWith('image/')) return '图片';
  return '未分类';
}

function sourceGroups(documents = []) {
  const groups = new Map();
  documents.forEach((document) => {
    const kind = documentKind(document.content_type);
    if (!groups.has(kind)) {
      groups.set(kind, []);
    }
    groups.get(kind).push(document);
  });
  return [...groups.entries()].map(([kind, items]) => ({ kind, items }));
}

function MiniMetric({ label, value }) {
  return (
    <div className="directory-mini-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function DatabaseSourcePanel({ datasets }) {
  const [sources, setSources] = useState([]);
  const [selectedSourceId, setSelectedSourceId] = useState('');
  const [selectedDatasetId, setSelectedDatasetId] = useState('');
  const [status, setStatus] = useState(null);
  const [schema, setSchema] = useState(null);
  const [profile, setProfile] = useState(null);
  const [loading, setLoading] = useState(false);
  const [actionBusy, setActionBusy] = useState('');
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');

  const selectedSource = sources.find((item) => item.id === selectedSourceId) || null;

  async function loadSources() {
    setLoading(true);
    setError('');
    try {
      const nextSources = await fetchDatabaseSourceOptions();
      setSources(nextSources);
      setSelectedSourceId((current) => (
        current && nextSources.some((item) => item.id === current)
          ? current
          : nextSources[0]?.id || ''
      ));
    } catch (loadError) {
      setSources([]);
      setError(loadError instanceof Error ? loadError.message : '数据库源不可用');
    } finally {
      setLoading(false);
    }
  }

  async function loadStatus(sourceId = selectedSourceId) {
    if (!sourceId) {
      setStatus(null);
      return;
    }
    setActionBusy('status');
    try {
      setStatus(await fetchDatabaseSourceStatus(sourceId));
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : '数据库源状态读取失败');
    } finally {
      setActionBusy('');
    }
  }

  async function runAction(kind) {
    if (!selectedSourceId) return;
    setActionBusy(kind);
    setNotice('');
    setError('');
    try {
      if (kind === 'test') {
        const result = await testDatabaseSourceConnection(selectedSourceId);
        setNotice(`连接正常 · ${result?.connection?.server_version || result?.connection?.database || '已通过'}`);
      } else if (kind === 'schema') {
        const nextSchema = await inspectDatabaseSourceSchema(selectedSourceId);
        setSchema(nextSchema);
        setNotice(`结构已读取 · ${nextSchema.tableCount} 张表`);
      } else if (kind === 'profile') {
        const nextProfile = await profileDatabaseSource(selectedSourceId);
        setProfile(nextProfile);
        setNotice(`语义画像已更新 · 指标 ${nextProfile.metricCount} · 维度 ${nextProfile.dimensionCount}`);
      } else if (kind === 'full' || kind === 'incremental') {
        const datasetId = selectedDatasetId || selectedSource?.source?.defaultDatasetId || '';
        if (!datasetId) {
          throw new Error('请先选择目标数据集');
        }
        const result = await startDatabaseSourceSync(selectedSourceId, kind, datasetId);
        setNotice(`同步已提交 · ${result?.sync_run_id || result?.syncRunId || '运行中'}`);
        await loadStatus(selectedSourceId);
      }
    } catch (actionError) {
      setError(actionError instanceof Error ? actionError.message : '数据库源操作失败');
    } finally {
      setActionBusy('');
    }
  }

  useEffect(() => {
    loadSources();
  }, []);

  useEffect(() => {
    if (selectedSourceId) {
      setStatus(null);
      setSchema(null);
      setProfile(null);
      loadStatus(selectedSourceId);
    }
  }, [selectedSourceId]);

  useEffect(() => {
    const defaultDatasetId = selectedSource?.source?.defaultDatasetId || '';
    setSelectedDatasetId(defaultDatasetId || datasets[0]?.id || '');
  }, [selectedSourceId, datasets.map((dataset) => dataset.id).join('|')]);

  const datasetReadiness = status?.datasetReadiness || null;
  const syncReadiness = status?.syncReadiness || null;
  const tableReadiness = status?.tableReadiness || [];
  const recentSyncRuns = status?.recentSyncRuns || [];
  const busy = Boolean(actionBusy);

  return (
    <section className="directory-card database-source-card">
      <div className="directory-section-head">
        <div>
          <h3>数据库源</h3>
          <p>数据库先同步到目标数据集，再进入问答、报表和模板产物链路。</p>
        </div>
        <button type="button" className="ghost-btn compact-action-btn" onClick={loadSources} disabled={loading || busy}>
          {loading ? '刷新中' : '刷新'}
        </button>
      </div>

      {sources.length ? (
        <>
          <div className="database-source-controls">
            <label>
              <span>连接</span>
              <select value={selectedSourceId} onChange={(event) => setSelectedSourceId(event.target.value)} disabled={busy}>
                {sources.map((source) => (
                  <option key={source.id} value={source.id}>
                    {source.displayName}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>目标数据集</span>
              <select value={selectedDatasetId} onChange={(event) => setSelectedDatasetId(event.target.value)} disabled={busy || !datasets.length}>
                {datasets.map((dataset) => (
                  <option key={dataset.id} value={dataset.id}>
                    {dataset.title || dataset.key}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="directory-metric-grid database-source-metrics">
            <MiniMetric label="数据库" value={selectedSource?.source?.database || '未配置'} />
            <MiniMetric label="映射表" value={selectedSource?.source?.tableCount || 0} />
            <MiniMetric label="问答状态" value={datasetReadiness?.label || '未知'} />
            <MiniMetric label="同步状态" value={syncReadiness?.label || '未同步'} />
          </div>

          <div className="database-source-actions">
            <button type="button" className="ghost-btn compact-action-btn" onClick={() => runAction('test')} disabled={busy}>
              {actionBusy === 'test' ? '测试中' : '测试连接'}
            </button>
            <button type="button" className="ghost-btn compact-action-btn" onClick={() => runAction('schema')} disabled={busy}>
              {actionBusy === 'schema' ? '读取中' : '读取结构'}
            </button>
            <button type="button" className="ghost-btn compact-action-btn" onClick={() => runAction('profile')} disabled={busy}>
              {actionBusy === 'profile' ? '画像中' : '语义画像'}
            </button>
            <button type="button" className="primary-btn compact-action-btn" onClick={() => runAction('incremental')} disabled={busy || !selectedDatasetId}>
              {actionBusy === 'incremental' ? '提交中' : '增量同步'}
            </button>
            <button type="button" className="ghost-btn compact-action-btn" onClick={() => runAction('full')} disabled={busy || !selectedDatasetId}>
              {actionBusy === 'full' ? '提交中' : '全量同步'}
            </button>
          </div>

          {notice ? <div className="database-source-notice">{notice}</div> : null}
          {error ? <div className="database-source-error">{error}</div> : null}

          <div className="database-source-detail-grid">
            <article>
              <strong>配置摘要</strong>
              <span>{selectedSource?.source?.kind || 'database'} · {selectedSource?.source?.connectionEnv || '未配置连接引用'}</span>
              <span>默认数据集：{selectedSource?.source?.defaultDatasetId || '未绑定'}</span>
              {selectedSource?.source?.tables?.length ? <span>表：{selectedSource.source.tables.slice(0, 6).join('、')}</span> : null}
            </article>
            <article>
              <strong>同步摘要</strong>
              <span>{syncReadiness?.latestStatus || '暂无同步'} · 行 {syncReadiness?.rowCount || 0} · 文档 {syncReadiness?.documentCount || 0}</span>
              <span>失败行 {syncReadiness?.failedRowCount || 0} · 跳过行 {syncReadiness?.skippedRowCount || 0}</span>
              <span>检查点：{syncReadiness?.checkpointSummary?.label || '无'}</span>
            </article>
          </div>

          {tableReadiness.length ? (
            <div className="database-source-table-list">
              {tableReadiness.slice(0, 6).map((table) => (
                <article key={table.table}>
                  <strong>{table.table}</strong>
                  <span>{table.label} · 文档 {table.documentCount || 0} · 索引 {table.indexedDocumentCount || 0}</span>
                </article>
              ))}
            </div>
          ) : null}

          {schema?.tables?.length ? (
            <div className="database-source-table-list">
              {schema.tables.slice(0, 6).map((table) => (
                <article key={table.table || table.name}>
                  <strong>{table.table || table.name}</strong>
                  <span>列 {table.columnCount} · 预估行 {table.approximateRowCount}</span>
                </article>
              ))}
            </div>
          ) : null}

          {profile?.tables?.length ? (
            <div className="database-source-table-list">
              {profile.tables.slice(0, 6).map((table) => (
                <article key={table.table}>
                  <strong>{table.table}</strong>
                  <span>指标 {table.metricCount} · 维度 {table.dimensionCount} · 置信 {table.mappingConfidence}</span>
                </article>
              ))}
            </div>
          ) : null}

          {recentSyncRuns.length ? (
            <div className="database-source-table-list">
              {recentSyncRuns.slice(0, 4).map((run) => (
                <article key={run.syncRunId}>
                  <strong>{run.status} · {run.syncKind}</strong>
                  <span>{formatRelativeTime(run.updatedAt || run.createdAt)} · 行 {run.rowCount} · 文档 {run.documentCount}</span>
                </article>
              ))}
            </div>
          ) : null}
        </>
      ) : (
        <div className="directory-empty">
          {loading ? '正在读取数据库源。' : '暂无已配置数据库源。'}
        </div>
      )}
    </section>
  );
}

function DatasetsPage({
  datasets,
  selectedDatasetId,
  selectedDatasetIds = [],
  onSelectDataset,
  onClearDatasetSelection,
  datasetDraft,
  onDatasetDraftChange,
  onCreateDataset,
  creatingDataset,
  documents,
  documentsLoading,
  documentSearch,
  onDocumentSearchChange,
  selectedDocumentId,
  onOpenDocumentPage,
  onRefreshDocuments,
  onUpdateDataset,
  onArchiveDataset,
  datasetActionBusy,
  onArchiveDocuments,
  documentActionBusy,
}) {
  const selectedIdSet = new Set(selectedDatasetIds.length ? selectedDatasetIds : selectedDatasetId ? [selectedDatasetId] : []);
  const selectedDataset = datasets.find((dataset) => dataset.id === selectedDatasetId) || null;
  const filteredDocuments = documents.filter((document) => {
    const inDataset = !selectedIdSet.size || selectedIdSet.has(document.dataset_id);
    const query = documentSearch.trim().toLowerCase();
    const matches = !query
      || String(document.title || '').toLowerCase().includes(query)
      || String(document.object_key || '').toLowerCase().includes(query)
      || String(document.content_type || '').toLowerCase().includes(query);
    return inDataset && matches;
  });
  const [selectedDocumentIds, setSelectedDocumentIds] = useState([]);
  const [datasetTitleDraft, setDatasetTitleDraft] = useState('');

  useEffect(() => {
    setDatasetTitleDraft(selectedDataset?.title || '');
  }, [selectedDataset?.id, selectedDataset?.title]);

  useEffect(() => {
    const visibleIds = new Set(filteredDocuments.map((document) => document.id));
    setSelectedDocumentIds((current) => current.filter((id) => visibleIds.has(id)));
  }, [filteredDocuments.map((document) => document.id).join('|')]);

  const toggleDocumentSelection = (documentId) => {
    setSelectedDocumentIds((current) => (
      current.includes(documentId)
        ? current.filter((id) => id !== documentId)
        : [...current, documentId]
    ));
  };

  return (
    <div className="directory-two-column">
      <section className="directory-card">
        <div className="directory-section-head">
          <div>
            <h3>数据集列表</h3>
            <p>左侧只负责选择供料范围；完整管理集中在这里。</p>
          </div>
          <button type="button" className="ghost-btn compact-action-btn" onClick={onClearDatasetSelection}>
            普通聊天
          </button>
        </div>
        <form
          className="directory-create-row"
          onSubmit={(event) => {
            event.preventDefault();
            onCreateDataset?.();
          }}
        >
          <input
            value={datasetDraft.key}
            onChange={(event) => onDatasetDraftChange?.('key', event.target.value)}
            placeholder="数据集 key"
            disabled={creatingDataset}
          />
          <input
            value={datasetDraft.title}
            onChange={(event) => onDatasetDraftChange?.('title', event.target.value)}
            placeholder="数据集标题"
            disabled={creatingDataset}
          />
          <button className="primary-btn" type="submit" disabled={creatingDataset}>
            {creatingDataset ? '创建中' : '新建'}
          </button>
        </form>
        <div className="directory-list">
          {datasets.map((dataset) => (
            <button
              type="button"
              key={dataset.id}
              className={`directory-list-item ${selectedIdSet.has(dataset.id) ? 'active' : ''}`.trim()}
              onClick={() => onSelectDataset?.(dataset.id)}
            >
              <strong>{dataset.title}</strong>
              <span>{dataset.key} · {dataset.visibility === 'private' ? '私密' : '公开'} · {dataset.lifecycle}</span>
            </button>
          ))}
        </div>
        <form
          className="directory-edit-box"
          onSubmit={(event) => {
            event.preventDefault();
            if (selectedDataset) {
              onUpdateDataset?.(selectedDataset.id, { title: datasetTitleDraft });
            }
          }}
        >
          <strong>{selectedDataset ? '当前数据集设置' : '未选择数据集'}</strong>
          <input
            value={datasetTitleDraft}
            onChange={(event) => setDatasetTitleDraft(event.target.value)}
            placeholder="选择数据集后可改名"
            disabled={!selectedDataset || Boolean(datasetActionBusy)}
          />
          <div className="directory-edit-actions">
            <button className="primary-btn compact-action-btn" type="submit" disabled={!selectedDataset || Boolean(datasetActionBusy)}>
              保存
            </button>
            <button
              className="ghost-btn compact-action-btn danger-action"
              type="button"
              disabled={!selectedDataset || Boolean(datasetActionBusy)}
              onClick={() => selectedDataset && onArchiveDataset?.(selectedDataset.id)}
            >
              归档
            </button>
          </div>
        </form>
      </section>

      <section className="directory-card">
        <div className="directory-section-head">
          <div>
            <h3>文档列表</h3>
            <p>支持按标题、路径和类型搜索；点击文档查看解析详情。</p>
          </div>
          <button type="button" className="ghost-btn compact-action-btn" onClick={onRefreshDocuments}>
            刷新
          </button>
        </div>
        <input
          className="directory-search"
          value={documentSearch}
          onChange={(event) => onDocumentSearchChange?.(event.target.value)}
          placeholder="搜索具体文档"
        />
        <div className="directory-batch-bar">
          <span>{documentsLoading ? '同步文档中...' : `当前 ${filteredDocuments.length} 个文档`}</span>
          <button type="button" className="ghost-btn compact-action-btn" disabled>批量归类</button>
          <button
            type="button"
            className="ghost-btn compact-action-btn danger-action"
            disabled={!selectedDocumentIds.length || Boolean(documentActionBusy)}
            onClick={async () => {
              await onArchiveDocuments?.(selectedDocumentIds);
              setSelectedDocumentIds([]);
            }}
          >
            归档 {selectedDocumentIds.length || ''}
          </button>
        </div>
        <div className="directory-document-list">
          {filteredDocuments.length ? filteredDocuments.map((document) => (
            <article
              key={document.id}
              className={`directory-document-item ${document.id === selectedDocumentId ? 'active' : ''}`.trim()}
            >
              <input
                type="checkbox"
                checked={selectedDocumentIds.includes(document.id)}
                onChange={() => toggleDocumentSelection(document.id)}
                aria-label={`选择 ${document.title}`}
              />
              <button type="button" className="directory-document-open" onClick={() => onOpenDocumentPage?.(document.id)}>
                <strong>{document.title}</strong>
                <span>{datasetTitle(document.dataset_id, datasets)} · {documentKind(document.content_type)} · {formatSnakeCaseLabel(document.lifecycle)}</span>
                <em>{truncateText(document.object_key, 64)}</em>
              </button>
            </article>
          )) : (
            <div className="directory-empty">暂无匹配文档。</div>
          )}
        </div>
      </section>
    </div>
  );
}

function DocumentDetailPage({
  datasets,
  documents,
  selectedDocumentId,
  selectedDocumentDetail,
  documentDetailLoading,
  onOpenDocumentPage,
  onBackToDatasets,
  onUpdateDocument,
  onArchiveDocuments,
  documentActionBusy,
}) {
  const {
    orderedChunks,
    evidences,
    selectedDocument,
    previousDocument,
    nextDocument,
    rawText,
    markdownSectionHints,
    modelFacing,
  } = buildDocumentDetailViewModel({
    documents,
    selectedDocumentId,
    selectedDocumentDetail,
  });
  const [documentTitleDraft, setDocumentTitleDraft] = useState('');

  useEffect(() => {
    setDocumentTitleDraft(selectedDocument?.title || '');
  }, [selectedDocument?.id, selectedDocument?.title]);

  if (!selectedDocumentId && !documentDetailLoading) {
    return (
      <div className="document-detail-page">
        <section className="directory-card document-detail-empty-card">
          <h3>尚未选择文档</h3>
          <p>从数据集页点击文档名称后，会在这里打开原文和解析详情。</p>
          <button type="button" className="primary-btn compact-action-btn" onClick={onBackToDatasets}>
            返回数据集
          </button>
        </section>
      </div>
    );
  }

  return (
    <div className="document-detail-page">
      <div className="document-detail-toolbar">
        <button type="button" className="ghost-btn compact-action-btn" onClick={onBackToDatasets}>
          返回数据集
        </button>
        <div className="document-detail-nav">
          <button
            type="button"
            className="ghost-btn compact-action-btn"
            disabled={!previousDocument}
            onClick={() => previousDocument && onOpenDocumentPage?.(previousDocument.id)}
          >
            上一份
          </button>
          <button
            type="button"
            className="ghost-btn compact-action-btn"
            disabled={!nextDocument}
            onClick={() => nextDocument && onOpenDocumentPage?.(nextDocument.id)}
          >
            下一份
          </button>
        </div>
      </div>

      {documentDetailLoading ? (
        <section className="directory-card">
          <div className="directory-empty">正在读取文档原文和解析详情...</div>
        </section>
      ) : selectedDocument ? (
        <div className="document-detail-layout">
          <section className="directory-card document-raw-card">
            <div className="directory-section-head">
              <div>
                <h3>{selectedDocument.title}</h3>
                <p>{datasetTitle(selectedDocument.dataset_id, datasets)} · {documentKind(selectedDocument.content_type)}</p>
              </div>
            </div>
            <div className="document-detail-meta-grid">
              <MiniMetric label="切片" value={orderedChunks.length} />
              <MiniMetric label="证据" value={evidences.length} />
              <MiniMetric label="状态" value={formatSnakeCaseLabel(selectedDocument.lifecycle)} />
            </div>
            <div className="document-original-block">
              <div className="document-block-title">
                <strong>Markdown 原文</strong>
                <span>{rawText ? `${rawText.length} 字符 · 可下滑查看全文` : '暂无可展示原文'}</span>
              </div>
              {rawText ? (
                <>
                  {markdownSectionHints.length ? (
                    <div className="document-md-hints" aria-label="段落标题线索">
                      {markdownSectionHints.map((hint) => <span key={hint}>{hint}</span>)}
                    </div>
                  ) : null}
                  <pre
                    className="document-original-markdown"
                    aria-label="Markdown 原文，可滚动查看全文"
                    tabIndex={0}
                  ><code>{rawText}</code></pre>
                </>
              ) : (
                <div className="directory-empty">当前详情接口没有返回原文内容；若文档已解析，这里会优先展示解析后的正文切片。</div>
              )}
            </div>
          </section>

          <aside className="document-detail-side">
            <section className="directory-card">
              <div className="directory-section-head">
                <div>
                  <h3>文档信息</h3>
                  <p>{truncateText(selectedDocument.object_key, 150)}</p>
                </div>
              </div>
              <div className="document-detail-facts">
                <span>内容类型：{selectedDocument.content_type || '未知'}</span>
                <span>创建：{formatDateTime(selectedDocument.created_at)}</span>
                <span>更新：{formatDateTime(selectedDocument.updated_at)}</span>
              </div>
              <form
                className="directory-edit-box"
                onSubmit={(event) => {
                  event.preventDefault();
                  onUpdateDocument?.(selectedDocument.id, { title: documentTitleDraft });
                }}
              >
                <strong>基础管理</strong>
                <input
                  value={documentTitleDraft}
                  onChange={(event) => setDocumentTitleDraft(event.target.value)}
                  placeholder="文档标题"
                  disabled={Boolean(documentActionBusy)}
                />
                <div className="directory-edit-actions">
                  <button className="primary-btn compact-action-btn" type="submit" disabled={Boolean(documentActionBusy)}>
                    保存标题
                  </button>
                  <button
                    className="ghost-btn compact-action-btn danger-action"
                    type="button"
                    disabled={Boolean(documentActionBusy)}
                    onClick={() => onArchiveDocuments?.([selectedDocument.id])}
                  >
                    归档文档
                  </button>
                </div>
              </form>
            </section>

            <section className="directory-card">
              <div className="directory-section-head">
                <div>
                  <h3>模型可见状态</h3>
                  <p>展示当前详情对模型供料的摘要信号。</p>
                </div>
              </div>
              {modelFacing ? (
                <div className="document-model-facing">
                  <span>证据：{formatSnakeCaseLabel(modelFacing.evidence_state)}</span>
                  <span>后续：{formatSnakeCaseLabel(modelFacing.continuation_state)}</span>
                  <span>工具：{modelFacing.recommended_tool_key || '无推荐'}</span>
                  {Array.isArray(modelFacing.signals) && modelFacing.signals.length ? (
                    <p>{modelFacing.signals.slice(0, 4).join(' · ')}</p>
                  ) : null}
                </div>
              ) : (
                <div className="directory-empty">暂无模型侧摘要。</div>
              )}
            </section>
          </aside>

          <section className="directory-card document-analysis-card">
            <div className="directory-section-head">
              <div>
                <h3>解析切片</h3>
                <p>按解析顺序展示正文切片；段落标题线索会作为 RAG 主要索引提示。</p>
              </div>
            </div>
            <div className="document-chunk-list">
              {orderedChunks.length ? orderedChunks.map((chunk) => {
                const hints = chunkSectionHints(chunk);
                return (
                  <article key={chunk.id} className="document-chunk-card">
                    <div>
                      <strong>Chunk {chunk.chunk_index} · {chunk.token_count} tokens</strong>
                      <span>{formatSnakeCaseLabel(chunk.state)}</span>
                    </div>
                    {hints.length ? <em>{hints.join(' / ')}</em> : null}
                    <p>{chunk.content}</p>
                  </article>
                );
              }) : (
                <div className="directory-empty">当前文档尚无解析切片。</div>
              )}
            </div>
          </section>

          <section className="directory-card document-analysis-card">
            <div className="directory-section-head">
              <div>
                <h3>检索证据</h3>
                <p>展示已经索引的召回证据和定位信息。</p>
              </div>
            </div>
            <div className="document-evidence-list">
              {evidences.length ? evidences.map((evidence) => (
                <article key={evidence.id} className="document-evidence-card">
                  <div>
                    <strong>Rank {evidence.evidence_manifest_view?.recall?.rank_hint ?? evidence.chunk_index}</strong>
                    <span>{Number.isFinite(Number(evidence.recall_score)) ? Number(evidence.recall_score).toFixed(3) : '0.000'}</span>
                  </div>
                  <p>{evidence.summary || evidence.content_excerpt}</p>
                  <em>{evidence.source_locator}</em>
                </article>
              )) : (
                <div className="directory-empty">暂无检索证据；文档完成检索索引后会显示。</div>
              )}
            </div>
          </section>
        </div>
      ) : (
        <section className="directory-card">
          <div className="directory-empty">没有找到这个文档，可能已归档或当前账号不可见。</div>
        </section>
      )}
    </div>
  );
}

function SourcesPage({ documents, datasets }) {
  const groups = sourceGroups(documents);
  return (
    <div className="directory-grid-cards">
      <DatabaseSourcePanel datasets={datasets} />
      {groups.length ? groups.map((group) => (
        <section className="directory-card" key={group.kind}>
          <div className="directory-section-head">
            <div>
              <h3>{group.kind}</h3>
              <p>{group.items.length} 个采集对象。</p>
            </div>
          </div>
          <div className="directory-source-list">
            {group.items.slice(0, 8).map((document) => (
              <article key={document.id}>
                <strong>{document.title}</strong>
                <span>{datasetTitle(document.dataset_id, datasets)} · {formatRelativeTime(document.updated_at)}</span>
              </article>
            ))}
          </div>
        </section>
      )) : (
        <section className="directory-card">
          <h3>暂无采集源</h3>
          <p>上传文件、网页采集或音视频后，会按类型进入这里。</p>
        </section>
      )}
    </div>
  );
}

function MembersLoginOverlay({ accountStatusSummary }) {
  return (
    <div className="members-login-overlay" role="status" aria-live="polite">
      <div className="members-login-card">
        <span>需要登录</span>
        <h3>成员页属于账号空间</h3>
        <p>用户、机器人、第三方页面和对话成员组会跟随登录账号。请先在右上角“登录状态”完成邮箱密钥或验证码登录。</p>
        <div className="members-login-steps">
          <strong>{accountStatusSummary?.label || '当前未登录'}</strong>
          <span>右上角“登录状态”里完成邮箱、验证码或密钥登录</span>
        </div>
      </div>
    </div>
  );
}

function MembersPage({ accountStatusSummary }) {
  const signedIn = Boolean(accountStatusSummary?.signedIn);
  const cards = [
    ['用户管理', accountStatusSummary?.label || '未登录', '邮箱、密钥和私密数据归属先在顶部登录状态里管理。'],
    ['对话 / 成员组', '入口已接入', '顶部对话名称可下拉选择、重命名或新建对话；后续补归档旧对话。'],
    ['机器人管理', '规划中', '后续把可复用机器人、默认提示和工具权限放在这里。'],
    ['第三方页面管理', '规划中', '外部页面、嵌入入口和公开分享页统一归档。'],
  ];
  return (
    <div className={`members-page-shell${signedIn ? '' : ' locked'}`}>
      <div className="directory-grid-cards members-grid" aria-hidden={!signedIn}>
        {cards.map(([title, status, detail]) => (
          <section className="directory-card member-card" key={title}>
            <span>{status}</span>
            <h3>{title}</h3>
            <p>{detail}</p>
            <button type="button" className="ghost-btn compact-action-btn" disabled>管理入口</button>
          </section>
        ))}
      </div>
      {signedIn ? null : <MembersLoginOverlay accountStatusSummary={accountStatusSummary} />}
    </div>
  );
}

function AuditPage({ stats, activityEvents, htmlArtifacts }) {
  return (
    <div className="directory-two-column audit-layout">
      <section className="directory-card">
        <div className="directory-section-head">
          <div>
            <h3>运行观测</h3>
            <p>先用项目级计数和本终端动作打底，后续接工作流追踪表。</p>
          </div>
        </div>
        <div className="directory-metric-grid">
          <MiniMetric label="会话" value={stats.sessions} />
          <MiniMetric label="输出" value={stats.outputs} />
          <MiniMetric label="报告计划" value={stats.plans} />
          <MiniMetric label="HTML 产物" value={htmlArtifacts.length} />
        </div>
      </section>
      <section className="directory-card">
        <div className="directory-section-head">
          <div>
            <h3>最近动作</h3>
            <p>上传、页面生成和关键操作会落到这里。</p>
          </div>
        </div>
        <div className="directory-detail-list">
          {activityEvents.length ? activityEvents.map((event) => (
            <article key={event.id}>
              <strong>{event.summary || event.kind}</strong>
              <p>{formatDateTime(event.created_at)}</p>
            </article>
          )) : <div className="directory-empty">暂无本终端操作记录。</div>}
        </div>
      </section>
    </div>
  );
}

export default function WorkspaceDirectoryPanel({
  activePage,
  datasets,
  selectedDatasetId,
  selectedDatasetIds = [],
  onSelectDataset,
  onClearDatasetSelection,
  datasetDraft,
  onDatasetDraftChange,
  onCreateDataset,
  creatingDataset,
  documents,
  documentsLoading,
  documentSearch,
  onDocumentSearchChange,
  selectedDocumentId,
  onOpenDocumentPage,
  onBackToDatasets,
  selectedDocumentDetail,
  documentDetailLoading,
  onRefreshDocuments,
  onUpdateDataset,
  onArchiveDataset,
  datasetActionBusy,
  onUpdateDocument,
  onArchiveDocuments,
  documentActionBusy,
  stats,
  accountStatusSummary,
  activityEvents,
  htmlArtifacts,
}) {
  const copy = PAGE_COPY[activePage] || PAGE_COPY.datasets;
  return (
    <section className="directory-panel">
      <div className="directory-hero">
        <span>智能助手 / {copy.title}</span>
        <h2>{copy.title}</h2>
        <p>{copy.subtitle}</p>
      </div>
      {activePage === 'datasets' ? (
        <DatasetsPage
          datasets={datasets}
          selectedDatasetId={selectedDatasetId}
          selectedDatasetIds={selectedDatasetIds}
          onSelectDataset={onSelectDataset}
          onClearDatasetSelection={onClearDatasetSelection}
          datasetDraft={datasetDraft}
          onDatasetDraftChange={onDatasetDraftChange}
          onCreateDataset={onCreateDataset}
          creatingDataset={creatingDataset}
          documents={documents}
          documentsLoading={documentsLoading}
          documentSearch={documentSearch}
          onDocumentSearchChange={onDocumentSearchChange}
          selectedDocumentId={selectedDocumentId}
          onOpenDocumentPage={onOpenDocumentPage}
          onRefreshDocuments={onRefreshDocuments}
          onUpdateDataset={onUpdateDataset}
          onArchiveDataset={onArchiveDataset}
          datasetActionBusy={datasetActionBusy}
          onArchiveDocuments={onArchiveDocuments}
          documentActionBusy={documentActionBusy}
        />
      ) : null}
      {activePage === 'document-detail' ? (
        <DocumentDetailPage
          datasets={datasets}
          documents={documents}
          selectedDocumentId={selectedDocumentId}
          selectedDocumentDetail={selectedDocumentDetail}
          documentDetailLoading={documentDetailLoading}
          onOpenDocumentPage={onOpenDocumentPage}
          onBackToDatasets={onBackToDatasets}
          onUpdateDocument={onUpdateDocument}
          onArchiveDocuments={onArchiveDocuments}
          documentActionBusy={documentActionBusy}
        />
      ) : null}
      {activePage === 'sources' ? <SourcesPage documents={documents} datasets={datasets} /> : null}
      {activePage === 'members' ? <MembersPage accountStatusSummary={accountStatusSummary} /> : null}
      {activePage === 'audit' ? <AuditPage stats={stats} activityEvents={activityEvents} htmlArtifacts={htmlArtifacts} /> : null}
      {activePage === 'model-pool' ? <ModelPoolPanel accountStatusSummary={accountStatusSummary} /> : null}
    </section>
  );
}
