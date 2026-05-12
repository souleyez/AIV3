import {
  normalizeChartRuntimeFromVisualization,
  sanitizeStaticPageChartOptions,
} from './static-page-chart-runtime.js';

export const STATIC_PAGE_STYLE_DIRECTIONS = [
  {
    key: 'decision-brief',
    label: '高层决策简报',
    description: '结论先行，强调 KPI、风险和行动建议。',
  },
  {
    key: 'client-delivery',
    label: '客户交付报告',
    description: '结构清晰，解释充分，适合项目交付和售前方案。',
  },
  {
    key: 'data-command',
    label: '数据运营看板',
    description: '指标密度更高，强调趋势、对比和构成。',
  },
];

export const STATIC_PAGE_VISUALIZATION_TYPES = [
  { type: 'headline', label: '大标题 + 关键结论' },
  { type: 'kpi-cards', label: '关键指标卡' },
  { type: 'bar-chart', label: '分类对比柱状图' },
  { type: 'line-chart', label: '趋势折线图' },
  { type: 'donut-chart', label: '占比环图' },
  { type: 'table', label: '证据表格' },
  { type: 'timeline', label: '阶段时间线' },
  { type: 'risk-matrix', label: '风险优先级矩阵' },
  { type: 'text-insight', label: '洞察文本块' },
];

export const STATIC_PAGE_DATA_SOURCE_TYPES = [
  {
    sourceId: 'model',
    type: 'model_summary',
    label: '模型总结',
    description: '使用当前对话和页面目标生成的摘要内容。',
  },
  {
    sourceId: 'selected_scope',
    type: 'selected_scope',
    label: '当前选中范围',
    description: '优先使用左侧已选中的公开或密钥匹配数据集。',
  },
  {
    sourceId: 'evidence',
    type: 'retrieval_evidence',
    label: '检索证据',
    description: '使用当前回答命中的文档片段、解析结果或检索证据。',
  },
  {
    sourceId: 'conversation_memory',
    type: 'conversation_memory',
    label: '对话历史',
    description: '使用本终端缓存的相关历史对话。',
  },
  {
    sourceId: 'session',
    type: 'conversation_summary',
    label: '当前会话摘要',
    description: '使用当前报表/静态页会话里已经确认的上下文。',
  },
  {
    sourceId: 'dataset',
    type: 'dataset_metrics',
    label: '数据集指标摘要',
    description: '使用数据集解析出的指标、字段和统计摘要。',
  },
];

export const DEFAULT_STATIC_PAGE_MODULES = [
  {
    id: 'hero',
    role: 'hero',
    title: '核心判断',
    content: '先给出一句客户能直接带走的主结论。',
    dataBinding: {
      type: 'conversation_summary',
      label: '来自当前会话摘要',
      sourceId: 'session',
    },
    visualization: {
      type: 'headline',
      label: '大标题 + 关键结论',
    },
    layout: { x: 0, y: 0, w: 12, h: 3 },
  },
  {
    id: 'kpi',
    role: 'metrics',
    title: '关键指标',
    content: '用 2-4 个指标解释当前结论的量化依据。',
    dataBinding: {
      type: 'dataset_metrics',
      label: '来自数据集指标摘要',
      sourceId: 'dataset',
    },
    visualization: {
      type: 'kpi-cards',
      label: '关键指标卡',
    },
    layout: { x: 0, y: 3, w: 5, h: 3 },
  },
  {
    id: 'trend',
    role: 'trend',
    title: '趋势变化',
    content: '展示主要指标随时间变化的方向和拐点。',
    dataBinding: {
      type: 'time_series',
      label: '来自会话中识别的趋势数据',
      sourceId: 'evidence',
    },
    visualization: {
      type: 'line-chart',
      label: '趋势折线图',
    },
    layout: { x: 5, y: 3, w: 7, h: 3 },
  },
  {
    id: 'risk',
    role: 'risk',
    title: '风险与机会',
    content: '列出需要客户优先关注的风险点和可推进机会。',
    dataBinding: {
      type: 'conversation_evidence',
      label: '来自当前会话证据',
      sourceId: 'evidence',
    },
    visualization: {
      type: 'risk-matrix',
      label: '风险优先级矩阵',
    },
    layout: { x: 0, y: 6, w: 6, h: 4 },
  },
  {
    id: 'next-steps',
    role: 'next_steps',
    title: '建议动作',
    content: '把页面结论转成可执行的下一步安排。',
    dataBinding: {
      type: 'model_summary',
      label: '来自模型总结',
      sourceId: 'model',
    },
    visualization: {
      type: 'timeline',
      label: '阶段时间线',
    },
    layout: { x: 6, y: 6, w: 6, h: 4 },
  },
];

const GRID_COLUMNS = 12;
const DEFAULT_STYLE_DIRECTION = 'client-delivery';
const STYLE_KEYS = new Set(STATIC_PAGE_STYLE_DIRECTIONS.map((item) => item.key));
const VISUALIZATION_TYPES = new Set(STATIC_PAGE_VISUALIZATION_TYPES.map((item) => item.type));
const DATA_SOURCE_IDS = new Set(STATIC_PAGE_DATA_SOURCE_TYPES.map((item) => item.sourceId));
const IMAGE_JOB_STATUSES = new Set(['idle', 'queued', 'running', 'preview_ready', 'failed', 'confirmed', 'stale']);
const STALEABLE_PREVIEW_CONTRACT_STATUSES = new Set([
  'queued',
  'running',
  'preview_ready',
  'confirmed',
  'rendering',
  'rendered',
]);
const DESIGN_MUTATION_TYPES = new Set([
  'update_module',
  'add_module',
  'remove_module',
  'move_module',
  'resize_module',
  'change_visualization',
  'change_data_binding',
  'reorder_modules',
  'change_style_direction',
]);
const MODULE_TARGET_KEYWORDS = [
  { id: 'hero', keywords: ['结论', '核心', '标题', '开头', '主判断', '判断'] },
  { id: 'kpi', keywords: ['指标', 'kpi', '数字', '数据卡', '量化'] },
  { id: 'trend', keywords: ['趋势', '变化', '走势', '折线', '时间'] },
  { id: 'risk', keywords: ['风险', '机会', '隐患', '预警'] },
  { id: 'next-steps', keywords: ['建议', '动作', '下一步', '计划', '推进'] },
];
const VISUALIZATION_KEYWORDS = [
  { type: 'bar-chart', keywords: ['柱状图', '柱图', '条形图', '对比图'] },
  { type: 'line-chart', keywords: ['折线图', '趋势图', '曲线图'] },
  { type: 'donut-chart', keywords: ['环图', '饼图', '占比图', '构成图'] },
  { type: 'kpi-cards', keywords: ['指标卡', 'kpi卡', '卡片'] },
  { type: 'timeline', keywords: ['时间线', '路线图', '阶段'] },
  { type: 'risk-matrix', keywords: ['风险矩阵', '优先级矩阵'] },
  { type: 'table', keywords: ['表格', '明细表', '证据表'] },
  { type: 'text-insight', keywords: ['文本', '洞察块', '说明块'] },
  { type: 'headline', keywords: ['大标题', '主标题', '关键结论'] },
];

const STATIC_PAGE_VISUAL_SPEC_PRESETS = {
  'decision-brief': {
    palette: {
      background: '#0f172a',
      surface: 'rgba(255,255,255,0.08)',
      text: '#f8fafc',
      mutedText: '#cbd5e1',
      accent: '#93c5fd',
      chart: '#38bdf8',
    },
    typography: {
      headingFamily: 'Aptos Display, ui-sans-serif, system-ui',
      bodyFamily: 'Aptos, ui-sans-serif, system-ui',
      density: 'compact',
    },
    surface: {
      radius: 26,
      shadow: 'deep',
      decoration: 'subtle-gradient',
    },
  },
  'client-delivery': {
    palette: {
      background: '#f7f8fb',
      surface: 'rgba(255,255,255,0.76)',
      text: '#101827',
      mutedText: '#475569',
      accent: '#2563eb',
      chart: '#0ea5e9',
    },
    typography: {
      headingFamily: 'Aptos Display, ui-sans-serif, system-ui',
      bodyFamily: 'Aptos, ui-sans-serif, system-ui',
      density: 'balanced',
    },
    surface: {
      radius: 26,
      shadow: 'soft',
      decoration: 'warm-gradient',
    },
  },
  'data-command': {
    palette: {
      background: '#064e3b',
      surface: 'rgba(255,255,255,0.09)',
      text: '#ecfeff',
      mutedText: '#cbd5e1',
      accent: '#5eead4',
      chart: '#22d3ee',
    },
    typography: {
      headingFamily: 'Aptos Display, ui-sans-serif, system-ui',
      bodyFamily: 'Aptos, ui-sans-serif, system-ui',
      density: 'dense',
    },
    surface: {
      radius: 22,
      shadow: 'glow',
      decoration: 'command-gradient',
    },
  },
};

const STATIC_PAGE_RENDER_SPEC = {
  renderer: 'static-page-renderer-v1',
  layoutEngine: 'css-grid-12',
  desktopGrid: { columns: GRID_COLUMNS, rowHeight: 96 },
  mobileLayout: 'single-column-sortable',
  componentModel: 'dom-text-svg-chart',
  chartRuntime: 'deterministic-with-echarts-advanced',
  chartRuntimePolicy: {
    default: 'deterministic',
    advanced: 'echarts',
    allowedRuntimes: ['deterministic', 'echarts'],
    advancedOptions: 'plain-json-echarts-option-only',
  },
  editableContent: ['title', 'content', 'dataBinding', 'visualization', 'chartRuntime', 'chartOptions', 'layout'],
  generationGuardrails: [
    '效果图必须服从模块网格布局和移动端顺序',
    '正文、指标、图表在最终静态页中必须是真 DOM 或 SVG，不允许只烘焙进图片',
    '复杂背景、纹理、装饰可以作为图片资产，核心数据表达必须可重新渲染',
    'ECharts 只允许纯 JSON 配置，不允许函数、HTML、远程 URL 或事件处理器字段',
    '避免 3D 透视、真实摄影 UI、不可复刻字体效果和过度复杂玻璃反射',
  ],
};

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

export function buildStaticPageVisualSpec(styleDirection = DEFAULT_STYLE_DIRECTION) {
  const preset = STATIC_PAGE_VISUAL_SPEC_PRESETS[styleDirection]
    || STATIC_PAGE_VISUAL_SPEC_PRESETS[DEFAULT_STYLE_DIRECTION];
  return {
    version: 1,
    styleDirection: STYLE_KEYS.has(styleDirection) ? styleDirection : DEFAULT_STYLE_DIRECTION,
    ...clone(preset),
  };
}

export function buildStaticPageRenderSpec() {
  return clone(STATIC_PAGE_RENDER_SPEC);
}

function stableStringify(value) {
  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(',')}]`;
  }
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableStringify(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function designFingerprint(value) {
  const text = stableStringify(value);
  let hash = 0;
  for (let index = 0; index < text.length; index += 1) {
    hash = (Math.imul(31, hash) + text.charCodeAt(index)) | 0;
  }
  return `design-${(hash >>> 0).toString(16).padStart(8, '0')}`;
}

export function buildStaticPageDataSnapshot(draft) {
  const modules = Array.isArray(draft?.modules) ? draft.modules : [];
  const dataSourceCandidates = buildStaticPageDataSourceCandidates(draft);
  const fieldCandidates = buildStaticPageFieldCandidates(draft);
  return {
    version: 1,
    source: 'static-page-draft',
    selectedDatasetId: draft?.datasetId || null,
    selectedSessionId: draft?.sessionId || null,
    evidenceIds: Array.isArray(draft?.source?.evidenceIds) ? [...draft.source.evidenceIds] : [],
    dataSourceCandidates,
    fieldCandidates,
    moduleBindings: modules.map((module) => {
      const visualization = normalizeVisualization(module.visualization || {});
      const sampleData = chartDataRowsFromVisualization(visualization);
      const binding = normalizeDataBinding(module.dataBinding || {});
      const dataQuality = sampleData.length ? 'module_data' : 'binding_only';
      const bindingQuality = analyzeStaticPageModuleBinding({
        binding,
        visualization,
        sampleData,
        dataQuality,
        fieldCandidates,
      });
      return {
        moduleId: module.id,
        title: module.title,
        binding,
        visualizationType: visualization.type,
        chartRuntime: visualization.chartRuntime,
        chartOptions: visualization.chartOptions,
        sampleData,
        dataQuality,
        bindingQuality,
        bindingQualityStatus: bindingQuality.status,
        chartDataFit: bindingQuality.chartDataFit,
        recommendedAction: bindingQuality.recommendedAction,
      };
    }),
  };
}

export function buildStaticPagePreviewContract(draft, patch = {}) {
  const contractSource = {
    styleDirection: draft?.styleDirection || DEFAULT_STYLE_DIRECTION,
    visualSpec: draft?.visualSpec || buildStaticPageVisualSpec(draft?.styleDirection),
    renderSpec: draft?.renderSpec || buildStaticPageRenderSpec(),
    modules: (Array.isArray(draft?.modules) ? draft.modules : []).map((module) => ({
      id: module.id,
      title: module.title,
      content: module.content,
      dataBinding: module.dataBinding,
      visualization: module.visualization,
      layout: normalizeLayout(module.layout),
    })),
    mobileOrder: normalizeMobileOrder(
      Array.isArray(draft?.modules) ? draft.modules : [],
      draft?.mobileOrder || [],
    ),
  };
  const base = {
    version: 1,
    kind: 'static-page-preview-contract',
    status: 'not_requested',
    draftFingerprint: designFingerprint(contractSource),
    imageJobId: null,
    assetKey: null,
    confirmedAt: null,
    renderExpectation: 'final HTML/CSS/SVG should reproduce the confirmed preview without baking editable text or charts into the image',
  };
  return {
    ...base,
    ...patch,
    version: base.version,
    kind: base.kind,
    draftFingerprint: base.draftFingerprint,
    renderExpectation: base.renderExpectation,
  };
}

function refreshStaticPageDesignSpec(draft, { markPreviewStale = false } = {}) {
  draft.modules = Array.isArray(draft.modules)
    ? draft.modules.map((module) => mergeModule(module))
    : [];
  draft.mobileOrder = normalizeMobileOrder(draft.modules, draft.mobileOrder || []);
  draft.visualSpec = buildStaticPageVisualSpec(draft.styleDirection || DEFAULT_STYLE_DIRECTION);
  draft.renderSpec = draft.renderSpec || buildStaticPageRenderSpec();
  draft.dataSnapshot = buildStaticPageDataSnapshot(draft);
  const previousContract = draft.previewContract || {};
  draft.previewContract = buildStaticPagePreviewContract(draft, previousContract);
  const fingerprintChanged = Boolean(previousContract.draftFingerprint)
    && previousContract.draftFingerprint !== draft.previewContract.draftFingerprint;
  if (markPreviewStale && fingerprintChanged && STALEABLE_PREVIEW_CONTRACT_STATUSES.has(previousContract.status)) {
    draft.previewContract.status = 'stale';
    draft.previewContract.previousAssetKey = previousContract.assetKey || draft.previewImage?.assetKey || null;
    draft.previewContract.staleReason = 'draft design changed after the preview was requested or confirmed';
    draft.previewContract.staleAt = new Date().toISOString();
    draft.previewImage = null;
    draft.finalPage = null;
    draft.imageJob = {
      id: draft.imageJob?.id || previousContract.imageJobId || null,
      status: 'stale',
      queuePosition: null,
      queueMessage: '规划已经改过，上一轮效果图任务已失效，需要重新生成。',
    };
  }
  return draft;
}

function visualizationLabel(type) {
  return STATIC_PAGE_VISUALIZATION_TYPES.find((item) => item.type === type)?.label || type;
}

function dataSourcePreset(sourceId) {
  return STATIC_PAGE_DATA_SOURCE_TYPES.find((item) => item.sourceId === sourceId)
    || STATIC_PAGE_DATA_SOURCE_TYPES[0];
}

function normalizeChartOptions(visualizationType, chartOptions = {}, chartRuntime = 'deterministic') {
  const type = VISUALIZATION_TYPES.has(visualizationType) ? visualizationType : 'text-insight';
  const options = chartOptions && typeof chartOptions === 'object' && !Array.isArray(chartOptions)
    ? { ...chartOptions }
    : {};
  if (chartRuntime === 'echarts') {
    return sanitizeStaticPageChartOptions(options, { runtime: chartRuntime });
  }
  const sanitizedOptions = sanitizeStaticPageChartOptions(options, { runtime: 'deterministic' });
  return {
    showLegend: !['headline', 'kpi-cards', 'text-insight'].includes(type),
    showAxis: ['bar-chart', 'line-chart'].includes(type),
    valueFormat: 'auto',
    ...sanitizedOptions,
  };
}

function chartRowLabel(row, index) {
  if (row && typeof row === 'object') {
    return row.label || row.name || row.month || row.date || row.period || row.category || row.title || row.x || `项${index + 1}`;
  }
  return `项${index + 1}`;
}

function chartRowValue(row) {
  if (typeof row === 'number' && Number.isFinite(row)) return row;
  if (!row || typeof row !== 'object') return null;
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

function normalizeChartDataRows(rows = []) {
  const items = Array.isArray(rows) ? rows : [];
  return items
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

function chartDataRowsFromVisualization(visualization = {}) {
  return normalizeChartDataRows(
    visualization.data
      || visualization.values
      || visualization.sampleData
      || visualization.sample_data
      || visualization.rows
      || visualization.items
      || [],
  );
}

function echartsSeriesType(visualizationType) {
  if (visualizationType === 'line-chart') return 'line';
  if (visualizationType === 'donut-chart') return 'pie';
  if (visualizationType === 'risk-matrix') return 'scatter';
  return 'bar';
}

function buildDefaultEchartsOptions(visualizationType, rows = []) {
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

function normalizeDataBinding(binding = {}) {
  const sourceId = DATA_SOURCE_IDS.has(binding.sourceId) ? binding.sourceId : 'model';
  const preset = dataSourcePreset(sourceId);
  return {
    type: binding.type || preset.type,
    label: binding.label || preset.label,
    sourceId,
    fieldPath: binding.fieldPath || binding.field || null,
    aggregation: binding.aggregation || null,
    evidenceIds: Array.isArray(binding.evidenceIds) ? [...binding.evidenceIds] : [],
  };
}

const VISUALIZATIONS_REQUIRING_SAMPLE_ROWS = new Set([
  'kpi-cards',
  'bar-chart',
  'line-chart',
  'donut-chart',
  'table',
  'risk-matrix',
]);

function moduleBindingHasSource(binding = {}) {
  return Boolean(binding.sourceId || binding.fieldPath || binding.label);
}

function matchingFieldCandidate(fieldCandidates = [], sourceId, fieldPath) {
  if (!fieldPath) return null;
  return fieldCandidates.find((candidate) => {
    const candidateField = candidate?.fieldPath || candidate?.field_path || candidate?.field || null;
    if (candidateField !== fieldPath) return false;
    const candidateSource = candidate?.sourceId || candidate?.source_id || null;
    return !candidateSource || candidateSource === sourceId;
  }) || null;
}

function bindingConfidence(candidate, sampleRows, status) {
  const candidateConfidence = Number(candidate?.confidence);
  if (Number.isFinite(candidateConfidence)) return candidateConfidence;
  if (status === 'confirmed' && sampleRows > 0) return 0.92;
  if (status === 'confirmed') return 0.72;
  if (status === 'partial' && sampleRows > 0) return 0.62;
  if (status === 'partial') return 0.48;
  return 0;
}

function analyzeStaticPageModuleBinding({
  binding,
  visualization,
  sampleData,
  dataQuality,
  fieldCandidates,
}) {
  const sourceId = binding?.sourceId || null;
  const fieldPath = binding?.fieldPath || null;
  const sampleRows = Array.isArray(sampleData) ? sampleData.length : 0;
  const chartNeedsRows = VISUALIZATIONS_REQUIRING_SAMPLE_ROWS.has(visualization?.type);
  const matchedFieldCandidate = matchingFieldCandidate(fieldCandidates, sourceId, fieldPath);
  const hasBinding = moduleBindingHasSource(binding);

  let status = 'partial';
  let reason = 'binding_without_sample_rows';
  let chartDataFit = 'needs_sample_rows';
  let recommendedAction = '已有绑定意图但缺少可渲染数据行，最终页会降级为待确认状态。';

  if (sampleRows > 0 && ['module_data', 'evidence_value'].includes(dataQuality)) {
    status = 'confirmed';
    reason = 'renderable_data_rows';
    chartDataFit = 'ready';
    recommendedAction = '数据样本可直接驱动该模块；交付前只需确认字段口径。';
  } else if (chartNeedsRows && !fieldPath && !hasBinding) {
    status = 'missing';
    reason = 'chart_without_binding';
    chartDataFit = 'missing_binding';
    recommendedAction = '图表模块缺少字段绑定和样本数据，需要先绑定字段或补充数据行。';
  } else if (chartNeedsRows && matchedFieldCandidate) {
    status = 'partial';
    reason = 'matched_field_candidate_without_rows';
    chartDataFit = 'needs_sample_rows';
    recommendedAction = '已匹配候选字段，但还缺少可渲染样本行；生成效果图前建议抽取或填写数据。';
  } else if (!chartNeedsRows && (hasBinding || ['model', 'session', 'conversation_memory'].includes(sourceId))) {
    status = 'confirmed';
    reason = 'non_chart_binding_ready';
    chartDataFit = 'not_required';
    recommendedAction = '文本或结论模块不强制要求数值样本，可按当前绑定继续规划。';
  } else if (!chartNeedsRows) {
    status = 'missing';
    reason = 'non_chart_without_binding';
    chartDataFit = 'not_required';
    recommendedAction = '该模块缺少内容来源，建议绑定模型总结、会话摘要或检索证据。';
  }

  return {
    status,
    reason,
    chartDataFit,
    recommendedAction,
    sourceId,
    fieldPath,
    visualizationType: visualization?.type || 'text-insight',
    chartRuntime: visualization?.chartRuntime || 'deterministic',
    sampleRows,
    dataQuality,
    matchedFieldCandidate,
    confidence: bindingConfidence(matchedFieldCandidate, sampleRows, status),
  };
}

function normalizeVisualization(visualization = {}) {
  const type = VISUALIZATION_TYPES.has(visualization.type) ? visualization.type : 'text-insight';
  const chartOptions = visualization.chartOptions && typeof visualization.chartOptions === 'object' && !Array.isArray(visualization.chartOptions)
    ? visualization.chartOptions
    : {};
  const chartRuntime = normalizeChartRuntimeFromVisualization(visualization, chartOptions);
  const data = chartDataRowsFromVisualization(visualization);
  const next = {
    type,
    label: visualization.label || visualizationLabel(type),
    chartRuntime,
    chartOptions: normalizeChartOptions(type, chartOptions, chartRuntime),
  };
  if (data.length) next.data = data;
  return next;
}

function normalizeLayout(layout = {}) {
  const width = Math.max(1, Math.min(GRID_COLUMNS, Number(layout.w || 1)));
  const x = Math.max(0, Math.min(GRID_COLUMNS - width, Number(layout.x || 0)));
  return {
    x,
    y: Math.max(0, Number(layout.y || 0)),
    w: width,
    h: Math.max(1, Number(layout.h || 1)),
  };
}

function mergeModule(module, patch = {}) {
  const next = {
    ...module,
    ...patch,
    dataBinding: normalizeDataBinding({
      ...(module.dataBinding || {}),
      ...(patch.dataBinding || {}),
    }),
    visualization: normalizeVisualization({
      ...(module.visualization || {}),
      ...(patch.visualization || {}),
      chartOptions: {
        ...(module.visualization?.chartOptions || {}),
        ...(patch.chartOptions || {}),
        ...(patch.visualization?.chartOptions || {}),
      },
    }),
    layout: normalizeLayout({
      ...module.layout,
      ...(patch.layout || {}),
    }),
  };

  return next;
}

export function buildStaticPageDataSourceCandidates(draft = {}) {
  const bySourceId = new Map(STATIC_PAGE_DATA_SOURCE_TYPES.map((item) => [
    item.sourceId,
    {
      ...item,
      available: ['model', 'conversation_memory'].includes(item.sourceId),
    },
  ]));

  if (draft.datasetId) {
    bySourceId.set('dataset', {
      ...dataSourcePreset('dataset'),
      available: true,
      datasetId: draft.datasetId,
    });
    bySourceId.set('selected_scope', {
      ...dataSourcePreset('selected_scope'),
      available: true,
      datasetId: draft.datasetId,
    });
  }
  if (draft.sessionId) {
    bySourceId.set('session', {
      sourceId: 'session',
      type: 'conversation_summary',
      label: '当前会话摘要',
      description: '使用当前报表/静态页会话里已经确认的上下文。',
      available: true,
      sessionId: draft.sessionId,
    });
  }
  if (Array.isArray(draft.source?.evidenceIds) && draft.source.evidenceIds.length > 0) {
    bySourceId.set('evidence', {
      ...dataSourcePreset('evidence'),
      available: true,
      evidenceIds: [...draft.source.evidenceIds],
    });
  }

  return [...bySourceId.values()];
}

function normalizeFieldCandidate(candidate = {}) {
  const sourceId = DATA_SOURCE_IDS.has(candidate.sourceId) ? candidate.sourceId : 'model';
  const fieldPath = candidate.fieldPath || candidate.field_path || candidate.field || null;
  return {
    sourceId,
    fieldPath,
    label: candidate.label || fieldPath || dataSourcePreset(sourceId).label,
    kind: candidate.kind || candidate.type || 'text',
    recommendedAggregation: candidate.recommendedAggregation
      || candidate.recommended_aggregation
      || candidate.aggregation
      || null,
    confidence: Number.isFinite(Number(candidate.confidence)) ? Number(candidate.confidence) : null,
    evidenceIds: Array.isArray(candidate.evidenceIds)
      ? [...candidate.evidenceIds]
      : Array.isArray(candidate.evidence_ids)
        ? [...candidate.evidence_ids]
        : [],
    evidenceRef: candidate.evidenceRef || candidate.evidence_ref || null,
    mediaKind: candidate.mediaKind || candidate.media_kind || null,
    timestamped: Boolean(candidate.timestamped || candidate.has_timestamped_evidence),
  };
}

function pushFieldCandidate(candidates, seen, candidate) {
  const normalized = normalizeFieldCandidate(candidate);
  if (!normalized.fieldPath) return;
  const key = `${normalized.sourceId}:${normalized.fieldPath}`;
  if (seen.has(key)) return;
  seen.add(key);
  candidates.push(normalized);
}

export function buildStaticPageFieldCandidates(draft = {}) {
  const candidates = [];
  const seen = new Set();
  const existingCandidates = [
    ...(Array.isArray(draft?.source?.fieldCandidates) ? draft.source.fieldCandidates : []),
    ...(Array.isArray(draft?.dataSnapshot?.fieldCandidates) ? draft.dataSnapshot.fieldCandidates : []),
    ...(Array.isArray(draft?.dataSnapshot?.field_candidates) ? draft.dataSnapshot.field_candidates : []),
    ...(Array.isArray(draft?.data_snapshot?.field_candidates) ? draft.data_snapshot.field_candidates : []),
  ];

  existingCandidates.forEach((candidate) => pushFieldCandidate(candidates, seen, candidate));

  if (draft.datasetId) {
    pushFieldCandidate(candidates, seen, {
      sourceId: 'dataset',
      fieldPath: 'dataset.metrics_summary',
      label: '数据集指标摘要',
      kind: 'summary',
      confidence: 0.55,
    });
  }

  const evidenceIds = Array.isArray(draft.source?.evidenceIds) ? draft.source.evidenceIds : [];
  if (evidenceIds.length > 0) {
    pushFieldCandidate(candidates, seen, {
      sourceId: 'evidence',
      fieldPath: 'retrieval.summary',
      label: '证据摘要',
      kind: 'text',
      confidence: 0.72,
      evidenceIds,
    });
    pushFieldCandidate(candidates, seen, {
      sourceId: 'evidence',
      fieldPath: 'retrieval.content_excerpt',
      label: '证据原文片段',
      kind: 'text',
      confidence: 0.70,
      evidenceIds,
    });
  }

  return candidates;
}

export function buildStaticPageModuleUpdateOperation(module, patch = {}) {
  const visualizationType = patch.visualizationType || patch.visualization?.type || module?.visualization?.type || 'text-insight';
  const dataBinding = normalizeDataBinding({
    ...(module?.dataBinding || {}),
    ...(patch.dataBinding || {}),
  });
  const requestedVisualization = {
    ...(module?.visualization || {}),
    ...(patch.visualization || {}),
  };
  const requestedRows = chartDataRowsFromVisualization(requestedVisualization);
  const requestedRuntime = requestedVisualization.chartRuntime
    || requestedVisualization.runtime
    || module?.visualization?.chartRuntime
    || 'deterministic';
  const defaultEchartsOptions = requestedRuntime === 'echarts'
    ? buildDefaultEchartsOptions(visualizationType, requestedRows)
    : {};
  const visualization = normalizeVisualization({
    ...requestedVisualization,
    type: visualizationType,
    chartOptions: {
      ...(module?.visualization?.chartOptions || {}),
      ...defaultEchartsOptions,
      ...(patch.chartOptions || {}),
      ...(patch.visualization?.chartOptions || {}),
      dataKey: patch.dataBinding?.fieldPath
        || patch.dataBinding?.field
        || module?.visualization?.chartOptions?.dataKey
        || null,
    },
  });

  const operationPatch = {
    dataBinding,
    visualization,
    chartOptions: visualization.chartOptions,
  };
  if (patch.title !== undefined) operationPatch.title = patch.title;
  if (patch.content !== undefined) operationPatch.content = patch.content;

  return {
    type: 'update_module',
    targetModuleId: module.id,
    patch: operationPatch,
  };
}

function normalizeMobileOrder(modules, order) {
  const moduleIds = modules.map((module) => module.id);
  const seen = new Set();
  const requested = Array.isArray(order) ? order : [];
  const next = requested.filter((id) => {
    if (!moduleIds.includes(id) || seen.has(id)) return false;
    seen.add(id);
    return true;
  });
  moduleIds.forEach((id) => {
    if (!seen.has(id)) next.push(id);
  });
  return next;
}

function promptContains(prompt, keywords) {
  return keywords.some((keyword) => prompt.includes(keyword.toLowerCase()));
}

function inferTargetModuleId(draft, prompt, fallback = 'hero') {
  const modules = Array.isArray(draft?.modules) ? draft.modules : [];
  const titleMatch = modules.find((module) => prompt.includes(String(module.title || '').toLowerCase()));
  if (titleMatch) return titleMatch.id;

  const keywordMatch = MODULE_TARGET_KEYWORDS.find((item) => promptContains(prompt, item.keywords));
  if (keywordMatch && modules.some((module) => module.id === keywordMatch.id)) return keywordMatch.id;

  if (promptContains(prompt, ['柱状图', '折线图', '环图', '趋势图', '对比图'])) {
    return modules.some((module) => module.id === 'trend') ? 'trend' : fallback;
  }

  return modules.some((module) => module.id === fallback) ? fallback : modules[0]?.id;
}

function inferVisualizationType(prompt) {
  return VISUALIZATION_KEYWORDS.find((item) => promptContains(prompt, item.keywords))?.type || null;
}

function firstModuleOrder(draft, moduleId) {
  const currentOrder = normalizeMobileOrder(draft.modules, draft.mobileOrder);
  return [moduleId, ...currentOrder.filter((id) => id !== moduleId)];
}

function lastModuleOrder(draft, moduleId) {
  const currentOrder = normalizeMobileOrder(draft.modules, draft.mobileOrder);
  return [...currentOrder.filter((id) => id !== moduleId), moduleId];
}

function shortModuleContent(module) {
  const title = module.title || '这个模块';
  return `${title}保留关键结论、数据依据和行动含义，减少解释性文字。`;
}

export function buildMockStaticPagePreview(draft) {
  const style = STATIC_PAGE_STYLE_DIRECTIONS.find((item) => item.key === draft.styleDirection);
  return {
    kind: 'mock-effect-preview',
    assetKey: `mock-preview-${draft.id}.json`,
    title: style?.label || draft.styleDirection,
    subtitle: draft.objective,
    queueMessage: draft.imageJob?.queueMessage || '资源正在排队，可以联系商务开通高级用户跳过等待。',
    modules: draft.modules.map((module) => ({
      id: module.id,
      title: module.title,
      visualizationType: module.visualization?.type || 'text-insight',
      width: normalizeLayout(module.layout).w,
    })),
  };
}

export function buildInitialStaticPageDraft({
  datasetId = null,
  sessionId = null,
  conversationSummary = '',
  evidenceIds = [],
  fieldCandidates = [],
} = {}) {
  const modules = clone(DEFAULT_STATIC_PAGE_MODULES);
  const draft = {
    id: `draft-local-${datasetId || 'dataset'}-${sessionId || 'session'}`,
    datasetId,
    sessionId,
    source: {
      conversationSummary,
      selectedMessageIds: [],
      evidenceIds: Array.isArray(evidenceIds) ? [...evidenceIds] : [],
      fieldCandidates: Array.isArray(fieldCandidates) ? [...fieldCandidates] : [],
    },
    status: 'planning',
    objective: '给客户展示当前数据结论，并生成可交付静态页',
    audience: '客户决策层',
    styleDirection: DEFAULT_STYLE_DIRECTION,
    modelSummary: conversationSummary || '模型将根据当前会话和数据集生成静态页结构。',
    mobileOrder: modules.map((module) => module.id),
    modules,
    visualSpec: buildStaticPageVisualSpec(DEFAULT_STYLE_DIRECTION),
    renderSpec: buildStaticPageRenderSpec(),
    dataSnapshot: null,
    previewContract: null,
    operations: [],
    imageJob: {
      id: null,
      status: 'idle',
      queuePosition: null,
      queueMessage: '',
    },
    previewImage: null,
    finalPage: null,
  };
  return refreshStaticPageDesignSpec(draft);
}

export function validateStaticPageLayout(layout) {
  const next = normalizeLayout(layout);
  return next.x >= 0
    && next.y >= 0
    && next.w >= 1
    && next.h >= 1
    && next.x + next.w <= GRID_COLUMNS;
}

export function validateMobileOrder(modules, order) {
  if (!Array.isArray(modules) || !Array.isArray(order)) return false;
  const moduleIds = modules.map((module) => module.id).sort();
  const orderIds = [...order].sort();
  return moduleIds.length === orderIds.length && moduleIds.every((id, index) => id === orderIds[index]);
}

export function applyStaticPageOperation(draft, operation = {}) {
  const next = clone(draft);
  const type = operation.type;
  const moduleId = operation.targetModuleId || operation.moduleId;
  const moduleIndex = next.modules.findIndex((module) => module.id === moduleId);

  if (type === 'update_module' && moduleIndex >= 0) {
    next.modules[moduleIndex] = mergeModule(next.modules[moduleIndex], operation.patch || {});
  }

  if (type === 'add_module') {
    const module = mergeModule({
      id: operation.module?.id || `module-${next.modules.length + 1}`,
      role: operation.module?.role || 'text',
      title: operation.module?.title || '新增模块',
      content: operation.module?.content || '补充这个模块要表达的内容。',
      dataBinding: operation.module?.dataBinding || {
        type: 'model_summary',
        label: '来自模型总结',
        sourceId: 'model',
      },
      visualization: operation.module?.visualization || {
        type: 'text-insight',
        label: '洞察文本块',
      },
      layout: operation.module?.layout || { x: 0, y: next.modules.length * 2, w: 6, h: 3 },
    });
    next.modules.push(module);
  }

  if (type === 'remove_module' && moduleIndex >= 0) {
    next.modules.splice(moduleIndex, 1);
  }

  if ((type === 'move_module' || type === 'resize_module') && moduleIndex >= 0) {
    next.modules[moduleIndex] = mergeModule(next.modules[moduleIndex], {
      layout: operation.layout || operation.patch || {},
    });
  }

  if (type === 'change_visualization' && moduleIndex >= 0 && VISUALIZATION_TYPES.has(operation.visualizationType)) {
    next.modules[moduleIndex] = mergeModule(next.modules[moduleIndex], {
      visualization: {
        type: operation.visualizationType,
        label: visualizationLabel(operation.visualizationType),
        chartRuntime: operation.chartRuntime || next.modules[moduleIndex]?.visualization?.chartRuntime,
        chartOptions: operation.chartOptions || next.modules[moduleIndex]?.visualization?.chartOptions || {},
      },
    });
  }

  if (type === 'change_data_binding' && moduleIndex >= 0) {
    next.modules[moduleIndex] = mergeModule(next.modules[moduleIndex], {
      dataBinding: operation.dataBinding || {},
    });
  }

  if (type === 'reorder_modules') {
    next.mobileOrder = normalizeMobileOrder(next.modules, operation.order);
  }

  if (type === 'change_style_direction' && STYLE_KEYS.has(operation.styleDirection)) {
    next.styleDirection = operation.styleDirection;
    next.visualSpec = buildStaticPageVisualSpec(operation.styleDirection);
  }

  if (type === 'refresh_summary' && operation.modelSummary) {
    next.modelSummary = String(operation.modelSummary);
  }

  if (type === 'queue_image_job') {
    next.status = 'queued';
    next.imageJob = {
      ...next.imageJob,
      id: operation.jobId || next.imageJob.id || `mock-image-job-${next.id}`,
      status: 'queued',
      queuePosition: operation.queuePosition ?? next.imageJob.queuePosition,
      queueMessage: operation.queueMessage || '资源正在排队，可以联系商务开通高级用户跳过等待。',
    };
    next.previewImage = null;
    next.finalPage = null;
    next.previewContract = buildStaticPagePreviewContract(next, {
      status: 'queued',
      imageJobId: next.imageJob.id,
      queuePosition: next.imageJob.queuePosition,
      assetKey: null,
      confirmedAt: null,
    });
  }

  if (type === 'update_image_job_status' && IMAGE_JOB_STATUSES.has(operation.status)) {
    next.status = operation.status === 'preview_ready'
      ? 'preview_ready'
      : operation.status === 'failed'
        ? 'planning'
        : next.status;
    next.imageJob = {
      ...next.imageJob,
      status: operation.status,
      queuePosition: operation.queuePosition ?? next.imageJob.queuePosition,
      queueMessage: operation.queueMessage || next.imageJob.queueMessage,
    };
    next.previewContract = buildStaticPagePreviewContract(next, {
      ...(next.previewContract || {}),
      status: operation.status,
      imageJobId: next.imageJob.id,
      queuePosition: next.imageJob.queuePosition,
      failureReason: operation.status === 'failed'
        ? (operation.queueMessage || next.imageJob.queueMessage || '效果图生成失败')
        : undefined,
    });
  }

  if (type === 'mark_preview_ready') {
    next.status = 'preview_ready';
    next.imageJob = {
      ...next.imageJob,
      status: 'preview_ready',
      queuePosition: null,
      queueMessage: '',
    };
    next.previewImage = operation.previewImage || buildMockStaticPagePreview(next);
    next.previewContract = buildStaticPagePreviewContract(next, {
      ...(next.previewContract || {}),
      status: 'preview_ready',
      imageJobId: next.imageJob.id,
      assetKey: next.previewImage?.assetKey || null,
      queuePosition: null,
    });
  }

  if (type === 'confirm_preview') {
    next.status = 'effect_confirmed';
    next.imageJob = {
      ...next.imageJob,
      status: operation.imageJobStatus || 'preview_ready',
      queuePosition: null,
    };
    next.previewImage = operation.previewImage || next.previewImage;
    next.previewContract = buildStaticPagePreviewContract(next, {
      ...(next.previewContract || {}),
      status: 'confirmed',
      imageJobId: next.imageJob.id,
      assetKey: next.previewImage?.assetKey || next.previewContract?.assetKey || null,
      queuePosition: null,
      confirmedAt: operation.confirmedAt || next.previewContract?.confirmedAt || null,
    });
  }

  if (type === 'reset_image_job') {
    next.status = 'planning';
    next.imageJob = {
      id: null,
      status: 'idle',
      queuePosition: null,
      queueMessage: '',
    };
    next.previewImage = null;
    next.finalPage = null;
    next.previewContract = buildStaticPagePreviewContract(next);
  }

  if (type === 'reset_final_render') {
    next.status = next.previewContract?.status === 'confirmed' || next.previewImage
      ? 'effect_confirmed'
      : 'preview_ready';
    next.finalPage = null;
    next.imageJob = {
      ...next.imageJob,
      status: next.imageJob?.status || 'preview_ready',
      queuePosition: null,
    };
    next.previewContract = buildStaticPagePreviewContract(next, {
      ...(next.previewContract || {}),
      status: next.previewContract?.status === 'confirmed' ? 'confirmed' : 'preview_ready',
      assetKey: next.previewImage?.assetKey || next.previewContract?.assetKey || null,
      queuePosition: null,
    });
  }

  if (type === 'request_final_render') {
    const backendFinalPage = operation.finalPage && typeof operation.finalPage === 'object'
      ? operation.finalPage
      : null;
    next.status = backendFinalPage?.status === 'rendered' ? 'rendered' : 'rendering';
    next.finalPage = backendFinalPage
      ? {
          ...next.finalPage,
          ...backendFinalPage,
          status: backendFinalPage.status || 'rendered',
          renderer: backendFinalPage.renderer || 'platform-api-static-page-renderer',
          payload: backendFinalPage.payload || buildStaticPageFinalRenderPayload(next),
        }
      : {
          ...next.finalPage,
          status: 'mock_ready',
          renderer: 'local-static-page-mock',
          notice: '后端 renderer 尚未接入，当前为前端静态页模拟结果。',
          payload: operation.payload || buildStaticPageFinalRenderPayload(next),
        };
  }

  next.mobileOrder = normalizeMobileOrder(next.modules, next.mobileOrder);
  refreshStaticPageDesignSpec(next, { markPreviewStale: DESIGN_MUTATION_TYPES.has(type) });
  next.operations = [...(next.operations || []), clone(operation)];
  return next;
}

export function applyStaticPageOperations(draft, operations = []) {
  return operations.reduce((current, operation) => applyStaticPageOperation(current, operation), draft);
}

export function canRequestStaticPageFinalRender(draft = {}) {
  return staticPageFinalRenderBlockReason(draft) === '';
}

function bindingSampleRows(binding = {}) {
  const qualityRows = Number(binding.bindingQuality?.sampleRows ?? binding.binding_quality?.sample_rows);
  if (Number.isFinite(qualityRows)) return qualityRows;
  const rows = binding.sampleData || binding.sample_data;
  return Array.isArray(rows) ? rows.length : 0;
}

function bindingNeedsPreviewAttention(binding = {}) {
  const status = String(
    binding.bindingQualityStatus
      || binding.binding_quality_status
      || binding.bindingQuality?.status
      || binding.binding_quality?.status
      || '',
  ).trim();
  if (status && !['confirmed', 'ready', 'non_chart'].includes(status)) return true;

  const chartDataFit = String(
    binding.chartDataFit
      || binding.chart_data_fit
      || binding.bindingQuality?.chartDataFit
      || binding.binding_quality?.chart_data_fit
      || '',
  ).trim();
  if (chartDataFit && !['ready', 'not_required', 'non_chart_ready'].includes(chartDataFit)) return true;

  const visualizationType = String(binding.visualizationType || binding.visualization_type || '').trim();
  return VISUALIZATIONS_REQUIRING_SAMPLE_ROWS.has(visualizationType) && bindingSampleRows(binding) === 0;
}

function bindingPreviewLabel(binding = {}) {
  const title = binding.title || binding.moduleTitle || binding.moduleId || binding.module_id || '未命名模块';
  const chartDataFit = String(binding.chartDataFit || binding.chart_data_fit || '').trim();
  const status = String(binding.bindingQualityStatus || binding.binding_quality_status || '').trim();
  const marker = chartDataFit && !['ready', 'not_required', 'non_chart_ready'].includes(chartDataFit)
    ? chartDataFit
    : status;
  return marker ? `${title}（${marker}）` : String(title);
}

function staticPageDataQualityBlockReason(draft = {}, actionLabel, nextInstruction) {
  const snapshot = draft?.dataSnapshot || draft?.data_snapshot || buildStaticPageDataSnapshot(draft);
  const bindings = snapshot?.moduleBindings || snapshot?.module_bindings || [];
  const attentionBindings = Array.isArray(bindings)
    ? bindings.filter(bindingNeedsPreviewAttention)
    : [];
  if (!attentionBindings.length) return '';
  const labels = attentionBindings.slice(0, 3).map(bindingPreviewLabel).join('、');
  return `当前静态页还有 ${attentionBindings.length} 个模块的数据绑定未达到${actionLabel}：${labels}。${nextInstruction}`;
}

export function staticPagePreviewBlockReason(draft = {}) {
  return staticPageDataQualityBlockReason(
    draft,
    '效果图生成要求',
    '请先回到模块编辑补充样本行、重新绑定字段，或让 V3 检索/修复模块数据。',
  );
}

export function staticPageFinalRenderBlockReason(draft = {}) {
  if (draft?.previewContract?.status === 'stale' || draft?.imageJob?.status === 'stale') {
    return '规划已经改过，需要重新生成并确认效果图。';
  }
  const finalStatus = draft?.finalPage?.status || '';
  const retryableFinalStatus = finalStatus === 'failed' || finalStatus === 'cancelled';
  if (draft?.status !== 'effect_confirmed' && !retryableFinalStatus) {
    return '先确认效果图，再按效果制作可交付静态页。';
  }
  if (draft?.previewContract?.status !== 'confirmed') {
    return '效果图确认状态未同步，请刷新或重新确认效果图。';
  }
  if (!draft?.previewImage?.assetKey && !draft?.previewContract?.assetKey) {
    return '效果图资源缺失，请重新生成效果图。';
  }
  return staticPageDataQualityBlockReason(
    draft,
    '最终页面生成要求',
    '请先回到模块编辑补充样本行、重新绑定字段，或让 V3 检索/修复模块数据，然后重新生成并确认效果图。',
  );
}

export function interpretStaticPagePrompt(draft, prompt = '') {
  const rawPrompt = String(prompt || '').trim();
  const normalizedPrompt = rawPrompt.toLowerCase();
  const operations = [];
  const summaryParts = [];

  if (!draft || !rawPrompt) {
    return {
      prompt: rawPrompt,
      summary: '没有可应用的静态页修改意图。',
      operations,
    };
  }

  const targetModuleId = inferTargetModuleId(draft, normalizedPrompt);
  const targetModule = draft.modules.find((module) => module.id === targetModuleId);

  if (promptContains(normalizedPrompt, ['老板', '高层', '董事会', '决策层', '管理层', 'ceo', '总裁'])) {
    operations.push({ type: 'change_style_direction', styleDirection: 'decision-brief' });
    summaryParts.push('改成高层决策简报，结论先行并突出行动重点');
  }

  if (promptContains(normalizedPrompt, ['客户交付', '交付报告', '售前', '方案', '客户汇报', '解释清楚'])) {
    operations.push({ type: 'change_style_direction', styleDirection: 'client-delivery' });
    summaryParts.push('改成客户交付报告，保留更完整的解释结构');
  }

  if (promptContains(normalizedPrompt, ['看板', '大屏', '运营监控', '数据密度', '指标密度'])) {
    operations.push({ type: 'change_style_direction', styleDirection: 'data-command' });
    summaryParts.push('改成数据运营看板，提高指标和图表密度');
  }

  if (promptContains(normalizedPrompt, ['风险', '隐患', '预警'])
    && promptContains(normalizedPrompt, ['突出', '强调', '优先', '放前', '前面', '高亮', '重点'])) {
    operations.push({
      type: 'update_module',
      targetModuleId: 'risk',
      patch: {
        title: '优先风险与机会',
        content: '把客户需要优先处理的风险、影响范围和可推进机会放在更显眼的位置。',
        layout: { x: 0, y: 3, w: 7, h: 4 },
      },
    });
    operations.push({ type: 'reorder_modules', order: firstModuleOrder(draft, 'risk') });
    summaryParts.push('把风险模块前置并放大，优先呈现风险和机会');
  }

  if (promptContains(normalizedPrompt, ['建议', '下一步', '动作'])
    && promptContains(normalizedPrompt, ['最后', '结尾', '放后', '收尾'])) {
    operations.push({ type: 'reorder_modules', order: lastModuleOrder(draft, 'next-steps') });
    summaryParts.push('把建议动作放到页面收尾位置');
  }

  if (promptContains(normalizedPrompt, ['减少文字', '少点字', '少一点字', '精简', '压缩', '简短'])) {
    draft.modules.forEach((module) => {
      operations.push({
        type: 'update_module',
        targetModuleId: module.id,
        patch: { content: shortModuleContent(module) },
      });
    });
    summaryParts.push('压缩所有模块文案，只保留结论、数据依据和行动含义');
  }

  const visualizationType = inferVisualizationType(normalizedPrompt);
  if (visualizationType && targetModuleId) {
    operations.push({
      type: 'change_visualization',
      targetModuleId,
      visualizationType,
    });
    summaryParts.push(`把${targetModule?.title || '目标模块'}改成${visualizationLabel(visualizationType)}`);
  }

  if (promptContains(normalizedPrompt, ['增加', '新增', '添加', '加一个', '补充', '多一个'])
    && promptContains(normalizedPrompt, ['数据', '明细', '证据', '分布', '来源'])) {
    operations.push({
      type: 'add_module',
      module: {
        id: `evidence-${draft.modules.length + 1}`,
        role: 'evidence',
        title: '补充数据证据',
        content: '新增一块数据证据，用来解释关键结论背后的来源、分布或明细。',
        dataBinding: {
          type: 'conversation_evidence',
          label: '来自会话补充证据',
          sourceId: 'evidence',
        },
        visualization: {
          type: promptContains(normalizedPrompt, ['分布', '占比', '构成']) ? 'donut-chart' : 'table',
          label: promptContains(normalizedPrompt, ['分布', '占比', '构成'])
            ? visualizationLabel('donut-chart')
            : visualizationLabel('table'),
        },
        layout: { x: 0, y: draft.modules.length * 2, w: 6, h: 3 },
      },
    });
    summaryParts.push('新增数据证据模块，补充来源、分布或明细');
  }

  if (!summaryParts.length && targetModuleId) {
    operations.push({
      type: 'update_module',
      targetModuleId,
      patch: {
        content: `${targetModule?.content || '保留当前内容'} 修改方向：${rawPrompt}`,
      },
    });
    summaryParts.push(`围绕${targetModule?.title || '目标模块'}应用用户修改意图`);
  }

  const summary = `模型理解：${summaryParts.join('；')}。`;
  operations.push({
    type: 'refresh_summary',
    modelSummary: summary,
  });

  return {
    prompt: rawPrompt,
    summary,
    operations,
  };
}

export function buildStaticPageImagePayload(draft, { oneClick = false } = {}) {
  const visualSpec = draft.visualSpec || buildStaticPageVisualSpec(draft.styleDirection);
  const renderSpec = draft.renderSpec || buildStaticPageRenderSpec();
  return {
    draftId: draft.id,
    datasetId: draft.datasetId,
    sessionId: draft.sessionId,
    oneClick,
    objective: draft.objective,
    audience: draft.audience,
    styleDirection: draft.styleDirection,
    visualSpec,
    renderSpec,
    dataSnapshot: draft.dataSnapshot || buildStaticPageDataSnapshot(draft),
    previewContract: draft.previewContract || buildStaticPagePreviewContract(draft),
    designContract: {
      contractSource: 'StaticPageDraft',
      renderer: renderSpec.renderer,
      editableCore: renderSpec.componentModel,
      guardrails: renderSpec.generationGuardrails,
    },
    modelSummary: draft.modelSummary,
    modules: draft.modules.map((module) => {
      const visualization = normalizeVisualization(module.visualization || {});
      return {
        id: module.id,
        title: module.title,
        content: module.content,
        dataLabel: module.dataBinding?.label || '',
        dataBinding: normalizeDataBinding(module.dataBinding || {}),
        visualizationType: visualization.type,
        chartRuntime: visualization.chartRuntime,
        chartOptions: visualization.chartOptions,
        sampleData: chartDataRowsFromVisualization(visualization),
        layout: normalizeLayout(module.layout),
      };
    }),
  };
}

export function buildStaticPageFinalRenderPayload(draft) {
  return {
    draftId: draft.id,
    styleDirection: draft.styleDirection,
    visualSpec: draft.visualSpec || buildStaticPageVisualSpec(draft.styleDirection),
    renderSpec: draft.renderSpec || buildStaticPageRenderSpec(),
    dataSnapshot: draft.dataSnapshot || buildStaticPageDataSnapshot(draft),
    previewContract: draft.previewContract || buildStaticPagePreviewContract(draft),
    previewImage: draft.previewImage,
    mobileOrder: normalizeMobileOrder(draft.modules, draft.mobileOrder),
    modules: draft.modules.map((module) => ({
      ...clone(module),
      layout: normalizeLayout(module.layout),
    })),
  };
}
