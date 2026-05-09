'use client';

import { useEffect, useState } from 'react';
import { formatDateTime, formatRelativeTime, formatSnakeCaseLabel, truncateText } from '../lib/formatters';

const PAGE_COPY = {
  datasets: {
    title: '数据集',
    subtitle: '数据集列表、文档列表、解析详情和基础批量管理。',
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

function DirectoryComposer({
  input,
  onInputChange,
  onSubmit,
  onUploadClick,
  onStartStaticPageDraft,
  submitting,
  uploadingFiles,
}) {
  return (
    <div className="directory-floating-composer">
      <textarea
        value={input}
        onChange={(event) => onInputChange?.(event.target.value)}
        placeholder="告诉智能助手你要查资料、整理数据集、生成页面或继续修改当前产物"
        disabled={submitting}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault();
            if (!submitting) onSubmit?.();
          }
        }}
      />
      <div className="directory-composer-actions">
        <button className="primary-btn" type="button" onClick={onSubmit} disabled={!input.trim() || submitting}>
          {submitting ? '发送中...' : '发送'}
        </button>
        <button className="ghost-btn" type="button" onClick={onUploadClick} disabled={submitting || !onUploadClick}>
          {uploadingFiles ? '上传中...' : '上传'}
        </button>
        <button className="ghost-btn" type="button" onClick={() => onStartStaticPageDraft?.({ oneClick: true })} disabled={submitting}>
          页面
        </button>
      </div>
    </div>
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
  onSelectDocument,
  selectedDocumentDetail,
  documentDetailLoading,
  onRefreshDocuments,
  onUpdateDataset,
  onArchiveDataset,
  datasetActionBusy,
  onUpdateDocument,
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
  const chunks = Array.isArray(selectedDocumentDetail?.chunks) ? selectedDocumentDetail.chunks : [];
  const evidences = Array.isArray(selectedDocumentDetail?.retrieval_evidences)
    ? selectedDocumentDetail.retrieval_evidences
    : [];
  const [selectedDocumentIds, setSelectedDocumentIds] = useState([]);
  const [datasetTitleDraft, setDatasetTitleDraft] = useState('');
  const [documentTitleDraft, setDocumentTitleDraft] = useState('');

  useEffect(() => {
    setDatasetTitleDraft(selectedDataset?.title || '');
  }, [selectedDataset?.id, selectedDataset?.title]);

  useEffect(() => {
    setDocumentTitleDraft(selectedDocumentDetail?.document?.title || '');
  }, [selectedDocumentDetail?.document?.id, selectedDocumentDetail?.document?.title]);

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
  const selectedDocument = selectedDocumentDetail?.document || null;

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
              <button type="button" className="directory-document-open" onClick={() => onSelectDocument?.(document.id)}>
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

      <section className="directory-card directory-detail-card">
        <div className="directory-section-head">
          <div>
            <h3>解析详情</h3>
            <p>基础解析状态、切片和检索证据先集中展示。</p>
          </div>
        </div>
        {documentDetailLoading ? (
          <div className="directory-empty">正在读取解析详情...</div>
        ) : selectedDocumentDetail ? (
          <>
            <div className="directory-metric-grid">
              <MiniMetric label="切片" value={chunks.length} />
              <MiniMetric label="证据" value={evidences.length} />
              <MiniMetric label="状态" value={formatSnakeCaseLabel(selectedDocumentDetail.document.lifecycle)} />
            </div>
            <div className="directory-detail-block">
              <strong>{selectedDocumentDetail.document.title}</strong>
              <span>{selectedDocumentDetail.document.content_type}</span>
              <p>{truncateText(selectedDocumentDetail.document.object_key, 160)}</p>
            </div>
            <form
              className="directory-edit-box"
              onSubmit={(event) => {
                event.preventDefault();
                if (selectedDocument) {
                  onUpdateDocument?.(selectedDocument.id, { title: documentTitleDraft });
                }
              }}
            >
              <strong>文档基础管理</strong>
              <input
                value={documentTitleDraft}
                onChange={(event) => setDocumentTitleDraft(event.target.value)}
                placeholder="文档标题"
                disabled={Boolean(documentActionBusy)}
              />
              <div className="directory-edit-actions">
                <button className="primary-btn compact-action-btn" type="submit" disabled={!selectedDocument || Boolean(documentActionBusy)}>
                  保存标题
                </button>
                <button
                  className="ghost-btn compact-action-btn danger-action"
                  type="button"
                  disabled={!selectedDocument || Boolean(documentActionBusy)}
                  onClick={() => selectedDocument && onArchiveDocuments?.([selectedDocument.id])}
                >
                  归档文档
                </button>
              </div>
            </form>
            <div className="directory-detail-list">
              {chunks.slice(0, 5).map((chunk) => (
                <article key={chunk.id}>
                  <strong>Chunk {chunk.chunk_index} · {chunk.token_count} tokens</strong>
                  <p>{truncateText(chunk.content, 180)}</p>
                </article>
              ))}
            </div>
          </>
        ) : (
          <div className="directory-empty">从文档列表选择一个文档查看解析详情。</div>
        )}
      </section>
    </div>
  );
}

function SourcesPage({ documents, datasets }) {
  const groups = sourceGroups(documents);
  return (
    <div className="directory-grid-cards">
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

function MembersPage({ accountStatusSummary }) {
  const cards = [
    ['用户管理', accountStatusSummary?.label || '未登录', '邮箱、密钥和私密数据归属先在顶部登录状态里管理。'],
    ['对话 / 成员组', '入口已接入', '顶部 + 新建对话；后续在这里下拉选择、重新唤起或归档旧对话。'],
    ['机器人管理', '规划中', '后续把可复用机器人、默认提示和工具权限放在这里。'],
    ['第三方页面管理', '规划中', '外部页面、嵌入入口和公开分享页统一归档。'],
  ];
  return (
    <div className="directory-grid-cards">
      {cards.map(([title, status, detail]) => (
        <section className="directory-card member-card" key={title}>
          <span>{status}</span>
          <h3>{title}</h3>
          <p>{detail}</p>
          <button type="button" className="ghost-btn compact-action-btn" disabled>管理入口</button>
        </section>
      ))}
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
  onSelectDocument,
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
  input,
  onInputChange,
  onSubmit,
  onUploadClick,
  onStartStaticPageDraft,
  submitting,
  uploadingFiles,
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
          onSelectDocument={onSelectDocument}
          selectedDocumentDetail={selectedDocumentDetail}
          documentDetailLoading={documentDetailLoading}
          onRefreshDocuments={onRefreshDocuments}
          onUpdateDataset={onUpdateDataset}
          onArchiveDataset={onArchiveDataset}
          datasetActionBusy={datasetActionBusy}
          onUpdateDocument={onUpdateDocument}
          onArchiveDocuments={onArchiveDocuments}
          documentActionBusy={documentActionBusy}
        />
      ) : null}
      {activePage === 'sources' ? <SourcesPage documents={documents} datasets={datasets} /> : null}
      {activePage === 'members' ? <MembersPage accountStatusSummary={accountStatusSummary} /> : null}
      {activePage === 'audit' ? <AuditPage stats={stats} activityEvents={activityEvents} htmlArtifacts={htmlArtifacts} /> : null}
      <DirectoryComposer
        input={input}
        onInputChange={onInputChange}
        onSubmit={onSubmit}
        onUploadClick={onUploadClick}
        onStartStaticPageDraft={onStartStaticPageDraft}
        submitting={submitting}
        uploadingFiles={uploadingFiles}
      />
    </section>
  );
}
