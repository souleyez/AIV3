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
