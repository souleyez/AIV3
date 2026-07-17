'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import {
  applyDatabaseSourceProfile,
  fetchDatabaseSourceOptions,
  fetchDatabaseSourceStatus,
  inspectDatabaseSourceSchema,
  profileDatabaseSource,
  startDatabaseSourceSync,
  testDatabaseSourceConnection,
} from '../lib/database-source';
import {
  ASSET_LIBRARY_PRESETS,
  assetLibraryContainsDataset,
  assetParseStatusEntries,
  assetProfileKindOptions,
  filterAssetProfileHints,
} from '../lib/asset-library-view-model';
import { buildDocumentDetailViewModel, chunkSectionHints } from '../lib/document-detail-view';
import { buildDatasetDictionaryView } from '../lib/dataset-dictionary-view-model';
import { fetchDatasetTabularSchema } from '../lib/dataset-tabular-schema-api';
import { formatDateTime, formatRelativeTime, formatSnakeCaseLabel, truncateText } from '../lib/formatters';
import {
  buildConnectedSourceCards,
  connectedDocumentCount,
  sourceDisplayName,
} from '../lib/source-workspace-view-model';
import { buildTabularDocumentPreview, isTabularDocument } from '../lib/tabular-document-view';
import DatasetUnderstandingGraph from './DatasetUnderstandingGraph';
import ModelPoolPanel from './ModelPoolPanel';

const SOURCE_DISPLAY_ALIAS_STORAGE_KEY = 'datamax-v3:source-display-aliases:v1';

const PAGE_COPY = {
  datasets: {
    title: '数据集',
    subtitle: '从顶部数据集选择器选中范围后查看解析关系图谱，并在下方进入文档原文与解析详情。',
  },
  'document-detail': {
    title: '文档详情',
    subtitle: '查看文档原文、解析切片、检索证据，并可在同一数据集内切换前后文档。',
  },
  sources: {
    title: '数据源',
    subtitle: '统一展示网页采集、数据库接入、ERP 登录、API 对接、MCP 对接等资料接入方式和已接入数据。',
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

function documentDatasetIds(document) {
  return [...new Set([
    document?.dataset_id,
    document?.datasetId,
    ...(Array.isArray(document?.dataset_ids) ? document.dataset_ids : []),
    ...(Array.isArray(document?.datasetIds) ? document.datasetIds : []),
  ].map((id) => String(id || '').trim()).filter(Boolean))];
}

function documentDatasetTitles(document, datasets = []) {
  const titles = documentDatasetIds(document)
    .map((datasetId) => datasetTitle(datasetId, datasets))
    .filter(Boolean);
  return titles.length ? titles.join('、') : '未知数据集';
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
    if (!groups.has(kind)) groups.set(kind, []);
    groups.get(kind).push(document);
  });
  return [...groups.entries()].map(([kind, items]) => ({ kind, items }));
}

function latestDocumentUpdatedAt(items = []) {
  const latest = items
    .map((item) => new Date(item.updated_at || item.updatedAt || item.created_at || item.createdAt || 0).getTime())
    .filter((value) => Number.isFinite(value) && value > 0)
    .sort((left, right) => right - left)[0];
  return latest ? new Date(latest).toISOString() : '';
}

const SOURCE_ACCESS_METHODS = [
  {
    title: '网页采集',
    status: '可接入',
    detail: '可把公开页面、业务后台页面、文档站和指定网页内容采集入库，后续进入问答、报表和静态页链路。',
  },
  {
    title: '数据库接入',
    status: '已接入',
    detail: '支持配置数据库连接、读取表结构、生成语义画像，并同步到目标数据集用于经营分析和报表。',
  },
  {
    title: 'ERP 登录',
    status: '规划接入',
    detail: '适合需要账号登录、页面跳转和权限隔离的 ERP/CRM/业务系统，接入后按授权范围采集业务数据。',
  },
  {
    title: 'API 对接',
    status: '可接入',
    detail: '第三方系统可通过稳定接口推送文档、分组权限、临时附件和对话请求，DataMax 负责解析、检索和产物回传。',
  },
  {
    title: 'MCP 对接',
    status: '规划接入',
    detail: '面向企业内部工具、知识库、工单和业务动作，把可调用能力作为工具暴露给本地智能体。',
  },
  {
    title: '文件与模板',
    status: '已接入',
    detail: '支持 PDF、Word、Excel、CSV、图片、音视频和模板参考文件，入库后可用于问答、报表和页面生成。',
  },
];

function AccessMethodGuide({ documents, datasets }) {
  return (
    <aside className="directory-card source-guide-card source-access-rail">
      <div className="directory-section-head">
        <div>
          <h3>接入方式</h3>
          <p>选择资料入口；接入后统一进入左侧动态监测。</p>
        </div>
      </div>
      <div className="source-method-list">
        {SOURCE_ACCESS_METHODS.map((item) => (
          <article key={item.title} className="source-method-card">
            <div>
              <strong>{item.title}</strong>
              <span>{item.status}</span>
            </div>
            <p>{item.detail}</p>
          </article>
        ))}
      </div>
      <div className="source-guide-summary" aria-label="当前接入概览">
        <MiniMetric label="已接入文档" value={documents.length} />
        <MiniMetric label="可归档数据集" value={datasets.length} />
        <MiniMetric label="支持方式" value={SOURCE_ACCESS_METHODS.length} />
      </div>
    </aside>
  );
}

function normalizedSourceAliases(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return Object.fromEntries(Object.entries(value)
    .map(([sourceId, displayName]) => [String(sourceId || '').trim(), String(displayName || '').trim()])
    .filter(([sourceId, displayName]) => sourceId && displayName));
}

function readSourceDisplayAliases() {
  if (typeof window === 'undefined') return {};
  try {
    return normalizedSourceAliases(JSON.parse(window.localStorage.getItem(SOURCE_DISPLAY_ALIAS_STORAGE_KEY) || '{}'));
  } catch {
    return {};
  }
}

function writeSourceDisplayAliases(aliases) {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(SOURCE_DISPLAY_ALIAS_STORAGE_KEY, JSON.stringify(normalizedSourceAliases(aliases)));
  } catch {
    // A blocked storage surface should not prevent the source page from working.
  }
}

function formatCompactCount(value) {
  const count = Number(value || 0);
  if (!Number.isFinite(count)) return '0';
  return new Intl.NumberFormat('zh-CN', { notation: count >= 10000 ? 'compact' : 'standard', maximumFractionDigits: 1 }).format(count);
}

function SourceDisplayNameEditor({
  sourceId,
  defaultName,
  aliases,
  editingSourceId,
  aliasDraft,
  onAliasDraftChange,
  onStartEditing,
  onSave,
  onCancel,
  onReset,
}) {
  const displayName = sourceDisplayName({ id: sourceId, title: defaultName }, aliases);
  const hasAlias = Boolean(aliases[sourceId]);
  const editing = editingSourceId === sourceId;

  if (editing) {
    return (
      <div className="source-alias-editor">
        <input
          className="source-alias-input"
          value={aliasDraft}
          maxLength={64}
          autoFocus
          aria-label={`${defaultName}的显示名称`}
          onChange={(event) => onAliasDraftChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') onSave(sourceId, defaultName);
            if (event.key === 'Escape') onCancel();
          }}
        />
        <button type="button" className="primary-btn compact-action-btn" onClick={() => onSave(sourceId, defaultName)}>保存</button>
        <button type="button" className="ghost-btn compact-action-btn" onClick={onCancel}>取消</button>
      </div>
    );
  }

  return (
    <div className="source-display-name">
      <div>
        <strong>{displayName}</strong>
        <span>{hasAlias ? `原始名称：${defaultName} · 别名仅当前浏览器可见` : '显示名称仅用于当前浏览器查看'}</span>
      </div>
      <div className="source-display-name-actions">
        <button type="button" className="ghost-btn compact-action-btn" onClick={() => onStartEditing(sourceId, displayName)}>命名</button>
        {hasAlias ? <button type="button" className="ghost-btn compact-action-btn" onClick={() => onReset(sourceId)}>恢复</button> : null}
      </div>
    </div>
  );
}

function SourceLiveMonitor({
  sources,
  understandingState,
  uniqueConnectedDocumentCount = 0,
  loading = false,
  lastCheckedAt,
  aliases,
  editingSourceId,
  aliasDraft,
  onAliasDraftChange,
  onStartEditing,
  onSave,
  onCancel,
  onReset,
  onRefresh,
  onOpenDocumentPage,
}) {
  const [expandedSourceId, setExpandedSourceId] = useState('');

  function toggleSourceDetails(sourceId) {
    const opening = expandedSourceId !== sourceId;
    setExpandedSourceId(opening ? sourceId : '');
  }

  return (
    <section className="directory-card connected-source-card">
      <div className="directory-section-head connected-source-toolbar">
        <div>
          <h3>动态导入监测</h3>
          <p>{uniqueConnectedDocumentCount ? `${sources.length} 个有内容数据集 · ${uniqueConnectedDocumentCount} 个去重可见对象；共享对象可能出现在多个分组。` : '暂无已接入数据，上传或采集完成后会显示在这里。'}</p>
        </div>
        <div className="source-live-state" aria-live="polite">
          <span className={`source-live-dot ${loading ? 'loading' : ''}`.trim()} />
          <div>
            <strong>{loading ? '正在抓取变化' : '动态监测中'}</strong>
            <span>{lastCheckedAt ? `最近检查 ${formatRelativeTime(lastCheckedAt)} · 每 60 秒` : '等待首次检查'}</span>
          </div>
          <button type="button" className="ghost-btn compact-action-btn" onClick={onRefresh} disabled={loading}>
            {loading ? '刷新中' : '立即刷新'}
          </button>
        </div>
      </div>
      {loading && !sources.length ? (
        <div className="directory-empty">正在读取已接入数据和最近变化。</div>
      ) : sources.length ? (
        <div className="connected-source-grid">
          {sources.map((source) => {
            const parseSummary = Object.entries(source.parseCounts || {})
              .filter(([, count]) => Number(count) > 0)
              .slice(0, 3)
              .map(([status, count]) => `${formatSnakeCaseLabel(status)} ${count}`)
              .join(' · ');
            const sourceKinds = [...new Set((source.contentTypes || []).map(documentKind))];
            const detailsExpanded = expandedSourceId === source.id;
            return (
              <article className="connected-source-card-item" key={source.id}>
                <div className="connected-source-card-head">
                  <SourceDisplayNameEditor
                    sourceId={source.id}
                    defaultName={source.title}
                    aliases={aliases}
                    editingSourceId={editingSourceId}
                    aliasDraft={aliasDraft}
                    onAliasDraftChange={onAliasDraftChange}
                    onStartEditing={onStartEditing}
                    onSave={onSave}
                    onCancel={onCancel}
                    onReset={onReset}
                  />
                  <span className="source-original-name">{sourceKinds.join(' / ') || '未分类'} · {source.latestUpdatedAt ? `更新于 ${formatRelativeTime(source.latestUpdatedAt)}` : '暂无更新时间'}</span>
                </div>

                <div className="source-stat-row" aria-label={`${source.title}数量概览`}>
                  <div className="source-stat"><span>采集对象</span><strong>{formatCompactCount(source.documentCount)}</strong></div>
                  <div className="source-stat"><span>预估字数</span><strong>{formatCompactCount(source.estimatedWordCount)}</strong></div>
                  <div className="source-stat"><span>近 24 小时</span><strong>{formatCompactCount(source.recent24hCount)}</strong></div>
                </div>

                <div>
                  <span className="source-section-label">结构 / 主题提示</span>
                  <div className="source-field-list">
                    {source.fieldHints.length ? source.fieldHints.map((field) => (
                      <span className="source-field-chip" key={field}>{field}</span>
                    )) : <span className="source-field-empty">暂无可用结构提示</span>}
                  </div>
                </div>

                <div className="source-change-schema-grid">
                  <div className="source-change-column">
                    <span className="source-section-label">最近变化</span>
                    <div className="source-change-list">
                      {source.recentDocuments.length ? source.recentDocuments.map((document) => (
                        <div className="source-change-item" key={document.id}>
                          <span className="source-live-dot" />
                          <div>
                            <strong>{document.title || '未命名资料'}</strong>
                            <span>{formatRelativeTime(document.updated_at || document.updatedAt || document.created_at || document.createdAt)} · {formatSnakeCaseLabel(document.parse_status || document.parseStatus || document.lifecycle || 'unknown')}</span>
                          </div>
                        </div>
                      )) : <span className="source-field-empty">暂无变化记录</span>}
                    </div>
                  </div>
                  <div className="source-inline-schema-column">
                    <div className="source-inline-schema-head">
                      <div>
                        <span className="source-section-label">响应字段</span>
                        <strong>业务表字段与字典</strong>
                        <small>CSV / TSV 真实表头；JSON 和 Markdown 仅作配置、质量或说明对象。</small>
                      </div>
                      <button
                        type="button"
                        className="ghost-btn compact-action-btn source-detail-toggle"
                        aria-expanded={detailsExpanded}
                        aria-controls={`source-detail-${source.id}`}
                        onClick={() => toggleSourceDetails(source.id)}
                      >
                        {detailsExpanded ? '收起详情' : '展开详情'}
                      </button>
                    </div>
                    {detailsExpanded ? (
                      <div id={`source-detail-${source.id}`} className="source-inline-schema-detail">
                        <SourceDatasetDetails
                          dataset={source}
                          understandingState={understandingState}
                        />
                      </div>
                    ) : (
                      <div className="source-inline-schema-placeholder">展开后显示结构表、字段中文释义、类型、语义角色和质量统计。</div>
                    )}
                  </div>
                </div>

                <span className="source-parse-summary">{parseSummary || '暂无解析状态'}</span>
              </article>
            );
          })}
        </div>
      ) : (
        <div className="directory-empty">暂无采集源。上传文件、网页采集或业务系统同步完成后，会进入动态监测。</div>
      )}
    </section>
  );
}

function ConnectedObjectDirectory({ groups, datasets, loading = false, onOpenDocumentPage }) {
  const total = groups.reduce((sum, group) => sum + group.items.length, 0);
  return (
    <section className="directory-card connected-source-card connected-object-browser">
      <div className="directory-section-head">
        <div>
          <h3>已接入数据明细</h3>
          <p>{total ? `当前已归档 ${total} 个采集对象，可按类型展开查看。` : '暂无已接入数据，上传或采集完成后会显示在这里。'}</p>
        </div>
      </div>
      {loading ? (
        <div className="directory-empty">正在读取已接入数据。</div>
      ) : groups.length ? (
        <div className="connected-source-list">
          {groups.map((group, index) => {
            const latestUpdatedAt = latestDocumentUpdatedAt(group.items);
            return (
              <details className="connected-source-group" key={group.kind} open={index === 0}>
                <summary>
                  <div>
                    <strong>{group.kind}</strong>
                    <span>
                      {group.items.length} 个对象
                      {latestUpdatedAt ? ` · 最近更新 ${formatRelativeTime(latestUpdatedAt)}` : ''}
                    </span>
                  </div>
                  <em>展开</em>
                </summary>
                <div className="directory-source-list connected-source-documents">
                  {group.items.map((document) => (
                    <button
                      key={document.id}
                      type="button"
                      className="connected-source-document"
                      onClick={() => onOpenDocumentPage?.(document.id)}
                      disabled={!document.id || !onOpenDocumentPage}
                    >
                      <div>
                        <strong>{document.title || '未命名资料'}</strong>
                        <span>
                          {documentDatasetTitles(document, datasets)}
                          {' · '}{formatRelativeTime(document.updated_at || document.updatedAt)}
                          {document.lifecycle ? ` · ${formatSnakeCaseLabel(document.lifecycle)}` : ''}
                        </span>
                        <small>{truncateText(document.object_key || document.objectKey || document.external_id || document.id, 88)}</small>
                      </div>
                      <em>查看明细</em>
                    </button>
                  ))}
                </div>
              </details>
            );
          })}
        </div>
      ) : (
        <div className="directory-empty">暂无采集源。上传文件、网页采集或业务系统同步完成后，会按类型进入这里。</div>
      )}
    </section>
  );
}

function MiniMetric({ label, value }) {
  return (
    <div className="directory-mini-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

const SEMANTIC_ROLE_LABELS = {
  primary_key: '主键标识',
  identifier: '标识字段',
  entity: '业务实体',
  dimension: '分析维度',
  metric: '统计指标',
  amount: '金额指标',
  time: '时间维度',
  name: '名称字段',
  text: '文本内容',
  boolean: '布尔状态',
  category: '分类维度',
  unknown: '待确认',
};

function semanticRoleLabel(role = '') {
  const normalized = String(role || '').toLowerCase();
  return SEMANTIC_ROLE_LABELS[normalized] || formatSnakeCaseLabel(normalized || 'unknown');
}

function dictionaryFieldDescription(field = {}) {
  if (field.description) return field.description;
  const label = field.label || field.technical_name || '当前字段';
  const role = semanticRoleLabel(field.semantic_role);
  const valueType = field.value_type && field.value_type !== 'unknown' ? `，值类型 ${field.value_type}` : '';
  return `${label}；系统识别为${role}${valueType}。`;
}

function dictionaryConfidenceLabel(value) {
  const confidence = Number(value);
  if (!Number.isFinite(confidence) || confidence <= 0) return '0%';
  return `${Math.round(confidence <= 1 ? confidence * 100 : confidence)}%`;
}

function SourceDatasetDetails({ dataset, understandingState }) {
  const datasetId = String(dataset?.id || '').trim();
  const requestScope = String(understandingState?.requestScope || '');
  const sourceVersion = `${dataset?.documentCount || 0}:${dataset?.latestUpdatedAt || ''}`;
  const [tabularSchemaState, setTabularSchemaState] = useState({
    datasetId,
    scopeKey: requestScope,
    requestScope,
    status: 'loading',
    data: null,
    error: '',
  });

  useEffect(() => {
    if (!datasetId) return undefined;
    const controller = new AbortController();
    let active = true;
    setTabularSchemaState({
      datasetId,
      scopeKey: requestScope,
      requestScope,
      status: 'loading',
      data: null,
      error: '',
    });
    fetchDatasetTabularSchema(datasetId, { signal: controller.signal }).then((data) => {
      if (!active) return;
      setTabularSchemaState({
        datasetId,
        scopeKey: requestScope,
        requestScope,
        status: data.tables.length ? 'ready' : 'empty',
        data,
        error: '',
      });
    }).catch((loadError) => {
      if (!active || loadError?.name === 'AbortError') return;
      setTabularSchemaState({
        datasetId,
        scopeKey: requestScope,
        requestScope,
        status: 'failed',
        data: null,
        error: loadError instanceof Error ? loadError.message : '真实表头读取失败',
      });
    });
    return () => {
      active = false;
      controller.abort();
    };
  }, [datasetId, requestScope, sourceVersion]);

  return (
    <DatasetDictionaryContent
      dataset={dataset}
      understandingState={understandingState}
      tabularSchemaState={tabularSchemaState}
    />
  );
}

function DatasetDictionaryContent({
  dataset,
  understandingState,
  tabularSchemaState,
}) {
  const selectedDataset = dataset || null;
  const selectedDatasetId = String(selectedDataset?.id || '').trim();
  const requestScope = String(
    understandingState?.requestScope || tabularSchemaState?.requestScope || '',
  );
  const stateMatches = understandingState?.datasetId === selectedDatasetId
    && (!requestScope || understandingState?.scopeKey === requestScope);
  const understanding = stateMatches ? understandingState?.data : null;
  const schemaStateMatches = tabularSchemaState?.datasetId === selectedDatasetId
    && (!requestScope || tabularSchemaState?.scopeKey === requestScope);
  const tabularSchema = schemaStateMatches ? tabularSchemaState?.data : null;
  const schemaStatus = schemaStateMatches ? tabularSchemaState?.status : 'loading';
  const schemaError = schemaStateMatches ? tabularSchemaState?.error : '';
  const dictionary = useMemo(
    () => buildDatasetDictionaryView({ understanding, tabularSchema }),
    [understanding, tabularSchema],
  );
  const fields = dictionary.allFields;
  const documentCount = Number(selectedDataset?.documentCount || 0);
  const latestUpdatedAt = selectedDataset?.latestUpdatedAt || '';

  return (
    <div className="source-dictionary-content">
      <div className="source-dictionary-inline-intro">
        当前数据集：{selectedDataset?.title || selectedDataset?.key || '未命名数据集'}。字段清单以 CSV / TSV 真实表头为准，业务释义与源字段名分开展示。
      </div>

      {selectedDataset ? (
        <div className="source-dictionary-summary">
          <MiniMetric label="资料对象" value={documentCount} />
          <MiniMetric label="结构表" value={dictionary.tableCount || '待识别'} />
          <MiniMetric label="字段列" value={dictionary.fieldCount || '待识别'} />
          <MiniMetric label="唯一字段" value={dictionary.uniqueFieldCount || '待识别'} />
          <MiniMetric label="最近变化" value={latestUpdatedAt ? formatRelativeTime(latestUpdatedAt) : '暂无'} />
        </div>
      ) : null}

      {selectedDataset && schemaStatus === 'loading' && !fields.length ? (
        <div className="directory-empty">正在读取真实表头并生成字段字典。</div>
      ) : null}
      {selectedDataset && schemaStatus === 'failed' ? (
        <div className="database-source-error">
          真实表头暂不可用：{schemaError || '字段结构读取失败'}
          {fields.length ? '；当前显示可核验的结构化回退结果。' : ''}
        </div>
      ) : null}
      {dictionary.businessClueCount ? (
        <div className="source-dictionary-note">
          已识别 {dictionary.businessClueCount} 条 README / 说明文档业务线索；它们继续用于口径理解，但不再冒充结构字段或计入字段数量。
        </div>
      ) : null}
      {dictionary.skippedTabularDocumentCount ? (
        <div className="source-dictionary-note warning">
          {dictionary.skippedTabularDocumentCount} 个表格文档未能在安全对象目录内读取表头，已跳过且未回退读取数据行。
        </div>
      ) : null}
      {dictionary.tables.length ? (
        <div className="source-dictionary-groups">
          {dictionary.tables.map((table, index) => (
            <details className="source-dictionary-group" key={table.id} open={index === 0}>
              <summary className="source-dictionary-group-head">
                <div>
                  <strong>{table.title}</strong>
                  <span>{table.fields.length} 列 · {table.structuralSource === 'file_header' ? '真实文件表头' : '结构化回退'}</span>
                </div>
                {table.updatedAt ? <time>{formatRelativeTime(table.updatedAt)}</time> : null}
              </summary>
              <div className="source-dictionary-table-wrap">
                <table className="source-detail-table source-dictionary-table">
                  <thead>
                    <tr>
                      <th>字段</th>
                      <th>字典说明</th>
                      <th>类型 / 角色</th>
                      <th>质量统计</th>
                    </tr>
                  </thead>
                  <tbody>
                    {table.fields.map((field) => (
                      <tr key={field.id || `${field.object_id}:${field.technical_name}`}>
                        <td>
                          <strong>{field.label || field.technical_name || '待解释字段'}</strong>
                          <code>{field.technical_name || '—'}</code>
                        </td>
                        <td>{dictionaryFieldDescription(field)}</td>
                        <td>
                          <span>{field.value_type || 'unknown'}</span>
                          <small>
                            {semanticRoleLabel(field.semantic_role)} · {field.structure_confirmed
                              ? '表头确认'
                              : `置信 ${dictionaryConfidenceLabel(field.confidence)}`}
                          </small>
                        </td>
                        <td>
                          {field.non_empty_count !== null && field.non_empty_count !== undefined ? (
                            <>
                              <span>非空 {Number(field.non_empty_count).toLocaleString('zh-CN')}</span>
                              <small>去重 {Number(field.distinct_count || 0).toLocaleString('zh-CN')}</small>
                            </>
                          ) : (
                            <span>待统计</span>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </details>
          ))}
        </div>
      ) : selectedDataset && schemaStatus !== 'loading' && schemaStatus !== 'failed' ? (
        <div className="directory-empty">当前数据集没有可公开核验的 CSV / TSV 表头；README、manifest 和说明文字不会被当作字段。</div>
      ) : null}
    </div>
  );
}

function DocumentMembershipEditor({
  datasets,
  document,
  busy,
  onToggleDocumentDatasetMembership,
}) {
  if (!document?.id) {
    return null;
  }
  const activeIds = new Set(documentDatasetIds(document));
  return (
    <section className="directory-membership-editor">
      <div>
        <strong>文档归属数据集</strong>
        <span>当前编辑：{document.title || '当前文档'}。点击数据集即可加入或移出。</span>
      </div>
      <div className="directory-membership-grid">
        {datasets.map((dataset) => {
          const active = activeIds.has(dataset.id);
          return (
            <button
              key={dataset.id}
              type="button"
              className={`directory-membership-chip ${active ? 'active' : ''}`.trim()}
              aria-pressed={active}
              disabled={Boolean(busy)}
              onClick={() => onToggleDocumentDatasetMembership?.(dataset.id)}
            >
              <strong>{dataset.title}</strong>
              <span>{active ? '已加入，点击移出' : '未加入，点击加入'}</span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

function AssetLibraryManager({
  datasets,
  assetLibraries = [],
  selectedAssetLibraryId = '',
  selectedAssetLibrary,
  assetLibraryScope,
  assetLibraryDraft,
  assetImageImportDraft,
  assetImportEnabled = false,
  creatingAssetLibrary,
  importingAssetImage,
  assetLibraryLoading,
  assetLibraryActionBusy,
  selectedDatasetId,
  selectedDatasetIds = [],
  onSelectAssetLibrary,
  onAssetLibraryDraftChange,
  onApplyAssetLibraryPreset,
  onCreateAssetLibrary,
  onAssetImageImportDraftChange,
  onImportFashionDesignImageAsset,
  onImportFashionDesignImageAssetFiles,
  onToggleAssetLibraryDataset,
  onRefreshAssetLibraries,
}) {
  const targetDatasetId = selectedDatasetId || selectedDatasetIds[0] || '';
  const selectedDataset = datasets.find((dataset) => dataset.id === targetDatasetId) || null;
  const selectedDatasetIsInLibrary = targetDatasetId
    ? assetLibraryContainsDataset(assetLibraryScope, targetDatasetId)
    : false;
  const [assetProfileQuery, setAssetProfileQuery] = useState('');
  const [assetProfileKind, setAssetProfileKind] = useState('all');
  const assetProfileKinds = assetProfileKindOptions(assetLibraryScope);
  const parseStatusEntries = assetParseStatusEntries(assetLibraryScope);
  const filteredAssetProfileHints = filterAssetProfileHints(assetLibraryScope, {
    query: assetProfileQuery,
    assetKind: assetProfileKind,
    limit: 8,
  });

  useEffect(() => {
    setAssetProfileQuery('');
    setAssetProfileKind('all');
  }, [selectedAssetLibraryId]);

  return (
    <section className="directory-card asset-library-card">
      <div className="directory-section-head">
        <div>
          <h3>资产库</h3>
          <p>{selectedAssetLibrary ? `${selectedAssetLibrary.name} · ${assetLibraryScope?.datasets?.length || 0} 个可见数据集` : '企业资产空间和数据集关系。'}</p>
        </div>
        <button type="button" className="ghost-btn compact-action-btn" onClick={onRefreshAssetLibraries} disabled={assetLibraryLoading}>
          刷新
        </button>
      </div>

      <form
        className="asset-library-create-row"
        onSubmit={(event) => {
          event.preventDefault();
          onCreateAssetLibrary?.();
        }}
      >
        <input
          value={assetLibraryDraft?.name || ''}
          onChange={(event) => onAssetLibraryDraftChange?.('name', event.target.value)}
          placeholder="资产库名称"
          disabled={creatingAssetLibrary}
        />
        <input
          value={assetLibraryDraft?.domain || ''}
          onChange={(event) => onAssetLibraryDraftChange?.('domain', event.target.value)}
          placeholder="领域，如 fashion_design"
          disabled={creatingAssetLibrary}
        />
        <button className="primary-btn compact-action-btn" type="submit" disabled={creatingAssetLibrary}>
          {creatingAssetLibrary ? '创建中' : '新建'}
        </button>
      </form>

      <div className="asset-library-preset-row">
        {ASSET_LIBRARY_PRESETS.map((preset) => (
          <button
            key={preset.id}
            type="button"
            className="ghost-btn compact-action-btn"
            onClick={() => onApplyAssetLibraryPreset?.(preset.id)}
            disabled={creatingAssetLibrary}
            title={preset.description}
          >
            {preset.label}
          </button>
        ))}
        {assetLibraryDraft?.description ? (
          <span>{assetLibraryDraft.description}</span>
        ) : null}
      </div>

      <div className="asset-library-list">
        {assetLibraries.length ? assetLibraries.map((library) => (
          <button
            key={library.id}
            type="button"
            className={`asset-library-item ${library.id === selectedAssetLibraryId ? 'active' : ''}`.trim()}
            onClick={() => onSelectAssetLibrary?.(library.id)}
          >
            <strong>{library.name}</strong>
            <span>{library.domain} · {library.visibility} · {library.datasetCount} 数据集</span>
          </button>
        )) : (
          <div className="directory-empty">{assetLibraryLoading ? '正在读取资产库。' : '暂无可见资产库。'}</div>
        )}
      </div>

      {selectedAssetLibrary ? (
        <div className="asset-library-scope-box">
          <div className="asset-library-scope-metrics">
            <MiniMetric label="已挂数据集" value={assetLibraryScope?.membershipCount || selectedAssetLibrary.datasetCount || 0} />
            <MiniMetric label="当前可见" value={assetLibraryScope?.authorizedDatasetCount || 0} />
            <MiniMetric label="无权限" value={assetLibraryScope?.deniedDatasetCount || 0} />
            <MiniMetric label="可见资产" value={assetLibraryScope?.assetCount || 0} />
            <MiniMetric label="画像摘要" value={assetLibraryScope?.assetProfileHintCount || 0} />
            <MiniMetric label="解析任务" value={assetLibraryScope?.assetParseRunCount || 0} />
          </div>
          {parseStatusEntries.length ? (
            <div className="asset-library-parse-status-row" aria-label="资产解析状态">
              {parseStatusEntries.map((entry) => (
                <span key={entry.status}>
                  {entry.label}
                  <strong>{entry.count}</strong>
                </span>
              ))}
            </div>
          ) : null}
          {assetLibraryScope?.assetProfileHints?.length ? (
            <div className="asset-profile-gallery">
              <div className="asset-profile-filter-row">
                <input
                  value={assetProfileQuery}
                  onChange={(event) => setAssetProfileQuery(event.target.value)}
                  placeholder="搜索素材、主题、字段"
                />
                <select
                  value={assetProfileKind}
                  onChange={(event) => setAssetProfileKind(event.target.value)}
                >
                  <option value="all">全部类型</option>
                  {assetProfileKinds.map((kind) => (
                    <option key={kind} value={kind}>{kind}</option>
                  ))}
                </select>
              </div>
              {filteredAssetProfileHints.length ? (
                <div className="asset-profile-hint-list">
                  {filteredAssetProfileHints.map((hint) => (
                    <div key={`${hint.assetId || hint.title}:${hint.profileKind}`} className="asset-profile-hint-item">
                      <div>
                        <strong>{hint.title || hint.assetId || '未命名资产'}</strong>
                        <span>{hint.assetKind}{hint.profileKind ? ` · ${hint.profileKind}` : ''}</span>
                      </div>
                      {hint.summary ? <p>{hint.summary}</p> : null}
                      {hint.nounTerms?.length || hint.facets?.length ? (
                        <div className="asset-profile-term-row">
                          {[...(hint.nounTerms || []), ...(hint.facets || [])].slice(0, 6).map((term) => (
                            <span key={term}>{term}</span>
                          ))}
                        </div>
                      ) : null}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="directory-empty compact-empty">当前筛选无匹配画像。</div>
              )}
            </div>
          ) : null}
          {assetImportEnabled ? (
          <form
            className="asset-library-import-form"
            onSubmit={(event) => {
              event.preventDefault();
              onImportFashionDesignImageAsset?.();
            }}
          >
            <div className="asset-library-import-head">
              <strong>导入设计图</strong>
              <span>{selectedDataset ? `写入 ${selectedDataset.title}` : '先选一个目标数据集'}</span>
            </div>
            <div className="asset-library-create-row">
              <input
                value={assetImageImportDraft?.title || ''}
                onChange={(event) => onAssetImageImportDraftChange?.('title', event.target.value)}
                placeholder="图片标题，可留空自动取文件名"
                disabled={importingAssetImage}
              />
              <input
                value={assetImageImportDraft?.externalId || ''}
                onChange={(event) => onAssetImageImportDraftChange?.('externalId', event.target.value)}
                placeholder="外部 ID，可选"
                disabled={importingAssetImage}
              />
              <button
                className="primary-btn compact-action-btn"
                type="submit"
                disabled={importingAssetImage || !selectedDataset}
              >
                {importingAssetImage ? '导入中' : '导入'}
              </button>
            </div>
            <textarea
              className="asset-library-import-url"
              value={assetImageImportDraft?.imageUrl || ''}
              onChange={(event) => onAssetImageImportDraftChange?.('imageUrl', event.target.value)}
              placeholder="https://.../image.png，可每行粘贴一个"
              disabled={importingAssetImage}
            />
            <textarea
              value={assetImageImportDraft?.profilePayloadText || ''}
              onChange={(event) => onAssetImageImportDraftChange?.('profilePayloadText', event.target.value)}
              placeholder='可选画像 JSON，如 {"category":"dress","color":["green"]}'
              disabled={importingAssetImage}
            />
            <div className="asset-library-import-file-row">
              <label className="ghost-btn compact-action-btn">
                选择图片/ZIP
                <input
                  type="file"
                  multiple
                  accept="image/*,.zip,application/zip"
                  disabled={importingAssetImage}
                  onChange={(event) => {
                    onImportFashionDesignImageAssetFiles?.(event.target.files);
                    event.target.value = '';
                  }}
                />
              </label>
              <span>图片直接入库；ZIP 会展开其中图片后入库。</span>
            </div>
          </form>
          ) : null}
          {selectedDataset ? (
            <button
              type="button"
              className={`asset-library-current-toggle ${selectedDatasetIsInLibrary ? 'active' : ''}`.trim()}
              disabled={Boolean(assetLibraryActionBusy)}
              onClick={() => onToggleAssetLibraryDataset?.(selectedDataset.id)}
            >
              <strong>{selectedDataset.title}</strong>
              <span>{selectedDatasetIsInLibrary ? '已在资产库中，点击移出' : '当前数据集，点击加入资产库'}</span>
            </button>
          ) : null}
          <div className="asset-library-dataset-grid">
            {datasets.map((dataset) => {
              const active = assetLibraryContainsDataset(assetLibraryScope, dataset.id);
              return (
                <button
                  key={dataset.id}
                  type="button"
                  className={`directory-membership-chip ${active ? 'active' : ''}`.trim()}
                  aria-pressed={active}
                  disabled={Boolean(assetLibraryActionBusy)}
                  onClick={() => onToggleAssetLibraryDataset?.(dataset.id)}
                >
                  <strong>{dataset.title}</strong>
                  <span>{active ? '已挂接' : '未挂接'}</span>
                </button>
              );
            })}
          </div>
        </div>
      ) : null}
    </section>
  );
}

function databaseSourceAliasId(sourceId) {
  return `database:${sourceId}`;
}

function databaseSyncIsLive(syncReadiness, recentSyncRuns = []) {
  const liveStates = new Set(['sync_running', 'sync_queued', 'running', 'queued', 'processing', 'indexing', 'pending']);
  const signal = String(syncReadiness?.signal || syncReadiness?.latestStatus || '').toLowerCase();
  const latestRunStatus = String(recentSyncRuns[0]?.status || '').toLowerCase();
  return liveStates.has(signal) || liveStates.has(latestRunStatus);
}

function DatabaseTableInspector({
  table,
  profileTable,
}) {
  if (!table) return null;
  const profileColumns = new Map(
    (profileTable?.columns || []).map((column) => [column.name, column]),
  );
  const tableName = table.table || table.name;

  return (
    <section className="database-table-inspector" aria-label={`${tableName} 字段字典`}>
      <div className="database-table-inspector-head">
        <div>
          <strong>{tableName}</strong>
          <span>
            {table.comment || '暂无源表注释'} · 字段 {table.columnCount}
            {table.approximateRowCount ? ` · 预估行 ${table.approximateRowCount}` : ''}
            {table.updateTime ? ` · 更新 ${formatRelativeTime(table.updateTime)}` : ''}
          </span>
        </div>
        <span className="database-table-access-note">源库行明细需在具备访问控制的环境中开放</span>
      </div>

      <div className="source-dictionary-table-wrap">
        <table className="source-detail-table database-dictionary-table">
          <thead>
            <tr>
              <th>字段</th>
              <th>源库注释</th>
              <th>数据类型</th>
              <th>约束</th>
              <th>系统语义</th>
              <th>样本统计</th>
            </tr>
          </thead>
          <tbody>
            {table.columns.map((column) => {
              const semantic = profileColumns.get(column.name);
              return (
                <tr key={column.name}>
                  <td><code>{column.name}</code></td>
                  <td>{column.comment || '—'}</td>
                  <td>{column.columnType || column.dataType || 'unknown'}</td>
                  <td>
                    <span>{column.primaryKey ? '主键' : column.indexed ? '索引' : '普通字段'}</span>
                    <small>{column.nullable ? '可为空' : '必填'}{column.defaultValue !== null ? ` · 默认 ${column.defaultValue}` : ''}</small>
                  </td>
                  <td>
                    <span>{semantic ? semanticRoleLabel(semantic.semanticRole) : '待读取画像'}</span>
                    <small>{semantic ? `置信 ${semantic.roleConfidence}` : '源库定义与系统推断分开'}</small>
                  </td>
                  <td>
                    <span>{semantic ? `去重 ${semantic.distinctSampleCount}` : '—'}</span>
                    <small>{semantic ? `空值 ${semantic.nullSampleCount}` : '点击“语义画像”补充'}</small>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

    </section>
  );
}

function DatabaseSourcePanel({
  datasets,
  aliases,
  editingSourceId,
  aliasDraft,
  onAliasDraftChange,
  onStartEditing,
  onSave,
  onCancel,
  onReset,
}) {
  const [sources, setSources] = useState([]);
  const [selectedSourceId, setSelectedSourceId] = useState('');
  const [selectedDatasetId, setSelectedDatasetId] = useState('');
  const [syncTargetMode, setSyncTargetMode] = useState('existing');
  const [targetExternalId, setTargetExternalId] = useState('');
  const [targetExternalTitle, setTargetExternalTitle] = useState('');
  const [status, setStatus] = useState(null);
  const [schema, setSchema] = useState(null);
  const [profile, setProfile] = useState(null);
  const [selectedTableName, setSelectedTableName] = useState('');
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

  async function loadStatus(sourceId = selectedSourceId, { silent = false } = {}) {
    if (!sourceId) {
      setStatus(null);
      return;
    }
    if (!silent) setActionBusy('status');
    try {
      setStatus(await fetchDatabaseSourceStatus(sourceId));
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : '数据库源状态读取失败');
    } finally {
      if (!silent) setActionBusy('');
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
        setSelectedTableName((current) => (
          current && nextSchema.tables.some((table) => (table.table || table.name) === current)
            ? current
            : nextSchema.tables[0]?.table || nextSchema.tables[0]?.name || ''
        ));
        setNotice(`结构已读取 · ${nextSchema.tableCount} 张表`);
      } else if (kind === 'profile') {
        const nextProfile = await profileDatabaseSource(selectedSourceId);
        setProfile(nextProfile);
        setSelectedTableName((current) => current || nextProfile.tables[0]?.table || '');
        setNotice(`语义画像已更新 · 指标 ${nextProfile.metricCount} · 维度 ${nextProfile.dimensionCount}`);
      } else if (kind === 'applyProfile') {
        const nextProfile = await applyDatabaseSourceProfile(selectedSourceId);
        setProfile(nextProfile);
        setNotice(`语义画像已应用 · 表 ${nextProfile.tableCount} · 指标 ${nextProfile.metricCount} · 维度 ${nextProfile.dimensionCount}`);
        await loadStatus(selectedSourceId);
        await loadSources();
      } else if (kind === 'full' || kind === 'incremental') {
        const datasetId = selectedDatasetId || selectedSource?.source?.defaultDatasetId || '';
        const datasetExternalId = targetExternalId.trim();
        const syncTarget = syncTargetMode === 'external'
          ? {
            datasetExternalId,
            datasetTitle: targetExternalTitle.trim() || selectedSource?.source?.database || selectedSource?.displayName || '',
          }
          : datasetId;
        if (syncTargetMode === 'external' && !datasetExternalId) {
          throw new Error('请填写目标分组 ID');
        }
        if (syncTargetMode === 'existing' && !datasetId) {
          throw new Error('请先选择目标数据集');
        }
        const result = await startDatabaseSourceSync(selectedSourceId, kind, syncTarget);
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
      setSelectedTableName('');
      loadStatus(selectedSourceId);
    }
  }, [selectedSourceId]);

  useEffect(() => {
    const defaultDatasetId = selectedSource?.source?.defaultDatasetId || '';
    setSelectedDatasetId(defaultDatasetId || datasets[0]?.id || '');
    if (!defaultDatasetId && !datasets.length) {
      setSyncTargetMode('external');
    }
  }, [selectedSourceId, datasets.map((dataset) => dataset.id).join('|')]);

  const datasetReadiness = status?.datasetReadiness || null;
  const syncReadiness = status?.syncReadiness || null;
  const tableReadiness = status?.tableReadiness || [];
  const recentSyncRuns = status?.recentSyncRuns || [];
  const liveSync = databaseSyncIsLive(syncReadiness, recentSyncRuns);
  const busy = Boolean(actionBusy);
  const canSync = syncTargetMode === 'external'
    ? Boolean(targetExternalId.trim())
    : Boolean(selectedDatasetId);
  const selectedSchemaTable = schema?.tables?.find((table) => (
    (table.table || table.name) === selectedTableName
  )) || null;
  const selectedProfileTable = profile?.tables?.find((table) => table.table === selectedTableName) || null;

  useEffect(() => {
    if (!selectedSourceId || !liveSync) return undefined;
    const intervalId = window.setInterval(() => {
      loadStatus(selectedSourceId, { silent: true });
    }, 5000);
    return () => window.clearInterval(intervalId);
  }, [selectedSourceId, liveSync]);

  return (
    <section className="directory-card database-source-card">
      <div className="directory-section-head">
        <div>
          <h3>数据库源</h3>
          <p>数据库先同步到目标数据集；运行中的同步每 5 秒抓取一次真实状态。</p>
        </div>
        <button type="button" className="ghost-btn compact-action-btn" onClick={loadSources} disabled={loading || busy}>
          {loading ? '刷新中' : '刷新'}
        </button>
      </div>

      {sources.length ? (
        <>
          {selectedSource ? (
            <div className="database-source-display-name">
              <SourceDisplayNameEditor
                sourceId={databaseSourceAliasId(selectedSource.id)}
                defaultName={selectedSource.displayName}
                aliases={aliases}
                editingSourceId={editingSourceId}
                aliasDraft={aliasDraft}
                onAliasDraftChange={onAliasDraftChange}
                onStartEditing={onStartEditing}
                onSave={onSave}
                onCancel={onCancel}
                onReset={onReset}
              />
              {liveSync ? <span className="database-live-sync"><span className="source-live-dot loading" />同步进行中</span> : null}
            </div>
          ) : null}
          <div className="database-source-controls">
            <label>
              <span>连接</span>
              <select value={selectedSourceId} onChange={(event) => setSelectedSourceId(event.target.value)} disabled={busy}>
                {sources.map((source) => (
                  <option key={source.id} value={source.id}>
                    {sourceDisplayName({ id: databaseSourceAliasId(source.id), title: source.displayName }, aliases)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>目标方式</span>
              <select value={syncTargetMode} onChange={(event) => setSyncTargetMode(event.target.value)} disabled={busy}>
                <option value="existing">已有数据集</option>
                <option value="external">外部分组 ID</option>
              </select>
            </label>
            {syncTargetMode === 'existing' ? (
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
            ) : (
              <>
                <label>
                  <span>目标分组 ID</span>
                  <input
                    value={targetExternalId}
                    onChange={(event) => setTargetExternalId(event.target.value)}
                    placeholder="workspace-db-main"
                    disabled={busy}
                  />
                </label>
                <label>
                  <span>数据集名称</span>
                  <input
                    value={targetExternalTitle}
                    onChange={(event) => setTargetExternalTitle(event.target.value)}
                    placeholder={selectedSource?.source?.database || selectedSource?.displayName || '数据库数据集'}
                    disabled={busy}
                  />
                </label>
              </>
            )}
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
            <button type="button" className="ghost-btn compact-action-btn" onClick={() => runAction('applyProfile')} disabled={busy}>
              {actionBusy === 'applyProfile' ? '应用中' : '应用画像'}
            </button>
            <button type="button" className="primary-btn compact-action-btn" onClick={() => runAction('incremental')} disabled={busy || !canSync}>
              {actionBusy === 'incremental' ? '提交中' : '增量同步'}
            </button>
            <button type="button" className="ghost-btn compact-action-btn" onClick={() => runAction('full')} disabled={busy || !canSync}>
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
            <div className="database-schema-browser">
              <div className="database-schema-tabs" aria-label="数据库表选择">
                {schema.tables.slice(0, 12).map((table) => {
                  const tableName = table.table || table.name;
                  return (
                    <button
                      key={tableName}
                      type="button"
                      className={tableName === selectedTableName ? 'active' : ''}
                      onClick={() => setSelectedTableName(tableName)}
                    >
                      <strong>{tableName}</strong>
                      <span>字段 {table.columnCount} · 预估行 {table.approximateRowCount}</span>
                    </button>
                  );
                })}
              </div>
              <DatabaseTableInspector
                table={selectedSchemaTable}
                profileTable={selectedProfileTable}
              />
            </div>
          ) : null}

          {profile?.tables?.length && !schema?.tables?.length ? (
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
  datasetUnderstandingState,
  documents,
  documentsLoading,
  documentSearch,
  onDocumentSearchChange,
  selectedDocumentId,
  onFocusDocumentMembership,
  onClearDocumentSelection,
  onOpenDocumentPage,
  onRefreshDocuments,
  onArchiveDocuments,
  documentActionBusy,
  onToggleDocumentDatasetMembership,
}) {
  const selectedIdSet = new Set(selectedDatasetIds.length ? selectedDatasetIds : selectedDatasetId ? [selectedDatasetId] : []);
  const selectedDataset = datasets.find((dataset) => dataset.id === selectedDatasetId) || null;
  const filteredDocuments = documents.filter((document) => {
    const inDataset = !selectedIdSet.size || documentDatasetIds(document).some((datasetId) => selectedIdSet.has(datasetId));
    const query = documentSearch.trim().toLowerCase();
    const matches = !query
      || String(document.title || '').toLowerCase().includes(query)
      || String(document.object_key || '').toLowerCase().includes(query)
      || String(document.content_type || '').toLowerCase().includes(query);
    return inDataset && matches;
  });
  const [selectedDocumentIds, setSelectedDocumentIds] = useState([]);
  const selectedDocument = documents.find((document) => document.id === selectedDocumentId) || null;

  useEffect(() => {
    const visibleIds = new Set(filteredDocuments.map((document) => document.id));
    setSelectedDocumentIds((current) => current.filter((id) => visibleIds.has(id)));
  }, [filteredDocuments.map((document) => document.id).join('|')]);

  const toggleDocumentSelection = (documentId) => {
    const alreadySelected = selectedDocumentIds.includes(documentId);
    const next = alreadySelected
      ? selectedDocumentIds.filter((id) => id !== documentId)
      : [...selectedDocumentIds, documentId];
    setSelectedDocumentIds(next);
    if (alreadySelected && documentId === selectedDocumentId) {
      const fallbackId = next[next.length - 1] || '';
      if (fallbackId) {
        onFocusDocumentMembership?.(fallbackId);
      } else {
        onClearDocumentSelection?.();
      }
      return;
    }
    if (!alreadySelected) {
      onFocusDocumentMembership?.(documentId);
    }
  };

  return (
    <div className="dataset-page-stack">
      <DatasetUnderstandingGraph
        dataset={selectedDataset}
        documents={documents}
        understandingState={datasetUnderstandingState}
      />

      <section className="directory-card dataset-page-documents-card">
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
          <span className="directory-batch-hint">
            {selectedDocument ? '下方可直接编辑当前文档归属' : '勾选文档后可编辑归属'}
          </span>
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
        <DocumentMembershipEditor
          datasets={datasets}
          document={selectedDocument}
          busy={documentActionBusy}
          onToggleDocumentDatasetMembership={onToggleDocumentDatasetMembership}
        />
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
                <span>{documentDatasetTitles(document, datasets)} · {documentKind(document.content_type)} · {formatSnakeCaseLabel(document.lifecycle)}</span>
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
  onToggleDocumentDatasetMembership,
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
  const tabularSourceText = orderedChunks
    .map((chunk) => String(chunk?.content || '').trim())
    .filter(Boolean)
    .join('\n');
  const tabularPreview = useMemo(
    () => buildTabularDocumentPreview(selectedDocument || {}, tabularSourceText || rawText, { maxRows: 20, maxColumns: 40 }),
    [
      selectedDocument?.id,
      selectedDocument?.title,
      selectedDocument?.content_type,
      selectedDocument?.contentType,
      selectedDocument?.object_key,
      selectedDocument?.objectKey,
      tabularSourceText,
      rawText,
    ],
  );
  const hideTabularDocumentText = isTabularDocument(selectedDocument || {});
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
                <p>{documentDatasetTitles(selectedDocument, datasets)} · {documentKind(selectedDocument.content_type)}</p>
              </div>
            </div>
            <div className="document-detail-meta-grid">
              <MiniMetric label="切片" value={orderedChunks.length} />
              <MiniMetric label="证据" value={evidences.length} />
              <MiniMetric label="状态" value={formatSnakeCaseLabel(selectedDocument.lifecycle)} />
            </div>
            {tabularPreview ? (
              <div className="document-table-preview">
                <div className="document-block-title">
                  <strong>表格明细预览</strong>
                  <span>
                    字段 {tabularPreview.columns.length} · 展示前 {tabularPreview.rows.length} 行
                    {tabularPreview.hasSensitiveData ? ' · 敏感值已隐藏' : ''}
                  </span>
                </div>
                <div className="source-dictionary-table-wrap">
                  <table className="source-detail-table document-preview-table">
                    <thead>
                      <tr>{tabularPreview.columns.map((column) => <th key={column}>{column}</th>)}</tr>
                    </thead>
                    <tbody>
                      {tabularPreview.rows.map((row, rowIndex) => (
                        <tr key={`document-row:${rowIndex}`}>
                          {tabularPreview.columns.map((column, columnIndex) => (
                            <td key={`${rowIndex}:${column}`} title={row[columnIndex] || '—'}>
                              {truncateText(row[columnIndex] || '—', 160)}
                            </td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            ) : null}
            <div className="document-original-block">
              <div className="document-block-title">
                <strong>Markdown 原文</strong>
                <span>{rawText ? `${rawText.length} 字符 · 可下滑查看全文` : '暂无可展示原文'}</span>
              </div>
              {hideTabularDocumentText ? (
                <div className="document-sensitive-guard">
                  公开页面仅展示经过字段级安全检查的表格样例，不展开表格原始全文；身份标识、联系方式和凭证形态值会自动隐藏。
                </div>
              ) : rawText ? (
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

            <DocumentMembershipEditor
              datasets={datasets}
              document={selectedDocument}
              busy={documentActionBusy}
              onToggleDocumentDatasetMembership={onToggleDocumentDatasetMembership}
            />

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
              {hideTabularDocumentText ? (
                <div className="document-sensitive-guard">表格解析切片可能包含后续行原值，已在公开页面隐藏。</div>
              ) : orderedChunks.length ? orderedChunks.map((chunk) => {
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
              {hideTabularDocumentText ? (
                <div className="document-sensitive-guard">表格检索证据可能包含原始值，已在公开页面隐藏。</div>
              ) : evidences.length ? evidences.map((evidence) => (
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

function SourcesPage({
  documents,
  datasets,
  datasetUnderstandingState,
  documentsLoading = false,
  onRefreshDocuments,
  onOpenDocumentPage,
}) {
  const [sourceAliases, setSourceAliases] = useState({});
  const [editingSourceId, setEditingSourceId] = useState('');
  const [aliasDraft, setAliasDraft] = useState('');
  const [lastCheckedAt, setLastCheckedAt] = useState('');
  const refreshDocumentsRef = useRef(onRefreshDocuments);
  const documentsLoadingRef = useRef(documentsLoading);
  const refreshInFlightRef = useRef(false);
  const sources = buildConnectedSourceCards(datasets, documents).filter((source) => source.documentCount > 0);
  const uniqueConnectedDocuments = connectedDocumentCount(datasets, documents);
  const groups = sourceGroups(documents);

  useEffect(() => {
    setSourceAliases(readSourceDisplayAliases());
  }, []);

  useEffect(() => {
    refreshDocumentsRef.current = onRefreshDocuments;
  }, [onRefreshDocuments]);

  useEffect(() => {
    documentsLoadingRef.current = documentsLoading;
    if (!documentsLoading) setLastCheckedAt(new Date().toISOString());
  }, [documentsLoading]);

  async function refreshSourceChanges() {
    if (documentsLoadingRef.current || refreshInFlightRef.current || !refreshDocumentsRef.current) return;
    refreshInFlightRef.current = true;
    try {
      await refreshDocumentsRef.current();
      setLastCheckedAt(new Date().toISOString());
    } finally {
      refreshInFlightRef.current = false;
    }
  }

  const refreshSourceChangesRef = useRef(refreshSourceChanges);
  refreshSourceChangesRef.current = refreshSourceChanges;

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      if (document.visibilityState === 'visible') refreshSourceChangesRef.current();
    }, 60000);
    return () => window.clearInterval(intervalId);
  }, []);

  function startEditingSourceAlias(sourceId, displayName) {
    setEditingSourceId(sourceId);
    setAliasDraft(displayName);
  }

  function saveSourceAlias(sourceId, defaultName) {
    const displayName = aliasDraft.trim();
    const nextAliases = { ...sourceAliases };
    if (!displayName || displayName === defaultName) {
      delete nextAliases[sourceId];
    } else {
      nextAliases[sourceId] = displayName;
    }
    setSourceAliases(nextAliases);
    writeSourceDisplayAliases(nextAliases);
    setEditingSourceId('');
    setAliasDraft('');
  }

  function resetSourceAlias(sourceId) {
    const nextAliases = { ...sourceAliases };
    delete nextAliases[sourceId];
    setSourceAliases(nextAliases);
    writeSourceDisplayAliases(nextAliases);
  }

  const aliasEditorProps = {
    aliases: sourceAliases,
    editingSourceId,
    aliasDraft,
    onAliasDraftChange: setAliasDraft,
    onStartEditing: startEditingSourceAlias,
    onSave: saveSourceAlias,
    onCancel: () => {
      setEditingSourceId('');
      setAliasDraft('');
    },
    onReset: resetSourceAlias,
  };
  return (
    <div className="source-workspace-layout">
      <div className="source-workspace-main">
        <SourceLiveMonitor
          sources={sources}
          understandingState={datasetUnderstandingState}
          uniqueConnectedDocumentCount={uniqueConnectedDocuments}
          loading={documentsLoading}
          lastCheckedAt={lastCheckedAt}
          onRefresh={refreshSourceChanges}
          {...aliasEditorProps}
        />
        <ConnectedObjectDirectory
          groups={groups}
          datasets={datasets}
          loading={documentsLoading}
          onOpenDocumentPage={onOpenDocumentPage}
        />
        <DatabaseSourcePanel datasets={datasets} {...aliasEditorProps} />
      </div>
      <AccessMethodGuide documents={documents} datasets={datasets} />
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
  assetLibraries,
  selectedAssetLibraryId,
  selectedAssetLibrary,
  assetLibraryScope,
  assetLibraryDraft,
  assetImageImportDraft,
  assetImportEnabled = false,
  creatingAssetLibrary,
  importingAssetImage,
  assetLibraryLoading,
  assetLibraryActionBusy,
  onSelectAssetLibrary,
  onAssetLibraryDraftChange,
  onApplyAssetLibraryPreset,
  onCreateAssetLibrary,
  onAssetImageImportDraftChange,
  onImportFashionDesignImageAsset,
  onImportFashionDesignImageAssetFiles,
  onToggleAssetLibraryDataset,
  onRefreshAssetLibraries,
  selectedDatasetId,
  selectedDatasetIds = [],
  datasetUnderstandingState,
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
  onFocusDocumentMembership,
  onClearDocumentSelection,
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
  onToggleDocumentDatasetMembership,
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
          assetLibraries={assetLibraries}
          selectedAssetLibraryId={selectedAssetLibraryId}
          selectedAssetLibrary={selectedAssetLibrary}
          assetLibraryScope={assetLibraryScope}
          assetLibraryDraft={assetLibraryDraft}
          assetImageImportDraft={assetImageImportDraft}
          assetImportEnabled={assetImportEnabled}
          creatingAssetLibrary={creatingAssetLibrary}
          importingAssetImage={importingAssetImage}
          assetLibraryLoading={assetLibraryLoading}
          assetLibraryActionBusy={assetLibraryActionBusy}
          onSelectAssetLibrary={onSelectAssetLibrary}
          onAssetLibraryDraftChange={onAssetLibraryDraftChange}
          onApplyAssetLibraryPreset={onApplyAssetLibraryPreset}
          onCreateAssetLibrary={onCreateAssetLibrary}
          onAssetImageImportDraftChange={onAssetImageImportDraftChange}
          onImportFashionDesignImageAsset={onImportFashionDesignImageAsset}
          onImportFashionDesignImageAssetFiles={onImportFashionDesignImageAssetFiles}
          onToggleAssetLibraryDataset={onToggleAssetLibraryDataset}
          onRefreshAssetLibraries={onRefreshAssetLibraries}
          selectedDatasetId={selectedDatasetId}
          selectedDatasetIds={selectedDatasetIds}
          datasetUnderstandingState={datasetUnderstandingState}
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
          onFocusDocumentMembership={onFocusDocumentMembership}
          onClearDocumentSelection={onClearDocumentSelection}
          onOpenDocumentPage={onOpenDocumentPage}
          onRefreshDocuments={onRefreshDocuments}
          onUpdateDataset={onUpdateDataset}
          onArchiveDataset={onArchiveDataset}
          datasetActionBusy={datasetActionBusy}
          onArchiveDocuments={onArchiveDocuments}
          documentActionBusy={documentActionBusy}
          onToggleDocumentDatasetMembership={onToggleDocumentDatasetMembership}
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
          onToggleDocumentDatasetMembership={onToggleDocumentDatasetMembership}
        />
      ) : null}
      {activePage === 'sources' ? (
        <SourcesPage
          documents={documents}
          datasets={datasets}
          datasetUnderstandingState={datasetUnderstandingState}
          documentsLoading={documentsLoading}
          onRefreshDocuments={onRefreshDocuments}
          onOpenDocumentPage={onOpenDocumentPage}
        />
      ) : null}
      {activePage === 'members' ? <MembersPage accountStatusSummary={accountStatusSummary} /> : null}
      {activePage === 'audit' ? <AuditPage stats={stats} activityEvents={activityEvents} htmlArtifacts={htmlArtifacts} /> : null}
      {activePage === 'model-pool' ? <ModelPoolPanel accountStatusSummary={accountStatusSummary} /> : null}
    </section>
  );
}
