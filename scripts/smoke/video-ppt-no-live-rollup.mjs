#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const DEFAULT_OUTPUT_DIR = 'target/video-ppt-no-live-rollup';
const REQUIRED_NOT_DELIVERABLE_FAILURE_CLASSES = [
  'source_access',
  'video_has_no_ppt',
  'frame_extraction',
  'artifact_visibility',
  'selection_quality',
];
const EXPECTED_REVIEW_FAILURE_CLASS_COUNTS = {
  manual_review: 1,
  subtitle_alignment: 1,
  ocr_evidence: 1,
  crop_quality: 1,
  selection_quality: 2,
  readability_quality: 2,
};

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
    id: 'upload_main_live_approval_gate_negative',
    description: 'main-site upload live smoke refuses missing live write approval before network',
    command: process.execPath,
    args: ['scripts/smoke/video-ppt-upload-main.mjs'],
    expectFailure: true,
    expectedFailureIncludes: 'live write approval required',
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
    id: 'external_video_ppt_live_approval_gate_negative',
    description: 'third-party video PPT live smoke refuses missing live write approval before network',
    command: process.execPath,
    args: ['scripts/smoke/external-video-ppt.mjs', '--allow-missing-bearer'],
    expectFailure: true,
    expectedFailureIncludes: 'live write approval required',
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
    id: 'authorized_capture_dry_run',
    description: 'authorized capture dry-run handoff planning without browser or FFmpeg',
    command: process.execPath,
    args: [
      'scripts/capture-authorized-video.mjs',
      '--dry-run',
      '--ack-authorized',
      '--duration-seconds',
      '30',
      '--handoff',
      'upload-main',
      '--output-dir',
      'target/video-ppt-no-live-rollup-authorized-capture-dry-run',
    ],
    env: {
      AUTHORIZED_CAPTURE_APPROVAL_ID: ['dry-run', 'approval', 'redacted'].join('-'),
      AUTHORIZED_CAPTURE_APPROVED_BY: ['dry-run', 'operator', 'redacted'].join('-'),
      AUTHORIZED_CAPTURE_URL: ['https:', '', 'example.com', 'authorized-video-page'].join('/'),
      AUTHORIZED_CAPTURE_PURPOSE: 'video-ppt-authorized-capture-dry-run',
    },
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
    id: 'scope_planner_syntax',
    description: 'front-end production scope planner syntax check',
    command: process.execPath,
    args: ['--check', 'apps/web/app/lib/scope-planner.js'],
  },
  {
    id: 'scope_planner_tests',
    description: 'front-end production scope planner video PPT trigger tests',
    command: process.execPath,
    args: ['--test', 'apps/web/app/lib/scope-planner.test.mjs'],
  },
  {
    id: 'assistant_startup_briefing_tests',
    description: 'front-end assistant startup briefing video PPT boundary tests',
    command: process.execPath,
    args: ['--test', 'apps/web/app/lib/assistant-startup-briefing.test.mjs'],
  },
  {
    id: 'assistant_runtime_video_ppt_scope_tests',
    description: 'Rust assistant-runtime production video PPT scope tests',
    command: 'cargo',
    args: ['test', '-p', 'assistant-runtime', 'video_ppt_scope', '--lib'],
  },
  {
    id: 'platform_api_video_url_resolution_tests',
    description: 'platform-api direct URL and login-gated video resolver tests',
    command: 'cargo',
    args: ['test', '-p', 'platform-api', 'video_url_resolution', '--lib'],
    env: { CC: 'clang', CXX: 'clang++' },
  },
  {
    id: 'platform_api_public_video_page_tests',
    description: 'platform-api public page video resolver tests',
    command: 'cargo',
    args: ['test', '-p', 'platform-api', 'public_video_page', '--lib'],
    env: { CC: 'clang', CXX: 'clang++' },
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
  - verifies video PPT upload live mode refuses missing live write approval before network calls
  - runs third-party video PPT self-test and preflight without network calls
  - verifies third-party video PPT live mode refuses missing live write approval before network calls
  - runs login-gated video handoff self-test and preflight without network calls
  - runs authorized-capture self-test without opening a browser or FFmpeg
  - runs authorized-capture dry-run handoff planning without opening a browser or FFmpeg
  - runs quality matrix self-test, including review-risk regression
  - runs front-end scope planner syntax/tests for video PPT trigger boundaries
  - runs front-end assistant startup briefing tests for video PPT source boundaries
  - runs Rust assistant-runtime video PPT scope tests
  - runs Rust platform-api video URL and public page resolver tests
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
      ...(command.env || {}),
      NO_COLOR: '1',
    },
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  const durationMs = Date.now() - started;
  const exitCode = typeof child.status === 'number' ? child.status : 1;
  const stdout = child.stdout || '';
  const stderr = child.stderr || '';
  const combinedOutput = `${stderr}\n${stdout}`;
  const expectedFailureMatched = command.expectFailure === true
    && exitCode !== 0
    && combinedOutput.includes(command.expectedFailureIncludes || '');
  const passed = command.expectFailure === true ? expectedFailureMatched : exitCode === 0;
  return {
    id: command.id,
    description: command.description,
    status: passed ? 'passed' : 'failed',
    exit_code: exitCode,
    expected_failure: command.expectFailure === true,
    duration_ms: durationMs,
    command: displayCommand(command),
    stdout_line_count: lineCount(stdout),
    stderr_line_count: lineCount(stderr),
    evidence: passed ? extractCommandEvidence(command.id, stdout, stderr, command, exitCode) : null,
    failure_excerpt: passed
      ? null
      : sanitizeExcerpt(combinedOutput),
  };
}

function extractCommandEvidence(commandId, stdout, stderr = '', command = {}, exitCode = 0) {
  if (
    commandId === 'upload_main_live_approval_gate_negative'
    || commandId === 'external_video_ppt_live_approval_gate_negative'
  ) {
    return extractLiveApprovalNegativeEvidence(commandId, stderr, command, exitCode);
  }
  if (commandId === 'upload_main_self_test') {
    return extractUploadMainSelfTestEvidence(stdout);
  }
  if (commandId === 'upload_main_preflight') {
    return extractUploadMainPreflightEvidence(stdout);
  }
  if (commandId === 'external_video_ppt_self_test') {
    return extractExternalVideoPptSelfTestEvidence(stdout);
  }
  if (commandId === 'external_video_ppt_preflight') {
    return extractExternalVideoPptPreflightEvidence(stdout);
  }
  if (commandId === 'video_ppt_handoff_self_test') {
    return extractVideoPptHandoffSelfTestEvidence(stdout);
  }
  if (commandId === 'video_ppt_handoff_preflight') {
    return extractVideoPptHandoffPreflightEvidence(stdout);
  }
  if (commandId === 'authorized_capture_self_test') {
    return extractAuthorizedCaptureSelfTestEvidence(stdout);
  }
  if (commandId === 'authorized_capture_dry_run') {
    return extractAuthorizedCaptureDryRunEvidence(stdout);
  }
  if (commandId === 'scope_planner_tests') {
    return extractScopePlannerTestsEvidence(stdout);
  }
  if (commandId === 'assistant_startup_briefing_tests') {
    return extractAssistantStartupBriefingTestsEvidence(stdout);
  }
  if (commandId === 'assistant_runtime_video_ppt_scope_tests') {
    return extractAssistantRuntimeVideoPptScopeTestsEvidence(stdout);
  }
  if (commandId === 'platform_api_video_url_resolution_tests') {
    return extractPlatformApiVideoUrlResolutionTestsEvidence(stdout);
  }
  if (commandId === 'platform_api_public_video_page_tests') {
    return extractPlatformApiPublicVideoPageTestsEvidence(stdout);
  }
  if (commandId !== 'quality_matrix_self_test') {
    return null;
  }
  return extractQualityMatrixSelfTestEvidence(stdout);
}

function extractLiveApprovalNegativeEvidence(commandId, stderr, command, exitCode) {
  const output = String(stderr || '');
  return {
    schema: 'v3.video_ppt_live_write_approval_negative_evidence.v1',
    command_id: commandId,
    expected_failure: true,
    exit_code: exitCode,
    approval_gate_enforced: output.includes(command.expectedFailureIncludes || ''),
    network_calls_run: false,
    production_write_allowed: false,
    upload_attempted: false,
    event_sent: false,
    browser_capture_allowed: false,
    service_deployment_allowed: false,
  };
}

function extractUploadMainSelfTestEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  const trigger = report.contract?.triggerClassifier || {};
  const approvalGate = report.contract?.liveWriteApprovalGate || {};
  const commandTemplate = report.contract?.liveCommandTemplateContract || {};
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
    live_write_approval_gate_schema: approvalGate.schema,
    live_write_approval_gate_case_count: approvalGate.caseCount,
    live_write_approval_gate_negative_case_count: approvalGate.negativeCaseCount,
    live_write_approval_gate_positive_case_count: approvalGate.positiveCaseCount,
    live_write_approval_rejects_missing_ack_and_approval:
      approvalGate.rejectsMissingAckAndApproval,
    live_write_approval_rejects_missing_approval: approvalGate.rejectsMissingApproval,
    live_write_approval_rejects_missing_ack: approvalGate.rejectsMissingAck,
    live_write_approval_allows_preflight_without_approval:
      approvalGate.allowsPreflightWithoutApproval,
    live_write_approval_allows_self_test_without_approval:
      approvalGate.allowsSelfTestWithoutApproval,
    live_write_approval_accepts_complete_live_approval_shape:
      approvalGate.acceptsCompleteLiveApprovalShape,
    live_write_approval_values_included: approvalGate.approvalValuesIncluded,
    live_command_template_contract_schema: commandTemplate.schema,
    live_command_template_contract_case_count: commandTemplate.caseCount,
    live_command_template_url_fixture_ready: commandTemplate.urlFixtureTemplateReady,
    live_command_template_local_file_ready: commandTemplate.localFileTemplateReady,
    live_command_template_all_include_ack_live_write:
      commandTemplate.allTemplatesIncludeAckLiveWrite,
    live_command_template_all_include_redacted_approval_id:
      commandTemplate.allTemplatesIncludeRedactedApprovalId,
    live_command_template_all_include_redacted_bearer:
      commandTemplate.allTemplatesIncludeRedactedBearer,
    live_command_template_raw_values_included: commandTemplate.rawValuesIncluded,
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

function extractUploadMainPreflightEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  const commandTemplate = String(report.commandTemplate || '');
  return {
    schema: 'v3.video_ppt_upload_main_preflight_rollup_evidence.v1',
    report_schema: report.schema,
    ok: report.summary?.ok,
    preflight: report.summary?.preflight,
    network_calls_run: report.summary?.networkCallsRun,
    production_write_allowed: report.summary?.productionWriteAllowed,
    live_write_approval_required: report.summary?.liveWriteApprovalRequired,
    live_write_approval_gate_enforced: report.summary?.liveWriteApprovalGateEnforced,
    live_write_approval_satisfied: report.summary?.liveWriteApprovalSatisfied,
    fixture_downloaded: report.summary?.fixtureDownloaded,
    upload_attempted: report.summary?.uploadAttempted,
    dataset_created: report.summary?.datasetCreated,
    document_registered: report.summary?.documentRegistered,
    assistant_run_created: report.summary?.assistantRunCreated,
    failure_count: report.summary?.failures?.length,
    fixture_source_kind: report.fixture?.sourceKind,
    fixture_media_kind: report.fixture?.mediaKind,
    fixture_supported_extension: report.fixture?.supportedExtension,
    fixture_prompt_requests_video_ppt: report.fixture?.promptRequestsVideoPpt,
    trigger_prompt_requests_video_ppt: report.trigger?.promptRequestsVideoPpt,
    planned_step_count: report.liveWriteScope?.plannedSteps?.length,
    writes_smoke_records: report.liveWriteScope?.writesSmokeRecords,
    deploys_services: report.liveWriteScope?.deploysServices,
    command_template_present: commandTemplate.length > 0,
    command_template_has_ack_live_write: commandTemplate.includes('--ack-live-write'),
    command_template_has_redacted_approval_id:
      commandTemplate.includes('--approval-id <redacted-approval-id>'),
    command_template_approval_values_included: /approval_ref|redacted-approval-ref/i
      .test(commandTemplate),
    command_template_raw_url_included: /https?:\/\//i.test(commandTemplate),
    command_template_local_path_included: /\/Users\/|\/home\/|[A-Za-z]:\\/i
      .test(commandTemplate),
    raw_fixture_url_included: report.redaction?.rawFixtureUrlIncluded,
    local_fixture_path_included: report.redaction?.localFixturePathIncluded,
    approval_id_included: report.redaction?.approvalIdIncluded,
    cookies_included: report.redaction?.cookiesIncluded,
    bearer_included: report.redaction?.bearerIncluded,
    object_keys_included: report.redaction?.objectKeysIncluded,
    provider_payloads_included: report.redaction?.providerPayloadsIncluded,
  };
}

function extractExternalVideoPptSelfTestEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  const trigger = report.contract?.triggerClassifier || {};
  const approvalGate = report.contract?.liveWriteApprovalGate || {};
  const commandTemplate = report.contract?.liveCommandTemplateContract || {};
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
    live_write_approval_gate_schema: approvalGate.schema,
    live_write_approval_gate_case_count: approvalGate.caseCount,
    live_write_approval_gate_negative_case_count: approvalGate.negativeCaseCount,
    live_write_approval_gate_positive_case_count: approvalGate.positiveCaseCount,
    live_write_approval_rejects_missing_ack_and_approval:
      approvalGate.rejectsMissingAckAndApproval,
    live_write_approval_rejects_missing_approval: approvalGate.rejectsMissingApproval,
    live_write_approval_rejects_missing_ack: approvalGate.rejectsMissingAck,
    live_write_approval_allows_preflight_without_approval:
      approvalGate.allowsPreflightWithoutApproval,
    live_write_approval_allows_self_test_without_approval:
      approvalGate.allowsSelfTestWithoutApproval,
    live_write_approval_accepts_complete_live_approval_shape:
      approvalGate.acceptsCompleteLiveApprovalShape,
    live_write_approval_values_included: approvalGate.approvalValuesIncluded,
    live_command_template_contract_schema: commandTemplate.schema,
    live_command_template_contract_case_count: commandTemplate.caseCount,
    live_command_template_url_fixture_ready: commandTemplate.urlFixtureTemplateReady,
    live_command_template_local_file_ready: commandTemplate.localFileTemplateReady,
    live_command_template_all_include_ack_live_write:
      commandTemplate.allTemplatesIncludeAckLiveWrite,
    live_command_template_all_include_redacted_approval_id:
      commandTemplate.allTemplatesIncludeRedactedApprovalId,
    live_command_template_all_include_redacted_bearer:
      commandTemplate.allTemplatesIncludeRedactedBearer,
    live_command_template_raw_values_included: commandTemplate.rawValuesIncluded,
    surface_ok: report.surface?.ok,
    surface_export_count: report.surface?.exportCount,
    surface_export_kind_count: report.surface?.exportKinds?.length,
    surface_missing_export_kind_count: report.surface?.missingExportKinds?.length,
    surface_artifact_link_count: report.surface?.artifactLinkCount,
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

function extractExternalVideoPptPreflightEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  const commandTemplate = String(report.commandTemplate || '');
  return {
    schema: 'v3.external_video_ppt_preflight_rollup_evidence.v1',
    report_schema: report.schema,
    ok: report.summary?.ok,
    preflight: report.summary?.preflight,
    network_calls_run: report.summary?.networkCallsRun,
    production_write_allowed: report.summary?.productionWriteAllowed,
    live_write_approval_required: report.summary?.liveWriteApprovalRequired,
    live_write_approval_gate_enforced: report.summary?.liveWriteApprovalGateEnforced,
    live_write_approval_satisfied: report.summary?.liveWriteApprovalSatisfied,
    credential_gate_satisfied: report.summary?.credentialGateSatisfied,
    live_credential_ready: report.summary?.liveCredentialReady,
    allow_missing_bearer: report.summary?.allowMissingBearer,
    fixture_downloaded: report.summary?.fixtureDownloaded,
    fixture_registered: report.summary?.fixtureRegistered,
    event_sent: report.summary?.eventSent,
    reply_polled: report.summary?.replyPolled,
    deliverables_downloaded_from_network: report.summary?.deliverablesDownloadedFromNetwork,
    failure_count: report.summary?.failures?.length,
    connection_id_present: report.target?.connectionIdPresent,
    source_id_present: report.target?.sourceIdPresent,
    tenant_external_id_present: report.target?.tenantExternalIdPresent,
    bot_external_id_present: report.target?.botExternalIdPresent,
    sender_external_id_present: report.target?.senderExternalIdPresent,
    fixture_source_kind: report.fixture?.sourceKind,
    fixture_media_kind: report.fixture?.mediaKind,
    fixture_supported_extension: report.fixture?.supportedExtension,
    trigger_text_requests_video_ppt: report.trigger?.textRequestsVideoPpt,
    default_prompt_guards_ordinary_video_to_ppt:
      report.trigger?.defaultPromptGuardsAgainstOrdinaryVideoToPpt,
    requested_video_ppt_skill: Array.isArray(report.trigger?.requestedSkillIds)
      ? report.trigger.requestedSkillIds.includes('video_ppt_extraction')
      : false,
    expected_action: report.trigger?.expectedAction,
    available_document_source_present: report.trigger?.availableDocumentSourcePresent,
    available_document_external_ids_count: report.trigger?.availableDocumentExternalIdsCount,
    dataset_external_ids_count: report.trigger?.datasetExternalIdsCount,
    planned_step_count: report.liveWriteScope?.plannedSteps?.length,
    writes_smoke_records: report.liveWriteScope?.writesSmokeRecords,
    deploys_services: report.liveWriteScope?.deploysServices,
    command_template_present: commandTemplate.length > 0,
    command_template_has_ack_live_write: commandTemplate.includes('--ack-live-write'),
    command_template_has_redacted_approval_id:
      commandTemplate.includes('--approval-id <redacted-approval-id>'),
    command_template_has_redacted_bearer:
      commandTemplate.includes('--bearer <redacted-inbound-bearer>'),
    command_template_approval_values_included: /approval_ref|redacted-approval-ref/i
      .test(commandTemplate),
    command_template_raw_url_included: /https?:\/\//i.test(commandTemplate),
    command_template_local_path_included: /\/Users\/|\/home\/|[A-Za-z]:\\/i
      .test(commandTemplate),
    raw_fixture_url_included: report.redaction?.rawFixtureUrlIncluded,
    local_fixture_path_included: report.redaction?.localFixturePathIncluded,
    approval_id_included: report.redaction?.approvalIdIncluded,
    bearer_included: report.redaction?.bearerIncluded,
    object_keys_included: report.redaction?.objectKeysIncluded,
    provider_payloads_included: report.redaction?.providerPayloadsIncluded,
  };
}

function extractVideoPptHandoffSelfTestEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  return {
    schema: 'v3.video_ppt_handoff_rollup_evidence.v1',
    prompt_mentions_wechat_video: report.summary?.prompt?.mentionsWeChatVideo,
    prompt_wants_slide_output: report.summary?.prompt?.wantsSlideOutput,
    network_calls_run: report.summary?.networkCallsRun,
    provider_called: report.summary?.providerCalled,
    react_toolchain_called: report.summary?.reactToolchainCalled,
    video_fetch_attempted: report.summary?.videoFetchAttempted,
    video_downloaded: report.summary?.videoDownloaded,
    frames_extracted: report.summary?.framesExtracted,
    ocr_run: report.summary?.ocrRun,
    ppt_generated: report.summary?.pptGenerated,
    final_pptx_ready_exposed: report.summary?.finalPptxReadyExposed,
    artifact_links_exposed: report.summary?.artifactLinksExposed,
    download_exports_exposed: report.summary?.downloadExportsExposed,
    negative_fixture_count: report.summary?.negativeFixtureCount,
    negative_fixtures_rejected: report.summary?.negativeFixturesRejected,
    main_ok: report.main?.ok,
    main_failure_reason: report.main?.failureReason,
    main_next_step_count: report.main?.nextStepKeys?.length,
    main_has_required_reason: report.main?.hasRequiredReason,
    main_has_handoff_type: report.main?.hasHandoffType,
    main_has_actionable_next_steps: report.main?.hasActionableNextSteps,
    main_unsafe_success_signal: report.main?.unsafeSuccessSignal,
    main_unsafe_artifact_link_signal: report.main?.unsafeArtifactLinkSignal,
    main_unsafe_credential_request: report.main?.unsafeCredentialRequest,
    main_raw_source_leaked: report.main?.rawSourceLeaked,
    external_ok: report.external?.ok,
    external_failure_reason: report.external?.failureReason,
    external_next_step_count: report.external?.nextStepKeys?.length,
    external_has_required_reason: report.external?.hasRequiredReason,
    external_has_handoff_type: report.external?.hasHandoffType,
    external_has_actionable_next_steps: report.external?.hasActionableNextSteps,
    external_unsafe_success_signal: report.external?.unsafeSuccessSignal,
    external_unsafe_artifact_link_signal: report.external?.unsafeArtifactLinkSignal,
    external_unsafe_credential_request: report.external?.unsafeCredentialRequest,
    external_raw_source_leaked: report.external?.rawSourceLeaked,
  };
}

function extractVideoPptHandoffPreflightEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  return {
    schema: 'v3.video_ppt_handoff_preflight_rollup_evidence.v1',
    report_schema: report.schema,
    ok: report.summary?.ok,
    preflight: report.summary?.preflight,
    mode: report.summary?.mode,
    target_mode_count: report.summary?.targetModes?.length,
    network_calls_run: report.summary?.networkCallsRun,
    source_page_fetched: report.summary?.sourcePageFetched,
    video_downloaded: report.summary?.videoDownloaded,
    frames_extracted: report.summary?.framesExtracted,
    ocr_run: report.summary?.ocrRun,
    ppt_generated: report.summary?.pptGenerated,
    provider_called: report.summary?.providerCalled,
    lightweight_smoke_writes_planned: report.summary?.lightweightSmokeWritesPlanned,
    deployment_approval_required: report.summary?.deploymentApprovalRequired,
    live_credential_ready: report.summary?.liveCredentialReady,
    credential_gate_satisfied: report.summary?.credentialGateSatisfied,
    allow_missing_bearer: report.summary?.allowMissingBearer,
    failure_count: report.summary?.failures?.length,
    main_planned_step_count: report.target?.main?.plannedSteps?.length,
    external_planned_step_count: report.target?.external?.plannedSteps?.length,
    external_connection_id_present: report.target?.external?.connectionIdPresent,
    external_source_id_present: report.target?.external?.sourceIdPresent,
    failure_reason: report.expectedSurface?.failureReason,
    supported_next_step_count: report.expectedSurface?.supportedNextSteps?.length,
    success_signal_rejected_count: report.expectedSurface?.successSignalsRejected?.length,
    prompt_mentions_wechat_video: report.prompt?.mentionsWeChatVideo,
    prompt_wants_slide_output: report.prompt?.wantsSlideOutput,
    prompt_mentions_login_gate: report.prompt?.mentionsLoginGate,
    raw_source_url_included: report.redaction?.rawSourceUrlIncluded,
    cookie_included: report.redaction?.cookieIncluded,
    bearer_included: report.redaction?.bearerIncluded,
    provider_payloads_included: report.redaction?.providerPayloadsIncluded,
    local_paths_included: report.redaction?.localPathsIncluded,
  };
}

function extractAuthorizedCaptureSelfTestEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  const receipt = report.sharedReceipt || {};
  return {
    schema: 'v3.authorized_capture_rollup_evidence.v1',
    mode: report.summary?.mode,
    self_test: report.summary?.selfTest,
    approval_reference_present: receipt.approvalReferencePresent,
    approval_id_redacted: receipt.approvalIdRedacted,
    approved_by_reference_present: receipt.approvedByReferencePresent,
    approved_by_redacted: receipt.approvedByRedacted,
    purpose_present: receipt.purposePresent,
    duration_seconds: receipt.durationSeconds,
    retention_days: receipt.retentionDays,
    capture_audio_allowed: receipt.captureAudioAllowed,
    capture_mode_present: typeof receipt.captureMode === 'string' && receipt.captureMode.length > 0,
    handoff_mode: receipt.output?.handoffMode,
    authorization_gate_negative_case_count: report.summary?.authorizationGateNegativeCaseCount,
    authorization_gate_negative_cases_rejected: report.summary?.authorizationGateNegativeCasesRejected,
    capture_attempted: report.capture?.attempted,
    dry_run: report.capture?.dryRun,
    browser_would_run: report.capture?.browserWouldRun,
    ffmpeg_would_run: report.capture?.ffmpegWouldRun,
    ack_authorized: report.safety?.ackAuthorized,
    isolated_browser_profile: report.safety?.isolatedBrowserProfile,
    persistent_profile_disabled: report.safety?.persistentProfileDisabled,
    raw_url_stored: report.safety?.rawUrlStored,
    cookies_stored: report.safety?.cookiesStored,
    har_stored: report.safety?.harStored,
    qr_screenshot_stored: report.safety?.qrScreenshotStored,
    local_path_redacted: receipt.output?.localPathRedacted,
    profile_path_redacted: receipt.output?.profilePathRedacted,
    approval_values_included: receipt.redactionFlags?.approvalValuesIncluded,
    raw_source_url_included: receipt.redactionFlags?.rawSourceUrlIncluded,
    local_paths_included: receipt.redactionFlags?.localPathsIncluded,
    credentials_included: receipt.redactionFlags?.credentialsIncluded,
    provider_payloads_included: receipt.redactionFlags?.providerPayloadsIncluded,
  };
}

function extractAuthorizedCaptureDryRunEvidence(stdout) {
  const report = readJsonReportFromStdout(stdout);
  if (!report) {
    return null;
  }
  const receipt = report.sharedReceipt || {};
  return {
    schema: 'v3.authorized_capture_dry_run_rollup_evidence.v1',
    ok: report.summary?.ok,
    mode: report.summary?.mode,
    source_scheme: report.summary?.source?.scheme,
    source_host_present: typeof report.summary?.source?.host === 'string'
      && report.summary.source.host.length > 0,
    duration_seconds: report.summary?.durationSeconds,
    retention_days: report.summary?.retentionDays,
    capture_audio_allowed: report.summary?.captureAudioAllowed,
    handoff: report.summary?.handoff,
    approval_reference_present: receipt.approvalReferencePresent,
    approval_id_redacted: receipt.approvalIdRedacted,
    approved_by_reference_present: receipt.approvedByReferencePresent,
    approved_by_redacted: receipt.approvedByRedacted,
    purpose_present: receipt.purposePresent,
    shared_receipt_source_scheme: receipt.source?.scheme,
    shared_receipt_source_host_present: typeof receipt.source?.host === 'string'
      && receipt.source.host.length > 0,
    output_file_name: receipt.output?.fileName,
    local_path_redacted: receipt.output?.localPathRedacted,
    profile_path_redacted: receipt.output?.profilePathRedacted,
    handoff_mode: receipt.output?.handoffMode,
    capture_mode_present: typeof receipt.captureMode === 'string' && receipt.captureMode.length > 0,
    capture_attempted: report.capture?.attempted,
    dry_run: report.capture?.dryRun,
    browser_would_run: report.capture?.browserWouldRun,
    ffmpeg_would_run: report.capture?.ffmpegWouldRun,
    ack_authorized: report.safety?.ackAuthorized,
    isolated_browser_profile: report.safety?.isolatedBrowserProfile,
    persistent_profile_disabled: report.safety?.persistentProfileDisabled,
    raw_url_stored: report.safety?.rawUrlStored,
    cookies_stored: report.safety?.cookiesStored,
    har_stored: report.safety?.harStored,
    qr_screenshot_stored: report.safety?.qrScreenshotStored,
    approval_values_included: receipt.redactionFlags?.approvalValuesIncluded,
    raw_source_url_included: receipt.redactionFlags?.rawSourceUrlIncluded,
    local_paths_included: receipt.redactionFlags?.localPathsIncluded,
    credentials_included: receipt.redactionFlags?.credentialsIncluded,
    provider_payloads_included: receipt.redactionFlags?.providerPayloadsIncluded,
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
    deliverables_mode_failure_class_gate_defaults_supported:
      report.gates?.deliverables_mode_failure_class_gate_defaults_supported,
    customer_authorization_argument_gate_supported:
      report.gates?.customer_authorization_argument_gate_supported,
    customer_retention_policy_argument_gate_supported:
      report.gates?.customer_retention_policy_argument_gate_supported,
    review_failure_class_summary_supported: report.gates?.review_failure_class_summary_supported,
    review_failure_class_summary_needs_manual_review_count:
      report.gates?.review_failure_class_summary_needs_manual_review_count,
    review_failure_class_summary_counts:
      sanitizeCountMap(report.gates?.review_failure_class_summary_counts),
    not_deliverable_failure_class_count: report.gates?.not_deliverable_failure_class_count,
    not_deliverable_failure_classes: Array.isArray(report.gates?.not_deliverable_failure_classes)
      ? report.gates.not_deliverable_failure_classes
      : [],
    failure_class_summary_supported: report.gates?.failure_class_summary_supported,
    failure_class_counts: sanitizeCountMap(report.summary?.failure_class_counts),
    not_deliverable_failure_class_counts: sanitizeCountMap(report.summary?.not_deliverable_failure_class_counts),
    needs_manual_review_failure_class_counts: sanitizeCountMap(report.summary?.needs_manual_review_failure_class_counts),
    failure_class_summary_regression_not_deliverable_count:
      report.gates?.failure_class_summary_regression_not_deliverable_count,
    failure_class_summary_regression_class_count:
      report.gates?.failure_class_summary_regression_class_count,
    failure_class_summary_regression_counts:
      sanitizeCountMap(report.gates?.failure_class_summary_regression_counts),
    failure_class_summary_regression_not_deliverable_counts:
      sanitizeCountMap(report.gates?.failure_class_summary_regression_not_deliverable_counts),
    live_smoke_run: report.gates?.live_smoke_run,
    production_write_allowed: report.gates?.production_write_allowed,
    generated_artifacts_committable: report.gates?.generated_artifacts_committable,
  };
}

function sanitizeCountMap(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return null;
  }
  const result = {};
  for (const [key, count] of Object.entries(value)) {
    if (typeof key === 'string' && Number.isInteger(count) && count >= 0) {
      result[key] = count;
    }
  }
  return result;
}

function extractScopePlannerTestsEvidence(stdout) {
  return {
    schema: 'v3.scope_planner_video_ppt_rollup_evidence.v1',
    test_count: parseTapSummaryCount(stdout, 'tests'),
    pass_count: parseTapSummaryCount(stdout, 'pass'),
    fail_count: parseTapSummaryCount(stdout, 'fail'),
    cancelled_count: parseTapSummaryCount(stdout, 'cancelled'),
    skipped_count: parseTapSummaryCount(stdout, 'skipped'),
    todo_count: parseTapSummaryCount(stdout, 'todo'),
    direct_video_ppt_positive_covered:
      stdout.includes('scope planner recommends direct video PPT extraction without forcing dataset retrieval'),
    public_video_page_positive_covered:
      stdout.includes('scope planner recommends public video page resolution before PPT extraction'),
    uploaded_video_positive_covered:
      stdout.includes('scope planner recommends uploaded video extraction without public URL resolution'),
    mkv_avi_positive_covered:
      stdout.includes('scope planner treats mkv and avi video names as PPT extraction triggers'),
    transcript_only_negative_covered:
      stdout.includes('scope planner does not use PPT extraction for transcript-only video requests'),
    shared_negative_fixture_covered:
      stdout.includes('scope planner rejects shared negative video PPT trigger fixture'),
    shared_positive_fixture_covered:
      stdout.includes('scope planner follows shared positive video PPT trigger fixture'),
  };
}

function extractAssistantStartupBriefingTestsEvidence(stdout) {
  return {
    schema: 'v3.assistant_startup_briefing_video_ppt_rollup_evidence.v1',
    test_count: parseTapSummaryCount(stdout, 'tests'),
    pass_count: parseTapSummaryCount(stdout, 'pass'),
    fail_count: parseTapSummaryCount(stdout, 'fail'),
    cancelled_count: parseTapSummaryCount(stdout, 'cancelled'),
    skipped_count: parseTapSummaryCount(stdout, 'skipped'),
    todo_count: parseTapSummaryCount(stdout, 'todo'),
    system_capability_test_covered:
      stdout.includes('startup briefing summarizes visible datasets and system capability'),
    no_dataset_formatted_boundary_test_covered:
      stdout.includes('formatted briefing tells model when no dataset is selected'),
    video_ppt_source_routing_boundary_covered:
      stdout.includes('startup briefing preserves video PPT source routing and handoff boundaries'),
  };
}

function extractAssistantRuntimeVideoPptScopeTestsEvidence(stdout) {
  const summary = parseRustTestSummary(stdout);
  return {
    schema: 'v3.assistant_runtime_video_ppt_scope_rollup_evidence.v1',
    ...summary,
    transcript_only_negative_covered:
      stdout.includes('transcript_only_video_request_does_not_trigger_video_ppt_scope'),
    shared_negative_fixture_covered:
      stdout.includes('video_ppt_scope_rejects_shared_negative_trigger_fixture'),
    shared_positive_fixture_covered:
      stdout.includes('video_ppt_scope_follows_shared_positive_trigger_fixture'),
  };
}

function extractPlatformApiVideoUrlResolutionTestsEvidence(stdout) {
  return {
    schema: 'v3.platform_api_video_url_resolution_rollup_evidence.v1',
    ...parseRustTestSummary(stdout),
    requires_direct_source_covered:
      stdout.includes('video_url_resolution_placeholder_requires_direct_source'),
    accepts_direct_video_url_covered:
      stdout.includes('video_url_resolution_accepts_direct_video_urls'),
    chinese_punctuation_direct_url_covered:
      stdout.includes('video_url_resolution_finds_direct_url_after_chinese_punctuation'),
    login_gated_source_blocked_covered:
      stdout.includes('video_url_resolution_placeholder_blocks_login_gated_sources'),
    dataset_title_helpers_covered:
      stdout.includes('video_url_resolution_helpers_select_dataset_and_title'),
  };
}

function extractPlatformApiPublicVideoPageTestsEvidence(stdout) {
  return {
    schema: 'v3.platform_api_public_video_page_rollup_evidence.v1',
    ...parseRustTestSummary(stdout),
    public_page_extracts_video_sources_covered:
      stdout.includes('public_video_page_extracts_video_sources_from_html'),
    public_page_blocks_private_or_login_gated_sources_covered:
      stdout.includes('public_video_page_blocks_private_or_login_gated_sources'),
    public_page_no_video_asset_covered:
      stdout.includes('public_video_page_handles_no_video_asset_fixture'),
    public_page_failure_result_structured_covered:
      stdout.includes('public_video_page_failure_result_is_structured'),
    public_page_failure_result_classification_covered:
      stdout.includes('public_video_page_failure_result_classifies_resolver_blocks'),
    public_page_unregistered_result_keeps_resolved_video_covered:
      stdout.includes('public_video_page_unregistered_result_keeps_resolved_video'),
  };
}

function parseRustTestSummary(stdout) {
  const runningMatch = stdout.match(/\brunning\s+(\d+)\s+tests?\b/);
  const resultMatch = stdout.match(/\btest result:\s+ok\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored;\s+(\d+)\s+measured;\s+(\d+)\s+filtered out\b/);
  return {
    test_count: runningMatch ? Number.parseInt(runningMatch[1], 10) : null,
    pass_count: resultMatch ? Number.parseInt(resultMatch[1], 10) : null,
    fail_count: resultMatch ? Number.parseInt(resultMatch[2], 10) : null,
    ignored_count: resultMatch ? Number.parseInt(resultMatch[3], 10) : null,
    measured_count: resultMatch ? Number.parseInt(resultMatch[4], 10) : null,
    filtered_out_count: resultMatch ? Number.parseInt(resultMatch[5], 10) : null,
  };
}

function parseTapSummaryCount(stdout, key) {
  const escapedKey = key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stdout.match(new RegExp(`^# ${escapedKey} (\\d+)`, 'm'));
  return match ? Number.parseInt(match[1], 10) : null;
}

function readJsonReportFromStdout(stdout) {
  const reportPath = extractReportPathFromStdout(stdout);
  if (!reportPath) {
    return null;
  }
  const normalized = safeLocalReportPath(reportPath);
  if (!normalized) {
    return null;
  }
  try {
    return JSON.parse(fs.readFileSync(normalized, 'utf8'));
  } catch {
    return null;
  }
}

function safeLocalReportPath(reportPath) {
  const normalized = path.normalize(reportPath);
  if (path.isAbsolute(normalized)) {
    const resolved = path.resolve(normalized);
    const targetRoot = path.join(process.cwd(), 'target') + path.sep;
    return resolved.startsWith(targetRoot) ? resolved : '';
  }
  if (normalized.startsWith('..') || !normalized.startsWith(`target${path.sep}`)) {
    return '';
  }
  return normalized;
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
  const summary = {
    command_count: results.length,
    passed_count: results.length - failed.length,
    failed_count: failed.length,
  };
  return {
    schema: 'v3.video_ppt_no_live_rollup.v1',
    status: failed.length === 0 ? 'passed' : 'failed',
    started_at: startedAt.toISOString(),
    completed_at: new Date().toISOString(),
    summary,
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
    acceptance_status: buildAcceptanceStatus({ summary, results }),
    next_actions: failed.length === 0
      ? [
        'keep this as the no-live regression rollup before live upload, third-party, handoff, capture, or deployment gates',
      ]
      : [
        'inspect failed command locally and rerun the no-live rollup before any live gate',
      ],
  };
}

function buildAcceptanceStatus({ summary, results }) {
  const noLivePassed = summary.failed_count === 0;
  const noLiveEvidenceSummary = buildNoLiveAcceptanceEvidenceSummary(results);
  const liveGateReadinessSummary = buildLiveGateReadinessSummary(results);
  const gates = [
    {
      id: 'P1_no_live_baseline',
      status: noLivePassed ? 'passed' : 'failed',
      evidence: [
        'no_live_command_results',
        'embedded_rollup_evidence',
        'redaction_validation',
      ],
      no_live_substitute_available: true,
    },
    {
      id: 'P2_main_upload_live_smoke',
      status: 'pending_authorization',
      requires: [
        'main_site_non_customer_write_approval',
        'safe_video_input',
        'same_fixture_preflight_then_live',
      ],
      no_live_substitute_available: false,
    },
    {
      id: 'P3_external_video_ppt_live_smoke',
      status: 'pending_credentials',
      requires: [
        'external_non_customer_write_approval',
        'inbound_bearer',
        'connection_id',
        'source_id',
        'safe_video_input',
      ],
      no_live_substitute_available: false,
    },
    {
      id: 'P4_login_gated_handoff_live_pass',
      status: 'pending_deployment_approval',
      requires: [
        'server_8_deployment_window_approval',
        'main_handoff_live_smoke',
        'external_context_if_external_mode',
      ],
      no_live_substitute_available: false,
    },
    {
      id: 'P5_authorized_capture_live_sample',
      status: 'pending_authorization',
      requires: [
        'operator_approval_record',
        'playable_authorized_source',
        'retention_policy',
        'manual_mp4_review_before_extraction',
      ],
      no_live_substitute_available: false,
    },
    {
      id: 'P6_customer_authorized_quality_matrix',
      status: 'pending_customer_input',
      requires: [
        'customer_or_operator_authorized_sample',
        'customer_approval_id',
        'retention_policy',
      ],
      no_live_substitute_available: false,
    },
    {
      id: 'P7_server_deployment_gate',
      status: 'pending_deployment_approval',
      requires: [
        'explicit_server_8_deployment_window',
        'pre_deploy_local_gate',
        'post_deploy_target_smoke',
      ],
      no_live_substitute_available: false,
    },
    {
      id: 'P8_full_acceptance_close',
      status: 'pending_live_and_customer_gates',
      requires: [
        'P2_or_explicit_not_executable_reason',
        'P3_or_explicit_not_executable_reason',
        'P4_live_pass_or_undeployed_reason',
        'P5_live_sample_or_missing_approval_reason',
        'P6_customer_matrix_or_pending_customer_reason',
      ],
      no_live_substitute_available: false,
    },
  ];
  const pendingGateRequirementsSummary = buildPendingGateRequirementsSummary(gates);
  const approvalRequestSummary = buildApprovalRequestSummary(gates);
  return {
    schema: 'v3.video_ppt_acceptance_status_rollup.v1',
    full_acceptance_ready: false,
    current_phase: 'no_live_local_baseline',
    no_live_status: noLivePassed ? 'passed' : 'failed',
    no_live_command_count: summary.command_count,
    no_live_passed_count: summary.passed_count,
    no_live_failed_count: summary.failed_count,
    gate_count: gates.length,
    completed_gate_count: noLivePassed ? 1 : 0,
    pending_authorization_gate_count: 2,
    pending_credentials_gate_count: 1,
    pending_deployment_gate_count: 2,
    pending_customer_gate_count: 1,
    pending_full_acceptance_gate_count: 1,
    no_live_evidence_summary: noLiveEvidenceSummary,
    live_gate_readiness_summary: liveGateReadinessSummary,
    pending_gate_requirements_summary: pendingGateRequirementsSummary,
    approval_request_summary: approvalRequestSummary,
    gates,
    next_authorized_paths: [
      'P2_main_upload_live_smoke',
      'P3_external_video_ppt_live_smoke',
      'P7_then_P4_login_gated_handoff_live_pass',
      'P5_authorized_capture_live_sample',
      'P6_customer_authorized_quality_matrix',
    ],
    safe_local_next_actions: [
      'public_quality_fixture_narrow_fix',
      'no_live_acceptance_regression_maintenance',
    ],
    safety: {
      live_smoke_run: false,
      production_write_allowed: false,
      browser_capture_allowed: false,
      service_deployment_allowed: false,
      server_8_touched: false,
      server_120_touched: false,
    },
  };
}

function buildApprovalRequestSummary(gates = []) {
  const gateById = new Map(gates.map((gate) => [gate.id, gate]));
  const actionableGateIds = [
    'P2_main_upload_live_smoke',
    'P3_external_video_ppt_live_smoke',
    'P4_login_gated_handoff_live_pass',
    'P5_authorized_capture_live_sample',
    'P6_customer_authorized_quality_matrix',
    'P7_server_deployment_gate',
  ];
  const actionableGates = actionableGateIds
    .map((gateId) => gateById.get(gateId))
    .filter(Boolean);
  return {
    schema: 'v3.video_ppt_approval_request_summary.v1',
    request_count: actionableGates.length,
    request_gate_ids: actionableGateIds,
    safe_to_share: true,
    raw_values_included: false,
    no_live_substitute_available_for_requests: false,
    main_upload_requires_write_approval: true,
    main_upload_requires_safe_video_input: true,
    main_upload_required_ack_flag: '--ack-live-write',
    main_upload_required_approval_id_placeholder: '<approval_ref>',
    external_requires_write_approval: true,
    external_requires_inbound_bearer: true,
    external_requires_connection_id: true,
    external_requires_source_id: true,
    external_requires_safe_video_input: true,
    external_required_ack_flag: '--ack-live-write',
    external_required_approval_id_placeholder: '<approval_ref>',
    login_gated_handoff_requires_server_8_deployment_window: true,
    authorized_capture_requires_approval_record: true,
    authorized_capture_requires_playable_authorized_source: true,
    authorized_capture_requires_retention_policy: true,
    customer_quality_matrix_requires_authorized_sample: true,
    customer_quality_matrix_requires_approval_id: true,
    customer_quality_matrix_requires_retention_policy: true,
    server_deployment_requires_explicit_window: true,
    full_acceptance_waits_on_live_customer_deployment:
      Boolean(gateById.get('P8_full_acceptance_close')),
  };
}

function buildPendingGateRequirementsSummary(gates = []) {
  const pendingGates = gates.filter((gate) => (
    gate.no_live_substitute_available === false
    && String(gate.status || '').startsWith('pending')
  ));
  const pendingGateIds = pendingGates.map((gate) => gate.id);
  return {
    schema: 'v3.video_ppt_pending_gate_requirements_summary.v1',
    pending_gate_count: pendingGates.length,
    pending_gate_ids: pendingGateIds,
    main_upload_requires_write_approval: pendingGateIds.includes('P2_main_upload_live_smoke'),
    external_video_ppt_requires_credentials:
      pendingGateIds.includes('P3_external_video_ppt_live_smoke'),
    external_video_ppt_requires_write_approval:
      pendingGateIds.includes('P3_external_video_ppt_live_smoke'),
    login_gated_handoff_requires_deployment_approval:
      pendingGateIds.includes('P4_login_gated_handoff_live_pass'),
    authorized_capture_requires_approval_record:
      pendingGateIds.includes('P5_authorized_capture_live_sample'),
    customer_quality_matrix_requires_authorized_sample:
      pendingGateIds.includes('P6_customer_authorized_quality_matrix'),
    server_deployment_requires_explicit_window:
      pendingGateIds.includes('P7_server_deployment_gate'),
    full_acceptance_waits_on_live_customer_deployment:
      pendingGateIds.includes('P8_full_acceptance_close'),
    no_live_substitute_available_for_pending_gates: false,
  };
}

function buildLiveGateReadinessSummary(results = []) {
  const commandById = new Map(results.map((result) => [result.id, result]));
  const uploadSelfTest = commandById.get('upload_main_self_test')?.evidence || {};
  const uploadPreflight = commandById.get('upload_main_preflight')?.evidence || {};
  const uploadApprovalNegative = commandById.get('upload_main_live_approval_gate_negative')?.evidence || {};
  const externalSelfTest = commandById.get('external_video_ppt_self_test')?.evidence || {};
  const externalPreflight = commandById.get('external_video_ppt_preflight')?.evidence || {};
  const externalApprovalNegative =
    commandById.get('external_video_ppt_live_approval_gate_negative')?.evidence || {};
  const handoffPreflight = commandById.get('video_ppt_handoff_preflight')?.evidence || {};
  const captureDryRun = commandById.get('authorized_capture_dry_run')?.evidence || {};
  const qualityEvidence = commandById.get('quality_matrix_self_test')?.evidence || {};
  return {
    schema: 'v3.video_ppt_live_gate_readiness_summary.v1',
    main_upload_preflight_ready: uploadPreflight.ok === true && uploadPreflight.preflight === true,
    main_upload_live_write_approval_required: uploadPreflight.live_write_approval_required === true,
    main_upload_live_write_approval_gate_enforced:
      uploadPreflight.live_write_approval_gate_enforced === true
      && uploadApprovalNegative.approval_gate_enforced === true,
    main_upload_live_approval_command_template_ready:
      uploadPreflight.command_template_has_ack_live_write === true
      && uploadPreflight.command_template_has_redacted_approval_id === true
      && uploadPreflight.command_template_approval_values_included === false
      && uploadPreflight.command_template_raw_url_included === false
      && uploadPreflight.command_template_local_path_included === false,
    main_upload_live_without_approval_rejected_before_network:
      uploadApprovalNegative.approval_gate_enforced === true
      && uploadApprovalNegative.network_calls_run === false
      && uploadApprovalNegative.production_write_allowed === false,
    main_upload_writes_smoke_records: uploadPreflight.writes_smoke_records === true,
    main_upload_artifact_self_test_ready:
      uploadSelfTest.deliverable_state === 'final_pptx_ready'
      && uploadSelfTest.required_file_kind_count >= 6
      && uploadSelfTest.download_validation_ok === true,
    main_upload_artifact_required_file_kind_count: uploadSelfTest.required_file_kind_count ?? null,
    main_upload_artifact_pptx_slide_count: uploadSelfTest.pptx_slide_count ?? null,
    main_upload_artifact_markdown_slide_heading_count:
      uploadSelfTest.markdown_slide_heading_count ?? null,
    main_upload_artifact_live_download_pending:
      uploadPreflight.live_write_approval_required === true
      && uploadPreflight.upload_attempted === false
      && uploadPreflight.assistant_run_created === false,
    external_preflight_ready: externalPreflight.ok === true && externalPreflight.preflight === true,
    external_live_write_approval_required: externalPreflight.live_write_approval_required === true,
    external_live_write_approval_gate_enforced:
      externalPreflight.live_write_approval_gate_enforced === true
      && externalApprovalNegative.approval_gate_enforced === true,
    external_live_approval_command_template_ready:
      externalPreflight.command_template_has_ack_live_write === true
      && externalPreflight.command_template_has_redacted_approval_id === true
      && externalPreflight.command_template_has_redacted_bearer === true
      && externalPreflight.command_template_approval_values_included === false
      && externalPreflight.command_template_raw_url_included === false
      && externalPreflight.command_template_local_path_included === false,
    external_live_without_approval_rejected_before_network:
      externalApprovalNegative.approval_gate_enforced === true
      && externalApprovalNegative.network_calls_run === false
      && externalApprovalNegative.production_write_allowed === false,
    external_context_present: externalPreflight.connection_id_present === true
      && externalPreflight.source_id_present === true,
    external_live_credential_ready: externalPreflight.live_credential_ready === true,
    external_artifact_surface_self_test_ready:
      externalSelfTest.surface_ok === true
      && externalSelfTest.surface_missing_export_kind_count === 0
      && externalSelfTest.download_validation_ok === true,
    external_artifact_required_export_kind_count: 6,
    external_artifact_export_kind_count: externalSelfTest.surface_export_kind_count ?? null,
    external_artifact_link_count: externalSelfTest.surface_artifact_link_count ?? null,
    external_artifact_live_download_pending:
      externalPreflight.deliverables_downloaded_from_network === false
      && externalPreflight.live_credential_ready === false,
    handoff_preflight_ready: handoffPreflight.ok === true && handoffPreflight.preflight === true,
    handoff_deployment_approval_required: handoffPreflight.deployment_approval_required === true,
    handoff_live_credential_ready: handoffPreflight.live_credential_ready === true,
    handoff_target_mode_count: handoffPreflight.target_mode_count ?? null,
    authorized_capture_dry_run_ready: captureDryRun.ok === true && captureDryRun.dry_run === true,
    authorized_capture_approval_reference_present: captureDryRun.approval_reference_present === true,
    authorized_capture_no_capture_attempted: captureDryRun.capture_attempted === false,
    customer_quality_matrix_argument_gates_ready:
      qualityEvidence.customer_authorization_argument_gate_supported === true
      && qualityEvidence.customer_retention_policy_argument_gate_supported === true,
    customer_quality_matrix_retention_policy_required: true,
    customer_quality_matrix_customer_sample_pending: true,
    live_approval_command_template_surface_count: [
      uploadPreflight,
      externalPreflight,
    ].filter((evidence) =>
      evidence.command_template_has_ack_live_write === true
      && evidence.command_template_has_redacted_approval_id === true
      && evidence.command_template_approval_values_included === false
      && evidence.command_template_raw_url_included === false
      && evidence.command_template_local_path_included === false).length,
    pending_main_live_write_approval: uploadPreflight.live_write_approval_required === true,
    pending_external_bearer: externalPreflight.live_credential_ready === false,
    pending_server_8_deployment_approval: handoffPreflight.deployment_approval_required === true,
    pending_authorized_capture_live_sample: true,
    pending_customer_authorized_sample: true,
    live_or_deploy_action_run: false,
  };
}

function buildNoLiveAcceptanceEvidenceSummary(results = []) {
  const commandById = new Map(results.map((result) => [result.id, result]));
  const uploadEvidence = commandById.get('upload_main_self_test')?.evidence || {};
  const externalEvidence = commandById.get('external_video_ppt_self_test')?.evidence || {};
  const uploadPreflight = commandById.get('upload_main_preflight')?.evidence || {};
  const externalPreflight = commandById.get('external_video_ppt_preflight')?.evidence || {};
  const uploadApprovalNegative =
    commandById.get('upload_main_live_approval_gate_negative')?.evidence || {};
  const externalApprovalNegative =
    commandById.get('external_video_ppt_live_approval_gate_negative')?.evidence || {};
  const qualityEvidence = commandById.get('quality_matrix_self_test')?.evidence || {};
  const videoUrlResolutionEvidence =
    commandById.get('platform_api_video_url_resolution_tests')?.evidence || {};
  const publicVideoPageEvidence =
    commandById.get('platform_api_public_video_page_tests')?.evidence || {};
  const startupBriefingEvidence =
    commandById.get('assistant_startup_briefing_tests')?.evidence || {};
  const passedEvidenceCommandCount = results.filter((result) => (
    result.status === 'passed' && result.evidence
  )).length;
  const uploadSupportedVideoExtensionCount = Number.isInteger(
    uploadEvidence.supported_video_extension_count,
  ) ? uploadEvidence.supported_video_extension_count : null;
  const externalSupportedVideoExtensionCount = Number.isInteger(
    externalEvidence.supported_video_extension_count,
  ) ? externalEvidence.supported_video_extension_count : null;
  const supportedVideoExtensionCounts = [
    uploadSupportedVideoExtensionCount,
    externalSupportedVideoExtensionCount,
  ].filter((count) => Number.isInteger(count));
  const supportedVideoExtensionEvidenceSurfaceCount = supportedVideoExtensionCounts
    .filter((count) => count >= 6).length;
  return {
    schema: 'v3.video_ppt_no_live_acceptance_evidence_summary.v1',
    embedded_evidence_command_count: passedEvidenceCommandCount,
    upload_main_self_test_evidence: hasPassedEvidence(commandById, 'upload_main_self_test'),
    upload_main_preflight_evidence: hasPassedEvidence(commandById, 'upload_main_preflight'),
    upload_main_live_approval_negative_evidence:
      hasPassedEvidence(commandById, 'upload_main_live_approval_gate_negative'),
    external_video_ppt_self_test_evidence: hasPassedEvidence(commandById, 'external_video_ppt_self_test'),
    external_video_ppt_preflight_evidence: hasPassedEvidence(commandById, 'external_video_ppt_preflight'),
    external_video_ppt_live_approval_negative_evidence:
      hasPassedEvidence(commandById, 'external_video_ppt_live_approval_gate_negative'),
    handoff_self_test_evidence: hasPassedEvidence(commandById, 'video_ppt_handoff_self_test'),
    handoff_preflight_evidence: hasPassedEvidence(commandById, 'video_ppt_handoff_preflight'),
    authorized_capture_self_test_evidence: hasPassedEvidence(commandById, 'authorized_capture_self_test'),
    authorized_capture_dry_run_evidence: hasPassedEvidence(commandById, 'authorized_capture_dry_run'),
    quality_matrix_self_test_evidence: hasPassedEvidence(commandById, 'quality_matrix_self_test'),
    production_scope_planner_evidence: hasPassedEvidence(commandById, 'scope_planner_tests'),
    assistant_startup_briefing_evidence:
      hasPassedEvidence(commandById, 'assistant_startup_briefing_tests'),
    assistant_startup_briefing_test_count:
      startupBriefingEvidence.test_count ?? null,
    assistant_startup_briefing_pass_count:
      startupBriefingEvidence.pass_count ?? null,
    assistant_startup_briefing_video_ppt_boundary_covered:
      startupBriefingEvidence.video_ppt_source_routing_boundary_covered === true,
    assistant_startup_briefing_system_capability_covered:
      startupBriefingEvidence.system_capability_test_covered === true,
    assistant_startup_briefing_no_dataset_boundary_covered:
      startupBriefingEvidence.no_dataset_formatted_boundary_test_covered === true,
    production_assistant_runtime_evidence:
      hasPassedEvidence(commandById, 'assistant_runtime_video_ppt_scope_tests'),
    platform_api_video_url_resolution_evidence:
      hasPassedEvidence(commandById, 'platform_api_video_url_resolution_tests'),
    platform_api_public_video_page_evidence:
      hasPassedEvidence(commandById, 'platform_api_public_video_page_tests'),
    platform_api_video_url_resolution_test_count:
      videoUrlResolutionEvidence.test_count ?? null,
    platform_api_video_url_resolution_pass_count:
      videoUrlResolutionEvidence.pass_count ?? null,
    platform_api_public_video_page_test_count:
      publicVideoPageEvidence.test_count ?? null,
    platform_api_public_video_page_pass_count:
      publicVideoPageEvidence.pass_count ?? null,
    direct_video_url_resolution_covered:
      videoUrlResolutionEvidence.accepts_direct_video_url_covered === true
      && videoUrlResolutionEvidence.chinese_punctuation_direct_url_covered === true,
    login_gated_video_source_rejection_covered:
      videoUrlResolutionEvidence.login_gated_source_blocked_covered === true
      && publicVideoPageEvidence.public_page_blocks_private_or_login_gated_sources_covered === true,
    public_page_video_source_extraction_covered:
      publicVideoPageEvidence.public_page_extracts_video_sources_covered === true
      && publicVideoPageEvidence.public_page_unregistered_result_keeps_resolved_video_covered === true,
    public_page_no_video_asset_failure_covered:
      publicVideoPageEvidence.public_page_no_video_asset_covered === true,
    public_page_resolver_failure_classification_covered:
      publicVideoPageEvidence.public_page_failure_result_structured_covered === true
      && publicVideoPageEvidence.public_page_failure_result_classification_covered === true,
    required_video_extension_count: 6,
    upload_main_supported_video_extension_count: uploadSupportedVideoExtensionCount,
    external_supported_video_extension_count: externalSupportedVideoExtensionCount,
    supported_video_extension_min_count: supportedVideoExtensionCounts.length > 0
      ? Math.min(...supportedVideoExtensionCounts)
      : null,
    supported_video_extension_evidence_surface_count:
      supportedVideoExtensionEvidenceSurfaceCount,
    supported_video_extension_evidence_ready:
      supportedVideoExtensionEvidenceSurfaceCount === 2
      && externalEvidence.unsupported_non_video_extensions_rejected === true,
    unsupported_non_video_extensions_rejected:
      externalEvidence.unsupported_non_video_extensions_rejected === true,
    live_approval_self_test_gate_surface_count: [
      uploadEvidence,
      externalEvidence,
    ].filter((evidence) =>
      evidence.live_write_approval_gate_schema
        === 'v3.video_ppt_live_write_approval_gate_contract.v1'
      && evidence.live_write_approval_rejects_missing_ack_and_approval === true
      && evidence.live_write_approval_rejects_missing_approval === true
      && evidence.live_write_approval_rejects_missing_ack === true
      && evidence.live_write_approval_accepts_complete_live_approval_shape === true
      && evidence.live_write_approval_values_included === false).length,
    upload_main_live_approval_self_test_gate_supported:
      uploadEvidence.live_write_approval_gate_case_count >= 6
      && uploadEvidence.live_write_approval_rejects_missing_ack_and_approval === true
      && uploadEvidence.live_write_approval_rejects_missing_approval === true
      && uploadEvidence.live_write_approval_rejects_missing_ack === true
      && uploadEvidence.live_write_approval_accepts_complete_live_approval_shape === true
      && uploadEvidence.live_write_approval_values_included === false,
    external_live_approval_self_test_gate_supported:
      externalEvidence.live_write_approval_gate_case_count >= 6
      && externalEvidence.live_write_approval_rejects_missing_ack_and_approval === true
      && externalEvidence.live_write_approval_rejects_missing_approval === true
      && externalEvidence.live_write_approval_rejects_missing_ack === true
      && externalEvidence.live_write_approval_accepts_complete_live_approval_shape === true
      && externalEvidence.live_write_approval_values_included === false,
    live_approval_command_template_self_test_surface_count: [
      hasCompleteLiveCommandTemplateSelfTestContract(uploadEvidence),
      hasCompleteLiveCommandTemplateSelfTestContract(externalEvidence, true),
    ].filter(Boolean).length,
    upload_main_live_approval_command_template_self_test_supported:
      hasCompleteLiveCommandTemplateSelfTestContract(uploadEvidence),
    external_live_approval_command_template_self_test_supported:
      hasCompleteLiveCommandTemplateSelfTestContract(externalEvidence, true),
    live_approval_command_template_surface_count: [
      uploadPreflight,
      externalPreflight,
    ].filter((evidence) =>
      evidence.command_template_has_ack_live_write === true
      && evidence.command_template_has_redacted_approval_id === true
      && evidence.command_template_approval_values_included === false
      && evidence.command_template_raw_url_included === false
      && evidence.command_template_local_path_included === false).length,
    upload_main_live_approval_command_template_ready:
      uploadPreflight.command_template_has_ack_live_write === true
      && uploadPreflight.command_template_has_redacted_approval_id === true
      && uploadPreflight.command_template_approval_values_included === false
      && uploadPreflight.command_template_raw_url_included === false
      && uploadPreflight.command_template_local_path_included === false,
    external_live_approval_command_template_ready:
      externalPreflight.command_template_has_ack_live_write === true
      && externalPreflight.command_template_has_redacted_approval_id === true
      && externalPreflight.command_template_has_redacted_bearer === true
      && externalPreflight.command_template_approval_values_included === false
      && externalPreflight.command_template_raw_url_included === false
      && externalPreflight.command_template_local_path_included === false,
    live_approval_negative_gate_count: [
      uploadApprovalNegative,
      externalApprovalNegative,
    ].filter((evidence) => evidence.approval_gate_enforced === true).length,
    live_without_approval_rejected_before_network:
      uploadApprovalNegative.approval_gate_enforced === true
      && externalApprovalNegative.approval_gate_enforced === true
      && uploadApprovalNegative.network_calls_run === false
      && externalApprovalNegative.network_calls_run === false
      && uploadApprovalNegative.production_write_allowed === false
      && externalApprovalNegative.production_write_allowed === false,
    main_upload_artifact_surface_ready:
      uploadEvidence.deliverable_state === 'final_pptx_ready'
      && uploadEvidence.required_file_kind_count >= 6
      && uploadEvidence.download_validation_ok === true,
    main_upload_artifact_required_file_kind_count: uploadEvidence.required_file_kind_count ?? null,
    main_upload_artifact_pptx_slide_count: uploadEvidence.pptx_slide_count ?? null,
    main_upload_artifact_markdown_slide_heading_count:
      uploadEvidence.markdown_slide_heading_count ?? null,
    main_upload_artifact_download_validation_ok:
      uploadEvidence.download_validation_ok === true,
    external_artifact_surface_ready:
      externalEvidence.surface_ok === true
      && externalEvidence.surface_missing_export_kind_count === 0
      && externalEvidence.download_validation_ok === true,
    external_artifact_required_export_kind_count: 6,
    external_artifact_export_kind_count: externalEvidence.surface_export_kind_count ?? null,
    external_artifact_missing_export_kind_count:
      externalEvidence.surface_missing_export_kind_count ?? null,
    external_artifact_link_count: externalEvidence.surface_artifact_link_count ?? null,
    external_artifact_download_validation_ok: externalEvidence.download_validation_ok === true,
    live_preflight_evidence_count: [
      'upload_main_preflight',
      'external_video_ppt_preflight',
      'video_ppt_handoff_preflight',
    ].filter((commandId) => hasPassedEvidence(commandById, commandId)).length,
    quality_matrix_case_count: qualityEvidence.case_count ?? null,
    quality_matrix_deliverable_count: qualityEvidence.deliverable_count ?? null,
    quality_matrix_pending_count: qualityEvidence.pending_count ?? null,
    quality_customer_authorization_argument_gate_supported:
      qualityEvidence.customer_authorization_argument_gate_supported === true,
    quality_customer_retention_policy_argument_gate_supported:
      qualityEvidence.customer_retention_policy_argument_gate_supported === true,
    quality_review_required_risk_flag_count: qualityEvidence.review_required_risk_flag_count ?? null,
    not_deliverable_failure_class_count: qualityEvidence.not_deliverable_failure_class_count ?? null,
    review_failure_class_summary_needs_manual_review_count:
      qualityEvidence.review_failure_class_summary_needs_manual_review_count ?? null,
    review_failure_class_summary_counts:
      sanitizeCountMap(qualityEvidence.review_failure_class_summary_counts),
    not_deliverable_regression_counts:
      sanitizeCountMap(qualityEvidence.failure_class_summary_regression_not_deliverable_counts),
  };
}

function hasPassedEvidence(commandById, commandId) {
  const result = commandById.get(commandId);
  return Boolean(result && result.status === 'passed' && result.evidence);
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
  validateUploadMainPreflightEvidence(report);
  validateLiveApprovalNegativeEvidence(report, 'upload_main_live_approval_gate_negative');
  validateExternalVideoPptEvidence(report);
  validateExternalVideoPptPreflightEvidence(report);
  validateLiveApprovalNegativeEvidence(report, 'external_video_ppt_live_approval_gate_negative');
  validateVideoPptHandoffEvidence(report);
  validateVideoPptHandoffPreflightEvidence(report);
  validateAuthorizedCaptureEvidence(report);
  validateAuthorizedCaptureDryRunEvidence(report);
  validateProductionTriggerEvidence(report);
  validateAssistantStartupBriefingEvidence(report);
  validatePlatformApiResolverEvidence(report);
  validateAcceptanceStatus(report);
  const serialized = JSON.stringify(report);
  if (serialized.match(/[A-Za-z]:[\\/]|[\\/]Users[\\/]|[\\/]home[\\/]|https?:\/\/|token=|cookie=|bearer=/i)) {
    throw new Error('no-live rollup report contains unredacted local path, URL, or token-like text');
  }
}

function validateAcceptanceStatus(report) {
  const acceptance = report.acceptance_status;
  const expectedNoLiveStatus = report.summary.failed_count === 0 ? 'passed' : 'failed';
  const expectedCompletedGateCount = report.summary.failed_count === 0 ? 1 : 0;
  if (
    !acceptance
    || acceptance.schema !== 'v3.video_ppt_acceptance_status_rollup.v1'
    || acceptance.full_acceptance_ready !== false
    || acceptance.current_phase !== 'no_live_local_baseline'
    || acceptance.no_live_command_count !== report.summary.command_count
    || acceptance.no_live_passed_count !== report.summary.passed_count
    || acceptance.no_live_failed_count !== report.summary.failed_count
    || acceptance.no_live_status !== expectedNoLiveStatus
    || acceptance.gate_count !== 8
    || acceptance.completed_gate_count !== expectedCompletedGateCount
    || acceptance.pending_authorization_gate_count !== 2
    || acceptance.pending_credentials_gate_count !== 1
    || acceptance.pending_deployment_gate_count !== 2
    || acceptance.pending_customer_gate_count !== 1
    || acceptance.pending_full_acceptance_gate_count !== 1
    || !Array.isArray(acceptance.gates)
    || acceptance.gates.length !== 8
    || acceptance.safety?.live_smoke_run !== false
    || acceptance.safety?.production_write_allowed !== false
    || acceptance.safety?.browser_capture_allowed !== false
    || acceptance.safety?.service_deployment_allowed !== false
    || acceptance.safety?.server_8_touched !== false
    || acceptance.safety?.server_120_touched !== false
  ) {
    throw new Error('no-live rollup acceptance status matrix is incomplete');
  }
  const gateById = new Map((acceptance.gates || []).map((gate) => [gate.id, gate]));
  const noLiveGate = gateById.get('P1_no_live_baseline');
  if (
    !noLiveGate
    || noLiveGate.status !== expectedNoLiveStatus
    || noLiveGate.no_live_substitute_available !== true
  ) {
    throw new Error('no-live rollup acceptance status no-live gate is incomplete');
  }
  validateNoLiveAcceptanceEvidenceSummary(acceptance, report);
  validateLiveGateReadinessSummary(acceptance, report);
  validatePendingGateRequirementsSummary(acceptance, report);
  validateApprovalRequestSummary(acceptance, report);
  for (const gateId of [
    'P2_main_upload_live_smoke',
    'P3_external_video_ppt_live_smoke',
    'P4_login_gated_handoff_live_pass',
    'P5_authorized_capture_live_sample',
    'P6_customer_authorized_quality_matrix',
    'P7_server_deployment_gate',
    'P8_full_acceptance_close',
  ]) {
    const gate = gateById.get(gateId);
    if (
      !gate
      || gate.no_live_substitute_available !== false
      || !Array.isArray(gate.requires)
      || gate.requires.length === 0
      || !String(gate.status || '').startsWith('pending')
    ) {
      throw new Error(`no-live rollup acceptance status gate is incomplete: ${gateId}`);
    }
  }
}

function validateApprovalRequestSummary(acceptance, report) {
  const summary = acceptance.approval_request_summary;
  if (
    !summary
    || summary.schema !== 'v3.video_ppt_approval_request_summary.v1'
    || summary.safe_to_share !== true
    || summary.raw_values_included !== false
    || summary.no_live_substitute_available_for_requests !== false
  ) {
    throw new Error('no-live rollup approval request summary is incomplete');
  }
  if (report.summary.failed_count !== 0) {
    return;
  }
  const requiredGateIds = [
    'P2_main_upload_live_smoke',
    'P3_external_video_ppt_live_smoke',
    'P4_login_gated_handoff_live_pass',
    'P5_authorized_capture_live_sample',
    'P6_customer_authorized_quality_matrix',
    'P7_server_deployment_gate',
  ];
  const requestGateIds = Array.isArray(summary.request_gate_ids)
    ? summary.request_gate_ids
    : [];
  if (
    summary.request_count !== requiredGateIds.length
    || requestGateIds.length !== requiredGateIds.length
    || requiredGateIds.some((gateId) => !requestGateIds.includes(gateId))
    || summary.main_upload_requires_write_approval !== true
    || summary.main_upload_requires_safe_video_input !== true
    || summary.main_upload_required_ack_flag !== '--ack-live-write'
    || summary.main_upload_required_approval_id_placeholder !== '<approval_ref>'
    || summary.external_requires_write_approval !== true
    || summary.external_requires_inbound_bearer !== true
    || summary.external_requires_connection_id !== true
    || summary.external_requires_source_id !== true
    || summary.external_requires_safe_video_input !== true
    || summary.external_required_ack_flag !== '--ack-live-write'
    || summary.external_required_approval_id_placeholder !== '<approval_ref>'
    || summary.login_gated_handoff_requires_server_8_deployment_window !== true
    || summary.authorized_capture_requires_approval_record !== true
    || summary.authorized_capture_requires_playable_authorized_source !== true
    || summary.authorized_capture_requires_retention_policy !== true
    || summary.customer_quality_matrix_requires_authorized_sample !== true
    || summary.customer_quality_matrix_requires_approval_id !== true
    || summary.customer_quality_matrix_requires_retention_policy !== true
    || summary.server_deployment_requires_explicit_window !== true
    || summary.full_acceptance_waits_on_live_customer_deployment !== true
  ) {
    throw new Error('no-live rollup approval request summary does not match pending gates');
  }
}

function validatePendingGateRequirementsSummary(acceptance, report) {
  const pending = acceptance.pending_gate_requirements_summary;
  if (
    !pending
    || pending.schema !== 'v3.video_ppt_pending_gate_requirements_summary.v1'
    || pending.no_live_substitute_available_for_pending_gates !== false
  ) {
    throw new Error('no-live rollup pending gate requirements summary is incomplete');
  }
  if (report.summary.failed_count !== 0) {
    return;
  }
  const requiredGateIds = [
    'P2_main_upload_live_smoke',
    'P3_external_video_ppt_live_smoke',
    'P4_login_gated_handoff_live_pass',
    'P5_authorized_capture_live_sample',
    'P6_customer_authorized_quality_matrix',
    'P7_server_deployment_gate',
    'P8_full_acceptance_close',
  ];
  const pendingGateIds = Array.isArray(pending.pending_gate_ids)
    ? pending.pending_gate_ids
    : [];
  if (
    pending.pending_gate_count !== requiredGateIds.length
    || pendingGateIds.length !== requiredGateIds.length
    || requiredGateIds.some((gateId) => !pendingGateIds.includes(gateId))
    || pending.main_upload_requires_write_approval !== true
    || pending.external_video_ppt_requires_credentials !== true
    || pending.external_video_ppt_requires_write_approval !== true
    || pending.login_gated_handoff_requires_deployment_approval !== true
    || pending.authorized_capture_requires_approval_record !== true
    || pending.customer_quality_matrix_requires_authorized_sample !== true
    || pending.server_deployment_requires_explicit_window !== true
    || pending.full_acceptance_waits_on_live_customer_deployment !== true
  ) {
    throw new Error('no-live rollup pending gate requirements summary does not match pending gates');
  }
}

function validateLiveGateReadinessSummary(acceptance, report) {
  const readiness = acceptance.live_gate_readiness_summary;
  if (
    !readiness
    || readiness.schema !== 'v3.video_ppt_live_gate_readiness_summary.v1'
    || readiness.live_or_deploy_action_run !== false
  ) {
    throw new Error('no-live rollup live gate readiness summary is incomplete');
  }
  if (report.summary.failed_count !== 0) {
    return;
  }
  if (
    readiness.main_upload_preflight_ready !== true
    || readiness.main_upload_live_write_approval_required !== true
    || readiness.main_upload_live_write_approval_gate_enforced !== true
    || readiness.main_upload_live_approval_command_template_ready !== true
    || readiness.main_upload_live_without_approval_rejected_before_network !== true
    || readiness.main_upload_writes_smoke_records !== true
    || readiness.main_upload_artifact_self_test_ready !== true
    || readiness.main_upload_artifact_required_file_kind_count < 6
    || readiness.main_upload_artifact_pptx_slide_count < 1
    || readiness.main_upload_artifact_markdown_slide_heading_count < 1
    || readiness.main_upload_artifact_live_download_pending !== true
    || readiness.external_preflight_ready !== true
    || readiness.external_live_write_approval_required !== true
    || readiness.external_live_write_approval_gate_enforced !== true
    || readiness.external_live_approval_command_template_ready !== true
    || readiness.external_live_without_approval_rejected_before_network !== true
    || readiness.external_context_present !== true
    || readiness.external_live_credential_ready !== false
    || readiness.external_artifact_surface_self_test_ready !== true
    || readiness.external_artifact_required_export_kind_count !== 6
    || readiness.external_artifact_export_kind_count < 6
    || readiness.external_artifact_link_count < 1
    || readiness.external_artifact_live_download_pending !== true
    || readiness.handoff_preflight_ready !== true
    || readiness.handoff_deployment_approval_required !== true
    || readiness.handoff_live_credential_ready !== false
    || readiness.handoff_target_mode_count !== 2
    || readiness.authorized_capture_dry_run_ready !== true
    || readiness.authorized_capture_approval_reference_present !== true
    || readiness.authorized_capture_no_capture_attempted !== true
    || readiness.customer_quality_matrix_argument_gates_ready !== true
    || readiness.customer_quality_matrix_retention_policy_required !== true
    || readiness.customer_quality_matrix_customer_sample_pending !== true
    || readiness.live_approval_command_template_surface_count !== 2
    || readiness.pending_main_live_write_approval !== true
    || readiness.pending_external_bearer !== true
    || readiness.pending_server_8_deployment_approval !== true
    || readiness.pending_authorized_capture_live_sample !== true
    || readiness.pending_customer_authorized_sample !== true
  ) {
    throw new Error('no-live rollup live gate readiness summary does not match pending live gates');
  }
}

function validateNoLiveAcceptanceEvidenceSummary(acceptance, report) {
  const evidence = acceptance.no_live_evidence_summary;
  if (
    !evidence
    || evidence.schema !== 'v3.video_ppt_no_live_acceptance_evidence_summary.v1'
    || !Number.isInteger(evidence.embedded_evidence_command_count)
    || evidence.embedded_evidence_command_count < 0
  ) {
    throw new Error('no-live rollup acceptance evidence summary is incomplete');
  }
  if (report.summary.failed_count !== 0) {
    return;
  }
  if (
    evidence.embedded_evidence_command_count < 13
    || evidence.upload_main_self_test_evidence !== true
    || evidence.upload_main_preflight_evidence !== true
    || evidence.upload_main_live_approval_negative_evidence !== true
    || evidence.external_video_ppt_self_test_evidence !== true
    || evidence.external_video_ppt_preflight_evidence !== true
    || evidence.external_video_ppt_live_approval_negative_evidence !== true
    || evidence.handoff_self_test_evidence !== true
    || evidence.handoff_preflight_evidence !== true
    || evidence.authorized_capture_self_test_evidence !== true
    || evidence.authorized_capture_dry_run_evidence !== true
    || evidence.quality_matrix_self_test_evidence !== true
    || evidence.production_scope_planner_evidence !== true
    || evidence.assistant_startup_briefing_evidence !== true
    || evidence.assistant_startup_briefing_test_count < 9
    || evidence.assistant_startup_briefing_pass_count
      !== evidence.assistant_startup_briefing_test_count
    || evidence.assistant_startup_briefing_video_ppt_boundary_covered !== true
    || evidence.assistant_startup_briefing_system_capability_covered !== true
    || evidence.assistant_startup_briefing_no_dataset_boundary_covered !== true
    || evidence.production_assistant_runtime_evidence !== true
    || evidence.platform_api_video_url_resolution_evidence !== true
    || evidence.platform_api_public_video_page_evidence !== true
    || evidence.platform_api_video_url_resolution_test_count < 5
    || evidence.platform_api_video_url_resolution_pass_count
      !== evidence.platform_api_video_url_resolution_test_count
    || evidence.platform_api_public_video_page_test_count < 6
    || evidence.platform_api_public_video_page_pass_count
      !== evidence.platform_api_public_video_page_test_count
    || evidence.direct_video_url_resolution_covered !== true
    || evidence.login_gated_video_source_rejection_covered !== true
    || evidence.public_page_video_source_extraction_covered !== true
    || evidence.public_page_no_video_asset_failure_covered !== true
    || evidence.public_page_resolver_failure_classification_covered !== true
    || evidence.required_video_extension_count !== 6
    || evidence.upload_main_supported_video_extension_count < 6
    || evidence.external_supported_video_extension_count < 6
    || evidence.supported_video_extension_min_count < 6
    || evidence.supported_video_extension_evidence_surface_count !== 2
    || evidence.supported_video_extension_evidence_ready !== true
    || evidence.unsupported_non_video_extensions_rejected !== true
    || evidence.live_approval_self_test_gate_surface_count !== 2
    || evidence.upload_main_live_approval_self_test_gate_supported !== true
    || evidence.external_live_approval_self_test_gate_supported !== true
    || evidence.live_approval_command_template_self_test_surface_count !== 2
    || evidence.upload_main_live_approval_command_template_self_test_supported !== true
    || evidence.external_live_approval_command_template_self_test_supported !== true
    || evidence.live_approval_command_template_surface_count !== 2
    || evidence.upload_main_live_approval_command_template_ready !== true
    || evidence.external_live_approval_command_template_ready !== true
    || evidence.live_approval_negative_gate_count !== 2
    || evidence.live_without_approval_rejected_before_network !== true
    || evidence.main_upload_artifact_surface_ready !== true
    || evidence.main_upload_artifact_required_file_kind_count < 6
    || evidence.main_upload_artifact_pptx_slide_count < 1
    || evidence.main_upload_artifact_markdown_slide_heading_count < 1
    || evidence.main_upload_artifact_download_validation_ok !== true
    || evidence.external_artifact_surface_ready !== true
    || evidence.external_artifact_required_export_kind_count !== 6
    || evidence.external_artifact_export_kind_count < 6
    || evidence.external_artifact_missing_export_kind_count !== 0
    || evidence.external_artifact_link_count < 1
    || evidence.external_artifact_download_validation_ok !== true
    || evidence.live_preflight_evidence_count !== 3
    || evidence.quality_matrix_case_count !== 3
    || evidence.quality_matrix_deliverable_count !== 1
    || evidence.quality_matrix_pending_count !== 2
    || evidence.quality_customer_authorization_argument_gate_supported !== true
    || evidence.quality_customer_retention_policy_argument_gate_supported !== true
    || evidence.quality_review_required_risk_flag_count !== 8
    || evidence.not_deliverable_failure_class_count !== 5
    || evidence.review_failure_class_summary_needs_manual_review_count !== 8
    || !hasExpectedReviewFailureClassCounts(evidence.review_failure_class_summary_counts)
    || !hasRequiredFailureClassCounts(evidence.not_deliverable_regression_counts)
  ) {
    throw new Error('no-live rollup acceptance evidence summary does not cover all passed gates');
  }
}

function validateProductionTriggerEvidence(report) {
  const scopePlannerCommand = report.commands.find((result) => result.id === 'scope_planner_tests');
  if (scopePlannerCommand?.status === 'passed') {
    const evidence = scopePlannerCommand.evidence;
    if (
      !evidence
      || evidence.schema !== 'v3.scope_planner_video_ppt_rollup_evidence.v1'
      || evidence.test_count < 21
      || evidence.pass_count !== evidence.test_count
      || evidence.fail_count !== 0
      || evidence.cancelled_count !== 0
      || evidence.direct_video_ppt_positive_covered !== true
      || evidence.public_video_page_positive_covered !== true
      || evidence.uploaded_video_positive_covered !== true
      || evidence.mkv_avi_positive_covered !== true
      || evidence.transcript_only_negative_covered !== true
      || evidence.shared_negative_fixture_covered !== true
      || evidence.shared_positive_fixture_covered !== true
    ) {
      throw new Error('no-live rollup scope planner production trigger evidence is incomplete');
    }
  }

  const assistantRuntimeCommand = report.commands.find((result) => (
    result.id === 'assistant_runtime_video_ppt_scope_tests'
  ));
  if (assistantRuntimeCommand?.status === 'passed') {
    const evidence = assistantRuntimeCommand.evidence;
    if (
      !evidence
      || evidence.schema !== 'v3.assistant_runtime_video_ppt_scope_rollup_evidence.v1'
      || evidence.test_count < 3
      || evidence.pass_count !== evidence.test_count
      || evidence.fail_count !== 0
      || evidence.transcript_only_negative_covered !== true
      || evidence.shared_negative_fixture_covered !== true
      || evidence.shared_positive_fixture_covered !== true
    ) {
      throw new Error('no-live rollup assistant-runtime production trigger evidence is incomplete');
    }
  }
}

function validateAssistantStartupBriefingEvidence(report) {
  const command = report.commands.find((result) => (
    result.id === 'assistant_startup_briefing_tests'
  ));
  if (!command || command.status !== 'passed') {
    throw new Error('no-live rollup missing assistant startup briefing evidence');
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.assistant_startup_briefing_video_ppt_rollup_evidence.v1'
    || evidence.test_count < 9
    || evidence.pass_count !== evidence.test_count
    || evidence.fail_count !== 0
    || evidence.cancelled_count !== 0
    || evidence.system_capability_test_covered !== true
    || evidence.no_dataset_formatted_boundary_test_covered !== true
    || evidence.video_ppt_source_routing_boundary_covered !== true
  ) {
    throw new Error('no-live rollup assistant startup briefing evidence is incomplete');
  }
}

function validatePlatformApiResolverEvidence(report) {
  const videoUrlCommand = report.commands.find((result) => (
    result.id === 'platform_api_video_url_resolution_tests'
  ));
  if (!videoUrlCommand || videoUrlCommand.status !== 'passed') {
    throw new Error('no-live rollup missing platform-api video URL resolution evidence');
  }
  const videoUrlEvidence = videoUrlCommand.evidence;
  if (
    !videoUrlEvidence
    || videoUrlEvidence.schema !== 'v3.platform_api_video_url_resolution_rollup_evidence.v1'
    || videoUrlEvidence.test_count < 5
    || videoUrlEvidence.pass_count !== videoUrlEvidence.test_count
    || videoUrlEvidence.fail_count !== 0
    || videoUrlEvidence.requires_direct_source_covered !== true
    || videoUrlEvidence.accepts_direct_video_url_covered !== true
    || videoUrlEvidence.chinese_punctuation_direct_url_covered !== true
    || videoUrlEvidence.login_gated_source_blocked_covered !== true
    || videoUrlEvidence.dataset_title_helpers_covered !== true
  ) {
    throw new Error('no-live rollup platform-api video URL resolution evidence is incomplete');
  }

  const publicPageCommand = report.commands.find((result) => (
    result.id === 'platform_api_public_video_page_tests'
  ));
  if (!publicPageCommand || publicPageCommand.status !== 'passed') {
    throw new Error('no-live rollup missing platform-api public video page evidence');
  }
  const publicPageEvidence = publicPageCommand.evidence;
  if (
    !publicPageEvidence
    || publicPageEvidence.schema !== 'v3.platform_api_public_video_page_rollup_evidence.v1'
    || publicPageEvidence.test_count < 6
    || publicPageEvidence.pass_count !== publicPageEvidence.test_count
    || publicPageEvidence.fail_count !== 0
    || publicPageEvidence.public_page_extracts_video_sources_covered !== true
    || publicPageEvidence.public_page_blocks_private_or_login_gated_sources_covered !== true
    || publicPageEvidence.public_page_no_video_asset_covered !== true
    || publicPageEvidence.public_page_failure_result_structured_covered !== true
    || publicPageEvidence.public_page_failure_result_classification_covered !== true
    || publicPageEvidence.public_page_unregistered_result_keeps_resolved_video_covered !== true
  ) {
    throw new Error('no-live rollup platform-api public video page evidence is incomplete');
  }
}

function validateUploadMainPreflightEvidence(report) {
  const command = report.commands.find((result) => result.id === 'upload_main_preflight');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.video_ppt_upload_main_preflight_rollup_evidence.v1'
    || evidence.report_schema !== 'v3.video_ppt_upload_main_smoke_preflight.v1'
    || evidence.ok !== true
    || evidence.preflight !== true
    || evidence.network_calls_run !== false
    || evidence.production_write_allowed !== false
    || evidence.live_write_approval_required !== true
    || evidence.live_write_approval_gate_enforced !== true
    || evidence.live_write_approval_satisfied !== false
    || evidence.fixture_downloaded !== false
    || evidence.upload_attempted !== false
    || evidence.dataset_created !== false
    || evidence.document_registered !== false
    || evidence.assistant_run_created !== false
    || evidence.failure_count !== 0
    || evidence.fixture_source_kind !== 'url'
    || evidence.fixture_media_kind !== 'video'
    || evidence.fixture_supported_extension !== true
    || evidence.fixture_prompt_requests_video_ppt !== true
    || evidence.trigger_prompt_requests_video_ppt !== true
    || evidence.planned_step_count < 7
    || evidence.writes_smoke_records !== true
    || evidence.deploys_services !== false
    || evidence.command_template_present !== true
    || evidence.command_template_has_ack_live_write !== true
    || evidence.command_template_has_redacted_approval_id !== true
    || evidence.command_template_approval_values_included !== false
    || evidence.command_template_raw_url_included !== false
    || evidence.command_template_local_path_included !== false
    || evidence.raw_fixture_url_included !== false
    || evidence.local_fixture_path_included !== false
    || evidence.approval_id_included !== false
    || evidence.cookies_included !== false
    || evidence.bearer_included !== false
    || evidence.object_keys_included !== false
    || evidence.provider_payloads_included !== false
  ) {
    throw new Error('no-live rollup upload-main preflight evidence is incomplete');
  }
}

function validateExternalVideoPptPreflightEvidence(report) {
  const command = report.commands.find((result) => result.id === 'external_video_ppt_preflight');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.external_video_ppt_preflight_rollup_evidence.v1'
    || evidence.report_schema !== 'v3.external_video_ppt_smoke_preflight.v1'
    || evidence.ok !== true
    || evidence.preflight !== true
    || evidence.network_calls_run !== false
    || evidence.production_write_allowed !== false
    || evidence.live_write_approval_required !== true
    || evidence.live_write_approval_gate_enforced !== true
    || evidence.live_write_approval_satisfied !== false
    || evidence.credential_gate_satisfied !== true
    || evidence.live_credential_ready !== false
    || evidence.allow_missing_bearer !== true
    || evidence.fixture_downloaded !== false
    || evidence.fixture_registered !== false
    || evidence.event_sent !== false
    || evidence.reply_polled !== false
    || evidence.deliverables_downloaded_from_network !== false
    || evidence.failure_count !== 0
    || evidence.connection_id_present !== true
    || evidence.source_id_present !== true
    || evidence.tenant_external_id_present !== true
    || evidence.bot_external_id_present !== true
    || evidence.sender_external_id_present !== true
    || evidence.fixture_source_kind !== 'url'
    || evidence.fixture_media_kind !== 'video'
    || evidence.fixture_supported_extension !== true
    || evidence.trigger_text_requests_video_ppt !== true
    || evidence.default_prompt_guards_ordinary_video_to_ppt !== true
    || evidence.requested_video_ppt_skill !== true
    || evidence.expected_action !== 'extract_video_ppt_transcript'
    || evidence.available_document_source_present !== true
    || evidence.available_document_external_ids_count < 1
    || evidence.dataset_external_ids_count < 1
    || evidence.planned_step_count < 5
    || evidence.writes_smoke_records !== true
    || evidence.deploys_services !== false
    || evidence.command_template_present !== true
    || evidence.command_template_has_ack_live_write !== true
    || evidence.command_template_has_redacted_approval_id !== true
    || evidence.command_template_has_redacted_bearer !== true
    || evidence.command_template_approval_values_included !== false
    || evidence.command_template_raw_url_included !== false
    || evidence.command_template_local_path_included !== false
    || evidence.raw_fixture_url_included !== false
    || evidence.local_fixture_path_included !== false
    || evidence.approval_id_included !== false
    || evidence.bearer_included !== false
    || evidence.object_keys_included !== false
    || evidence.provider_payloads_included !== false
  ) {
    throw new Error('no-live rollup external preflight evidence is incomplete');
  }
}

function validateLiveApprovalNegativeEvidence(report, commandId) {
  const command = report.commands.find((result) => result.id === commandId);
  if (!command || command.status !== 'passed') {
    throw new Error(`no-live rollup missing live approval negative evidence: ${commandId}`);
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.video_ppt_live_write_approval_negative_evidence.v1'
    || evidence.command_id !== commandId
    || evidence.expected_failure !== true
    || evidence.exit_code === 0
    || evidence.approval_gate_enforced !== true
    || evidence.network_calls_run !== false
    || evidence.production_write_allowed !== false
    || evidence.upload_attempted !== false
    || evidence.event_sent !== false
    || evidence.browser_capture_allowed !== false
    || evidence.service_deployment_allowed !== false
  ) {
    throw new Error(`no-live rollup live approval negative evidence is incomplete: ${commandId}`);
  }
}

function hasCompleteLiveApprovalSelfTestGate(evidence) {
  return evidence.live_write_approval_gate_schema
    === 'v3.video_ppt_live_write_approval_gate_contract.v1'
    && evidence.live_write_approval_gate_case_count >= 6
    && evidence.live_write_approval_gate_negative_case_count >= 3
    && evidence.live_write_approval_gate_positive_case_count >= 3
    && evidence.live_write_approval_rejects_missing_ack_and_approval === true
    && evidence.live_write_approval_rejects_missing_approval === true
    && evidence.live_write_approval_rejects_missing_ack === true
    && evidence.live_write_approval_allows_preflight_without_approval === true
    && evidence.live_write_approval_allows_self_test_without_approval === true
    && evidence.live_write_approval_accepts_complete_live_approval_shape === true
    && evidence.live_write_approval_values_included === false;
}

function hasCompleteLiveCommandTemplateSelfTestContract(evidence, requiresBearer = false) {
  return evidence.live_command_template_contract_schema
    === 'v3.video_ppt_live_command_template_redaction_contract.v1'
    && evidence.live_command_template_contract_case_count >= 2
    && evidence.live_command_template_url_fixture_ready === true
    && evidence.live_command_template_local_file_ready === true
    && evidence.live_command_template_all_include_ack_live_write === true
    && evidence.live_command_template_all_include_redacted_approval_id === true
    && (requiresBearer
      ? evidence.live_command_template_all_include_redacted_bearer === true
      : evidence.live_command_template_all_include_redacted_bearer !== true)
    && evidence.live_command_template_raw_values_included === false;
}

function validateVideoPptHandoffPreflightEvidence(report) {
  const command = report.commands.find((result) => result.id === 'video_ppt_handoff_preflight');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.video_ppt_handoff_preflight_rollup_evidence.v1'
    || evidence.report_schema !== 'v3.video_ppt_handoff_smoke_preflight.v1'
    || evidence.ok !== true
    || evidence.preflight !== true
    || evidence.mode !== 'both'
    || evidence.target_mode_count !== 2
    || evidence.network_calls_run !== false
    || evidence.source_page_fetched !== false
    || evidence.video_downloaded !== false
    || evidence.frames_extracted !== false
    || evidence.ocr_run !== false
    || evidence.ppt_generated !== false
    || evidence.provider_called !== false
    || evidence.lightweight_smoke_writes_planned !== true
    || evidence.deployment_approval_required !== true
    || evidence.live_credential_ready !== false
    || evidence.credential_gate_satisfied !== true
    || evidence.allow_missing_bearer !== true
    || evidence.failure_count !== 0
    || evidence.main_planned_step_count < 2
    || evidence.external_planned_step_count < 2
    || evidence.external_connection_id_present !== true
    || evidence.external_source_id_present !== true
    || evidence.failure_reason !== 'login_gated_video_source_not_supported'
    || evidence.supported_next_step_count !== 3
    || evidence.success_signal_rejected_count < 4
    || evidence.prompt_mentions_wechat_video !== true
    || evidence.prompt_wants_slide_output !== true
    || evidence.prompt_mentions_login_gate !== false
    || evidence.raw_source_url_included !== false
    || evidence.cookie_included !== false
    || evidence.bearer_included !== false
    || evidence.provider_payloads_included !== false
    || evidence.local_paths_included !== false
  ) {
    throw new Error('no-live rollup handoff preflight evidence is incomplete');
  }
}

function validateVideoPptHandoffEvidence(report) {
  const command = report.commands.find((result) => result.id === 'video_ppt_handoff_self_test');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.video_ppt_handoff_rollup_evidence.v1'
    || evidence.prompt_mentions_wechat_video !== true
    || evidence.prompt_wants_slide_output !== true
    || evidence.network_calls_run !== false
    || evidence.provider_called !== false
    || evidence.react_toolchain_called !== false
    || evidence.video_fetch_attempted !== false
    || evidence.video_downloaded !== false
    || evidence.frames_extracted !== false
    || evidence.ocr_run !== false
    || evidence.ppt_generated !== false
    || evidence.final_pptx_ready_exposed !== false
    || evidence.artifact_links_exposed !== false
    || evidence.download_exports_exposed !== false
    || evidence.negative_fixture_count < 5
    || evidence.negative_fixtures_rejected !== evidence.negative_fixture_count
    || evidence.main_ok !== true
    || evidence.main_failure_reason !== 'login_gated_video_source_not_supported'
    || evidence.main_next_step_count !== 3
    || evidence.main_has_required_reason !== true
    || evidence.main_has_handoff_type !== true
    || evidence.main_has_actionable_next_steps !== true
    || evidence.main_unsafe_success_signal !== false
    || evidence.main_unsafe_artifact_link_signal !== false
    || evidence.main_unsafe_credential_request !== false
    || evidence.main_raw_source_leaked !== false
    || evidence.external_ok !== true
    || evidence.external_failure_reason !== 'login_gated_video_source_not_supported'
    || evidence.external_next_step_count !== 3
    || evidence.external_has_required_reason !== true
    || evidence.external_has_handoff_type !== true
    || evidence.external_has_actionable_next_steps !== true
    || evidence.external_unsafe_success_signal !== false
    || evidence.external_unsafe_artifact_link_signal !== false
    || evidence.external_unsafe_credential_request !== false
    || evidence.external_raw_source_leaked !== false
  ) {
    throw new Error('no-live rollup handoff evidence is incomplete');
  }
}

function validateAuthorizedCaptureEvidence(report) {
  const command = report.commands.find((result) => result.id === 'authorized_capture_self_test');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.authorized_capture_rollup_evidence.v1'
    || evidence.mode !== 'dry-run'
    || evidence.self_test !== true
    || evidence.approval_reference_present !== true
    || evidence.approval_id_redacted !== true
    || evidence.approved_by_reference_present !== true
    || evidence.approved_by_redacted !== true
    || evidence.purpose_present !== true
    || evidence.duration_seconds < 5
    || evidence.retention_days < 0
    || evidence.capture_audio_allowed !== false
    || evidence.capture_mode_present !== true
    || !['none', 'upload-main', 'external-video'].includes(evidence.handoff_mode)
    || evidence.authorization_gate_negative_case_count < 6
    || evidence.authorization_gate_negative_cases_rejected !== evidence.authorization_gate_negative_case_count
    || evidence.capture_attempted !== false
    || evidence.dry_run !== true
    || evidence.browser_would_run !== true
    || evidence.ffmpeg_would_run !== true
    || evidence.ack_authorized !== true
    || evidence.isolated_browser_profile !== true
    || evidence.persistent_profile_disabled !== true
    || evidence.raw_url_stored !== false
    || evidence.cookies_stored !== false
    || evidence.har_stored !== false
    || evidence.qr_screenshot_stored !== false
    || evidence.local_path_redacted !== true
    || evidence.profile_path_redacted !== true
    || evidence.approval_values_included !== false
    || evidence.raw_source_url_included !== false
    || evidence.local_paths_included !== false
    || evidence.credentials_included !== false
    || evidence.provider_payloads_included !== false
  ) {
    throw new Error('no-live rollup authorized-capture evidence is incomplete');
  }
}

function validateAuthorizedCaptureDryRunEvidence(report) {
  const command = report.commands.find((result) => result.id === 'authorized_capture_dry_run');
  if (!command || command.status !== 'passed') {
    return;
  }
  const evidence = command.evidence;
  if (
    !evidence
    || evidence.schema !== 'v3.authorized_capture_dry_run_rollup_evidence.v1'
    || evidence.ok !== true
    || evidence.mode !== 'dry-run'
    || evidence.source_scheme !== 'https'
    || evidence.source_host_present !== true
    || evidence.duration_seconds !== 30
    || evidence.retention_days !== 7
    || evidence.capture_audio_allowed !== false
    || evidence.handoff !== 'upload-main'
    || evidence.approval_reference_present !== true
    || evidence.approval_id_redacted !== true
    || evidence.approved_by_reference_present !== true
    || evidence.approved_by_redacted !== true
    || evidence.purpose_present !== true
    || evidence.shared_receipt_source_scheme !== 'https'
    || evidence.shared_receipt_source_host_present !== true
    || evidence.output_file_name !== 'authorized-capture.mp4'
    || evidence.local_path_redacted !== true
    || evidence.profile_path_redacted !== true
    || evidence.handoff_mode !== 'upload-main'
    || evidence.capture_mode_present !== true
    || evidence.capture_attempted !== false
    || evidence.dry_run !== true
    || evidence.browser_would_run !== true
    || evidence.ffmpeg_would_run !== true
    || evidence.ack_authorized !== true
    || evidence.isolated_browser_profile !== true
    || evidence.persistent_profile_disabled !== true
    || evidence.raw_url_stored !== false
    || evidence.cookies_stored !== false
    || evidence.har_stored !== false
    || evidence.qr_screenshot_stored !== false
    || evidence.approval_values_included !== false
    || evidence.raw_source_url_included !== false
    || evidence.local_paths_included !== false
    || evidence.credentials_included !== false
    || evidence.provider_payloads_included !== false
  ) {
    throw new Error('no-live rollup authorized-capture dry-run evidence is incomplete');
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
    || !hasCompleteLiveApprovalSelfTestGate(evidence)
    || !hasCompleteLiveCommandTemplateSelfTestContract(evidence)
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
    || !hasCompleteLiveApprovalSelfTestGate(evidence)
    || !hasCompleteLiveCommandTemplateSelfTestContract(evidence, true)
    || evidence.surface_ok !== true
    || evidence.surface_export_count < 6
    || evidence.surface_export_kind_count < 6
    || evidence.surface_missing_export_kind_count !== 0
    || evidence.surface_artifact_link_count < 1
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
    || evidence.deliverables_mode_failure_class_gate_defaults_supported !== true
    || evidence.customer_authorization_argument_gate_supported !== true
    || evidence.customer_retention_policy_argument_gate_supported !== true
    || evidence.review_failure_class_summary_supported !== true
    || evidence.review_failure_class_summary_needs_manual_review_count !== 8
    || !hasExpectedReviewFailureClassCounts(evidence.review_failure_class_summary_counts)
    || evidence.not_deliverable_failure_class_count !== 5
    || !Array.isArray(evidence.not_deliverable_failure_classes)
    || !evidence.not_deliverable_failure_classes.includes('source_access')
    || !evidence.not_deliverable_failure_classes.includes('video_has_no_ppt')
    || !evidence.not_deliverable_failure_classes.includes('frame_extraction')
    || !evidence.not_deliverable_failure_classes.includes('artifact_visibility')
    || !evidence.not_deliverable_failure_classes.includes('selection_quality')
    || evidence.failure_class_summary_supported !== true
    || !isCountMap(evidence.failure_class_counts)
    || !isCountMap(evidence.not_deliverable_failure_class_counts)
    || !isCountMap(evidence.needs_manual_review_failure_class_counts)
    || evidence.failure_class_summary_regression_not_deliverable_count !== 5
    || evidence.failure_class_summary_regression_class_count !== 5
    || !hasRequiredFailureClassCounts(evidence.failure_class_summary_regression_counts)
    || !hasRequiredFailureClassCounts(evidence.failure_class_summary_regression_not_deliverable_counts)
    || evidence.live_smoke_run !== false
    || evidence.production_write_allowed !== false
    || evidence.generated_artifacts_committable !== false
  ) {
    throw new Error('no-live rollup quality matrix evidence is incomplete');
  }
}

function isCountMap(value) {
  return Boolean(value)
    && typeof value === 'object'
    && !Array.isArray(value)
    && Object.values(value).every((count) => Number.isInteger(count) && count >= 0);
}

function hasRequiredFailureClassCounts(counts) {
  return isCountMap(counts)
    && REQUIRED_NOT_DELIVERABLE_FAILURE_CLASSES.every((failureClass) => counts[failureClass] === 1);
}

function hasExpectedReviewFailureClassCounts(counts) {
  return isCountMap(counts)
    && Object.entries(EXPECTED_REVIEW_FAILURE_CLASS_COUNTS).every(
      ([failureClass, expectedCount]) => counts[failureClass] === expectedCount,
    );
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
