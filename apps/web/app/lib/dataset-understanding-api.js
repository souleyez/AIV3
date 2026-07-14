import { readLocalSecretBindingIdsHeader } from './local-account-state.js';
import { readLocalThreadId } from './local-browser-state.js';

export const DATASET_UNDERSTANDING_LIMITS = Object.freeze({
  objects: 40,
  fields: 160,
  relations: 240,
  examples: 5,
  evidenceRefs: 5,
});

const EVIDENCE_CLASSES = new Set(['confirmed', 'observed', 'inferred']);
const SEMANTIC_STATUSES = new Set(['confirmed', 'observed', 'inferred', 'unresolved']);
const CONTRACT_STATUSES = new Set(['ready', 'empty', 'building', 'failed']);

function text(value) {
  return typeof value === 'string' ? value.trim() : '';
}

function finiteNumber(value, fallback = 0) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function boundedConfidence(value) {
  return Math.max(0, Math.min(1, finiteNumber(value)));
}

function array(value, path) {
  if (!Array.isArray(value)) throw new TypeError(`${path} must be an array`);
  return value;
}

function assertLimit(items, key) {
  const limit = DATASET_UNDERSTANDING_LIMITS[key];
  if (items.length > limit) throw new RangeError(`${key} exceeds limit ${limit}`);
}

function requiredId(value, path) {
  const id = text(value);
  if (!id) throw new TypeError(`${path} is required`);
  return id;
}

export function datasetUnderstandingSelectionKey(datasetId) {
  return `single:${requiredId(datasetId, 'datasetId')}`;
}

function semanticStatus(value, path) {
  const status = text(value).toLowerCase();
  if (!SEMANTIC_STATUSES.has(status)) throw new TypeError(`${path} is invalid`);
  return status;
}

function normalizeEvidenceRefs(value, path) {
  const refs = Array.isArray(value) ? value : [];
  if (refs.length > DATASET_UNDERSTANDING_LIMITS.evidenceRefs) {
    throw new RangeError(`${path} exceeds limit ${DATASET_UNDERSTANDING_LIMITS.evidenceRefs}`);
  }
  return refs.map((reference, index) => ({
    source_kind: text(reference?.source_kind),
    source_id: requiredId(reference?.source_id, `${path}[${index}].source_id`),
    label: text(reference?.label),
  }));
}

export function isSensitiveSemanticExample(value) {
  const sample = text(String(value ?? ''));
  if (!sample) return false;
  return /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/i.test(sample)
    || /\b(?:postgres(?:ql)?|mysql|mariadb|mongodb|redis|jdbc):\/\//i.test(sample)
    || /\b(?:bearer\s+|api[_-]?key\s*[:=]|access[_-]?token\s*[:=]|password\s*[:=])/i.test(sample)
    || /^[A-Za-z]:[\\/]/.test(sample)
    || /^\/(?:home|users|root|etc|var)\//i.test(sample)
    || /\b1[3-9]\d{9}\b/.test(sample)
    || /\b\d{17}[\dXx]\b/.test(sample);
}

function safeExamples(value, path) {
  const examples = Array.isArray(value) ? value : [];
  if (examples.length > DATASET_UNDERSTANDING_LIMITS.examples) {
    throw new RangeError(`${path} exceeds limit ${DATASET_UNDERSTANDING_LIMITS.examples}`);
  }
  return [...new Set(examples
    .map((example) => text(String(example ?? '')))
    .filter((example) => example && !isSensitiveSemanticExample(example)))];
}

function normalizeObject(object, index) {
  return {
    id: requiredId(object?.id, `objects[${index}].id`),
    kind: text(object?.kind) || 'unknown',
    label: text(object?.label) || '待解释对象',
    technical_name: text(object?.technical_name),
    description: text(object?.description),
    label_source: text(object?.label_source),
    confidence: boundedConfidence(object?.confidence),
    coverage_count: Math.max(0, finiteNumber(object?.coverage_count)),
    status: semanticStatus(object?.status, `objects[${index}].status`),
    evidence_refs: normalizeEvidenceRefs(object?.evidence_refs, `objects[${index}].evidence_refs`),
  };
}

function normalizeField(field, index, objectIds) {
  const objectId = requiredId(field?.object_id, `fields[${index}].object_id`);
  if (!objectIds.has(objectId)) throw new TypeError(`fields[${index}].object_id references unknown object`);
  return {
    id: requiredId(field?.id, `fields[${index}].id`),
    object_id: objectId,
    label: text(field?.label) || '待解释字段',
    technical_name: text(field?.technical_name),
    semantic_role: text(field?.semantic_role) || 'unknown',
    value_type: text(field?.value_type) || 'unknown',
    non_empty_count: Math.max(0, finiteNumber(field?.non_empty_count)),
    distinct_count: Math.max(0, finiteNumber(field?.distinct_count)),
    examples: safeExamples(field?.examples, `fields[${index}].examples`),
    status: semanticStatus(field?.status, `fields[${index}].status`),
    label_source: text(field?.label_source),
    confidence: boundedConfidence(field?.confidence),
    evidence_refs: normalizeEvidenceRefs(field?.evidence_refs, `fields[${index}].evidence_refs`),
  };
}

function normalizeRelation(relation, index, nodeIds) {
  const evidenceClass = text(relation?.evidence_class).toLowerCase();
  if (!EVIDENCE_CLASSES.has(evidenceClass)) {
    throw new TypeError(`relations[${index}].evidence_class is invalid`);
  }
  const sourceId = requiredId(relation?.source_id, `relations[${index}].source_id`);
  const targetId = requiredId(relation?.target_id, `relations[${index}].target_id`);
  if (!nodeIds.has(sourceId)) throw new TypeError(`relations[${index}] references unknown source_id`);
  if (!nodeIds.has(targetId)) throw new TypeError(`relations[${index}] references unknown target_id`);
  return {
    id: requiredId(relation?.id, `relations[${index}].id`),
    source_id: sourceId,
    target_id: targetId,
    relation_type: text(relation?.relation_type) || 'related_to',
    label: text(relation?.label) || '相关',
    evidence_class: evidenceClass,
    confidence: boundedConfidence(relation?.confidence),
    reason: text(relation?.reason),
    evidence_refs: normalizeEvidenceRefs(relation?.evidence_refs, `relations[${index}].evidence_refs`),
  };
}

export function normalizeDatasetUnderstanding(payload) {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    throw new TypeError('dataset understanding payload must be an object');
  }
  const status = text(payload.status).toLowerCase();
  if (!CONTRACT_STATUSES.has(status)) throw new TypeError('status is invalid');
  const objectsInput = array(payload.objects, 'objects');
  const fieldsInput = array(payload.fields, 'fields');
  const relationsInput = array(payload.relations, 'relations');
  assertLimit(objectsInput, 'objects');
  assertLimit(fieldsInput, 'fields');
  assertLimit(relationsInput, 'relations');

  const objects = objectsInput.map(normalizeObject);
  const objectIds = new Set(objects.map((object) => object.id));
  if (objectIds.size !== objects.length) throw new TypeError('objects contain duplicate ids');
  const fields = fieldsInput.map((field, index) => normalizeField(field, index, objectIds));
  const fieldIds = new Set(fields.map((field) => field.id));
  if (fieldIds.size !== fields.length) throw new TypeError('fields contain duplicate ids');
  const nodeIds = new Set([...objectIds, ...fieldIds]);
  const relations = relationsInput.map((relation, index) => normalizeRelation(relation, index, nodeIds));
  if (new Set(relations.map((relation) => relation.id)).size !== relations.length) {
    throw new TypeError('relations contain duplicate ids');
  }

  return {
    schema_version: text(payload.schema_version),
    generation_version: text(payload.generation_version),
    status,
    dataset: {
      id: requiredId(payload.dataset?.id, 'dataset.id'),
      title: text(payload.dataset?.title) || '当前数据集',
    },
    coverage: Object.fromEntries([
      'source_count',
      'document_count',
      'record_count',
      'asset_count',
      'retrieval_evidence_count',
      'confirmed_fact_count',
      'unresolved_field_count',
    ].map((key) => [key, Math.max(0, finiteNumber(payload.coverage?.[key]))])),
    summary: {
      headline: text(payload.summary?.headline),
      limitations: (Array.isArray(payload.summary?.limitations) ? payload.summary.limitations : [])
        .map(text)
        .filter(Boolean),
    },
    objects,
    fields,
    relations,
    source_groups: (Array.isArray(payload.source_groups) ? payload.source_groups : []).map((group, index) => ({
      id: requiredId(group?.id, `source_groups[${index}].id`),
      kind: text(group?.kind),
      label: text(group?.label),
      object_ids: (Array.isArray(group?.object_ids) ? group.object_ids : []).filter((id) => objectIds.has(id)),
      coverage_count: Math.max(0, finiteNumber(group?.coverage_count)),
    })),
    pipeline: (Array.isArray(payload.pipeline) ? payload.pipeline : []).map((stage, index) => ({
      id: requiredId(stage?.id, `pipeline[${index}].id`),
      label: text(stage?.label),
      status: text(stage?.status),
      detail: text(stage?.detail),
      evidence_count: Math.max(0, finiteNumber(stage?.evidence_count)),
    })),
    generated_at: text(payload.generated_at),
    stale: payload.stale === true,
    truncated: {
      objects: Math.max(0, finiteNumber(payload.truncated?.objects)),
      fields: Math.max(0, finiteNumber(payload.truncated?.fields)),
      relations: Math.max(0, finiteNumber(payload.truncated?.relations)),
    },
  };
}

async function responseError(response) {
  const contentType = response.headers.get('content-type') || '';
  const payload = contentType.includes('application/json')
    ? await response.json()
    : await response.text();
  const message = typeof payload === 'string'
    ? payload
    : payload?.message || payload?.error || `Request failed: ${response.status}`;
  const error = new Error(message);
  error.status = response.status;
  error.payload = payload;
  return error;
}

export function createDatasetUnderstandingClient(dependencies = {}) {
  const {
    fetchImpl = (...args) => globalThis.fetch(...args),
    readSecretBindingIdsHeader = readLocalSecretBindingIdsHeader,
    readLocalThreadId: readThreadId = readLocalThreadId,
  } = dependencies;

  return async function fetchDatasetUnderstanding(datasetId, options = {}) {
    const id = requiredId(datasetId, 'datasetId');
    const selectionKey = datasetUnderstandingSelectionKey(id);
    const secretBindingIds = readSecretBindingIdsHeader();
    const etag = text(options.etag);
    const response = await fetchImpl(`/api/v3/datasets/${encodeURIComponent(id)}/understanding`, {
      method: 'GET',
      cache: 'no-store',
      credentials: 'include',
      signal: options.signal,
      headers: {
        Accept: 'application/json',
        ...(secretBindingIds ? { 'X-AI-Data-Platform-Secret-Binding-Ids': secretBindingIds } : {}),
        'X-AI-Data-Platform-Local-Thread-Id': readThreadId(),
        ...(etag ? { 'If-None-Match': etag } : {}),
      },
    });
    const responseEtag = response.headers.get('etag') || etag;
    if (response.status === 304) {
      return { data: null, etag: responseEtag, notModified: true, selectionKey };
    }
    if (!response.ok) throw await responseError(response);
    return {
      data: normalizeDatasetUnderstanding(await response.json()),
      etag: responseEtag,
      notModified: false,
      selectionKey,
    };
  };
}

export const fetchDatasetUnderstanding = createDatasetUnderstandingClient();
