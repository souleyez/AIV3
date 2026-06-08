#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const DEFAULT_OUTPUT_DIR = 'target/video-ppt-no-live-rollup';

const COMMANDS = [
  {
    id: 'upload_main_syntax',
    description: 'main-site upload smoke syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/smoke/video-ppt-upload-main.mjs'],
  },
  {
    id: 'upload_main_self_test',
    description: 'main-site upload smoke offline artifact contract',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-upload-main.mjs', '--self-test'],
  },
  {
    id: 'upload_main_preflight',
    description: 'main-site upload smoke live gate preflight without network',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-upload-main.mjs', '--preflight'],
  },
  {
    id: 'external_video_ppt_syntax',
    description: 'third-party video PPT smoke syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/smoke/external-video-ppt.mjs'],
  },
  {
    id: 'external_video_ppt_self_test',
    description: 'third-party video PPT offline reply and deliverable contract',
    command: process.execPath,
    args: ['scripts/smoke/external-video-ppt.mjs', '--self-test'],
  },
  {
    id: 'external_video_ppt_preflight',
    description: 'third-party video PPT live gate preflight without network',
    command: process.execPath,
    args: ['scripts/smoke/external-video-ppt.mjs', '--preflight', '--allow-missing-bearer'],
  },
  {
    id: 'video_ppt_handoff_syntax',
    description: 'login-gated video handoff syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/smoke/video-ppt-handoff.mjs'],
  },
  {
    id: 'video_ppt_handoff_self_test',
    description: 'login-gated video handoff offline contract',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-handoff.mjs', '--self-test'],
  },
  {
    id: 'video_ppt_handoff_preflight',
    description: 'login-gated video handoff live gate preflight without network',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-handoff.mjs', '--preflight', '--allow-missing-bearer'],
  },
  {
    id: 'authorized_capture_syntax',
    description: 'authorized capture helper syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/capture-authorized-video.mjs'],
  },
  {
    id: 'authorized_capture_self_test',
    description: 'authorized capture helper authorization gates',
    command: process.execPath,
    args: ['scripts/capture-authorized-video.mjs', '--self-test'],
  },
  {
    id: 'quality_matrix_syntax',
    description: 'quality matrix syntax check',
    command: process.execPath,
    args: ['--check', 'scripts/smoke/video-ppt-quality-matrix.mjs'],
  },
  {
    id: 'quality_matrix_self_test',
    description: 'quality matrix deterministic self-test and review-risk regression',
    command: process.execPath,
    args: [
      'scripts/smoke/video-ppt-quality-matrix.mjs',
      '--self-test',
      '--output-dir',
      'target/video-ppt-no-live-rollup-quality-matrix',
    ],
  },
  {
    id: 'rust_format_check',
    description: 'Rust workspace formatting check',
    command: 'cargo',
    args: ['fmt', '--check'],
  },
  {
    id: 'media_worker_lib_tests',
    description: 'media-worker video PPT extraction library tests',
    command: 'cargo',
    args: ['test', '-p', 'media-worker', '--lib'],
  },
  {
    id: 'media_worker_offline_smoke_bin_check',
    description: 'media-worker offline video PPT smoke binary compile check',
    command: 'cargo',
    args: ['check', '-p', 'media-worker', '--bin', 'video_ppt_offline_smoke'],
  },
  {
    id: 'video_deliverables_validator_syntax',
    description: 'video deliverables validator syntax check',
    command: process.execPath,
    args: ['--check', 'tools/validate-video-deliverables.mjs'],
  },
  {
    id: 'video_deliverables_validator_tests',
    description: 'video deliverables validator test suite',
    command: 'npm',
    args: ['run', 'test:video-deliverables'],
  },
];

function parseArgs(argv) {
  const args = {
    selfTest: false,
    outputDir: process.env.VIDEO_PPT_NO_LIVE_ROLLUP_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    pretty: false,
    help: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--output-dir') {
      args.outputDir = requiredValue(argv, index += 1, arg);
    } else if (arg === '--pretty') {
      args.pretty = true;
    } else if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return args;
}

function requiredValue(argv, index, flag) {
  const value = argv[index];
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function usage() {
  return `Usage:
  npm run smoke:video-ppt-no-live-rollup -- --self-test [--pretty] [--output-dir target/video-ppt-no-live-rollup]

Checks:
  - runs video PPT upload self-test and preflight without network calls
  - runs third-party video PPT self-test and preflight without network calls
  - runs login-gated video handoff self-test and preflight without network calls
  - runs authorized-capture self-test without opening a browser or FFmpeg
  - runs quality matrix self-test, including review-risk regression
  - runs Rust formatting check
  - runs media-worker video PPT library tests and offline smoke binary check
  - runs video deliverables validator tests

Safety:
  - does not call DataMax live endpoints
  - does not download videos
  - does not upload files or create datasets/documents/runs
  - does not open browsers, record screens, or run FFmpeg capture
  - does not deploy, restart, or touch any server`;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(usage());
    return;
  }
  if (!args.selfTest) {
    throw new Error('--self-test is required for the no-live rollup');
  }
  const startedAt = new Date();
  const results = [];
  for (const command of COMMANDS) {
    process.stdout.write(`running ${command.id}... `);
    const result = runCommand(command);
    results.push(result);
    process.stdout.write(`${result.status}\n`);
  }
  const report = buildReport({ startedAt, results });
  validateReport(report);
  const reportPath = writeReport(args.outputDir, report, args.pretty);
  const failedCount = report.summary.failed_count;
  console.log([
    'OK video PPT no-live rollup:',
    `commands=${report.summary.command_count}`,
    `passed=${report.summary.passed_count}`,
    `failed=${failedCount}`,
    `report=${reportPath}`,
  ].join(' '));
  if (failedCount > 0) {
    process.exitCode = 1;
  }
}

function runCommand(command) {
  const started = Date.now();
  const child = spawnSync(command.command, command.args, {
    cwd: process.cwd(),
    env: {
      ...process.env,
      NO_COLOR: '1',
    },
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  const durationMs = Date.now() - started;
  const exitCode = typeof child.status === 'number' ? child.status : 1;
  const stdout = child.stdout || '';
  const stderr = child.stderr || '';
  return {
    id: command.id,
    description: command.description,
    status: exitCode === 0 ? 'passed' : 'failed',
    exit_code: exitCode,
    duration_ms: durationMs,
    command: displayCommand(command),
    stdout_line_count: lineCount(stdout),
    stderr_line_count: lineCount(stderr),
    evidence: exitCode === 0 ? extractCommandEvidence(command.id, stdout) : null,
    failure_excerpt: exitCode === 0
      ? null
      : sanitizeExcerpt(`${stderr}\n${stdout}`),
  };
}

function extractCommandEvidence(commandId, stdout) {
  if (commandId === 'upload_main_self_test') {
    return extractUploadMainSelfTestEvidence(stdout);
  }
  if (commandId === 'external_video_ppt_self_test') {
    return extractExternalVideoPptSelfTestEvidence(stdout);
  }
  if (commandId !== 'quality_matrix_self_test') {
    return null;
  }
  return extractQualityMatrixSelfTestEvidence(stdout);
}

function extractUploadMainSelfTestEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  const trigger = report.contract?.triggerClassifier || {};
  return {
    schema: 'v3.video_ppt_upload_main_rollup_evidence.v1',
    selected_scope_intent: report.contract?.selectedScopeIntent,
    candidate_source: report.contract?.candidateSource,
    deliverable_state: report.contract?.deliverableState,
    required_file_kind_count: report.contract?.requiredFileKinds?.length,
    supported_video_extension_count: report.contract?.supportedVideoExtensions?.length,
    trigger_fixture_schema: trigger.fixtureSchema,
    trigger_fixture_version: trigger.fixtureVersion,
    trigger_shared_fixture_case_count: trigger.sharedFixtureCaseCount,
    trigger_script_specific_case_count: trigger.scriptSpecificCaseCount,
    positive_prompt_count: trigger.positivePromptCount,
    negative_prompt_count: trigger.negativePromptCount,
    source_summary_redacted: report.contract?.sourceSummaryRedacted,
    download_validation_ok: report.contract?.downloadValidation?.ok,
    pptx_slide_count: report.contract?.downloadValidation?.pptxSlideCount,
    markdown_slide_heading_count: report.contract?.downloadValidation?.markdownSlideHeadingCount,
    network_calls_run: report.summary?.networkCallsRun,
    production_write_allowed: report.summary?.productionWriteAllowed,
    fixture_downloaded: report.summary?.fixtureDownloaded,
    upload_attempted: report.summary?.uploadAttempted,
    assistant_run_created: report.summary?.assistantRunCreated,
    source_urls_included: report.safety?.sourceUrlsIncluded,
    object_keys_included: report.safety?.objectKeysIncluded,
    cookies_included: report.safety?.cookiesIncluded,
    bearer_included: report.safety?.bearerIncluded,
    provider_payloads_included: report.safety?.providerPayloadsIncluded,
  };
}

function extractExternalVideoPptSelfTestEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  const trigger = report.contract?.triggerClassifier || {};
  return {
    schema: 'v3.external_video_ppt_rollup_evidence.v1',
    trigger_text_requests_video_ppt: report.contract?.triggerTextRequestsVideoPpt,
    default_prompt_guards_ordinary_video_to_ppt:
      report.contract?.defaultPromptGuardsAgainstOrdinaryVideoToPpt,
    requested_video_ppt_skill: Array.isArray(report.contract?.requestedSkillIds)
      ? report.contract.requestedSkillIds.includes('video_ppt_extraction')
      : false,
    expected_action: report.contract?.expectedAction,
    available_document_source_present: report.contract?.availableDocumentSourcePresent,
    available_document_external_ids_count: report.contract?.availableDocumentExternalIdsCount,
    dataset_external_ids_count: report.contract?.datasetExternalIdsCount,
    supported_video_extension_count: report.contract?.supportedVideoExtensions?.length,
    unsupported_non_video_extensions_rejected: report.contract?.unsupportedNonVideoExtensionsRejected,
    trigger_fixture_schema: trigger.fixtureSchema,
    trigger_fixture_version: trigger.fixtureVersion,
    trigger_shared_fixture_case_count: trigger.sharedFixtureCaseCount,
    trigger_script_specific_case_count: trigger.scriptSpecificCaseCount,
    positive_prompt_count: trigger.positivePromptCount,
    negative_prompt_count: trigger.negativePromptCount,
    source_summary_redacted: report.contract?.sourceSummaryRedacted,
    download_validation_ok: report.downloadValidation?.ok,
    pptx_slide_count: report.downloadValidation?.pptxSlideCount,
    markdown_slide_heading_count: report.downloadValidation?.markdownSlideHeadingCount,
    network_calls_run: report.summary?.networkCallsRun,
    fixture_registered: report.summary?.fixtureRegistered,
    event_sent: report.summary?.eventSent,
    deliverables_downloaded_from_network: report.summary?.deliverablesDownloadedFromNetwork,
  };
}

function extractQualityMatrixSelfTestEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  return {
    schema: 'v3.video_ppt_quality_matrix_rollup_evidence.v1',
    input_mode: report.input_mode,
    case_count: report.summary?.case_count,
    deliverable_count: report.summary?.deliverable_count,
    pending_count: report.summary?.pending_count,
    expectation_mismatch_count: report.summary?.expectation_mismatch_count,
    review_required_risk_flag_count: report.gates?.review_required_risk_flag_count,
    review_required_risk_flag_object_shape_supported: report.gates?.review_required_risk_flag_object_shape_supported,
    review_required_risk_flag_object_shape_case_count: report.gates?.review_required_risk_flag_object_shape_case_count,
    live_smoke_run: report.gates?.live_smoke_run,
    production_write_allowed: report.gates?.production_write_allowed,
    generated_artifacts_committable: report.gates?.generated_artifacts_committable,
  };
}

function readJsonReportFromStdout(stdout) {
  const reportPath = extractReportPathFromStdout(stdout);
  if (!reportPath) {
    return null;
  }
  const normalized = path.normalize(reportPath);
  if (
    path.isAbsolute(normalized)
    || normalized.startsWith('..')
    || !normalized.startsWith(`target${path.sep}`)
  ) {
    return null;
  }
  try {
    return JSON.parse(fs.readFileSync(normalized, 'utf8'));
  } catch {
    return null;
  }
}

function extractReportPathFromStdout(stdout) {
  const reportEquals = stdout.match(/\breport=([^\s]+)/);
  if (reportEquals) {
    return reportEquals[1];
  }
  try {
    const parsed = JSON.parse(stdout.trim());
    return typeof parsed.reportPath === 'string' ? parsed.reportPath : '';
  } catch {
    return '';
  }
}

function displayCommand(command) {
  if (command.command === process.execPath) {
    return ['node', ...command.args].join(' ');
  }
  return [command.command, ...command.args].join(' ');
}

function lineCount(text) {
  if (!text) {
    return 0;
  }
  return text.trim().split(/\r?\n/).filter(Boolean).length;
}

function sanitizeExcerpt(text) {
  return text
    .split(/\r?\n/)
    .filter(Boolean)
    .slice(-20)
    .join('\n')
    .replaceAll(process.cwd(), '[repo]')
    .replaceAll(process.env.HOME || '', '[home]')
    .replace(/[A-Za-z]:[\\/][^\s"']+/g, '[redacted-path]')
    .replace(/\/Users\/[^\s"']+/g, '[redacted-path]')
    .replace(/\/home\/[^\s"']+/g, '[redacted-path]')
    .replace(/https?:\/\/[^\s"']+/g, '[redacted-url]')
    .replace(/(bearer|authorization|cookie|token|password|secret|provider_key)=?[^\s"']*/gi, '$1=[redacted]');
}

function buildReport({ startedAt, results }) {
  const failed = results.filter((result) => result.status !== 'passed');
  return {
    schema: 'v3.video_ppt_no_live_rollup.v1',
    status: failed.length === 0 ? 'passed' : 'failed',
    started_at: startedAt.toISOString(),
    completed_at: new Date().toISOString(),
    summary: {
      command_count: results.length,
      passed_count: results.length - failed.length,
      failed_count: failed.length,
    },
    gates: {
      live_smoke_run: false,
      production_write_allowed: false,
      network_download_allowed: false,
      file_upload_allowed: false,
      browser_capture_allowed: false,
      service_deployment_allowed: false,
      server_8_touched: false,
      server_120_touched: false,
    },
    commands: results,
    redaction: {
      status: 'applied',
      source_urls_included: false,
      local_paths_included: false,
      credentials_included: false,
      provider_payloads_included: false,
    },
    next_actions: failed.length === 0
      ? [
        'keep this as the no-live regression rollup before live upload, third-party, handoff, capture, or deployment gates',
      ]
      : [
        'inspect failed command locally and rerun the no-live rollup before any live gate',
      ],
  };
}

function validateReport(report) {
  if (report.schema !== 'v3.video_ppt_no_live_rollup.v1') {
    throw new Error('invalid no-live rollup schema');
  }
  if (report.summary.command_count !== COMMANDS.length) {
    throw new Error('no-live rollup command count mismatch');
  }
  if (
    report.gates.live_smoke_run
    || report.gates.production_write_allowed
    || report.gates.network_download_allowed
    || report.gates.file_upload_allowed
    || report.gates.browser_capture_allowed
    || report.gates.service_deployment_allowed
    || report.gates.server_8_touched
    || report.gates.server_120_touched
  ) {
    throw new Error('no-live rollup safety gates are invalid');
  }
  validateQualityMatrixEvidence(report);
  validateUploadMainEvidence(report);
  validateExternalVideoPptEvidence(report);
  const serialized = JSON.stringify(report);
  if (serialized.match(/[A-Za-z]:[\\/]|[\\/]Users[\\/]|[\\/]home[\\/]|https?:\/\/|token=|cookie=|bearer=/i)) {
    throw new Error('no-live rollup report contains unredacted local path, URL, or token-like text');
  }
}

function validateUploadMainEvidence(report) {
  const command = report.commands.find((result) => result.id === 'upload_main_self_test');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.video_ppt_upload_main_rollup_evidence.v1'
    || evidence.selected_scope_intent !== 'video_ppt_extraction'
    || evidence.candidate_source !== 'video_ppt_upload_main_smoke'
    || evidence.deliverable_state !== 'final_pptx_ready'
    || evidence.required_file_kind_count < 6
    || evidence.supported_video_extension_count < 6
    || evidence.trigger_fixture_schema !== 'v3.video_ppt_trigger_classifier_fixture.v1'
    || evidence.trigger_shared_fixture_case_count !== evidence.positive_prompt_count + evidence.negative_prompt_count
    || evidence.trigger_script_specific_case_count !== 0
    || evidence.positive_prompt_count < 6
    || evidence.negative_prompt_count < 6
    || evidence.source_summary_redacted !== true
    || evidence.download_validation_ok !== true
    || evidence.pptx_slide_count < 1
    || evidence.markdown_slide_heading_count < 1
    || evidence.network_calls_run !== false
    || evidence.production_write_allowed !== false
    || evidence.fixture_downloaded !== false
    || evidence.upload_attempted !== false
    || evidence.assistant_run_created !== false
    || evidence.source_urls_included !== false
    || evidence.object_keys_included !== false
    || evidence.cookies_included !== false
    || evidence.bearer_included !== false
    || evidence.provider_payloads_included !== false
  ) {
    throw new Error('no-live rollup upload-main evidence is incomplete');
  }
}

function validateExternalVideoPptEvidence(report) {
  const command = report.commands.find((result) => result.id === 'external_video_ppt_self_test');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.external_video_ppt_rollup_evidence.v1'
    || evidence.trigger_text_requests_video_ppt !== true
    || evidence.default_prompt_guards_ordinary_video_to_ppt !== true
    || evidence.requested_video_ppt_skill !== true
    || evidence.expected_action !== 'extract_video_ppt_transcript'
    || evidence.available_document_source_present !== true
    || evidence.available_document_external_ids_count < 1
    || evidence.dataset_external_ids_count < 1
    || evidence.supported_video_extension_count < 6
    || evidence.unsupported_non_video_extensions_rejected !== true
    || evidence.trigger_fixture_schema !== 'v3.video_ppt_trigger_classifier_fixture.v1'
    || evidence.trigger_shared_fixture_case_count !== evidence.positive_prompt_count + evidence.negative_prompt_count
    || evidence.trigger_script_specific_case_count !== 0
    || evidence.positive_prompt_count < 6
    || evidence.negative_prompt_count < 6
    || evidence.source_summary_redacted !== true
    || evidence.download_validation_ok !== true
    || evidence.pptx_slide_count < 1
    || evidence.markdown_slide_heading_count < 1
    || evidence.network_calls_run !== false
    || evidence.fixture_registered !== false
    || evidence.event_sent !== false
    || evidence.deliverables_downloaded_from_network !== false
  ) {
    throw new Error('no-live rollup external-video evidence is incomplete');
  }
}

function validateQualityMatrixEvidence(report) {
  const command = report.commands.find((result) => result.id === 'quality_matrix_self_test');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.video_ppt_quality_matrix_rollup_evidence.v1'
    || evidence.input_mode !== 'self_test'
    || evidence.case_count !== 3
    || evidence.deliverable_count !== 1
    || evidence.pending_count !== 2
    || evidence.expectation_mismatch_count !== 0
    || evidence.review_required_risk_flag_count !== 8
    || evidence.review_required_risk_flag_object_shape_supported !== true
    || evidence.review_required_risk_flag_object_shape_case_count !== 8
    || evidence.live_smoke_run !== false
    || evidence.production_write_allowed !== false
    || evidence.generated_artifacts_committable !== false
  ) {
    throw new Error('no-live rollup quality matrix evidence is incomplete');
  }
}

function writeReport(outputDir, report, pretty) {
  fs.mkdirSync(outputDir, { recursive: true });
  const timestamp = new Date().toISOString().replace(/[-:]/g, '').replace(/\..+/, '');
  const reportPath = path.join(outputDir, `${timestamp}-${process.pid}-no-live-rollup.json`);
  fs.writeFileSync(reportPath, JSON.stringify(report, null, pretty ? 2 : 0));
  return reportPath;
}

try {
  main();
} catch (error) {
  console.error(`video PPT no-live rollup failed: ${error.message}`);
  process.exitCode = 1;
}
