'use client';

const TEMPLATE_STATUS_LABELS = {
  enabled: '已启用',
  selected: '已选用',
  id_only: '仅记录 ID',
  none: '未选择',
};

const MISSING_STATUS_LABELS = {
  ready: '证据就绪',
  needs_evidence: '需要补证',
};

const STRUCTURE_STATUS_LABELS = {
  available: '标题线索可用',
  none: '未发现标题',
};

const ACTION_LABELS = {
  retrieve_evidence: '检索供料',
  read_document_detail: '读取文档详情',
  static_page_update_draft: '更新草稿',
  'static_page.update_draft': '更新草稿',
  update_static_page_module: '更新模块',
};

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function firstObject(...values) {
  for (const value of values) {
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      return value;
    }
    if (Array.isArray(value)) {
      const found = value.find((item) => item && typeof item === 'object' && !Array.isArray(item));
      if (found) return found;
    }
  }
  return null;
}

function cleanText(value) {
  return String(value || '').replace(/\s+/g, ' ').trim();
}

function collectCleanTextList(value, out = []) {
  if (typeof value === 'string') {
    const text = cleanText(value);
    if (text && !out.includes(text)) out.push(text);
    return out;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      collectCleanTextList(item, out);
    }
  }
  return out;
}

function labelForAction(value) {
  const normalized = cleanText(value);
  return ACTION_LABELS[normalized] || normalized.replace(/[_-]+/g, ' ') || '待处理';
}

export function staticPageTemplateReferenceFromDraft(draft) {
  if (!draft) return null;
  return firstObject(
    draft.templateReference,
    draft.template_reference,
    draft.designReferences,
    draft.design_references,
    draft.source?.templateReference,
    draft.source?.template_reference,
    draft.source?.templateReferences,
    draft.source?.template_references,
  );
}

export function staticPageMissingEvidenceFromDraft(draft) {
  if (!draft) return null;
  return firstObject(
    draft.missingEvidence,
    draft.missing_evidence,
    draft.source?.missingEvidence,
    draft.source?.missing_evidence,
  );
}

export function staticPageTemplateAdaptationFromDraft(draft) {
  if (!draft) return null;
  return firstObject(
    draft.templateAdaptation,
    draft.template_adaptation,
    draft.source?.templateAdaptation,
    draft.source?.template_adaptation,
  );
}

export function staticPageStructureSignalsFromDraft(draft) {
  if (!draft) return null;
  const raw = firstObject(
    draft.structureSignals,
    draft.structure_signals,
    draft.dataSnapshot?.structureSignals,
    draft.dataSnapshot?.structure_signals,
    draft.source?.structureSignals,
    draft.source?.structure_signals,
  );
  if (!raw) return null;

  const sectionTitleHints = collectCleanTextList(raw.sectionTitleHints || raw.section_title_hints, []);
  const fieldCandidates = asArray(raw.fieldCandidates || raw.field_candidates).slice(0, 4);
  for (const candidate of fieldCandidates) {
    collectCleanTextList(candidate.sectionTitleHints || candidate.section_title_hints, sectionTitleHints);
  }
  const boundModules = asArray(raw.boundModules || raw.bound_modules)
    .map((item) => ({
      moduleId: cleanText(item.moduleId || item.module_id),
      title: cleanText(item.title),
      status: cleanText(item.bindingQualityStatus || item.binding_quality_status || item.status),
    }))
    .filter((item) => item.moduleId || item.title)
    .slice(0, 8);
  const status = cleanText(raw.status) || (sectionTitleHints.length ? 'available' : 'none');
  if (status === 'none' && !sectionTitleHints.length && !boundModules.length) {
    return null;
  }
  return {
    status,
    policy: cleanText(raw.policy),
    sectionTitleHints: sectionTitleHints.slice(0, 8),
    boundModules,
  };
}

function TemplateReferenceMeta({ reference }) {
  const source = cleanText(reference?.source) || 'html-anything';
  const importPolicy = cleanText(reference?.importPolicy || reference?.import_policy) || 'metadata_and_constraints_only';
  const styleDirection = cleanText(reference?.styleDirection || reference?.style_direction);
  const providerPolicy = reference?.providerPolicy || reference?.provider_policy || {};
  const forbiddenOutput = asArray(providerPolicy.forbiddenOutput || providerPolicy.forbidden_output);
  const status = cleanText(reference?.status) || 'selected';

  return (
    <div className="static-page-template-meta" aria-label="模板参考元数据">
      <div>
        <span>来源</span>
        <strong>{source}</strong>
      </div>
      <div>
        <span>导入</span>
        <strong>{importPolicy}</strong>
      </div>
      <div>
        <span>风格</span>
        <strong>{styleDirection || '跟随草稿'}</strong>
      </div>
      <div>
        <span>状态</span>
        <strong>{TEMPLATE_STATUS_LABELS[status] || status}</strong>
      </div>
      {forbiddenOutput.length ? (
        <div className="wide">
          <span>禁出</span>
          <strong>{forbiddenOutput.slice(0, 5).join(' / ')}</strong>
        </div>
      ) : null}
    </div>
  );
}

function MissingEvidenceList({ missingEvidence }) {
  const status = cleanText(missingEvidence?.status) || 'ready';
  const items = asArray(missingEvidence?.items).slice(0, 4);
  if (!items.length && status === 'ready') {
    return (
      <div className="static-page-missing-evidence ready">
        <span>证据状态</span>
        <strong>{MISSING_STATUS_LABELS[status] || status}</strong>
      </div>
    );
  }

  return (
    <div className={`static-page-missing-evidence ${status === 'needs_evidence' ? 'attention' : ''}`.trim()}>
      <div className="static-page-missing-head">
        <span>证据状态</span>
        <strong>{MISSING_STATUS_LABELS[status] || status}</strong>
      </div>
      {items.length ? (
        <div className="static-page-missing-list">
          {items.map((item, index) => {
            const code = cleanText(item.code) || `missing-${index + 1}`;
            const message = cleanText(item.message);
            const action = labelForAction(item.recommendedAction || item.recommended_action);
            return (
              <div className="static-page-missing-item" key={`${code}-${index}`}>
                <span>{code}</span>
                <strong>{action}</strong>
                {message ? <p>{message}</p> : null}
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

function StructureSignalsList({ structureSignals }) {
  if (!structureSignals) return null;
  const hints = asArray(structureSignals.sectionTitleHints).slice(0, 8);
  const boundModules = asArray(structureSignals.boundModules).slice(0, 5);
  const status = cleanText(structureSignals.status) || (hints.length ? 'available' : 'none');

  return (
    <div className={`static-page-structure-signals ${status === 'available' ? 'ready' : ''}`.trim()}>
      <div className="static-page-structure-head">
        <span>源结构</span>
        <strong>{STRUCTURE_STATUS_LABELS[status] || status}</strong>
      </div>
      {hints.length ? (
        <div className="static-page-structure-hints" aria-label="源文档标题线索">
          {hints.map((hint) => <span key={hint}>{hint}</span>)}
        </div>
      ) : null}
      {boundModules.length ? (
        <div className="static-page-structure-modules" aria-label="已绑定结构模块">
          {boundModules.map((module) => (
            <em key={module.moduleId || module.title}>
              {module.title || module.moduleId}
            </em>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function TemplateAdaptationSummary({ adaptation }) {
  if (!adaptation) return null;
  const summary = cleanText(adaptation.summary);
  const subject = cleanText(adaptation.adaptedSubject || adaptation.adapted_subject);
  const focus = asArray(adaptation.focus)
    .map((item) => cleanText(item.label || item.code || item.instruction))
    .filter(Boolean)
    .slice(0, 3);
  if (!summary && !subject && !focus.length) return null;

  return (
    <div className="static-page-template-adaptation" aria-label="模板按本轮意向调整">
      <div className="static-page-template-adaptation-head">
        <span>本轮意向</span>
        <strong>{subject || '已按当前需求调整'}</strong>
      </div>
      {summary ? <p>{summary}</p> : null}
      {focus.length ? (
        <div className="static-page-template-hints">
          {focus.map((item) => <span key={item}>{item}</span>)}
        </div>
      ) : null}
    </div>
  );
}

export default function StaticPageTemplateReferencePanel({ draft }) {
  const reference = staticPageTemplateReferenceFromDraft(draft);
  const missingEvidence = staticPageMissingEvidenceFromDraft(draft);
  const adaptation = staticPageTemplateAdaptationFromDraft(draft);
  const structureSignals = staticPageStructureSignalsFromDraft(draft);
  const missingItems = asArray(missingEvidence?.items);
  const hasStructureSignals = Boolean(
    structureSignals?.sectionTitleHints?.length || structureSignals?.boundModules?.length,
  );
  if (!reference && !adaptation && !missingItems.length && !hasStructureSignals) {
    return null;
  }

  const templateId = cleanText(reference?.templateId || reference?.template_id || reference?.id);
  const label = cleanText(reference?.label || reference?.name) || templateId || '结构线索';
  const designIntent = cleanText(reference?.designIntent || reference?.design_intent);
  const promptHints = asArray(reference?.promptHints || reference?.prompt_hints)
    .map(cleanText)
    .filter(Boolean)
    .slice(0, 3);

  return (
    <section className="static-page-template-reference" aria-label="模板参考与证据状态">
      <div className="static-page-template-head">
        <div>
          <span>模板参考</span>
          <strong>{label}</strong>
        </div>
        {templateId ? <em>{templateId}</em> : null}
      </div>
      {designIntent ? <p className="static-page-template-intent">{designIntent}</p> : null}
      {adaptation ? <TemplateAdaptationSummary adaptation={adaptation} /> : null}
      {reference ? <TemplateReferenceMeta reference={reference} /> : null}
      {promptHints.length ? (
        <div className="static-page-template-hints">
          {promptHints.map((hint) => <span key={hint}>{hint}</span>)}
        </div>
      ) : null}
      {structureSignals ? <StructureSignalsList structureSignals={structureSignals} /> : null}
      {missingEvidence ? <MissingEvidenceList missingEvidence={missingEvidence} /> : null}
    </section>
  );
}
