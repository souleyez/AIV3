'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import {
  buildDatasetUnderstandingGraph,
  DATASET_GRAPH_CATEGORIES,
} from '../lib/dataset-understanding-graph';

function compactNumber(value) {
  const number = Number(value) || 0;
  if (number >= 10000) return `${(number / 10000).toFixed(number >= 100000 ? 0 : 1)} 万`;
  return number.toLocaleString('zh-CN');
}

function graphCategory(model, key) {
  return model.categories.find((category) => category.key === key) || model.categories[0];
}

function optionForModel(model, activeCategory, activeRelationType) {
  const visibleNodes = activeCategory === 'all'
    ? model.nodes
    : model.nodes.filter((node) => node.kind === 'dataset' || node.kind === activeCategory);
  const visibleIds = new Set(visibleNodes.map((node) => node.id));
  const visibleLinks = model.links.filter((link) => (
    visibleIds.has(link.source)
      && visibleIds.has(link.target)
      && (activeRelationType === 'all' || link.type === activeRelationType)
  ));

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
          const relationType = params.data?.type === 'inferred' ? '推断关系' : '事实关系';
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
        },
        itemStyle: {
          color: graphCategory(model, node.kind).color,
          borderColor: node.kind === 'dataset' ? '#ffffff' : 'rgba(255,255,255,0.52)',
          borderWidth: node.kind === 'dataset' ? 2 : 1,
          shadowBlur: node.kind === 'dataset' ? 28 : 12,
          shadowColor: `${graphCategory(model, node.kind).color}55`,
        },
      })),
      links: visibleLinks.map((link) => ({
        ...link,
        lineStyle: {
          color: link.rootRelation
            ? 'rgba(148, 163, 184, 0.32)'
            : link.type === 'inferred'
              ? 'rgba(251, 191, 36, 0.78)'
              : 'rgba(94, 234, 212, 0.84)',
          type: link.type === 'inferred' ? 'dashed' : 'solid',
          width: link.rootRelation ? 0.75 : link.type === 'inferred' ? 1.35 : 1.8,
          opacity: link.rootRelation ? 0.24 : link.type === 'inferred' ? 0.62 : 0.76,
          curveness: link.rootRelation ? 0.02 : 0.12,
        },
      })),
      force: {
        repulsion: 182,
        gravity: 0.085,
        edgeLength: [58, 126],
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
      edgeLabel: { show: false },
      emphasis: {
        focus: 'adjacency',
        lineStyle: { width: 3, opacity: 1 },
        label: { color: '#ffffff', fontWeight: 800 },
      },
    }],
  };
}

function PipelineStage({ stage, index, last }) {
  return (
    <article className={`dataset-understanding-stage ${stage.status}`.trim()}>
      <div className="dataset-understanding-stage-index">{String(index + 1).padStart(2, '0')}</div>
      <div>
        <span>{stage.label}</span>
        <strong>{stage.value}</strong>
        <small>{stage.detail}</small>
      </div>
      {!last ? <i aria-hidden="true">→</i> : null}
    </article>
  );
}

export default function DatasetUnderstandingGraph({ dataset, documents = [] }) {
  const chartRef = useRef(null);
  const model = useMemo(
    () => buildDatasetUnderstandingGraph(dataset, documents),
    [dataset, documents],
  );
  const [activeCategory, setActiveCategory] = useState('all');
  const [activeRelationType, setActiveRelationType] = useState('all');
  const [selectedNodeId, setSelectedNodeId] = useState('');
  const [selectedLinkId, setSelectedLinkId] = useState('');
  const [chartState, setChartState] = useState('loading');
  const selectedNode = model.nodes.find((node) => node.id === selectedNodeId) || model.nodes[0] || null;
  const selectedLink = model.links.find((link) => link.id === selectedLinkId) || null;
  const selectedLinkSource = selectedLink
    ? model.nodes.find((node) => node.id === selectedLink.source) || null
    : null;
  const selectedLinkTarget = selectedLink
    ? model.nodes.find((node) => node.id === selectedLink.target) || null
    : null;
  const categoryCounts = useMemo(() => Object.fromEntries(
    DATASET_GRAPH_CATEGORIES.map((category) => [
      category.key,
      model.nodes.filter((node) => node.kind === category.key).length,
    ]),
  ), [model.nodes]);
  const relationCounts = useMemo(() => ({
    all: model.links.length,
    observed: model.metrics.observedRelationCount,
    inferred: model.metrics.inferredRelationCount,
  }), [model.links.length, model.metrics.observedRelationCount, model.metrics.inferredRelationCount]);
  const chartOption = useMemo(
    () => optionForModel(model, activeCategory, activeRelationType),
    [model, activeCategory, activeRelationType],
  );
  const chartSignature = JSON.stringify(chartOption, (key, value) => typeof value === 'function' ? String(value) : value);

  useEffect(() => {
    setSelectedNodeId(model.nodes[0]?.id || '');
    setSelectedLinkId('');
    setActiveCategory('all');
    setActiveRelationType('all');
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
        }
        if (params.dataType === 'edge' && params.data?.id) {
          setSelectedLinkId(params.data.id);
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
          <h3>选择左侧数据集，查看系统理解</h3>
          <p>这里会根据现有数据展示解析清洗链路、知识点、章节结构、资料类型和可追溯连线。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="dataset-understanding-panel" aria-label={`${model.title} 数据集理解图谱`}>
      <header className="dataset-understanding-head">
        <div>
          <span className="dataset-understanding-eyebrow">DATASET INTELLIGENCE · 基于现有数据</span>
          <h3>{model.title}</h3>
          <p>实线表示已有字段可证实关系；虚线表示同组共现或文本亲和推断，可点击连线核对依据。</p>
        </div>
        <div className="dataset-understanding-metrics" aria-label="数据集理解指标">
          <span><small>资料</small><strong>{model.metrics.documentCount}</strong></span>
          <span><small>知识线索</small><strong>{model.metrics.knowledgeCount}</strong></span>
          <span className="relations"><small>交叉关系</small><strong>{model.metrics.crossNodeRelationCount}</strong></span>
          <span><small>可检索</small><strong>{model.metrics.readyDocumentCount}</strong></span>
          <span className={model.metrics.attentionDocumentCount ? 'attention' : ''}>
            <small>待关注</small><strong>{model.metrics.attentionDocumentCount}</strong>
          </span>
        </div>
      </header>

      <div className="dataset-understanding-pipeline" aria-label="数据处理链路">
        {model.pipeline.map((stage, index) => (
          <PipelineStage key={stage.key} stage={stage} index={index} last={index === model.pipeline.length - 1} />
        ))}
      </div>

      <div className="dataset-understanding-filter" aria-label="图谱类型筛选">
        <button
          type="button"
          className={activeCategory === 'all' ? 'active' : ''}
          onClick={() => setActiveCategory('all')}
        >
          全部 <small>{Math.max(0, model.nodes.length - 1)}</small>
        </button>
        {DATASET_GRAPH_CATEGORIES.filter((category) => category.key !== 'dataset').map((category) => (
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
        <div className="dataset-understanding-relation-filter" aria-label="关系可信度筛选">
          <span>关系视图</span>
          {[
            ['all', '全部关系'],
            ['observed', '事实关系'],
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
          <span><i className="observed" />实线 · 可证实</span>
          <span><i className="inferred" />虚线 · 待验证</span>
        </div>
      </div>

      <div className="dataset-understanding-canvas-grid">
        <div className="dataset-understanding-chart-shell">
          <div ref={chartRef} className="dataset-understanding-chart" role="img" aria-label={`${model.title} 知识连接图`} />
          {chartState === 'loading' ? <div className="dataset-understanding-chart-state">正在生成理解图谱…</div> : null}
          {chartState === 'error' ? <div className="dataset-understanding-chart-state">图谱画布加载失败，可使用右侧证据详情和下方清单。</div> : null}
          <div className="dataset-understanding-chart-hint">滚轮缩放 · 拖拽节点 · 点击查看证据</div>
        </div>

        <aside className="dataset-understanding-evidence" aria-live="polite">
          {selectedLink ? (
            <>
              <span>当前关系依据</span>
              <div className={`dataset-understanding-node-kind relation ${selectedLink.type}`.trim()}>
                <i />
                {selectedLink.type === 'inferred' ? '推断关系' : '事实关系'}
              </div>
              <h4>{selectedLinkSource?.name || '来源节点'} <em>{selectedLink.relation}</em> {selectedLinkTarget?.name || '目标节点'}</h4>
              <p>{selectedLink.evidence}</p>
              <dl>
                <div><dt>关系类型</dt><dd>{selectedLink.type === 'inferred' ? '前端推断，待后端关系抽取验证' : '现有接口字段可直接证实'}</dd></div>
                <div><dt>置信度</dt><dd>{Math.round(selectedLink.confidence * 100)}%</dd></div>
                <div><dt>节点范围</dt><dd>{selectedLink.rootRelation ? '数据集归属关系' : '知识网络交叉关系'}</dd></div>
              </dl>
              {selectedLink.type === 'inferred' ? (
                <small className="dataset-understanding-honesty-note">虚线不等同于已确认的业务因果或实体关系。</small>
              ) : null}
            </>
          ) : (
            <>
              <span>当前节点证据</span>
              <div className="dataset-understanding-node-kind">
                <i style={{ background: graphCategory(model, selectedNode?.kind).color }} />
                {graphCategory(model, selectedNode?.kind).name}
              </div>
              <h4>{selectedNode?.name || model.title}</h4>
              <p>{selectedNode?.detail || '点击图谱节点查看系统为什么展示这条信息。'}</p>
              <dl>
                <div><dt>数据来源</dt><dd>{selectedNode?.evidence || '数据集摘要接口'}</dd></div>
                <div><dt>处理状态</dt><dd>{selectedNode?.status || '已返回'}</dd></div>
                <div><dt>估算字数</dt><dd>{model.metrics.estimatedWordCount ? compactNumber(model.metrics.estimatedWordCount) : '接口未返回'}</dd></div>
              </dl>
              {model.emptyKnowledgeMessage ? <small className="dataset-understanding-honesty-note">{model.emptyKnowledgeMessage}</small> : null}
            </>
          )}
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
                  <button key={link.id} type="button" onClick={() => setSelectedLinkId(link.id)}>
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
