const HTML_ARTIFACT_KIND = 'html_artifact';
const HTML_ARTIFACT_VERSION = 1;

export const HTML_ARTIFACT_TEMPLATE_IDS = Object.freeze([
  'codex_execution_report',
  'static_page_planning_handoff',
  'static_page_data_quality_report',
  'report_render_summary',
  'code_review_summary',
  'video_extraction_summary',
  'wechat_video_login_handoff',
]);

export const HTML_ARTIFACT_SOURCE_TYPES = Object.freeze([
  'codex_host',
  'static_page',
  'report',
  'code_review',
  'video_extraction',
  'manual',
]);

export const HTML_ARTIFACT_INTERACTION_MODES = Object.freeze([
  'read_only',
  'json_patch',
  'action_intent',
]);

const TEMPLATE_LABELS = {
  codex_execution_report: 'Codex 执行报告',
  static_page_planning_handoff: '静态页规划交接',
  static_page_data_quality_report: '静态页数据质量报告',
  report_render_summary: '报告渲染摘要',
  code_review_summary: '代码审查摘要',
  video_extraction_summary: '视频提取摘要',
  wechat_video_login_handoff: '视频来源受限',
};

const SOURCE_LABELS = {
  codex_host: 'Codex Host',
  static_page: '静态页',
  report: '报告',
  code_review: '代码审查',
  video_extraction: '视频提取',
  manual: '手动产物',
};

const UNSAFE_STRING_PATTERNS = [
  /<\s*script\b/i,
  /<\s*iframe\b/i,
  /<\s*object\b/i,
  /<\s*embed\b/i,
  /<\s*link\b/i,
  /<\s*meta\b/i,
  /<\s*form\b/i,
  /\son[a-z]+\s*=/i,
  /\bjavascript\s*:/i,
  /\bdata\s*:\s*text\/html/i,
  /\bsrcdoc\s*=/i,
  /\bsrc\s*=/i,
  /\bhref\s*=/i,
  /https?:\/\//i,
  /\/\/[a-z0-9.-]+\.[a-z]{2,}/i,
  /@import\b/i,
  /\burl\s*\(/i,
  /\b(api[_-]?key|access[_-]?token|authorization|bearer\s+|cookie|secret)\b/i,
];

function isPlainObject(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function stringOrFallback(value, fallback = '') {
  return typeof value === 'string' ? value.trim() : fallback;
}

function arrayOrEmpty(value) {
  return Array.isArray(value) ? value : [];
}

function clampText(value, max = 240) {
  const text = stringOrFallback(value);
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

function unsafeStringReason(value) {
  if (typeof value !== 'string') return '';
  const hit = UNSAFE_STRING_PATTERNS.find((pattern) => pattern.test(value));
  return hit ? `unsafe string matched ${hit.source}` : '';
}

function findUnsafePayloadPath(value, path = '$') {
  if (typeof value === 'function' || typeof value === 'symbol' || typeof value === 'undefined') {
    return `${path}: unsupported value type`;
  }
  if (typeof value === 'string') {
    const reason = unsafeStringReason(value);
    return reason ? `${path}: ${reason}` : '';
  }
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index += 1) {
      const reason = findUnsafePayloadPath(value[index], `${path}[${index}]`);
      if (reason) return reason;
    }
    return '';
  }
  if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) {
      const reason = unsafeStringReason(key) || findUnsafePayloadPath(child, `${path}.${key}`);
      if (reason) return reason.startsWith('$') ? reason : `${path}.${key}: ${reason}`;
    }
  }
  return '';
}

function safeJsonClone(value) {
  if (value === null || typeof value === 'number' || typeof value === 'boolean' || typeof value === 'string') {
    return value;
  }
  if (Array.isArray(value)) {
    return value.map(safeJsonClone);
  }
  if (isPlainObject(value)) {
    return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, safeJsonClone(child)]));
  }
  return null;
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function normalizeOwnerScope(value) {
  const scope = isPlainObject(value) ? value : {};
  return {
    type: clampText(scope.type || 'local', 48) || 'local',
    id: clampText(scope.id || 'local', 120) || 'local',
  };
}

function normalizeDataRefs(value) {
  return arrayOrEmpty(value).slice(0, 20).map((item, index) => {
    const ref = isPlainObject(item) ? item : {};
    return {
      kind: clampText(ref.kind || 'unknown', 48) || 'unknown',
      id: clampText(ref.id || `ref-${index + 1}`, 160) || `ref-${index + 1}`,
      label: clampText(ref.label || ref.id || `Ref ${index + 1}`, 160) || `Ref ${index + 1}`,
    };
  });
}

function normalizeProvenance(value) {
  const provenance = isPlainObject(value) ? value : {};
  return {
    producer: clampText(provenance.producer || 'v3', 80) || 'v3',
    reason: clampText(provenance.reason || '', 280),
    sourceRunId: clampText(provenance.sourceRunId || provenance.source_run_id || '', 160),
  };
}

function invalidManifest(reason, input) {
  return {
    manifest: null,
    rejected: true,
    reason,
    sourceId: stringOrFallback(input?.id),
  };
}

export function normalizeHtmlArtifactManifest(input = {}) {
  if (!isPlainObject(input)) {
    return invalidManifest('manifest must be an object', input);
  }

  const kind = stringOrFallback(input.kind, HTML_ARTIFACT_KIND);
  if (kind !== HTML_ARTIFACT_KIND) {
    return invalidManifest(`unsupported kind ${kind || '<empty>'}`, input);
  }

  const templateId = stringOrFallback(input.templateId || input.template_id);
  if (!HTML_ARTIFACT_TEMPLATE_IDS.includes(templateId)) {
    return invalidManifest(`unsupported template ${templateId || '<empty>'}`, input);
  }

  const sourceType = stringOrFallback(input.sourceType || input.source_type, 'manual');
  if (!HTML_ARTIFACT_SOURCE_TYPES.includes(sourceType)) {
    return invalidManifest(`unsupported source type ${sourceType || '<empty>'}`, input);
  }

  const interactionMode = stringOrFallback(input.interactionMode || input.interaction_mode, 'read_only');
  if (!HTML_ARTIFACT_INTERACTION_MODES.includes(interactionMode)) {
    return invalidManifest(`unsupported interaction mode ${interactionMode || '<empty>'}`, input);
  }

  const payload = isPlainObject(input.payload) ? input.payload : {};
  const unsafeReason = findUnsafePayloadPath(payload);
  if (unsafeReason) {
    return invalidManifest(unsafeReason, input);
  }

  const manifest = {
    kind: HTML_ARTIFACT_KIND,
    version: Number.isInteger(input.version) ? input.version : HTML_ARTIFACT_VERSION,
    id: clampText(input.id || `html-artifact-${templateId}`, 160) || `html-artifact-${templateId}`,
    title: clampText(input.title || TEMPLATE_LABELS[templateId] || 'HTML 产物', 160),
    sourceType,
    sourceLabel: SOURCE_LABELS[sourceType] || sourceType,
    templateId,
    templateLabel: TEMPLATE_LABELS[templateId] || templateId,
    ownerScope: normalizeOwnerScope(input.ownerScope || input.owner_scope),
    dataRefs: normalizeDataRefs(input.dataRefs || input.data_refs),
    provenance: normalizeProvenance(input.provenance),
    interactionMode,
    createdAt: clampText(input.createdAt || input.created_at || new Date(0).toISOString(), 80),
    payload: safeJsonClone(payload),
  };

  return {
    manifest,
    rejected: false,
    reason: '',
    sourceId: manifest.id,
  };
}

function renderKeyValueGrid(items = []) {
  return `
    <div class="kv-grid">
      ${items.map((item) => `
        <div>
          <span>${escapeHtml(item.label)}</span>
          <strong>${escapeHtml(item.value || '未提供')}</strong>
        </div>
      `).join('')}
    </div>
  `;
}

function renderList(items = [], emptyText = '暂无记录') {
  if (!items.length) {
    return `<p class="empty">${escapeHtml(emptyText)}</p>`;
  }
  return `
    <div class="list">
      ${items.map((item) => `
        <article>
          <strong>${escapeHtml(item.title || item.label || item.id || '未命名')}</strong>
          <p>${escapeHtml(item.detail || item.summary || item.description || '')}</p>
          ${item.meta ? `<small>${escapeHtml(item.meta)}</small>` : ''}
        </article>
      `).join('')}
    </div>
  `;
}

function renderVisualBridge(value = {}) {
  if (!isPlainObject(value)) {
    return '';
  }
  return `
    <section>
      <h2>视觉链路</h2>
      ${renderKeyValueGrid([
        { label: '生图通道', value: value.providerLane || value.provider_lane || 'gpt-image-2-cloudflare-queue' },
        { label: '效果图状态', value: value.status || 'not_requested' },
        { label: '排队位置', value: value.queuePosition || value.queue_position ? String(value.queuePosition || value.queue_position) : '未排队' },
        { label: '图片任务', value: value.imageJobId || value.image_job_id || '未创建' },
        { label: '预览资产', value: value.previewAssetKey || value.preview_asset_key || '未返回' },
        { label: '过期原因', value: value.staleReason || value.stale_reason || '未过期' },
        { label: '确认指纹', value: value.draftFingerprint || value.draft_fingerprint || '未生成' },
        { label: '最终渲染', value: value.finalRenderStatus || value.final_render_status || 'not_requested' },
        { label: '渲染模型', value: value.renderModel || value.render_model || 'renderer manifest' },
      ])}
      <p>${escapeHtml(value.rule || '效果图只作为视觉参考；最终 HTML 仍由结构化草稿、数据快照和 renderer 生成。')}</p>
    </section>
  `;
}

function renderCodexExecutionReport(manifest) {
  const payload = manifest.payload || {};
  return `
    ${renderKeyValueGrid([
      { label: '模式', value: payload.mode || 'dry_run' },
      { label: '状态', value: payload.status || 'planned' },
      { label: '能力', value: payload.capability || 'inspect_project' },
      { label: '工作区', value: payload.workspaceLabel || payload.workspace_label || '未配置' },
    ])}
    <section>
      <h2>执行摘要</h2>
      <p>${escapeHtml(payload.summary || 'Codex Host 已生成安全摘要，等待后续任务接入更详细步骤。')}</p>
    </section>
    <section>
      <h2>步骤</h2>
      ${renderList(arrayOrEmpty(payload.steps), '暂无步骤。')}
    </section>
    <section>
      <h2>风险与后续</h2>
      ${renderList(arrayOrEmpty(payload.risks), '暂无风险。')}
    </section>
  `;
}

function renderStaticPagePlanningHandoff(manifest) {
  const payload = manifest.payload || {};
  const modules = arrayOrEmpty(payload.modules).map((module) => ({
    title: module.title || module.id || '未命名模块',
    detail: [
      module.content,
      module.dataBinding || module.data_binding ? `数据：${module.dataBinding || module.data_binding}` : '',
      module.visualizationType || module.visualization_type ? `图表：${module.visualizationType || module.visualization_type}` : '',
      module.layout ? `布局：${JSON.stringify(module.layout)}` : '',
    ].filter(Boolean).join(' ｜ '),
    meta: module.dataQuality || module.data_quality || '',
  }));
  return `
    <section>
      <h2>页面目标</h2>
      <p>${escapeHtml(payload.objective || manifest.title)}</p>
    </section>
    ${renderVisualBridge(payload.visualBridge || payload.visual_bridge)}
    <section>
      <h2>模块规划</h2>
      ${renderList(modules, '暂无模块规划。')}
    </section>
  `;
}

function qualityStatusLabel(status) {
  if (status === 'confirmed') return '已确认';
  if (status === 'partial') return '部分确认';
  if (status === 'missing') return '缺失';
  return status || '待确认';
}

function renderStaticPageDataQualityReport(manifest) {
  const payload = manifest.payload || {};
  const summary = isPlainObject(payload.summary) ? payload.summary : {};
  const modules = arrayOrEmpty(payload.modules).map((module) => ({
    title: module.title || module.moduleId || module.module_id || '未命名模块',
    detail: [
      `状态：${qualityStatusLabel(module.dataQualityStatus || module.data_quality_status)}`,
      `运行时：${module.chartRuntime || module.chart_runtime || 'deterministic'}`,
      `样本：${module.sampleDataRows ?? module.sample_data_rows ?? 0} 行`,
      module.fallback ? '最终页使用静态回退' : '',
    ].filter(Boolean).join(' ｜ '),
    meta: module.recommendedAction || module.recommended_action || module.dataQualityReason || module.data_quality_reason || '',
  }));
  return `
    <section>
      <h2>数据质量汇总</h2>
      ${renderKeyValueGrid([
        { label: '已确认', value: String(summary.confirmedModules ?? summary.confirmed_modules ?? 0) },
        { label: '部分确认', value: String(summary.partialModules ?? summary.partial_modules ?? 0) },
        { label: '缺失', value: String(summary.missingModules ?? summary.missing_modules ?? 0) },
        { label: '需关注', value: String(summary.attentionModules ?? summary.attention_modules ?? 0) },
      ])}
      <p>${escapeHtml(payload.note || '数据质量报告来自最终渲染 manifest，用于交付前检查模块数据和图表回退状态。')}</p>
    </section>
    <section>
      <h2>模块检查</h2>
      ${renderList(modules, '暂无模块数据质量记录。')}
    </section>
  `;
}

function reportRenderStatusLabel(status) {
  if (status === 'rendered') return '已渲染';
  if (status === 'failed') return '失败';
  return status || '未知';
}

function renderReportRenderSummary(manifest) {
  const payload = manifest.payload || {};
  const modelFacing = isPlainObject(payload.modelFacing || payload.model_facing)
    ? payload.modelFacing || payload.model_facing
    : {};
  const handoff = isPlainObject(payload.serviceHandoff || payload.service_handoff)
    ? payload.serviceHandoff || payload.service_handoff
    : {};
  const warnings = arrayOrEmpty(payload.warnings).map((warning, index) => ({
    title: warning.title || warning.label || `提醒 ${index + 1}`,
    detail: warning.detail || warning.summary || warning.message || '',
    meta: warning.meta || '',
  }));
  return `
    ${renderKeyValueGrid([
      { label: '报告', value: payload.reportTitle || payload.report_title || manifest.title },
      { label: 'Surface', value: payload.surface || 'pc' },
      { label: '状态', value: reportRenderStatusLabel(payload.status) },
      { label: '可发布', value: payload.publishable ? '是' : '否' },
      { label: '资产类型', value: payload.assetKind || payload.asset_kind || '未识别' },
      { label: '资产路径', value: payload.assetPath || payload.asset_path || '未生成' },
    ])}
    <section>
      <h2>报告目标</h2>
      <p>${escapeHtml(payload.objective || '未提供报告目标。')}</p>
    </section>
    <section>
      <h2>运行与发布建议</h2>
      ${renderKeyValueGrid([
        { label: 'Plan ID', value: payload.reportPlanId || payload.report_plan_id || '' },
        { label: 'Output ID', value: payload.reportRenderOutputId || payload.report_render_output_id || '' },
        { label: 'Workflow', value: payload.workflowExecutionId || payload.workflow_execution_id || '' },
        { label: 'AST Version', value: payload.astVersionId || payload.ast_version_id || '' },
        { label: '推荐工具', value: modelFacing.recommendedToolKey || modelFacing.recommended_tool_key || '' },
        { label: '报告入口', value: handoff.reportEntryState || handoff.report_entry_state || '' },
      ])}
    </section>
    <section>
      <h2>注意事项</h2>
      ${renderList(warnings, '暂无注意事项。')}
    </section>
  `;
}

function renderCodeReviewSummary(manifest) {
  const payload = manifest.payload || {};
  const findings = arrayOrEmpty(payload.findings).map((finding) => ({
    title: `${finding.severity || 'P?'} · ${finding.title || finding.file || '问题'}`,
    detail: finding.detail || finding.body || finding.summary || '',
    meta: [finding.file, finding.line ? `L${finding.line}` : ''].filter(Boolean).join(':'),
  }));
  return `
    <section>
      <h2>审查摘要</h2>
      <p>${escapeHtml(payload.summary || '暂无摘要。')}</p>
    </section>
    <section>
      <h2>发现</h2>
      ${renderList(findings, '暂无发现。')}
    </section>
  `;
}

function formatSeconds(value) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '';
  const minutes = Math.floor(value / 60);
  const seconds = Math.round(value % 60).toString().padStart(2, '0');
  return `${minutes}:${seconds}`;
}

const VIDEO_ARTIFACT_KIND_LABELS = {
  pptx: 'PPTX',
  final_deliverables_manifest: '交付清单',
  extraction_artifacts_manifest: '产物索引',
  ppt_outline: 'PPT 大纲',
  slide_notes: '讲稿备注',
  subtitle_page_map: '字幕对页',
  transcript_text: '原文',
  source_text: '来源文本',
  timestamp_map: '时间映射',
};

const VIDEO_FOLLOW_UP_ACTION_LABELS = {
  open_video_extraction_summary: '打开视频提取摘要',
  download_pptx: '下载 PPTX',
  review_final_deliverables_manifest: '复核交付清单',
  review_subtitle_page_map: '复核字幕对页',
  complete_keep_list_or_review_missing_inputs: '完成选页或补齐缺失输入',
  retry_frame_extraction: '重试原始帧提取',
  provide_local_media_file_or_parsed_frames: '提供本地视频或已解析帧',
  retry_generated_artifact_writer: '重试生成文件写入',
  attach_or_parse_transcript_evidence: '补充或解析转写证据',
  generate_contact_sheet_from_raw_frames: '基于原始帧生成联系表',
  fill_ppt_keep_list_template: '填写 PPT 保留页清单',
  review_low_confidence_transcript: '复核低置信度转写片段',
  rerun_or_review_subtitle_ocr: '重跑或复核字幕 OCR',
  rerun_or_refresh_video_parse: '重跑或刷新视频解析',
  check_video_provider_configuration: '检查视频解析提供方配置',
};

function videoArtifactKindLabel(kind) {
  return VIDEO_ARTIFACT_KIND_LABELS[kind] || kind || '交付文件';
}

function videoFollowUpActionLabel(action) {
  return VIDEO_FOLLOW_UP_ACTION_LABELS[action] || action || '后续动作';
}

function renderVideoExtractionSummary(manifest) {
  const payload = manifest.payload || {};
  const document = isPlainObject(payload.document) ? payload.document : {};
  const summary = isPlainObject(payload.summary) ? payload.summary : {};
  const missing = arrayOrEmpty(payload.missing).map((value, index) => ({
    title: value || `缺失项 ${index + 1}`,
    detail: '当前解析结果没有提供这一类证据，后续应补充转写、关键帧或 OCR 能力后再生成完整 PPT。',
  }));
  const transcript = arrayOrEmpty(payload.transcriptSegments || payload.transcript_segments).map((segment, index) => {
    const start = formatSeconds(segment.startSeconds ?? segment.start_seconds);
    const end = formatSeconds(segment.endSeconds ?? segment.end_seconds);
    return {
      title: start || end ? `${start || '?'} - ${end || '?'}` : `原文片段 ${index + 1}`,
      detail: segment.text || '',
      meta: segment.source || '',
    };
  });
  const scenes = arrayOrEmpty(payload.scenes).map((scene, index) => {
    const start = formatSeconds(scene.startSeconds ?? scene.start_seconds);
    const end = formatSeconds(scene.endSeconds ?? scene.end_seconds);
    return {
      title: start || end ? `${start || '?'} - ${end || '?'}` : `场景 ${index + 1}`,
      detail: scene.summary || '',
      meta: scene.source || '',
    };
  });
  const ocr = arrayOrEmpty(payload.keyframeOcrSnippets || payload.keyframe_ocr_snippets).map((snippet, index) => {
    const timestamp = formatSeconds(snippet.timestampSeconds ?? snippet.timestamp_seconds);
    return {
      title: timestamp ? `关键帧 ${timestamp}` : `关键帧 ${index + 1}`,
      detail: snippet.text || '',
      meta: snippet.source || '',
    };
  });
  const providers = arrayOrEmpty(payload.providerEvidence || payload.provider_evidence).map((evidence) => ({
    title: `${evidence.provider || 'provider'} · ${evidence.capability || 'capability'}`,
    detail: evidence.detail || evidence.status || '',
    meta: evidence.supported ? 'supported' : 'not_supported',
  }));
  const deliverableStatus = isPlainObject(payload.deliverableStatus || payload.deliverable_status)
    ? payload.deliverableStatus || payload.deliverable_status
    : {};
  const generatedArtifacts = isPlainObject(payload.generatedArtifacts || payload.generated_artifacts)
    ? payload.generatedArtifacts || payload.generated_artifacts
    : {};
  const generatedFiles = arrayOrEmpty(generatedArtifacts.files).map((file, index) => ({
    title: file.title || file.artifactKind || file.artifact_kind || `生成文件 ${index + 1}`,
    detail: [
      file.format || '',
      file.path || file.uri || '',
    ].filter(Boolean).join(' · '),
    meta: file.artifactKind || file.artifact_kind || '',
  }));
  const qualityWarnings = arrayOrEmpty(deliverableStatus.warnings).map((warning, index) => ({
    title: warning.code || `质量提示 ${index + 1}`,
    detail: warning.message || warning.detail || '',
    meta: warning.severity || 'info',
  }));
  const completionFollowUp = isPlainObject(payload.completionFollowUp || payload.completion_follow_up)
    ? payload.completionFollowUp || payload.completion_follow_up
    : {};
  const readyFileKinds = arrayOrEmpty(completionFollowUp.readyFileKinds || completionFollowUp.ready_file_kinds);
  const readyFiles = readyFileKinds.map((kind, index) => ({
    title: videoArtifactKindLabel(kind),
    detail: kind,
    meta: `ready-${index + 1}`,
  }));
  const nextActions = arrayOrEmpty(completionFollowUp.nextActions || completionFollowUp.next_actions).map((action, index) => ({
    title: videoFollowUpActionLabel(action),
    detail: action,
    meta: `next-${index + 1}`,
  }));
  const followUpModel = isPlainObject(completionFollowUp.modelFollowUp || completionFollowUp.model_follow_up)
    ? completionFollowUp.modelFollowUp || completionFollowUp.model_follow_up
    : {};
  const followUpStatus = completionFollowUp.status || deliverableStatus.state || 'unknown';
  return `
    ${renderKeyValueGrid([
      { label: '文档', value: document.title || manifest.title },
      { label: '媒体类型', value: payload.mediaKind || payload.media_kind || 'video' },
      { label: '解析状态', value: payload.parseStatus || payload.parse_status || 'unknown' },
      { label: '证据状态', value: payload.evidenceStatus || payload.evidence_status || 'missing' },
      { label: '交付状态', value: deliverableStatus.state || 'unknown' },
      { label: 'PPTX', value: deliverableStatus.hasPptx || deliverableStatus.has_pptx ? 'ready' : 'not ready' },
      { label: '质量提示', value: String(deliverableStatus.warningCount ?? deliverableStatus.warning_count ?? qualityWarnings.length) },
      { label: '就绪文件', value: String(readyFiles.length || generatedFiles.length) },
      { label: '后续提醒', value: followUpModel.required ? 'model follow-up required' : 'not requested' },
      { label: '原文片段', value: String(summary.transcriptSegmentCount ?? summary.transcript_segment_count ?? transcript.length) },
      { label: '场景片段', value: String(summary.sceneCount ?? summary.scene_count ?? scenes.length) },
      { label: '关键帧 OCR', value: String(summary.keyframeOcrSnippetCount ?? summary.keyframe_ocr_snippet_count ?? ocr.length) },
    ])}
    <section>
      <h2>说明</h2>
      <p>${escapeHtml(payload.note || '该摘要来自后台视频解析证据，用于让模型继续生成原文、页面映射、PPT 大纲或截图型 PPT。缺失项不会被补造。')}</p>
    </section>
    <section>
      <h2>完成状态</h2>
      <p>${escapeHtml(`后台提取状态：${followUpStatus}。${followUpModel.instruction || '下一次模型回复应基于这些结构化状态继续，不补造缺失文件。'}`)}</p>
      ${renderList(readyFiles, '暂无已就绪交付文件。')}
    </section>
    <section>
      <h2>下一步</h2>
      ${renderList(nextActions, '暂无建议动作。')}
    </section>
    <section>
      <h2>质量提示</h2>
      ${renderList(qualityWarnings, '暂无质量提示。')}
    </section>
    <section>
      <h2>原文片段</h2>
      ${renderList(transcript, '暂无原文片段。')}
    </section>
    <section>
      <h2>场景片段</h2>
      ${renderList(scenes, '暂无场景片段。')}
    </section>
    <section>
      <h2>关键帧 OCR</h2>
      ${renderList(ocr, '暂无关键帧 OCR。')}
    </section>
    <section>
      <h2>缺失证据</h2>
      ${renderList(missing, '当前没有缺失项。')}
    </section>
    <section>
      <h2>解析提供方</h2>
      ${renderList(providers, '暂无提供方记录。')}
    </section>
    <section>
      <h2>生成文件</h2>
      ${renderList(generatedFiles, '暂无生成文件。')}
    </section>
  `;
}

function renderWechatVideoLoginHandoff(manifest) {
  const payload = manifest.payload || {};
  const acquisitionSteps = [
    {
      title: '提供可解析视频素材',
      detail: '当前只支持上传视频文件、直接视频 URL，或公开页面里可直接解析到的视频地址。',
      meta: 'required',
    },
    {
      title: '进入后台解析',
      detail: '拿到视频素材后再执行抽音频、抽关键帧、字幕/OCR 和 PPT/原文产物生成。',
      meta: 'waiting_video',
    },
  ];
  const extractionSteps = [
    {
      title: '提取原文',
      detail: '从视频音频、字幕或转写结果中形成带时间戳的原文。',
      meta: 'waiting_video',
    },
    {
      title: '提取 PPT 画面',
      detail: '从关键帧或页面区域中识别截图型幻灯片和页面文字。',
      meta: 'waiting_video',
    },
    {
      title: '生成交付产物',
      detail: '输出原文、页面映射、截图型 PPT/大纲和缺失证据说明。',
      meta: 'waiting_video',
    },
  ];
  return `
    ${renderKeyValueGrid([
      { label: '来源', value: payload.sourcePlatform || payload.source_platform || '登录受限视频来源' },
      { label: '来源标识', value: payload.shortCode || payload.short_code || '未识别' },
      { label: '当前阶段', value: 'unsupported_login_gated_source' },
      { label: '产物目标', value: payload.targetArtifact || payload.target_artifact || '视频 PPT 提取' },
    ])}
    <section>
      <h2>来源受限</h2>
      <p>当前开发切片不执行扫码登录、Cookie、登录态页面获取或录屏绕过。请上传视频文件，或提供可以直接访问的视频 URL；拿到视频素材后再进入同一套 PPT/原文提取流程。</p>
    </section>
    <section>
      <h2>获取视频</h2>
      ${renderList(acquisitionSteps, '暂无获取步骤。')}
    </section>
    <section>
      <h2>解析成 PPT</h2>
      ${renderList(extractionSteps, '暂无解析步骤。')}
    </section>
    <section>
      <h2>缺少视频素材</h2>
      <p>没有直接视频文件或可解析视频地址时，本轮不会继续伪造解析结果。用户补充素材后，系统再排后台任务并把提取产物写入智能助手产物区。</p>
    </section>
  `;
}

function jsonPatchPayloadFromManifest(manifest) {
  const payload = manifest.payload || {};
  const operations = arrayOrEmpty(payload.operations || payload.patch || payload.pendingPatch || payload.pending_patch);
  return operations.length ? { operations } : null;
}

function hasSubmittableArtifactAction(manifest) {
  if (manifest.interactionMode === 'action_intent') return true;
  return manifest.interactionMode === 'json_patch' && Boolean(jsonPatchPayloadFromManifest(manifest));
}

function renderInteractionScript(manifest) {
  if (manifest.interactionMode === 'read_only' || !hasSubmittableArtifactAction(manifest)) return '';
  const eventType = manifest.interactionMode === 'json_patch' ? 'html_artifact.patch' : 'html_artifact.action_intent';
  const patchPayload = manifest.interactionMode === 'json_patch' ? jsonPatchPayloadFromManifest(manifest) : null;
  const defaultAction = stringOrFallback(manifest.payload?.defaultAction || manifest.payload?.action, 'submit');
  return `
    <script>
      (() => {
        const artifactId = ${JSON.stringify(manifest.id)};
        const eventType = ${JSON.stringify(eventType)};
        const patchPayload = ${JSON.stringify(patchPayload)};
        const defaultAction = ${JSON.stringify(defaultAction)};
        document.querySelectorAll('[data-artifact-action]').forEach((button) => {
          button.addEventListener('click', () => {
            const intentInput = document.querySelector('[data-artifact-intent]');
            const prompt = intentInput && typeof intentInput.value === 'string' ? intentInput.value.trim() : '';
            window.parent.postMessage({
              source: 'v3-html-artifact',
              artifactId,
              type: eventType,
              payload: patchPayload || {
                action: button.getAttribute('data-artifact-action') || defaultAction,
                prompt
              }
            }, '*');
          });
        });
      })();
    </script>
  `;
}

function renderActionIntentControls(manifest) {
  if (manifest.interactionMode !== 'action_intent') return '';
  const placeholder = manifest.payload?.intentPlaceholder
    || manifest.payload?.placeholder
    || '写一句你希望 V3 如何修改这个产物。';
  return `
    <section>
      <h2>修改意图</h2>
      <p>用自然语言描述要改哪里、怎么改。V3 会把它翻译成受限操作后再更新产物。</p>
      <textarea data-artifact-intent rows="4" placeholder="${escapeHtml(placeholder)}"></textarea>
    </section>
  `;
}

export function renderHtmlArtifactDocument(input = {}) {
  const normalized = normalizeHtmlArtifactManifest(input);
  if (normalized.rejected) {
    return {
      ...normalized,
      html: '',
      sandbox: '',
    };
  }
  const manifest = normalized.manifest;
  const body = manifest.templateId === 'codex_execution_report'
    ? renderCodexExecutionReport(manifest)
    : manifest.templateId === 'static_page_planning_handoff'
      ? renderStaticPagePlanningHandoff(manifest)
      : manifest.templateId === 'static_page_data_quality_report'
        ? renderStaticPageDataQualityReport(manifest)
        : manifest.templateId === 'report_render_summary'
        ? renderReportRenderSummary(manifest)
        : manifest.templateId === 'code_review_summary'
          ? renderCodeReviewSummary(manifest)
          : manifest.templateId === 'video_extraction_summary'
            ? renderVideoExtractionSummary(manifest)
            : renderWechatVideoLoginHandoff(manifest);
  const allowScripts = manifest.interactionMode !== 'read_only';
  const script = renderInteractionScript(manifest);
  const canSubmit = hasSubmittableArtifactAction(manifest);
  const html = `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; base-uri 'none'; form-action 'none'; img-src data:; style-src 'unsafe-inline'; ${allowScripts ? "script-src 'unsafe-inline';" : "script-src 'none';"} connect-src 'none';">
  <title>${escapeHtml(manifest.title)}</title>
  <style>
    :root { color-scheme: light; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
    body { margin: 0; background: #f8fafc; color: #0f172a; }
    main { min-height: 100vh; padding: 24px; box-sizing: border-box; }
    header { padding: 18px 18px 16px; border-radius: 22px; background: linear-gradient(135deg, #0f172a, #334155); color: #fff; }
    header span { display: inline-flex; margin-bottom: 8px; font-size: 12px; opacity: .72; }
    h1 { margin: 0; font-size: 24px; letter-spacing: -.03em; }
    h2 { margin: 0 0 10px; font-size: 15px; }
    section, .kv-grid > div, .list article { margin-top: 14px; border-radius: 18px; background: rgba(255,255,255,.94); box-shadow: 0 18px 50px rgba(15,23,42,.08); }
    section { padding: 16px; }
    p { margin: 0; line-height: 1.65; color: #475569; }
    small, .empty { color: #64748b; }
    .kv-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: 10px; margin-top: 14px; }
    .kv-grid > div { padding: 14px; display: grid; gap: 5px; }
    .kv-grid span { color: #64748b; font-size: 12px; }
    .kv-grid strong { font-size: 14px; overflow-wrap: anywhere; }
    .list { display: grid; gap: 10px; }
    .list article { padding: 14px; margin-top: 0; display: grid; gap: 6px; }
    .list strong { font-size: 14px; }
    .qr-box { margin-top: 14px; display: inline-grid; padding: 12px; border-radius: 18px; background: #fff; }
    .qr-box img { width: min(220px, 56vw); height: auto; display: block; }
    textarea { width: 100%; box-sizing: border-box; resize: vertical; margin-top: 12px; border: 0; border-radius: 16px; padding: 12px; background: #f1f5f9; color: #0f172a; font: inherit; line-height: 1.5; outline: 2px solid transparent; }
    textarea:focus { outline-color: #94a3b8; background: #fff; }
    .actions { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 16px; }
    button { border: 0; border-radius: 999px; padding: 9px 12px; background: #0f172a; color: #fff; cursor: pointer; }
  </style>
</head>
<body>
  <main>
    <header>
      <span>${escapeHtml(manifest.templateLabel)} · ${escapeHtml(manifest.sourceLabel)} · ${escapeHtml(manifest.interactionMode)}</span>
      <h1>${escapeHtml(manifest.title)}</h1>
    </header>
    ${body}
    ${renderActionIntentControls(manifest)}
    ${allowScripts && canSubmit ? '<div class="actions"><button type="button" data-artifact-action="submit">提交到 V3</button></div>' : ''}
    ${allowScripts && !canSubmit ? '<section><p class="empty">此交互产物暂未包含可提交的结构化动作。</p></section>' : ''}
  </main>
  ${script}
</body>
</html>`;

  return {
    ...normalized,
    html,
    sandbox: allowScripts ? 'allow-scripts' : '',
  };
}

function isValidActionIntentPayload(payload) {
  if (!isPlainObject(payload)) return false;
  const action = stringOrFallback(payload.action);
  return Boolean(action) && action.length <= 120;
}

function isValidJsonPatchPayload(payload) {
  if (!isPlainObject(payload)) return false;
  const operations = arrayOrEmpty(payload.operations || payload.patch);
  if (!operations.length || operations.length > 50) return false;
  return operations.every((operation) => {
    if (!isPlainObject(operation)) return false;
    const op = stringOrFallback(operation.op);
    const path = stringOrFallback(operation.path);
    if (!['add', 'replace', 'remove', 'move', 'copy', 'test'].includes(op)) return false;
    if (!path.startsWith('/') || path.length > 240) return false;
    if (['move', 'copy'].includes(op)) {
      const from = stringOrFallback(operation.from);
      return from.startsWith('/') && from.length <= 240;
    }
    return true;
  });
}

export function isAllowedHtmlArtifactMessage(eventData, manifestInput) {
  const normalized = normalizeHtmlArtifactManifest(manifestInput);
  if (normalized.rejected) return false;
  const manifest = normalized.manifest;
  if (!isPlainObject(eventData)) return false;
  if (eventData.source !== 'v3-html-artifact') return false;
  if (eventData.artifactId !== manifest.id) return false;
  if (manifest.interactionMode === 'read_only') return false;
  const expectedType = manifest.interactionMode === 'json_patch'
    ? 'html_artifact.patch'
    : 'html_artifact.action_intent';
  if (eventData.type !== expectedType) return false;
  const payload = eventData.payload || {};
  if (findUnsafePayloadPath(payload)) return false;
  return manifest.interactionMode === 'json_patch'
    ? isValidJsonPatchPayload(payload)
    : isValidActionIntentPayload(payload);
}
