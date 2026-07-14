#!/usr/bin/env node

import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

import { buildDatasetUnderstandingGraph } from '../apps/web/app/lib/dataset-understanding-graph.js';
import {
  newbaiProjectMaterialsNoisyFallback,
} from '../apps/web/app/lib/fixtures/newbai-project-materials-noisy-fallback.js';

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

export function auditDatasetUnderstandingGraph(model, fixture = 'custom') {
  const nodes = Array.isArray(model?.nodes) ? model.nodes : [];
  const links = Array.isArray(model?.links) ? model.links : [];
  const labels = nodes.map((node) => text(node.name));
  const startCounts = countBy(labels, labelStartClass);
  const categoryCounts = countBy(nodes, (node) => text(node.kind) || 'unknown');
  const countLabels = (pattern) => labels.filter((label) => pattern.test(label)).length;

  return {
    fixture,
    mode: text(model?.mode) || 'unknown',
    snapshotStatus: text(model?.snapshotStatus) || 'unknown',
    totalNodes: nodes.length,
    totalEdges: links.length,
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
    missingEvidenceEdges: links.filter((link) => !text(link.evidence)).length,
  };
}

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : '';
}

function runCli() {
  const fixtureName = optionValue('--fixture');
  if (fixtureName !== 'newbai-project-materials') {
    throw new Error('Use --fixture newbai-project-materials.');
  }
  const { dataset, documents, understanding } = newbaiProjectMaterialsNoisyFallback;
  const model = buildDatasetUnderstandingGraph(dataset, documents, understanding);
  const report = auditDatasetUnderstandingGraph(model, fixtureName);
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : '';
if (invokedPath === fileURLToPath(import.meta.url)) {
  runCli();
}
