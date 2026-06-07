#!/usr/bin/env node

import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const DEFAULT_OUTPUT_DIR = 'target/document-dedup-readthrough-smoke';

function parseArgs(argv) {
  const args = {
    selfTest: false,
    outputDir: process.env.DOCUMENT_DEDUP_READTHROUGH_SMOKE_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    pretty: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return args;
}

function requireValue(name, value) {
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  console.log(`Usage:
  node scripts/smoke/document-dedup-readthrough.mjs --self-test

The self-test is deterministic and does not call DataMax. It verifies the
authorization semantics expected from the live duplicate/canonical read-through
path:
  - duplicate document ids resolve to canonical ids before chunk supply
  - dataset_external_ids may authorize multiple groups
  - document and group authorization de-duplicate the same canonical document
  - conversation_external_id restores the prior visible scope
`);
}

function makeRunId() {
  return new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 14);
}

function buildFixture() {
  const canonical = {
    documentId: 'doc-canonical-alpha',
    externalId: 'external-alpha-v1',
    datasetExternalIds: ['dataset-group-a'],
    canonicalDocumentId: 'doc-canonical-alpha',
    dedupState: 'canonical',
    chunkRefs: ['chunk-alpha-canonical'],
  };
  const duplicate = {
    documentId: 'doc-duplicate-alpha',
    externalId: 'external-alpha-v1-duplicate',
    datasetExternalIds: ['dataset-group-b'],
    canonicalDocumentId: 'doc-canonical-alpha',
    dedupState: 'duplicate',
    chunkRefs: [],
  };
  const beta = {
    documentId: 'doc-canonical-beta',
    externalId: 'external-beta-v1',
    datasetExternalIds: ['dataset-group-a', 'dataset-group-c'],
    canonicalDocumentId: 'doc-canonical-beta',
    dedupState: 'canonical',
    chunkRefs: ['chunk-beta-canonical'],
  };
  const documents = [canonical, duplicate, beta];
  return {
    documents,
    byDocumentId: new Map(documents.map((document) => [document.documentId, document])),
    byExternalId: new Map(documents.map((document) => [document.externalId, document])),
    canonicalChunks: new Map([
      ['doc-canonical-alpha', ['chunk-alpha-canonical']],
      ['doc-canonical-beta', ['chunk-beta-canonical']],
    ]),
    conversationScopes: new Map(),
  };
}

function unique(values) {
  const seen = new Set();
  const result = [];
  for (const value of values) {
    if (!value || seen.has(value)) {
      continue;
    }
    seen.add(value);
    result.push(value);
  }
  return result;
}

function resolveDocumentRef(fixture, ref) {
  return fixture.byDocumentId.get(ref) || fixture.byExternalId.get(ref) || null;
}

function canonicalizeDocuments(documents) {
  return unique(documents.map((document) => document?.canonicalDocumentId).filter(Boolean));
}

function authorizedDocumentsFromDatasets(fixture, datasetExternalIds = []) {
  const groups = new Set(datasetExternalIds);
  return fixture.documents.filter((document) => (
    document.datasetExternalIds.some((datasetExternalId) => groups.has(datasetExternalId))
  ));
}

function authorizeScope(fixture, request) {
  const conversationExternalId = String(request.conversationExternalId || '').trim();
  const datasetExternalIds = Array.isArray(request.datasetExternalIds) ? request.datasetExternalIds : [];
  const documentRefs = Array.isArray(request.availableDocumentExternalIds)
    ? request.availableDocumentExternalIds
    : [];
  const explicitDocuments = documentRefs
    .map((ref) => resolveDocumentRef(fixture, ref))
    .filter(Boolean);
  const groupedDocuments = authorizedDocumentsFromDatasets(fixture, datasetExternalIds);
  const suppliedCanonicalDocumentIds = canonicalizeDocuments([
    ...groupedDocuments,
    ...explicitDocuments,
  ]);
  const inherited = conversationExternalId
    ? fixture.conversationScopes.get(conversationExternalId) || []
    : [];
  const canonicalDocumentIds = suppliedCanonicalDocumentIds.length
    ? suppliedCanonicalDocumentIds
    : inherited;
  if (conversationExternalId && suppliedCanonicalDocumentIds.length) {
    fixture.conversationScopes.set(conversationExternalId, canonicalDocumentIds);
  }
  const chunkRefs = canonicalDocumentIds.flatMap((documentId) => (
    fixture.canonicalChunks.get(documentId) || []
  ));
  return {
    conversationExternalId,
    canonicalDocumentIds,
    chunkRefs,
    groupedDocumentCount: groupedDocuments.length,
    explicitDocumentCount: explicitDocuments.length,
    inherited: !suppliedCanonicalDocumentIds.length && inherited.length > 0,
  };
}

function summarizeScope(scope) {
  return {
    conversationExternalId: scope.conversationExternalId,
    canonicalDocumentIds: scope.canonicalDocumentIds,
    canonicalDocumentCount: scope.canonicalDocumentIds.length,
    chunkRefs: scope.chunkRefs,
    chunkCount: scope.chunkRefs.length,
    groupedDocumentCount: scope.groupedDocumentCount,
    explicitDocumentCount: scope.explicitDocumentCount,
    inherited: scope.inherited,
  };
}

function runSelfTest() {
  const fixture = buildFixture();
  const cases = [];

  const duplicateDocument = authorizeScope(fixture, {
    conversationExternalId: 'conv-duplicate-doc',
    availableDocumentExternalIds: ['external-alpha-v1-duplicate'],
  });
  assert.deepEqual(duplicateDocument.canonicalDocumentIds, ['doc-canonical-alpha']);
  assert.deepEqual(duplicateDocument.chunkRefs, ['chunk-alpha-canonical']);
  cases.push({ caseId: 'duplicate_document_ref_reads_canonical_chunks', ...summarizeScope(duplicateDocument) });

  const duplicateDataset = authorizeScope(fixture, {
    conversationExternalId: 'conv-duplicate-dataset',
    datasetExternalIds: ['dataset-group-b'],
  });
  assert.deepEqual(duplicateDataset.canonicalDocumentIds, ['doc-canonical-alpha']);
  assert.deepEqual(duplicateDataset.chunkRefs, ['chunk-alpha-canonical']);
  cases.push({ caseId: 'duplicate_dataset_ref_reads_canonical_chunks', ...summarizeScope(duplicateDataset) });

  const multiDataset = authorizeScope(fixture, {
    conversationExternalId: 'conv-multi-dataset',
    datasetExternalIds: ['dataset-group-a', 'dataset-group-b'],
  });
  assert.deepEqual(multiDataset.canonicalDocumentIds, ['doc-canonical-alpha', 'doc-canonical-beta']);
  assert.deepEqual(multiDataset.chunkRefs, ['chunk-alpha-canonical', 'chunk-beta-canonical']);
  cases.push({ caseId: 'multiple_dataset_groups_deduplicate_canonical_supply', ...summarizeScope(multiDataset) });

  const mixedAuthorization = authorizeScope(fixture, {
    conversationExternalId: 'conv-mixed-auth',
    datasetExternalIds: ['dataset-group-b'],
    availableDocumentExternalIds: ['external-alpha-v1'],
  });
  assert.deepEqual(mixedAuthorization.canonicalDocumentIds, ['doc-canonical-alpha']);
  assert.equal(mixedAuthorization.chunkRefs.length, 1);
  cases.push({ caseId: 'document_and_dataset_authorization_deduplicate_supply', ...summarizeScope(mixedAuthorization) });

  const inherited = authorizeScope(fixture, {
    conversationExternalId: 'conv-mixed-auth',
  });
  assert.deepEqual(inherited.canonicalDocumentIds, ['doc-canonical-alpha']);
  assert.equal(inherited.inherited, true);
  cases.push({ caseId: 'conversation_scope_inherits_previous_authorization', ...summarizeScope(inherited) });

  const isolated = authorizeScope(fixture, {
    conversationExternalId: 'conv-isolated',
  });
  assert.deepEqual(isolated.canonicalDocumentIds, []);
  assert.equal(isolated.inherited, false);
  cases.push({ caseId: 'different_conversation_without_authorization_stays_empty', ...summarizeScope(isolated) });

  return {
    reportType: 'document_dedup_readthrough_self_test',
    generatedAt: new Date().toISOString(),
    result: 'passed',
    caseCount: cases.length,
    cases,
    redaction: {
      rawDocumentTextIncluded: false,
      rawPayloadIncluded: false,
      rawObjectPathIncluded: false,
      credentialIncluded: false,
    },
  };
}

async function writeReport(outputDir, report, pretty = false) {
  await mkdir(outputDir, { recursive: true });
  const filename = `document-dedup-readthrough-self-test-${makeRunId()}.json`;
  const path = join(outputDir, filename);
  await writeFile(path, JSON.stringify(report, null, pretty ? 2 : 0));
  return path;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.selfTest) {
    printHelp();
    throw new Error('--self-test is required for this deterministic smoke');
  }
  const report = runSelfTest();
  const reportPath = await writeReport(args.outputDir, report, args.pretty);
  console.log(`OK document dedup readthrough self-test: cases=${report.caseCount} report=${reportPath}`);
}

main().catch((error) => {
  console.error(error?.message || error);
  process.exit(1);
});
