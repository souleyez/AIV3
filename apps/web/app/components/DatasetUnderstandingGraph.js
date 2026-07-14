'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import {
  buildDatasetUnderstandingGraph,
  DATASET_GRAPH_CATEGORIES,
  filterDatasetUnderstandingGraph,
} from '../lib/dataset-understanding-graph';

function compactNumber(value) {
  const number = Number(value) || 0;
  if (number >= 10000) return `${(number / 10000).toFixed(number >= 100000 ? 0 : 1)} 万`;
  return number.toLocaleString('zh-CN');
}

function graphCategory(model, key) {
  return model.categories.find((category) => category.key === key) || model.categories[0];
}

function relationClassLabel(value) {
  return {
    confirmed: '已确认',
    observed: '已观察',
    inferred: '推断',
  }[value] || '关系';
}

function sourceKindLabel(value) {
  return {
    database: '数据库',
    database_table: '数据库表',
    spreadsheet: '表格',
    spreadsheet_table: '工作表',
    document: '文档',
    document_section: '文档结构',
    asset: '资产',
    asset_profile: '资产画像',
    media: '音视频',
    media_segment: '媒体片段',
    web_api: '网页 / API',
    api_resource: 'API 资源',
  }[value] || value || '未返回';
}

function semanticRoleLabel(value) {
  return {
    identifier: '标识符',
    name: '名称',
    date: '日期',
    amount: '金额',
    quantity: '数量',
    category: '分类',
    status: '状态',
    location: '位置',
    text: '文本',
    unknown: '待判断',
  }[value] || value || '待判断';
}

function optionForModel(model, filters) {
  const { nodes: visibleNodes, links: visibleLinks } = filterDatasetUnderstandingGraph(model, filters);

  return {
    animationDuration: 650,
    animationDurationUpdate: 420,
    backgroundColor: 'transparent',
    tooltip: {
      trigger: 'item',
      renderMode: 'richText',
      backgroundColor: 'rgba(7, 14, 24, 0.94)',
      borderColor: 'rgba(148, 163, 184, 0.2)',
      textStyle: { color: '#e5edf8', fontSize: 12 },
      formatter(params) {
        if (params.dataType === 'edge') {
          const relationType = relationClassLabel(params.data?.type);
          const confidence = Math.round((Number(params.data?.confidence) || 0) * 100);
          return [
            params.data?.relation || '关联',
            `${relationType} · 置信 ${confidence}%`,
            params.data?.evidence,
          ].filter(Boolean).join('\n');
        }
        return [params.data?.name, params.data?.detail].filter(Boolean).join('\n');
      },
    },
    series: [{
      type: 'graph',
      layout: 'force',
      roam: true,
      draggable: true,
      cursor: 'grab',
      categories: model.categories.map((category) => ({
        name: category.name,
        itemStyle: { color: category.color },
      })),
      data: visibleNodes.map((node) => ({
        ...node,
        label: {
          show: true,
          fontSize: node.kind === 'dataset' ? 11 : 9,
          fontWeight: node.kind === 'dataset' ? 800 : 650,
          color: node.signal === 'identifier' ? '#8794a8' : '#dce6f4',
        },
        itemStyle: {
          color: graphCategory(model, node.kind).color,
          opacity: node.signal === 'identifier' ? 0.62 : 1,
          borderColor: node.kind === 'dataset' ? '#ffffff' : 'rgba(255,255,255,0.52)',
          borderWidth: node.kind === 'dataset' ? 2 : 1,
          shadowBlur: node.kind === 'dataset' ? 28 : 12,
          shadowColor: `${graphCategory(model, node.kind).color}55`,
        },
      })),
      links: visibleLinks.map((link) => ({
        ...link,
        symbol: model.mode === 'semantic' && !link.rootRelation ? ['none', 'arrow'] : ['none', 'none'],
        symbolSize: 7,
        lineStyle: {
          color: link.rootRelation
            ? 'rgba(148, 163, 184, 0.32)'
            : link.type === 'inferred'
              ? 'rgba(251, 191, 36, 0.78)'
              : link.type === 'confirmed'
                ? 'rgba(52, 211, 153, 0.94)'
                : 'rgba(94, 234, 212, 0.84)',
          type: link.type === 'inferred' ? 'dashed' : 'solid',
          width: link.rootRelation ? 0.75 : link.type === 'inferred' ? 1.35 : 1.8,
          opacity: link.rootRelation ? 0.24 : link.type === 'inferred' ? 0.62 : 0.76,
          curveness: link.rootRelation ? 0.02 : 0.12,
        },
      })),
      force: {
        repulsion: model.mode === 'semantic' ? 235 : 182,
        gravity: model.mode === 'semantic' ? 0.055 : 0.085,
        edgeLength: model.mode === 'semantic' ? [74, 148] : [58, 126],
        friction: 0.24,
      },
      label: {
        show: true,
        position: 'right',
        distance: 5,
        color: '#dce6f4',
        fontSize: 9,
        textBorderColor: 'rgba(2, 8, 18, 0.92)',
        textBorderWidth: 3,
        formatter(params) {
          return String(params.data?.shortLabel || '').slice(0, 5);
        },
      },
      labelLayout: { hideOverlap: false },
      edgeLabel: {
        show: false,
        color: '#e5edf8',
        fontSize: 9,
        backgroundColor: 'rgba(2, 8, 18, 0.88)',
        padding: [3, 5],
        borderRadius: 6,
        formatter(params) {
          return params.data?.relation || '关联';
        },
      },
      emphasis: {
        focus: 'adjacency',
        lineStyle: { width: 3, opacity: 1 },
        label: { color: '#ffffff', fontWeight: 800 },
        edgeLabel: { show: true },
      },
    }],
  };
}

function PipelineStage({ stage, index, last, active, onSelect }) {
  return (
    <button
      type="button"
      className={`dataset-understanding-stage ${stage.status} ${active ? 'active' : ''}`.trim()}
      aria-pressed={active}
      onClick={onSelect}
    >
      <div className="dataset-understanding-stage-index">{String(index + 1).padStart(2, '0')}</div>
      <div>
        <span>{stage.label}</span>
        <strong>{stage.value}</strong>
        <small>{stage.detail}</small>
      </div>
      {!last ? <i aria-hidden="true">→</i> : null}
    </button>
  );
}

function UnderstandingOverview({ model }) {
  const { understanding } = model;
  const hiddenQualityEntries = Object.entries(model.fallbackQuality?.hiddenByClass || {});
  const qualityClassLabels = {
    document: '资料标题',
    technical: '技术标识',
    row: '原始数据行',
    sql: 'SQL / 注释',
    numeric: '数字编号',
    unknown: '未分类线索',
  };
  return (
    <>
      <span>{model.mode === 'semantic' ? '系统理解摘要' : '资料来源摘要'}</span>
      <div className={`dataset-understanding-node-kind overview ${model.mode}`.trim()}>
        <i />{model.viewLabel || (model.mode === 'semantic' ? '真实语义快照' : '资料来源图')}
      </div>
      <h4>{model.overviewTitle || (model.mode === 'semantic' ? '系统已经理解到什么' : '当前资料来源包含什么')}</h4>
      <p>{understanding.summary}</p>
      <div className="dataset-understanding-insight-group">
        <strong>{model.mode === 'semantic' ? '核心概念' : '可信中文线索'}</strong>
        <div className="dataset-understanding-insight-tags">
          {understanding.keyConcepts.length
            ? understanding.keyConcepts.slice(0, 10).map((term) => <span key={term}>{term}</span>)
            : <small>接口暂无包含明确中文语义的知识词。</small>}
        </div>
      </div>
      <div className="dataset-understanding-insight-group">
        <strong>结构主线</strong>
        <p>{understanding.structurePath.length ? understanding.structurePath.join(' → ') : '接口暂无章节或结构线索。'}</p>
      </div>
      {understanding.technicalIdentifiers.length ? (
        <div className="dataset-understanding-insight-group muted">
          <strong>技术标识（仅详情）</strong>
          <p>{understanding.technicalIdentifiers.join(' · ')}</p>
        </div>
      ) : null}
      {model.mode === 'fallback' && model.fallbackQuality?.hiddenCount ? (
        <div className="dataset-understanding-insight-group muted">
          <strong>待解释清单</strong>
          <p>
            已从主画布隐藏 {model.fallbackQuality.hiddenCount} 个低质量或技术项
            {model.fallbackQuality.deduplicatedDocumentTitles
              ? `，并合并 ${model.fallbackQuality.deduplicatedDocumentTitles} 个重复资料标题`
              : ''}。
          </p>
          <div className="dataset-understanding-insight-tags">
            {hiddenQualityEntries.map(([qualityClass, count]) => (
              <span key={qualityClass}>{qualityClassLabels[qualityClass] || '待解释'} {count}</span>
            ))}
          </div>
          <small>原始数据行、SQL、路径、编号和 hash 不在界面回显；安全的技术原名只保留在阶段详情中。</small>
        </div>
      ) : null}
      {model.limitations?.length ? (
        <div className="dataset-understanding-insight-group muted">
          <strong>理解边界</strong>
          <p>{model.limitations.join('；')}</p>
        </div>
      ) : null}
      <small className="dataset-understanding-honesty-note">
        {model.mode === 'semantic'
          ? '摘要、对象、字段和关系均来自版本化语义快照；推断关系不等同于已确认事实。'
          : '资料来源图只归纳通过质量门禁的摘要字段和可见资料；语义快照生成前不声称系统已经理解业务。'}
      </small>
    </>
  );
}

function ConnectionGroup({ title, items, model, onSelectLink, direction }) {
  return (
    <div className="dataset-understanding-insight-group connections">
      <strong>{title}</strong>
      <div className="dataset-understanding-connection-list">
        {items.length ? items.map(({ link, node }) => (
          <button key={`${direction}:${link.id}`} type="button" onClick={() => onSelectLink(link.id)}>
            <i className={link.type} />
            <span>
              <strong>{direction === 'incoming' ? `${node?.name || '关联节点'} → ` : ''}{link.relation}{direction === 'outgoing' ? ` → ${node?.name || '关联节点'}` : ''}</strong>
              <small>{relationClassLabel(link.type)} · {Math.round(link.confidence * 100)}% · {link.evidence}</small>
            </span>
          </button>
        )) : <small>当前没有这类直接关系。</small>}
      </div>
    </div>
  );
}

function NodeInspector({ model, node, onSelectLink }) {
  if (!node) return null;
  const incoming = model.links
    .filter((link) => link.target === node.id)
    .map((link) => ({ link, node: model.nodes.find((candidate) => candidate.id === link.source) || null }));
  const outgoing = model.links
    .filter((link) => link.source === node.id)
    .map((link) => ({ link, node: model.nodes.find((candidate) => candidate.id === link.target) || null }));
  const objectFields = node.entityType === 'object'
    ? model.nodes.filter((candidate) => candidate.objectId === node.id)
    : [];
  const parentObject = node.entityType === 'field'
    ? model.nodes.find((candidate) => candidate.id === node.objectId) || null
    : null;
  const nonEmptyRate = parentObject?.coverageCount
    ? Math.min(100, Math.round((node.nonEmptyCount / parentObject.coverageCount) * 100))
    : null;
  const objectExamples = [...new Set(objectFields.flatMap((field) => field.examples || []))].slice(0, 5);

  return (
    <>
      <span>{node.entityType === 'field' ? '字段理解详情' : node.entityType === 'object' ? '业务对象详情' : '当前节点证据'}</span>
      <div className="dataset-understanding-node-kind">
        <i style={{ background: graphCategory(model, node.kind).color }} />
        {graphCategory(model, node.kind).name}
      </div>
      <h4>{node.name}</h4>
      <p>{node.detail || '点击图谱节点查看系统为什么展示这条信息。'}</p>
      {node.entityType === 'object' ? (
        <dl>
          {node.rawLabel && node.rawLabel !== node.name ? <div><dt>原始标签</dt><dd>{node.rawLabel}</dd></div> : null}
          <div><dt>技术来源</dt><dd>{node.technicalName || '未返回'}</dd></div>
          <div><dt>来源类型</dt><dd>{sourceKindLabel(node.sourceKind)}{node.groupLabel ? ` · ${sourceKindLabel(node.groupLabel)}` : ''}</dd></div>
          <div><dt>覆盖量</dt><dd>{compactNumber(node.coverageCount)} 条</dd></div>
          <div><dt>业务标签</dt><dd>{node.labelSource || '未返回'} · {node.status}</dd></div>
          <div><dt>可信度</dt><dd>{Math.round((node.confidence || 0) * 100)}%</dd></div>
          <div><dt>证据来源</dt><dd>{node.evidence || '语义快照'}</dd></div>
        </dl>
      ) : node.entityType === 'field' ? (
        <dl>
          {node.rawLabel && node.rawLabel !== node.name ? <div><dt>原始标签</dt><dd>{node.rawLabel}</dd></div> : null}
          <div><dt>原始字段名</dt><dd>{node.technicalName || '未返回'}</dd></div>
          <div><dt>类型 / 角色</dt><dd>{node.valueType || '未知'} · {semanticRoleLabel(node.semanticRole)}</dd></div>
          <div><dt>非空率</dt><dd>{nonEmptyRate === null ? '分母未返回' : `${nonEmptyRate}%`}（{compactNumber(node.nonEmptyCount)} 条）</dd></div>
          <div><dt>去重数</dt><dd>{compactNumber(node.distinctCount)}</dd></div>
          <div><dt>标签来源</dt><dd>{node.labelSource || '未返回'} · {node.status}</dd></div>
          <div><dt>可信度</dt><dd>{Math.round((node.confidence || 0) * 100)}%</dd></div>
          <div><dt>证据来源</dt><dd>{node.evidence || '语义快照'}</dd></div>
        </dl>
      ) : (
        <dl>
          <div><dt>数据来源</dt><dd>{node.evidence || '数据集摘要接口'}</dd></div>
          <div><dt>处理状态</dt><dd>{node.status || '已返回'}</dd></div>
          <div><dt>估算字数</dt><dd>{model.metrics.estimatedWordCount ? compactNumber(model.metrics.estimatedWordCount) : '接口未返回'}</dd></div>
        </dl>
      )}
      {node.examples?.length ? (
        <div className="dataset-understanding-insight-group">
          <strong>安全示例</strong>
          <div className="dataset-understanding-insight-tags">
            {node.examples.map((example) => <span key={example}>{example}</span>)}
          </div>
        </div>
      ) : null}
      {node.entityType === 'object' && objectExamples.length ? (
        <div className="dataset-understanding-insight-group">
          <strong>字段安全示例</strong>
          <div className="dataset-understanding-insight-tags">
            {objectExamples.map((example) => <span key={example}>{example}</span>)}
          </div>
        </div>
      ) : null}
      {objectFields.length ? (
        <div className="dataset-understanding-insight-group">
          <strong>已理解字段</strong>
          <div className="dataset-understanding-insight-tags">
            {objectFields.slice(0, 12).map((field) => <span key={field.id}>{field.name}</span>)}
          </div>
        </div>
      ) : null}
      {model.mode === 'semantic' ? (
        <>
          <ConnectionGroup title="上游依据 / 入向关系" items={incoming} model={model} onSelectLink={onSelectLink} direction="incoming" />
          <ConnectionGroup title="下游理解 / 出向关系" items={outgoing} model={model} onSelectLink={onSelectLink} direction="outgoing" />
        </>
      ) : (
        <ConnectionGroup title="直接关联" items={[...incoming, ...outgoing].slice(0, 12)} model={model} onSelectLink={onSelectLink} direction="outgoing" />
      )}
      {model.emptyKnowledgeMessage ? <small className="dataset-understanding-honesty-note">{model.emptyKnowledgeMessage}</small> : null}
    </>
  );
}

function StageInspector({ stage, onSelectNode }) {
  return (
    <>
      <span>处理阶段详情</span>
      <div className={`dataset-understanding-stage-badge ${stage.status}`.trim()}>
        <i />{stage.status === 'complete' ? '已完成' : stage.status === 'attention' ? '需要关注' : '暂无明细'}
      </div>
      <h4>{stage.label}</h4>
      <p>{stage.summary}</p>
      <dl>
        <div><dt>阶段结果</dt><dd>{stage.value}</dd></div>
        <div><dt>字段来源</dt><dd>{stage.source}</dd></div>
        <div><dt>明细数量</dt><dd>{stage.items.length}</dd></div>
      </dl>
      <div className="dataset-understanding-inspector-list">
        {stage.items.length ? stage.items.map((item) => (
          <button key={item.id} type="button" onClick={() => item.nodeId && onSelectNode(item.nodeId)}>
            <span><strong>{item.label}</strong><em>{item.meta}</em></span>
            <small>{item.detail}</small>
            <small className="source">{item.evidence}</small>
          </button>
        )) : (
          <div className="dataset-understanding-inspector-empty">当前接口没有返回这个阶段的逐条明细。</div>
        )}
      </div>
    </>
  );
}

export default function DatasetUnderstandingGraph({ dataset, documents = [], understandingState = null }) {
  const chartRef = useRef(null);
  const model = useMemo(
    () => buildDatasetUnderstandingGraph(dataset, documents, understandingState?.data || null),
    [dataset, documents, understandingState?.data],
  );
  const [activeCategory, setActiveCategory] = useState('all');
  const [activeRelationType, setActiveRelationType] = useState('all');
  const [viewMode, setViewMode] = useState('business');
  const [focusDepth, setFocusDepth] = useState('all');
  const [selectedNodeId, setSelectedNodeId] = useState('');
  const [selectedLinkId, setSelectedLinkId] = useState('');
  const [selectedStageKey, setSelectedStageKey] = useState('overview');
  const [chartState, setChartState] = useState('loading');
  const selectedNode = model.nodes.find((node) => node.id === selectedNodeId) || model.nodes[0] || null;
  const selectedLink = model.links.find((link) => link.id === selectedLinkId) || null;
  const selectedLinkSource = selectedLink
    ? model.nodes.find((node) => node.id === selectedLink.source) || null
    : null;
  const selectedLinkTarget = selectedLink
    ? model.nodes.find((node) => node.id === selectedLink.target) || null
    : null;
  const selectedStage = model.pipeline.find((stage) => stage.key === selectedStageKey) || null;
  const overviewGraph = useMemo(() => filterDatasetUnderstandingGraph(model, {
    activeCategory: 'all',
    activeRelationType: 'all',
    viewMode,
    focusDepth: 'all',
  }), [model, viewMode]);
  const categoryCounts = useMemo(() => Object.fromEntries(
    DATASET_GRAPH_CATEGORIES.map((category) => [
      category.key,
      (category.key === 'unresolved' ? model.nodes : overviewGraph.nodes)
        .filter((node) => node.kind === category.key).length,
    ]),
  ), [model.nodes, overviewGraph.nodes]);
  const relationCounts = useMemo(() => ({
    all: overviewGraph.links.length,
    confirmed: overviewGraph.links.filter((link) => link.type === 'confirmed').length,
    observed: overviewGraph.links.filter((link) => link.type === 'observed').length,
    inferred: overviewGraph.links.filter((link) => link.type === 'inferred').length,
  }), [overviewGraph.links]);
  const chartOption = useMemo(
    () => optionForModel(model, {
      activeCategory,
      activeRelationType,
      viewMode,
      focusNodeId: selectedNodeId,
      focusDepth,
    }),
    [model, activeCategory, activeRelationType, viewMode, selectedNodeId, focusDepth],
  );
  const chartSignature = JSON.stringify(chartOption, (key, value) => typeof value === 'function' ? String(value) : value);

  useEffect(() => {
    setSelectedNodeId(model.nodes[0]?.id || '');
    setSelectedLinkId('');
    setSelectedStageKey('overview');
    setActiveCategory('all');
    setActiveRelationType('all');
    setViewMode('business');
    setFocusDepth('all');
  }, [model.datasetId]);

  useEffect(() => {
    setSelectedLinkId('');
  }, [activeCategory, activeRelationType]);

  useEffect(() => {
    if (!model.hasDataset || !chartRef.current || !model.nodes.length) {
      setChartState('empty');
      return undefined;
    }

    let chart = null;
    let disposed = false;
    let observer = null;
    setChartState('loading');

    import('echarts').then((echarts) => {
      if (disposed || !chartRef.current) return;
      echarts.getInstanceByDom(chartRef.current)?.dispose();
      chart = echarts.init(chartRef.current, null, { renderer: 'canvas' });
      chart.setOption(chartOption, true);
      chart.on('click', (params) => {
        if (params.dataType === 'node' && params.data?.id) {
          setSelectedNodeId(params.data.id);
          setSelectedLinkId('');
          setSelectedStageKey('');
        }
        if (params.dataType === 'edge' && params.data?.id) {
          setSelectedLinkId(params.data.id);
          setSelectedStageKey('');
        }
      });
      observer = typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(() => chart?.resize());
      observer?.observe(chartRef.current);
      setChartState('ready');
    }).catch(() => {
      if (!disposed) setChartState('error');
    });

    return () => {
      disposed = true;
      observer?.disconnect();
      chart?.dispose();
    };
  }, [model.hasDataset, model.datasetId, model.nodes.length, chartSignature]);

  if (!model.hasDataset) {
    return (
      <section className="dataset-understanding-panel empty" aria-label="数据集理解图谱">
        <div className="dataset-understanding-empty-orbit" aria-hidden="true">
          <span />
          <span />
          <span />
        </div>
        <div>
          <span className="dataset-understanding-eyebrow">DATASET INTELLIGENCE</span>
          <h3>从顶部选择数据集，查看系统理解</h3>
          <p>这里会根据现有数据展示解析清洗链路、知识点、章节结构、资料类型和可追溯连线。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="dataset-understanding-panel" aria-label={`${model.title} 数据集理解图谱`}>
      <header className="dataset-understanding-head">
        <div>
          <span className="dataset-understanding-eyebrow">DATASET INTELLIGENCE · {model.viewLabel || (model.mode === 'semantic' ? '真实语义快照' : '资料来源图')}</span>
          <h3>{model.title}</h3>
          <p>{model.mode === 'semantic'
            ? '业务对象、字段与关系来自版本化语义快照；点击节点可查看入向依据、出向理解和原始字段。'
            : '语义快照尚未生成；当前只展示通过质量门禁的资料来源与摘要连线，不能代表系统已经形成业务理解。'}</p>
        </div>
        <div className="dataset-understanding-metrics" aria-label="数据集理解指标">
          <span><small>{model.mode === 'semantic' ? '来源' : '资料'}</small><strong>{model.mode === 'semantic' ? model.metrics.sourceCount : model.metrics.documentCount}</strong></span>
          <span><small>{model.mode === 'semantic' ? '业务对象' : '知识线索'}</small><strong>{model.mode === 'semantic' ? model.metrics.objectCount : model.metrics.knowledgeCount}</strong></span>
          <span className="relations"><small>{model.mode === 'semantic' ? '语义关系' : '交叉关系'}</small><strong>{model.metrics.crossNodeRelationCount}</strong></span>
          <span><small>{model.mode === 'semantic' ? '已确认事实' : '可检索'}</small><strong>{model.mode === 'semantic' ? model.metrics.confirmedFactCount : model.metrics.readyDocumentCount}</strong></span>
          <span className={model.metrics.attentionDocumentCount ? 'attention' : ''}>
            <small>{model.mode === 'semantic' ? '待解释' : '待关注'}</small><strong>{model.mode === 'semantic' ? model.metrics.unresolvedFieldCount : model.metrics.attentionDocumentCount}</strong>
          </span>
        </div>
      </header>

      <div className={`dataset-understanding-snapshot-state ${understandingState?.status || model.snapshotStatus} ${model.stale ? 'stale' : ''}`.trim()}>
        <i />
        <span>
          {understandingState?.status === 'loading'
            ? '正在读取真实语义理解快照；暂时保留当前可用视图。'
            : understandingState?.status === 'failed'
              ? '真实语义快照暂时不可用；当前展示通过质量门禁的资料来源图。'
              : model.statusMessage}
        </span>
      </div>

      <div className="dataset-understanding-pipeline" aria-label="数据处理链路">
        {model.pipeline.map((stage, index) => (
          <PipelineStage
            key={stage.key}
            stage={stage}
            index={index}
            last={index === model.pipeline.length - 1}
            active={selectedStageKey === stage.key}
            onSelect={() => {
              setSelectedStageKey(stage.key);
              setSelectedLinkId('');
            }}
          />
        ))}
      </div>

      <div className="dataset-understanding-semantic-brief" aria-label={model.mode === 'semantic' ? '系统理解概览' : '资料来源概览'}>
        <button type="button" onClick={() => setSelectedStageKey(model.mode === 'semantic' ? 'structure' : 'knowledge')}>
          <span>{model.mode === 'semantic' ? '业务对象' : '可信线索'}</span>
          <strong>{model.understanding.keyConcepts.slice(0, 4).join(' · ') || '待识别'}</strong>
          <small>{model.understanding.keyConcepts.length} 个{model.mode === 'semantic' ? '业务语义词' : '可信中文线索'}</small>
        </button>
        <button type="button" onClick={() => setSelectedStageKey(model.mode === 'semantic' ? 'labels' : 'structure')}>
          <span>{model.mode === 'semantic' ? '关键字段' : '结构主线'}</span>
          <strong>{(model.mode === 'semantic' ? model.understanding.keyFields : model.understanding.structurePath).slice(0, 4).join(' → ') || '待识别'}</strong>
          <small>{model.mode === 'semantic' ? `${model.metrics.fieldCount} 个字段，${model.metrics.unresolvedFieldCount} 个待解释` : `${model.understanding.structurePath.length} 个结构线索`}</small>
        </button>
        <button type="button" onClick={() => setSelectedStageKey(model.mode === 'semantic' ? 'source' : 'ready')}>
          <span>可检索覆盖</span>
          <strong>{model.understanding.retrievalCoverage.ready} / {model.understanding.retrievalCoverage.total}</strong>
          <small>{model.mode === 'semantic' ? '可追溯覆盖情况' : '明确进入检索的资料'}</small>
        </button>
        <button type="button" onClick={() => setSelectedStageKey(model.mode === 'semantic' ? 'relations' : 'overview')}>
          <span>{model.mode === 'semantic' ? '关系理解' : '关系线索'}</span>
          <strong>{model.metrics.confirmedRelationCount || 0} 确 · {model.metrics.observedRelationCount} 观 · {model.metrics.inferredRelationCount} 推</strong>
          <small>点击图中节点查看关联依据</small>
        </button>
      </div>

      <div className="dataset-understanding-filter" aria-label="图谱类型筛选">
        <button
          type="button"
          className={activeCategory === 'all' ? 'active' : ''}
          onClick={() => setActiveCategory('all')}
        >
          全部 <small>{Math.max(0, overviewGraph.nodes.length - 1)}</small>
        </button>
        {DATASET_GRAPH_CATEGORIES.filter((category) => (
          category.key !== 'dataset'
            && (model.mode === 'semantic'
              ? ['object', 'field', 'unresolved'].includes(category.key)
              : ['document', 'knowledge', 'section', 'material', 'strategy'].includes(category.key))
        )).map((category) => (
          <button
            key={category.key}
            type="button"
            className={activeCategory === category.key ? 'active' : ''}
            style={{ '--category-color': category.color }}
            disabled={!categoryCounts[category.key]}
            onClick={() => setActiveCategory(category.key)}
          >
            {category.name} <small>{categoryCounts[category.key]}</small>
          </button>
        ))}
      </div>

      <div className="dataset-understanding-relationship-toolbar">
        {model.mode === 'semantic' ? (
          <div className="dataset-understanding-view-switch" aria-label="业务与技术视图切换">
            <span>显示</span>
            <button type="button" className={viewMode === 'business' ? 'active' : ''} onClick={() => setViewMode('business')}>业务视图</button>
            <button type="button" className={viewMode === 'technical' ? 'active' : ''} onClick={() => setViewMode('technical')}>技术视图</button>
          </div>
        ) : null}
        <div className="dataset-understanding-relation-filter" aria-label="关系可信度筛选">
          <span>关系视图</span>
          {[
            ['all', '全部关系'],
            ...(model.mode === 'semantic' ? [['confirmed', '已确认'], ['observed', '已观察']] : [['observed', '事实关系']]),
            ['inferred', '推断关系'],
          ].map(([key, label]) => (
            <button
              key={key}
              type="button"
              className={activeRelationType === key ? 'active' : ''}
              onClick={() => setActiveRelationType(key)}
            >
              {label} <small>{relationCounts[key]}</small>
            </button>
          ))}
        </div>
        <div className="dataset-understanding-relation-legend" aria-label="关系图例">
          {model.mode === 'semantic' ? <span><i className="confirmed" />绿线 · 已确认</span> : null}
          <span><i className="observed" />实线 · 已观察</span>
          <span><i className="inferred" />虚线 · 待验证</span>
        </div>
      </div>

      {model.mode === 'semantic' ? (
        <div className="dataset-understanding-local-toolbar" aria-label="局部图深度">
          <span>图谱范围</span>
          {[['all', '全局'], [1, '一跳'], [2, '两跳']].map(([depth, label]) => (
            <button
              key={depth}
              type="button"
              className={focusDepth === depth ? 'active' : ''}
              onClick={() => setFocusDepth(depth)}
            >
              {label}
            </button>
          ))}
          <small>{focusDepth === 'all' ? '查看全部业务网络' : `以“${selectedNode?.name || model.title}”为中心聚焦`}</small>
        </div>
      ) : null}

      <div className="dataset-understanding-canvas-grid">
        <div className="dataset-understanding-chart-shell">
          <div ref={chartRef} className="dataset-understanding-chart" role="img" aria-label={`${model.title} ${model.mode === 'semantic' ? '知识连接图' : '资料来源连接图'}`} />
          {chartState === 'loading' ? <div className="dataset-understanding-chart-state">正在生成理解图谱…</div> : null}
          {chartState === 'error' ? <div className="dataset-understanding-chart-state">图谱画布加载失败，可使用右侧证据详情和下方清单。</div> : null}
          <div className="dataset-understanding-chart-hint">滚轮缩放 · 拖拽节点 · 点击查看证据{model.mode === 'semantic' ? ' · 每个对象优先展示 4 个中文字段' : ' · 低质量标签已移入待解释清单'}</div>
        </div>

        <aside className="dataset-understanding-evidence dataset-understanding-inspector" aria-live="polite">
          {selectedStageKey === 'overview' ? (
            <UnderstandingOverview model={model} />
          ) : selectedStage ? (
            <StageInspector
              stage={selectedStage}
              onSelectNode={(nodeId) => {
                setSelectedNodeId(nodeId);
                setSelectedLinkId('');
                setSelectedStageKey('');
              }}
            />
          ) : selectedLink ? (
            <>
              <span>当前关系依据</span>
              <div className={`dataset-understanding-node-kind relation ${selectedLink.type}`.trim()}>
                <i />
                {relationClassLabel(selectedLink.type)}关系
              </div>
              <h4>{selectedLinkSource?.name || '来源节点'} <em>{selectedLink.relation}</em> {selectedLinkTarget?.name || '目标节点'}</h4>
              <p>{selectedLink.evidence}</p>
              <dl>
                <div><dt>证据等级</dt><dd>{relationClassLabel(selectedLink.type)}</dd></div>
                <div><dt>关系类型</dt><dd>{selectedLink.relationType || (selectedLink.structural ? '结构归属' : '相关关系')}</dd></div>
                <div><dt>置信度</dt><dd>{Math.round(selectedLink.confidence * 100)}%</dd></div>
                <div><dt>节点范围</dt><dd>{selectedLink.rootRelation ? '数据集归属关系' : '知识网络交叉关系'}</dd></div>
              </dl>
              {selectedLink.type === 'inferred' ? (
                <small className="dataset-understanding-honesty-note">虚线不等同于已确认的业务因果或实体关系。</small>
              ) : null}
            </>
          ) : <NodeInspector model={model} node={selectedNode} onSelectLink={setSelectedLinkId} />}
        </aside>
      </div>

      <details className="dataset-understanding-ledger">
        <summary>查看图谱证据清单</summary>
        <div className="dataset-understanding-ledger-groups">
          <section>
            <h5>节点证据</h5>
            <div>
              {model.nodes.slice(1).map((node) => (
                <button
                  key={node.id}
                  type="button"
                  onClick={() => {
                    setSelectedNodeId(node.id);
                    setSelectedLinkId('');
                    setSelectedStageKey('');
                  }}
                >
                  <i style={{ background: graphCategory(model, node.kind).color }} />
                  <span><strong>{node.name}</strong><small>{node.evidence}</small></span>
                </button>
              ))}
            </div>
          </section>
          <section>
            <h5>交叉关系</h5>
            <div>
              {model.links.filter((link) => !link.rootRelation).slice(0, 36).map((link) => {
                const source = model.nodes.find((node) => node.id === link.source);
                const target = model.nodes.find((node) => node.id === link.target);
                return (
                  <button key={link.id} type="button" onClick={() => {
                    setSelectedLinkId(link.id);
                    setSelectedStageKey('');
                  }}>
                    <i className={`relation ${link.type}`.trim()} />
                    <span>
                      <strong>{source?.name || '节点'} · {link.relation} · {target?.name || '节点'}</strong>
                      <small>{link.type === 'inferred' ? `推断 ${Math.round(link.confidence * 100)}%` : '事实关系'} · {link.evidence}</small>
                    </span>
                  </button>
                );
              })}
            </div>
          </section>
        </div>
      </details>
    </section>
  );
}
