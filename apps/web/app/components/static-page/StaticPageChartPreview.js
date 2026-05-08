'use client';

import { useEffect, useRef } from 'react';
import { sanitizeStaticPageChartOptions } from '../../lib/static-page-chart-runtime';

function hasSeriesData(option) {
  if (Array.isArray(option?.dataset?.source) && option.dataset.source.length > 0) return true;
  const series = Array.isArray(option?.series) ? option.series : [];
  return series.some((item) => Array.isArray(item?.data) && item.data.length > 0);
}

function buildPreviewOption(module) {
  const chartOptions = sanitizeStaticPageChartOptions(module?.visualization?.chartOptions || {}, {
    runtime: 'echarts',
  });
  return {
    animation: false,
    tooltip: { trigger: module?.visualization?.type === 'donut-chart' ? 'item' : 'axis' },
    grid: { left: 28, right: 16, top: 22, bottom: 28, containLabel: true },
    ...chartOptions,
  };
}

export default function StaticPageChartPreview({
  module,
  compact = false,
}) {
  const chartRef = useRef(null);
  const chartRuntime = module?.visualization?.chartRuntime || 'deterministic';
  const option = buildPreviewOption(module);
  const optionText = JSON.stringify(option);
  const canRender = chartRuntime === 'echarts' && hasSeriesData(option);

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

  if (chartRuntime !== 'echarts') return null;

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

