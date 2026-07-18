'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import {
  buildCrossDatasetUnderstandingGraph,
  buildDatasetUnderstandingGraph,
  DATASET_GRAPH_CATEGORIES,
  filterDatasetUnderstandingGraph,
  layoutCrossDatasetUnderstandingGraph,
} from '../lib/dataset-understanding-graph';
import {
  graphBudgetStatusText,
  graphDensityForContainerWidth,
} from '../lib/dataset-understanding-graph-budget';
import {
  datasetUnderstandingForceConfig,
  graphNodeLabelVisible,
  layoutDatasetUnderstandingGraph,
} from '../lib/dataset-understanding-graph-layout';
import { normalizeDatasetIds } from '../lib/dataset-record-scope';
import { buildKnowledgeGraphLenses } from '../lib/knowledge-graph-lenses';

const DATASET_GRAPH_SERIES_ID = 'dataset-understanding-graph';
let echartsModulePromise = null;

function loadEchartsModule() {
  if (!echartsModulePromise) echartsModulePromise = import('echarts');
  return echartsModulePromise;
}

if (typeof window !== 'undefined') void loadEchartsModule();

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

function normalizedKnowledgeGraphLenses(value) {
  const input = Array.isArray(value)
    ? value
    : Array.isArray(value?.lenses)
      ? value.lenses
      : Array.isArray(value?.facets)
        ? value.facets
        : [];
  const lenses = input.map((lens, index) => {
    const focusNodeId = String(lens?.focusNodeId || '').trim();
    const nodeIds = [...new Set([
      ...(Array.isArray(lens?.nodeIds) ? lens.nodeIds : []),
      ...(Array.isArray(lens?.nodes) ? lens.nodes.map((node) => (
        typeof node === 'string' ? node : node?.id
      )) : []),
      focusNodeId,
    ].map((id) => String(id || '').trim()).filter(Boolean))];
    const label = String(lens?.label || lens?.name || lens?.title || '').trim();
    const declaredCount = Number(lens?.count ?? lens?.nodeCount);
    return {
      key: String(lens?.key || lens?.id || `lens-${index + 1}`).trim(),
      label: label || `领域镜头 ${index + 1}`,
      description: String(lens?.description || lens?.detail || lens?.summary || '').trim(),
      nodeIds,
      focusNodeId: focusNodeId || nodeIds[0] || '',
      count: Number.isFinite(declaredCount) ? Math.max(0, declaredCount) : nodeIds.length,
      color: String(lens?.color || '').trim(),
    };
  }).filter((lens) => lens.key && lens.nodeIds.length);
  return {
    lenses,
    description: Array.isArray(value)
      ? ''
      : String(value?.description || value?.summary || '').trim(),
  };
}

function staticLayoutExtentAnchors(nodes) {
  const positioned = (Array.isArray(nodes) ? nodes : []).filter((node) => (
    Number.isFinite(Number(node?.x)) && Number.isFinite(Number(node?.y))
  ));
  if (!positioned.length) return [];
  const xs = positioned.map((node) => Number(node.x));
  const ys = positioned.map((node) => Number(node.y));
  const minimumX = Math.min(...xs) - 90;
  const maximumX = Math.max(...xs) + 90;
  const minimumY = Math.min(...ys) - 90;
  const maximumY = Math.max(...ys) + 90;
  return [
    ['north-west', minimumX, minimumY],
    ['north-east', maximumX, minimumY],
    ['south-east', maximumX, maximumY],
    ['south-west', minimumX, maximumY],
  ].map(([key, x, y]) => ({
    id: `__dataset-layout-anchor:${key}`,
    name: '',
    x,
    y,
    fixed: true,
    layoutAnchor: true,
    symbolSize: 0,
  }));
}

function optionForModel(model, filters, projectedGraph = null) {
  const { nodes: visibleNodes, links: visibleLinks } = projectedGraph
    || filterDatasetUnderstandingGraph(model, filters);
  const layoutInputNodes = model.nodes;
  const layoutBaseNodes = model.mode === 'cross'
    ? layoutCrossDatasetUnderstandingGraph(layoutInputNodes, model.datasetClusters)
    : layoutDatasetUnderstandingGraph(layoutInputNodes);
  const layoutById = new Map(layoutBaseNodes.map((node) => [node.id, node]));
  const positionedNodes = visibleNodes.map((node) => {
    const layoutNode = layoutById.get(node.id);
    return layoutNode ? {
      ...node,
      x: layoutNode.x,
      y: layoutNode.y,
      fixed: layoutNode.fixed,
      layoutTier: layoutNode.layoutTier,
      symbol: layoutNode.symbol || node.symbol,
      symbolSize: layoutNode.symbolSize || node.symbolSize,
    } : node;
  });
  const force = datasetUnderstandingForceConfig(layoutBaseNodes.length);
  const staticLayout = model.mode === 'fallback'
    || model.mode === 'cross'
    || layoutBaseNodes.length > 120;
  const seriesNodes = staticLayout
    ? [...positionedNodes, ...staticLayoutExtentAnchors(layoutBaseNodes)]
    : positionedNodes;
  const zoom = Number(filters?.zoom) || 1;
  const selectedNodeId = filters?.focusNodeId || '';

  return {
    animationDuration: staticLayout ? 0 : 650,
    animationDurationUpdate: staticLayout ? 0 : 420,
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
        return [
          params.data?.shared ? '共享节点' : '',
          params.data?.name,
          params.data?.detail,
          params.data?.sourceDatasets?.length
            ? `可见来源：${params.data.sourceDatasets.map((dataset) => dataset.title).join('、')}`
            : '',
        ].filter(Boolean).join('\n');
      },
    },
    series: [{
      id: DATASET_GRAPH_SERIES_ID,
      type: 'graph',
      // Large single-dataset and every clustered cross-dataset view already
      // have deterministic coordinates. Keeping the layout strategy tied to
      // the full model preserves the same mental map as filters change.
      layout: staticLayout ? 'none' : 'force',
      roam: true,
      draggable: !staticLayout,
      cursor: 'grab',
      categories: model.categories.map((category) => ({
        name: category.name,
        itemStyle: { color: category.color },
      })),
      data: seriesNodes.map((node) => node.layoutAnchor ? {
        ...node,
        silent: true,
        tooltip: { show: false },
        label: { show: false },
        itemStyle: { opacity: 0 },
      } : ({
        ...node,
        label: {
          fontSize: node.kind === 'dataset' ? 11 : 9,
          fontWeight: node.kind === 'dataset' ? 800 : 650,
          color: node.signal === 'identifier' ? '#8794a8' : '#dce6f4',
        },
        itemStyle: {
          color: model.mode === 'cross' ? node.clusterColor : graphCategory(model, node.kind).color,
          opacity: node.signal === 'identifier' ? 0.62 : 1,
          borderColor: node.shared || node.kind === 'dataset' ? '#ffffff' : 'rgba(255,255,255,0.52)',
          borderWidth: node.shared || node.kind === 'dataset' ? 2 : 1,
          shadowBlur: node.shared || node.kind === 'dataset' ? 28 : 12,
          shadowColor: model.mode === 'cross'
            ? `${node.clusterColor}66`
            : `${graphCategory(model, node.kind).color}55`,
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
          type: link.lineKind || (link.type === 'inferred' ? 'dashed' : 'solid'),
          width: link.rootRelation ? 0.75 : link.type === 'inferred' ? 1.35 : 1.8,
          opacity: link.rootRelation ? 0.24 : link.type === 'inferred' ? 0.62 : 0.76,
          curveness: link.rootRelation ? 0.02 : 0.12,
        },
      })),
      force: {
        ...force,
      },
      label: {
        show: true,
        position: 'right',
        distance: 5,
        color: '#dce6f4',
        fontSize: 9,
        textBorderColor: 'rgba(2, 8, 18, 0.92)',
        textBorderWidth: 3,
        rich: {
          shared: {
            color: '#0f172a',
            backgroundColor: '#f8fafc',
            borderRadius: 7,
            padding: [2, 5],
            fontSize: 8,
            fontWeight: 800,
          },
        },
        formatter(params) {
          if (!graphNodeLabelVisible(params.data, zoom, selectedNodeId)) return '';
          const label = String(params.data?.shortLabel || '').slice(0, 5);
          return params.data?.shared ? `{shared|共享}\n${label}` : label;
        },
      },
      labelLayout: { hideOverlap: true },
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

function updateChartLabelLod(chart, zoom, selectedNodeId, element = null) {
  if (!chart || chart.isDisposed?.()) return;
  const option = {
    series: [{
      id: DATASET_GRAPH_SERIES_ID,
      label: {
        formatter(params) {
          if (!graphNodeLabelVisible(params.data, zoom, selectedNodeId)) return '';
          const label = String(params.data?.shortLabel || '').slice(0, 5);
          return params.data?.shared ? `{shared|共享}\n${label}` : label;
        },
      },
    }],
  };
  markChartRenderRequested(element, option);
  chart.setOption(option, { lazyUpdate: true });
}

function markChartRenderRequested(element, option) {
  if (!element) return 0;
  const revision = Number(element.dataset.echartsRequestedRevision || 0) + 1;
  const series = option?.series?.[0] || {};
  element.dataset.echartsRequestedRevision = String(revision);
  element.dataset.echartsUpdateCount = String(revision);
  element.dataset.echartsReady = 'false';
  if (series.layout) element.dataset.echartsLayout = String(series.layout);
  if (Array.isArray(series.data)) {
    element.dataset.echartsNodeCount = String(series.data.filter((node) => !node.layoutAnchor).length);
  }
  if (Array.isArray(series.links)) element.dataset.echartsEdgeCount = String(series.links.length);
  return revision;
}

function markChartRenderFinished(element) {
  if (!element) return;
  const requested = Number(element.dataset.echartsRequestedRevision || 0);
  const rendered = Number(element.dataset.echartsRenderedRevision || 0);
  if (requested <= rendered) return;
  element.dataset.echartsRenderedRevision = String(requested);
  element.dataset.echartsFinishedAt = String(performance.now());
  element.dataset.echartsReady = 'true';
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

function CrossDatasetStory({ model, activeLensKey, onSelectLens }) {
  if (model.mode !== 'cross' || !model.jointStory) return null;
  return (
    <section className="dataset-understanding-joint-story" aria-label="联合数据理解">
      <div className="dataset-understanding-joint-story-head">
        <span>JOINT ANALYSIS MAP</span>
        <strong>{model.jointStory.headline}</strong>
        <p>{model.jointStory.detail}</p>
        <small className={model.jointStory.evidenceClass}>
          {relationClassLabel(model.jointStory.evidenceClass)} · {model.jointStory.alignmentEvidence}
          {model.jointStory.partial ? ' · 当前可见部分' : ''}
        </small>
      </div>
      <div className="dataset-understanding-joint-story-grid">
        {model.datasetStories.map((story) => (
          <article key={story.id} style={{ '--story-color': story.color || '#38bdf8' }}>
            <i />
            <span>{story.role}</span>
            <strong>{story.title}</strong>
            <p>{story.summary}</p>
            <small>{story.keyNodes.slice(0, 4).join(' · ') || '业务语义生成中'}</small>
          </article>
        ))}
        <article className="joint-bridge">
          <i />
          <span>联合理解</span>
          <strong>{model.jointStory.headline}</strong>
          <p>{model.jointStory.alignmentState === 'explicit'
            ? '显式共同维度负责对齐，互补指标负责解释，质量边界负责约束结论。'
            : model.jointStory.alignmentState === 'candidate'
              ? '已找到共同概念候选；先核验时间口径，再用互补指标解释业务。'
              : '待确认共同维度；当前只并列展示业务视角，不声称数据已经对齐。'}</p>
          <small>{model.analysisFacets.map((facet) => facet.name).join(' · ') || '共同维度待识别'}</small>
        </article>
      </div>
      <div className="dataset-understanding-analysis-lenses" role="group" aria-label="联合分析视角">
        <span>分析镜头</span>
        <button
          type="button"
          className={!activeLensKey ? 'active' : ''}
          aria-pressed={!activeLensKey}
          onClick={() => onSelectLens(null)}
        >
          业务全景 <small>{model.partial ? '可见 ' : ''}{model.nodes.length}</small>
        </button>
        {model.analysisFacets.map((facet) => (
          <button
            key={facet.key}
            type="button"
            className={activeLensKey === facet.key ? 'active' : ''}
            aria-pressed={activeLensKey === facet.key}
            style={{ '--lens-color': facet.color }}
            onClick={() => onSelectLens(facet)}
          >
            {facet.name} <small>{model.partial ? '可见 ' : ''}{facet.nodeCount}</small>
          </button>
        ))}
      </div>
      <small className="dataset-understanding-joint-guardrail">{model.jointStory.guardrail}</small>
    </section>
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
      <span>{model.mode === 'cross' ? '联合理解摘要' : model.mode === 'semantic' ? '系统理解摘要' : '资料来源摘要'}</span>
      <div className={`dataset-understanding-node-kind overview ${model.mode}`.trim()}>
        <i />{model.viewLabel || (model.mode === 'semantic' ? '真实语义快照' : '资料来源图')}
      </div>
      <h4>{model.overviewTitle || (model.mode === 'semantic' ? '系统已经理解到什么' : '当前资料来源包含什么')}</h4>
      <p>{understanding.summary}</p>
      <div className="dataset-understanding-insight-group">
        <strong>{model.mode === 'cross' ? '联合分析概念' : model.mode === 'semantic' ? '核心概念' : '可信中文线索'}</strong>
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
        {model.mode === 'cross'
          ? model.jointStory?.guardrail || '跨数据集关系只用于当前可见范围内的理解导航。'
          : model.mode === 'semantic'
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

function NodeInspector({ model, node, onSelectLink, onFocusDataset }) {
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

  if (model.mode === 'cross') {
    const sourceDatasets = Array.isArray(node.sourceDatasets) ? node.sourceDatasets : [];
    const focusDatasetId = node.kind === 'dataset' ? node.datasetRefs?.[0] : '';
    return (
      <>
        <span>{node.shared ? '共享节点详情' : '跨数据集节点详情'}</span>
        <div className={`dataset-understanding-node-kind ${node.shared ? 'shared' : ''}`.trim()}>
          <i style={{ background: node.clusterColor }} />
          {node.shared ? '共享' : graphCategory(model, node.kind).name}
        </div>
        <h4>{node.name}</h4>
        <p>{node.detail || '当前节点只来自本次请求可见的数据集范围。'}</p>
        <dl>
          <div><dt>节点类型</dt><dd>{graphCategory(model, node.kind).name}</dd></div>
          <div><dt>可见来源</dt><dd>{sourceDatasets.map((dataset) => dataset.title).join(' · ') || '未返回'}</dd></div>
          <div><dt>可见贡献数</dt><dd>{compactNumber(node.visibleProvenanceCount)}</dd></div>
          <div><dt>共享状态</dt><dd>{node.shared
            ? node.kind === 'concept'
              ? '多个数据集映射到同一受控业务概念，不代表记录身份一致'
              : '确定身份依据形成共享节点'
            : '当前数据集内节点'}</dd></div>
          <div><dt>证据范围</dt><dd>{node.evidence || '本次可见数据集范围'}</dd></div>
        </dl>
        {sourceDatasets.length ? (
          <div className="dataset-understanding-insight-group">
            <strong>来源数据集</strong>
            <div className="dataset-understanding-insight-tags">
              {sourceDatasets.map((sourceDataset) => (
                <span key={sourceDataset.id}>{sourceDataset.title}</span>
              ))}
            </div>
          </div>
        ) : null}
        {focusDatasetId ? (
          <button
            type="button"
            className="dataset-understanding-cluster-focus-action"
            onClick={() => onFocusDataset?.(focusDatasetId)}
          >
            聚焦此数据集
          </button>
        ) : null}
        <ConnectionGroup title="入向关系" items={incoming} model={model} onSelectLink={onSelectLink} direction="incoming" />
        <ConnectionGroup title="出向关系" items={outgoing} model={model} onSelectLink={onSelectLink} direction="outgoing" />
        {model.emptyCrossMessage ? <small className="dataset-understanding-honesty-note">{model.emptyCrossMessage}</small> : null}
      </>
    );
  }

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

export default function DatasetUnderstandingGraph({
  dataset,
  documents = [],
  understandingState = null,
  initialDatasetIds = [],
}) {
  const panelRef = useRef(null);
  const chartRef = useRef(null);
  const chartInstanceRef = useRef(null);
  const chartOptionRef = useRef(null);
  const selectedNodeIdRef = useRef('');
  const graphZoomRef = useRef(1);
  const graphCenterRef = useRef(null);
  const resizeFrameRef = useRef(null);
  const [graphMode, setGraphMode] = useState('single');
  const preferredCrossDatasetIds = normalizeDatasetIds([
    dataset?.id,
    ...initialDatasetIds,
  ]).slice(0, 8);
  const preferredCrossDatasetIdsKey = preferredCrossDatasetIds.join('|');
  const [crossDatasetIds, setCrossDatasetIds] = useState(() => preferredCrossDatasetIds);
  const [focusDatasetId, setFocusDatasetId] = useState('');
  const [activeLensKey, setActiveLensKey] = useState('');
  const [activeKnowledgeLensKey, setActiveKnowledgeLensKey] = useState('');
  const [nodeSearch, setNodeSearch] = useState('');
  const singleModel = useMemo(
    () => buildDatasetUnderstandingGraph(dataset, documents, understandingState?.data || null),
    [dataset, documents, understandingState?.data],
  );
  const crossGraphState = understandingState?.crossGraphState || null;
  const crossModel = useMemo(
    () => buildCrossDatasetUnderstandingGraph(
      crossGraphState?.rootDatasetId === dataset?.id ? crossGraphState?.data : null,
    ),
    [crossGraphState?.data, crossGraphState?.rootDatasetId, dataset?.id],
  );
  const model = graphMode === 'cross' && crossModel.hasDataset ? crossModel : singleModel;
  const knowledgeLensModel = useMemo(
    () => normalizedKnowledgeGraphLenses(buildKnowledgeGraphLenses(model, { profile: 'auto' })),
    [model],
  );
  const [activeCategory, setActiveCategory] = useState('all');
  const [activeRelationType, setActiveRelationType] = useState('all');
  const [viewMode, setViewMode] = useState('business');
  const [focusDepth, setFocusDepth] = useState('all');
  const [selectedNodeId, setSelectedNodeId] = useState('');
  const [selectedLinkId, setSelectedLinkId] = useState('');
  const [selectedStageKey, setSelectedStageKey] = useState('overview');
  const [chartState, setChartState] = useState('loading');
  const [densityPreference, setDensityPreference] = useState('auto');
  const [chartContainerWidth, setChartContainerWidth] = useState(1024);
  const [focusMode, setFocusMode] = useState(false);
  const availableDatasets = Array.isArray(understandingState?.availableDatasets)
    ? understandingState.availableDatasets
    : [];
  const crossGraphAvailable = understandingState?.crossGraphAvailable === true;
  const responsiveDensity = graphDensityForContainerWidth(chartContainerWidth);
  const graphDensity = densityPreference === 'auto' ? responsiveDensity : densityPreference;
  const graphRequestStatus = graphMode === 'cross'
    ? crossGraphState?.status || 'idle'
    : understandingState?.status || model.snapshotStatus;
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
    density: graphDensity,
  }), [model, viewMode, graphDensity]);
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
  const activeKnowledgeLens = useMemo(
    () => knowledgeLensModel.lenses.find((lens) => lens.key === activeKnowledgeLensKey) || null,
    [activeKnowledgeLensKey, knowledgeLensModel.lenses],
  );
  const activeLensNodeIds = useMemo(() => {
    const facet = model.analysisFacets?.find((item) => item.key === activeLensKey);
    if (facet) return facet.nodes.map((node) => node.id);
    return activeKnowledgeLens?.nodeIds || [];
  }, [model.analysisFacets, activeLensKey, activeKnowledgeLens]);
  const activeGraph = useMemo(() => filterDatasetUnderstandingGraph(model, {
    activeCategory,
    activeRelationType,
    viewMode,
    focusNodeId: selectedNodeId,
    focusDepth,
    focusNodeIds: activeLensNodeIds,
    focusDatasetId,
    density: graphDensity,
  }), [
    model,
    activeCategory,
    activeRelationType,
    viewMode,
    selectedNodeId,
    focusDepth,
    activeLensNodeIds,
    focusDatasetId,
    graphDensity,
  ]);
  const chartOption = useMemo(
    () => optionForModel(model, {
      activeCategory,
      activeRelationType,
      viewMode,
      focusNodeId: selectedNodeId,
      focusDepth,
      focusNodeIds: activeLensNodeIds,
      focusDatasetId,
      density: graphDensity,
    }, activeGraph),
    [
      model,
      activeCategory,
      activeRelationType,
      viewMode,
      selectedNodeId,
      focusDepth,
      activeLensNodeIds,
      focusDatasetId,
      graphDensity,
      activeGraph,
    ],
  );
  const nodeSearchResults = useMemo(() => {
    const query = nodeSearch.trim().toLocaleLowerCase();
    if (!query) return [];
    return model.nodes.filter((node) => {
      if (
        model.mode === 'semantic'
        && viewMode === 'business'
        && node.kind !== 'dataset'
        && (node.kind === 'unresolved' || node.technicalOnly)
      ) return false;
      // Search only user-visible semantic labels. Raw values and technical fields may
      // contain resume PII and must not become a side-channel through search results.
      return [node.name, node.shortLabel, semanticRoleLabel(node.semanticRole)]
        .some((value) => String(value || '').toLocaleLowerCase().includes(query));
    }).slice(0, 10);
  }, [model.mode, model.nodes, nodeSearch, viewMode]);
  chartOptionRef.current = chartOption;
  selectedNodeIdRef.current = selectedNodeId;

  const requestCrossGraph = (datasetIds, autoNeighbors = 0) => {
    const rootDatasetId = String(dataset?.id || '').trim();
    if (!rootDatasetId || typeof understandingState?.loadCrossGraph !== 'function') return;
    void understandingState.loadCrossGraph({
      rootDatasetId,
      datasetIds: datasetIds.filter((id) => id !== rootDatasetId),
      autoNeighbors,
      maxNodes: 160,
      maxEdges: 240,
      depth: 1,
    });
  };

  const selectGraphMode = (nextMode) => {
    setGraphMode(nextMode);
    setFocusDatasetId('');
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    if (nextMode !== 'cross') return;
    const selection = preferredCrossDatasetIds;
    setCrossDatasetIds(selection);
    requestCrossGraph(selection, selection.length > 1 ? 0 : 3);
  };

  const toggleCrossDataset = (datasetId) => {
    const rootDatasetId = String(dataset?.id || '').trim();
    if (!datasetId || datasetId === rootDatasetId) return;
    const selected = crossDatasetIds.includes(datasetId);
    const next = selected
      ? crossDatasetIds.filter((id) => id !== datasetId)
      : [...crossDatasetIds, datasetId].slice(0, 8);
    const normalized = [rootDatasetId, ...next.filter((id) => id && id !== rootDatasetId)].slice(0, 8);
    setCrossDatasetIds(normalized);
    setFocusDatasetId('');
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    requestCrossGraph(normalized, 0);
  };

  const focusDatasetCluster = (datasetId) => {
    const nextDatasetId = focusDatasetId === datasetId ? '' : datasetId;
    setFocusDatasetId(nextDatasetId);
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    if (!nextDatasetId) return;
    const datasetNode = model.nodes.find((node) => (
      node.kind === 'dataset' && node.datasetRefs?.includes(nextDatasetId)
    ));
    if (datasetNode) {
      setSelectedNodeId(datasetNode.id);
      setSelectedLinkId('');
      setSelectedStageKey('');
    }
  };

  const selectAnalysisLens = (facet) => {
    const nextKey = facet?.key && facet.key !== activeLensKey ? facet.key : '';
    setActiveLensKey(nextKey);
    setActiveKnowledgeLensKey('');
    setFocusDatasetId('');
    setActiveCategory('all');
    setActiveRelationType('all');
    setSelectedLinkId('');
    setSelectedStageKey('');
    if (nextKey && facet?.focusNodeId) {
      setSelectedNodeId(facet.focusNodeId);
      setFocusDepth('all');
      return;
    }
    const root = model.nodes.find((node) => node.rootDataset || node.kind === 'dataset');
    setSelectedNodeId(root?.id || model.nodes[0]?.id || '');
    setFocusDepth('all');
  };

  const selectCategory = (category) => {
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    setActiveCategory(category);
  };

  const selectRelationType = (relationType) => {
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    setActiveRelationType(relationType);
  };

  const selectFocusDepth = (depth) => {
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    setFocusDepth(depth);
  };

  const selectKnowledgeLens = (lens) => {
    const nextKey = lens?.key && lens.key !== activeKnowledgeLensKey ? lens.key : '';
    setActiveKnowledgeLensKey(nextKey);
    setActiveLensKey('');
    setFocusDatasetId('');
    setActiveCategory('all');
    setActiveRelationType('all');
    setFocusDepth('all');
    setSelectedLinkId('');
    setSelectedStageKey('');
    const root = model.nodes.find((node) => node.rootDataset || node.kind === 'dataset');
    setSelectedNodeId(nextKey ? lens.focusNodeId : root?.id || model.nodes[0]?.id || '');
  };

  const quickFocusNode = (node) => {
    if (!node?.id) return;
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    setFocusDatasetId('');
    setActiveCategory('all');
    setActiveRelationType('all');
    setFocusDepth(1);
    setSelectedNodeId(node.id);
    setSelectedLinkId('');
    setSelectedStageKey('');
  };

  const resetGraphView = () => {
    const root = model.nodes.find((node) => node.rootDataset || node.kind === 'dataset');
    setActiveCategory('all');
    setActiveRelationType('all');
    setViewMode('business');
    setFocusDepth('all');
    setFocusDatasetId('');
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    setNodeSearch('');
    setSelectedNodeId(root?.id || model.nodes[0]?.id || '');
    setSelectedLinkId('');
    setSelectedStageKey('overview');
    setDensityPreference('auto');
    graphZoomRef.current = 1;
    graphCenterRef.current = null;
  };

  useEffect(() => {
    setGraphMode('single');
    setCrossDatasetIds(preferredCrossDatasetIds);
    setFocusDatasetId('');
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    setNodeSearch('');
  }, [dataset?.id]);

  useEffect(() => {
    if (graphMode !== 'cross') return;
    setCrossDatasetIds(preferredCrossDatasetIds);
    if (preferredCrossDatasetIds.length) requestCrossGraph(preferredCrossDatasetIds, 0);
  }, [preferredCrossDatasetIdsKey]);

  useEffect(() => {
    if (!crossGraphAvailable && graphMode === 'cross') setGraphMode('single');
  }, [crossGraphAvailable, graphMode]);

  useEffect(() => {
    if (graphMode !== 'cross' || !crossGraphState?.data) return;
    const returnedIds = crossGraphState.data.datasets
      .map((item) => item.id)
      .filter(Boolean)
      .slice(0, 8);
    setCrossDatasetIds(returnedIds);
  }, [crossGraphState?.selectionKey, crossGraphState?.data, graphMode]);

  useEffect(() => {
    setSelectedNodeId(model.nodes[0]?.id || '');
    setSelectedLinkId('');
    setSelectedStageKey('overview');
    setActiveCategory('all');
    setActiveRelationType('all');
    setViewMode('business');
    setFocusDepth('all');
    setActiveLensKey('');
    setActiveKnowledgeLensKey('');
    setNodeSearch('');
    setDensityPreference('auto');
    setFocusMode(false);
    graphZoomRef.current = 1;
    graphCenterRef.current = null;
  }, [model.datasetId, model.mode, crossGraphState?.selectionKey]);

  useEffect(() => {
    setSelectedLinkId('');
  }, [activeCategory, activeRelationType]);

  useEffect(() => {
    const visibleNodeIds = new Set(activeGraph.nodes.map((node) => node.id));
    const visibleLinkIds = new Set(activeGraph.links.map((link) => link.id));
    if (selectedLinkId && !visibleLinkIds.has(selectedLinkId)) setSelectedLinkId('');
    if (selectedNodeId && visibleNodeIds.has(selectedNodeId)) return;
    if (focusDepth !== 'all') setFocusDepth('all');
    const fallback = activeGraph.nodes.find((node) => node.rootDataset || node.kind === 'dataset')
      || activeGraph.nodes[0]
      || null;
    setSelectedNodeId(fallback?.id || '');
  }, [activeGraph.links, activeGraph.nodes, focusDepth, selectedLinkId, selectedNodeId]);

  useEffect(() => {
    if (!focusMode) return undefined;
    const previousOverflow = document.body.style.overflow;
    const handleKeyDown = (event) => {
      if (event.key === 'Escape') setFocusMode(false);
    };
    document.body.style.overflow = 'hidden';
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.body.style.overflow = previousOverflow;
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [focusMode]);

  useEffect(() => {
    if (!model.hasDataset || !chartRef.current || !model.nodes.length) {
      setChartState('empty');
      return undefined;
    }

    let cancelled = false;
    let observer = null;
    setChartState('loading');

    loadEchartsModule().then((echarts) => {
      if (cancelled || !chartRef.current) return;
      const chart = echarts.getInstanceByDom(chartRef.current)
        || echarts.init(chartRef.current, null, { renderer: 'canvas' });
      chartInstanceRef.current = chart;
      chartRef.current.dataset.echartsInstanceId = chart.id;
      chartRef.current.dataset.echartsInitCount = String(
        (Number(chartRef.current.dataset.echartsInitCount) || 0) + 1,
      );
      chartRef.current.__datasetGraphScreenPositionSnapshot = () => {
        const seriesModel = chart.getModel?.().getSeriesByIndex?.(0);
        const data = seriesModel?.getData?.();
        if (!data) return [];
        const positions = [];
        for (let index = 0; index < data.count(); index += 1) {
          const id = String(data.getId(index) || '');
          if (!id || id.startsWith('__dataset-layout-anchor:')) continue;
          const graphicElement = data.getItemGraphicEl(index);
          const globalPosition = graphicElement?.transformCoordToGlobal?.(0, 0);
          const x = Number(globalPosition?.[0]);
          const y = Number(globalPosition?.[1]);
          if (Number.isFinite(x) && Number.isFinite(y)) positions.push({ id, x, y });
        }
        return positions;
      };
      const handleFinished = () => {
        if (cancelled || !chartRef.current) return;
        markChartRenderFinished(chartRef.current);
        setChartState('ready');
      };
      chart.on('finished', handleFinished);
      markChartRenderRequested(chartRef.current, chartOptionRef.current);
      chart.setOption(chartOptionRef.current, {
        replaceMerge: ['series'],
        lazyUpdate: true,
      });
      const handleClick = (params) => {
        if (params.dataType === 'node' && params.data?.id) {
          setActiveLensKey('');
          setSelectedNodeId(params.data.id);
          setSelectedLinkId('');
          setSelectedStageKey('');
        }
        if (params.dataType === 'edge' && params.data?.id) {
          setActiveLensKey('');
          setSelectedLinkId(params.data.id);
          setSelectedStageKey('');
        }
      };
      const handleGraphRoam = () => {
        const liveSeries = chart.getOption()?.series?.[0] || {};
        const zoom = Number(liveSeries.zoom) || graphZoomRef.current;
        graphZoomRef.current = zoom;
        if (Array.isArray(liveSeries.center) && liveSeries.center.length === 2) {
          graphCenterRef.current = [...liveSeries.center];
        }
        updateChartLabelLod(chart, zoom, selectedNodeIdRef.current, chartRef.current);
      };
      const scheduleChartResize = () => {
        if (resizeFrameRef.current !== null) return;
        resizeFrameRef.current = window.requestAnimationFrame(() => {
          resizeFrameRef.current = null;
          if (cancelled || !chartRef.current || chart.isDisposed?.()) return;
          const width = Math.round(chartRef.current.getBoundingClientRect().width || 0);
          if (width > 0) setChartContainerWidth((current) => current === width ? current : width);
          chart.resize();
        });
      };
      chart.on('click', handleClick);
      chart.on('graphRoam', handleGraphRoam);
      observer = typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(scheduleChartResize);
      observer?.observe(chartRef.current);
      scheduleChartResize();
      chartRef.current.__datasetGraphCleanup = () => {
        chart.off('click', handleClick);
        chart.off('graphRoam', handleGraphRoam);
        chart.off('finished', handleFinished);
        if (chartRef.current) delete chartRef.current.__datasetGraphScreenPositionSnapshot;
      };
    }).catch(() => {
      if (!cancelled) setChartState('error');
    });

    return () => {
      cancelled = true;
      observer?.disconnect();
      if (resizeFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
      chartRef.current?.__datasetGraphCleanup?.();
      if (chartInstanceRef.current && !chartInstanceRef.current.isDisposed?.()) {
        chartInstanceRef.current.dispose();
      }
      chartInstanceRef.current = null;
    };
  }, [model.hasDataset]);

  useEffect(() => {
    const chart = chartInstanceRef.current;
    if (!model.hasDataset || !chart || chart.isDisposed?.()) return;
    const option = {
      ...chartOption,
      series: chartOption.series.map((series) => ({
        ...series,
        zoom: graphZoomRef.current,
        ...(graphCenterRef.current ? { center: graphCenterRef.current } : {}),
      })),
    };
    markChartRenderRequested(chartRef.current, option);
    chart.setOption(option, {
      replaceMerge: ['series'],
      lazyUpdate: true,
    });
  }, [model.hasDataset, chartOption, selectedNodeId]);

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
    <section
      ref={panelRef}
      className={`dataset-understanding-panel ${focusMode ? 'focus-mode' : ''}`.trim()}
      aria-label={`${model.title} 数据集理解图谱`}
    >
      <header className="dataset-understanding-head">
        <div>
          <span className="dataset-understanding-eyebrow">DATASET INTELLIGENCE · {graphMode === 'cross' ? '跨数据集语义图' : model.viewLabel || (model.mode === 'semantic' ? '真实语义快照' : '资料来源图')}</span>
          <h3>{model.title}</h3>
          <p>{graphMode === 'cross'
            ? '把可见数据集组织为业务对象、共同维度和互补指标；事实、观察与分析推断保持分层。'
            : model.mode === 'semantic'
            ? '业务对象、字段与关系来自版本化语义快照；点击节点可查看入向依据、出向理解和原始字段。'
            : '语义快照尚未生成；当前只展示通过质量门禁的资料来源与摘要连线，不能代表系统已经形成业务理解。'}</p>
        </div>
        <div className="dataset-understanding-metrics" aria-label="数据集理解指标">
          <span><small>{model.mode === 'cross' ? '可见数据集' : model.mode === 'semantic' ? '来源' : '资料'}</small><strong>{model.mode === 'cross' ? model.datasetClusters.length : model.mode === 'semantic' ? model.metrics.sourceCount : model.metrics.documentCount}</strong></span>
          <span><small>{model.mode === 'cross' ? '共享节点' : model.mode === 'semantic' ? '业务对象' : '知识线索'}</small><strong>{model.mode === 'cross' ? model.metrics.sharedNodeCount : model.mode === 'semantic' ? model.metrics.objectCount : model.metrics.knowledgeCount}</strong></span>
          <span className="relations"><small>{model.mode === 'cross' ? '跨集关系' : model.mode === 'semantic' ? '语义关系' : '交叉关系'}</small><strong>{model.metrics.crossNodeRelationCount}</strong></span>
          <span><small>{model.mode === 'cross' ? '可靠邻居' : model.mode === 'semantic' ? '已确认事实' : '可检索'}</small><strong>{model.mode === 'cross' ? model.reliableNeighborDatasetIds.length : model.mode === 'semantic' ? model.metrics.confirmedFactCount : model.metrics.readyDocumentCount}</strong></span>
          <span className={model.metrics.attentionDocumentCount ? 'attention' : ''}>
            <small>{model.mode === 'cross' ? '推断线索' : model.mode === 'semantic' ? '待解释' : '待关注'}</small><strong>{model.mode === 'cross' ? model.metrics.inferredRelationCount : model.mode === 'semantic' ? model.metrics.unresolvedFieldCount : model.metrics.attentionDocumentCount}</strong>
          </span>
        </div>
      </header>

      <div className="dataset-understanding-mode-toolbar" aria-label="数据集图谱模式">
        <div className="dataset-understanding-mode-switch">
          <span>图谱范围</span>
          <button type="button" className={graphMode === 'single' ? 'active' : ''} onClick={() => selectGraphMode('single')}>当前数据集</button>
          {crossGraphAvailable ? (
            <button type="button" aria-label="联合图谱（跨数据集）" className={graphMode === 'cross' ? 'active' : ''} onClick={() => selectGraphMode('cross')}>联合图谱</button>
          ) : null}
        </div>
        {graphMode === 'cross' ? (
          <div className="dataset-understanding-cross-selection">
            <details>
              <summary>选择数据集 <small>{crossDatasetIds.length} / 8</small></summary>
              <div>
                {availableDatasets.map((candidate) => {
                  const active = crossDatasetIds.includes(candidate.id);
                  const root = candidate.id === dataset?.id;
                  return (
                    <button
                      key={candidate.id}
                      type="button"
                      className={active ? 'active' : ''}
                      disabled={root || (!active && crossDatasetIds.length >= 8)}
                      onClick={() => toggleCrossDataset(candidate.id)}
                    >
                      <i style={{ background: model.datasetClusters?.find((cluster) => cluster.id === candidate.id)?.color }} />
                      <span>{candidate.title}</span>
                      <small>{root ? '当前' : active ? '已选' : '加入'}</small>
                    </button>
                  );
                })}
              </div>
            </details>
            <small>自动邻居最多 3 个，必须有已确认或已观察证据；手动选择最多 8 个。</small>
          </div>
        ) : null}
      </div>

      {graphMode === 'cross' && model.mode === 'cross' ? (
        <div className="dataset-understanding-cluster-toolbar" aria-label="数据集颜色与聚焦">
          <button type="button" className={!focusDatasetId ? 'active' : ''} onClick={() => setFocusDatasetId('')}>全部集群</button>
          {model.datasetClusters.map((cluster) => (
            <button
              key={cluster.id}
              type="button"
              className={focusDatasetId === cluster.id ? 'active' : ''}
              onClick={() => focusDatasetCluster(cluster.id)}
            >
              <i style={{ background: cluster.color }} />
              {cluster.title}
              <small>{cluster.nodeCount}</small>
            </button>
          ))}
        </div>
      ) : null}

      {graphMode === 'cross' && model.mode === 'cross' ? (
        <CrossDatasetStory
          model={model}
          activeLensKey={activeLensKey}
          onSelectLens={selectAnalysisLens}
        />
      ) : null}

      {graphMode === 'cross' && model.mode === 'cross' && model.emptyCrossMessage ? (
        <div className="dataset-understanding-cross-empty">{model.emptyCrossMessage}</div>
      ) : null}

      {knowledgeLensModel.lenses.length ? (
        <section aria-label="领域知识镜头">
          <div className="dataset-understanding-analysis-lenses" role="group" aria-label="领域知识镜头">
            <span>领域镜头</span>
            <button
              type="button"
              className={!activeKnowledgeLensKey ? 'active' : ''}
              aria-pressed={!activeKnowledgeLensKey}
              onClick={() => selectKnowledgeLens(null)}
            >
              全部概览 <small>{model.nodes.length}</small>
            </button>
            {knowledgeLensModel.lenses.map((lens) => (
              <button
                key={lens.key}
                type="button"
                className={activeKnowledgeLensKey === lens.key ? 'active' : ''}
                aria-pressed={activeKnowledgeLensKey === lens.key}
                style={lens.color ? { '--lens-color': lens.color } : undefined}
                onClick={() => selectKnowledgeLens(lens)}
              >
                {lens.label} <small>{lens.count}</small>
              </button>
            ))}
          </div>
          <small className="dataset-understanding-joint-guardrail">
            {activeKnowledgeLens?.description
              || knowledgeLensModel.description
              || '领域镜头只改变当前图谱的观察范围，不会把推断关系升级为已确认事实。'}
          </small>
        </section>
      ) : null}

      <div className={`dataset-understanding-snapshot-state ${graphRequestStatus} ${model.stale ? 'stale' : ''}`.trim()}>
        <i />
        <span>
          {graphMode === 'cross' && graphRequestStatus === 'loading'
            ? '正在读取跨数据集语义图谱；当前数据集视图保持可用。'
            : graphMode === 'cross' && graphRequestStatus === 'failed'
              ? `跨数据集图谱暂时不可用：${crossGraphState?.error || '请求失败'}`
              : graphMode === 'cross' && model.mode !== 'cross'
                ? '切换到联合图谱后，将自动寻找最多 3 个有确认或观察证据的可见邻居。'
                : understandingState?.status === 'loading'
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

      <div className="dataset-understanding-semantic-brief" aria-label={model.mode === 'cross' ? '跨数据集概览' : model.mode === 'semantic' ? '系统理解概览' : '资料来源概览'}>
        <button type="button" onClick={() => setSelectedStageKey(model.mode === 'cross' ? 'semantics' : model.mode === 'semantic' ? 'structure' : 'knowledge')}>
          <span>{model.mode === 'cross' ? '分析视角' : model.mode === 'semantic' ? '业务对象' : '可信线索'}</span>
          <strong>{model.understanding.keyConcepts.slice(0, 4).join(' · ') || '待识别'}</strong>
          <small>{model.mode === 'cross' ? `${model.analysisFacets?.length || 0} 个业务镜头，${model.metrics.sharedNodeCount} 个共享节点` : `${model.understanding.keyConcepts.length} 个${model.mode === 'semantic' ? '业务语义词' : '可信中文线索'}`}</small>
        </button>
        <button type="button" onClick={() => setSelectedStageKey(model.mode === 'cross' ? 'scope' : model.mode === 'semantic' ? 'labels' : 'structure')}>
          <span>{model.mode === 'cross' ? '联合结构' : model.mode === 'semantic' ? '关键字段' : '结构主线'}</span>
          <strong>{(model.mode === 'semantic' ? model.understanding.keyFields : model.understanding.structurePath).slice(0, 4).join(' → ') || '待识别'}</strong>
          <small>{model.mode === 'cross' ? `${model.datasetClusters.length} 个可见集群` : model.mode === 'semantic' ? `${model.metrics.fieldCount} 个字段，${model.metrics.unresolvedFieldCount} 个待解释` : `${model.understanding.structurePath.length} 个结构线索`}</small>
        </button>
        <button type="button" onClick={() => setSelectedStageKey(model.mode === 'cross' ? 'scope' : model.mode === 'semantic' ? 'source' : 'ready')}>
          <span>{model.mode === 'cross' ? '可见快照' : '可检索覆盖'}</span>
          <strong>{model.understanding.retrievalCoverage.ready} / {model.understanding.retrievalCoverage.total}</strong>
          <small>{model.mode === 'cross' ? '当前快照 / 本次可见数据集' : model.mode === 'semantic' ? '可追溯覆盖情况' : '明确进入检索的资料'}</small>
        </button>
        <button type="button" onClick={() => setSelectedStageKey(model.mode === 'cross' || model.mode === 'semantic' ? 'relations' : 'overview')}>
          <span>{model.mode === 'cross' ? '联合证据' : model.mode === 'semantic' ? '关系理解' : '关系线索'}</span>
          <strong>{model.metrics.confirmedRelationCount || 0} 确 · {model.metrics.observedRelationCount} 观 · {model.metrics.inferredRelationCount} 推</strong>
          <small>点击图中节点查看关联依据</small>
        </button>
      </div>

      <div className="dataset-understanding-local-toolbar dataset-understanding-search-toolbar" aria-label="节点搜索与视图操作">
        <span>节点搜索</span>
        <input
          className="dataset-understanding-node-search"
          type="search"
          value={nodeSearch}
          onChange={(event) => setNodeSearch(event.target.value)}
          placeholder="搜索对象、字段、概念或经历"
          aria-label="搜索图谱节点"
        />
        <small aria-live="polite">
          当前可见 {activeGraph.nodes.length} 个节点 · {activeGraph.links.length} 条关系
        </small>
        <button type="button" onClick={resetGraphView}>重置视图</button>
      </div>
      {nodeSearch.trim() ? (
        <div className="dataset-understanding-cluster-toolbar dataset-understanding-search-results" aria-label="节点搜索结果">
          {nodeSearchResults.length ? nodeSearchResults.map((node) => (
            <button
              key={node.id}
              type="button"
              className={selectedNodeId === node.id ? 'active' : ''}
              aria-pressed={selectedNodeId === node.id}
              onClick={() => quickFocusNode(node)}
            >
              <i style={{ background: model.mode === 'cross' ? node.clusterColor : graphCategory(model, node.kind).color }} />
              {node.name}
              <small>{graphCategory(model, node.kind).name}</small>
            </button>
          )) : <small>没有匹配的可见业务节点。</small>}
        </div>
      ) : null}

      <div className="dataset-understanding-filter" aria-label="图谱类型筛选">
        <button
          type="button"
          className={activeCategory === 'all' ? 'active' : ''}
          onClick={() => selectCategory('all')}
        >
          全部 <small>{Math.max(0, overviewGraph.nodes.length - 1)}</small>
        </button>
        {DATASET_GRAPH_CATEGORIES.filter((category) => (
          category.key !== 'dataset'
            && (model.mode === 'cross'
              ? ['document', 'object', 'field', 'concept', 'structure'].includes(category.key)
              : model.mode === 'semantic'
              ? ['object', 'field', 'unresolved'].includes(category.key)
              : ['document', 'knowledge', 'section', 'material', 'strategy'].includes(category.key))
        )).map((category) => (
          <button
            key={category.key}
            type="button"
            className={activeCategory === category.key ? 'active' : ''}
            style={{ '--category-color': category.color }}
            disabled={!categoryCounts[category.key]}
            onClick={() => selectCategory(category.key)}
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
            ...(model.mode === 'semantic' || model.mode === 'cross' ? [['confirmed', '已确认'], ['observed', '已观察']] : [['observed', '事实关系']]),
            ['inferred', '推断关系'],
          ].map(([key, label]) => (
            <button
              key={key}
              type="button"
              className={activeRelationType === key ? 'active' : ''}
              onClick={() => selectRelationType(key)}
            >
              {label} <small>{relationCounts[key]}</small>
            </button>
          ))}
        </div>
        <div className="dataset-understanding-relation-legend" aria-label="关系图例">
          {model.mode === 'semantic' || model.mode === 'cross' ? <span><i className="confirmed" />绿线 · 已确认</span> : null}
          <span><i className="observed" />实线 · 已观察</span>
          <span><i className="inferred" />虚线 · 待验证</span>
        </div>
      </div>

      {model.mode === 'semantic' || model.mode === 'cross' ? (
        <div className="dataset-understanding-local-toolbar" aria-label="局部图深度">
          <span>图谱范围</span>
          {[['all', '全局'], [1, '一跳'], [2, '两跳']].map(([depth, label]) => (
            <button
              key={depth}
              type="button"
              className={focusDepth === depth ? 'active' : ''}
              onClick={() => selectFocusDepth(depth)}
            >
              {label}
            </button>
          ))}
          <small>{focusDepth === 'all' ? '查看全部业务网络' : `以“${selectedNode?.name || model.title}”为中心聚焦`}</small>
        </div>
      ) : null}

      <div className="dataset-understanding-local-toolbar" aria-label="图谱节点密度">
        <span>节点密度</span>
        {[
          ['auto', `自适应（${responsiveDensity === 'compact' ? '精简' : '标准'}）`],
          ['compact', '精简'],
          ['standard', '标准'],
          ['expanded', '展开'],
        ].map(([key, label]) => (
          <button
            key={key}
            type="button"
            className={densityPreference === key ? 'active' : ''}
            onClick={() => setDensityPreference(key)}
          >
            {label}
          </button>
        ))}
        <small>{graphBudgetStatusText(activeGraph.stats)}</small>
        <button
          type="button"
          className={`dataset-understanding-focus-toggle ${focusMode ? 'active' : ''}`.trim()}
          aria-pressed={focusMode}
          onClick={() => setFocusMode((current) => !current)}
        >
          {focusMode ? '退出专注' : '专注图谱'}
        </button>
      </div>

      <div className="dataset-understanding-canvas-grid">
        <div className="dataset-understanding-chart-shell">
          <div ref={chartRef} className="dataset-understanding-chart" role="img" aria-label={`${model.title} ${model.mode === 'cross' ? '跨数据集语义连接图' : model.mode === 'semantic' ? '知识连接图' : '资料来源连接图'}`} />
          {chartState === 'loading' ? <div className="dataset-understanding-chart-state">正在生成理解图谱…</div> : null}
          {chartState === 'error' ? <div className="dataset-understanding-chart-state">图谱画布加载失败，可使用右侧证据详情和下方清单。</div> : null}
          <div className="dataset-understanding-chart-hint">
            {graphBudgetStatusText(activeGraph.stats)} · 滚轮缩放 · 拖拽节点 · 点击查看证据
            {model.mode === 'cross' ? ' · 颜色代表数据集集群 · 菱形代表共享节点' : model.mode === 'semantic' ? ' · 字段按对象公平分配' : ' · 低质量标签已移入待解释清单'}
          </div>
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
                <div><dt>证据等级</dt><dd>{relationClassLabel(selectedLink.evidenceClass || selectedLink.type)}</dd></div>
                <div><dt>关系类型</dt><dd>{selectedLink.relationType || (selectedLink.structural ? '结构归属' : '相关关系')}</dd></div>
                {model.mode === 'cross' ? <div><dt>关系语义</dt><dd>{selectedLink.relationSemantics === 'similarity' ? '相似线索（不折叠）' : selectedLink.relationSemantics === 'identity' ? '确定身份共享' : selectedLink.relationSemantics === 'reference' ? '明确引用' : '结构归属'}</dd></div> : null}
                <div><dt>置信度</dt><dd>{Math.round(selectedLink.confidence * 100)}%</dd></div>
                <div><dt>节点范围</dt><dd>{selectedLink.rootRelation ? '数据集归属关系' : '知识网络交叉关系'}</dd></div>
                {model.mode === 'cross' ? <div><dt>可见来源</dt><dd>{selectedLink.supportingDatasets?.map((item) => item.title).join(' · ') || '未返回'}</dd></div> : null}
                {model.mode === 'cross' ? <div><dt>可见贡献数</dt><dd>{selectedLink.visibleContributionCount}</dd></div> : null}
                {model.mode === 'cross' ? <div><dt>匹配依据</dt><dd>{selectedLink.reason}</dd></div> : null}
              </dl>
              {selectedLink.relationSemantics === 'similarity' || selectedLink.type === 'inferred' ? (
                <small className="dataset-understanding-honesty-note">虚线只是相似线索，不折叠端点，也不升级为确定身份或已确认关系。</small>
              ) : null}
            </>
          ) : (
            <NodeInspector
              model={model}
              node={selectedNode}
              onSelectLink={setSelectedLinkId}
              onFocusDataset={focusDatasetCluster}
            />
          )}
        </aside>
      </div>

      <details className="dataset-understanding-ledger">
        <summary>查看图谱证据清单</summary>
        <div className="dataset-understanding-ledger-groups">
          <section>
            <h5>节点证据</h5>
            <div>
              {model.nodes.filter((node, index) => (
                model.mode === 'cross' ? !node.rootDataset : index > 0
              )).map((node) => (
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
