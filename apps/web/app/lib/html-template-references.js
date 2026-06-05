const TEMPLATE_SOURCE = Object.freeze({
  source: 'html-anything',
  sourceKind: 'template_design_reference',
  upstream: 'nexu-io/html-anything',
  license: 'Apache-2.0',
  importPolicy: 'metadata_and_constraints_only',
});

const SAFE_STATIC_PAGE_GUARDRAILS = Object.freeze([
  'template reference controls style and module recipe only',
  'DataMax model routing, permissions, datasets, evidence, and artifacts remain authoritative',
  'model output must become structured draft data, not raw final HTML',
  'missing or partial evidence must stay visible in draft and rendered output',
]);

const PAUSED_SURFACE_GUARDRAILS = Object.freeze([
  'reference retained for future design review only',
  'PPT, video, frame, and motion output tracks are paused in DataMax',
  'do not route this template into quick output until the relevant track is explicitly resumed',
]);

const DATA_BINDINGS = Object.freeze({
  model: { type: 'model_summary', label: '来自模型总结', sourceId: 'model' },
  session: { type: 'conversation_summary', label: '来自当前会话摘要', sourceId: 'session' },
  dataset: { type: 'dataset_metrics', label: '来自数据集指标摘要', sourceId: 'dataset' },
  evidence: { type: 'retrieval_evidence', label: '来自检索证据', sourceId: 'evidence' },
  selectedScope: { type: 'selected_scope', label: '来自当前选中范围', sourceId: 'selected_scope' },
});

const HTML_TEMPLATE_REFERENCES = Object.freeze([
  {
    id: 'data-report',
    label: '数据可视化报告',
    category: 'data',
    scenario: 'finance',
    surface: 'static_page',
    status: 'enabled',
    quickOutput: true,
    styleDirection: 'data-command',
    aspectHint: 'desktop-long-page',
    designIntent: '把可见 CSV、Excel、JSON、文档指标或会话数据整理成 KPI、趋势、对比和证据表。',
    objective: '快速生成一页数据可视化报告，展示关键指标、趋势、结构和可核查证据。',
    audience: '业务负责人和客户决策层',
    moduleBlueprints: [
      {
        id: 'hero',
        role: 'hero',
        title: '报告结论',
        content: '用一句话说明当前数据最重要的业务判断，并标出数据来源状态。',
        dataBinding: DATA_BINDINGS.session,
        visualization: { type: 'headline', label: '大标题 + 关键结论' },
        layout: { x: 0, y: 0, w: 12, h: 3 },
      },
      {
        id: 'kpi',
        role: 'metrics',
        title: '核心 KPI',
        content: '提炼 3-5 个最重要指标；没有可见数值时明确标注需要补充数据。',
        dataBinding: DATA_BINDINGS.dataset,
        visualization: { type: 'kpi-cards', label: '关键指标卡' },
        layout: { x: 0, y: 3, w: 5, h: 3 },
      },
      {
        id: 'trend',
        role: 'trend',
        title: '趋势变化',
        content: '展示关键指标随时间、阶段或类别的变化方向。',
        dataBinding: DATA_BINDINGS.evidence,
        visualization: { type: 'line-chart', label: '趋势折线图' },
        layout: { x: 5, y: 3, w: 7, h: 3 },
      },
      {
        id: 'comparison',
        role: 'comparison',
        title: '分类对比',
        content: '用对比图解释不同渠道、品类、地区或阶段的差异。',
        dataBinding: DATA_BINDINGS.selectedScope,
        visualization: { type: 'bar-chart', label: '分类对比柱状图' },
        layout: { x: 0, y: 6, w: 6, h: 4 },
      },
      {
        id: 'evidence',
        role: 'evidence',
        title: '证据与方法',
        content: '列出数据口径、可见证据和当前缺失项，避免模板隐藏不完整数据。',
        dataBinding: DATA_BINDINGS.evidence,
        visualization: { type: 'text-insight', label: '洞察文本块' },
        layout: { x: 6, y: 6, w: 6, h: 4 },
      },
    ],
    promptHints: [
      'prefer KPI cards, trend charts, comparison charts, and evidence notes',
      'never invent numbers; ask DataMax retrieval or data repair when chart rows are missing',
    ],
  },
  {
    id: 'dashboard',
    label: '管理后台仪表板',
    category: 'dashboard',
    scenario: 'operations',
    surface: 'static_page',
    status: 'enabled',
    quickOutput: true,
    styleDirection: 'data-command',
    aspectHint: 'desktop-dashboard',
    designIntent: '把运营状态整理成密集但可扫描的 KPI、趋势、风险和最近活动。',
    objective: '快速生成一页运营仪表板，帮助用户扫清当前状态、异常和下一步动作。',
    audience: '运营负责人和项目管理人员',
    moduleBlueprints: [
      {
        id: 'hero',
        role: 'hero',
        title: '运营总览',
        content: '总结当前状态、异常等级和本轮关注重点。',
        dataBinding: DATA_BINDINGS.session,
        visualization: { type: 'headline', label: '大标题 + 关键结论' },
        layout: { x: 0, y: 0, w: 12, h: 2 },
      },
      {
        id: 'kpi',
        role: 'metrics',
        title: '关键状态',
        content: '显示 3-5 个用于判断健康度、效率、进度或风险的指标。',
        dataBinding: DATA_BINDINGS.dataset,
        visualization: { type: 'kpi-cards', label: '关键指标卡' },
        layout: { x: 0, y: 2, w: 5, h: 3 },
      },
      {
        id: 'trend',
        role: 'trend',
        title: '运行趋势',
        content: '展示任务量、转化、响应、质量或异常的时间走势。',
        dataBinding: DATA_BINDINGS.evidence,
        visualization: { type: 'line-chart', label: '趋势折线图' },
        layout: { x: 5, y: 2, w: 7, h: 3 },
      },
      {
        id: 'risk',
        role: 'risk',
        title: '风险预警',
        content: '把阻塞项、异常项和机会点按优先级展示。',
        dataBinding: DATA_BINDINGS.evidence,
        visualization: { type: 'risk-matrix', label: '风险优先级矩阵' },
        layout: { x: 0, y: 5, w: 6, h: 4 },
      },
      {
        id: 'activity',
        role: 'activity',
        title: '最近动作',
        content: '列出最近更新、待处理动作和负责人线索；不可见时标注未供料。',
        dataBinding: DATA_BINDINGS.model,
        visualization: { type: 'timeline', label: '阶段时间线' },
        layout: { x: 6, y: 5, w: 6, h: 4 },
      },
    ],
    promptHints: [
      'favor dense status scanning over marketing hero composition',
      'surface unresolved risks and missing operational evidence',
    ],
  },
  {
    id: 'docs-page',
    label: '技术文档页',
    category: 'doc',
    scenario: 'engineering',
    surface: 'static_page',
    status: 'enabled',
    quickOutput: true,
    styleDirection: 'client-delivery',
    aspectHint: 'documentation-page',
    designIntent: '把文档、接口说明或方案内容整理成清晰的阅读页。',
    objective: '快速生成一页技术文档或交接说明，保留结构、步骤、注意事项和缺失信息。',
    audience: '技术对接人员和项目成员',
    moduleBlueprints: [
      {
        id: 'hero',
        role: 'hero',
        title: '文档概览',
        content: '说明这份文档解决什么问题、适用对象和当前信息完整度。',
        dataBinding: DATA_BINDINGS.session,
        visualization: { type: 'headline', label: '大标题 + 关键结论' },
        layout: { x: 0, y: 0, w: 12, h: 3 },
      },
      {
        id: 'scope',
        role: 'scope',
        title: '范围与边界',
        content: '列出系统边界、权限边界、已供料和未供料范围。',
        dataBinding: DATA_BINDINGS.evidence,
        visualization: { type: 'text-insight', label: '洞察文本块' },
        layout: { x: 0, y: 3, w: 6, h: 3 },
      },
      {
        id: 'steps',
        role: 'steps',
        title: '流程步骤',
        content: '把关键流程拆成可执行步骤，保留前后依赖。',
        dataBinding: DATA_BINDINGS.model,
        visualization: { type: 'timeline', label: '阶段时间线' },
        layout: { x: 6, y: 3, w: 6, h: 3 },
      },
      {
        id: 'interfaces',
        role: 'interfaces',
        title: '接口与数据',
        content: '整理接口、字段、输入输出或配置项；没有真实接口时标注待补。',
        dataBinding: DATA_BINDINGS.selectedScope,
        visualization: { type: 'text-insight', label: '洞察文本块' },
        layout: { x: 0, y: 6, w: 6, h: 4 },
      },
      {
        id: 'checks',
        role: 'checks',
        title: '校验与交付',
        content: '列出验证命令、验收标准、风险和下一步交付动作。',
        dataBinding: DATA_BINDINGS.model,
        visualization: { type: 'text-insight', label: '洞察文本块' },
        layout: { x: 6, y: 6, w: 6, h: 4 },
      },
    ],
    promptHints: [
      'preserve source headings and section hierarchy when DataMax supplied document detail',
      'show unavailable interface details as missing evidence instead of guessing',
    ],
  },
  {
    id: 'deck-swiss-international',
    label: '瑞士国际主义 Deck',
    category: 'slides',
    scenario: 'marketing',
    surface: 'deck',
    status: 'paused',
    pauseReason: 'ppt_video_track_frozen',
    quickOutput: false,
    styleDirection: 'client-delivery',
    aspectHint: '16:9-horizontal-deck',
    designIntent: '保留为未来 PPT/deck 视觉参考，当前不进入 DataMax 快速产出。',
    objective: '',
    audience: '',
    moduleBlueprints: [],
    promptHints: [],
    guardrails: PAUSED_SURFACE_GUARDRAILS,
  },
  {
    id: 'video-hyperframes',
    label: 'Hyperframes 视频帧',
    category: 'video',
    scenario: 'video',
    surface: 'video',
    status: 'paused',
    pauseReason: 'ppt_video_track_frozen',
    quickOutput: false,
    styleDirection: 'client-delivery',
    aspectHint: '1920x1080-frame-script',
    designIntent: '保留为未来 motion/frame 视觉参考，当前不进入 DataMax 快速产出。',
    objective: '',
    audience: '',
    moduleBlueprints: [],
    promptHints: [],
    guardrails: PAUSED_SURFACE_GUARDRAILS,
  },
]);

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function stringOrEmpty(value) {
  return typeof value === 'string' ? value.trim() : '';
}

function normalizeReferenceId(value) {
  const id = stringOrEmpty(value).toLowerCase();
  return /^[a-z0-9][a-z0-9-]*$/.test(id) ? id : '';
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function intentText(value) {
  if (Array.isArray(value)) {
    return value.map(intentText).filter(Boolean).join('\n');
  }
  if (value && typeof value === 'object') {
    return Object.values(value).map(intentText).filter(Boolean).join('\n');
  }
  return typeof value === 'string' ? value.toLowerCase() : '';
}

function textIncludesAny(text, keywords) {
  return keywords.some((keyword) => text.includes(keyword));
}

export function inferStaticPageTemplateReferenceId(value = '') {
  const text = intentText(value);
  if (!text.trim()) return '';
  if (textIncludesAny(text, [
    '技术方案',
    '接口',
    'api',
    'readme',
    '说明书',
    '交接',
    'handoff',
    'sop',
    '教程',
    '操作手册',
    '验收',
    '对接',
    '联调',
    '接入',
    '文档页',
    '文档说明',
    '文档指南',
    '文档中心',
    '文档规范',
    '文档清单',
  ])) {
    return 'docs-page';
  }
  if (textIncludesAny(text, [
    '看板',
    '仪表板',
    '仪表盘',
    'dashboard',
    '后台',
    '运营总览',
    '监控',
    '状态总览',
    '实时状态',
    'overview',
  ])) {
    return 'dashboard';
  }
  if (textIncludesAny(text, [
    '报告',
    '分析',
    '经营',
    '数据可视化',
    '图表',
    '指标',
    'kpi',
    'report',
    'analysis',
    'metrics',
    'one-pager',
    'one pager',
    'html',
    '网页',
    '可视化页',
    '数据页',
  ])) {
    return 'data-report';
  }
  return '';
}

function toDesignReference(reference) {
  const guardrails = asArray(reference.guardrails).length
    ? reference.guardrails
    : SAFE_STATIC_PAGE_GUARDRAILS;
  return {
    ...TEMPLATE_SOURCE,
    templateId: reference.id,
    label: reference.label,
    category: reference.category,
    scenario: reference.scenario,
    surface: reference.surface,
    status: reference.status,
    pauseReason: reference.pauseReason || '',
    quickOutput: Boolean(reference.quickOutput),
    aspectHint: reference.aspectHint,
    styleDirection: reference.styleDirection,
    designIntent: reference.designIntent,
    promptHints: clone(asArray(reference.promptHints)),
    guardrails: clone(guardrails),
  };
}

function findReference(id) {
  const normalized = normalizeReferenceId(id);
  if (!normalized) return null;
  return HTML_TEMPLATE_REFERENCES.find((reference) => reference.id === normalized) || null;
}

export function listHtmlTemplateReferences({ surface = '', status = '', includePaused = false } = {}) {
  const surfaceFilter = stringOrEmpty(surface);
  const statusFilter = stringOrEmpty(status);
  return HTML_TEMPLATE_REFERENCES
    .filter((reference) => !surfaceFilter || reference.surface === surfaceFilter)
    .filter((reference) => !statusFilter || reference.status === statusFilter)
    .filter((reference) => includePaused || reference.status === 'enabled')
    .map((reference) => clone({
      ...reference,
      designReference: toDesignReference(reference),
    }));
}

export function getHtmlTemplateReference(id, { includePaused = false } = {}) {
  const reference = findReference(id);
  if (!reference) return null;
  if (!includePaused && reference.status !== 'enabled') return null;
  return clone({
    ...reference,
    designReference: toDesignReference(reference),
  });
}

export function getStaticPageTemplateReference(id) {
  const reference = getHtmlTemplateReference(id);
  if (!reference || reference.surface !== 'static_page') return null;
  return reference;
}

export function normalizeTemplateDesignReferences(value = []) {
  return asArray(value).slice(0, 5).map((item) => {
    const reference = item && typeof item === 'object' ? item : {};
    const templateId = normalizeReferenceId(reference.templateId || reference.template_id || reference.id);
    if (!templateId) return null;
    const guardrails = asArray(reference.guardrails).map(stringOrEmpty).filter(Boolean);
    return {
      source: stringOrEmpty(reference.source) || TEMPLATE_SOURCE.source,
      sourceKind: stringOrEmpty(reference.sourceKind || reference.source_kind) || TEMPLATE_SOURCE.sourceKind,
      upstream: stringOrEmpty(reference.upstream) || TEMPLATE_SOURCE.upstream,
      license: stringOrEmpty(reference.license) || TEMPLATE_SOURCE.license,
      importPolicy: stringOrEmpty(reference.importPolicy || reference.import_policy) || TEMPLATE_SOURCE.importPolicy,
      templateId,
      label: stringOrEmpty(reference.label) || templateId,
      category: stringOrEmpty(reference.category),
      scenario: stringOrEmpty(reference.scenario),
      surface: stringOrEmpty(reference.surface),
      status: stringOrEmpty(reference.status) || 'enabled',
      pauseReason: stringOrEmpty(reference.pauseReason || reference.pause_reason),
      quickOutput: Boolean(reference.quickOutput || reference.quick_output),
      aspectHint: stringOrEmpty(reference.aspectHint || reference.aspect_hint),
      styleDirection: stringOrEmpty(reference.styleDirection || reference.style_direction),
      designIntent: stringOrEmpty(reference.designIntent || reference.design_intent),
      promptHints: asArray(reference.promptHints || reference.prompt_hints).map(stringOrEmpty).filter(Boolean),
      guardrails: guardrails.length ? guardrails : clone(SAFE_STATIC_PAGE_GUARDRAILS),
    };
  }).filter(Boolean);
}

export function staticPageTemplateReferenceDraftSeed(id) {
  const reference = getStaticPageTemplateReference(id);
  if (!reference) return null;
  return {
    templateId: reference.id,
    designReference: reference.designReference,
    objective: reference.objective,
    audience: reference.audience,
    styleDirection: reference.styleDirection,
    modelSummary: `已收到模板参考：将以「${reference.label}」作为页面结构、版式风格和字段组织参考；事实内容仍以可见数据集、检索证据和缺失项为准。`,
    modules: clone(reference.moduleBlueprints),
  };
}

export function templateReferenceProviderPromptPolicy(id) {
  const reference = getHtmlTemplateReference(id, { includePaused: true });
  if (!reference) return null;
  return {
    templateId: reference.id,
    status: reference.status,
    surface: reference.surface,
    designReference: reference.designReference,
    providerOutput: reference.status === 'enabled' && reference.surface === 'static_page'
      ? 'structured_static_page_draft_json'
      : 'not_allowed_for_current_v3_track',
    forbiddenOutput: [
      'raw_html',
      'remote_script',
      'remote_css',
      'provider_secret',
      'queue_credential',
      'private_path',
    ],
  };
}
