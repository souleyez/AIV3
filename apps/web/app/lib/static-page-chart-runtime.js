export const STATIC_PAGE_CHART_RUNTIMES = ['deterministic', 'echarts'];

const DEFAULT_CHART_RUNTIME = 'deterministic';
const RUNTIME_SET = new Set(STATIC_PAGE_CHART_RUNTIMES);
const BASIC_CHART_OPTION_KEYS = new Set([
  'showLegend',
  'showAxis',
  'valueFormat',
  'dataKey',
  'categoryKey',
  'labelKey',
  'valueKey',
  'seriesKey',
  'chartType',
]);
const ECHARTS_TOP_LEVEL_KEYS = new Set([
  'animation',
  'aria',
  'backgroundColor',
  'color',
  'dataset',
  'grid',
  'legend',
  'series',
  'title',
  'tooltip',
  'xAxis',
  'yAxis',
  'radiusAxis',
  'angleAxis',
  'polar',
  'radar',
  'visualMap',
]);
const ECHARTS_SERIES_TYPES = new Set([
  'bar',
  'line',
  'pie',
  'scatter',
  'gauge',
  'radar',
  'heatmap',
  'treemap',
]);
const UNSAFE_STRING_PATTERN = /<\s*\/?\s*[a-z][^>]*>|javascript\s*:|data\s*:\s*text\/html|https?:\/\/|@import|expression\s*\(|\bon[a-z]+\s*=/i;
const DANGEROUS_KEYS = new Set(['__proto__', 'prototype', 'constructor', 'renderItem']);
const CHART_DATA_KEYS = ['data', 'values', 'rows', 'sampleData', 'sample_data', 'items'];

function isPlainObject(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function isSafeString(value) {
  return !UNSAFE_STRING_PATTERN.test(value);
}

function sanitizeJsonValue(value) {
  if (value === null) return null;
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number') return Number.isFinite(value) ? value : undefined;
  if (typeof value === 'string') return isSafeString(value) ? value : undefined;
  if (Array.isArray(value)) {
    const items = value
      .map((item) => sanitizeJsonValue(item))
      .filter((item) => item !== undefined);
    return items;
  }
  if (isPlainObject(value)) {
    const output = {};
    Object.entries(value).forEach(([key, item]) => {
      if (DANGEROUS_KEYS.has(key) || /^on[A-Z_:-]?/i.test(key)) return;
      const sanitized = sanitizeJsonValue(item);
      if (sanitized !== undefined) output[key] = sanitized;
    });
    return output;
  }
  return undefined;
}

function sanitizeBasicChartOptions(chartOptions) {
  const sanitized = sanitizeJsonValue(chartOptions);
  if (!isPlainObject(sanitized)) return {};
  const output = {};
  Object.entries(sanitized).forEach(([key, value]) => {
    if (BASIC_CHART_OPTION_KEYS.has(key)) output[key] = value;
  });
  return output;
}

function sanitizeEChartsSeries(series) {
  const list = Array.isArray(series) ? series : [series];
  return list
    .filter(isPlainObject)
    .map((item) => {
      const type = ECHARTS_SERIES_TYPES.has(item.type) ? item.type : null;
      if (!type) return null;
      return {
        ...item,
        type,
      };
    })
    .filter(Boolean);
}

function sanitizeEChartsOptions(chartOptions) {
  const sanitized = sanitizeJsonValue(chartOptions);
  if (!isPlainObject(sanitized)) return {};
  const output = {};
  Object.entries(sanitized).forEach(([key, value]) => {
    if (!ECHARTS_TOP_LEVEL_KEYS.has(key)) return;
    if (key === 'series') {
      const series = sanitizeEChartsSeries(value);
      if (series.length > 0) output.series = series;
      return;
    }
    output[key] = value;
  });
  return output;
}

function firstArrayCandidate(value) {
  if (Array.isArray(value)) return value;
  if (!isPlainObject(value)) return [];
  for (const key of CHART_DATA_KEYS) {
    if (Array.isArray(value[key])) return value[key];
  }
  return [];
}

function chartRowLabel(row, index) {
  if (isPlainObject(row)) {
    return row.label || row.name || row.month || row.date || row.period || row.category || row.title || row.x || row['月份'] || row['日期'] || row['分类'] || `项${index + 1}`;
  }
  return `项${index + 1}`;
}

function chartRowValue(row) {
  if (typeof row === 'number' && Number.isFinite(row)) return row;
  if (!isPlainObject(row)) return null;
  const candidates = [
    row.value,
    row.amount,
    row.count,
    row.score,
    row.rate,
    row.total,
    row.y,
    row['订单金额'],
    row['金额'],
    row['收入'],
    row['数量'],
  ];
  const matched = candidates.find((value) => value !== undefined && value !== null && value !== '');
  const numeric = Number(String(matched ?? '').replace(/[%,$，,]/g, '').trim());
  return Number.isFinite(numeric) ? numeric : null;
}

function echartsSeriesType(visualizationType) {
  if (visualizationType === 'line-chart') return 'line';
  if (visualizationType === 'donut-chart') return 'pie';
  if (visualizationType === 'risk-matrix') return 'scatter';
  return 'bar';
}

function defaultEchartsOption(visualizationType, rows) {
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
      data: rows.map((row) => row.value),
    }],
  };
}

export function normalizeChartRuntime(value) {
  return RUNTIME_SET.has(value) ? value : DEFAULT_CHART_RUNTIME;
}

export function normalizeChartRuntimeFromVisualization(visualization = {}, chartOptions = {}) {
  return normalizeChartRuntime(
    visualization.chartRuntime
      || visualization.runtime
      || chartOptions.chartRuntime
      || chartOptions.runtime
      || DEFAULT_CHART_RUNTIME,
  );
}

export function sanitizeStaticPageChartOptions(chartOptions = {}, { runtime = DEFAULT_CHART_RUNTIME } = {}) {
  const normalizedRuntime = normalizeChartRuntime(runtime);
  if (normalizedRuntime === 'echarts') {
    return sanitizeEChartsOptions(chartOptions);
  }
  return sanitizeBasicChartOptions(chartOptions);
}

export function normalizeStaticPageChartRuntimeConfig(visualization = {}, chartOptions = {}) {
  const chartRuntime = normalizeChartRuntimeFromVisualization(visualization, chartOptions);
  return {
    chartRuntime,
    chartOptions: sanitizeStaticPageChartOptions(chartOptions, { runtime: chartRuntime }),
  };
}

export function normalizeStaticPageChartRows(source = []) {
  return firstArrayCandidate(source)
    .slice(0, 24)
    .map((row, index) => {
      const value = chartRowValue(row);
      if (value === null) return null;
      return {
        label: String(chartRowLabel(row, index)).trim() || `项${index + 1}`,
        value,
      };
    })
    .filter(Boolean);
}

export function staticPageChartRowsFromModule(module = {}) {
  const visualization = module?.visualization || {};
  const candidates = [
    visualization.data,
    visualization.values,
    visualization.sampleData,
    visualization.sample_data,
    visualization.rows,
    visualization.items,
    module?.dataBinding,
    module?.data,
  ];
  for (const candidate of candidates) {
    const rows = normalizeStaticPageChartRows(candidate);
    if (rows.length) return rows;
  }
  return [];
}

export function hasRenderableEchartsData(option = {}) {
  if (Array.isArray(option?.dataset?.source) && option.dataset.source.length > 0) return true;
  const series = Array.isArray(option?.series) ? option.series : [];
  return series.some((item) => Array.isArray(item?.data) && item.data.length > 0);
}

export function buildStaticPageEchartsPreviewOption(module = {}) {
  const visualization = module?.visualization || {};
  const rows = staticPageChartRowsFromModule(module);
  const fallbackOption = defaultEchartsOption(visualization.type, rows);
  const chartOptions = sanitizeStaticPageChartOptions(visualization.chartOptions || {}, {
    runtime: 'echarts',
  });
  return {
    animation: false,
    tooltip: { trigger: visualization.type === 'donut-chart' ? 'item' : 'axis' },
    grid: { left: 28, right: 16, top: 22, bottom: 28, containLabel: true },
    ...fallbackOption,
    ...chartOptions,
  };
}
