#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';

const DEFAULT_OUTPUT_DIR = 'target/production-placeholder-readiness';

function parseArgs(argv) {
  const args = {
    envFiles: [],
    allowNotReady: false,
    jsonStdout: false,
    selfTest: false,
    outputDir: process.env.PRODUCTION_PLACEHOLDER_READINESS_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--env-file') {
      args.envFiles.push(requireValue(arg, next));
      index += 1;
    } else if (arg === '--allow-not-ready') {
      args.allowNotReady = true;
    } else if (arg === '--json-stdout') {
      args.jsonStdout = true;
    } else if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--output-dir') {
      args.outputDir = requireValue(arg, next);
      index += 1;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (!args.envFiles.length) {
    args.envFiles = ['/etc/aiv3/aiv3.env', '/etc/aiv3/minimax.env'];
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
  node scripts/smoke/production-placeholder-readiness.mjs \\
    --env-file /etc/aiv3/aiv3.env \\
    --env-file /etc/aiv3/minimax.env \\
    --allow-not-ready

Options:
  --env-file PATH      Parse a deployment env file without sourcing it. Can repeat.
  --allow-not-ready    Write/report readiness but exit 0 when placeholder risks remain.
  --json-stdout        Print redacted JSON to stdout instead of writing files.
  --self-test          Run deterministic safe fixtures.
  --output-dir DIR     Report directory.

The smoke prints only key names, booleans, labels, and value-presence metadata.
It never prints provider key values or raw env-file values.`);
}

function parseEnvLineValue(raw) {
  let value = String(raw || '').trim();
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    value = value.slice(1, -1);
  }
  return value;
}

function parseEnvText(text) {
  const parsed = {};
  for (const rawLine of String(text || '').split(/\r?\n/)) {
    let line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    if (line.startsWith('export ')) {
      line = line.slice('export '.length).trim();
    }
    const match = line.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/);
    if (!match) continue;
    parsed[match[1]] = parseEnvLineValue(match[2]);
  }
  return parsed;
}

function loadEnvFiles(paths) {
  const merged = {};
  const files = [];
  for (const path of paths) {
    if (!existsSync(path)) {
      files.push({ path, status: 'missing' });
      continue;
    }
    const parsed = parseEnvText(readFileSync(path, 'utf8'));
    Object.assign(merged, parsed);
    files.push({
      path,
      status: 'read',
      key_count: Object.keys(parsed).length,
      secret_values_printed: false,
    });
  }
  return { merged, files };
}

function headShort() {
  try {
    return execFileSync('git', ['rev-parse', '--short', 'HEAD'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
  } catch {
    return 'unknown';
  }
}

function valueLabel(value) {
  const normalized = String(value || '').trim();
  if (!normalized) return 'missing';
  if (normalized.toLowerCase().includes('placeholder')) return 'placeholder';
  return 'configured';
}

function checkDatasetOutputRuntime(env) {
  const mode = env.DATASET_OUTPUT_RUNTIME_MODE || '';
  const provider = env.DATASET_OUTPUT_RUNTIME_PROVIDER || '';
  const model = env.DATASET_OUTPUT_RUNTIME_MODEL || '';
  const checks = {
    mode_configured: Boolean(mode),
    mode_not_placeholder: Boolean(mode) && valueLabel(mode) !== 'placeholder',
    provider_configured: Boolean(provider),
    provider_not_placeholder: Boolean(provider) && valueLabel(provider) !== 'placeholder',
    model_configured: Boolean(model),
    model_not_placeholder: Boolean(model) && valueLabel(model) !== 'placeholder',
    model_not_default_placeholder: model !== 'placeholder-dataset-output-v1',
  };
  const ready = Object.values(checks).every(Boolean);
  const missing = [];
  if (!checks.mode_configured) missing.push('DATASET_OUTPUT_RUNTIME_MODE');
  if (!checks.provider_configured) missing.push('DATASET_OUTPUT_RUNTIME_PROVIDER');
  if (!checks.model_configured) missing.push('DATASET_OUTPUT_RUNTIME_MODEL');
  const placeholder = [];
  if (mode && !checks.mode_not_placeholder) placeholder.push('DATASET_OUTPUT_RUNTIME_MODE');
  if (provider && !checks.provider_not_placeholder) placeholder.push('DATASET_OUTPUT_RUNTIME_PROVIDER');
  if (model && (!checks.model_not_placeholder || !checks.model_not_default_placeholder)) {
    placeholder.push('DATASET_OUTPUT_RUNTIME_MODEL');
  }
  return {
    ready,
    checks,
    labels: {
      mode: valueLabel(mode),
      provider: valueLabel(provider),
      model: valueLabel(model),
    },
    missing_keys: missing,
    placeholder_keys: placeholder,
    raw_values_printed: false,
  };
}

function buildSelfTestReports() {
  const readyEnv = {
    DATASET_OUTPUT_RUNTIME_MODE: 'provider',
    DATASET_OUTPUT_RUNTIME_PROVIDER: 'rightcode',
    DATASET_OUTPUT_RUNTIME_MODEL: 'gpt-5.5',
  };
  const notReadyEnv = {
    DATASET_OUTPUT_RUNTIME_MODE: 'placeholder',
    DATASET_OUTPUT_RUNTIME_PROVIDER: 'placeholder',
    DATASET_OUTPUT_RUNTIME_MODEL: 'placeholder-dataset-output-v1',
  };
  const missingEnv = {};
  return [
    { name: 'provider_runtime_ready', result: checkDatasetOutputRuntime(readyEnv), expected_ready: true },
    { name: 'placeholder_runtime_not_ready', result: checkDatasetOutputRuntime(notReadyEnv), expected_ready: false },
    { name: 'missing_runtime_not_ready', result: checkDatasetOutputRuntime(missingEnv), expected_ready: false },
  ];
}

function inspectReportPlannerAst() {
  const file = 'crates/report-planner-worker/src/main.rs';
  if (!existsSync(file)) {
    return {
      ready: false,
      file,
      source_status: 'missing',
      known_replace_required: [
        {
          area: 'report-planner-worker',
          file,
          reason: 'report planner source file was not found for readiness inspection',
        },
      ],
    };
  }

  const source = readFileSync(file, 'utf8');
  const oldSkeletonMarkers = [
    'fn build_placeholder_ast',
    'planner skeleton',
    'Supporting Evidence',
    'schema_version": "0.1.0"',
  ];
  const oldSkeletonPresent = oldSkeletonMarkers.some((marker) => source.includes(marker));
  const deterministicPlannerPresent =
    source.includes('fn build_report_ast') &&
    source.includes('deterministic_business_template') &&
    source.includes('xinbai-functional-modular-template-20260604');

  if (oldSkeletonPresent || !deterministicPlannerPresent) {
    return {
      ready: false,
      file,
      source_status: oldSkeletonPresent ? 'generic_skeleton_present' : 'deterministic_planner_missing',
      known_replace_required: [
        {
          area: 'report-planner-worker',
          file,
          reason: oldSkeletonPresent
            ? 'generic report AST skeleton markers are still present'
            : 'deterministic business-template report AST planner was not detected',
        },
      ],
    };
  }

  return {
    ready: true,
    file,
    source_status: 'deterministic_business_template',
    known_replace_required: [],
  };
}

function markdownReport(report) {
  const lines = [
    '# Production Placeholder Readiness',
    '',
    `- Machine summary: ${report.summary?.ok ? 'passed' : 'failed'}`,
    `- Ready: ${report.ready}`,
    `- Pending: ${report.pending}`,
    `- Head: ${report.head}`,
    `- Generated: ${report.generated_at}`,
    `- Raw values printed: ${report.redaction.raw_env_values_printed}`,
    '',
    '## Dataset Output Runtime',
    '',
    `- Ready: ${report.dataset_output.ready}`,
    `- Mode label: ${report.dataset_output.labels.mode}`,
    `- Provider label: ${report.dataset_output.labels.provider}`,
    `- Model label: ${report.dataset_output.labels.model}`,
    `- Missing keys: ${report.dataset_output.missing_keys.join(', ') || 'none'}`,
    `- Placeholder keys: ${report.dataset_output.placeholder_keys.join(', ') || 'none'}`,
    '',
    '## Report Planner AST',
    '',
    `- Ready: ${report.report_planner.ready}`,
    `- Source status: ${report.report_planner.source_status}`,
    `- File: ${report.report_planner.file}`,
    '',
    '## Env Files',
    '',
    ...report.env_files.map((file) => `- ${file.path}: ${file.status}`),
  ];
  if (report.self_test_cases?.length) {
    lines.push('', '## Self Test Cases', '');
    for (const item of report.self_test_cases) {
      lines.push(`- ${item.name}: ready=${item.result.ready}, expected=${item.expected_ready}`);
    }
  }
  return `${lines.join('\n')}\n`;
}

function buildSummary(args, report) {
  const selfTestCases = Array.isArray(report.self_test_cases) ? report.self_test_cases : [];
  const selfTestPassed = selfTestCases.every((item) => item.result.ready === item.expected_ready);
  const missingKeyCount = report.dataset_output?.missing_keys?.length || 0;
  const placeholderKeyCount = report.dataset_output?.placeholder_keys?.length || 0;
  const knownReplaceCount = report.known_replace_required?.length || 0;
  const checks = {
    terminalStateIsConsistent: report.ready === !report.pending,
    readinessSatisfiedOrAllowed: report.ready === true || args.allowNotReady === true,
    datasetOutputEvaluated: Boolean(report.dataset_output?.checks)
      && typeof report.dataset_output.ready === 'boolean',
    reportPlannerInspected: Boolean(report.report_planner?.file)
      && typeof report.report_planner.ready === 'boolean',
    notReadyHasReason: report.ready === true
      || missingKeyCount > 0
      || placeholderKeyCount > 0
      || knownReplaceCount > 0,
    selfTestFixturesPassed: report.self_test !== true || selfTestPassed,
    noRawEnvValuesPrinted: report.redaction?.raw_env_values_printed === false
      && report.dataset_output?.raw_values_printed === false,
    noProviderKeyValuesPrinted: report.redaction?.provider_key_values_printed === false,
  };
  return {
    ok: Object.values(checks).every(Boolean),
    checks,
    ready: report.ready === true,
    pending: report.pending === true,
    not_ready_allowed: args.allowNotReady === true,
    self_test: report.self_test === true,
    dataset_output_ready: report.dataset_output?.ready === true,
    report_planner_ready: report.report_planner?.ready === true,
    missing_key_count: missingKeyCount,
    placeholder_key_count: placeholderKeyCount,
    known_replace_required_count: knownReplaceCount,
    self_test_case_count: selfTestCases.length,
    self_test_passed: selfTestPassed,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const generatedAt = new Date().toISOString();
  let env = {};
  let envFiles = [];
  let selfTestCases = [];
  if (args.selfTest) {
    selfTestCases = buildSelfTestReports();
    const passed = selfTestCases.every((item) => item.result.ready === item.expected_ready);
    env = {
      DATASET_OUTPUT_RUNTIME_MODE: 'provider',
      DATASET_OUTPUT_RUNTIME_PROVIDER: 'rightcode',
      DATASET_OUTPUT_RUNTIME_MODEL: 'gpt-5.5',
    };
    envFiles = [{ path: 'self-test-fixture', status: passed ? 'read' : 'failed' }];
  } else {
    const loaded = loadEnvFiles(args.envFiles);
    env = loaded.merged;
    envFiles = loaded.files;
  }
  const datasetOutput = checkDatasetOutputRuntime(env);
  const reportPlanner = inspectReportPlannerAst();
  const ready =
    datasetOutput.ready &&
    reportPlanner.ready &&
    (!args.selfTest || selfTestCases.every((item) => item.result.ready === item.expected_ready));
  const pending = !ready;
  const report = {
    schema: 'v3.production_placeholder_readiness.v1',
    ready,
    pending,
    head: headShort(),
    generated_at: generatedAt,
    env_files: envFiles,
    dataset_output: datasetOutput,
    report_planner: {
      ready: reportPlanner.ready,
      file: reportPlanner.file,
      source_status: reportPlanner.source_status,
    },
    known_replace_required: reportPlanner.known_replace_required,
    self_test: args.selfTest,
    self_test_cases: selfTestCases,
    redaction: {
      raw_env_values_printed: false,
      provider_key_values_printed: false,
    },
  };
  report.summary = buildSummary(args, report);
  report.ok = report.summary.ok;

  if (args.jsonStdout) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    await mkdir(args.outputDir, { recursive: true });
    const stamp = generatedAt.replace(/[-:]/g, '').replace(/\.\d+Z$/, 'Z');
    const jsonPath = join(args.outputDir, `production-placeholder-readiness-${stamp}.json`);
    const mdPath = join(args.outputDir, `production-placeholder-readiness-${stamp}.md`);
    await writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`);
    await writeFile(mdPath, markdownReport(report));
    console.log(JSON.stringify({
      ready: report.ready,
      pending: report.pending,
      dataset_output_ready: report.dataset_output.ready,
      report_planner_ready: report.report_planner.ready,
      missing_keys: report.dataset_output.missing_keys,
      placeholder_keys: report.dataset_output.placeholder_keys,
      ok: report.ok,
      report: jsonPath,
    }, null, 2));
    console.log(`summary=${mdPath}`);
  }

  if (!report.ok) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error?.stack || error?.message || String(error));
  process.exit(1);
});
