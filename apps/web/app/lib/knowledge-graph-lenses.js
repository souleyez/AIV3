const SAFE_LENS_NODE_KINDS = new Set([
  'object',
  'field',
  'concept',
  'structure',
  'section',
  'unresolved',
]);

const EVIDENCE_CLASSES = new Set(['confirmed', 'observed', 'inferred']);

const SENSITIVE_DISPLAY_VALUE = /(?:\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b|\b1[3-9]\d{9}\b|\b\d{17}[\dXx]\b|(?:password|token|secret|credential|person[_-]?id|raw[_-]?pid)\s*[:=]?|(?:https?|postgres(?:ql)?|mysql|jdbc):\/\/|^[A-Za-z]:[\\/]|^\/(?:home|users|root|etc|var)\/)/iu;

const RAW_ROW_VALUE = /\t|(?:^|\s)(?:[^\s|,;，；]+[|,;，；]\s*){3,}/u;

const CANDIDATE_SCHEMA_LABEL = /^(?:(?:候选人|人才|人员|求职者)(?:主体|档案|画像|信息|标识|编号|姓名)?|姓名|人员姓名)$/u;

const GENERIC_LENSES = [
  {
    key: 'entity',
    name: '业务主体',
    color: '#38bdf8',
    description: '聚焦业务对象、参与主体与核心实体。',
    kinds: ['object'],
    pattern: /业务对象|主体|实体|客户|会员|门店|店铺|合同|商品|设备|人员/u,
  },
  {
    key: 'organization',
    name: '组织关系',
    color: '#a78bfa',
    description: '识别公司、部门、团队、机构及其组织关系。',
    pattern: /公司|组织|部门|团队|机构|企业|单位|雇主/u,
  },
  {
    key: 'time',
    name: '时间维度',
    color: '#22d3ee',
    description: '识别日期、时间、周期、年份与持续时长。',
    roles: ['date'],
    pattern: /日期|时间|年份|年度|月份|小时|时段|周期|开始|结束|时长|年限/u,
  },
  {
    key: 'location',
    name: '空间位置',
    color: '#34d399',
    description: '识别地点、区域、层级与地理位置。',
    roles: ['location'],
    pattern: /地点|地址|城市|地区|区域|楼层|位置|所在地|工作地/u,
  },
  {
    key: 'metric',
    name: '度量指标',
    color: '#fb7185',
    description: '聚合数量、金额、比例、均值与业务指标。',
    roles: ['amount', 'quantity'],
    pattern: /数量|金额|比例|比率|均值|平均|总额|计数|人次|人数|指标|时长/u,
  },
  {
    key: 'classification',
    name: '分类状态',
    color: '#f97316',
    description: '呈现分类、类型、状态和阶段等业务口径。',
    roles: ['category', 'status'],
    pattern: /分类|类别|类型|状态|阶段|等级|标签/u,
  },
  {
    key: 'quality',
    name: '质量边界',
    color: '#fbbf24',
    description: '集中呈现缺失、重复、异常、隐私和待解释项。',
    kinds: ['unresolved'],
    pattern: /数据质量|质量边界|缺失|重复|异常|校验|错配|隐私|待解释|未解析|风险/u,
  },
];

const RESUME_LENSES = [
  {
    key: 'resume-structure',
    name: '履历结构',
    color: '#818cf8',
    description: '在语义快照尚未完成时，仅按已识别的章节结构浏览履历，不把标题当作候选人事实。',
    kinds: ['structure', 'section'],
  },
  {
    key: 'candidate',
    name: '候选人主体',
    color: '#38bdf8',
    description: '只识别候选人字段语义，不展示或推断具体个人身份。',
    pattern: CANDIDATE_SCHEMA_LABEL,
  },
  {
    key: 'experience',
    name: '任职经历',
    color: '#60a5fa',
    description: '组织公司、岗位、职责与工作年限等任职结构。',
    pattern: /工作经历|任职经历|从业经历|职业经历|工作年限|任职年限|公司|雇主|岗位|职位|职务|职责/u,
  },
  {
    key: 'project',
    name: '项目交付',
    color: '#fb7185',
    description: '组织项目经历、项目职责、成果与交付线索。',
    pattern: /项目经历|项目经验|项目名称|项目职责|项目成果|项目交付|项目数量/u,
  },
  {
    key: 'capability',
    name: '技能能力',
    color: '#a78bfa',
    description: '组织技能、技术栈、能力、证书与资质；跨集共享技能只表示共同概念。',
    pattern: /技能|能力|技术栈|专长|证书|资质|语言能力/u,
    sharedConceptOnly: true,
  },
  {
    key: 'education',
    name: '教育资质',
    color: '#34d399',
    description: '组织教育经历、学历、学位、院校与专业。',
    pattern: /教育经历|教育背景|学历|学位|院校|学校|毕业院校|专业/u,
  },
  {
    key: 'time',
    name: '履历时间',
    color: '#22d3ee',
    description: '识别任职、项目和教育经历中的日期、年份与持续时间。',
    roles: ['date'],
    pattern: /日期|时间|年份|年度|开始|结束|最近年份|年限|任职时间|毕业时间/u,
  },
  {
    key: 'location',
    name: '地点范围',
    color: '#f97316',
    description: '识别工作、项目与教育相关的城市和地点。',
    roles: ['location'],
    pattern: /地点|城市|地区|地址|所在地|工作地|项目地/u,
  },
  {
    key: 'quality',
    name: '质量边界',
    color: '#fbbf24',
    description: '呈现缺失、冲突、待解释与隐私边界，不把推断升级为个人事实。',
    kinds: ['unresolved'],
    pattern: /缺失|重复|异常|冲突|待解释|未解析|隐私|风险|质量/u,
  },
];

export const KNOWLEDGE_GRAPH_LENS_PROFILES = Object.freeze({
  generic: Object.freeze(GENERIC_LENSES.map((lens) => Object.freeze({ ...lens }))),
  resume: Object.freeze(RESUME_LENSES.map((lens) => Object.freeze({ ...lens }))),
});

function text(value) {
  return typeof value === 'string' ? value.trim() : '';
}

export function knowledgeGraphLensPreferredViewMode(model, lens) {
  const nodeIds = new Set((Array.isArray(lens?.nodeIds) ? lens.nodeIds : [])
    .map(text)
    .filter(Boolean));
  if (!nodeIds.size) return 'business';
  const requiresTechnicalView = (Array.isArray(model?.nodes) ? model.nodes : []).some((node) => (
    nodeIds.has(text(node?.id))
      && (text(node?.kind).toLowerCase() === 'unresolved' || node?.technicalOnly === true)
  ));
  return requiresTechnicalView ? 'technical' : 'business';
}

function stableTextCompare(left, right) {
  return text(left).localeCompare(text(right), 'en');
}

function nodeKind(node) {
  return text(node?.entityType || node?.kind).toLowerCase();
}

function safeNodeDisplayLabel(node) {
  const label = text(node?.name) || text(node?.display_label) || text(node?.displayLabel);
  if (!label || SENSITIVE_DISPLAY_VALUE.test(label) || RAW_ROW_VALUE.test(label)) return '';
  return label;
}

function safeLensNode(node) {
  const id = text(node?.id);
  const label = safeNodeDisplayLabel(node);
  const kind = text(node?.kind).toLowerCase();
  const entityType = nodeKind(node);
  if (!id || !label || !SAFE_LENS_NODE_KINDS.has(kind) && !SAFE_LENS_NODE_KINDS.has(entityType)) {
    return null;
  }
  return {
    id,
    name: label,
    kind: kind || entityType,
    entityType: entityType || kind,
    semanticRole: text(node?.semanticRole || node?.semantic_role).toLowerCase(),
    datasetRefs: [...new Set((Array.isArray(node?.datasetRefs) ? node.datasetRefs : [])
      .map(text)
      .filter(Boolean))].sort(stableTextCompare),
    shared: node?.shared === true,
    businessScore: Number.isFinite(Number(node?.businessScore)) ? Number(node.businessScore) : 0,
  };
}

function nodeRank(node) {
  if (node.kind === 'concept') return 0;
  if (node.entityType === 'object' || node.kind === 'object') return 1;
  if (node.entityType === 'field' || node.kind === 'field') return 2;
  if (node.kind === 'structure') return 3;
  return 4;
}

function stableNodeCompare(left, right) {
  return Number(right.shared) - Number(left.shared)
    || nodeRank(left) - nodeRank(right)
    || right.businessScore - left.businessScore
    || stableTextCompare(left.name, right.name)
    || stableTextCompare(left.id, right.id);
}

function lensMatchesNode(lens, node) {
  if (lens.sharedConceptOnly && node.shared && node.kind !== 'concept') return false;
  const kindMatch = Array.isArray(lens.kinds)
    && lens.kinds.includes(node.kind || node.entityType);
  const roleMatch = Array.isArray(lens.roles)
    && lens.roles.includes(node.semanticRole);
  const labelMatch = lens.pattern instanceof RegExp && lens.pattern.test(node.name);
  return kindMatch || roleMatch || labelMatch;
}

function edgeEndpoint(edge, key) {
  return text(edge?.[key] || edge?.[`${key}_id`]);
}

function evidenceCountsForNodeIds(links, nodeIds) {
  const counts = { confirmed: 0, observed: 0, inferred: 0 };
  const seen = new Set();
  links.forEach((link) => {
    const source = edgeEndpoint(link, 'source');
    const target = edgeEndpoint(link, 'target');
    if (!nodeIds.has(source) && !nodeIds.has(target)) return;
    const evidenceClass = text(link?.evidenceClass || link?.evidence_class || link?.type).toLowerCase();
    if (!EVIDENCE_CLASSES.has(evidenceClass)) return;
    const key = text(link?.id) || `${source}|${target}|${evidenceClass}`;
    if (seen.has(key)) return;
    seen.add(key);
    counts[evidenceClass] += 1;
  });
  return counts;
}

function resumeSignalScore(model, nodes) {
  const title = text(model?.title);
  if (/简历|履历|候选人|人才库|招聘/u.test(title)) return Number.POSITIVE_INFINITY;
  const labels = nodes.map((node) => node.name).join(' ');
  return [
    /候选人|人才档案|人员画像/u,
    /工作经历|任职经历|工作年限/u,
    /项目经历|项目经验|项目交付/u,
    /技能|技术栈|能力|证书/u,
    /学历|学位|院校|教育经历/u,
  ].filter((pattern) => pattern.test(labels)).length;
}

export function resolveKnowledgeGraphLensProfile(model, requestedProfile = 'auto') {
  const requested = text(requestedProfile).toLowerCase() || 'auto';
  if (requested !== 'auto' && !KNOWLEDGE_GRAPH_LENS_PROFILES[requested]) {
    throw new TypeError(`unknown knowledge graph lens profile: ${requested}`);
  }
  if (requested !== 'auto') return requested;
  const nodes = (Array.isArray(model?.nodes) ? model.nodes : [])
    .map(safeLensNode)
    .filter(Boolean);
  return resumeSignalScore(model, nodes) >= 2 ? 'resume' : 'generic';
}

export function buildKnowledgeGraphLenses(model, options = {}) {
  const profile = resolveKnowledgeGraphLensProfile(model, options.profile);
  const links = Array.isArray(model?.links) ? model.links : [];
  const seenNodeIds = new Set();
  const nodes = (Array.isArray(model?.nodes) ? model.nodes : [])
    .map(safeLensNode)
    .filter((node) => {
      if (!node || seenNodeIds.has(node.id)) return false;
      seenNodeIds.add(node.id);
      return true;
    })
    .sort(stableNodeCompare);
  const matchedNodeIds = new Set();
  const facets = KNOWLEDGE_GRAPH_LENS_PROFILES[profile]
    .map((lens) => {
      const matchedNodes = nodes.filter((node) => lensMatchesNode(lens, node));
      matchedNodes.forEach((node) => matchedNodeIds.add(node.id));
      const nodeIds = matchedNodes.map((node) => node.id);
      const nodeIdSet = new Set(nodeIds);
      const datasetIds = [...new Set(matchedNodes.flatMap((node) => node.datasetRefs))]
        .sort(stableTextCompare);
      return {
        key: lens.key,
        name: lens.name,
        color: lens.color,
        description: lens.description,
        nodes: matchedNodes,
        nodeIds,
        nodeCount: matchedNodes.length,
        datasetIds,
        shared: matchedNodes.some((node) => node.shared),
        focusNodeId: nodeIds[0] || '',
        evidenceCounts: evidenceCountsForNodeIds(links, nodeIdSet),
      };
    })
    .filter((facet) => facet.nodeCount > 0);

  return {
    profile,
    facets,
    unmatchedNodeCount: nodes.filter((node) => !matchedNodeIds.has(node.id)).length,
    privacy: {
      usesRawExamples: false,
      identityMerging: false,
      sharedSkillSemantics: 'concept_only',
    },
  };
}
