import {
  normalizeChartRuntimeFromVisualization,
  sanitizeStaticPageChartOptions,
} from './static-page-chart-runtime.js';
import {
  inferStaticPageTemplateReferenceId,
  normalizeTemplateDesignReferences,
  staticPageTemplateReferenceDraftSeed,
} from './html-template-references.js';

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
    '可视化必须服从模块网格布局和移动端顺序',
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
  const headingCandidate = staticPageHeadingFieldCandidate(fieldCandidates);
  const designReferences = normalizeTemplateDesignReferences(
    draft?.designReferences || draft?.source?.templateReferences || [],
  );
  const moduleBindings = modules.map((module) => {
    const visualization = normalizeVisualization(module.visualization || {});
    const sampleData = chartDataRowsFromVisualization(visualization);
    const binding = enrichDocsPageHeadingBinding({
      draft,
      module,
      binding: normalizeDataBinding(module.dataBinding || {}),
      headingCandidate,
    });
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
  });
  return {
    version: 1,
    source: 'static-page-draft',
    selectedDatasetId: draft?.datasetId || null,
    selectedSessionId: draft?.sessionId || null,
    evidenceIds: Array.isArray(draft?.source?.evidenceIds) ? [...draft.source.evidenceIds] : [],
    designReferences,
    dataSourceCandidates,
    fieldCandidates,
    moduleBindings,
    structureSignals: buildStaticPageStructureSignals(fieldCandidates, moduleBindings),
  };
}

export function buildStaticPagePreviewContract(draft, patch = {}) {
  const designReferences = normalizeTemplateDesignReferences(
    draft?.designReferences || draft?.source?.templateReferences || [],
  );
  const contractSource = {
    styleDirection: draft?.styleDirection || DEFAULT_STYLE_DIRECTION,
    visualSpec: draft?.visualSpec || buildStaticPageVisualSpec(draft?.styleDirection),
    renderSpec: draft?.renderSpec || buildStaticPageRenderSpec(),
    designReferences,
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
    renderExpectation: 'final HTML/CSS/SVG should reproduce the generated preview without baking editable text or charts into the image',
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
  draft.designReferences = normalizeTemplateDesignReferences(
    draft.designReferences || draft.source?.templateReferences || [],
  );
  draft.source = {
    ...(draft.source || {}),
    templateReferences: draft.designReferences,
  };
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
      queueMessage: '规划已经改过，上一轮可视化任务已失效，需要重新生成。',
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
    recommendedAction = '已匹配候选字段，但还缺少可渲染样本行；生成可视化前建议抽取或填写数据。';
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

function isDocsPageTemplateDraft(draft = {}) {
  const references = normalizeTemplateDesignReferences(
    draft.designReferences || draft.source?.templateReferences || draft.dataSnapshot?.designReferences || [],
  );
  return draft.templateReferenceId === 'docs-page'
    || references.some((reference) => reference.templateId === 'docs-page');
}

function docsPageStructureModuleUsesHeadingHints(module = {}) {
  return ['scope', 'steps', 'interfaces', 'checks'].includes(module.id || module.role || '');
}

function staticPageHeadingFieldCandidate(fieldCandidates = []) {
  return fieldCandidates.find((candidate) => (
    candidate?.sourceId === 'evidence'
    && candidate?.fieldPath === 'retrieval.section_title_hints'
    && Array.isArray(candidate?.sectionTitleHints)
    && candidate.sectionTitleHints.length > 0
  )) || null;
}

function enrichDocsPageHeadingBinding({ draft, module, binding, headingCandidate }) {
  if (!headingCandidate || !isDocsPageTemplateDraft(draft) || !docsPageStructureModuleUsesHeadingHints(module)) {
    return binding;
  }
  if (binding.fieldPath && binding.fieldPath !== 'retrieval.section_title_hints') {
    return binding;
  }
  return normalizeDataBinding({
    ...binding,
    type: 'retrieval_evidence',
    label: headingCandidate.label || '文档段落标题线索',
    sourceId: 'evidence',
    fieldPath: 'retrieval.section_title_hints',
    evidenceIds: headingCandidate.evidenceIds,
  });
}

function buildStaticPageStructureSignals(fieldCandidates = [], moduleBindings = []) {
  const sectionTitleHints = [];
  const pushHints = (value) => collectStaticPageStringHints(value, sectionTitleHints);
  const hintsFrom = (value) => {
    const hints = [];
    collectStaticPageStringHints(value, hints);
    return hints.slice(0, 12);
  };
  const headingCandidates = fieldCandidates
    .filter((candidate) => candidate?.fieldPath === 'retrieval.section_title_hints')
    .slice(0, 4)
    .map((candidate) => {
      const candidateHints = hintsFrom(candidate.sectionTitleHints || candidate.section_title_hints);
      pushHints(candidateHints);
      return {
        sourceId: candidate.sourceId || candidate.source_id || '',
        fieldPath: candidate.fieldPath || candidate.field_path || '',
        label: candidate.label || '',
        kind: candidate.kind || '',
        confidence: candidate.confidence ?? null,
        evidenceIds: candidate.evidenceIds || candidate.evidence_ids || [],
        sectionTitleHints: candidateHints,
      };
    });
  const boundModules = moduleBindings
    .filter((binding) => {
      const fieldPath = binding?.binding?.fieldPath
        || binding?.binding?.field_path
        || binding?.bindingQuality?.fieldPath
        || binding?.bindingQuality?.matchedFieldCandidate?.fieldPath;
      return fieldPath === 'retrieval.section_title_hints';
    })
    .slice(0, 8)
    .map((binding) => {
      const matched = binding?.bindingQuality?.matchedFieldCandidate || {};
      const matchedHints = hintsFrom(matched.sectionTitleHints || matched.section_title_hints);
      pushHints(matchedHints);
      return {
        moduleId: binding.moduleId || '',
        title: binding.title || '',
        fieldPath: 'retrieval.section_title_hints',
        bindingQualityStatus: binding.bindingQualityStatus || binding.bindingQuality?.status || '',
        sectionTitleHints: matchedHints,
      };
    });
  return {
    version: 1,
    status: sectionTitleHints.length ? 'available' : 'none',
    policy: 'source_structure_only_no_body_no_sample_rows',
    sectionTitleHints: sectionTitleHints.slice(0, 12),
    fieldCandidates: headingCandidates,
    boundModules,
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
  const source = candidate && typeof candidate === 'object' ? candidate : {};
  const sourceId = DATA_SOURCE_IDS.has(source.sourceId) ? source.sourceId : 'model';
  const fieldPath = source.fieldPath || source.field_path || source.field || null;
  const sectionTitleHints = collectStaticPageSectionTitleHintsFromValue(source).slice(0, 12);
  return {
    sourceId,
    fieldPath,
    label: source.label || fieldPath || dataSourcePreset(sourceId).label,
    kind: source.kind || source.type || 'text',
    recommendedAggregation: source.recommendedAggregation
      || source.recommended_aggregation
      || source.aggregation
      || null,
    confidence: Number.isFinite(Number(source.confidence)) ? Number(source.confidence) : null,
    evidenceIds: Array.isArray(source.evidenceIds)
      ? [...source.evidenceIds]
      : Array.isArray(source.evidence_ids)
        ? [...source.evidence_ids]
        : [],
    evidenceRef: source.evidenceRef || source.evidence_ref || null,
    sectionTitleHints,
    mediaKind: source.mediaKind || source.media_kind || null,
    timestamped: Boolean(source.timestamped || source.has_timestamped_evidence),
  };
}

function pushStaticPageStringHint(output, value) {
  const normalized = String(value || '').trim().slice(0, 80);
  if (normalized && !output.includes(normalized)) {
    output.push(normalized);
  }
}

function collectStaticPageStringHints(value, output) {
  if (!value) return;
  if (typeof value === 'string') {
    pushStaticPageStringHint(output, value);
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item) => collectStaticPageStringHints(item, output));
  }
}

function collectStaticPageSectionTitleHintsFromValue(value = {}) {
  const source = value && typeof value === 'object' ? value : {};
  const hints = [];
  [
    source.sectionTitleHints,
    source.section_title_hints,
    source.sectionTitles,
    source.section_titles,
    source.headingHints,
    source.heading_hints,
    source.evidenceRef?.sectionTitleHints,
    source.evidenceRef?.section_title_hints,
    source.evidence_ref?.sectionTitleHints,
    source.evidence_ref?.section_title_hints,
    source.evidenceRef?.evidence?.sectionTitleHints,
    source.evidenceRef?.evidence?.section_title_hints,
    source.evidence_ref?.evidence?.sectionTitleHints,
    source.evidence_ref?.evidence?.section_title_hints,
  ].forEach((item) => collectStaticPageStringHints(item, hints));
  return hints;
}

function collectStaticPageSectionTitleHintsFromCandidates(candidates = []) {
  const hints = [];
  candidates.forEach((candidate) => {
    collectStaticPageStringHints(candidate.sectionTitleHints, hints);
    collectStaticPageStringHints(candidate.section_title_hints, hints);
    collectStaticPageStringHints(candidate.evidenceRef?.sectionTitleHints, hints);
    collectStaticPageStringHints(candidate.evidenceRef?.section_title_hints, hints);
  });
  return hints.slice(0, 24);
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
  const sectionTitleHints = collectStaticPageSectionTitleHintsFromCandidates(candidates);

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
  if (evidenceIds.length > 0 && sectionTitleHints.length > 0) {
    pushFieldCandidate(candidates, seen, {
      sourceId: 'evidence',
      fieldPath: 'retrieval.section_title_hints',
      label: '文档段落标题线索',
      kind: 'section_titles',
      confidence: 0.78,
      evidenceIds,
      sectionTitleHints,
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

function compactPlanningText(value, limit = 160) {
  return String(value || '')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, limit);
}

function planningSummaryLine(summary = '', prefixes = []) {
  const lines = String(summary || '').split(/\r?\n/);
  const matched = lines.find((line) => prefixes.some((prefix) => line.trim().startsWith(prefix)));
  if (!matched) return '';
  const separator = matched.indexOf('：') >= 0 ? matched.indexOf('：') : matched.indexOf(':');
  return separator >= 0 ? matched.slice(separator + 1).trim() : matched.trim();
}

function extractStaticPageSubject({ templateIntent = '', conversationSummary = '' } = {}) {
  const directGoal = planningSummaryLine(conversationSummary, ['用户要求', '目标', '主题']);
  const datasetLine = planningSummaryLine(conversationSummary, ['数据集']);
  const candidates = [
    templateIntent,
    directGoal,
    datasetLine,
    conversationSummary,
  ].map((item) => String(item || '').trim()).filter(Boolean);

  for (const candidate of candidates) {
    const patternMatch = candidate.match(/(?:基于|根据|围绕|关于|把|将)\s*([^，。；\n]+?)(?:生成|做成|做一个|制作|整理|转成|输出|出一版|做|$)/);
    const raw = patternMatch?.[1] || candidate.split(/[，。；\n]/)[0];
    const cleaned = raw
      .replace(/用户要求[:：]?/g, '')
      .replace(/数据集[:：]?/g, '')
      .replace(/(帮我|请|快速|直接|马上|立即|一键|生成|制作|做一个|做成|输出|静态页|静态页面|页面|效果图|html|HTML|报告|可视化|数据|资料|基于|根据|围绕|关于)/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
    if (cleaned.length >= 2) {
      return cleaned.slice(0, 32);
    }
  }

  return '当前主题';
}

function buildStaticPagePlanningBrief({
  templateIntent = '',
  conversationSummary = '',
  templateLabel = '',
} = {}) {
  const subject = extractStaticPageSubject({ templateIntent, conversationSummary });
  const userGoal = compactPlanningText(
    planningSummaryLine(conversationSummary, ['用户要求', '目标'])
      || templateIntent
      || `生成${subject}静态页`,
    180,
  );
  const datasetLine = compactPlanningText(planningSummaryLine(conversationSummary, ['数据集']), 180);
  const contextLine = compactPlanningText(planningSummaryLine(conversationSummary, ['最近内容', '上下文']), 220);
  const documentLine = compactPlanningText(planningSummaryLine(conversationSummary, ['文档线索']), 220);
  const evidenceLine = compactPlanningText(planningSummaryLine(conversationSummary, ['证据线索', '供料线索']), 220);
  const availableLines = [
    datasetLine ? `数据集：${datasetLine}` : '',
    documentLine ? `文档线索：${documentLine}` : '',
    evidenceLine ? `证据线索：${evidenceLine}` : '',
    contextLine ? `上下文：${contextLine}` : '',
  ].filter(Boolean);

  return {
    version: 1,
    subject,
    userGoal,
    templateLabel,
    sourceMarkdown: [
      `# ${subject} 静态页规划简稿`,
      '',
      `- 用户目标：${userGoal}`,
      templateLabel ? `- 模板参考：${templateLabel}` : '',
      ...availableLines.map((line) => `- ${line}`),
      availableLines.length ? '' : '- 当前缺少结构化供料，先生成可编辑设计规划，后续补证据和样本数据。',
    ].filter((line) => line !== '').join('\n'),
    datasetLine,
    contextLine,
    documentLine,
    evidenceLine,
  };
}

function plannedModuleCopy(module, { templateId = '', subject = '', brief = {} } = {}) {
  const role = module.role || module.id;
  const goal = brief.userGoal || `生成${subject}静态页`;
  const context = brief.contextLine || brief.documentLine || brief.evidenceLine || '当前对话和可见资料';
  const dataset = brief.datasetLine || '当前可见数据集';
  const evidence = brief.evidenceLine || brief.documentLine || '已供料证据和待补信息';

  if (templateId === 'docs-page') {
    const copies = {
      hero: [`${subject}概览`, `说明${subject}要解决的问题、适用对象、当前资料完整度和交付边界。目标：${goal}`],
      scope: ['范围与边界', `从${context}中整理系统边界、权限边界、已确认范围和仍需第三方补充的信息。`],
      steps: ['关键流程', `把${subject}拆成准备、请求、处理、回传、验收等阶段，保留每一步依赖和责任方线索。`],
      interfaces: ['接口与数据', `沉淀${subject}涉及的接口、字段、入参出参、鉴权和数据来源；缺少真实字段时明确标注待补。`],
      checks: ['验收与风险', `列出联调验收项、风险点、失败重试和下一步交付动作，避免只给抽象说明。`],
    };
    return copies[module.id] || copies[role] || [module.title, module.content];
  }

  if (templateId === 'dashboard') {
    const copies = {
      hero: [`${subject}总览`, `用一句话概括${subject}当前状态、异常等级和本轮最应该关注的判断。`],
      kpi: ['关键状态指标', `从${dataset}中提炼能判断健康度、进度、效率或风险的指标；没有数值时保留待补口径。`],
      trend: ['运行趋势', `围绕${subject}展示时间、阶段或类别变化方向，优先承接已供料线索。`],
      risk: ['风险预警', `把${evidence}里的阻塞项、异常项和机会点按优先级展示。`],
      activity: ['最近动作', `整理${context}中已经发生、正在等待和下一步应执行的动作。`],
    };
    return copies[module.id] || copies[role] || [module.title, module.content];
  }

  const copies = {
    hero: [`${subject}核心结论`, `先给出${subject}最重要的判断，并说明判断来自${context}。`],
    kpi: ['关键指标与口径', `从${dataset}中规划 3-5 个指标位；没有真实数值时显示为待补口径，不编造数字。`],
    trend: ['趋势与变化', `展示${subject}随时间、阶段或类别的变化方向，优先使用已供料字段或证据线索。`],
    comparison: ['分类对比', `对${subject}的渠道、类型、角色或阶段差异做对比规划，缺少数据时保留补数提示。`],
    evidence: ['证据与缺口', `列出${evidence}、当前不可见内容和后续需要补齐的数据。`],
    risk: ['风险与机会', `提炼${subject}中需要优先处理的风险、影响范围和可推进机会。`],
    'next-steps': ['建议动作', `把${subject}结论转成后续动作、责任线索和验收标准。`],
  };
  return copies[module.id] || copies[role] || [module.title, module.content];
}

function applyPlanningBriefToModules(modules, { templateId = '', brief = {} } = {}) {
  return modules.map((module) => {
    const [title, content] = plannedModuleCopy(module, {
      templateId,
      subject: brief.subject || '当前主题',
      brief,
    });
    return mergeModule(module, { title, content });
  });
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
  templateReferenceId = null,
  templateReferenceFallbackId = null,
  templateIntent = '',
} = {}) {
  const templateInferenceInput = [
    templateIntent,
    conversationSummary,
    fieldCandidates,
  ].filter(Boolean);
  const resolvedTemplateReferenceId = templateReferenceId
    || inferStaticPageTemplateReferenceId(templateInferenceInput)
    || templateReferenceFallbackId;
  const templateSeed = staticPageTemplateReferenceDraftSeed(resolvedTemplateReferenceId);
  const hasPlanningContext = Boolean(String(templateIntent || '').trim() || String(conversationSummary || '').trim());
  const planningBrief = buildStaticPagePlanningBrief({
    templateIntent,
    conversationSummary,
    templateLabel: templateSeed?.designReference?.label || '',
  });
  const seedModules = clone(templateSeed?.modules || DEFAULT_STATIC_PAGE_MODULES);
  const modules = hasPlanningContext
    ? applyPlanningBriefToModules(seedModules, {
        templateId: templateSeed?.templateId || '',
        brief: planningBrief,
      })
    : seedModules;
  const styleDirection = templateSeed?.styleDirection || DEFAULT_STYLE_DIRECTION;
  const designReferences = templateSeed ? [templateSeed.designReference] : [];
  const draft = {
    id: `draft-local-${datasetId || 'dataset'}-${sessionId || 'session'}`,
    datasetId,
    sessionId,
    source: {
      conversationSummary,
      selectedMessageIds: [],
      evidenceIds: Array.isArray(evidenceIds) ? [...evidenceIds] : [],
      fieldCandidates: Array.isArray(fieldCandidates) ? [...fieldCandidates] : [],
      templateReferences: designReferences,
      planningBrief,
    },
    status: 'planning',
    templateReferenceId: templateSeed?.templateId || '',
    objective: hasPlanningContext
      ? planningBrief.userGoal || templateSeed?.objective || '给客户展示当前数据结论，并生成可交付静态页'
      : templateSeed?.objective || '给客户展示当前数据结论，并生成可交付静态页',
    audience: templateSeed?.audience || '客户决策层',
    styleDirection,
    modelSummary: hasPlanningContext
      ? planningBrief.sourceMarkdown || conversationSummary || templateSeed?.modelSummary || '模型将根据当前会话和数据集生成静态页结构。'
      : templateSeed?.modelSummary || '模型将根据当前会话和数据集生成静态页结构。',
    designReferences,
    mobileOrder: modules.map((module) => module.id),
    modules,
    visualSpec: buildStaticPageVisualSpec(styleDirection),
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
        ? (operation.queueMessage || next.imageJob.queueMessage || '可视化生成失败')
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
    const directHtml = Boolean(operation.directHtml || backendFinalPage?.directHtml);
    next.status = backendFinalPage?.status === 'rendered' ? 'rendered' : 'rendering';
    next.finalPage = backendFinalPage
      ? {
          ...next.finalPage,
          ...backendFinalPage,
          status: backendFinalPage.status || 'rendered',
          renderer: backendFinalPage.renderer || 'platform-api-static-page-renderer',
          directHtml,
          payload: backendFinalPage.payload || buildStaticPageFinalRenderPayload(next),
        }
      : {
          ...next.finalPage,
          status: 'mock_ready',
          renderer: 'local-static-page-mock',
          directHtml,
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

export function canRequestStaticPageDirectHtml(draft = {}) {
  return staticPageDirectHtmlBlockReason(draft) === '';
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
  if (status && !['confirmed', 'ready', 'non_chart', 'partial'].includes(status)) return true;

  const chartDataFit = String(
    binding.chartDataFit
      || binding.chart_data_fit
      || binding.bindingQuality?.chartDataFit
      || binding.binding_quality?.chart_data_fit
      || '',
  ).trim();
  if (chartDataFit && !['ready', 'not_required', 'non_chart_ready', 'needs_sample_rows', 'inferred_signal'].includes(chartDataFit)) return true;

  const visualizationType = String(binding.visualizationType || binding.visualization_type || '').trim();
  const hasBinding = bindingHasSource(binding);
  return VISUALIZATIONS_REQUIRING_SAMPLE_ROWS.has(visualizationType) && bindingSampleRows(binding) === 0 && !hasBinding;
}

function bindingHasSource(binding = {}) {
  const source = binding.binding || binding.dataBinding || binding.data_binding || {};
  const quality = binding.bindingQuality || binding.binding_quality || {};
  return Boolean(
    binding.sourceId
      || binding.source_id
      || binding.fieldPath
      || binding.field_path
      || source.sourceId
      || source.source_id
      || source.fieldPath
      || source.field_path
      || source.label
      || quality.sourceId
      || quality.source_id
      || quality.fieldPath
      || quality.field_path,
  );
}

function bindingNeedsFinalAttention(binding = {}) {
  const status = String(
    binding.bindingQualityStatus
      || binding.binding_quality_status
      || binding.bindingQuality?.status
      || binding.binding_quality?.status
      || '',
  ).trim();
  if (status && !['confirmed', 'ready', 'non_chart', 'partial'].includes(status)) return true;

  const chartDataFit = String(
    binding.chartDataFit
      || binding.chart_data_fit
      || binding.bindingQuality?.chartDataFit
      || binding.binding_quality?.chart_data_fit
      || '',
  ).trim();
  if (chartDataFit && !['ready', 'not_required', 'non_chart_ready', 'needs_sample_rows', 'inferred_signal'].includes(chartDataFit)) return true;

  const visualizationType = String(binding.visualizationType || binding.visualization_type || '').trim();
  return VISUALIZATIONS_REQUIRING_SAMPLE_ROWS.has(visualizationType) && bindingSampleRows(binding) === 0 && !bindingHasSource(binding);
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

function staticPageDataQualityBlockReason(draft = {}, actionLabel, nextInstruction, { mode = 'final' } = {}) {
  const snapshot = draft?.dataSnapshot || draft?.data_snapshot || buildStaticPageDataSnapshot(draft);
  const bindings = snapshot?.moduleBindings || snapshot?.module_bindings || [];
  const predicate = mode === 'preview' ? bindingNeedsPreviewAttention : bindingNeedsFinalAttention;
  const attentionBindings = Array.isArray(bindings)
    ? bindings.filter(predicate)
    : [];
  if (!attentionBindings.length) return '';
  const labels = attentionBindings.slice(0, 3).map(bindingPreviewLabel).join('、');
  return `当前静态页还有 ${attentionBindings.length} 个模块的数据绑定未达到${actionLabel}：${labels}。${nextInstruction}`;
}

export function staticPagePreviewBlockReason(draft = {}) {
  return staticPageDataQualityBlockReason(
    draft,
    '可视化生成要求',
    '请先调整可视化文案，或让 DataMax 检索/修复内容来源；只有缺少来源的模块会阻断可视化，缺少样本行的图表会先作为设计预览进入出图。',
    { mode: 'preview' },
  );
}

export function staticPageFinalRenderBlockReason(draft = {}) {
  if (draft?.previewContract?.status === 'stale' || draft?.imageJob?.status === 'stale') {
    return '规划已经改过，需要重新生成可视化。';
  }
  const finalStatus = draft?.finalPage?.status || '';
  const retryableFinalStatus = finalStatus === 'failed' || finalStatus === 'cancelled';
  const previewStatus = draft?.previewContract?.status || '';
  const imageJobStatus = draft?.imageJob?.status || '';
  const effectivePreviewStatus = ['preview_ready', 'confirmed'].includes(imageJobStatus)
    ? imageJobStatus
    : previewStatus;
  const previewReady = previewStatus === 'preview_ready'
    || previewStatus === 'confirmed'
    || imageJobStatus === 'preview_ready'
    || imageJobStatus === 'confirmed'
    || draft?.status === 'preview_ready'
    || draft?.status === 'effect_confirmed';
  if (!previewReady && !retryableFinalStatus) {
    return '先生成可视化，再按可视化制作可交付静态页。';
  }
  if (!['preview_ready', 'confirmed'].includes(effectivePreviewStatus) && !retryableFinalStatus) {
    return '可视化状态未同步，请刷新或重新生成可视化。';
  }
  if (!draft?.previewImage?.assetKey && !draft?.previewContract?.assetKey) {
    return '可视化资源缺失，请重新生成可视化。';
  }
  return '';
}

export function staticPageDirectHtmlBlockReason(draft = {}) {
  if (staticPageUsesTemplateOrStyleGuide(draft)) {
    return '已引用模板或风格指南，需先用 GPT-Image2 生成视觉可视化，再按图和真实数据制作静态页。';
  }
  return '';
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

function compactStaticPageImagePromptText(value, maxLength = 600) {
  const text = String(value || '').replace(/\s+/g, ' ').trim();
  if (!text) return '';
  return text.length > maxLength ? `${text.slice(0, maxLength - 3)}...` : text;
}

function normalizeStaticPageConfirmedPromptText(value, maxLength = 6000) {
  const text = String(value || '').replace(/\r\n/g, '\n').replace(/\r/g, '\n').trim();
  if (!text) return '';
  return text.length > maxLength ? `${text.slice(0, maxLength - 3)}...` : text;
}

function staticPageStyleDirectionLabel(styleDirection) {
  return STATIC_PAGE_STYLE_DIRECTIONS.find((item) => item.key === styleDirection)?.label
    || styleDirection
    || '客户交付报告';
}

function staticPageImagePromptModuleLine(module = {}, index = 0) {
  const visualization = normalizeVisualization(module.visualization || {});
  const layout = normalizeLayout(module.layout || {});
  const title = module.title || module.id || `模块${index + 1}`;
  const content = compactStaticPageImagePromptText(module.content, 180);
  const dataLabel = module.dataBinding?.label || module.dataLabel || module.data_label || '';
  const visualLabel = visualization.label || visualizationLabel(visualization.type);
  const parts = [
    `${index + 1}. ${title}`,
    content ? `内容：${content}` : '',
    visualLabel ? `图形：${visualLabel}` : '',
    dataLabel ? `数据来源：${dataLabel}` : '',
    `版位：${layout.w}x${layout.h}`,
  ].filter(Boolean);
  return `- ${parts.join('；')}`;
}

function staticPagePromptBindings(dataSnapshot = {}) {
  return Array.isArray(dataSnapshot.moduleBindings)
    ? dataSnapshot.moduleBindings
    : Array.isArray(dataSnapshot.module_bindings)
      ? dataSnapshot.module_bindings
      : [];
}

function staticPagePromptRows(value) {
  if (!value) return [];
  if (Array.isArray(value)) return value;
  if (Array.isArray(value.sampleData)) return value.sampleData;
  if (Array.isArray(value.sample_data)) return value.sample_data;
  if (Array.isArray(value.rows)) return value.rows;
  if (Array.isArray(value.data)) return value.data;
  if (Array.isArray(value.values)) return value.values;
  return [];
}

function staticPagePromptValue(row = {}) {
  const raw = row.value ?? row.amount ?? row.count ?? row.total ?? row.y ?? row.xuzengxiaoshou ?? row.quekou;
  const number = Number(String(raw ?? '').replace(/[,，%]/g, ''));
  return Number.isFinite(number) ? number : null;
}

function staticPagePromptLabel(row = {}, index = 0) {
  return String(
    row.label
      || row.name
      || row.date
      || row.month
      || row.period
      || row.category
      || row.store_init
      || row.shopdesc
      || row.brandcode
      || row.title
      || `项${index + 1}`,
  ).trim();
}

function formatStaticPagePromptNumber(value) {
  const number = Number(value);
  if (!Number.isFinite(number)) return '';
  const abs = Math.abs(number);
  if (abs >= 100000000) return `${(number / 100000000).toFixed(1)}亿`;
  if (abs >= 10000) return `${(number / 10000).toFixed(1)}万`;
  return Math.abs(number - Math.round(number)) < 0.000001 ? String(Math.round(number)) : number.toFixed(1);
}

function compactStaticPagePromptRows(rows = [], limit = 5) {
  return rows
    .slice(0, limit)
    .map((row, index) => {
      const label = staticPagePromptLabel(row, index);
      const value = staticPagePromptValue(row);
      return value === null ? label : `${label} ${formatStaticPagePromptNumber(value)}`;
    })
    .filter(Boolean)
    .join('、');
}

function staticPagePromptBindingByNeedle(dataSnapshot, needles = []) {
  const loweredNeedles = needles.map((item) => String(item || '').toLowerCase());
  return staticPagePromptBindings(dataSnapshot).find((binding) => {
    const text = [
      binding.moduleId,
      binding.module_id,
      binding.id,
      binding.title,
      binding.label,
      binding.binding?.label,
      binding.binding?.fieldPath,
      binding.binding?.field_path,
    ].filter(Boolean).join(' ').toLowerCase();
    return loweredNeedles.some((needle) => text.includes(needle));
  });
}

function collectStaticPagePromptDates(value, out = new Set(), depth = 0) {
  if (!value || depth > 8) return out;
  if (typeof value === 'string') {
    const match = value.trim().match(/^\d{4}-\d{2}-\d{2}/);
    if (match) out.add(match[0]);
    return out;
  }
  if (Array.isArray(value)) {
    value.forEach((item) => collectStaticPagePromptDates(item, out, depth + 1));
    return out;
  }
  if (typeof value === 'object') {
    Object.values(value).forEach((item) => collectStaticPagePromptDates(item, out, depth + 1));
  }
  return out;
}

function staticPagePromptCommissionRows(dataSnapshot = {}) {
  const directRows = [
    dataSnapshot.commissionTargets,
    dataSnapshot.commission_targets,
    dataSnapshot.highCommissionTargets,
    dataSnapshot.high_commission_targets,
    dataSnapshot.brandTargets,
    dataSnapshot.brand_targets,
  ].flatMap(staticPagePromptRows);
  const bindingRows = staticPagePromptBindings(dataSnapshot)
    .filter((binding) => {
      const text = [
        binding.moduleId,
        binding.module_id,
        binding.title,
        binding.label,
      ].filter(Boolean).join(' ').toLowerCase();
      return /commission|brand|target|分成|品牌|助推|高分成/.test(text);
    })
    .flatMap(staticPagePromptRows);
  return [...directRows, ...bindingRows]
    .filter((row) => row && typeof row === 'object')
    .filter((row) => row.shopdesc || row.brandcode || row.brand || row.brandName || row['品牌']);
}

function buildStaticPageImageDataRequirementLines(draft = {}) {
  const dataSnapshot = draft.dataSnapshot || buildStaticPageDataSnapshot(draft);
  const lines = [];
  const dates = Array.from(collectStaticPagePromptDates(dataSnapshot)).sort();
  if (dates.length) {
    lines.push(`实际时间范围：${dates[0]} ~ ${dates[dates.length - 1]}。`);
  }
  const overviewRows = staticPagePromptRows(staticPagePromptBindingByNeedle(dataSnapshot, ['overview', '总览']));
  if (overviewRows.length) {
    lines.push(`真实总览指标：${compactStaticPagePromptRows(overviewRows, 6)}。`);
  }
  const levelRows = staticPagePromptRows(staticPagePromptBindingByNeedle(dataSnapshot, ['warning-level', '预警等级']));
  if (levelRows.length) {
    lines.push(`预警等级分布：${compactStaticPagePromptRows(levelRows, 6)}。`);
  }
  const highStoreRows = staticPagePromptRows(staticPagePromptBindingByNeedle(dataSnapshot, ['store-high', '门店高预警', '高预警门店']));
  if (highStoreRows.length) {
    lines.push(`高预警门店排行：${compactStaticPagePromptRows(highStoreRows, 6)}。`);
  }
  const gapRows = staticPagePromptRows(staticPagePromptBindingByNeedle(dataSnapshot, ['store-gap', '缺口', '需增']));
  if (gapRows.length) {
    lines.push(`门店增收缺口机会：${compactStaticPagePromptRows(gapRows, 6)}。`);
  }
  const commissionRows = staticPagePromptCommissionRows(dataSnapshot);
  if (commissionRows.length) {
    lines.push(`品牌/店铺高分成线明细：${compactStaticPagePromptRows(commissionRows, 6)}。`);
  } else if (highStoreRows.length || gapRows.length) {
    lines.push('品牌/店铺明细：当前快照未提供 shopdesc/brandcode 行，不要编造品牌名单；视觉稿先展示门店级机会池，并保留“明细待同步/待钻取”的表达。');
  }
  return lines;
}

function buildStaticPageImageProductionRules(draft = {}) {
  const subject = `${draft.objective || ''}\n${draft.title || ''}\n${draft.modelSummary || ''}`;
  const storeOrBrandPage = /新世界|新百|门店|分店|高分成|品牌店|店总|经营分析/.test(subject);
  const templateOrStyleGuidePage = staticPageUsesTemplateOrStyleGuide(draft);
  const fixedTemplatePublish = storeOrBrandPage || templateOrStyleGuidePage;
  return {
    workflow: 'requirements_to_image2_then_image_to_html',
    imageFirst: true,
    realDataRequired: true,
    fakeDataAllowed: false,
    snapshotAggregationPolicy: 'latest_snapshot_for_state_modules',
    trendAggregationPolicy: 'date_series_only_for_trends',
    unitPolicy: 'format raw amounts as 万 below 1亿 and 亿 at or above 1亿; investigate aggregation before changing units',
    detailPolicy: storeOrBrandPage
      ? 'store/brand/customer detail lists are required for actionable store-manager pages'
      : templateOrStyleGuidePage
        ? 'style-guide/template pages must preserve the visual system through Image2 before HTML production'
        : 'include drill-down details when the customer asks for action lists',
    publishTarget: 'v3_generated_artifacts_on_8_server',
    codexHostEscalation: {
      eligible: true,
      defaultMode: fixedTemplatePublish ? 'fixed_template_exec_schema' : 'plan_only_or_dry_run',
      templateId: fixedTemplatePublish ? 'static_page_image2_data_publish' : '',
      requiredCapability: fixedTemplatePublish ? 'static_page_image2_data_publish' : 'static_page_edit',
      legacyCapability: fixedTemplatePublish ? 'static_page_advanced_publish' : '',
      confirmationPolicy: fixedTemplatePublish ? 'auto_for_new_generated_artifact' : 'human_review_for_writes',
      publishMode: fixedTemplatePublish ? 'new_generated_artifact_only' : 'manual_or_draft_only',
      humanReviewRequiredForWrites: !fixedTemplatePublish,
      humanReviewRequiredForOverwrite: true,
      humanReviewRequiredForUncertainDataPolicy: true,
    },
  };
}

function staticPageUsesTemplateOrStyleGuide(draft = {}) {
  const designReferences = normalizeTemplateDesignReferences(
    draft.designReferences || draft.source?.templateReferences || draft.dataSnapshot?.designReferences || [],
  );
  if (designReferences.length > 0 || draft.templateReferenceId || draft.template_reference_id) {
    return true;
  }
  const subject = [
    draft.objective,
    draft.title,
    draft.modelSummary,
    draft.source?.templateEvidenceSummary,
    draft.templateEvidenceSummary,
  ].filter(Boolean).join('\n');
  return /风格指南|样式指南|视觉规范|设计规范|style\s*guide|data\s*buddy|模板参考|引用模板/i.test(subject);
}

function buildStaticPageImageBusinessRequirementLines(draft = {}) {
  const subject = draft.objective || draft.title || '经营分析静态页';
  const isStoreTopic = /新世界|新百|门店|分店|高分成|品牌店|店总/.test(`${subject}\n${draft.modelSummary || ''}`);
  const isTemplateOrStyleGuidePage = staticPageUsesTemplateOrStyleGuide(draft);
  return [
    '顶部默认要有时间选择区，视觉上可以像筛选条，但不要做成后台编辑框。',
    '默认要有主要分区选择；如果是新世界/新百项目，主分区优先呈现不同门店/分店。',
    '页面要体现数据可刷新：文档或数据库更新后，页面数据应能按最新快照更新。',
    isStoreTopic
      ? '新世界/新百这版要服务店总：让店总快速看到哪些品牌店或门店快达到高分成线，便于运营助推。'
      : '',
    isTemplateOrStyleGuidePage
      ? '已引用模板或风格指南：必须先把模板视觉语言交给 GPT-Image2 出可视化，再根据可视化制作最终 HTML；不要直接把风格指南翻译成低保真 HTML。'
      : '',
    '如果数据来自快照表，KPI、排行、机会池和明细默认只取最新快照；只有趋势图可以按日期序列展开，禁止把多天快照重复累加成当前状态。',
    '金额单位必须先校验取数口径，再按数值展示：低于1亿用“万”，达到1亿才用“亿”，不要用改单位掩盖聚合错误。',
    '这是生图需求，不是固定模块版位；请让 GPT-Image2 先发挥视觉设计能力，后续系统会读图并接入真实数据制作 HTML。',
  ].filter(Boolean);
}

export function buildStaticPageImagePromptText(draft = {}) {
  const designReferences = normalizeTemplateDesignReferences(
    draft.designReferences || draft.source?.templateReferences || [],
  );
  const templateLine = designReferences.length
    ? `参考版式：${designReferences.slice(0, 3).map((item) => item.title || item.templateId || item.source).filter(Boolean).join('、')}`
    : '';
  const businessRequirementLines = buildStaticPageImageBusinessRequirementLines(draft);
  const dataRequirementLines = buildStaticPageImageDataRequirementLines(draft);
  const productionRules = buildStaticPageImageProductionRules(draft);
  const lines = [
    '请用 GPT-Image2 先生成一张 1536x1024 的中文企业静态页可视化预览图。',
    `主题：${draft.objective || draft.title || '经营分析静态页'}`,
    `受众：${draft.audience || '客户决策层'}`,
    `风格：${staticPageStyleDirectionLabel(draft.styleDirection)}`,
    templateLine,
    draft.modelSummary ? `资料与目标：${compactStaticPageImagePromptText(draft.modelSummary, 420)}` : '',
    '业务要求：',
    ...businessRequirementLines.map((line) => `- ${line}`),
    dataRequirementLines.length ? '真实数据摘要：' : '',
    ...dataRequirementLines.map((line) => `- ${line}`),
    '后续制作与发布要求：',
    `- 视觉稿生成后，系统应按 ${productionRules.workflow} 制作真实数据 HTML。`,
    '- 高级取数、口径纠偏、明细补齐或发布类需求，可升级为 DataMax 受控 Codex Host 固定模板任务；新 generated-artifact 产物发布可自动执行，覆盖旧产物、稳定链接替换、口径不确定或扩大权限仍需人工确认。',
    '视觉要求：画面像客户汇报页或经营分析页，不要浏览器边框，不要后台管理系统，不要出现可编辑文本框或拖拽编辑框。',
    '重点：中文标题清晰，图表关系可信，适合客户直接看效果；不要生成假数字、假门店、假品牌。',
    '完成后必须返回一张图片 artifact。',
  ].filter(Boolean);
  return lines.join('\n');
}

export function buildStaticPageImagePayload(draft, { oneClick = false, promptText = '', promptOnly = false } = {}) {
  const visualSpec = draft.visualSpec || buildStaticPageVisualSpec(draft.styleDirection);
  const renderSpec = draft.renderSpec || buildStaticPageRenderSpec();
  const designReferences = normalizeTemplateDesignReferences(
    draft.designReferences || draft.source?.templateReferences || [],
  );
  const dataSnapshot = draft.dataSnapshot || buildStaticPageDataSnapshot(draft);
  const productionRules = buildStaticPageImageProductionRules(draft);
  const normalizedPromptText = normalizeStaticPageConfirmedPromptText(promptText, 6000);
  const promptMetadata = {};
  if (normalizedPromptText) {
    promptMetadata.prompt = normalizedPromptText;
    promptMetadata.promptText = normalizedPromptText;
    promptMetadata.prompt_text = normalizedPromptText;
  }
  if (promptOnly) {
    promptMetadata.promptOnly = true;
    promptMetadata.prompt_only = true;
  }
  if (promptOnly) {
    return {
      draftId: draft.id,
      datasetId: draft.datasetId,
      sessionId: draft.sessionId,
      oneClick,
      ...promptMetadata,
      objective: draft.objective,
      audience: draft.audience,
      styleDirection: draft.styleDirection,
      designReferences,
      visualSpec,
      renderSpec: {
        ...renderSpec,
        workflow: productionRules.workflow,
        finalHtmlRule: 'After the effect image is confirmed, infer the visual layout from the image and connect real dataSnapshot values into HTML.',
        snapshotAggregationPolicy: productionRules.snapshotAggregationPolicy,
        trendAggregationPolicy: productionRules.trendAggregationPolicy,
        publishTarget: productionRules.publishTarget,
        fixedTaskTemplateId: productionRules.codexHostEscalation.templateId || '',
        fixedTaskPublishMode: productionRules.codexHostEscalation.publishMode || '',
      },
      dataSnapshot,
      previewContract: draft.previewContract || buildStaticPagePreviewContract(draft),
      imageFirstContract: {
        role: 'requirements_prompt_for_gpt_image_2',
        rule: 'Do not treat draft modules or layout as a fixed page plan; use confirmed prompt text as the source of truth for image generation.',
        finalRenderRule: 'Confirmed image is the visual blueprint; final HTML should be produced after reading the image and binding real data.',
        realDataRequired: true,
        fakeDataAllowed: false,
        snapshotAggregationPolicy: productionRules.snapshotAggregationPolicy,
        trendAggregationPolicy: productionRules.trendAggregationPolicy,
        fixedTaskTemplateId: productionRules.codexHostEscalation.templateId || '',
        confirmationPolicy: productionRules.codexHostEscalation.confirmationPolicy || '',
      },
      productionRules,
      dataRequirements: buildStaticPageImageDataRequirementLines({ ...draft, dataSnapshot }),
      businessRequirements: buildStaticPageImageBusinessRequirementLines(draft),
      modelSummary: draft.modelSummary,
    };
  }
  return {
    draftId: draft.id,
    datasetId: draft.datasetId,
    sessionId: draft.sessionId,
    oneClick,
    ...promptMetadata,
    objective: draft.objective,
    audience: draft.audience,
    styleDirection: draft.styleDirection,
    designReferences,
    visualSpec,
    renderSpec,
    dataSnapshot: draft.dataSnapshot || buildStaticPageDataSnapshot(draft),
    productionRules,
    previewContract: draft.previewContract || buildStaticPagePreviewContract(draft),
    designContract: {
      contractSource: 'StaticPageDraft',
      renderer: renderSpec.renderer,
      editableCore: renderSpec.componentModel,
      guardrails: renderSpec.generationGuardrails,
      templateReferences: designReferences,
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
  const designReferences = normalizeTemplateDesignReferences(
    draft.designReferences || draft.source?.templateReferences || [],
  );
  const dataSnapshot = draft.dataSnapshot || buildStaticPageDataSnapshot(draft);
  return {
    draftId: draft.id,
    styleDirection: draft.styleDirection,
    designReferences,
    visualSpec: draft.visualSpec || buildStaticPageVisualSpec(draft.styleDirection),
    renderSpec: draft.renderSpec || buildStaticPageRenderSpec(),
    dataSnapshot,
    previewContract: draft.previewContract || buildStaticPagePreviewContract(draft),
    previewImage: draft.previewImage,
    imageFirstContract: {
      role: 'generated_effect_image_to_html_blueprint',
      rule: 'Use the generated effect image as the visual blueprint, then bind real dataSnapshot values into DOM/SVG/HTML.',
      dataRequirements: buildStaticPageImageDataRequirementLines({ ...draft, dataSnapshot }),
      fakeDataAllowed: false,
    },
    mobileOrder: normalizeMobileOrder(draft.modules, draft.mobileOrder),
    modules: draft.modules.map((module) => ({
      ...clone(module),
      layout: normalizeLayout(module.layout),
    })),
  };
}

const DEFAULT_STATIC_PAGE_QUEUE_MESSAGE = '资源正在排队，可以联系商务开通高级用户跳过等待。';

export function mergeBackendStaticPageDraft(localDraft, backendDraft) {
  const payload = backendDraft?.draft_payload && typeof backendDraft.draft_payload === 'object'
    ? backendDraft.draft_payload
    : {};
  const matchedDatasetIds = [
    ...new Set([
      ...(Array.isArray(localDraft?.matchedDatasetIds) ? localDraft.matchedDatasetIds : []),
      ...(Array.isArray(localDraft?.matched_dataset_ids) ? localDraft.matched_dataset_ids : []),
      ...(Array.isArray(payload?.matchedDatasetIds) ? payload.matchedDatasetIds : []),
      ...(Array.isArray(payload?.matched_dataset_ids) ? payload.matched_dataset_ids : []),
      ...(Array.isArray(backendDraft?.matchedDatasetIds) ? backendDraft.matchedDatasetIds : []),
      ...(Array.isArray(backendDraft?.matched_dataset_ids) ? backendDraft.matched_dataset_ids : []),
    ]),
  ];
  const merged = {
    ...localDraft,
    ...payload,
    id: backendDraft?.id || localDraft.id,
    localDraftId: localDraft.localDraftId || localDraft.id,
    backendDraftId: backendDraft?.id || localDraft.backendDraftId || '',
    assistantRunId: backendDraft?.assistant_run_id || localDraft.assistantRunId || '',
    backendStatus: backendDraft?.status || localDraft.backendStatus || '',
    backendUpdatedAt: backendDraft?.updated_at || localDraft.backendUpdatedAt || '',
    source_refs: backendDraft?.source_refs || localDraft.source_refs || payload.source_refs || null,
    sourceRefs: backendDraft?.source_refs || localDraft.sourceRefs || payload.sourceRefs || payload.source_refs || null,
    draft_payload: backendDraft?.draft_payload || localDraft.draft_payload || payload,
    draftPayload: backendDraft?.draft_payload || localDraft.draftPayload || payload,
    matchedDatasetIds,
  };
  if (backendDraft?.status === 'rendered' && merged.finalPage?.status === 'rendered') {
    merged.status = 'rendered';
  }
  if (backendDraft?.status === 'archived') {
    merged.status = 'archived';
  }
  if (backendDraft?.status === 'confirmed' && merged.status !== 'rendered') {
    merged.status = 'effect_confirmed';
  }
  return merged;
}

export function normalizeBackendStaticPageDraft(backendDraft) {
  const payload = backendDraft?.draft_payload && typeof backendDraft.draft_payload === 'object'
    ? backendDraft.draft_payload
    : {};
  return mergeBackendStaticPageDraft({
    ...payload,
    id: payload.id || backendDraft?.id,
    localDraftId: payload.localDraftId || backendDraft?.source_refs?.local_draft_id || backendDraft?.id,
  }, backendDraft);
}

export function mergeStaticPageRenderOutput(draft, renderOutput) {
  if (!draft || !renderOutput) {
    return draft;
  }
  const renderStatus = renderOutput.status || draft.finalPage?.status || 'rendered';
  const nextStatus = renderStatus === 'rendered'
    ? 'rendered'
    : ['queued', 'rendering'].includes(renderStatus)
      ? 'rendering'
      : draft.status;
  return {
    ...draft,
    status: nextStatus,
    finalPage: {
      ...(draft.finalPage || {}),
      status: renderStatus,
      renderer: 'platform-api-static-page-renderer',
      renderOutputId: renderOutput.id,
      imageJobId: renderOutput.image_job_id || draft.finalPage?.imageJobId || null,
      assetManifest: renderOutput.asset_manifest || draft.finalPage?.assetManifest || {},
      html: renderOutput.html || draft.finalPage?.html || '',
      htmlPreviewUrl: renderOutput.html_preview_url || renderOutput.htmlPreviewUrl || draft.finalPage?.htmlPreviewUrl || '',
      htmlDownloadUrl: renderOutput.html_download_url || renderOutput.htmlDownloadUrl || draft.finalPage?.htmlDownloadUrl || '',
      directHtml: Boolean(draft.finalPage?.directHtml || renderOutput.asset_manifest?.directHtml || renderOutput.asset_manifest?.direct_html),
    },
  };
}

export function buildConfirmedStaticPagePreview(draft, imageJob, fallbackPreview = null) {
  return {
    ...(fallbackPreview || buildMockStaticPagePreview(draft)),
    kind: 'static-page-effect-preview',
    assetKey: imageJob?.preview_asset_key || fallbackPreview?.assetKey || `static-page-previews/${imageJob?.id || draft.id}.json`,
    imageJobId: imageJob?.id || draft?.imageJob?.id || null,
  };
}

export function mergeStaticPageImageJob(draft, imageJob, { queueMessage = DEFAULT_STATIC_PAGE_QUEUE_MESSAGE } = {}) {
  if (!draft || !imageJob) {
    return draft;
  }
  const status = imageJob.status === 'confirmed' ? 'confirmed' : imageJob.status;
  const nextStatus = (() => {
    if (draft.status === 'rendered' || draft.status === 'effect_confirmed') {
      return draft.status;
    }
    if (status === 'preview_ready') {
      return 'preview_ready';
    }
    if (status === 'confirmed') {
      return 'effect_confirmed';
    }
    if (status === 'queued' || status === 'running') {
      return 'queued';
    }
    if (status === 'failed') {
      return 'planning';
    }
    return draft.status;
  })();
  return {
    ...draft,
    status: nextStatus,
    imageJob: {
      ...(draft.imageJob || {}),
      id: imageJob.id,
      status,
      queuePosition: imageJob.queue_position ?? null,
      queueMessage: imageJob.failure_reason
        || (status === 'preview_ready' ? '可视化已生成，将自动继续制作页面。' : queueMessage),
    },
    previewImage: imageJob.preview_asset_key
      ? buildConfirmedStaticPagePreview(draft, imageJob, draft.previewImage)
      : draft.previewImage,
    previewContract: imageJob.preview_asset_key && status === 'preview_ready'
      ? {
          ...(draft.previewContract || {}),
          status: 'preview_ready',
          imageJobId: imageJob.id,
          assetKey: imageJob.preview_asset_key,
          queuePosition: null,
        }
      : imageJob.preview_asset_key && status === 'confirmed'
        ? {
            ...(draft.previewContract || {}),
            status: 'confirmed',
            imageJobId: imageJob.id,
            assetKey: imageJob.preview_asset_key,
            queuePosition: null,
          }
        : draft.previewContract,
  };
}

export function isBackendStaticPageImageJobId(jobId) {
  return Boolean(jobId) && !String(jobId).startsWith('mock-image-job-');
}

export function staticPageImageJobQueueOperation(imageJob, fallback = {}, { queueMessage = DEFAULT_STATIC_PAGE_QUEUE_MESSAGE } = {}) {
  return {
    type: 'queue_image_job',
    jobId: imageJob?.id || fallback.jobId || fallback.id,
    queuePosition: imageJob?.queue_position ?? imageJob?.queuePosition ?? fallback.queuePosition ?? null,
    queueMessage: fallback.queueMessage || queueMessage,
  };
}

export function staticPageOperationIsPromptOnly(operation = {}) {
  const payload = operation.imagePromptPayload || operation.image_prompt_payload || {};
  return Boolean(
    operation.promptOnly
      || operation.prompt_only
      || payload.promptOnly
      || payload.prompt_only,
  );
}

export function buildPromptOnlyStaticPageQueueOperation(
  draft,
  operation = {},
  { queueMessage = DEFAULT_STATIC_PAGE_QUEUE_MESSAGE } = {},
) {
  const prompt = String(
    operation.prompt
      || operation.promptText
      || operation.prompt_text
      || buildStaticPageImagePromptText(draft),
  ).trim();
  const providedPayload = operation.imagePromptPayload || operation.image_prompt_payload;
  const basePayload = providedPayload && typeof providedPayload === 'object' && !Array.isArray(providedPayload)
    ? providedPayload
    : buildStaticPageImagePayload(draft, {
      oneClick: Boolean(operation.oneClick || draft?.source?.oneClick),
      promptText: prompt,
      promptOnly: true,
    });
  const imagePromptPayload = {
    ...basePayload,
    prompt,
    promptText: prompt,
    prompt_text: prompt,
    promptOnly: true,
    prompt_only: true,
  };
  return {
    ...operation,
    type: 'queue_image_job',
    prompt,
    promptText: prompt,
    prompt_text: prompt,
    promptOnly: true,
    prompt_only: true,
    queueMessage: operation.queueMessage || queueMessage,
    imagePromptPayload,
  };
}
