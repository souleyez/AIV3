function text(value) {
  return String(value ?? '').trim();
}

function canonicalFieldName(value) {
  return text(value)
    .normalize('NFKC')
    .toLocaleLowerCase('en')
    .replace(/[^\p{L}\p{N}]/gu, '');
}

const FIELD_GLOSSARY = new Map(Object.entries({
  sourcemallid: {
    label: '源请求商场标识', type: 'string', role: 'identifier',
    description: '发起客流接口请求时使用的商场标识，用于核对请求范围。',
  },
  mallid: {
    label: '源响应商场标识', type: 'string', role: 'identifier',
    description: '上游响应返回的商场标识，用于确认记录归属。',
  },
  entitytype: {
    label: '空间实体类型代码', type: 'integer', role: 'category',
    description: '点位、区域、店铺、出入口等空间实体的类型代码。',
  },
  entitytypename: {
    label: '空间实体类型名称', type: 'string', role: 'category',
    description: '空间实体类型的可读名称。',
  },
  entityname: {
    label: '点位 / 区域显示名', type: 'string', role: 'name',
    description: '摄像头点位、区域、店铺或出入口的显示名称；即使内容为纯数字也按文本处理。',
  },
  entityid: {
    label: '源系统实体标识', type: 'string', role: 'identifier',
    description: '上游客流系统中的空间实体标识。',
  },
  aibeeentityid: {
    label: 'AIBee 实体标识', type: 'string', role: 'identifier',
    description: 'AIBee 客流系统内用于关联点位目录和客流记录的实体标识。',
  },
  day: {
    label: '统计日期', type: 'date', role: 'time',
    description: '客流汇总所属日期，源数据采用 YYYYMMDD 口径。',
  },
  date: {
    label: '日期', type: 'date', role: 'time',
    description: '画像聚合结果所属自然日。',
  },
  hour: {
    label: '小时桶', type: 'integer', role: 'time',
    description: '小时级汇总的起始小时。',
  },
  minute: {
    label: '分钟偏移', type: 'integer', role: 'time',
    description: '小时桶内的分钟偏移。',
  },
  interval: {
    label: '聚合粒度代码', type: 'string', role: 'category',
    description: '上游接口返回的时间聚合粒度代码。',
  },
  trafficin: {
    label: '进入人次', type: 'integer', role: 'metric',
    description: '在当前实体与时间桶内识别到的进入人次，不做跨实体去重。',
  },
  trafficout: {
    label: '离开人次', type: 'integer', role: 'metric',
    description: '在当前实体与时间桶内识别到的离开人次。',
  },
  visitors: {
    label: '去重到访人数', type: 'integer', role: 'metric',
    description: '当前实体与时间桶口径下的去重到访人数，不可跨实体直接相加。',
  },
  averagestay: {
    label: '平均停留时长', type: 'decimal', role: 'metric',
    description: '平均停留时长，单位为秒；小时数据中的 0 可能表示该粒度不可用，不能直接解释为真实停留 0 秒。',
  },
  floor: {
    label: '楼层编码', type: 'string', role: 'category',
    description: '点位所属楼层的源系统编码。',
  },
  floorname: {
    label: '楼层名称', type: 'string', role: 'category',
    description: '点位所属楼层的显示名称。',
  },
  entitystatus: {
    label: '实体状态', type: 'integer', role: 'category',
    description: '上游系统记录的空间实体状态代码。',
  },
  area: {
    label: '面积值', type: 'decimal', role: 'metric',
    description: '点位目录中的面积值；源资料未明确单位，因此不擅自换算或标注平方米。',
  },
  l1retailformat: {
    label: '一级业态', type: 'string', role: 'category',
    description: '点位或店铺对应的一级零售业态。',
  },
  l2retailformat: {
    label: '二级业态', type: 'string', role: 'category',
    description: '点位或店铺对应的二级零售业态。',
  },
  recordcount: {
    label: '记录数', type: 'integer', role: 'metric',
    description: '当前汇总粒度内的记录数量；字典仅展示聚合计数，不公开原始记录。',
  },
  uniquepidcount: {
    label: '当日去重 PID 数', type: 'integer', role: 'metric',
    description: '单日内去重后的 PID 数；各日数值相加不能解释为全周期去重人数。',
  },
  duplicaterecordcount: {
    label: '日内重复记录数', type: 'integer', role: 'metric',
    description: '同一日期内重复出现的画像记录数。',
  },
  missingpidcount: {
    label: '缺失 PID 记录数', type: 'integer', role: 'metric',
    description: '源记录中 PID 缺失的记录数量，仅展示质量统计。',
  },
  datemismatchcount: {
    label: '日期不一致记录数', type: 'integer', role: 'metric',
    description: '记录日期与请求日期不一致的记录数量。',
  },
  invalidagecount: {
    label: '非法年龄记录数', type: 'integer', role: 'metric',
    description: '年龄值未通过合法性校验的记录数量。',
  },
  agebucket: {
    label: '年龄段', type: 'string', role: 'category',
    description: '去标识画像中的年龄分桶，不包含个人年龄明细。',
  },
  gender: {
    label: '性别分类', type: 'string', role: 'category',
    description: '去标识画像中的性别分类。',
  },
  grouptype: {
    label: '群组类型', type: 'string', role: 'category',
    description: '去标识画像中的到访群组类型。',
  },
  count: {
    label: '分类记录数', type: 'integer', role: 'metric',
    description: '当前分类桶中的聚合记录数量。',
  },
  share: {
    label: '分类占比', type: 'decimal', role: 'metric',
    description: '当前分类记录数占对应日期或全量聚合记录数的比例。',
  },
}));

function inferValueType(fieldName) {
  const key = canonicalFieldName(fieldName);
  if (key === 'date' || key === 'day' || key.endsWith('date')) return 'date';
  if (/(count|hour|minute|trafficin|trafficout|visitors|status)$/.test(key)) return 'integer';
  if (/(share|rate|ratio|amount|area|stay)$/.test(key)) return 'decimal';
  if (key.endsWith('id') || key.includes('identifier')) return 'string';
  return 'string';
}

function inferSemanticRole(fieldName) {
  const key = canonicalFieldName(fieldName);
  if (key.endsWith('id') || key.includes('identifier')) return 'identifier';
  if (/(date|day|hour|minute|time|timestamp)/.test(key)) return 'time';
  if (/(count|share|rate|ratio|amount|area|traffic|visitors|stay)/.test(key)) return 'metric';
  if (/(type|status|bucket|gender|group|category|format|floor|interval)/.test(key)) return 'category';
  if (key.endsWith('name')) return 'name';
  return 'dimension';
}

function semanticEvidenceIsStructural(field = {}) {
  if (text(field.label_source).toLowerCase() === 'document_fact') return false;
  return (Array.isArray(field.evidence_refs) ? field.evidence_refs : []).some((reference) => (
    ['database', 'spreadsheet'].includes(text(reference?.source_kind).toLowerCase())
  ));
}

function semanticFieldsByTechnicalName(understanding = {}) {
  const matches = new Map();
  for (const field of (Array.isArray(understanding?.fields) ? understanding.fields : [])) {
    if (!semanticEvidenceIsStructural(field)) continue;
    const key = canonicalFieldName(field.technical_name);
    if (!key || matches.has(key)) continue;
    matches.set(key, field);
  }
  return matches;
}

function glossaryFor(fieldName) {
  return FIELD_GLOSSARY.get(canonicalFieldName(fieldName)) || null;
}

function buildStructuralField(column, table, semanticMatches) {
  const fieldName = text(column?.name);
  const semantic = semanticMatches.get(canonicalFieldName(fieldName)) || null;
  const glossary = glossaryFor(fieldName);
  const semanticType = text(semantic?.value_type);
  const semanticRole = text(semantic?.semantic_role);
  const nonEmptyCount = Number(semantic?.non_empty_count);
  const distinctCount = Number(semantic?.distinct_count);
  const semanticLabel = text(semantic?.label);
  return {
    id: `${table.documentId}:${Number(column?.ordinal) || 0}:${fieldName}`,
    ordinal: Number(column?.ordinal) || 0,
    tableId: table.documentId,
    tableTitle: table.title,
    technical_name: fieldName,
    label: glossary?.label || semanticLabel || fieldName,
    description: glossary?.description
      || text(semantic?.description)
      || `${fieldName} 来自 ${table.title} 的真实表头；当前尚无更详细的业务注释。`,
    value_type: glossary?.type || (semanticType && semanticType !== 'unknown' ? semanticType : inferValueType(fieldName)),
    semantic_role: glossary?.role || (semanticRole && semanticRole !== 'unknown' ? semanticRole : inferSemanticRole(fieldName)),
    confidence: Number.isFinite(Number(semantic?.confidence)) ? Number(semantic.confidence) : null,
    structure_confirmed: true,
    non_empty_count: Number.isFinite(nonEmptyCount) && nonEmptyCount > 0 ? nonEmptyCount : null,
    distinct_count: Number.isFinite(distinctCount) && distinctCount > 0 ? distinctCount : null,
    examples: [],
  };
}

function fallbackSemanticFields(understanding = {}) {
  return (Array.isArray(understanding?.fields) ? understanding.fields : [])
    .filter((field) => semanticEvidenceIsStructural(field))
    .filter((field) => /^[A-Za-z_][A-Za-z0-9_.-]{0,79}$/.test(text(field.technical_name)))
    .map((field, index) => {
      const fieldName = text(field.technical_name);
      const glossary = glossaryFor(fieldName);
      const semanticType = text(field.value_type);
      const semanticRole = text(field.semantic_role);
      return {
        ...field,
        id: text(field.id) || `semantic:${fieldName}:${index}`,
        ordinal: index + 1,
        tableId: 'semantic-fallback',
        tableTitle: '结构化字段',
        label: glossary?.label || text(field.label) || fieldName,
        description: glossary?.description || text(field.description) || `${fieldName} 来自结构化数据理解结果。`,
        value_type: glossary?.type || (semanticType && semanticType !== 'unknown' ? semanticType : inferValueType(fieldName)),
        semantic_role: glossary?.role || (semanticRole && semanticRole !== 'unknown' ? semanticRole : inferSemanticRole(fieldName)),
        structure_confirmed: false,
        examples: [],
      };
    });
}

export function buildDatasetDictionaryView({ tabularSchema = {}, understanding = {} } = {}) {
  const semanticMatches = semanticFieldsByTechnicalName(understanding);
  const structuralTables = (Array.isArray(tabularSchema?.tables) ? tabularSchema.tables : [])
    .filter((table) => Array.isArray(table?.columns) && table.columns.length)
    .map((table) => ({
      id: table.documentId,
      title: table.title,
      contentType: table.contentType,
      updatedAt: table.updatedAt,
      structuralSource: table.structuralSource || 'file_header',
      fields: table.columns.map((column) => buildStructuralField(column, table, semanticMatches)),
    }));
  const fallbackFields = structuralTables.length ? [] : fallbackSemanticFields(understanding);
  const tables = structuralTables.length
    ? structuralTables
    : (fallbackFields.length ? [{
      id: 'semantic-fallback',
      title: '结构化字段',
      contentType: '',
      updatedAt: '',
      structuralSource: 'semantic_fallback',
      fields: fallbackFields,
    }] : []);
  const allFields = tables.flatMap((table) => table.fields);
  const uniqueFieldCount = new Set(allFields.map((field) => canonicalFieldName(field.technical_name))).size;
  const businessClueCount = (Array.isArray(understanding?.fields) ? understanding.fields : [])
    .filter((field) => text(field.label_source).toLowerCase() === 'document_fact')
    .length;

  return {
    tables,
    allFields,
    tableCount: structuralTables.length,
    fieldCount: allFields.length,
    uniqueFieldCount,
    businessClueCount,
    skippedTabularDocumentCount: Number(tabularSchema?.skippedTabularDocumentCount) || 0,
    source: structuralTables.length ? 'file_header' : (fallbackFields.length ? 'semantic_fallback' : 'empty'),
  };
}
