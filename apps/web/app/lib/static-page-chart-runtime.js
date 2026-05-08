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

