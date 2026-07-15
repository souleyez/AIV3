#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, '..', '..');
const defaultFixturePath = resolve(repoRoot, 'fixtures', 'semantic-supply-ab', 'cases.jsonl');
const defaultRuntimeInputPath = resolve(
  repoRoot,
  'fixtures',
  'semantic-supply-ab',
  'runtime-inputs.jsonl',
);
const defaultOutputDir = resolve(repoRoot, 'target', 'semantic-supply-ab-smoke');

const resultSchemaVersion = 'semantic-supply-ab-result.v2';
const runtimeInputSchemaVersion = 'semantic-supply-runtime-input.v2';
const answerReceiptSchemaVersion = 'semantic-supply-answer-evaluation.v2';
const rustOfflineExecutionKind = 'rust_offline_retrieval';
const holdoutCaseIds = new Set([
  'semantic-alias-005',
  'semantic-alias-006',
  'semantic-source-004',
  'semantic-cross-doc-005',
  'semantic-cross-doc-006',
  'semantic-cross-dataset-004',
  'semantic-negative-003',
  'semantic-negative-004',
]);

const requiredCategoryMinimums = Object.freeze({
  field_alias: 6,
  source_localization: 4,
  cross_document_evidence: 6,
  multi_dataset_selected_scope: 4,
  negative_control: 4,
});

const thresholds = Object.freeze({
  recall_noninferiority: 0,
  mrr_noninferiority: -0.01,
  targeted_mrr_gain: 0.1,
  targeted_precision_at_5_gain: 0.15,
  targeted_recall_gain_for_supplement: 0.1,
  targeted_precision_at_5_noninferiority: 0,
  top5_source_overlap_relative_reduction: 0.3,
  top5_source_overlap_nonincrease: 0.01,
  latency_absolute_increase_ms: 100,
  latency_relative_increase: 0.2,
});

function usage() {
  return `Usage:
  node scripts/smoke/semantic-supply-ab.mjs [--fixture PATH] [--output-dir PATH]
  node scripts/smoke/semantic-supply-ab.mjs --self-test [--pretty]
  node scripts/smoke/semantic-supply-ab.mjs \\
    --runtime-input runtime-inputs.jsonl \\
    --a-results control.jsonl --b-results rerank.jsonl --c-results supplement.jsonl \\
    --require-retrieval-metrics [--pretty]
  node scripts/smoke/semantic-supply-ab.mjs \\
    --runtime-input runtime-inputs.jsonl \\
    --a-results control.jsonl --b-results rerank.jsonl --c-results supplement.jsonl \\
    --answer-receipt answer-evaluation.json --require-metrics [--pretty]

Modes:
  fixture-only  Validates the synthetic case matrix and writes a receipt.
  self-test     Tests the evaluator itself with safe synthetic rows. It never
                produces experiment evidence or a promotion-ready decision.
  results       Evaluates one exact Rust-exported v2 retrieval row per fixture
                case for all arms. --runtime-input is mandatory and independently
                supplies question, scope, permission, candidate pools and semantic
                snapshots whose hashes are recomputed before A/B/C are trusted.

--require-retrieval-metrics requires trustworthy grounding/retrieval/permission/
contract/latency evidence, but permits answer_evaluation.status=not_run.
--require-metrics is intentionally fail-closed for the v2 answer receipt. A
future reviewed schema must add independent per-case legacy-corpus receipts and
claim-to-document/chunk/source-locator citation binding before full answer
evidence can become promotion-eligible.`;
}

function parseArgs(argv) {
  const args = {
    fixturePath: defaultFixturePath,
    outputDir: defaultOutputDir,
    aResultsPath: null,
    bResultsPath: null,
    cResultsPath: null,
    runtimeInputPath: null,
    answerReceiptPath: null,
    requireMetrics: false,
    requireRetrievalMetrics: false,
    selfTest: false,
    pretty: false,
    help: false,
  };
  const valueFlags = new Map([
    ['--fixture', 'fixturePath'],
    ['--output-dir', 'outputDir'],
    ['--a-results', 'aResultsPath'],
    ['--control-results', 'aResultsPath'],
    ['--b-results', 'bResultsPath'],
    ['--rerank-results', 'bResultsPath'],
    ['--c-results', 'cResultsPath'],
    ['--supplement-results', 'cResultsPath'],
    ['--runtime-input', 'runtimeInputPath'],
    ['--answer-receipt', 'answerReceiptPath'],
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (valueFlags.has(arg)) {
      const value = argv[index + 1];
      if (!value || value.startsWith('--')) {
        throw new Error(`${arg} requires a value`);
      }
      args[valueFlags.get(arg)] = resolve(value);
      index += 1;
    } else if (arg === '--require-metrics') {
      args.requireMetrics = true;
      args.requireRetrievalMetrics = true;
    } else if (arg === '--require-retrieval-metrics') {
      args.requireRetrievalMetrics = true;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  const suppliedResultCount = [args.aResultsPath, args.bResultsPath, args.cResultsPath]
    .filter(Boolean).length;
  if (suppliedResultCount > 0 && suppliedResultCount < 3) {
    throw new Error('A/B/C result paths must be supplied together');
  }
  if (args.selfTest && suppliedResultCount > 0) {
    throw new Error('--self-test cannot be combined with result paths');
  }
  if (args.selfTest && args.answerReceiptPath) {
    throw new Error('--self-test cannot be combined with --answer-receipt');
  }
  if (args.selfTest && (args.requireMetrics || args.requireRetrievalMetrics)) {
    throw new Error('--self-test cannot be combined with metric requirement flags');
  }
  if (args.answerReceiptPath && suppliedResultCount === 0) {
    throw new Error('--answer-receipt requires A/B/C result paths');
  }
  if (suppliedResultCount === 3 && !args.runtimeInputPath) {
    throw new Error('--runtime-input is required with A/B/C result paths');
  }
  if (args.runtimeInputPath && suppliedResultCount === 0) {
    throw new Error('--runtime-input requires A/B/C result paths');
  }
  return args;
}

function determineCommandGate({
  mode,
  fixtureReady,
  evaluatorSelfTestReady,
  retrievalReady,
  answerEvaluation,
  requireRetrievalMetrics,
  requireMetrics,
  answerReceiptSupplied,
}) {
  let commandReady;
  if (mode === 'evaluator-self-test') {
    commandReady = fixtureReady && evaluatorSelfTestReady;
  } else if (mode === 'results') {
    const fullEvidenceReady = retrievalReady && answerEvaluation.ready;
    commandReady = fixtureReady && (requireMetrics || answerReceiptSupplied
      ? fullEvidenceReady
      : retrievalReady);
  } else {
    commandReady = fixtureReady && !requireRetrievalMetrics && !requireMetrics;
  }
  const fullEvidenceReady = mode === 'results' && retrievalReady && answerEvaluation.ready;
  const acceptanceStatus = mode === 'evaluator-self-test'
    ? commandReady ? 'evaluator_self_test_passed' : 'evaluator_self_test_failed'
    : mode === 'fixture-only'
      ? commandReady ? 'fixture_contract_valid' : 'metrics_requested_without_results'
      : fullEvidenceReady
        ? 'full_evidence_ready'
        : retrievalReady && answerEvaluation.status === 'not_run'
          ? 'retrieval_ready_answer_not_run'
          : retrievalReady
            ? 'answer_evaluation_failed'
            : 'retrieval_gate_failed';
  return {
    command_ready: commandReady,
    full_evidence_ready: fullEvidenceReady,
    decision_eligible: fullEvidenceReady,
    acceptance_status: acceptanceStatus,
  };
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function round(value, digits = 6) {
  if (!Number.isFinite(value)) return null;
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

function mean(values) {
  const finite = values.filter(Number.isFinite);
  return finite.length > 0 ? finite.reduce((sum, value) => sum + value, 0) / finite.length : 0;
}

function percentile95(values) {
  const sorted = values.filter(Number.isFinite).sort((left, right) => left - right);
  if (sorted.length === 0) return null;
  return sorted[Math.max(0, Math.ceil(sorted.length * 0.95) - 1)];
}

function stableValue(value) {
  if (Array.isArray(value)) return value.map(stableValue);
  if (!isObject(value)) return value;
  return Object.fromEntries(
    Object.keys(value).sort().map((key) => [key, stableValue(value[key])]),
  );
}

function stableStringify(value) {
  return JSON.stringify(stableValue(value));
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function isSha256(value) {
  return typeof value === 'string' && /^[a-f0-9]{64}$/u.test(value);
}

function fixtureSplit(fixture) {
  return holdoutCaseIds.has(fixture.id) ? 'holdout' : 'development';
}

function nonNegativeInteger(value) {
  return Number.isInteger(value) && value >= 0;
}

function finiteRatio(value) {
  return Number.isFinite(value) && value >= 0 && value <= 1;
}

function safeRelativePath(path) {
  if (!path) return null;
  const value = relative(repoRoot, path).replaceAll('\\', '/');
  return value.startsWith('..') ? '[outside-repository]' : value;
}

async function loadJsonl(path, idField) {
  const content = await readFile(path, 'utf8');
  const rows = [];
  const invalidRows = [];
  const duplicateIds = new Set();
  const seenIds = new Set();
  for (const [offset, rawLine] of content.split(/\r?\n/u).entries()) {
    if (!rawLine.trim()) continue;
    const line = offset + 1;
    try {
      const value = JSON.parse(rawLine);
      if (!isObject(value)) {
        invalidRows.push({ line, error: 'row_not_object' });
        continue;
      }
      const id = value[idField];
      if (typeof id === 'string' && id) {
        if (seenIds.has(id)) duplicateIds.add(id);
        seenIds.add(id);
      }
      rows.push({ line, value });
    } catch {
      invalidRows.push({ line, error: 'invalid_json' });
    }
  }
  return {
    rows,
    invalidRows,
    duplicateIds: [...duplicateIds].sort(),
    contentSha256: sha256(content),
    canonicalSha256: sha256(stableStringify(rows.map((row) => row.value))),
  };
}

async function loadJsonObject(path) {
  const content = await readFile(path, 'utf8');
  const value = JSON.parse(content);
  if (!isObject(value)) throw new Error('JSON receipt must be an object');
  return value;
}

function loadedFromValues(values, idField = 'case_id') {
  const duplicateIds = new Set();
  const seenIds = new Set();
  const rows = values.map((value, index) => {
    const id = value?.[idField];
    if (typeof id === 'string' && id) {
      if (seenIds.has(id)) duplicateIds.add(id);
      seenIds.add(id);
    }
    return { line: index + 1, value };
  });
  return {
    rows,
    invalidRows: [],
    duplicateIds: [...duplicateIds].sort(),
    canonicalSha256: sha256(stableStringify(rows.map((row) => row.value))),
  };
}

function validateFixtures(loaded) {
  const issues = loaded.invalidRows.map((item) => ({
    line: item.line,
    code: item.error,
  }));
  const cases = [];
  const ids = new Set();
  const categoryCounts = {};
  const safePromptForbidden = [
    /[A-Za-z]:\\/u,
    /\/(?:srv|home|etc)\//u,
    /https?:\/\//iu,
    /[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}/u,
    /(?:\d{1,3}\.){3}\d{1,3}/u,
  ];
  for (const { line, value } of loaded.rows) {
    const caseIssues = [];
    const id = value.id;
    if (typeof id !== 'string' || !id) caseIssues.push('missing_id');
    if (typeof id === 'string' && ids.has(id)) caseIssues.push('duplicate_id');
    if (typeof id === 'string') ids.add(id);
    if (value.synthetic !== true) caseIssues.push('synthetic_flag_required');
    if (typeof value.category !== 'string' || !(value.category in requiredCategoryMinimums)) {
      caseIssues.push('unknown_category');
    }
    if (typeof value.dataset_key !== 'string' || !value.dataset_key.startsWith('synthetic-')) {
      caseIssues.push('synthetic_dataset_key_required');
    }
    const selectedDatasetKeys = value.selected_scope?.selected_dataset_keys;
    if (!isObject(value.selected_scope)
      || stableStringify(Object.keys(value.selected_scope).sort())
        !== stableStringify(['selected_dataset_keys'])
      || !Array.isArray(selectedDatasetKeys)
      || selectedDatasetKeys.some((datasetKey) => (
        typeof datasetKey !== 'string' || !datasetKey.startsWith('synthetic-')
      ))
      || new Set(selectedDatasetKeys || []).size !== selectedDatasetKeys?.length) {
      caseIssues.push('selected_scope_dataset_keys_invalid');
    } else if (value.category === 'multi_dataset_selected_scope') {
      if (selectedDatasetKeys.length < 2) {
        caseIssues.push('multi_dataset_selected_scope_requires_two_datasets');
      }
    } else if (selectedDatasetKeys.length !== 1 || selectedDatasetKeys[0] !== value.dataset_key) {
      caseIssues.push('single_dataset_scope_must_match_dataset_key');
    }
    if (stableStringify(value.allowed_evidence_classes) !== stableStringify(['confirmed', 'observed'])) {
      caseIssues.push('allowed_evidence_classes_must_be_confirmed_observed');
    }
    if (typeof value.prompt !== 'string' || !value.prompt.trim()) caseIssues.push('missing_prompt');
    if (typeof value.prompt === 'string' && safePromptForbidden.some((pattern) => pattern.test(value.prompt))) {
      caseIssues.push('prompt_contains_non_synthetic_locator');
    }
    if (typeof value.graph_target !== 'boolean') caseIssues.push('graph_target_boolean_required');
    if (!['rerank', 'supplement', 'none'].includes(value.expected_graph_behavior)) {
      caseIssues.push('invalid_expected_graph_behavior');
    }
    if (value.category === 'negative_control' && (value.graph_target !== false || value.expected_graph_behavior !== 'none')) {
      caseIssues.push('negative_control_must_be_no_effect');
    }
    if (!Array.isArray(value.expected_sources) || value.expected_sources.length === 0
      || value.expected_sources.some((source) => typeof source !== 'string' || !source.startsWith('synthetic://visible/'))) {
      caseIssues.push('synthetic_expected_sources_required');
    }
    if (!Array.isArray(value.forbidden_sources) || value.forbidden_sources.length === 0
      || value.forbidden_sources.some((source) => typeof source !== 'string' || !source.startsWith('synthetic://hidden/'))) {
      caseIssues.push('synthetic_forbidden_sources_required');
    }
    if (typeof value.overlap_group !== 'string' || !value.overlap_group) caseIssues.push('overlap_group_required');
    for (const forbiddenField of [
      'forbidden_answer_patterns',
      'expected_route',
      'expected_artifact_count',
      'expected_provider_call_count',
      'max_provider_retry_count',
    ]) {
      if (Object.hasOwn(value, forbiddenField)) {
        caseIssues.push(`dialogue_route_or_side_effect_oracle_forbidden:${forbiddenField}`);
      }
    }
    if (caseIssues.length > 0) {
      for (const code of caseIssues) issues.push({ line, case_id: typeof id === 'string' ? id : null, code });
      continue;
    }
    cases.push(value);
    categoryCounts[value.category] = (categoryCounts[value.category] || 0) + 1;
  }
  for (const id of loaded.duplicateIds) {
    if (!issues.some((item) => item.case_id === id && item.code === 'duplicate_id')) {
      issues.push({ line: null, case_id: id, code: 'duplicate_id' });
    }
  }
  if (cases.length < 24) issues.push({ line: null, case_id: null, code: 'minimum_24_cases_required' });
  for (const [category, minimum] of Object.entries(requiredCategoryMinimums)) {
    if ((categoryCounts[category] || 0) < minimum) {
      issues.push({ line: null, case_id: null, code: `category_minimum_not_met:${category}:${minimum}` });
    }
    const categoryCases = cases.filter((fixture) => fixture.category === category);
    if (!categoryCases.some((fixture) => fixtureSplit(fixture) === 'holdout')) {
      issues.push({ line: null, case_id: null, code: `holdout_missing:${category}` });
    }
    if (!categoryCases.some((fixture) => fixtureSplit(fixture) === 'development')) {
      issues.push({ line: null, case_id: null, code: `development_split_missing:${category}` });
    }
  }
  return {
    ready: issues.length === 0,
    case_count: cases.length,
    graph_target_case_count: cases.filter((item) => item.graph_target).length,
    negative_control_case_count: cases.filter((item) => !item.graph_target).length,
    categories: categoryCounts,
    splits: {
      development: cases.filter((fixture) => fixtureSplit(fixture) === 'development').length,
      holdout: cases.filter((fixture) => fixtureSplit(fixture) === 'holdout').length,
    },
    issues,
    cases,
  };
}

function exactObjectKeys(value, expectedKeys) {
  return isObject(value)
    && stableStringify(Object.keys(value).sort()) === stableStringify([...expectedKeys].sort());
}

function forbiddenRuntimeInputFields(value, path = '$', matches = []) {
  if (Array.isArray(value)) {
    value.forEach((item, index) => forbiddenRuntimeInputFields(item, `${path}[${index}]`, matches));
    return matches;
  }
  if (!isObject(value)) return matches;
  const forbiddenExact = new Set([
    'action',
    'action_catalog',
    'answer',
    'answer_template',
    'assistant',
    'artifact',
    'conversation',
    'dialogue',
    'graph_target',
    'intent',
    'messages',
    'model',
    'prompt',
    'provider',
    'response',
    'route',
    'split',
    'system_prompt',
    'tool',
    'workflow',
  ]);
  for (const [key, child] of Object.entries(value)) {
    if (forbiddenExact.has(key)
      || key.startsWith('answer_')
      || key.startsWith('artifact_')
      || key.startsWith('expected_')
      || key.startsWith('forbidden_')
      || key.startsWith('intent_')
      || key.startsWith('route_')) {
      matches.push(`${path}.${key}`);
    }
    forbiddenRuntimeInputFields(child, `${path}.${key}`, matches);
  }
  return matches;
}

function validateRuntimeInputs(loaded, fixtures, sourcePath = null) {
  const issues = loaded.invalidRows.map((item) => ({ line: item.line, code: item.error }));
  const datasets = new Map();
  const cases = new Map();
  const allowedSupplyKinds = new Set([
    'document',
    'structured_query',
    'structured_row_scan',
    'parse_status',
    'ordinary_knowledge',
  ]);
  const allowedEvaluationKinds = new Set([
    'retrieval_evidence',
    'structured_query',
    'structured_row_scan',
    'parse_status',
    'ordinary_chat',
  ]);
  const allowedSemanticRoles = new Set([
    'field',
    'relation_key',
    'document_topic',
    'guard_evidence',
  ]);
  for (const { line, value } of loaded.rows) {
    const rowIssues = [];
    if (value.schema_version !== runtimeInputSchemaVersion) rowIssues.push('schema_version_invalid');
    const forbiddenFields = forbiddenRuntimeInputFields(value);
    if (forbiddenFields.length > 0) rowIssues.push('oracle_or_conversation_field_forbidden');
    if (value.record_kind === 'dataset') {
      if (!exactObjectKeys(value, [
        'schema_version',
        'record_kind',
        'dataset_key',
        'candidate_pool',
        'semantic_snapshot',
      ])) {
        rowIssues.push('dataset_record_fields_invalid');
      }
      if (typeof value.dataset_key !== 'string' || !value.dataset_key.startsWith('synthetic-')) {
        rowIssues.push('dataset_key_invalid');
      }
      if (!Array.isArray(value.candidate_pool) || value.candidate_pool.length === 0) {
        rowIssues.push('candidate_pool_required');
      }
      const candidateIds = new Set();
      const sourceIds = new Set();
      for (const [index, candidate] of (value.candidate_pool || []).entries()) {
        if (!exactObjectKeys(candidate, [
          'candidate_id',
          'source_id',
          'supply_kind',
          'document_id',
        ])) {
          rowIssues.push(`candidate_fields_invalid:${index}`);
          continue;
        }
        if (typeof candidate.candidate_id !== 'string' || !candidate.candidate_id) {
          rowIssues.push(`candidate_id_invalid:${index}`);
        } else if (candidateIds.has(candidate.candidate_id)) {
          rowIssues.push(`candidate_id_duplicate:${index}`);
        }
        candidateIds.add(candidate.candidate_id);
        if (typeof candidate.source_id !== 'string'
          || !candidate.source_id.startsWith('synthetic://visible/')) {
          rowIssues.push(`candidate_source_invalid:${index}`);
        }
        sourceIds.add(candidate.source_id);
        if (!allowedSupplyKinds.has(candidate.supply_kind)) {
          rowIssues.push(`candidate_supply_kind_invalid:${index}`);
        }
        if (candidate.supply_kind === 'document') {
          if (typeof candidate.document_id !== 'string' || !candidate.document_id) {
            rowIssues.push(`candidate_document_id_required:${index}`);
          }
        } else if (candidate.document_id !== null) {
          rowIssues.push(`nondocument_candidate_document_id_must_be_null:${index}`);
        }
      }
      const snapshot = value.semantic_snapshot;
      if (!exactObjectKeys(snapshot, [
        'snapshot_key',
        'version',
        'status',
        'stale',
        'compatible',
        'source_fingerprint_sha256',
        'nodes',
      ])) {
        rowIssues.push('semantic_snapshot_fields_invalid');
      } else {
        if (typeof snapshot.snapshot_key !== 'string' || !snapshot.snapshot_key) {
          rowIssues.push('semantic_snapshot_key_invalid');
        }
        if (!Number.isInteger(snapshot.version) || snapshot.version < 1) {
          rowIssues.push('semantic_snapshot_version_invalid');
        }
        if (snapshot.status !== 'ready' || snapshot.stale !== false || snapshot.compatible !== true) {
          rowIssues.push('semantic_snapshot_not_ready_compatible');
        }
        if (!isSha256(snapshot.source_fingerprint_sha256)) {
          rowIssues.push('semantic_source_fingerprint_invalid');
        }
        if (!Array.isArray(snapshot.nodes)) {
          rowIssues.push('semantic_nodes_array_required');
        }
        const nodeIds = new Set();
        for (const [index, node] of (snapshot.nodes || []).entries()) {
          if (!exactObjectKeys(node, [
            'node_id',
            'canonical_label',
            'technical_names',
            'semantic_role',
            'evidence_class',
            'source_ids',
          ])) {
            rowIssues.push(`semantic_node_fields_invalid:${index}`);
            continue;
          }
          if (typeof node.node_id !== 'string' || !node.node_id || nodeIds.has(node.node_id)) {
            rowIssues.push(`semantic_node_id_invalid:${index}`);
          }
          nodeIds.add(node.node_id);
          if (typeof node.canonical_label !== 'string' || !node.canonical_label.trim()) {
            rowIssues.push(`semantic_node_label_invalid:${index}`);
          }
          if (!Array.isArray(node.technical_names)
            || node.technical_names.some((name) => typeof name !== 'string' || !name)) {
            rowIssues.push(`semantic_node_technical_names_invalid:${index}`);
          }
          if (!allowedSemanticRoles.has(node.semantic_role)) {
            rowIssues.push(`semantic_node_role_invalid:${index}`);
          }
          if (!['confirmed', 'observed'].includes(node.evidence_class)) {
            rowIssues.push(`semantic_node_evidence_class_invalid:${index}`);
          }
          if (!Array.isArray(node.source_ids)
            || node.source_ids.length === 0
            || node.source_ids.some((sourceId) => !sourceIds.has(sourceId))) {
            rowIssues.push(`semantic_node_source_ids_invalid:${index}`);
          }
        }
      }
      if (rowIssues.length === 0) {
        if (datasets.has(value.dataset_key)) rowIssues.push('dataset_record_duplicate');
        else datasets.set(value.dataset_key, value);
      }
    } else if (value.record_kind === 'case') {
      if (!exactObjectKeys(value, [
        'schema_version',
        'record_kind',
        'case_id',
        'question',
        'selected_scope',
        'permission',
        'evaluation_kind',
      ])) {
        rowIssues.push('case_record_fields_invalid');
      }
      if (typeof value.case_id !== 'string' || !value.case_id) rowIssues.push('case_id_invalid');
      if (typeof value.question !== 'string' || !value.question.trim()) rowIssues.push('question_invalid');
      if (!exactObjectKeys(value.selected_scope, ['selected_dataset_keys'])
        || !Array.isArray(value.selected_scope?.selected_dataset_keys)
        || value.selected_scope.selected_dataset_keys.length === 0
        || value.selected_scope.selected_dataset_keys.some((datasetKey) => (
          typeof datasetKey !== 'string' || !datasetKey.startsWith('synthetic-')
        ))) {
        rowIssues.push('selected_scope_invalid');
      }
      if (!exactObjectKeys(value.permission, [
        'tenant_key',
        'visible_dataset_keys',
        'allowed_evidence_classes',
      ])) {
        rowIssues.push('permission_fields_invalid');
      } else {
        if (typeof value.permission.tenant_key !== 'string'
          || !value.permission.tenant_key.startsWith('synthetic-')) {
          rowIssues.push('permission_tenant_invalid');
        }
        if (stableStringify(value.permission.visible_dataset_keys)
          !== stableStringify(value.selected_scope?.selected_dataset_keys)) {
          rowIssues.push('permission_visible_scope_mismatch');
        }
        if (stableStringify(value.permission.allowed_evidence_classes)
          !== stableStringify(['confirmed', 'observed'])) {
          rowIssues.push('permission_evidence_classes_invalid');
        }
      }
      if (!allowedEvaluationKinds.has(value.evaluation_kind)) {
        rowIssues.push('evaluation_kind_invalid');
      }
      if (rowIssues.length === 0) {
        if (cases.has(value.case_id)) rowIssues.push('case_record_duplicate');
        else cases.set(value.case_id, value);
      }
    } else {
      rowIssues.push('record_kind_invalid');
    }
    for (const code of [...new Set(rowIssues)]) issues.push({ line, code });
  }

  const fixtureById = new Map(fixtures.map((fixture) => [fixture.id, fixture]));
  const missingCaseIds = [...fixtureById.keys()].filter((caseId) => !cases.has(caseId)).sort();
  const extraCaseIds = [...cases.keys()].filter((caseId) => !fixtureById.has(caseId)).sort();
  for (const caseId of missingCaseIds) issues.push({ case_id: caseId, code: 'runtime_case_missing' });
  for (const caseId of extraCaseIds) issues.push({ case_id: caseId, code: 'runtime_case_extra' });
  const caseReceipts = new Map();
  for (const fixture of fixtures) {
    const runtimeCase = cases.get(fixture.id);
    if (!runtimeCase) continue;
    if (runtimeCase.question !== fixture.prompt) {
      issues.push({ case_id: fixture.id, code: 'runtime_question_fixture_mismatch' });
    }
    if (stableStringify(runtimeCase.selected_scope) !== stableStringify(fixture.selected_scope)) {
      issues.push({ case_id: fixture.id, code: 'runtime_scope_fixture_mismatch' });
    }
    if (stableStringify(runtimeCase.permission.allowed_evidence_classes)
      !== stableStringify(fixture.allowed_evidence_classes)) {
      issues.push({ case_id: fixture.id, code: 'runtime_permission_fixture_mismatch' });
    }
    const selectedDatasets = [];
    for (const datasetKey of runtimeCase.selected_scope.selected_dataset_keys) {
      const dataset = datasets.get(datasetKey);
      if (!dataset) issues.push({ case_id: fixture.id, code: 'runtime_selected_dataset_missing' });
      else selectedDatasets.push(dataset);
    }
    const candidates = selectedDatasets.flatMap((dataset) => dataset.candidate_pool.map((candidate) => ({
      dataset_key: dataset.dataset_key,
      ...candidate,
    })));
    if (new Set(candidates.map((candidate) => candidate.candidate_id)).size !== candidates.length) {
      issues.push({ case_id: fixture.id, code: 'runtime_candidate_id_collision' });
    }
    const semanticSnapshots = selectedDatasets.map((dataset) => ({
      dataset_key: dataset.dataset_key,
      semantic_snapshot: dataset.semantic_snapshot,
    }));
    const questionSha256 = sha256(runtimeCase.question);
    const scopeSha256 = sha256(stableStringify(runtimeCase.selected_scope));
    const permissionSha256 = sha256(stableStringify(runtimeCase.permission));
    const candidateSnapshotSha256 = sha256(stableStringify(candidates));
    const semanticSnapshotSha256 = sha256(stableStringify(semanticSnapshots));
    const evaluationKindSha256 = sha256(runtimeCase.evaluation_kind);
    const runtimeInputMaterial = {
      schema_version: runtimeInputSchemaVersion,
      case_id: fixture.id,
      question_sha256: questionSha256,
      scope_sha256: scopeSha256,
      permission_sha256: permissionSha256,
      evaluation_kind: runtimeCase.evaluation_kind,
      evaluation_kind_sha256: evaluationKindSha256,
      candidate_snapshot_sha256: candidateSnapshotSha256,
      semantic_snapshot_sha256: semanticSnapshotSha256,
    };
    const sourceToCandidate = new Map();
    for (const candidate of candidates) {
      if (!sourceToCandidate.has(candidate.source_id)) {
        sourceToCandidate.set(candidate.source_id, candidate);
      }
    }
    caseReceipts.set(fixture.id, {
      case_id: fixture.id,
      evaluation_kind: runtimeCase.evaluation_kind,
      question_sha256: questionSha256,
      scope_sha256: scopeSha256,
      permission_sha256: permissionSha256,
      evaluation_kind_sha256: evaluationKindSha256,
      candidate_snapshot_sha256: candidateSnapshotSha256,
      semantic_snapshot_sha256: semanticSnapshotSha256,
      runtime_input_sha256: sha256(stableStringify(runtimeInputMaterial)),
      candidate_ids: candidates.map((candidate) => candidate.candidate_id),
      candidate_count: candidates.length,
      selected_dataset_count: selectedDatasets.length,
      _candidates: candidates,
      _source_to_candidate: sourceToCandidate,
      _runtime_case: runtimeCase,
    });
  }
  return {
    ready: issues.length === 0 && caseReceipts.size === fixtures.length,
    source_path: safeRelativePath(sourcePath),
    schema_version: runtimeInputSchemaVersion,
    dataset_record_count: datasets.size,
    case_record_count: cases.size,
    runtime_input_file_sha256:
      loaded.contentSha256 || sha256(stableStringify(loaded.rows.map((row) => row.value))),
    runtime_input_canonical_sha256:
      loaded.canonicalSha256 || sha256(stableStringify(loaded.rows.map((row) => row.value))),
    missing_case_ids: missingCaseIds,
    extra_case_ids: extraCaseIds,
    issues,
    case_receipts: caseReceipts,
  };
}

function safeRuntimeInputReport(runtimeInputs) {
  const { case_receipts: receipts, ...safe } = runtimeInputs;
  return {
    ...safe,
    case_receipts: [...receipts.values()].map((receipt) => ({
      case_id: receipt.case_id,
      evaluation_kind: receipt.evaluation_kind,
      question_sha256: receipt.question_sha256,
      scope_sha256: receipt.scope_sha256,
      permission_sha256: receipt.permission_sha256,
      candidate_snapshot_sha256: receipt.candidate_snapshot_sha256,
      semantic_snapshot_sha256: receipt.semantic_snapshot_sha256,
      runtime_input_sha256: receipt.runtime_input_sha256,
      candidate_count: receipt.candidate_count,
      selected_dataset_count: receipt.selected_dataset_count,
    })),
  };
}

function validateResultRow(
  value,
  expectedArm = null,
  runtimeInputFileSha256 = null,
  runtimeInputCanonicalSha256 = null,
) {
  const errors = [];
  if (value.schema_version !== resultSchemaVersion) errors.push('schema_version_invalid');
  if (typeof value.case_id !== 'string' || !value.case_id) errors.push('missing_case_id');
  if (typeof value.experiment_id !== 'string' || !value.experiment_id) errors.push('experiment_id_required');
  if (!['A', 'B', 'C'].includes(value.arm)) errors.push('arm_invalid');
  if (expectedArm && value.arm !== expectedArm) errors.push(`arm_mismatch:${expectedArm}`);
  if (value.execution_kind !== rustOfflineExecutionKind) errors.push('execution_kind_invalid');
  for (const field of [
    'fixture_sha256',
    'candidate_snapshot_sha256',
    'semantic_snapshot_sha256',
    'scope_sha256',
    'permission_sha256',
    'runtime_input_sha256',
    'grounding_sha256',
  ]) {
    if (!isSha256(value[field])) errors.push(`${field}_invalid`);
  }
  if (typeof value.evaluation_kind !== 'string' || !value.evaluation_kind) {
    errors.push('evaluation_kind_invalid');
  }
  if (!isObject(value.producer)
    || value.producer.kind !== 'platform-api-rust-fixture-export'
    || typeof value.producer.exporter_version !== 'string'
    || !value.producer.exporter_version
    || typeof value.producer.git_head !== 'string'
    || !/^[a-f0-9]{40}$/u.test(value.producer.git_head)
    || value.producer.git_head_source !== 'git_rev_parse_head'
    || value.producer.worktree_clean !== true) {
    errors.push('producer_receipt_invalid');
  }
  if (!isSha256(value.producer?.runtime_input_file_sha256)) {
    errors.push('producer_runtime_input_file_sha256_invalid');
  } else if (runtimeInputFileSha256
    && value.producer.runtime_input_file_sha256 !== runtimeInputFileSha256) {
    errors.push('producer_runtime_input_file_sha256_mismatch');
  }
  if (!isSha256(value.producer?.runtime_input_canonical_sha256)) {
    errors.push('producer_runtime_input_canonical_sha256_invalid');
  } else if (runtimeInputCanonicalSha256
    && value.producer.runtime_input_canonical_sha256 !== runtimeInputCanonicalSha256) {
    errors.push('producer_runtime_input_canonical_sha256_mismatch');
  }
  if (!Number.isFinite(value.latency_ms) || value.latency_ms < 0) errors.push('latency_ms_invalid');
  if (!Array.isArray(value.candidate_ids)
    || value.candidate_ids.length === 0
    || value.candidate_ids.some((candidateId) => typeof candidateId !== 'string' || !candidateId)) {
    errors.push('candidate_ids_invalid');
  } else if (new Set(value.candidate_ids).size !== value.candidate_ids.length) {
    errors.push('candidate_ids_duplicate');
  }
  if (!nonNegativeInteger(value.input_candidate_count)
    || value.input_candidate_count !== value.candidate_ids?.length) {
    errors.push('input_candidate_count_invalid');
  }
  if (value.answer !== undefined) errors.push('offline_answer_must_be_absent');
  if (!isObject(value.answer_evaluation)
    || value.answer_evaluation.status !== 'not_run'
    || typeof value.answer_evaluation.reason !== 'string'
    || !value.answer_evaluation.reason) {
    errors.push('answer_evaluation_not_run_required');
  }
  if (!Array.isArray(value.hits)) {
    errors.push('hits_array_required');
  } else {
    const ranks = new Set();
    const sources = new Set();
    value.hits.forEach((hit, index) => {
      if (!isObject(hit) || typeof hit.source_id !== 'string' || !hit.source_id
        || typeof hit.candidate_id !== 'string' || !hit.candidate_id) {
        errors.push(`hit_source_id_invalid:${index}`);
        return;
      }
      if (!value.candidate_ids?.includes(hit.candidate_id)) {
        errors.push(`hit_not_in_candidate_snapshot:${index}`);
      }
      if (!['baseline', 'reranked', 'supplement'].includes(hit.selection_kind)) {
        errors.push(`hit_selection_kind_invalid:${index}`);
      }
      if (hit.selection_kind === 'supplement') {
        for (const field of [
          'retrieval_evidence_id',
          'document_id',
          'document_chunk_id',
          'source_locator',
        ]) {
          if (typeof hit[field] !== 'string' || !hit[field].trim()) {
            errors.push(`supplement_${field}_invalid:${index}`);
          }
        }
      }
      if (sources.has(hit.source_id)) errors.push(`duplicate_hit_source:${index}`);
      sources.add(hit.source_id);
      if (hit.rank !== undefined && (!Number.isInteger(hit.rank) || hit.rank < 1)) {
        errors.push(`hit_rank_invalid:${index}`);
      } else if (hit.rank !== undefined) {
        if (ranks.has(hit.rank)) errors.push(`duplicate_hit_rank:${index}`);
        ranks.add(hit.rank);
      }
    });
  }
  if (!Array.isArray(value.citations) || value.citations.length !== 0) {
    errors.push('offline_citations_must_be_empty');
  }
  if (!isObject(value.route)) errors.push('route_object_required');
  if (!Array.isArray(value.artifacts)) errors.push('artifacts_array_required');
  if (!isObject(value.contract)) {
    errors.push('contract_receipt_required');
  } else {
    for (const field of [
      'question_sha256',
      'system_prompt_sha256',
      'answer_policy_sha256',
      'provider_config_sha256',
      'selected_scope_sha256',
      'permission_sha256',
      'evaluation_kind_sha256',
      'runtime_input_sha256',
      'action_catalog_sha256',
      'route_sha256',
      'immutable_component_sha256',
      'model_visible_evidence_sha256',
      'provider_input_sha256',
    ]) {
      if (!isSha256(value.contract[field])) errors.push(`contract_${field}_invalid`);
    }
    const immutableComponents = {
      question_sha256: value.contract.question_sha256,
      system_prompt_sha256: value.contract.system_prompt_sha256,
      answer_policy_sha256: value.contract.answer_policy_sha256,
      provider_config_sha256: value.contract.provider_config_sha256,
      selected_scope_sha256: value.contract.selected_scope_sha256,
      permission_sha256: value.contract.permission_sha256,
      evaluation_kind_sha256: value.contract.evaluation_kind_sha256,
      runtime_input_sha256: value.contract.runtime_input_sha256,
      action_catalog_sha256: value.contract.action_catalog_sha256,
      route_sha256: value.contract.route_sha256,
    };
    if (value.contract.immutable_component_sha256 !== sha256(stableStringify(immutableComponents))) {
      errors.push('contract_immutable_component_hash_mismatch');
    }
    if (isObject(value.route)
      && value.contract.route_sha256 !== sha256(stableStringify(value.route))) {
      errors.push('contract_route_hash_mismatch');
    }
    if (value.scope_sha256 !== value.contract.selected_scope_sha256) {
      errors.push('scope_contract_hash_mismatch');
    }
    if (value.permission_sha256 !== value.contract.permission_sha256) {
      errors.push('permission_contract_hash_mismatch');
    }
    if (value.runtime_input_sha256 !== value.contract.runtime_input_sha256) {
      errors.push('runtime_input_contract_hash_mismatch');
    }
    if (typeof value.evaluation_kind === 'string'
      && value.contract.evaluation_kind_sha256 !== sha256(value.evaluation_kind)) {
      errors.push('evaluation_kind_contract_hash_mismatch');
    }
    if (!nonNegativeInteger(value.contract.provider_input_bytes)) {
      errors.push('contract_provider_input_bytes_invalid');
    }
    for (const field of [
      'provider_call_count',
      'provider_retry_count',
      'workflow_delta_count',
      'artifact_delta_count',
      'report_delta_count',
      'template_delta_count',
      'static_page_delta_count',
    ]) {
      if (!nonNegativeInteger(value.contract[field])) errors.push(`contract_${field}_invalid`);
    }
    if (typeof value.contract.same_run_off_path_self_consistency !== 'boolean') {
      errors.push('contract_same_run_off_path_self_consistency_boolean_required');
    }
  }
  if (!isObject(value.grounding)) {
    errors.push('grounding_receipt_required');
  } else {
    if (!['measured', 'not_applicable'].includes(value.grounding.status)) {
      errors.push('grounding_status_invalid');
    }
    for (const field of [
      'target_node_count',
      'resolved_target_node_count',
      'confirmed_or_observed_used_count',
      'inferred_used_count',
      'unresolved_used_count',
      'stale_used_count',
      'hidden_source_use_count',
      'unresolvable_source_use_count',
    ]) {
      if (!nonNegativeInteger(value.grounding[field])) errors.push(`grounding_${field}_invalid`);
    }
    if (nonNegativeInteger(value.grounding.target_node_count)
      && nonNegativeInteger(value.grounding.resolved_target_node_count)
      && value.grounding.resolved_target_node_count > value.grounding.target_node_count) {
      errors.push('grounding_resolved_exceeds_target');
    }
    if (value.grounding.status === 'measured'
      && (value.grounding.snapshot_status !== 'ready'
        || value.grounding.snapshot_stale !== false
        || value.grounding.snapshot_compatible !== true)) {
      errors.push('grounding_snapshot_not_ready_compatible');
    }
    if (isSha256(value.grounding_sha256)
      && value.grounding_sha256 !== sha256(stableStringify(value.grounding))) {
      errors.push('grounding_hash_mismatch');
    }
  }
  if (value.permission_leaks !== undefined && !Array.isArray(value.permission_leaks)) {
    errors.push('permission_leaks_array_invalid');
  }
  return [...new Set(errors)].sort();
}

function sourceMatches(source, pattern) {
  if (typeof source !== 'string' || typeof pattern !== 'string') return false;
  return pattern.endsWith('*') ? source.startsWith(pattern.slice(0, -1)) : source === pattern;
}

function orderedSourceIds(result) {
  return result.hits
    .map((hit, index) => ({ source: hit.source_id, rank: hit.rank ?? index + 1, index }))
    .sort((left, right) => left.rank - right.rank || left.index - right.index)
    .map((item) => item.source);
}

function top5SourceOverlap(cases, resultMap) {
  const comparable = cases
    .filter((fixture) => resultMap.has(fixture.id))
    .map((fixture) => ({
      group: fixture.overlap_group,
      sources: new Set(orderedSourceIds(resultMap.get(fixture.id)).slice(0, 5)),
    }));
  const scores = [];
  for (let left = 0; left < comparable.length; left += 1) {
    for (let right = left + 1; right < comparable.length; right += 1) {
      if (comparable[left].group === comparable[right].group) continue;
      const union = new Set([...comparable[left].sources, ...comparable[right].sources]);
      if (union.size === 0) {
        scores.push(0);
        continue;
      }
      let intersection = 0;
      for (const source of comparable[left].sources) {
        if (comparable[right].sources.has(source)) intersection += 1;
      }
      scores.push(intersection / union.size);
    }
  }
  return { value: round(mean(scores)), pair_count: scores.length };
}

function metricSummary(caseMetrics) {
  return {
    case_count: caseMetrics.length,
    recall_at_20: round(mean(caseMetrics.map((item) => item.recall_at_20))),
    mrr_at_20: round(mean(caseMetrics.map((item) => item.mrr_at_20))),
    precision_at_5: round(mean(caseMetrics.map((item) => item.precision_at_5))),
  };
}

function metricSliceSummary(fixtures, caseMetrics) {
  const byCaseId = new Map(caseMetrics.map((item) => [item.case_id, item]));
  const summarize = (selectedFixtures) => {
    const selected = selectedFixtures.map((fixture) => byCaseId.get(fixture.id)).filter(Boolean);
    return {
      metrics: metricSummary(selected),
      targeted_metrics: metricSummary(selected.filter((item) => item.graph_target)),
      negative_control_metrics: metricSummary(selected.filter((item) => !item.graph_target)),
    };
  };
  return {
    by_category: Object.fromEntries(
      Object.keys(requiredCategoryMinimums).sort().map((category) => [
        category,
        summarize(fixtures.filter((fixture) => fixture.category === category)),
      ]),
    ),
    by_split: {
      development: summarize(fixtures.filter((fixture) => fixtureSplit(fixture) === 'development')),
      holdout: summarize(fixtures.filter((fixture) => fixtureSplit(fixture) === 'holdout')),
    },
  };
}

function groundingSummary(fixtures, results) {
  const targetCaseIds = fixtures.filter((fixture) => fixture.graph_target).map((fixture) => fixture.id);
  let targetNodeCount = 0;
  let resolvedTargetNodeCount = 0;
  const measuredTargetCaseIds = new Set();
  let unsafeNodeUseCount = 0;
  const issues = [];
  const fixtureById = new Map(fixtures.map((fixture) => [fixture.id, fixture]));
  for (const result of results) {
    const fixture = fixtureById.get(result.case_id);
    if (!fixture) continue;
    const grounding = result.grounding;
    if (fixture.graph_target) {
      if (grounding.status === 'measured' && grounding.target_node_count >= 1) {
        measuredTargetCaseIds.add(fixture.id);
      }
      targetNodeCount += grounding.target_node_count;
      resolvedTargetNodeCount += grounding.resolved_target_node_count;
    } else if (grounding.status !== 'not_applicable'
      || grounding.target_node_count !== 0
      || grounding.resolved_target_node_count !== 0) {
      issues.push({ case_id: fixture.id, code: 'negative_control_grounding_must_be_not_applicable' });
    }
    unsafeNodeUseCount += grounding.inferred_used_count
      + grounding.unresolved_used_count
      + grounding.stale_used_count
      + grounding.hidden_source_use_count
      + grounding.unresolvable_source_use_count;
  }
  const provenanceResolutionRate = targetNodeCount > 0
    ? resolvedTargetNodeCount / targetNodeCount
    : 0;
  const targetCaseCoverageRate = targetCaseIds.length > 0
    ? measuredTargetCaseIds.size / targetCaseIds.length
    : 0;
  const unmeasuredTargetCaseIds = targetCaseIds.filter((caseId) => !measuredTargetCaseIds.has(caseId));
  return {
    target_case_count: targetCaseIds.length,
    measured_target_case_count: measuredTargetCaseIds.size,
    target_case_coverage_rate: round(targetCaseCoverageRate),
    target_case_coverage_threshold: 0.9,
    unmeasured_target_case_ids: unmeasuredTargetCaseIds,
    target_node_count: targetNodeCount,
    resolved_target_node_count: resolvedTargetNodeCount,
    target_node_provenance_resolution_rate: round(provenanceResolutionRate),
    unsafe_node_use_count: unsafeNodeUseCount,
    issues,
    ready: targetCaseCoverageRate >= 0.9
      && provenanceResolutionRate >= 0.9
      && unsafeNodeUseCount === 0
      && issues.length === 0,
  };
}

function evaluateArm(
  name,
  fixtures,
  loaded,
  sourcePath = null,
  expectedArm = null,
  runtimeInputs = null,
) {
  const fixtureById = new Map(fixtures.map((item) => [item.id, item]));
  const resultMap = new Map();
  const invalidRows = loaded.invalidRows.map((item) => ({ line: item.line, errors: [item.error] }));
  for (const { line, value } of loaded.rows) {
    const errors = validateResultRow(
      value,
      expectedArm,
      runtimeInputs?.runtime_input_file_sha256 || null,
      runtimeInputs?.runtime_input_canonical_sha256 || null,
    );
    if (errors.length > 0) {
      invalidRows.push({ line, case_id: typeof value.case_id === 'string' ? value.case_id : null, errors });
      continue;
    }
    if (!resultMap.has(value.case_id)) resultMap.set(value.case_id, value);
  }
  const expectedIds = new Set(fixtures.map((item) => item.id));
  const missingCaseIds = [...expectedIds].filter((id) => !resultMap.has(id)).sort();
  const extraCaseIds = [...resultMap.keys()].filter((id) => !expectedIds.has(id)).sort();
  const duplicateCaseIds = loaded.duplicateIds.filter((id) => expectedIds.has(id)).sort();
  const permissionLeaks = [];
  const caseMetrics = [];
  let rustOfflineRetrievalCount = 0;
  let offlineRouteNotRunCount = 0;
  let offlineArtifactZeroCount = 0;
  let offlineProviderZeroCount = 0;
  let sideEffectZeroCount = 0;
  let sameRunOffPathSelfConsistencyCount = 0;
  const armContractIssues = [];
  const evaluatedResults = [];
  for (const fixture of fixtures) {
    const result = resultMap.get(fixture.id);
    if (!result || invalidRows.some((item) => item.case_id === fixture.id)) continue;
    evaluatedResults.push(result);
    const runtimeReceipt = runtimeInputs?.case_receipts.get(fixture.id);
    if (!runtimeReceipt) {
      armContractIssues.push({ case_id: fixture.id, code: 'runtime_input_receipt_missing' });
    } else {
      const runtimeHashFields = [
        ['runtime_input_sha256', 'runtime_input_sha256'],
        ['candidate_snapshot_sha256', 'candidate_snapshot_sha256'],
        ['semantic_snapshot_sha256', 'semantic_snapshot_sha256'],
        ['scope_sha256', 'scope_sha256'],
        ['permission_sha256', 'permission_sha256'],
      ];
      for (const [resultField, receiptField] of runtimeHashFields) {
        if (result[resultField] !== runtimeReceipt[receiptField]) {
          armContractIssues.push({
            case_id: fixture.id,
            code: `runtime_input_${resultField}_mismatch`,
          });
        }
      }
      if (result.evaluation_kind !== runtimeReceipt.evaluation_kind) {
        armContractIssues.push({ case_id: fixture.id, code: 'runtime_input_evaluation_kind_mismatch' });
      }
      if (result.contract.question_sha256 !== runtimeReceipt.question_sha256) {
        armContractIssues.push({ case_id: fixture.id, code: 'runtime_input_question_hash_mismatch' });
      }
      if (result.contract.permission_sha256 !== runtimeReceipt.permission_sha256
        || result.contract.evaluation_kind_sha256 !== runtimeReceipt.evaluation_kind_sha256
        || result.contract.runtime_input_sha256 !== runtimeReceipt.runtime_input_sha256) {
        armContractIssues.push({ case_id: fixture.id, code: 'runtime_input_contract_hash_mismatch' });
      }
      if (stableStringify(result.candidate_ids) !== stableStringify(runtimeReceipt.candidate_ids)) {
        armContractIssues.push({ case_id: fixture.id, code: 'runtime_input_candidate_ids_mismatch' });
      }
      const candidateById = new Map(runtimeReceipt._candidates.map((candidate) => [
        candidate.candidate_id,
        candidate,
      ]));
      if (result.hits.some((hit) => candidateById.get(hit.candidate_id)?.source_id !== hit.source_id)) {
        armContractIssues.push({ case_id: fixture.id, code: 'runtime_input_candidate_source_mismatch' });
      }
      if (result.hits.some((hit) => (
        hit.selection_kind === 'supplement'
        && candidateById.get(hit.candidate_id)?.document_id !== hit.document_id
      ))) {
        armContractIssues.push({ case_id: fixture.id, code: 'runtime_input_supplement_document_mismatch' });
      }
      const requiredSupplyKind = {
        structured_query: 'structured_query',
        structured_row_scan: 'structured_row_scan',
        parse_status: 'parse_status',
        ordinary_chat: 'ordinary_knowledge',
      }[runtimeReceipt.evaluation_kind];
      if (requiredSupplyKind) {
        if (result.hits.some((hit) => (
          candidateById.get(hit.candidate_id)?.supply_kind !== requiredSupplyKind
          || hit.selection_kind !== 'baseline'
        ))) {
          armContractIssues.push({ case_id: fixture.id, code: 'guard_case_document_or_graph_hit_forbidden' });
        }
      }
    }
    const sources = orderedSourceIds(result);
    const top20 = sources.slice(0, 20);
    const top5 = sources.slice(0, 5);
    const matchedExpected = fixture.expected_sources.filter((expected) =>
      top20.some((source) => sourceMatches(source, expected)));
    const firstExpectedRank = sources.findIndex((source) =>
      fixture.expected_sources.some((expected) => sourceMatches(source, expected))) + 1;
    const precisionMatches = top5.filter((source) =>
      fixture.expected_sources.some((expected) => sourceMatches(source, expected))).length;
    caseMetrics.push({
      case_id: fixture.id,
      graph_target: fixture.graph_target,
      recall_at_20: matchedExpected.length / fixture.expected_sources.length,
      mrr_at_20: firstExpectedRank > 0 && firstExpectedRank <= 20 ? 1 / firstExpectedRank : 0,
      precision_at_5: precisionMatches / 5,
    });
    for (const forbidden of fixture.forbidden_sources) {
      if (sources.some((source) => sourceMatches(source, forbidden))) {
        permissionLeaks.push({ case_id: fixture.id, type: 'forbidden_source' });
      }
    }
    if (Array.isArray(result.permission_leaks) && result.permission_leaks.length > 0) {
      permissionLeaks.push({ case_id: fixture.id, type: 'reported_permission_leak' });
    }
    if (result.execution_kind === rustOfflineExecutionKind) {
      rustOfflineRetrievalCount += 1;
      if (result.route.status === 'not_run') offlineRouteNotRunCount += 1;
      if (result.artifacts.length === 0) offlineArtifactZeroCount += 1;
    }
    if (result.contract.provider_call_count === 0 && result.contract.provider_retry_count === 0) {
      offlineProviderZeroCount += 1;
    }
    if ([
      'workflow_delta_count',
      'artifact_delta_count',
      'report_delta_count',
      'template_delta_count',
      'static_page_delta_count',
    ].every((field) => result.contract[field] === 0)) {
      sideEffectZeroCount += 1;
    }
    if (result.contract.same_run_off_path_self_consistency === true) {
      sameRunOffPathSelfConsistencyCount += 1;
    }
    if (result.scope_sha256 !== sha256(stableStringify(fixture.selected_scope))) {
      armContractIssues.push({ case_id: fixture.id, code: 'selected_scope_hash_mismatch' });
    }
    const supplementHits = result.hits.filter((hit) => hit.selection_kind === 'supplement');
    if (expectedArm === 'A' && result.hits.some((hit) => hit.selection_kind !== 'baseline')) {
      armContractIssues.push({ case_id: fixture.id, code: 'control_contains_nonbaseline_hit' });
    }
    if (expectedArm === 'B' && supplementHits.length > 0) {
      armContractIssues.push({ case_id: fixture.id, code: 'rerank_contains_supplement' });
    }
    if (expectedArm === 'C' && supplementHits.length > 2) {
      armContractIssues.push({ case_id: fixture.id, code: 'supplement_limit_exceeded' });
    }
  }
  const targeted = caseMetrics.filter((item) => item.graph_target);
  const negative = caseMetrics.filter((item) => !item.graph_target);
  const overlap = top5SourceOverlap(fixtures, resultMap);
  const denominator = evaluatedResults.length || 1;
  const dataReady = missingCaseIds.length === 0
    && extraCaseIds.length === 0
    && duplicateCaseIds.length === 0
    && invalidRows.length === 0
    && permissionLeaks.length === 0;
  const allResultsAreRustOfflineRetrieval = evaluatedResults.length === fixtures.length
    && rustOfflineRetrievalCount === fixtures.length;
  const executionExpectationsReady = allResultsAreRustOfflineRetrieval
    && offlineRouteNotRunCount === fixtures.length
    && offlineArtifactZeroCount === fixtures.length
    && offlineProviderZeroCount === fixtures.length
    && sideEffectZeroCount === fixtures.length;
  const expectationsReady = evaluatedResults.length === fixtures.length
    && executionExpectationsReady
    && (expectedArm !== 'A' || sameRunOffPathSelfConsistencyCount === fixtures.length)
    && armContractIssues.length === 0;
  const slices = metricSliceSummary(fixtures, caseMetrics);
  const grounding = groundingSummary(fixtures, evaluatedResults);
  const experimentIds = [...new Set(evaluatedResults.map((result) => result.experiment_id))];
  const fixtureHashes = [...new Set(evaluatedResults.map((result) => result.fixture_sha256))];
  const expectedFixtureHash = sha256(stableStringify(fixtures));
  const producerReceipts = [...new Set(evaluatedResults.map((result) => stableStringify(result.producer)))];
  const integrityReady = experimentIds.length === 1
    && fixtureHashes.length === 1
    && fixtureHashes[0] === expectedFixtureHash
    && producerReceipts.length === 1;
  return {
    arm: name,
    source_path: safeRelativePath(sourcePath),
    result_case_count: resultMap.size,
    missing_case_ids: missingCaseIds,
    extra_case_ids: extraCaseIds,
    duplicate_case_ids: duplicateCaseIds,
    invalid_rows: invalidRows,
    permission_leak_count: permissionLeaks.length,
    permission_leaks: permissionLeaks,
    metrics: metricSummary(caseMetrics),
    targeted_metrics: metricSummary(targeted),
    negative_control_metrics: metricSummary(negative),
    metrics_by_category: slices.by_category,
    metrics_by_split: slices.by_split,
    grounding,
    top5_source_overlap: overlap.value,
    top5_source_overlap_pair_count: overlap.pair_count,
    p95_latency_ms: percentile95(evaluatedResults.map((item) => item.latency_ms)),
    execution_kind: allResultsAreRustOfflineRetrieval ? rustOfflineExecutionKind : 'mixed_or_other',
    offline_route_not_run_rate: round(offlineRouteNotRunCount / denominator),
    offline_artifact_zero_rate: round(offlineArtifactZeroCount / denominator),
    offline_provider_zero_rate: round(offlineProviderZeroCount / denominator),
    side_effect_zero_rate: round(sideEffectZeroCount / denominator),
    same_run_off_path_self_consistency_rate: round(
      sameRunOffPathSelfConsistencyCount / denominator,
    ),
    arm_contract_issues: armContractIssues,
    experiment_ids: experimentIds,
    fixture_hashes: fixtureHashes,
    expected_fixture_sha256: expectedFixtureHash,
    integrity_ready: integrityReady,
    runtime_input_link_ready: runtimeInputs?.ready === true
      && dataReady
      && evaluatedResults.length === fixtures.length
      && !armContractIssues.some((issue) => issue.code.startsWith('runtime_input_')),
    data_ready: dataReady,
    expectations_ready: expectationsReady,
    ready: dataReady && expectationsReady && integrityReady && grounding.ready,
    _result_map: resultMap,
    _fixture_by_id: fixtureById,
  };
}

function safeArmReport(arm) {
  const { _result_map: ignoredMap, _fixture_by_id: ignoredFixtures, ...safe } = arm;
  return safe;
}

function normalizedProvider(result) {
  return {
    provider_config_sha256: result.contract.provider_config_sha256,
    call_count: result.contract.provider_call_count,
    retry_count: result.contract.provider_retry_count,
  };
}

function comparisonParity(fixtures, control, treatment) {
  let comparable = 0;
  let offlineNotRunRouteMatches = 0;
  let artifactMatches = 0;
  let providerMatches = 0;
  let linkedSnapshotMatches = 0;
  let immutableComponentMatches = 0;
  let sideEffectMatches = 0;
  let negativeSupplyMatches = 0;
  let negativeComparable = 0;
  let newArtifactCount = 0;
  let providerCallIncrease = 0;
  let providerRetryIncrease = 0;
  let maximumNewHitCount = 0;
  for (const fixture of fixtures) {
    const left = control._result_map.get(fixture.id);
    const right = treatment._result_map.get(fixture.id);
    if (!left || !right) continue;
    comparable += 1;
    if (left.route?.status === 'not_run'
      && right.route?.status === 'not_run'
      && stableStringify(left.route) === stableStringify(right.route)) {
      offlineNotRunRouteMatches += 1;
    }
    if (stableStringify(left.artifacts) === stableStringify(right.artifacts)) artifactMatches += 1;
    if (stableStringify(normalizedProvider(left)) === stableStringify(normalizedProvider(right))) {
      providerMatches += 1;
    }
    if (left.experiment_id === right.experiment_id
      && left.fixture_sha256 === right.fixture_sha256
      && left.candidate_snapshot_sha256 === right.candidate_snapshot_sha256
      && left.semantic_snapshot_sha256 === right.semantic_snapshot_sha256
      && left.scope_sha256 === right.scope_sha256
      && left.permission_sha256 === right.permission_sha256
      && left.runtime_input_sha256 === right.runtime_input_sha256
      && left.evaluation_kind === right.evaluation_kind
      && stableStringify(left.candidate_ids) === stableStringify(right.candidate_ids)) {
      linkedSnapshotMatches += 1;
    }
    if (left.contract.immutable_component_sha256 === right.contract.immutable_component_sha256) {
      immutableComponentMatches += 1;
    }
    const sideEffectFields = [
      'workflow_delta_count',
      'artifact_delta_count',
      'report_delta_count',
      'template_delta_count',
      'static_page_delta_count',
    ];
    if (sideEffectFields.every((field) => left.contract[field] === right.contract[field])) {
      sideEffectMatches += 1;
    }
    newArtifactCount += Math.max(0, right.artifacts.length - left.artifacts.length);
    providerCallIncrease += Math.max(0, right.contract.provider_call_count - left.contract.provider_call_count);
    providerRetryIncrease += Math.max(0, right.contract.provider_retry_count - left.contract.provider_retry_count);
    const leftCandidateIds = new Set(left.hits.map((hit) => hit.candidate_id));
    const newHitCount = right.hits.filter((hit) => !leftCandidateIds.has(hit.candidate_id)).length;
    maximumNewHitCount = Math.max(maximumNewHitCount, newHitCount);
    if (!fixture.graph_target) {
      negativeComparable += 1;
      if (stableStringify(orderedSourceIds(left)) === stableStringify(orderedSourceIds(right))) {
        negativeSupplyMatches += 1;
      }
    }
  }
  const denominator = comparable || 1;
  return {
    comparable_case_count: comparable,
    offline_route_not_run_consistency_rate: round(offlineNotRunRouteMatches / denominator),
    artifact_parity_rate: round(artifactMatches / denominator),
    provider_parity_rate: round(providerMatches / denominator),
    linked_snapshot_parity_rate: round(linkedSnapshotMatches / denominator),
    immutable_component_parity_rate: round(immutableComponentMatches / denominator),
    side_effect_parity_rate: round(sideEffectMatches / denominator),
    negative_supply_parity_rate: round(negativeSupplyMatches / (negativeComparable || 1)),
    new_artifact_count: newArtifactCount,
    provider_call_increase: providerCallIncrease,
    provider_retry_increase: providerRetryIncrease,
    maximum_new_hit_count: maximumNewHitCount,
  };
}

function providerInputSizeComparison(fixtures, control, treatment) {
  let maximumRelativeIncrease = 0;
  let comparable = 0;
  for (const fixture of fixtures) {
    const left = control._result_map.get(fixture.id);
    const right = treatment._result_map.get(fixture.id);
    if (!left || !right) continue;
    comparable += 1;
    const relative = (right.contract.provider_input_bytes - left.contract.provider_input_bytes)
      / Math.max(left.contract.provider_input_bytes, 1);
    maximumRelativeIncrease = Math.max(maximumRelativeIncrease, relative);
  }
  return {
    comparable_case_count: comparable,
    maximum_relative_increase: round(maximumRelativeIncrease),
  };
}

function slicedQualityGates(controlSlice, treatmentSlice, profile) {
  const recallDelta = treatmentSlice.metrics.recall_at_20 - controlSlice.metrics.recall_at_20;
  const mrrDelta = treatmentSlice.metrics.mrr_at_20 - controlSlice.metrics.mrr_at_20;
  const targetedRecallDelta = treatmentSlice.targeted_metrics.recall_at_20
    - controlSlice.targeted_metrics.recall_at_20;
  const targetedMrrDelta = treatmentSlice.targeted_metrics.mrr_at_20
    - controlSlice.targeted_metrics.mrr_at_20;
  const targetedPrecisionDelta = treatmentSlice.targeted_metrics.precision_at_5
    - controlSlice.targeted_metrics.precision_at_5;
  return profile === 'supplement'
    ? {
      recall_at_20_noninferior: recallDelta >= thresholds.recall_noninferiority,
      mrr_at_20_noninferior: mrrDelta >= thresholds.mrr_noninferiority,
      targeted_recall_at_20_gain: targetedRecallDelta >= thresholds.targeted_recall_gain_for_supplement,
      targeted_precision_at_5_noninferior: targetedPrecisionDelta >= thresholds.targeted_precision_at_5_noninferiority,
    }
    : {
      recall_at_20_noninferior: recallDelta >= thresholds.recall_noninferiority,
      mrr_at_20_noninferior: mrrDelta >= thresholds.mrr_noninferiority,
      targeted_mrr_at_20_gain: targetedMrrDelta >= thresholds.targeted_mrr_gain,
      targeted_precision_at_5_gain: targetedPrecisionDelta >= thresholds.targeted_precision_at_5_gain,
    };
}

function categoryCoverageReady(fixtures, control, treatment) {
  return Object.keys(requiredCategoryMinimums).every((category) => {
    const expected = fixtures.filter((fixture) => fixture.category === category).length;
    return control.metrics_by_category[category]?.metrics.case_count === expected
      && treatment.metrics_by_category[category]?.metrics.case_count === expected;
  });
}

function buildComparison(label, fixtures, control, treatment, profile = 'primary') {
  const parity = comparisonParity(fixtures, control, treatment);
  const recallDelta = treatment.metrics.recall_at_20 - control.metrics.recall_at_20;
  const mrrDelta = treatment.metrics.mrr_at_20 - control.metrics.mrr_at_20;
  const targetedRecallDelta = treatment.targeted_metrics.recall_at_20 - control.targeted_metrics.recall_at_20;
  const targetedMrrDelta = treatment.targeted_metrics.mrr_at_20 - control.targeted_metrics.mrr_at_20;
  const targetedPrecisionDelta = treatment.targeted_metrics.precision_at_5
    - control.targeted_metrics.precision_at_5;
  const overlapReduction = control.top5_source_overlap > 0
    ? (control.top5_source_overlap - treatment.top5_source_overlap) / control.top5_source_overlap
    : treatment.top5_source_overlap === 0 ? 0 : Number.NEGATIVE_INFINITY;
  const latencyDelta = treatment.p95_latency_ms - control.p95_latency_ms;
  const latencyRelativeDelta = latencyDelta / Math.max(control.p95_latency_ms, 1);
  const providerInputSize = providerInputSizeComparison(fixtures, control, treatment);
  const holdoutGates = slicedQualityGates(
    control.metrics_by_split.holdout,
    treatment.metrics_by_split.holdout,
    profile,
  );
  const commonGates = {
    control_and_treatment_ready: control.ready && treatment.ready,
    grounding_ready: control.grounding.ready && treatment.grounding.ready,
    permission_leaks_zero: control.permission_leak_count === 0 && treatment.permission_leak_count === 0,
    recall_at_20_noninferior: recallDelta >= thresholds.recall_noninferiority,
    mrr_at_20_noninferior: mrrDelta >= thresholds.mrr_noninferiority,
    offline_route_not_run_consistency:
      parity.offline_route_not_run_consistency_rate === 1,
    artifact_parity: parity.artifact_parity_rate === 1 && parity.new_artifact_count === 0,
    provider_parity: parity.provider_parity_rate === 1
      && parity.provider_call_increase === 0
      && parity.provider_retry_increase === 0,
    same_candidate_snapshot_and_scope: parity.linked_snapshot_parity_rate === 1,
    immutable_model_components_equal: parity.immutable_component_parity_rate === 1,
    side_effect_parity: parity.side_effect_parity_rate === 1,
    negative_supply_parity: parity.negative_supply_parity_rate === 1,
    latency_absolute_gate: latencyDelta <= thresholds.latency_absolute_increase_ms,
    latency_relative_gate: latencyRelativeDelta <= thresholds.latency_relative_increase,
    provider_input_size_gate: providerInputSize.maximum_relative_increase <= 0.15,
    per_category_coverage_complete: categoryCoverageReady(fixtures, control, treatment),
    holdout_coverage_complete: control.metrics_by_split.holdout.metrics.case_count
      === fixtures.filter((fixture) => fixtureSplit(fixture) === 'holdout').length
      && treatment.metrics_by_split.holdout.metrics.case_count
        === fixtures.filter((fixture) => fixtureSplit(fixture) === 'holdout').length,
    holdout_quality_gate: Object.values(holdoutGates).every(Boolean),
  };
  const profileGates = profile === 'supplement'
    ? {
      targeted_recall_at_20_gain: targetedRecallDelta >= thresholds.targeted_recall_gain_for_supplement,
      targeted_precision_at_5_noninferior: targetedPrecisionDelta >= thresholds.targeted_precision_at_5_noninferiority,
      top5_source_overlap_nonincrease: treatment.top5_source_overlap - control.top5_source_overlap
        <= thresholds.top5_source_overlap_nonincrease,
      bounded_supplement_delta: parity.maximum_new_hit_count <= 2,
    }
    : {
      targeted_mrr_at_20_gain: targetedMrrDelta >= thresholds.targeted_mrr_gain,
      targeted_precision_at_5_gain: targetedPrecisionDelta >= thresholds.targeted_precision_at_5_gain,
      top5_source_overlap_reduced: overlapReduction >= thresholds.top5_source_overlap_relative_reduction,
    };
  const gates = { ...commonGates, ...profileGates };
  return {
    comparison: label,
    profile,
    deltas: {
      recall_at_20: round(recallDelta),
      mrr_at_20: round(mrrDelta),
      targeted_recall_at_20: round(targetedRecallDelta),
      targeted_mrr_at_20: round(targetedMrrDelta),
      targeted_precision_at_5: round(targetedPrecisionDelta),
      top5_source_overlap_relative_reduction: round(overlapReduction),
      p95_latency_ms: round(latencyDelta),
      p95_latency_relative: round(latencyRelativeDelta),
    },
    parity,
    provider_input_size: providerInputSize,
    holdout_gates: holdoutGates,
    category_metrics: Object.fromEntries(
      Object.keys(requiredCategoryMinimums).sort().map((category) => [category, {
        control: control.metrics_by_category[category],
        treatment: treatment.metrics_by_category[category],
      }]),
    ),
    thresholds,
    gates,
    ready: Object.values(gates).every(Boolean),
  };
}

const supplementSafetyGateNames = Object.freeze([
  'control_and_treatment_ready',
  'grounding_ready',
  'permission_leaks_zero',
  'offline_route_not_run_consistency',
  'artifact_parity',
  'provider_parity',
  'same_candidate_snapshot_and_scope',
  'immutable_model_components_equal',
  'side_effect_parity',
  'negative_supply_parity',
  'provider_input_size_gate',
  'per_category_coverage_complete',
  'holdout_coverage_complete',
]);

function supplementComparisonSafetyReady(comparison, requireBoundedSupplement = false) {
  return supplementSafetyGateNames.every((gate) => comparison.gates[gate] === true)
    && (!requireBoundedSupplement || comparison.gates.bounded_supplement_delta === true);
}

function retrievalCandidateReadiness({
  runtimeInputReady,
  comparisonB,
  comparisonC,
  comparisonCvsB,
  independentFeatureOffBaselineReady = false,
}) {
  const rerankCandidateReady = runtimeInputReady
    && independentFeatureOffBaselineReady
    && comparisonB.ready;
  const supplementSafetyReady = runtimeInputReady
    && independentFeatureOffBaselineReady
    && supplementComparisonSafetyReady(comparisonC)
    && supplementComparisonSafetyReady(comparisonCvsB, true);
  const supplementCandidateReady = rerankCandidateReady
    && supplementSafetyReady
    && comparisonC.ready
    && comparisonCvsB.ready;
  return {
    feature_off_baseline_status: independentFeatureOffBaselineReady
      ? 'independently_measured'
      : 'not_independently_measured',
    feature_off_baseline_independently_measured: independentFeatureOffBaselineReady,
    retrieval_ready: rerankCandidateReady && supplementSafetyReady,
    rerank_candidate_ready: rerankCandidateReady,
    supplement_safety_ready: supplementSafetyReady,
    supplement_candidate_ready: supplementCandidateReady,
    retrieval_candidate: supplementCandidateReady
      ? 'supplement'
      : rerankCandidateReady && supplementSafetyReady ? 'rerank' : null,
  };
}

function unavailableAnswerEvaluation(reason = 'answer_receipt_not_supplied') {
  return {
    recorded: false,
    ready: false,
    status: 'not_run',
    provider_execution_status: 'not_run',
    issues: [{ code: reason }],
    gates: {
      independent_case_citation_and_legacy_corpus_receipts: false,
      complete_linked_answer_receipt: false,
      provider_execution_completed: false,
      answer_route_parity: false,
      newbai_required_business_evidence_8_of_8: false,
      semantic_target_answer_pass_gain: false,
      unsupported_false_claims_zero: false,
      internal_field_leakage_zero: false,
      citation_accuracy_noninferior: false,
      document_quality_11_of_11: false,
      regression_guards_clean: false,
      repeated_opening_5gram_within_limit: false,
      blind_naturalness_no_material_loss: false,
      per_category_answer_quality: false,
      per_split_answer_quality: false,
    },
  };
}

function evaluateAnswerReceipt(receipt, fixtures, arms, sourcePath = null) {
  const issues = [];
  if (!isObject(receipt) || receipt.schema_version !== answerReceiptSchemaVersion) {
    return {
      ...unavailableAnswerEvaluation('answer_receipt_schema_invalid'),
      recorded: true,
      status: 'invalid',
      provider_execution_status: 'invalid',
    };
  }
  const experimentIds = [...new Set(arms.flatMap((arm) => arm.experiment_ids))];
  const fixtureHashes = [...new Set(arms.flatMap((arm) => arm.fixture_hashes))];
  if (experimentIds.length !== 1 || receipt.experiment_id !== experimentIds[0]) {
    issues.push({ code: 'answer_receipt_experiment_mismatch' });
  }
  if (fixtureHashes.length !== 1 || receipt.fixture_sha256 !== fixtureHashes[0]) {
    issues.push({ code: 'answer_receipt_fixture_hash_mismatch' });
  }
  if (receipt.status !== 'measured') issues.push({ code: 'answer_receipt_status_not_measured' });
  issues.push({
    code: 'answer_receipt_v2_not_promotion_eligible_missing_independent_case_citation_and_legacy_corpus_receipts',
  });
  if (!isObject(receipt.producer)
    || receipt.producer.kind !== 'answer-evaluation-runner'
    || typeof receipt.producer.evaluator_version !== 'string'
    || !receipt.producer.evaluator_version) {
    issues.push({ code: 'answer_receipt_producer_invalid' });
  }
  const expectedInvocationCount = fixtures.length * 3;
  const providerExecution = receipt.provider_execution;
  if (!isObject(providerExecution)
    || providerExecution.status !== 'completed'
    || typeof providerExecution.provider_name !== 'string'
    || !providerExecution.provider_name
    || typeof providerExecution.model !== 'string'
    || !providerExecution.model
    || providerExecution.invocation_count !== expectedInvocationCount
    || !isSha256(providerExecution.run_receipt_sha256)) {
    issues.push({ code: 'answer_provider_execution_invalid' });
  }
  if (!Array.isArray(receipt.cases)) issues.push({ code: 'answer_receipt_cases_required' });
  const expectedKeys = fixtures.flatMap((fixture) => ['A', 'B', 'C'].map((arm) => `${fixture.id}:${arm}`));
  const resultByKey = new Map();
  for (const arm of arms) {
    for (const [caseId, result] of arm._result_map.entries()) {
      resultByKey.set(`${caseId}:${result.arm}`, result);
    }
  }
  const rowsByKey = new Map();
  for (const [index, row] of (Array.isArray(receipt.cases) ? receipt.cases : []).entries()) {
    const rowIssues = [];
    if (!isObject(row) || typeof row.case_id !== 'string' || !['A', 'B', 'C'].includes(row.arm)) {
      rowIssues.push('identity_invalid');
    }
    if (!isSha256(row?.answer_sha256)) rowIssues.push('answer_sha256_invalid');
    if (!isSha256(row?.provider_input_sha256)) rowIssues.push('provider_input_sha256_invalid');
    if (!isSha256(row?.provider_response_sha256)) rowIssues.push('provider_response_sha256_invalid');
    if (!isObject(row?.route)) rowIssues.push('route_object_required');
    if (typeof row?.answer_pass !== 'boolean') rowIssues.push('answer_pass_boolean_required');
    if (typeof row?.required_business_evidence_pass !== 'boolean') {
      rowIssues.push('required_business_evidence_pass_boolean_required');
    }
    for (const field of ['unsupported_claim_count', 'internal_field_leak_count']) {
      if (!nonNegativeInteger(row?.[field])) rowIssues.push(`${field}_invalid`);
    }
    if (!nonNegativeInteger(row?.citation_claim_count)) rowIssues.push('citation_claim_count_invalid');
    if (!nonNegativeInteger(row?.supported_citation_claim_count)
      || row.supported_citation_claim_count > row.citation_claim_count) {
      rowIssues.push('supported_citation_claim_count_invalid');
    }
    if (!finiteRatio(row?.citation_accuracy)) rowIssues.push('citation_accuracy_invalid');
    const recomputedCitationAccuracy = nonNegativeInteger(row?.citation_claim_count)
      && nonNegativeInteger(row?.supported_citation_claim_count)
      ? row.citation_claim_count > 0
        ? row.supported_citation_claim_count / row.citation_claim_count
        : 0
      : null;
    if (recomputedCitationAccuracy !== null
      && Math.abs(row.citation_accuracy - recomputedCitationAccuracy) > 0.000001) {
      rowIssues.push('citation_accuracy_mismatch');
    }
    if (row?.required_business_evidence_pass === true && row?.citation_claim_count < 1) {
      rowIssues.push('business_evidence_pass_requires_citation');
    }
    if (!finiteRatio(row?.repeated_opening_5gram_overlap)) {
      rowIssues.push('repeated_opening_5gram_overlap_invalid');
    }
    if (!isObject(row?.naturalness)
      || row.naturalness.blind_reviewed !== true
      || typeof row.naturalness.material_loss !== 'boolean') {
      rowIssues.push('naturalness_blind_review_invalid');
    }
    if (rowIssues.length > 0) {
      issues.push({ code: 'answer_case_invalid', index, errors: rowIssues });
      continue;
    }
    const key = `${row.case_id}:${row.arm}`;
    const linkedResult = resultByKey.get(key);
    if (!linkedResult
      || row.provider_input_sha256 !== linkedResult.contract.provider_input_sha256) {
      issues.push({ code: 'answer_provider_input_link_mismatch', key });
      continue;
    }
    if (rowsByKey.has(key)) issues.push({ code: 'answer_case_duplicate', key });
    rowsByKey.set(key, row);
  }
  const missingKeys = expectedKeys.filter((key) => !rowsByKey.has(key));
  const extraKeys = [...rowsByKey.keys()].filter((key) => !expectedKeys.includes(key));
  if (missingKeys.length > 0) issues.push({ code: 'answer_cases_missing', count: missingKeys.length });
  if (extraKeys.length > 0) issues.push({ code: 'answer_cases_extra', count: extraKeys.length });

  const fixtureById = new Map(fixtures.map((fixture) => [fixture.id, fixture]));
  const routeParityMismatchCaseIds = fixtures
    .filter((fixture) => {
      const routes = ['A', 'B', 'C']
        .map((arm) => rowsByKey.get(`${fixture.id}:${arm}`)?.route)
        .filter((route) => route !== undefined)
        .map((route) => stableStringify(route));
      return routes.length !== 3 || new Set(routes).size !== 1;
    })
    .map((fixture) => fixture.id);
  if (routeParityMismatchCaseIds.length > 0) {
    issues.push({
      code: 'answer_route_parity_mismatch',
      count: routeParityMismatchCaseIds.length,
    });
  }
  const summarizeRows = (rows) => {
    const targeted = rows.filter((row) => fixtureById.get(row.case_id)?.graph_target);
    return {
      case_count: rows.length,
      target_case_count: targeted.length,
      all_answer_pass_rate: round(mean(rows.map((row) => (row.answer_pass ? 1 : 0)))),
      target_answer_pass_rate: round(mean(targeted.map((row) => (row.answer_pass ? 1 : 0)))),
      required_business_evidence_pass_rate: round(mean(targeted.map((row) => (
        row.required_business_evidence_pass ? 1 : 0
      )))),
      unsupported_claim_count: rows.reduce((sum, row) => sum + row.unsupported_claim_count, 0),
      internal_field_leak_count: rows.reduce((sum, row) => sum + row.internal_field_leak_count, 0),
      citation_accuracy: round(mean(rows.map((row) => row.citation_accuracy))),
      repeated_opening_5gram_overlap: round(mean(rows.map((row) => row.repeated_opening_5gram_overlap))),
      material_naturalness_loss_count: rows.filter((row) => row.naturalness.material_loss).length,
    };
  };
  const allRows = [...rowsByKey.values()];
  const summarizeArm = (arm, allowedCaseIds = null) => summarizeRows(allRows.filter((row) => (
    row.arm === arm && (!allowedCaseIds || allowedCaseIds.has(row.case_id))
  )));
  const armMetrics = Object.fromEntries(['A', 'B', 'C'].map((arm) => [arm, summarizeArm(arm)]));
  const sliceMetrics = (selectedFixtures) => {
    const selectedIds = new Set(selectedFixtures.map((fixture) => fixture.id));
    return Object.fromEntries(['A', 'B', 'C'].map((arm) => [arm, summarizeArm(arm, selectedIds)]));
  };
  const sliceGate = (selectedFixtures, metrics, minimumTargetGain = 0) => {
    const expectedCount = selectedFixtures.length;
    const controlMetric = metrics.A.target_case_count > 0
      ? metrics.A.target_answer_pass_rate
      : metrics.A.all_answer_pass_rate;
    const treatmentMetrics = ['B', 'C'].map((arm) => ({
      arm,
      metric: metrics[arm].target_case_count > 0
        ? metrics[arm].target_answer_pass_rate
        : metrics[arm].all_answer_pass_rate,
    }));
    const gates = {
      complete: ['A', 'B', 'C'].every((arm) => metrics[arm].case_count === expectedCount),
      answer_pass_noninferior: treatmentMetrics.every(({ metric }) => (
        metric - controlMetric >= minimumTargetGain
      )),
      unsupported_false_claims_zero: ['B', 'C'].every((arm) => metrics[arm].unsupported_claim_count === 0),
      internal_field_leakage_zero: ['B', 'C'].every((arm) => metrics[arm].internal_field_leak_count === 0),
      citation_accuracy_noninferior: ['B', 'C'].every((arm) => (
        metrics[arm].citation_accuracy >= metrics.A.citation_accuracy
      )),
      repeated_opening_5gram_within_limit: ['B', 'C'].every((arm) => (
        metrics[arm].repeated_opening_5gram_overlap
          - metrics.A.repeated_opening_5gram_overlap <= 0.05
      )),
      blind_naturalness_no_material_loss: ['B', 'C'].every((arm) => (
        metrics[arm].material_naturalness_loss_count === 0
      )),
    };
    return { gates, ready: Object.values(gates).every(Boolean) };
  };
  const categoryMetrics = Object.fromEntries(
    Object.keys(requiredCategoryMinimums).sort().map((category) => [
      category,
      sliceMetrics(fixtures.filter((fixture) => fixture.category === category)),
    ]),
  );
  const categoryGates = Object.fromEntries(
    Object.entries(categoryMetrics).map(([category, metrics]) => [
      category,
      sliceGate(fixtures.filter((fixture) => fixture.category === category), metrics),
    ]),
  );
  const splitMetrics = {
    development: sliceMetrics(fixtures.filter((fixture) => fixtureSplit(fixture) === 'development')),
    holdout: sliceMetrics(fixtures.filter((fixture) => fixtureSplit(fixture) === 'holdout')),
  };
  const splitGates = {
    development: sliceGate(
      fixtures.filter((fixture) => fixtureSplit(fixture) === 'development'),
      splitMetrics.development,
    ),
    holdout: sliceGate(
      fixtures.filter((fixture) => fixtureSplit(fixture) === 'holdout'),
      splitMetrics.holdout,
      0.2,
    ),
  };
  const summary = isObject(receipt.summary) ? receipt.summary : {};
  const regressionFields = [
    'database_topn_regression_count',
    'row_completeness_regression_count',
    'resume_statistics_regression_count',
    'ordinary_chat_regression_count',
  ];
  const gates = {
    independent_case_citation_and_legacy_corpus_receipts: false,
    complete_linked_answer_receipt: issues.length === 0
      && armMetrics.A.case_count === fixtures.length
      && armMetrics.B.case_count === fixtures.length
      && armMetrics.C.case_count === fixtures.length,
    provider_execution_completed: providerExecution?.status === 'completed'
      && providerExecution?.invocation_count === expectedInvocationCount,
    answer_route_parity: routeParityMismatchCaseIds.length === 0,
    newbai_required_business_evidence_8_of_8:
      summary.newbai_001_008_required_business_evidence_pass_count === 8,
    semantic_target_answer_pass_gain:
      armMetrics.B.target_answer_pass_rate - armMetrics.A.target_answer_pass_rate >= 0.2
      && armMetrics.C.target_answer_pass_rate - armMetrics.A.target_answer_pass_rate >= 0.2,
    unsupported_false_claims_zero:
      armMetrics.B.unsupported_claim_count === 0 && armMetrics.C.unsupported_claim_count === 0,
    internal_field_leakage_zero:
      armMetrics.B.internal_field_leak_count === 0 && armMetrics.C.internal_field_leak_count === 0,
    citation_accuracy_noninferior:
      armMetrics.B.citation_accuracy >= armMetrics.A.citation_accuracy
      && armMetrics.C.citation_accuracy >= armMetrics.A.citation_accuracy,
    document_quality_11_of_11: summary.document_quality_pass_count === 11,
    regression_guards_clean: regressionFields.every((field) => summary[field] === 0),
    repeated_opening_5gram_within_limit:
      armMetrics.B.repeated_opening_5gram_overlap - armMetrics.A.repeated_opening_5gram_overlap <= 0.05
      && armMetrics.C.repeated_opening_5gram_overlap - armMetrics.A.repeated_opening_5gram_overlap <= 0.05,
    blind_naturalness_no_material_loss:
      armMetrics.B.material_naturalness_loss_count === 0
      && armMetrics.C.material_naturalness_loss_count === 0,
    per_category_answer_quality: Object.values(categoryGates).every((value) => value.ready),
    per_split_answer_quality: Object.values(splitGates).every((value) => value.ready),
  };
  return {
    recorded: true,
    ready: Object.values(gates).every(Boolean),
    status: 'measured',
    provider_execution_status: providerExecution?.status || 'invalid',
    source_path: safeRelativePath(sourcePath),
    receipt_sha256: sha256(stableStringify(receipt)),
    issues,
    arm_metrics: armMetrics,
    metrics_by_category: categoryMetrics,
    category_gates: categoryGates,
    metrics_by_split: splitMetrics,
    split_gates: splitGates,
    gates,
  };
}

function syntheticResultRows(fixtures, arm, runtimeInputs) {
  const fixtureHash = sha256(stableStringify(fixtures));
  return fixtures.map((fixture, index) => {
    const runtimeReceipt = runtimeInputs.case_receipts.get(fixture.id);
    if (!runtimeReceipt) throw new Error(`self-test runtime input missing for ${fixture.id}`);
    const common = [...new Set(runtimeReceipt._candidates
      .map((candidate) => candidate.source_id)
      .filter((sourceId) => sourceId.startsWith('synthetic://visible/generic/')))].slice(0, 5);
    const distractors = [...new Set(runtimeReceipt._candidates
      .map((candidate) => candidate.source_id)
      .filter((sourceId) => sourceId.includes('/distractor-')))];
    let sources;
    if (runtimeReceipt.evaluation_kind !== 'retrieval_evidence') {
      sources = [fixture.expected_sources[0]];
    } else if (arm === 'A') {
      sources = [...common, fixture.expected_sources[0]];
    } else if (arm === 'B') {
      sources = [fixture.expected_sources[0], ...distractors.slice(0, 4)];
    } else {
      sources = [...fixture.expected_sources, ...distractors].slice(0, 5);
    }
    const route = {
      status: 'not_run',
      reason: 'retrieval_only_exporter_does_not_compute_route',
    };
    const routeHash = sha256(stableStringify(route));
    const immutableComponents = {
      question_sha256: runtimeReceipt.question_sha256,
      system_prompt_sha256: sha256('system-prompt:self-test'),
      answer_policy_sha256: sha256('answer-policy:self-test'),
      provider_config_sha256: sha256('provider-config:not-called'),
      selected_scope_sha256: runtimeReceipt.scope_sha256,
      permission_sha256: runtimeReceipt.permission_sha256,
      evaluation_kind_sha256: runtimeReceipt.evaluation_kind_sha256,
      runtime_input_sha256: runtimeReceipt.runtime_input_sha256,
      action_catalog_sha256: sha256('action-catalog:self-test'),
      route_sha256: routeHash,
    };
    const grounding = fixture.graph_target
      ? {
        status: 'measured',
        snapshot_status: 'ready',
        snapshot_stale: false,
        snapshot_compatible: true,
        target_node_count: 1,
        resolved_target_node_count: 1,
        confirmed_or_observed_used_count: 1,
        inferred_used_count: 0,
        unresolved_used_count: 0,
        stale_used_count: 0,
        hidden_source_use_count: 0,
        unresolvable_source_use_count: 0,
      }
      : {
        status: 'not_applicable',
        snapshot_status: 'not_applicable',
        snapshot_stale: false,
        snapshot_compatible: true,
        target_node_count: 0,
        resolved_target_node_count: 0,
        confirmed_or_observed_used_count: 0,
        inferred_used_count: 0,
        unresolved_used_count: 0,
        stale_used_count: 0,
        hidden_source_use_count: 0,
        unresolvable_source_use_count: 0,
      };
    const hits = sources.map((sourceId, offset) => {
      const candidate = runtimeReceipt._source_to_candidate.get(sourceId);
      if (!candidate) throw new Error(`self-test candidate missing for ${fixture.id}`);
      const isSupplement = arm === 'C'
        && fixture.expected_graph_behavior === 'supplement'
        && offset > 0
        && fixture.expected_sources.includes(sourceId);
      return {
        rank: offset + 1,
        candidate_id: candidate.candidate_id,
        source_id: sourceId,
        selection_kind: runtimeReceipt.evaluation_kind !== 'retrieval_evidence' || arm === 'A'
          ? 'baseline'
          : isSupplement ? 'supplement' : 'reranked',
        ...(isSupplement ? {
          retrieval_evidence_id: `evidence:${fixture.id}:${offset}`,
          document_id: candidate.document_id,
          document_chunk_id: `chunk:${fixture.id}:${offset}`,
          source_locator: sourceId,
        } : {}),
      };
    });
    const providerInputBytes = 1000 + index + (arm === 'A' ? 0 : arm === 'B' ? 80 : 100);
    return {
      schema_version: resultSchemaVersion,
      case_id: fixture.id,
      experiment_id: 'evaluator-self-test-not-experiment-evidence',
      arm,
      execution_kind: rustOfflineExecutionKind,
      evaluation_kind: runtimeReceipt.evaluation_kind,
      fixture_sha256: fixtureHash,
      runtime_input_sha256: runtimeReceipt.runtime_input_sha256,
      candidate_snapshot_sha256: runtimeReceipt.candidate_snapshot_sha256,
      semantic_snapshot_sha256: runtimeReceipt.semantic_snapshot_sha256,
      scope_sha256: runtimeReceipt.scope_sha256,
      permission_sha256: runtimeReceipt.permission_sha256,
      grounding_sha256: sha256(stableStringify(grounding)),
      producer: {
        kind: 'platform-api-rust-fixture-export',
        exporter_version: 'self-test-v2',
        git_head: '0000000000000000000000000000000000000000',
        git_head_source: 'git_rev_parse_head',
        worktree_clean: true,
        runtime_input_file_sha256: runtimeInputs.runtime_input_file_sha256,
        runtime_input_canonical_sha256: runtimeInputs.runtime_input_canonical_sha256,
      },
      latency_ms: 100 + index + (arm === 'A' ? 0 : arm === 'B' ? 10 : 15),
      candidate_ids: runtimeReceipt.candidate_ids,
      input_candidate_count: runtimeReceipt.candidate_count,
      hits,
      citations: [],
      route,
      artifacts: [],
      contract: {
        ...immutableComponents,
        immutable_component_sha256: sha256(stableStringify(immutableComponents)),
        model_visible_evidence_sha256: sha256(`model-evidence:${fixture.id}:${arm}`),
        provider_input_sha256: sha256(`provider-input:${fixture.id}:${arm}`),
        provider_input_bytes: providerInputBytes,
        provider_call_count: 0,
        provider_retry_count: 0,
        workflow_delta_count: 0,
        artifact_delta_count: 0,
        report_delta_count: 0,
        template_delta_count: 0,
        static_page_delta_count: 0,
        same_run_off_path_self_consistency: arm === 'A',
      },
      grounding,
      answer_evaluation: {
        status: 'not_run',
        reason: 'evaluator_self_test_does_not_call_provider',
      },
      permission_leaks: [],
    };
  });
}

function syntheticAnswerReceipt(fixtures, rowsByArm) {
  const fixtureHash = rowsByArm.A[0].fixture_sha256;
  const experimentId = rowsByArm.A[0].experiment_id;
  const resultByKey = new Map(Object.entries(rowsByArm).flatMap(([arm, rows]) => (
    rows.map((row) => [`${row.case_id}:${arm}`, row])
  )));
  return {
    schema_version: answerReceiptSchemaVersion,
    experiment_id: experimentId,
    fixture_sha256: fixtureHash,
    status: 'measured',
    producer: {
      kind: 'answer-evaluation-runner',
      evaluator_version: 'self-test-v2',
    },
    provider_execution: {
      status: 'completed',
      provider_name: 'synthetic-self-test-provider',
      model: 'synthetic-self-test-model',
      invocation_count: fixtures.length * 3,
      run_receipt_sha256: sha256('synthetic-self-test-provider-run'),
    },
    cases: fixtures.flatMap((fixture, index) => ['A', 'B', 'C'].map((arm) => ({
      case_id: fixture.id,
      arm,
      answer_sha256: sha256(`self-test-answer:${fixture.id}:${arm}`),
      provider_input_sha256: resultByKey.get(`${fixture.id}:${arm}`).contract.provider_input_sha256,
      provider_response_sha256: sha256(`self-test-provider-response:${fixture.id}:${arm}`),
      route: { observed_route: 'self_test_model_selected_route' },
      answer_pass: !fixture.graph_target || arm !== 'A' || index % 2 === 0,
      required_business_evidence_pass: !fixture.graph_target || arm !== 'A',
      unsupported_claim_count: 0,
      internal_field_leak_count: 0,
      citation_claim_count: 10,
      supported_citation_claim_count: arm === 'A' ? 8 : 9,
      citation_accuracy: arm === 'A' ? 0.8 : 0.9,
      repeated_opening_5gram_overlap: arm === 'A' ? 0.1 : 0.12,
      naturalness: {
        blind_reviewed: true,
        material_loss: false,
      },
    }))),
    summary: {
      newbai_001_008_required_business_evidence_pass_count: 8,
      document_quality_pass_count: 11,
      database_topn_regression_count: 0,
      row_completeness_regression_count: 0,
      resume_statistics_regression_count: 0,
      ordinary_chat_regression_count: 0,
    },
  };
}

function refreshImmutableComponentHash(row) {
  const immutableComponents = {
    question_sha256: row.contract.question_sha256,
    system_prompt_sha256: row.contract.system_prompt_sha256,
    answer_policy_sha256: row.contract.answer_policy_sha256,
    provider_config_sha256: row.contract.provider_config_sha256,
    selected_scope_sha256: row.contract.selected_scope_sha256,
    permission_sha256: row.contract.permission_sha256,
    evaluation_kind_sha256: row.contract.evaluation_kind_sha256,
    runtime_input_sha256: row.contract.runtime_input_sha256,
    action_catalog_sha256: row.contract.action_catalog_sha256,
    route_sha256: row.contract.route_sha256,
  };
  row.contract.immutable_component_sha256 = sha256(stableStringify(immutableComponents));
}

function runSelfTest(fixtures, runtimeInputs, loadedRuntimeInputs) {
  const runtimeValues = loadedRuntimeInputs.rows.map(({ value }) => structuredClone(value));
  const forbiddenRuntimeValues = structuredClone(runtimeValues);
  forbiddenRuntimeValues.find((value) => value.record_kind === 'case').expected_sources = [];
  const forbiddenRuntimeInputs = validateRuntimeInputs(
    loadedFromValues(forbiddenRuntimeValues, null),
    fixtures,
  );
  const candidateMutatedValues = structuredClone(runtimeValues);
  candidateMutatedValues.find((value) => value.record_kind === 'dataset')
    .candidate_pool[0].candidate_id += ':mutated';
  const candidateMutatedRuntimeInputs = validateRuntimeInputs(
    loadedFromValues(candidateMutatedValues, null),
    fixtures,
  );
  const semanticMutatedValues = structuredClone(runtimeValues);
  semanticMutatedValues.find((value) => (
    value.record_kind === 'dataset' && value.semantic_snapshot.nodes.length > 0
  )).semantic_snapshot.nodes[0].canonical_label += ' mutated';
  const semanticMutatedRuntimeInputs = validateRuntimeInputs(
    loadedFromValues(semanticMutatedValues, null),
    fixtures,
  );
  const questionMutatedValues = structuredClone(runtimeValues);
  questionMutatedValues.find((value) => value.record_kind === 'case').question += ' mutated';
  const questionMutatedRuntimeInputs = validateRuntimeInputs(
    loadedFromValues(questionMutatedValues, null),
    fixtures,
  );
  const permissionMutatedValues = structuredClone(runtimeValues);
  permissionMutatedValues.find((value) => value.record_kind === 'case')
    .permission.allowed_evidence_classes.push('inferred');
  const permissionMutatedRuntimeInputs = validateRuntimeInputs(
    loadedFromValues(permissionMutatedValues, null),
    fixtures,
  );
  const invalidSingleScopeFixtures = structuredClone(fixtures);
  invalidSingleScopeFixtures[0].selected_scope.selected_dataset_keys.push('synthetic-extra-dataset');
  const invalidSingleScope = validateFixtures(loadedFromValues(invalidSingleScopeFixtures, 'id'));
  const invalidCrossScopeFixtures = structuredClone(fixtures);
  const crossFixture = invalidCrossScopeFixtures.find((fixture) => (
    fixture.category === 'multi_dataset_selected_scope'
  ));
  crossFixture.selected_scope.selected_dataset_keys = [crossFixture.selected_scope.selected_dataset_keys[0]];
  const invalidCrossScope = validateFixtures(loadedFromValues(invalidCrossScopeFixtures, 'id'));
  const invalidEvidenceClassFixtures = structuredClone(fixtures);
  invalidEvidenceClassFixtures[0].allowed_evidence_classes.push('inferred');
  const invalidEvidenceClass = validateFixtures(loadedFromValues(invalidEvidenceClassFixtures, 'id'));
  const rowsA = syntheticResultRows(fixtures, 'A', runtimeInputs);
  const rowsB = syntheticResultRows(fixtures, 'B', runtimeInputs);
  const rowsC = syntheticResultRows(fixtures, 'C', runtimeInputs);
  const targetCaseIds = fixtures.filter((fixture) => fixture.graph_target).map((fixture) => fixture.id);
  const withoutMeasuredGrounding = (rows, caseIds) => {
    const selected = new Set(caseIds);
    const mutated = structuredClone(rows);
    for (const row of mutated.filter((item) => selected.has(item.case_id))) {
      row.grounding = {
        status: 'not_applicable',
        snapshot_status: 'not_applicable',
        snapshot_stale: false,
        snapshot_compatible: true,
        target_node_count: 0,
        resolved_target_node_count: 0,
        confirmed_or_observed_used_count: 0,
        inferred_used_count: 0,
        unresolved_used_count: 0,
        stale_used_count: 0,
        hidden_source_use_count: 0,
        unresolvable_source_use_count: 0,
      };
      row.grounding_sha256 = sha256(stableStringify(row.grounding));
    }
    return mutated;
  };
  const grounding19Of20 = evaluateArm(
    'grounding_19_of_20',
    fixtures,
    loadedFromValues(withoutMeasuredGrounding(rowsB, targetCaseIds.slice(0, 1))),
    null,
    'B',
    runtimeInputs,
  );
  const grounding17Of20 = evaluateArm(
    'grounding_17_of_20',
    fixtures,
    loadedFromValues(withoutMeasuredGrounding(rowsB, targetCaseIds.slice(0, 3))),
    null,
    'B',
    runtimeInputs,
  );
  const invalidNegativeGroundingRows = structuredClone(rowsB);
  const negativeFixture = fixtures.find((fixture) => !fixture.graph_target);
  const invalidNegativeGrounding = invalidNegativeGroundingRows.find((row) => (
    row.case_id === negativeFixture?.id
  ));
  if (!invalidNegativeGrounding) throw new Error('self-test negative control fixture missing');
  invalidNegativeGrounding.grounding = {
    ...invalidNegativeGrounding.grounding,
    status: 'measured',
    snapshot_status: 'ready',
    target_node_count: 1,
    resolved_target_node_count: 1,
    confirmed_or_observed_used_count: 1,
  };
  invalidNegativeGrounding.grounding_sha256 = sha256(stableStringify(
    invalidNegativeGrounding.grounding,
  ));
  const strictNegativeGrounding = evaluateArm(
    'negative_control_grounding_strict',
    fixtures,
    loadedFromValues(invalidNegativeGroundingRows),
    null,
    'B',
    runtimeInputs,
  );
  const control = evaluateArm('A_control', fixtures, loadedFromValues(rowsA), null, 'A', runtimeInputs);
  const rerank = evaluateArm('B_graph_rerank', fixtures, loadedFromValues(rowsB), null, 'B', runtimeInputs);
  const supplement = evaluateArm(
    'C_graph_rerank_plus_evidence',
    fixtures,
    loadedFromValues(rowsC),
    null,
    'C',
    runtimeInputs,
  );
  const candidateManifestMismatchRows = structuredClone(rowsB);
  for (const row of candidateManifestMismatchRows) {
    row.producer.runtime_input_file_sha256 = candidateMutatedRuntimeInputs.runtime_input_file_sha256;
    row.producer.runtime_input_canonical_sha256 =
      candidateMutatedRuntimeInputs.runtime_input_canonical_sha256;
  }
  const candidateManifestMismatchArm = evaluateArm(
    'negative_runtime_candidate_manifest',
    fixtures,
    loadedFromValues(candidateManifestMismatchRows),
    null,
    'B',
    candidateMutatedRuntimeInputs,
  );
  const semanticManifestMismatchRows = structuredClone(rowsB);
  for (const row of semanticManifestMismatchRows) {
    row.producer.runtime_input_file_sha256 = semanticMutatedRuntimeInputs.runtime_input_file_sha256;
    row.producer.runtime_input_canonical_sha256 =
      semanticMutatedRuntimeInputs.runtime_input_canonical_sha256;
  }
  const semanticManifestMismatchArm = evaluateArm(
    'negative_runtime_semantic_manifest',
    fixtures,
    loadedFromValues(semanticManifestMismatchRows),
    null,
    'B',
    semanticMutatedRuntimeInputs,
  );
  const comparisonB = buildComparison('B_vs_A', fixtures, control, rerank);
  const comparisonC = buildComparison('C_vs_A', fixtures, control, supplement);
  const comparisonCvsB = buildComparison('C_vs_B', fixtures, rerank, supplement, 'supplement');
  const noGainSupplementRows = structuredClone(rowsB);
  for (const row of noGainSupplementRows) row.arm = 'C';
  const noGainSupplement = evaluateArm(
    'C_safe_without_incremental_gain',
    fixtures,
    loadedFromValues(noGainSupplementRows),
    null,
    'C',
    runtimeInputs,
  );
  const noGainComparisonC = buildComparison(
    'C_safe_without_incremental_gain_vs_A',
    fixtures,
    control,
    noGainSupplement,
  );
  const noGainComparisonCvsB = buildComparison(
    'C_safe_without_incremental_gain_vs_B',
    fixtures,
    rerank,
    noGainSupplement,
    'supplement',
  );
  const noGainCandidateReadiness = retrievalCandidateReadiness({
    runtimeInputReady: runtimeInputs.ready,
    comparisonB,
    comparisonC: noGainComparisonC,
    comparisonCvsB: noGainComparisonCvsB,
    independentFeatureOffBaselineReady: true,
  });
  const unmeasuredBaselineCandidateReadiness = retrievalCandidateReadiness({
    runtimeInputReady: runtimeInputs.ready,
    comparisonB,
    comparisonC: noGainComparisonC,
    comparisonCvsB: noGainComparisonCvsB,
  });

  const unsafeSupplementRows = structuredClone(noGainSupplementRows);
  unsafeSupplementRows[0].artifacts = [{ kind: 'static_page' }];
  unsafeSupplementRows[0].contract.artifact_delta_count = 1;
  const unsafeSupplement = evaluateArm(
    'C_unsafe_side_effect',
    fixtures,
    loadedFromValues(unsafeSupplementRows),
    null,
    'C',
    runtimeInputs,
  );
  const unsafeComparisonC = buildComparison(
    'C_unsafe_side_effect_vs_A', fixtures, control, unsafeSupplement,
  );
  const unsafeComparisonCvsB = buildComparison(
    'C_unsafe_side_effect_vs_B', fixtures, rerank, unsafeSupplement, 'supplement',
  );
  const unsafeCandidateReadiness = retrievalCandidateReadiness({
    runtimeInputReady: runtimeInputs.ready,
    comparisonB,
    comparisonC: unsafeComparisonC,
    comparisonCvsB: unsafeComparisonCvsB,
    independentFeatureOffBaselineReady: true,
  });

  const offlineNoRouteRows = {
    A: structuredClone(rowsA),
    B: structuredClone(rowsB),
    C: structuredClone(rowsC),
  };
  for (const rows of Object.values(offlineNoRouteRows)) {
    for (const row of rows) {
      row.route = {
        status: 'not_run',
        reason: 'retrieval_only_exporter_does_not_compute_route',
      };
      row.contract.route_sha256 = sha256(stableStringify(row.route));
      refreshImmutableComponentHash(row);
    }
  }
  const offlineNoRouteControl = evaluateArm(
    'offline_no_route_A',
    fixtures,
    loadedFromValues(offlineNoRouteRows.A),
    null,
    'A',
    runtimeInputs,
  );
  const offlineNoRouteRerank = evaluateArm(
    'offline_no_route_B',
    fixtures,
    loadedFromValues(offlineNoRouteRows.B),
    null,
    'B',
    runtimeInputs,
  );
  const offlineNoRouteSupplement = evaluateArm(
    'offline_no_route_C',
    fixtures,
    loadedFromValues(offlineNoRouteRows.C),
    null,
    'C',
    runtimeInputs,
  );
  const offlineNoRouteComparisonB = buildComparison(
    'offline_no_route_B_vs_A',
    fixtures,
    offlineNoRouteControl,
    offlineNoRouteRerank,
  );
  const offlineNoRouteComparisonC = buildComparison(
    'offline_no_route_C_vs_A',
    fixtures,
    offlineNoRouteControl,
    offlineNoRouteSupplement,
  );
  const offlineNoRouteComparisonCvsB = buildComparison(
    'offline_no_route_C_vs_B',
    fixtures,
    offlineNoRouteRerank,
    offlineNoRouteSupplement,
    'supplement',
  );

  const missing = evaluateArm(
    'negative_missing', fixtures, loadedFromValues(rowsB.slice(1)), null, 'B', runtimeInputs,
  );
  const duplicate = evaluateArm(
    'negative_duplicate',
    fixtures,
    loadedFromValues([...rowsB, structuredClone(rowsB[0])]),
    null,
    'B',
    runtimeInputs,
  );
  const invalidRows = structuredClone(rowsB);
  invalidRows[0].hits = 'not-an-array';
  const invalid = evaluateArm(
    'negative_invalid', fixtures, loadedFromValues(invalidRows), null, 'B', runtimeInputs,
  );
  const leakRows = structuredClone(rowsB);
  leakRows[0].hits[0].source_id = fixtures[0].forbidden_sources[0];
  const leak = evaluateArm(
    'negative_leak', fixtures, loadedFromValues(leakRows), null, 'B', runtimeInputs,
  );
  const parityRows = structuredClone(rowsB);
  parityRows[0].route = { intent: 'report_request', capability: 'static_page', artifact_mode: 'new' };
  parityRows[0].artifacts = [{ kind: 'static_page' }];
  parityRows[0].contract.route_sha256 = sha256(stableStringify(parityRows[0].route));
  parityRows[0].contract.immutable_component_sha256 = sha256(stableStringify({
    question_sha256: parityRows[0].contract.question_sha256,
    system_prompt_sha256: parityRows[0].contract.system_prompt_sha256,
    answer_policy_sha256: parityRows[0].contract.answer_policy_sha256,
    provider_config_sha256: parityRows[0].contract.provider_config_sha256,
    selected_scope_sha256: parityRows[0].contract.selected_scope_sha256,
    permission_sha256: parityRows[0].contract.permission_sha256,
    evaluation_kind_sha256: parityRows[0].contract.evaluation_kind_sha256,
    runtime_input_sha256: parityRows[0].contract.runtime_input_sha256,
    action_catalog_sha256: parityRows[0].contract.action_catalog_sha256,
    route_sha256: parityRows[0].contract.route_sha256,
  }));
  parityRows[0].contract.provider_call_count += 1;
  parityRows[0].contract.artifact_delta_count += 1;
  const parityArm = evaluateArm(
    'negative_parity', fixtures, loadedFromValues(parityRows), null, 'B', runtimeInputs,
  );
  const parityComparison = buildComparison('negative_parity_vs_A', fixtures, control, parityArm);
  const slowRows = structuredClone(rowsB);
  for (const row of slowRows) row.latency_ms += 500;
  const slowArm = evaluateArm(
    'negative_latency', fixtures, loadedFromValues(slowRows), null, 'B', runtimeInputs,
  );
  const slowComparison = buildComparison('negative_latency_vs_A', fixtures, control, slowArm);

  const snapshotMismatchRows = structuredClone(rowsB);
  snapshotMismatchRows[0].candidate_snapshot_sha256 = sha256('different-candidate-snapshot');
  const snapshotMismatchArm = evaluateArm(
    'negative_snapshot_mismatch',
    fixtures,
    loadedFromValues(snapshotMismatchRows),
    null,
    'B',
    runtimeInputs,
  );
  const snapshotMismatchComparison = buildComparison(
    'negative_snapshot_mismatch_vs_A',
    fixtures,
    control,
    snapshotMismatchArm,
  );
  const producerHashTamperRows = structuredClone(rowsB);
  producerHashTamperRows[0].runtime_input_sha256 = sha256('producer-runtime-input-tamper');
  producerHashTamperRows[0].contract.runtime_input_sha256 = producerHashTamperRows[0].runtime_input_sha256;
  producerHashTamperRows[0].permission_sha256 = sha256('producer-permission-tamper');
  producerHashTamperRows[0].contract.permission_sha256 = producerHashTamperRows[0].permission_sha256;
  producerHashTamperRows[0].contract.question_sha256 = sha256('producer-question-tamper');
  refreshImmutableComponentHash(producerHashTamperRows[0]);
  const producerHashTamperArm = evaluateArm(
    'negative_producer_hash_tamper',
    fixtures,
    loadedFromValues(producerHashTamperRows),
    null,
    'B',
    runtimeInputs,
  );
  const missingProducerRuntimeFileHashRows = structuredClone(rowsB);
  delete missingProducerRuntimeFileHashRows[0].producer.runtime_input_file_sha256;
  const missingProducerRuntimeFileHashArm = evaluateArm(
    'negative_missing_producer_runtime_input_file_hash',
    fixtures,
    loadedFromValues(missingProducerRuntimeFileHashRows),
    null,
    'B',
    runtimeInputs,
  );
  const mismatchedProducerRuntimeFileHashRows = structuredClone(rowsB);
  mismatchedProducerRuntimeFileHashRows[0].producer.runtime_input_file_sha256 = sha256(
    'different-runtime-input-file-bytes',
  );
  const mismatchedProducerRuntimeFileHashArm = evaluateArm(
    'negative_mismatched_producer_runtime_input_file_hash',
    fixtures,
    loadedFromValues(mismatchedProducerRuntimeFileHashRows),
    null,
    'B',
    runtimeInputs,
  );
  const missingProducerRuntimeCanonicalHashRows = structuredClone(rowsB);
  delete missingProducerRuntimeCanonicalHashRows[0].producer.runtime_input_canonical_sha256;
  const missingProducerRuntimeCanonicalHashArm = evaluateArm(
    'negative_missing_producer_runtime_input_canonical_hash',
    fixtures,
    loadedFromValues(missingProducerRuntimeCanonicalHashRows),
    null,
    'B',
    runtimeInputs,
  );
  const mismatchedProducerRuntimeCanonicalHashRows = structuredClone(rowsB);
  mismatchedProducerRuntimeCanonicalHashRows[0].producer.runtime_input_canonical_sha256 = sha256(
    'different-runtime-input-canonical-material',
  );
  const mismatchedProducerRuntimeCanonicalHashArm = evaluateArm(
    'negative_mismatched_producer_runtime_input_canonical_hash',
    fixtures,
    loadedFromValues(mismatchedProducerRuntimeCanonicalHashRows),
    null,
    'B',
    runtimeInputs,
  );
  const unboundProducerGitRows = structuredClone(rowsB);
  delete unboundProducerGitRows[0].producer.git_head_source;
  unboundProducerGitRows[0].producer.worktree_clean = false;
  const unboundProducerGitArm = evaluateArm(
    'negative_unbound_producer_git_state',
    fixtures,
    loadedFromValues(unboundProducerGitRows),
    null,
    'B',
    runtimeInputs,
  );
  const scopeMismatchRows = structuredClone(rowsB);
  scopeMismatchRows[0].scope_sha256 = sha256('different-selected-scope');
  scopeMismatchRows[0].contract.selected_scope_sha256 = scopeMismatchRows[0].scope_sha256;
  scopeMismatchRows[0].contract.immutable_component_sha256 = sha256(stableStringify({
    question_sha256: scopeMismatchRows[0].contract.question_sha256,
    system_prompt_sha256: scopeMismatchRows[0].contract.system_prompt_sha256,
    answer_policy_sha256: scopeMismatchRows[0].contract.answer_policy_sha256,
    provider_config_sha256: scopeMismatchRows[0].contract.provider_config_sha256,
    selected_scope_sha256: scopeMismatchRows[0].contract.selected_scope_sha256,
    permission_sha256: scopeMismatchRows[0].contract.permission_sha256,
    evaluation_kind_sha256: scopeMismatchRows[0].contract.evaluation_kind_sha256,
    runtime_input_sha256: scopeMismatchRows[0].contract.runtime_input_sha256,
    action_catalog_sha256: scopeMismatchRows[0].contract.action_catalog_sha256,
    route_sha256: scopeMismatchRows[0].contract.route_sha256,
  }));
  const scopeMismatchArm = evaluateArm(
    'negative_scope_mismatch',
    fixtures,
    loadedFromValues(scopeMismatchRows),
    null,
    'B',
    runtimeInputs,
  );
  const scopeMismatchComparison = buildComparison(
    'negative_scope_mismatch_vs_A',
    fixtures,
    control,
    scopeMismatchArm,
  );
  const semanticSnapshotMismatchRows = structuredClone(rowsB);
  semanticSnapshotMismatchRows[0].semantic_snapshot_sha256 = sha256('different-semantic-snapshot');
  const semanticSnapshotMismatchArm = evaluateArm(
    'negative_semantic_snapshot_mismatch',
    fixtures,
    loadedFromValues(semanticSnapshotMismatchRows),
    null,
    'B',
    runtimeInputs,
  );
  const semanticSnapshotMismatchComparison = buildComparison(
    'negative_semantic_snapshot_mismatch_vs_A',
    fixtures,
    control,
    semanticSnapshotMismatchArm,
  );

  const answerRows = structuredClone(rowsB);
  answerRows[0].answer = '';
  const answerArm = evaluateArm(
    'negative_offline_answer', fixtures, loadedFromValues(answerRows), null, 'B', runtimeInputs,
  );

  const oversizedRows = structuredClone(rowsB);
  for (const row of oversizedRows) row.contract.provider_input_bytes += 500;
  const oversizedArm = evaluateArm(
    'negative_provider_input_size', fixtures, loadedFromValues(oversizedRows), null, 'B', runtimeInputs,
  );
  const oversizedComparison = buildComparison(
    'negative_provider_input_size_vs_A',
    fixtures,
    control,
    oversizedArm,
  );
  const missingAnswerEvaluation = unavailableAnswerEvaluation();
  const positiveAnswerReceipt = syntheticAnswerReceipt(fixtures, { A: rowsA, B: rowsB, C: rowsC });
  const positiveAnswerEvaluation = evaluateAnswerReceipt(
    positiveAnswerReceipt,
    fixtures,
    [control, rerank, supplement],
  );
  const routeParityAnswerReceipt = structuredClone(positiveAnswerReceipt);
  routeParityAnswerReceipt.cases.find((row) => row.arm === 'B').route = {
    observed_route: 'different_self_test_model_route',
  };
  const routeParityAnswerEvaluation = evaluateAnswerReceipt(
    routeParityAnswerReceipt,
    fixtures,
    [control, rerank, supplement],
  );
  const providerlessAnswerReceipt = structuredClone(positiveAnswerReceipt);
  providerlessAnswerReceipt.provider_execution.status = 'not_run';
  providerlessAnswerReceipt.provider_execution.invocation_count = 0;
  const providerlessAnswerEvaluation = evaluateAnswerReceipt(
    providerlessAnswerReceipt,
    fixtures,
    [control, rerank, supplement],
  );
  const unlinkedAnswerReceipt = structuredClone(positiveAnswerReceipt);
  unlinkedAnswerReceipt.cases[0].provider_input_sha256 = sha256('different-provider-input');
  const unlinkedAnswerEvaluation = evaluateAnswerReceipt(
    unlinkedAnswerReceipt,
    fixtures,
    [control, rerank, supplement],
  );
  const unsafeAnswerReceipt = structuredClone(positiveAnswerReceipt);
  const unsafeB = unsafeAnswerReceipt.cases.find((row) => row.arm === 'B');
  unsafeB.unsupported_claim_count = 1;
  unsafeB.internal_field_leak_count = 1;
  unsafeB.naturalness.material_loss = true;
  for (const row of unsafeAnswerReceipt.cases.filter((item) => item.arm === 'B')) {
    row.supported_citation_claim_count = 0;
    row.citation_accuracy = 0;
    row.repeated_opening_5gram_overlap = 0.9;
  }
  const unsafeAnswerEvaluation = evaluateAnswerReceipt(
    unsafeAnswerReceipt,
    fixtures,
    [control, rerank, supplement],
  );
  const retrievalOnlyCliGate = determineCommandGate({
    mode: 'results',
    fixtureReady: true,
    evaluatorSelfTestReady: false,
    retrievalReady: true,
    answerEvaluation: missingAnswerEvaluation,
    requireRetrievalMetrics: true,
    requireMetrics: false,
    answerReceiptSupplied: false,
  });
  const fullCliGateWithoutAnswer = determineCommandGate({
    mode: 'results',
    fixtureReady: true,
    evaluatorSelfTestReady: false,
    retrievalReady: true,
    answerEvaluation: missingAnswerEvaluation,
    requireRetrievalMetrics: true,
    requireMetrics: true,
    answerReceiptSupplied: false,
  });

  const checks = {
    runtime_input_fixture_ready: runtimeInputs.ready,
    runtime_input_physically_excludes_oracle_and_conversation_fields:
      forbiddenRuntimeInputFields(runtimeValues).length === 0,
    runtime_input_forbidden_oracle_field_rejected:
      !forbiddenRuntimeInputs.ready
      && forbiddenRuntimeInputs.issues.some((issue) => (
        issue.code === 'oracle_or_conversation_field_forbidden'
      )),
    runtime_input_candidate_manifest_recomputed:
      candidateMutatedRuntimeInputs.ready
      && !candidateManifestMismatchArm.ready
      && candidateManifestMismatchArm.arm_contract_issues.some((issue) => (
        issue.code === 'runtime_input_candidate_snapshot_sha256_mismatch'
        || issue.code === 'runtime_input_candidate_ids_mismatch'
      )),
    runtime_input_semantic_manifest_recomputed:
      semanticMutatedRuntimeInputs.ready
      && !semanticManifestMismatchArm.ready
      && semanticManifestMismatchArm.arm_contract_issues.some((issue) => (
        issue.code === 'runtime_input_semantic_snapshot_sha256_mismatch'
      )),
    runtime_input_question_fixture_mismatch_rejected:
      !questionMutatedRuntimeInputs.ready
      && questionMutatedRuntimeInputs.issues.some((issue) => (
        issue.code === 'runtime_question_fixture_mismatch'
      )),
    runtime_input_permission_inferred_rejected:
      !permissionMutatedRuntimeInputs.ready
      && permissionMutatedRuntimeInputs.issues.some((issue) => (
        issue.code === 'permission_evidence_classes_invalid'
      )),
    producer_runtime_permission_question_hash_tamper_rejected:
      !producerHashTamperArm.ready
      && ['runtime_input_runtime_input_sha256_mismatch', 'runtime_input_permission_sha256_mismatch',
        'runtime_input_question_hash_mismatch'].every((code) => (
        producerHashTamperArm.arm_contract_issues.some((issue) => issue.code === code)
      )),
    evaluator_runtime_input_raw_file_hash_is_actual_loaded_content:
      runtimeInputs.runtime_input_file_sha256 === loadedRuntimeInputs.contentSha256,
    evaluator_runtime_input_canonical_hash_is_independently_recomputed:
      runtimeInputs.runtime_input_canonical_sha256 === loadedRuntimeInputs.canonicalSha256
      && runtimeInputs.runtime_input_canonical_sha256
        === sha256(stableStringify(loadedRuntimeInputs.rows.map((row) => row.value))),
    missing_producer_runtime_input_file_hash_rejected:
      !missingProducerRuntimeFileHashArm.ready
      && !missingProducerRuntimeFileHashArm.runtime_input_link_ready
      && missingProducerRuntimeFileHashArm.invalid_rows.some((row) => (
        row.errors.includes('producer_runtime_input_file_sha256_invalid')
      )),
    mismatched_producer_runtime_input_file_hash_rejected:
      !mismatchedProducerRuntimeFileHashArm.ready
      && !mismatchedProducerRuntimeFileHashArm.runtime_input_link_ready
      && mismatchedProducerRuntimeFileHashArm.invalid_rows.some((row) => (
        row.errors.includes('producer_runtime_input_file_sha256_mismatch')
      )),
    missing_producer_runtime_input_canonical_hash_rejected:
      !missingProducerRuntimeCanonicalHashArm.ready
      && !missingProducerRuntimeCanonicalHashArm.runtime_input_link_ready
      && missingProducerRuntimeCanonicalHashArm.invalid_rows.some((row) => (
        row.errors.includes('producer_runtime_input_canonical_sha256_invalid')
      )),
    mismatched_producer_runtime_input_canonical_hash_rejected:
      !mismatchedProducerRuntimeCanonicalHashArm.ready
      && !mismatchedProducerRuntimeCanonicalHashArm.runtime_input_link_ready
      && mismatchedProducerRuntimeCanonicalHashArm.invalid_rows.some((row) => (
        row.errors.includes('producer_runtime_input_canonical_sha256_mismatch')
      )),
    producer_git_head_requires_actual_clean_repository_binding:
      !unboundProducerGitArm.ready
      && unboundProducerGitArm.invalid_rows.some((row) => (
        row.errors.includes('producer_receipt_invalid')
      )),
    fixture_single_dataset_scope_is_exact:
      !invalidSingleScope.ready
      && invalidSingleScope.issues.some((issue) => (
        issue.code === 'single_dataset_scope_must_match_dataset_key'
      )),
    fixture_multi_dataset_selected_scope_requires_two:
      !invalidCrossScope.ready
      && invalidCrossScope.issues.some((issue) => (
        issue.code === 'multi_dataset_selected_scope_requires_two_datasets'
      )),
    fixture_inferred_evidence_class_rejected:
      !invalidEvidenceClass.ready
      && invalidEvidenceClass.issues.some((issue) => (
        issue.code === 'allowed_evidence_classes_must_be_confirmed_observed'
      )),
    positive_three_arm_retrieval_evaluator_ready:
      control.ready && comparisonB.ready && comparisonC.ready && comparisonCvsB.ready,
    grounding_19_of_20_meets_preregistered_coverage:
      grounding19Of20.ready
      && grounding19Of20.grounding.target_case_count === 20
      && grounding19Of20.grounding.measured_target_case_count === 19
      && grounding19Of20.grounding.target_case_coverage_rate === 0.95
      && grounding19Of20.grounding.unmeasured_target_case_ids.length === 1
      && grounding19Of20.grounding.target_node_provenance_resolution_rate >= 0.9
      && grounding19Of20.grounding.unsafe_node_use_count === 0,
    grounding_17_of_20_fails_preregistered_coverage:
      !grounding17Of20.ready
      && grounding17Of20.grounding.target_case_count === 20
      && grounding17Of20.grounding.measured_target_case_count === 17
      && grounding17Of20.grounding.target_case_coverage_rate === 0.85
      && grounding17Of20.grounding.unmeasured_target_case_ids.length === 3,
    negative_control_grounding_remains_strict:
      !strictNegativeGrounding.ready
      && strictNegativeGrounding.grounding.issues.some((issue) => (
        issue.case_id === negativeFixture.id
          && issue.code === 'negative_control_grounding_must_be_not_applicable'
      )),
    rust_offline_retrieval_allows_not_run_route_with_zero_side_effect_parity:
      offlineNoRouteControl.ready
      && offlineNoRouteRerank.ready
      && offlineNoRouteSupplement.ready
      && offlineNoRouteComparisonB.ready
      && offlineNoRouteComparisonC.ready
      && offlineNoRouteComparisonCvsB.ready,
    rerank_candidate_survives_safe_supplement_without_incremental_gain:
      noGainCandidateReadiness.retrieval_ready
      && noGainCandidateReadiness.rerank_candidate_ready
      && noGainCandidateReadiness.supplement_safety_ready
      && !noGainCandidateReadiness.supplement_candidate_ready
      && noGainCandidateReadiness.retrieval_candidate === 'rerank',
    independent_feature_off_baseline_is_required_for_any_candidate:
      !unmeasuredBaselineCandidateReadiness.retrieval_ready
      && !unmeasuredBaselineCandidateReadiness.feature_off_baseline_independently_measured
      && unmeasuredBaselineCandidateReadiness.feature_off_baseline_status
        === 'not_independently_measured'
      && unmeasuredBaselineCandidateReadiness.retrieval_candidate === null,
    unsafe_supplement_still_blocks_retrieval_readiness:
      !unsafeCandidateReadiness.retrieval_ready
      && !unsafeCandidateReadiness.supplement_safety_ready,
    full_answer_checks_route_parity_without_prescribing_route:
      !routeParityAnswerEvaluation.ready
      && !routeParityAnswerEvaluation.gates.answer_route_parity
      && routeParityAnswerEvaluation.issues.some((issue) => (
        issue.code === 'answer_route_parity_mismatch'
      )),
    guard_evaluation_kinds_never_emit_document_or_graph_hits:
      [rowsA, rowsB, rowsC].every((rows) => rows
        .filter((row) => row.evaluation_kind !== 'retrieval_evidence')
        .every((row) => row.hits.every((hit) => hit.selection_kind === 'baseline'))),
    missing_result_detected: missing.missing_case_ids.length === 1 && !missing.ready,
    duplicate_result_detected: duplicate.duplicate_case_ids.length === 1 && !duplicate.ready,
    invalid_result_detected: invalid.invalid_rows.length > 0 && !invalid.ready,
    permission_leak_detected: leak.permission_leak_count > 0 && !leak.ready,
    offline_route_not_run_consistency_artifact_provider_guard_detected:
      !parityComparison.gates.offline_route_not_run_consistency
      && !parityComparison.gates.artifact_parity
      && !parityComparison.gates.provider_parity,
    latency_regression_detected: !slowComparison.gates.latency_absolute_gate
      && !slowComparison.gates.latency_relative_gate,
    candidate_snapshot_mismatch_detected:
      !snapshotMismatchComparison.gates.same_candidate_snapshot_and_scope,
    selected_scope_mismatch_detected:
      !scopeMismatchArm.ready
      && !scopeMismatchComparison.gates.same_candidate_snapshot_and_scope,
    semantic_snapshot_mismatch_detected:
      !semanticSnapshotMismatchComparison.gates.same_candidate_snapshot_and_scope,
    offline_answer_rejected: answerArm.invalid_rows.length > 0 && !answerArm.ready,
    provider_input_size_regression_detected: !oversizedComparison.gates.provider_input_size_gate,
    missing_answer_receipt_is_not_full_evidence: !missingAnswerEvaluation.ready,
    retrieval_only_cli_allows_answer_not_run:
      retrievalOnlyCliGate.command_ready
      && !retrievalOnlyCliGate.full_evidence_ready
      && retrievalOnlyCliGate.acceptance_status === 'retrieval_ready_answer_not_run',
    full_cli_rejects_answer_not_run:
      !fullCliGateWithoutAnswer.command_ready
      && !fullCliGateWithoutAnswer.full_evidence_ready
      && !fullCliGateWithoutAnswer.decision_eligible,
    answer_receipt_v2_stays_fail_closed_without_independent_case_citation_and_legacy_receipts:
      !positiveAnswerEvaluation.ready
      && !positiveAnswerEvaluation.gates.independent_case_citation_and_legacy_corpus_receipts
      && positiveAnswerEvaluation.issues.some((issue) => (
        issue.code
          === 'answer_receipt_v2_not_promotion_eligible_missing_independent_case_citation_and_legacy_corpus_receipts'
      )),
    providerless_answer_receipt_rejected:
      !providerlessAnswerEvaluation.ready
      && !providerlessAnswerEvaluation.gates.provider_execution_completed,
    unlinked_provider_input_rejected:
      !unlinkedAnswerEvaluation.ready
      && unlinkedAnswerEvaluation.issues.some((issue) => (
        issue.code === 'answer_provider_input_link_mismatch'
      )),
    unsupported_citation_naturalness_regressions_detected:
      !unsafeAnswerEvaluation.gates.unsupported_false_claims_zero
      && !unsafeAnswerEvaluation.gates.internal_field_leakage_zero
      && !unsafeAnswerEvaluation.gates.citation_accuracy_noninferior
      && !unsafeAnswerEvaluation.gates.repeated_opening_5gram_within_limit
      && !unsafeAnswerEvaluation.gates.blind_naturalness_no_material_loss,
    per_category_and_holdout_metrics_recorded:
      comparisonB.gates.per_category_coverage_complete
      && comparisonB.gates.holdout_coverage_complete
      && comparisonB.gates.holdout_quality_gate,
  };
  if (!Object.values(checks).every(Boolean)) {
    throw new Error(`semantic supply evaluator self-test failed: ${Object.entries(checks).filter(([, passed]) => !passed).map(([name]) => name).join(',')}`);
  }
  return { control, rerank, supplement, comparisonB, comparisonC, comparisonCvsB, checks };
}

function markdownForReport(report) {
  const lines = [
    '# Semantic Supply A/B/C Smoke',
    '',
    `- Command gate: ${report.command_ready ? 'passed' : 'failed'}`,
    `- Acceptance status: ${report.acceptance_status}`,
    `- Mode: ${report.mode}`,
    `- Independent runtime input: ${report.runtime_input.ready ? 'ready' : 'not ready'}`,
    `- Retrieval evidence: ${report.retrieval_ready ? 'ready' : 'not ready'}`,
    `- Retrieval candidate: ${report.retrieval_candidate || 'none'}`,
    `- Rerank candidate: ${report.metrics.candidate_readiness?.rerank_candidate_ready ? 'ready' : 'not ready'}`,
    `- Supplement safety: ${report.metrics.candidate_readiness?.supplement_safety_ready ? 'ready' : 'not ready'}`,
    `- Supplement candidate: ${report.metrics.candidate_readiness?.supplement_candidate_ready ? 'ready' : 'not ready'}`,
    `- Answer evaluation: ${report.answer_evaluation.status}`,
    `- Full evidence: ${report.full_evidence_ready ? 'ready' : 'not ready'}`,
    `- Decision eligible: ${report.decision_eligible}`,
    `- Fixture cases: ${report.fixture.case_count}`,
    `- Graph-target cases: ${report.fixture.graph_target_case_count}`,
    `- Negative controls: ${report.fixture.negative_control_case_count}`,
    `- Development / holdout: ${report.fixture.splits.development} / ${report.fixture.splits.holdout}`,
    `- Retrieval metrics required: ${report.metrics.retrieval_required}`,
    `- Full metrics required: ${report.metrics.full_required}`,
    `- Retrieval rows recorded: ${report.metrics.recorded}`,
    '',
    '## Fixture categories',
    '',
    '| Category | Count |',
    '| --- | ---: |',
    ...Object.entries(report.fixture.categories).sort().map(([category, count]) => `| ${category} | ${count} |`),
    '',
    '## Independent runtime input',
    '',
    `- Dataset records: ${report.runtime_input.dataset_record_count}`,
    `- Case records: ${report.runtime_input.case_record_count}`,
    `- File SHA-256: ${report.runtime_input.runtime_input_file_sha256 || 'not-recorded'}`,
  ];
  for (const issue of report.runtime_input.issues || []) {
    lines.push(`- issue: ${issue.case_id || `line-${issue.line || 'unknown'}`}:${issue.code}`);
  }
  if (report.metrics.recorded) {
    lines.push(
      '',
      report.mode === 'evaluator-self-test'
        ? '## Synthetic evaluator arm metrics (not experiment evidence)'
        : '## Arm retrieval metrics',
      '',
      '| Arm | Ready | Recall@20 | MRR@20 | Precision@5 | Target MRR | Target P@5 | Top5 overlap | p95 ms | Leaks |',
      '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |',
    );
    for (const arm of report.metrics.arms) {
      lines.push(`| ${arm.arm} | ${arm.ready} | ${arm.metrics.recall_at_20} | ${arm.metrics.mrr_at_20} | ${arm.metrics.precision_at_5} | ${arm.targeted_metrics.mrr_at_20} | ${arm.targeted_metrics.precision_at_5} | ${arm.top5_source_overlap} | ${arm.p95_latency_ms} | ${arm.permission_leak_count} |`);
    }
    lines.push('', '## Comparison gates', '');
    for (const comparison of report.metrics.comparisons) {
      lines.push(`### ${comparison.comparison}`, '', `- Ready: ${comparison.ready}`);
      for (const [name, passed] of Object.entries(comparison.gates)) {
        lines.push(`- ${name}: ${passed ? 'passed' : 'failed'}`);
      }
      lines.push('');
    }
  } else {
    lines.push('', 'Metrics were not recorded. Supply all A/B/C result JSONL files to evaluate quality gates.', '');
  }
  lines.push('', '## Answer-evaluation gates', '');
  for (const [name, passed] of Object.entries(report.answer_evaluation.gates)) {
    lines.push(`- ${name}: ${passed ? 'passed' : 'failed'}`);
  }
  for (const issue of report.answer_evaluation.issues || []) {
    lines.push(`- issue: ${issue.code}`);
  }
  lines.push('');
  if (report.self_test) {
    lines.push(
      '## Evaluator self-test (not experiment evidence)',
      '',
      '- Provider execution: not performed',
      '- Promotion decision: ineligible',
      '',
    );
    for (const [name, passed] of Object.entries(report.self_test.checks)) {
      lines.push(`- ${name}: ${passed ? 'passed' : 'failed'}`);
    }
    lines.push('');
  }
  if (report.fixture.issues.length > 0) {
    lines.push('## Fixture issues', '');
    for (const issue of report.fixture.issues) lines.push(`- ${issue.case_id || `line-${issue.line || 'unknown'}`}: ${issue.code}`);
    lines.push('');
  }
  return `${lines.join('\n')}\n`;
}

async function writeReport(args, report) {
  await mkdir(args.outputDir, { recursive: true });
  const timestamp = new Date().toISOString().replaceAll(/[-:.]/gu, '').replace('Z', 'Z');
  const basename = `semantic-supply-ab-${timestamp}-${process.pid}`;
  const jsonPath = resolve(args.outputDir, `${basename}.json`);
  const markdownPath = resolve(args.outputDir, `${basename}.md`);
  await writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  await writeFile(markdownPath, markdownForReport(report), 'utf8');
  return { jsonPath, markdownPath };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(usage());
    return;
  }
  const loadedFixture = await loadJsonl(args.fixturePath, 'id');
  const fixture = validateFixtures(loadedFixture);
  const hasResults = Boolean(args.aResultsPath && args.bResultsPath && args.cResultsPath);
  const mode = args.selfTest ? 'evaluator-self-test' : hasResults ? 'results' : 'fixture-only';
  let loadedRuntimeInputs = null;
  let runtimeInputs = {
    ready: false,
    source_path: null,
    schema_version: runtimeInputSchemaVersion,
    dataset_record_count: 0,
    case_record_count: 0,
    runtime_input_file_sha256: null,
    missing_case_ids: [],
    extra_case_ids: [],
    issues: [{ code: 'runtime_input_not_applicable' }],
    case_receipts: new Map(),
  };
  if (fixture.ready && (args.selfTest || hasResults)) {
    const runtimeInputPath = args.selfTest ? defaultRuntimeInputPath : args.runtimeInputPath;
    loadedRuntimeInputs = await loadJsonl(runtimeInputPath, null);
    runtimeInputs = validateRuntimeInputs(loadedRuntimeInputs, fixture.cases, runtimeInputPath);
  }
  let evaluatorSelfTestReady = false;
  let retrievalReady = false;
  let candidateReadiness = {
    feature_off_baseline_status: 'not_independently_measured',
    feature_off_baseline_independently_measured: false,
    retrieval_ready: false,
    rerank_candidate_ready: false,
    supplement_safety_ready: false,
    supplement_candidate_ready: false,
    retrieval_candidate: null,
  };
  let answerEvaluation = unavailableAnswerEvaluation(
    mode === 'evaluator-self-test'
      ? 'evaluator_self_test_does_not_produce_answer_evidence'
      : mode === 'fixture-only'
        ? 'answer_evaluation_not_applicable_without_results'
        : 'answer_receipt_not_supplied',
  );
  let metrics = {
    retrieval_required: args.requireRetrievalMetrics,
    full_required: args.requireMetrics,
    recorded: false,
    retrieval_gate_evaluator_passed: false,
      retrieval_ready: false,
      candidate_readiness: candidateReadiness,
      arms: [],
    comparisons: [],
  };
  let selfTest = null;

  if (fixture.ready && args.selfTest) {
    if (!runtimeInputs.ready) {
      throw new Error('semantic supply evaluator self-test runtime input fixture is invalid');
    }
    const evaluated = runSelfTest(fixture.cases, runtimeInputs, loadedRuntimeInputs);
    evaluatorSelfTestReady = Object.values(evaluated.checks).every(Boolean);
    metrics = {
      retrieval_required: false,
      full_required: false,
      recorded: true,
      retrieval_gate_evaluator_passed:
        evaluated.control.ready && evaluated.comparisonB.ready && evaluated.comparisonC.ready
        && evaluated.comparisonCvsB.ready,
      retrieval_ready: false,
      candidate_readiness: candidateReadiness,
      arms: [evaluated.control, evaluated.rerank, evaluated.supplement].map(safeArmReport),
      comparisons: [evaluated.comparisonB, evaluated.comparisonC, evaluated.comparisonCvsB],
    };
    selfTest = {
      status: evaluatorSelfTestReady ? 'passed' : 'failed',
      evidence_kind: 'synthetic_evaluator_only',
      provider_execution: 'not_run',
      decision_eligible: false,
      checks: evaluated.checks,
    };
  } else if (fixture.ready && hasResults) {
    const [loadedA, loadedB, loadedC] = await Promise.all([
      loadJsonl(args.aResultsPath, 'case_id'),
      loadJsonl(args.bResultsPath, 'case_id'),
      loadJsonl(args.cResultsPath, 'case_id'),
    ]);
    const control = evaluateArm(
      'A_control', fixture.cases, loadedA, args.aResultsPath, 'A', runtimeInputs,
    );
    const rerank = evaluateArm(
      'B_graph_rerank', fixture.cases, loadedB, args.bResultsPath, 'B', runtimeInputs,
    );
    const supplement = evaluateArm(
      'C_graph_rerank_plus_evidence',
      fixture.cases,
      loadedC,
      args.cResultsPath,
      'C',
      runtimeInputs,
    );
    const comparisonB = buildComparison('B_vs_A', fixture.cases, control, rerank);
    const comparisonC = buildComparison('C_vs_A', fixture.cases, control, supplement);
    const comparisonCvsB = buildComparison('C_vs_B', fixture.cases, rerank, supplement, 'supplement');
    candidateReadiness = retrievalCandidateReadiness({
      runtimeInputReady: runtimeInputs.ready,
      comparisonB,
      comparisonC,
      comparisonCvsB,
    });
    retrievalReady = candidateReadiness.retrieval_ready;
    if (args.answerReceiptPath) {
      const receipt = await loadJsonObject(args.answerReceiptPath);
      answerEvaluation = evaluateAnswerReceipt(
        receipt,
        fixture.cases,
        [control, rerank, supplement],
        args.answerReceiptPath,
      );
    }
    metrics = {
      retrieval_required: args.requireRetrievalMetrics,
      full_required: args.requireMetrics,
      recorded: true,
      retrieval_gate_evaluator_passed: retrievalReady,
      retrieval_ready: retrievalReady,
      candidate_readiness: candidateReadiness,
      arms: [control, rerank, supplement].map(safeArmReport),
      comparisons: [comparisonB, comparisonC, comparisonCvsB],
    };
  }

  const commandGate = determineCommandGate({
    mode,
    fixtureReady: fixture.ready,
    evaluatorSelfTestReady,
    retrievalReady,
    answerEvaluation,
    requireRetrievalMetrics: args.requireRetrievalMetrics,
    requireMetrics: args.requireMetrics,
    answerReceiptSupplied: Boolean(args.answerReceiptPath),
  });
  const commandReady = commandGate.command_ready;
  const fullEvidenceReady = commandGate.full_evidence_ready;
  const decisionEligible = commandGate.decision_eligible;
  const acceptanceStatus = commandGate.acceptance_status;
  const report = {
    smoke: 'semantic-supply-ab',
    schema_version: 'semantic-supply-ab-report.v2',
    mode,
    command_ready: commandReady,
    acceptance_status: acceptanceStatus,
    evaluator_self_test_ready: mode === 'evaluator-self-test' ? evaluatorSelfTestReady : null,
    retrieval_ready: retrievalReady,
    retrieval_candidate: candidateReadiness.retrieval_candidate,
    answer_ready: answerEvaluation.ready,
    full_evidence_ready: fullEvidenceReady,
    decision_eligible: decisionEligible,
    started_and_finished_at: new Date().toISOString(),
    fixture_path: safeRelativePath(args.fixturePath),
    output_policy: 'hashes_and_aggregate_metrics_only_no_prompts_answers_or_source_values',
    evidence_boundaries: {
      cross_dataset_relation_evidence: 'not_measured',
      link_snapshot_used: false,
      multi_dataset_selected_scope_case_count:
        fixture.categories.multi_dataset_selected_scope || 0,
      feature_off_baseline_status:
        candidateReadiness.feature_off_baseline_status,
      answer_receipt_v2_promotion_eligible: false,
      citation_binding_status: 'not_implemented',
      legacy_58_per_case_link_status: 'not_linked',
    },
    thresholds,
    fixture: {
      ready: fixture.ready,
      case_count: fixture.case_count,
      graph_target_case_count: fixture.graph_target_case_count,
      negative_control_case_count: fixture.negative_control_case_count,
      categories: fixture.categories,
      splits: fixture.splits,
      issues: fixture.issues,
    },
    runtime_input: safeRuntimeInputReport(runtimeInputs),
    metrics,
    answer_evaluation: answerEvaluation,
    self_test: selfTest,
  };
  const paths = await writeReport(args, report);
  if (args.pretty) console.log(JSON.stringify(report, null, 2));
  if (mode === 'evaluator-self-test') {
    console.log(`Semantic supply evaluator self-test: status=${selfTest?.status || 'failed'} runtime_input_ready=${runtimeInputs.ready} experiment_evidence=not_produced decision_eligible=false cases=${fixture.case_count}`);
  } else {
    console.log(`Semantic supply A/B/C smoke: command_gate=${commandReady ? 'passed' : 'failed'} mode=${mode} runtime_input_ready=${runtimeInputs.ready} retrieval_ready=${retrievalReady} answer_status=${answerEvaluation.status} full_evidence_ready=${fullEvidenceReady} decision_eligible=${decisionEligible} cases=${fixture.case_count}`);
  }
  console.log(`JSON report: ${paths.jsonPath}`);
  console.log(`Markdown report: ${paths.markdownPath}`);
  if (!report.command_ready) {
    if (args.requireRetrievalMetrics && !metrics.recorded) {
      throw new Error('semantic supply retrieval metrics are required but A/B/C v2 results were not supplied');
    }
    if (hasResults && !runtimeInputs.ready) {
      throw new Error('semantic supply independent runtime input did not pass validation');
    }
    if (args.requireMetrics && !retrievalReady) {
      throw new Error('full semantic supply metrics require retrieval evidence, but the retrieval gate did not pass');
    }
    if (args.requireMetrics && answerEvaluation.status === 'not_run') {
      throw new Error('full semantic supply metrics require answer evidence, but answer evaluation status is not_run');
    }
    throw new Error('semantic supply A/B/C smoke did not pass the requested fail-closed gate');
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
