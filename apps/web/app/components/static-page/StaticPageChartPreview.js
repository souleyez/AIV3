'use client';

import { useEffect, useRef } from 'react';
import {
  buildStaticPageEchartsPreviewOption,
  hasRenderableEchartsData,
  staticPageChartRowsFromModule,
} from '../../lib/static-page-chart-runtime';

const PREVIEWABLE_STATIC_TYPES = new Set([
  'kpi-cards',
  'bar-chart',
  'line-chart',
  'donut-chart',
  'table',
  'timeline',
  'risk-matrix',
]);

function formatValue(value) {
  return Number.isInteger(value) ? value.toLocaleString('zh-CN') : Number(value).toLocaleString('zh-CN', {
    maximumFractionDigits: 2,
  });
}

function rowMax(rows) {
  return Math.max(1, ...rows.map((row) => Math.abs(Number(row.value) || 0)));
}

function linePoints(rows) {
  const values = rows.map((row) => Number(row.value) || 0);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  return rows.map((row, index) => {
    const x = rows.length > 1 ? 6 + (index / (rows.length - 1)) * 88 : 50;
    const y = 88 - (((Number(row.value) || 0) - min) / span) * 68;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  }).join(' ');
}

function DeterministicChartPreview({ module, rows, compact }) {
  const type = module?.visualization?.type || 'text-insight';
  if (!PREVIEWABLE_STATIC_TYPES.has(type) || rows.length === 0) return null;
  const max = rowMax(rows);
  const previewRows = rows.slice(0, compact ? 4 : 6);

  if (type === 'kpi-cards') {
    return (
      <div className={`static-page-chart-preview deterministic kpi${compact ? ' compact' : ''}`}>
        {previewRows.slice(0, 4).map((row) => (
          <div key={row.label} className="static-page-mini-kpi">
            <strong>{formatValue(row.value)}</strong>
            <span>{row.label}</span>
          </div>
        ))}
      </div>
    );
  }

  if (type === 'table') {
    return (
      <div className={`static-page-chart-preview deterministic table${compact ? ' compact' : ''}`}>
        {previewRows.map((row) => (
          <div key={row.label} className="static-page-mini-row">
            <span>{row.label}</span>
            <strong>{formatValue(row.value)}</strong>
          </div>
        ))}
      </div>
    );
  }

  if (type === 'timeline') {
    return (
      <ol className={`static-page-chart-preview deterministic timeline${compact ? ' compact' : ''}`}>
        {previewRows.map((row) => (
          <li key={row.label}>
            <span>{row.label}</span>
            <strong>{formatValue(row.value)}</strong>
          </li>
        ))}
      </ol>
    );
  }

  if (type === 'line-chart') {
    return (
      <div className={`static-page-chart-preview deterministic${compact ? ' compact' : ''}`}>
        <svg className="static-page-mini-svg" viewBox="0 0 100 100" role="img" aria-label="基础折线图预览">
          <polyline className="static-page-mini-line" points={linePoints(rows)} />
          {previewRows.map((row, index) => {
            const [x, y] = linePoints(rows).split(' ')[index].split(',');
            return <circle key={row.label} cx={x} cy={y} r="2.8" />;
          })}
        </svg>
      </div>
    );
  }

  return (
    <div className={`static-page-chart-preview deterministic bars${compact ? ' compact' : ''}`}>
      {previewRows.map((row) => (
        <div key={row.label} className="static-page-mini-bar">
          <span>{row.label}</span>
          <i style={{ '--bar-width': `${Math.max(4, (Math.abs(row.value) / max) * 100)}%` }} />
          <strong>{formatValue(row.value)}</strong>
        </div>
      ))}
    </div>
  );
}

export default function StaticPageChartPreview({
  module,
  compact = false,
}) {
  const chartRef = useRef(null);
  const chartRuntime = module?.visualization?.chartRuntime || 'deterministic';
  const option = buildStaticPageEchartsPreviewOption(module);
  const optionText = JSON.stringify(option);
  const canRender = chartRuntime === 'echarts' && hasRenderableEchartsData(option);
  const rows = staticPageChartRowsFromModule(module);

  useEffect(() => {
    if (!canRender || !chartRef.current) return undefined;

    let chart = null;
    let disposed = false;
    let resize = () => {};

    import('echarts').then((echarts) => {
      if (disposed || !chartRef.current) return;
      echarts.getInstanceByDom(chartRef.current)?.dispose();
      chart = echarts.init(chartRef.current, null, { renderer: 'canvas' });
      chart.setOption(JSON.parse(optionText), true);
      resize = () => chart?.resize();
      window.addEventListener('resize', resize);
    });

    return () => {
      disposed = true;
      window.removeEventListener('resize', resize);
      chart?.dispose();
    };
  }, [canRender, optionText]);

  if (chartRuntime !== 'echarts') {
    return <DeterministicChartPreview module={module} rows={rows} compact={compact} />;
  }

  if (!canRender) {
    return (
      <div className={`static-page-chart-preview empty${compact ? ' compact' : ''}`}>
        <span>ECharts 高级图表</span>
        <strong>等待真实数据</strong>
        <p>该模块已切到高级图表运行时，但还没有可渲染的 series 或 dataset。</p>
      </div>
    );
  }

  return (
    <div className={`static-page-chart-preview${compact ? ' compact' : ''}`}>
      <div className="static-page-chart-preview-head">
        <span>ECharts 高级图表</span>
        <strong>{module?.visualization?.label || module?.visualization?.type || '图表预览'}</strong>
      </div>
      <div ref={chartRef} className="static-page-chart-preview-canvas" />
    </div>
  );
}
