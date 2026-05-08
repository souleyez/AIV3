'use client';

import { useEffect, useState } from 'react';
import {
  STATIC_PAGE_DATA_SOURCE_TYPES,
  STATIC_PAGE_VISUALIZATION_TYPES,
  buildStaticPageModuleUpdateOperation,
} from '../../lib/static-page-draft';

function stopControlPropagation(event) {
  event.stopPropagation();
}

function visualizationLabel(type) {
  return STATIC_PAGE_VISUALIZATION_TYPES.find((item) => item.type === type)?.label || type;
}

function chartRuntimeLabel(runtime) {
  return runtime === 'echarts' ? 'ECharts 高级' : '基础静态';
}

function candidateFieldPath(candidate) {
  return candidate?.fieldPath || candidate?.field_path || candidate?.field || '';
}

function candidateLabel(candidate) {
  return candidate?.label || candidateFieldPath(candidate);
}

function moduleDataRows(module) {
  return Array.isArray(module?.visualization?.data) ? module.visualization.data : [];
}

function dataRowsToText(rows = []) {
  return rows
    .map((row) => `${row.label || row.name || ''}, ${row.value ?? row.amount ?? ''}`.trim())
    .filter(Boolean)
    .join('\n');
}

function parseDataRowsText(text = '') {
  return String(text)
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line, index) => {
      const [labelPart, valuePart] = line.split(/[,，\t|]/);
      const label = (labelPart || `项${index + 1}`).trim();
      const rawValue = (valuePart || '').replace(/[%,$，,]/g, '').trim();
      const value = Number(rawValue);
      if (!Number.isFinite(value)) return null;
      return { label, value };
    })
    .filter(Boolean);
}

function echartsSeriesType(visualizationType) {
  if (visualizationType === 'line-chart') return 'line';
  if (visualizationType === 'donut-chart') return 'pie';
  if (visualizationType === 'risk-matrix') return 'scatter';
  return 'bar';
}

function buildEchartsOptions(visualizationType, rows = []) {
  if (!rows.length) return {};
  const seriesType = echartsSeriesType(visualizationType);
  if (seriesType === 'pie') {
    return {
      tooltip: { trigger: 'item' },
      series: [{
        type: 'pie',
        radius: ['46%', '72%'],
        data: rows.map((row) => ({ name: row.label, value: row.value })),
      }],
    };
  }
  return {
    tooltip: { trigger: 'axis' },
    xAxis: { type: 'category', data: rows.map((row) => row.label) },
    yAxis: { type: 'value' },
    series: [{
      type: seriesType,
      name: visualizationLabel(visualizationType),
      data: rows.map((row) => row.value),
    }],
  };
}

export default function StaticPageModuleCard({
  module,
  onApplyOperation,
  compact = false,
  fieldCandidates = [],
}) {
  const fieldListId = `static-page-field-candidates-${module.id}`;
  const chartRuntime = module.visualization?.chartRuntime || 'deterministic';
  const [draft, setDraft] = useState({
    title: module.title || '',
    content: module.content || '',
    dataLabel: module.dataBinding?.label || '',
    dataSourceId: module.dataBinding?.sourceId || 'model',
    dataField: module.dataBinding?.fieldPath || '',
    visualizationType: module.visualization?.type || 'text-insight',
    chartRuntime,
    dataRowsText: dataRowsToText(moduleDataRows(module)),
  });

  useEffect(() => {
    setDraft({
      title: module.title || '',
      content: module.content || '',
      dataLabel: module.dataBinding?.label || '',
      dataSourceId: module.dataBinding?.sourceId || 'model',
      dataField: module.dataBinding?.fieldPath || '',
      visualizationType: module.visualization?.type || 'text-insight',
      chartRuntime: module.visualization?.chartRuntime || 'deterministic',
      dataRowsText: dataRowsToText(moduleDataRows(module)),
    });
  }, [
    module.id,
    module.title,
    module.content,
    module.dataBinding?.label,
    module.dataBinding?.sourceId,
    module.dataBinding?.fieldPath,
    module.visualization?.type,
    module.visualization?.chartRuntime,
    JSON.stringify(module.visualization?.data || []),
  ]);

  const dirty = draft.title !== (module.title || '')
    || draft.content !== (module.content || '')
    || draft.dataLabel !== (module.dataBinding?.label || '')
    || draft.dataSourceId !== (module.dataBinding?.sourceId || 'model')
    || draft.dataField !== (module.dataBinding?.fieldPath || '')
    || draft.visualizationType !== (module.visualization?.type || 'text-insight')
    || draft.chartRuntime !== (module.visualization?.chartRuntime || 'deterministic')
    || draft.dataRowsText !== dataRowsToText(moduleDataRows(module));

  function applyModuleEdit() {
    if (!dirty) return;
    const sourcePreset = STATIC_PAGE_DATA_SOURCE_TYPES.find((item) => item.sourceId === draft.dataSourceId)
      || STATIC_PAGE_DATA_SOURCE_TYPES[0];
    const dataRows = parseDataRowsText(draft.dataRowsText);
    const chartOptions = draft.chartRuntime === 'echarts'
      ? buildEchartsOptions(draft.visualizationType, dataRows)
      : {};
    onApplyOperation?.(
      buildStaticPageModuleUpdateOperation(module, {
        title: draft.title.trim() || module.title || '未命名模块',
        content: draft.content.trim() || module.content || '补充这个模块要表达的内容。',
        dataBinding: {
          type: sourcePreset.type,
          sourceId: sourcePreset.sourceId,
          label: draft.dataLabel.trim() || '数据绑定待确认',
          fieldPath: draft.dataField.trim() || null,
        },
        visualization: {
          type: draft.visualizationType,
          label: visualizationLabel(draft.visualizationType),
          chartRuntime: draft.chartRuntime,
          chartOptions,
          data: dataRows,
        },
      }),
    );
  }

  return (
    <article className={`static-page-module-card${compact ? ' compact' : ''}`}>
      <div className="static-page-module-drag-handle" title="拖动模块">
        <span>{module.role}</span>
        <strong>{module.title}</strong>
      </div>
      <p>{module.content}</p>
      <div className="static-page-module-meta">
        <span>数据：{module.dataBinding?.label || '未绑定'}</span>
        <span>图表：{module.visualization?.label || module.visualization?.type || '未选择'}</span>
        <span className={`static-page-runtime-chip ${chartRuntime}`}>运行：{chartRuntimeLabel(chartRuntime)}</span>
      </div>
      <div className="static-page-module-layout">
        x{module.layout?.x ?? 0} y{module.layout?.y ?? 0} · {module.layout?.w ?? 1}x{module.layout?.h ?? 1}
      </div>
      {onApplyOperation ? (
        <details className="static-page-module-editor" onPointerDown={stopControlPropagation}>
          <summary>微调模块</summary>
          <div className="static-page-module-editor-grid">
            <label>
              <span>标题</span>
              <input
                value={draft.title}
                onChange={(event) => setDraft((current) => ({ ...current, title: event.target.value }))}
                onBlur={applyModuleEdit}
              />
            </label>
            <label>
              <span>数据名称</span>
              <input
                value={draft.dataLabel}
                onChange={(event) => setDraft((current) => ({ ...current, dataLabel: event.target.value }))}
                onBlur={applyModuleEdit}
              />
            </label>
            <label>
              <span>数据源</span>
              <select
                value={draft.dataSourceId}
                onChange={(event) => setDraft((current) => ({ ...current, dataSourceId: event.target.value }))}
                onBlur={applyModuleEdit}
              >
                {STATIC_PAGE_DATA_SOURCE_TYPES.map((item) => (
                  <option key={item.sourceId} value={item.sourceId}>{item.label}</option>
                ))}
              </select>
            </label>
            <label>
              <span>字段</span>
              <input
                value={draft.dataField}
                placeholder="自动识别"
                list={fieldListId}
                onChange={(event) => setDraft((current) => ({ ...current, dataField: event.target.value }))}
                onBlur={applyModuleEdit}
              />
              {fieldCandidates.length > 0 ? (
                <datalist id={fieldListId}>
                  {fieldCandidates.map((candidate) => {
                    const fieldPath = candidateFieldPath(candidate);
                    if (!fieldPath) return null;
                    return (
                      <option key={`${candidate?.sourceId || 'field'}:${fieldPath}`} value={fieldPath}>
                        {candidateLabel(candidate)}
                      </option>
                    );
                  })}
                </datalist>
              ) : null}
            </label>
            <label className="wide">
              <span>内容</span>
              <textarea
                value={draft.content}
                rows={compact ? 2 : 3}
                onChange={(event) => setDraft((current) => ({ ...current, content: event.target.value }))}
                onBlur={applyModuleEdit}
              />
            </label>
            <label>
              <span>图表</span>
              <select
                value={draft.visualizationType}
                onChange={(event) => setDraft((current) => ({ ...current, visualizationType: event.target.value }))}
                onBlur={applyModuleEdit}
              >
                {STATIC_PAGE_VISUALIZATION_TYPES.map((item) => (
                  <option key={item.type} value={item.type}>{item.label}</option>
                ))}
              </select>
            </label>
            <label>
              <span>运行</span>
              <select
                value={draft.chartRuntime}
                onChange={(event) => setDraft((current) => ({ ...current, chartRuntime: event.target.value }))}
                onBlur={applyModuleEdit}
              >
                <option value="deterministic">基础静态</option>
                <option value="echarts">ECharts 高级</option>
              </select>
            </label>
            <label className="wide">
              <span>数据行（每行：名称, 数值）</span>
              <textarea
                value={draft.dataRowsText}
                rows={compact ? 2 : 3}
                placeholder={'一月, 1200\n二月, 1380'}
                onChange={(event) => setDraft((current) => ({ ...current, dataRowsText: event.target.value }))}
                onBlur={applyModuleEdit}
              />
            </label>
            <button type="button" className="ghost-btn compact-action-btn" onClick={applyModuleEdit} disabled={!dirty}>
              应用调整
            </button>
          </div>
        </details>
      ) : null}
    </article>
  );
}
