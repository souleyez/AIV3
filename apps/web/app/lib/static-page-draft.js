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

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function visualizationLabel(type) {
  return STATIC_PAGE_VISUALIZATION_TYPES.find((item) => item.type === type)?.label || type;
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
    dataBinding: {
      ...module.dataBinding,
      ...(patch.dataBinding || {}),
    },
    visualization: {
      ...module.visualization,
      ...(patch.visualization || {}),
    },
    layout: normalizeLayout({
      ...module.layout,
      ...(patch.layout || {}),
    }),
  };

  if (next.visualization?.type) {
    next.visualization.label = next.visualization.label || visualizationLabel(next.visualization.type);
  }

  return next;
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

export function buildInitialStaticPageDraft({
  datasetId = null,
  sessionId = null,
  conversationSummary = '',
  evidenceIds = [],
} = {}) {
  const modules = clone(DEFAULT_STATIC_PAGE_MODULES);
  return {
    id: `draft-local-${datasetId || 'dataset'}-${sessionId || 'session'}`,
    datasetId,
    sessionId,
    source: {
      conversationSummary,
      selectedMessageIds: [],
      evidenceIds: Array.isArray(evidenceIds) ? [...evidenceIds] : [],
    },
    status: 'planning',
    objective: '给客户展示当前数据结论，并生成可交付静态页',
    audience: '客户决策层',
    styleDirection: DEFAULT_STYLE_DIRECTION,
    modelSummary: conversationSummary || '模型将根据当前会话和数据集生成静态页结构。',
    mobileOrder: modules.map((module) => module.id),
    modules,
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
  }

  if (type === 'refresh_summary' && operation.modelSummary) {
    next.modelSummary = String(operation.modelSummary);
  }

  if (type === 'queue_image_job') {
    next.status = 'queued';
    next.imageJob = {
      ...next.imageJob,
      id: operation.jobId || next.imageJob.id,
      status: 'queued',
      queuePosition: operation.queuePosition ?? next.imageJob.queuePosition,
      queueMessage: operation.queueMessage || '资源正在排队，可以联系商务开通高级用户跳过等待。',
    };
  }

  if (type === 'confirm_preview') {
    next.status = 'effect_confirmed';
    next.previewImage = operation.previewImage || next.previewImage;
  }

  if (type === 'request_final_render') {
    next.status = 'rendering';
    next.finalPage = {
      ...next.finalPage,
      status: 'queued',
    };
  }

  next.mobileOrder = normalizeMobileOrder(next.modules, next.mobileOrder);
  next.operations = [...(next.operations || []), clone(operation)];
  return next;
}

export function applyStaticPageOperations(draft, operations = []) {
  return operations.reduce((current, operation) => applyStaticPageOperation(current, operation), draft);
}

export function buildStaticPageImagePayload(draft, { oneClick = false } = {}) {
  return {
    draftId: draft.id,
    datasetId: draft.datasetId,
    sessionId: draft.sessionId,
    oneClick,
    objective: draft.objective,
    audience: draft.audience,
    styleDirection: draft.styleDirection,
    modelSummary: draft.modelSummary,
    modules: draft.modules.map((module) => ({
      id: module.id,
      title: module.title,
      content: module.content,
      dataLabel: module.dataBinding?.label || '',
      visualizationType: module.visualization?.type || 'text-insight',
      layout: normalizeLayout(module.layout),
    })),
  };
}

export function buildStaticPageFinalRenderPayload(draft) {
  return {
    draftId: draft.id,
    styleDirection: draft.styleDirection,
    previewImage: draft.previewImage,
    mobileOrder: normalizeMobileOrder(draft.modules, draft.mobileOrder),
    modules: draft.modules.map((module) => ({
      ...clone(module),
      layout: normalizeLayout(module.layout),
    })),
  };
}
