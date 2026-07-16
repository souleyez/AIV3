#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

import { buildDatasetUnderstandingGraph } from '../apps/web/app/lib/dataset-understanding-graph.js';
import { applyDatasetUnderstandingGraphBudget } from '../apps/web/app/lib/dataset-understanding-graph-budget.js';
import { layoutDatasetUnderstandingGraph } from '../apps/web/app/lib/dataset-understanding-graph-layout.js';
import {
  newbaiProjectMaterialsNoisyFallback,
} from '../apps/web/app/lib/fixtures/newbai-project-materials-noisy-fallback.js';

export const DATASET_GRAPH_QUALITY_THRESHOLDS = Object.freeze({
  chineseStartedPercent: 95,
  nonChineseStartedPercent: 5,
  standardMinNodes: 90,
  standardMaxNodes: 120,
  rootNodePx: 42,
});

function text(value) {
  return typeof value === 'string' ? value.trim() : '';
}

function normalizedDocumentTitle(value) {
  return text(value)
    .normalize('NFKC')
    .toLocaleLowerCase()
    .replace(/\.(?:xlsx?|xlsm|csv|zip|pdf|docx?|pptx?|txt|md)$/i, '')
    .replace(/[^\p{L}\p{N}]+/gu, '')
    .trim();
}

function countBy(values, classifier) {
  return values.reduce((counts, value) => {
    const key = classifier(value);
    counts[key] = (counts[key] || 0) + 1;
    return counts;
  }, {});
}

function labelStartClass(value) {
  const valueText = text(value);
  if (/^\p{Script=Han}/u.test(valueText)) return 'chineseStarted';
  if (/^\d/u.test(valueText)) return 'numericStarted';
  if (/^[A-Za-z]/u.test(valueText)) return 'asciiStarted';
  return 'punctuationStarted';
}

function countNormalizedDocumentDuplicates(nodes) {
  const groups = countBy(
    nodes.filter((node) => node.kind === 'document'),
    (node) => normalizedDocumentTitle(node.name) || `empty:${node.id}`,
  );
  return Object.values(groups).reduce((total, count) => total + Math.max(0, count - 1), 0);
}

function roundPercent(value, total) {
  return total > 0 ? Number(((value / total) * 100).toFixed(2)) : 0;
}

function collectStringValues(value, values = []) {
  if (typeof value === 'string') {
    values.push(value);
    return values;
  }
  if (Array.isArray(value)) {
    value.forEach((item) => collectStringValues(item, values));
    return values;
  }
  if (value && typeof value === 'object') {
    Object.values(value).forEach((item) => collectStringValues(item, values));
  }
  return values;
}

function countMatches(values, pattern) {
  return values.filter((value) => pattern.test(text(value))).length;
}

function nodeLabel(node) {
  return text(node?.name) || text(node?.display_label) || text(node?.label);
}

function edgeEvidence(edge) {
  return text(edge?.evidence)
    || text(edge?.reason)
    || text(edge?.evidence_class)
    || text(edge?.type);
}

function graphLinks(model) {
  if (Array.isArray(model?.links)) return model.links;
  if (!Array.isArray(model?.edges)) return [];
  return model.edges.map((edge) => ({
    ...edge,
    source: edge.source ?? edge.source_id,
    target: edge.target ?? edge.target_id,
  }));
}

function semanticPayloadAsGraph(payload) {
  const datasetId = text(payload?.dataset?.id)
    || text(payload?.dataset_id)
    || text(payload?.root_dataset_id)
    || 'audit-dataset';
  const objects = Array.isArray(payload?.objects) ? payload.objects : [];
  const fields = Array.isArray(payload?.fields) ? payload.fields : [];
  const relations = Array.isArray(payload?.relations) ? payload.relations : [];
  const rootId = `dataset:${datasetId}`;
  const nodes = [{
    id: rootId,
    kind: 'dataset',
    entityType: 'dataset',
    name: text(payload?.dataset?.title) || text(payload?.dataset_title) || '当前数据集',
  }];
  const links = [];

  objects.forEach((object, index) => {
    const id = text(object?.id) || `object:${index}`;
    nodes.push({
      id,
      kind: text(object?.kind) || 'object',
      entityType: 'object',
      name: text(object?.label) || text(object?.display_label) || '业务对象',
      businessScore: Number(object?.confidence) || 0,
    });
    links.push({
      id: `audit:dataset-object:${id}`,
      source: rootId,
      target: id,
      structural: true,
      rootRelation: true,
      type: 'observed',
      evidence: '语义快照对象归属',
    });
  });

  fields.forEach((field, index) => {
    const id = text(field?.id) || `field:${index}`;
    const objectId = text(field?.object_id);
    nodes.push({
      id,
      kind: 'field',
      entityType: 'field',
      objectId,
      name: text(field?.label) || text(field?.display_label) || '业务字段',
      businessScore: Number(field?.confidence) || Number(field?.non_empty_count) || 0,
      technicalOnly: field?.status === 'unresolved',
    });
    if (objectId) {
      links.push({
        id: `audit:object-field:${id}`,
        source: objectId,
        target: id,
        structural: true,
        rootRelation: false,
        type: 'observed',
        evidence: '语义快照字段归属',
      });
    }
  });

  relations.forEach((relation, index) => {
    const source = text(relation?.source_id) || text(relation?.source_object_id);
    const target = text(relation?.target_id) || text(relation?.target_object_id);
    if (!source || !target) return;
    links.push({
      id: text(relation?.id) || `audit:relation:${index}`,
      source,
      target,
      type: text(relation?.evidence_class) || 'observed',
      evidence: text(relation?.reason) || text(relation?.label) || '语义快照关系证据',
    });
  });

  return {
    mode: 'semantic',
    snapshotStatus: text(payload?.status) || 'unknown',
    nodes,
    links,
    auditPayload: payload,
  };
}

function normalizeAuditInput(input) {
  if (Array.isArray(input?.objects) || Array.isArray(input?.fields)) {
    return semanticPayloadAsGraph(input);
  }
  const nodes = Array.isArray(input?.nodes)
    ? input.nodes.map((node) => ({ ...node, name: nodeLabel(node) }))
    : [];
  return {
    ...input,
    mode: text(input?.mode) || (text(input?.schema_version) ? 'cross' : 'unknown'),
    snapshotStatus: text(input?.snapshotStatus)
      || text(input?.snapshot_status)
      || text(input?.status)
      || text(input?.cross_links_status)
      || 'unknown',
    nodes,
    links: graphLinks(input),
    auditPayload: input,
  };
}

function projectGraph(model, density) {
  try {
    return applyDatasetUnderstandingGraphBudget({
      nodes: Array.isArray(model?.nodes) ? model.nodes : [],
      links: graphLinks(model),
    }, { density });
  } catch {
    return null;
  }
}

export function auditDatasetUnderstandingGraph(model, fixture = 'custom') {
  const nodes = Array.isArray(model?.nodes) ? model.nodes : [];
  const links = graphLinks(model);
  const labels = nodes.map(nodeLabel);
  const startCounts = countBy(labels, labelStartClass);
  const categoryCounts = countBy(nodes, (node) => text(node.kind) || 'unknown');
  const countLabels = (pattern) => labels.filter((label) => pattern.test(label)).length;
  const publicPayloadMeasured = Object.hasOwn(model || {}, 'auditPayload');
  const payloadValues = publicPayloadMeasured ? collectStringValues(model.auditPayload) : [];
  const contractMetadataValues = new Set([
    text(model?.auditPayload?.schema_version),
    text(model?.auditPayload?.generation_version),
  ].filter(Boolean));
  const userFacingPayloadValues = payloadValues.filter((value) => !contractMetadataValues.has(value));
  const standardProjection = projectGraph(model, 'standard');
  const expandedProjection = projectGraph(model, 'expanded');
  const positioned = layoutDatasetUnderstandingGraph(nodes);
  const root = positioned.find((node) => node.kind === 'dataset');
  const nonChineseStarted = (startCounts.numericStarted || 0)
    + (startCounts.asciiStarted || 0)
    + (startCounts.punctuationStarted || 0);

  return {
    fixture,
    mode: text(model?.mode) || 'unknown',
    snapshotStatus: text(model?.snapshotStatus) || 'unknown',
    totalNodes: nodes.length,
    totalEdges: links.length,
    labelQuality: {
      total: labels.length,
      chineseStartedPercent: roundPercent(startCounts.chineseStarted || 0, labels.length),
      nonChineseStartedPercent: roundPercent(nonChineseStarted, labels.length),
    },
    projection: {
      standardNodes: standardProjection?.nodes?.length ?? null,
      standardEdges: standardProjection?.links?.length ?? null,
      standardAvailableNodes: standardProjection?.stats?.availableNodeCount ?? nodes.length,
      expandedNodes: expandedProjection?.nodes?.length ?? null,
      expandedEdges: expandedProjection?.links?.length ?? null,
      rootNodePx: root?.symbolSize ?? null,
    },
    categoryCounts: Object.fromEntries(Object.entries(categoryCounts).sort(([left], [right]) => left.localeCompare(right))),
    noise: {
      chineseStarted: startCounts.chineseStarted || 0,
      numericStarted: startCounts.numericStarted || 0,
      asciiStarted: startCounts.asciiStarted || 0,
      punctuationStarted: startCounts.punctuationStarted || 0,
      suspectedDataRows: countLabels(/\t|(?:\d[\s,|;，；]+){3,}\d/u),
      sqlOrComments: countLabels(/\/\*|\*\/|--|\b(?:select|from|where|date_add|datediff|case\s+when|as\s+[a-z_]+)\b/iu),
      mimeValues: countLabels(/^(?:application|audio|image|text|video)\/[a-z0-9.+-]+/iu),
      strategyIdentifiers: countLabels(/(?:paragraph_aware|noun_terms|profile|parser|strategy)(?:_[a-z0-9]+)*/iu),
      extensionDocumentTitles: nodes.filter((node) => (
        node.kind === 'document'
          && /\.(?:xlsx?|xlsm|csv|zip|pdf|docx?|pptx?|txt|md)$/i.test(text(node.name))
      )).length,
      normalizedDuplicateDocumentTitles: countNormalizedDocumentDuplicates(nodes),
    },
    publicPayloadLeakage: {
      status: publicPayloadMeasured ? 'measured' : 'skipped',
      reason: publicPayloadMeasured ? 'sanitized input JSON scanned' : 'fixture mode only audits the main canvas',
      suspectedDataRows: publicPayloadMeasured ? countMatches(payloadValues, /\t|(?:\d[\s,|;，；]+){3,}\d/u) : null,
      sqlOrComments: publicPayloadMeasured ? countMatches(payloadValues, /\/\*|\*\/|--|\b(?:select|from|where|date_add|datediff|case\s+when|as\s+[a-z_]+)\b/iu) : null,
      mimeValues: publicPayloadMeasured ? countMatches(payloadValues, /^(?:application|audio|image|text|video)\/[a-z0-9.+-]+/iu) : null,
      strategyIdentifiers: publicPayloadMeasured ? countMatches(userFacingPayloadValues, /(?:paragraph_aware|noun_terms|profile|parser|strategy)(?:_[a-z0-9]+)*/iu) : null,
      internalPaths: publicPayloadMeasured ? countMatches(payloadValues, /(?:[A-Za-z]:\\|\/(?:etc|home|opt|srv|tmp|var)\/)/u) : null,
      connectionStrings: publicPayloadMeasured ? countMatches(payloadValues, /(?:jdbc|mongodb|oracle|postgres(?:ql)?|redis):\/\/|(?:host|password|server|user\s*id)\s*=/iu) : null,
      hex64: publicPayloadMeasured ? countMatches(payloadValues, /\b[a-f0-9]{64}\b/iu) : null,
    },
    missingEvidenceEdges: links.filter((link) => !edgeEvidence(link)).length,
  };
}

function check(name, status, actual, expected, scope) {
  return { name, status, actual, expected, scope };
}

export function evaluateDatasetUnderstandingQuality(report, {
  profile = 'fixture-sanitization',
  requireAll = false,
} = {}) {
  if (!['fixture-sanitization', 'task12'].includes(profile)) {
    throw new Error(`unsupported quality profile: ${profile}`);
  }
  const thresholds = DATASET_GRAPH_QUALITY_THRESHOLDS;
  const checks = [];
  const readyRequired = profile === 'task12';
  checks.push(readyRequired
    ? check('ready_snapshot', report.snapshotStatus === 'ready' ? 'passed' : 'failed', report.snapshotStatus, 'ready', 'input')
    : check('ready_snapshot', 'skipped', report.snapshotStatus, 'ready', 'live semantic input required'));
  checks.push(check(
    'chinese_main_labels',
    report.labelQuality.chineseStartedPercent >= thresholds.chineseStartedPercent ? 'passed' : 'failed',
    report.labelQuality.chineseStartedPercent,
    `>=${thresholds.chineseStartedPercent}%`,
    'main canvas',
  ));
  checks.push(check(
    'non_chinese_main_labels',
    report.labelQuality.nonChineseStartedPercent <= thresholds.nonChineseStartedPercent ? 'passed' : 'failed',
    report.labelQuality.nonChineseStartedPercent,
    `<=${thresholds.nonChineseStartedPercent}%`,
    'main canvas',
  ));

  for (const key of [
    'suspectedDataRows',
    'sqlOrComments',
    'mimeValues',
    'strategyIdentifiers',
    'extensionDocumentTitles',
    'normalizedDuplicateDocumentTitles',
  ]) {
    checks.push(check(`main_canvas_${key}`, report.noise[key] === 0 ? 'passed' : 'failed', report.noise[key], 0, 'main canvas'));
  }
  checks.push(check(
    'edge_evidence',
    report.missingEvidenceEdges === 0 ? 'passed' : 'failed',
    report.missingEvidenceEdges,
    0,
    'visible edges',
  ));
  checks.push(check(
    'root_node_size',
    report.projection.rootNodePx === thresholds.rootNodePx ? 'passed' : 'failed',
    report.projection.rootNodePx,
    thresholds.rootNodePx,
    'deterministic layout',
  ));

  if (report.projection.standardAvailableNodes >= thresholds.standardMinNodes) {
    const standardNodes = report.projection.standardNodes;
    checks.push(check(
      'standard_node_budget',
      standardNodes >= thresholds.standardMinNodes && standardNodes <= thresholds.standardMaxNodes ? 'passed' : 'failed',
      standardNodes,
      `${thresholds.standardMinNodes}-${thresholds.standardMaxNodes}`,
      'standard projection',
    ));
  } else {
    checks.push(check(
      'standard_node_budget',
      'skipped',
      report.projection.standardAvailableNodes,
      `available nodes >=${thresholds.standardMinNodes}`,
      'insufficient high-quality fixture nodes',
    ));
  }

  if (profile === 'task12') {
    checks.push(check(
      'public_payload_scan',
      report.publicPayloadLeakage.status === 'measured' ? 'passed' : 'failed',
      report.publicPayloadLeakage.status,
      'measured',
      'sanitized public payload',
    ));
    for (const key of [
      'suspectedDataRows',
      'sqlOrComments',
      'mimeValues',
      'strategyIdentifiers',
      'internalPaths',
      'connectionStrings',
      'hex64',
    ]) {
      const value = report.publicPayloadLeakage[key];
      checks.push(check(`public_payload_${key}`, value === 0 ? 'passed' : 'failed', value, 0, 'sanitized public payload'));
    }
  }

  const failed = checks.filter((item) => item.status === 'failed');
  const skipped = checks.filter((item) => item.status === 'skipped');
  if (requireAll && skipped.length) {
    const requiredCheck = check('require_all', 'failed', skipped.length, 0, 'release gate');
    checks.push(requiredCheck);
    failed.push(requiredCheck);
  }
  return {
    profile,
    status: failed.length ? 'failed' : 'passed',
    complete: failed.length === 0 && skipped.length === 0,
    checks,
    failedCount: failed.length,
    skippedCount: skipped.length,
  };
}

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : '';
}

async function runCli() {
  const fixtureName = optionValue('--fixture');
  const inputPath = optionValue('--input');
  if (fixtureName && inputPath) {
    throw new Error('Use either --fixture or --input, not both.');
  }
  let model;
  let sourceName;
  if (inputPath) {
    const input = JSON.parse(await readFile(resolve(inputPath), 'utf8'));
    model = normalizeAuditInput(input);
    sourceName = 'input-json';
  } else {
    if (fixtureName !== 'newbai-project-materials') {
      throw new Error('Use --fixture newbai-project-materials or --input <sanitized-json>.');
    }
    const { dataset, documents, understanding } = newbaiProjectMaterialsNoisyFallback;
    model = buildDatasetUnderstandingGraph(dataset, documents, understanding);
    sourceName = fixtureName;
  }
  const profile = optionValue('--profile') || (inputPath ? 'task12' : 'fixture-sanitization');
  const report = auditDatasetUnderstandingGraph(model, sourceName);
  const gate = evaluateDatasetUnderstandingQuality(report, {
    profile,
    requireAll: process.argv.includes('--require-all'),
  });
  process.stdout.write(`${JSON.stringify({ ...report, gate }, null, 2)}\n`);
  if (gate.status !== 'passed') process.exitCode = 1;
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : '';
if (invokedPath === fileURLToPath(import.meta.url)) {
  await runCli();
}
