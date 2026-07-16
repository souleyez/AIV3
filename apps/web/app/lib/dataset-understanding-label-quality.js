const DOCUMENT_EXTENSION = /\.(?:xlsx?|xlsm|csv|zip|pdf|docx?|pptx?|txt|md)$/i;
const MIME_VALUE = /^(?:application|audio|font|image|message|model|multipart|text|video)\/[a-z0-9!#$&^_.+-]+$/i;
const UUID_VALUE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const HASH_VALUE = /^(?:[0-9a-f]{32}|[0-9a-f]{40}|[0-9a-f]{64}|[0-9a-f]{128})$/i;
const PATH_VALUE = /^(?:[a-z]:[\\/]|\\\\|\/(?:[^/]+\/)+)/i;
const STRATEGY_VALUE = /(?:paragraph_aware|noun_terms|profile|parser|parse_strategy|understanding_strategy)(?:_[a-z0-9]+)*/i;
const SQL_VALUE = /\/\*|\*\/|--|^\s*(?:select|with|insert|update|delete|merge)\b|\b(?:from|where|date_add|datediff|case\s+when|as\s+[a-z_]+)\b/iu;

function text(value) {
  return typeof value === 'string' ? value.trim() : '';
}

function withoutDocumentExtension(value) {
  return text(value).replace(DOCUMENT_EXTENSION, '').trim();
}

function chineseProjection(value) {
  const prepared = withoutDocumentExtension(value)
    .replace(/\d+\s*个/gu, ' ')
    .replace(/[A-Za-z][A-Za-z0-9_.-]*/g, ' ')
    .replace(/\d+(?:[./_-]\d+)*/g, ' ');
  const characters = (prepared.match(/\p{Script=Han}+/gu) || []).join('');
  return Array.from(characters).slice(0, 16).join('');
}

function genericNormalizedKey(value) {
  return text(value)
    .normalize('NFKC')
    .toLocaleLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '')
    .trim();
}

function result(value, qualityClass, mainCanvasAllowed, reason, kind) {
  const normalizedKey = kind === 'document' || DOCUMENT_EXTENSION.test(text(value))
    ? canonicalDocumentTitle(value)
    : mainCanvasAllowed
      ? genericNormalizedKey(value)
      : qualityClass;
  return {
    display_label: text(value),
    quality_class: qualityClass,
    main_canvas_allowed: mainCanvasAllowed,
    normalized_key: normalizedKey || qualityClass || 'unknown',
    reason,
  };
}

export function canonicalDocumentTitle(value) {
  return genericNormalizedKey(withoutDocumentExtension(value));
}

export function classifyGraphLabel(value, options = {}) {
  const valueText = text(value);
  const kind = options.kind === 'document' ? 'document' : text(options.kind);
  if (!valueText) return result(valueText, 'unknown', false, '标签为空', kind);

  if (/\t/u.test(valueText) || /(?:^|\s)(?:[^\s|,;，；]+[|,;，；]\s*){3,}/u.test(valueText)) {
    return result(valueText, 'row', false, '疑似多列原始数据行', kind);
  }
  if (SQL_VALUE.test(valueText)) return result(valueText, 'sql', false, '疑似 SQL 或代码注释', kind);
  if (MIME_VALUE.test(valueText)) return result(valueText, 'technical', false, 'MIME 技术值', kind);
  if (STRATEGY_VALUE.test(valueText)) return result(valueText, 'technical', false, '解析策略技术标识', kind);
  if (UUID_VALUE.test(valueText)) return result(valueText, 'technical', false, 'UUID 技术标识', kind);
  if (HASH_VALUE.test(valueText)) return result(valueText, 'technical', false, 'hash 技术标识', kind);
  if (PATH_VALUE.test(valueText) || /[\\/][^\\/]+\.[a-z0-9]{1,8}$/i.test(valueText)) {
    return result(valueText, 'technical', false, '内部路径或文件定位符', kind);
  }
  if (/^[\d\s.,:/_+-]+$/u.test(valueText)) return result(valueText, 'numeric', false, '纯数字或日期编号', kind);
  if (kind === 'strategy') return result(valueText, 'technical', false, '解析策略只进入技术详情', kind);

  const documentLike = kind === 'document' || DOCUMENT_EXTENSION.test(valueText);
  const chinese = chineseProjection(valueText);
  if (documentLike) {
    return result(valueText, 'document', Array.from(chinese).length >= 2, chinese ? '文档标题包含可信中文' : '文档标题缺少可信中文', 'document');
  }

  const digits = (valueText.match(/\d/gu) || []).length;
  const semanticCharacters = (valueText.match(/[\p{L}\p{N}]/gu) || []).length;
  if (digits >= 4 && semanticCharacters && digits / semanticCharacters >= 0.55) {
    return result(valueText, 'numeric', false, '数字占比过高', kind);
  }
  if (Array.from(chinese).length >= 2) return result(valueText, 'business', true, '可信中文业务短语', kind);
  if (/^[A-Za-z][A-Za-z0-9_.-]*$/u.test(valueText)) {
    return result(valueText, 'technical', false, '英文或代码式技术标识', kind);
  }
  return result(valueText, 'unknown', false, '没有足够证据形成中文业务标签', kind);
}

function safeDetailLabel(value, classification, kind) {
  const valueText = text(value);
  if (classification.main_canvas_allowed) {
    const projectedLabel = chineseProjection(valueText);
    return projectedLabel === valueText ? classification.reason : `原始标签：${valueText}`;
  }
  if (kind === 'document') return `原始标题：${valueText}`;
  if (
    classification.quality_class === 'technical'
    && !UUID_VALUE.test(valueText)
    && !HASH_VALUE.test(valueText)
    && !PATH_VALUE.test(valueText)
    && valueText.length <= 160
  ) {
    return `原始技术标识：${valueText}`;
  }
  return `${classification.reason}；原始内容已从界面隐藏`;
}

export function projectFallbackLabel(value, options = {}) {
  const kind = options.kind === 'document' ? 'document' : text(options.kind) || 'knowledge';
  const classification = classifyGraphLabel(value, { kind });
  const chinese = chineseProjection(value);
  const displayLabel = classification.main_canvas_allowed && chinese
    ? chinese
    : kind === 'document'
      ? '待解释资料'
      : '待解释线索';
  return {
    ...classification,
    display_label: displayLabel,
    normalized_key: kind === 'document'
      ? canonicalDocumentTitle(value) || classification.normalized_key
      : classification.normalized_key,
    detail_label: safeDetailLabel(value, classification, kind),
  };
}
