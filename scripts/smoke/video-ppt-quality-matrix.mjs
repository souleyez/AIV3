#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { validateVideoDeliverables } from '../../tools/validate-video-deliverables.mjs';

const DEFAULT_OUTPUT_DIR = 'target/video-ppt-quality-matrix-smoke';
const REQUIRED_CATEGORIES = [
  'synthetic_ppt_playback',
  'public_course_video',
  'customer_authorized_video',
];

function parseArgs(argv) {
  const args = {
    selfTest: false,
    syntheticDeliverables: process.env.VIDEO_PPT_QUALITY_MATRIX_SYNTHETIC_DELIVERABLES || '',
    publicCourseDeliverables: process.env.VIDEO_PPT_QUALITY_MATRIX_PUBLIC_COURSE_DELIVERABLES || '',
    customerDeliverables: process.env.VIDEO_PPT_QUALITY_MATRIX_CUSTOMER_DELIVERABLES || '',
    customerApprovalId: process.env.VIDEO_PPT_QUALITY_MATRIX_CUSTOMER_APPROVAL_ID || '',
    outputDir: process.env.VIDEO_PPT_QUALITY_MATRIX_OUTPUT_DIR || DEFAULT_OUTPUT_DIR,
    pretty: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--self-test') {
      args.selfTest = true;
    } else if (arg === '--synthetic-deliverables') {
      args.syntheticDeliverables = requiredValue(argv, index += 1, arg);
    } else if (arg === '--public-course-deliverables') {
      args.publicCourseDeliverables = requiredValue(argv, index += 1, arg);
    } else if (arg === '--customer-deliverables') {
      args.customerDeliverables = requiredValue(argv, index += 1, arg);
    } else if (arg === '--customer-approval-id') {
      args.customerApprovalId = requiredValue(argv, index += 1, arg);
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
  npm run smoke:video-ppt-quality-matrix -- --self-test [--pretty] [--output-dir target/video-ppt-quality-matrix-smoke]
  npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables target/<video-extraction>/generated_artifacts [--pretty]
  npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables target/<video-extraction>/generated_artifacts [--pretty]
  npm run smoke:video-ppt-quality-matrix -- --customer-deliverables target/<video-extraction>/generated_artifacts --customer-approval-id APPROVAL-... [--pretty]
  npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables target/<synthetic>/generated_artifacts --public-course-deliverables target/<public>/generated_artifacts --customer-deliverables target/<customer>/generated_artifacts --customer-approval-id APPROVAL-... [--pretty]

Checks:
  - deterministic P2-2E quality matrix shape for video/PPT extraction
  - synthetic PPT-playback sample can be marked deliverable from local evidence
  - local synthetic deliverables can be validated and classified through the same matrix
  - local public-course deliverables can be validated and classified through the same matrix
  - local customer-authorized deliverables can be validated only after explicit approval/input exists
  - combined deliverables inputs can produce a complete three-category matrix when all required categories are provided
  - customer-authorized samples stay pending unless --customer-deliverables and --customer-approval-id are provided
  - report never claims live/customer/video-channel extraction from self-test evidence

Safety:
  - --self-test does not call DataMax, download video, upload files, record browser sessions, or deploy services
  - generated reports stay under target/ and must not include source URLs, object paths, cookies, tokens, or provider payloads`;
}

function buildSelfTestCases() {
  return [
    buildSelfTestSyntheticCase(),
    buildPendingPublicCourseCase(),
    buildPendingCustomerAuthorizedCase(),
  ];
}

function buildSelfTestSyntheticCase() {
  return {
    case_id: 'synthetic-ppt-playback-local',
    category: 'synthetic_ppt_playback',
    input_type: 'deterministic_local_fixture',
    source_access_status: 'local_fixture',
    approval_status: 'not_required',
    trigger: 'extract_ppt_slides_courseware_already_shown_in_video',
    deliverable_status: {
      state: 'final_pptx_ready',
      frame_count: 180,
      selected_count: 12,
      pptx_slide_count: 12,
      markdown_slide_count: 12,
      has_quality_report: true,
    },
    quality_report: {
      quality_score: 88,
      risk_flags: ['screenshot_based_pptx'],
      summary: {
        full_frame_fallback_count: 0,
        detector_crop_count: 12,
        subtitle_missing_count: 0,
        ocr_missing_count: 0,
        sharpness_high_count: 0,
        sharpness_unknown_count: 0,
        readability_high_count: 0,
        readability_unknown_count: 0,
      },
    },
    expected_verdict: 'deliverable',
  };
}

function buildPendingPublicCourseCase() {
  return {
    case_id: 'public-course-video-pending',
    category: 'public_course_video',
    input_type: 'public_course_video_sample',
    source_access_status: 'sample_required',
    approval_status: 'not_required',
    trigger: 'extract_ppt_slides_courseware_already_shown_in_video',
    deliverable_status: null,
    quality_report: null,
    expected_verdict: 'pending_accessible_sample',
  };
}

function buildPendingCustomerAuthorizedCase() {
  return {
    case_id: 'customer-authorized-video-pending',
    category: 'customer_authorized_video',
    input_type: 'customer_uploaded_or_authorized_capture',
    source_access_status: 'approval_required',
    approval_status: 'missing',
    trigger: 'extract_ppt_slides_courseware_already_shown_in_video',
    deliverable_status: null,
    quality_report: null,
    expected_verdict: 'pending_authorization',
  };
}

function buildCasesFromDeliverableArgs(args) {
  return [
    args.syntheticDeliverables
      ? buildCaseFromDeliverables(args.syntheticDeliverables, {
        caseId: 'synthetic-ppt-playback-deliverables',
        category: 'synthetic_ppt_playback',
        inputType: 'local_video_ppt_deliverables',
        sourceAccessStatus: 'local_fixture',
        approvalStatus: 'not_required',
      })
      : buildSelfTestSyntheticCase(),
    args.publicCourseDeliverables
      ? buildCaseFromDeliverables(args.publicCourseDeliverables, {
        caseId: 'public-course-video-deliverables',
        category: 'public_course_video',
        inputType: 'public_course_video_deliverables',
        sourceAccessStatus: 'anonymous_public_video_fixture',
        approvalStatus: 'not_required',
      })
      : buildPendingPublicCourseCase(),
    args.customerDeliverables
      ? buildCaseFromDeliverables(args.customerDeliverables, {
        caseId: 'customer-authorized-video-deliverables',
        category: 'customer_authorized_video',
        inputType: 'customer_uploaded_or_authorized_capture_deliverables',
        sourceAccessStatus: 'customer_authorized_input',
        approvalStatus: 'operator_authorized',
        approvalReferencePresent: true,
      })
      : buildPendingCustomerAuthorizedCase(),
  ];
}

function buildCaseFromDeliverables(inputPath, {
  caseId,
  category,
  inputType,
  sourceAccessStatus,
  approvalStatus,
  approvalReferencePresent = false,
}) {
  const validation = validateVideoDeliverables(inputPath);
  const artifactsDir = validation.artifactsDir;
  const finalManifest = readJsonIfPresent(path.join(artifactsDir, 'final_deliverables_manifest.json'));
  const qualityReport = readJsonIfPresent(path.join(artifactsDir, 'slide_quality_report.json'));
  const validationSummary = validation.summary || {};
  const qualitySlideCount = Number.isInteger(qualityReport?.slide_count) ? qualityReport.slide_count : 0;
  const selectedCount = numberOrNull(validationSummary.selected_count) ?? qualitySlideCount;
  const pptxSlideCount = numberOrNull(validationSummary.pptx_slide_count) ?? selectedCount;
  const markdownSlideCount = numberOrNull(validationSummary.markdown_slide_count) ?? selectedCount;
  const deliverableState = validation.ok
    ? finalManifest?.deliverable_status?.state || finalManifest?.status || 'final_pptx_ready'
    : 'deliverable_contract_invalid';
  const qualitySummary = qualityReport?.summary || {};
  const riskFlags = normalizeRiskFlags(qualityReport?.risk_flags);
  return {
    case_id: caseId,
    category,
    input_type: inputType,
    source_access_status: sourceAccessStatus,
    approval_status: approvalStatus,
    approval_reference_present: approvalReferencePresent,
    approval_reference_redacted: approvalReferencePresent,
    trigger: 'extract_ppt_slides_courseware_already_shown_in_video',
    deliverable_status: {
      state: deliverableState,
      validator_ok: validation.ok,
      validator_error_codes: validation.errors.map((error) => error.code),
      validator_warning_codes: validation.warnings.map((warning) => warning.code),
      checked_file_kinds: validation.files.filter((file) => file.exists).map((file) => file.kind),
      frame_count: numberOrNull(validationSummary.frame_count) ?? numberOrNull(finalManifest?.frame_extraction?.frame_count),
      selected_count: selectedCount,
      pptx_slide_count: pptxSlideCount,
      markdown_slide_count: markdownSlideCount,
      has_quality_report: Boolean(qualityReport),
    },
    quality_report: qualityReport ? {
      quality_score: qualityReport.quality_score,
      risk_flags: riskFlags,
      summary: {
        full_frame_fallback_count: qualitySummary.full_frame_fallback_count || 0,
        detector_crop_count: qualitySummary.detector_crop_count || 0,
        subtitle_missing_count: qualitySummary.subtitle_missing_count || 0,
        ocr_missing_count: qualitySummary.ocr_missing_count || 0,
        sharpness_high_count: qualitySummary.sharpness_high_count || 0,
        sharpness_unknown_count: qualitySummary.sharpness_unknown_count || 0,
        readability_high_count: qualitySummary.readability_high_count || 0,
        readability_unknown_count: qualitySummary.readability_unknown_count || 0,
        single_slide_output: qualitySummary.single_slide_output === true,
      },
    } : null,
    deliverables_input_redacted: true,
  };
}

function normalizeRiskFlags(riskFlags) {
  if (!Array.isArray(riskFlags)) {
    return [];
  }
  return riskFlags
    .map((flag) => {
      if (typeof flag === 'string') {
        return flag;
      }
      if (flag && typeof flag.code === 'string') {
        return flag.code;
      }
      return '';
    })
    .filter(Boolean);
}

function numberOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function readJsonIfPresent(filePath) {
  try {
    if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
      return null;
    }
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch {
    return null;
  }
}

function evaluateCase(testCase) {
  if (testCase.approval_status === 'missing' || testCase.source_access_status === 'approval_required') {
    return {
      verdict: 'pending_authorization',
      reason: 'operator approval and customer-safe source details are required',
      review_conclusion: 'not_run',
    };
  }
  if (testCase.source_access_status === 'sample_required') {
    return {
      verdict: 'pending_accessible_sample',
      reason: 'an approved public course video sample is required',
      review_conclusion: 'not_run',
    };
  }
  const deliverable = testCase.deliverable_status;
  if (!deliverable || deliverable.state !== 'final_pptx_ready') {
    return {
      verdict: 'extraction_failed',
      reason: 'final_pptx_ready was not proven',
      review_conclusion: 'not_deliverable',
    };
  }
  if (
    deliverable.selected_count !== deliverable.pptx_slide_count
    || deliverable.selected_count !== deliverable.markdown_slide_count
  ) {
    return {
      verdict: 'artifact_count_mismatch',
      reason: 'selected slide count, PPTX slide count, and Markdown slide count must match',
      review_conclusion: 'not_deliverable',
    };
  }
  if (!deliverable.has_quality_report || !testCase.quality_report) {
    return {
      verdict: 'quality_report_missing',
      reason: 'quality report is required for P2-2E review decisions',
      review_conclusion: 'needs_manual_review',
    };
  }
  const quality = testCase.quality_report;
  const summary = quality.summary || {};
  const riskFlags = new Set(Array.isArray(quality.risk_flags) ? quality.risk_flags : []);
  const hasHighRiskFrames = (summary.full_frame_fallback_count || 0) > 0
    || (summary.sharpness_high_count || 0) > 0
    || (summary.sharpness_unknown_count || 0) > 0
    || (summary.readability_high_count || 0) > 0
    || (summary.readability_unknown_count || 0) > 0
    || summary.single_slide_output === true
    || riskFlags.has('manual_review_required')
    || riskFlags.has('missing_transcript_alignment')
    || riskFlags.has('frame_sharpness_review_required')
    || riskFlags.has('slide_readability_review_required');
  if (quality.quality_score < 70 || hasHighRiskFrames) {
    return {
      verdict: 'needs_manual_review',
      reason: 'quality report contains high-risk or low-score pages',
      review_conclusion: 'needs_manual_review',
    };
  }
  return {
    verdict: 'deliverable',
    reason: 'slide counts align and quality report has no high-risk local signals',
    review_conclusion: 'deliverable',
  };
}

function buildSelfTestReport() {
  const report = buildQualityMatrixReport({
    status: 'partial_local_self_test_ready',
    selfTest: true,
    inputMode: 'self_test',
    cases: buildSelfTestCases(),
    gates: {
      public_course_sample_required: true,
      customer_authorization_required: true,
      local_deliverables_input_reviewed: false,
    },
    nextActions: [
      'run synthetic PPT playback extraction and attach real target/ report when available',
      'run public course video review only with an anonymous direct video URL or upload fixture',
      'run customer sample review only after explicit customer/operator authorization',
    ],
  });
  validateSelfTestReport(report);
  return report;
}

function buildDeliverablesReportFromArgs(args) {
  const inputKinds = deliverablesInputKinds(args);
  const allDeliverablesInputsReviewed = inputKinds.length === REQUIRED_CATEGORIES.length;
  const report = buildQualityMatrixReport({
    status: allDeliverablesInputsReviewed
      ? 'complete_deliverables_matrix_reviewed'
      : statusForPartialDeliverablesInput(inputKinds),
    selfTest: false,
    inputMode: inputKinds.length > 1 ? 'combined_deliverables' : inputModeForSingleDeliverablesInput(inputKinds[0]),
    matrixComplete: allDeliverablesInputsReviewed,
    cases: buildCasesFromDeliverableArgs(args),
    gates: {
      public_course_sample_required: !args.publicCourseDeliverables,
      customer_authorization_required: !args.customerDeliverables,
      customer_approval_reference_present: Boolean(args.customerDeliverables && args.customerApprovalId),
      synthetic_deliverables_reviewed: Boolean(args.syntheticDeliverables),
      public_course_deliverables_reviewed: Boolean(args.publicCourseDeliverables),
      customer_authorized_deliverables_reviewed: Boolean(args.customerDeliverables),
      all_deliverables_inputs_reviewed: allDeliverablesInputsReviewed,
      local_deliverables_input_reviewed: true,
    },
    redaction: {
      synthetic_deliverables_input_redacted: Boolean(args.syntheticDeliverables),
      public_course_deliverables_input_redacted: Boolean(args.publicCourseDeliverables),
      customer_authorized_deliverables_input_redacted: Boolean(args.customerDeliverables),
      customer_approval_reference_redacted: Boolean(args.customerDeliverables && args.customerApprovalId),
    },
    nextActions: nextActionsForDeliverablesInputs(args),
  });
  validateQualityMatrixReport(report);
  return report;
}

function deliverablesInputKinds(args) {
  const kinds = [];
  if (args.syntheticDeliverables) {
    kinds.push('synthetic');
  }
  if (args.publicCourseDeliverables) {
    kinds.push('public_course');
  }
  if (args.customerDeliverables) {
    kinds.push('customer');
  }
  return kinds;
}

function inputModeForSingleDeliverablesInput(kind) {
  if (kind === 'synthetic') {
    return 'synthetic_deliverables';
  }
  if (kind === 'public_course') {
    return 'public_course_deliverables';
  }
  if (kind === 'customer') {
    return 'customer_deliverables';
  }
  return 'combined_deliverables';
}

function statusForPartialDeliverablesInput(inputKinds) {
  if (inputKinds.length > 1) {
    return 'partial_combined_deliverables_reviewed';
  }
  if (inputKinds[0] === 'synthetic') {
    return 'partial_local_deliverables_reviewed';
  }
  if (inputKinds[0] === 'public_course') {
    return 'partial_public_course_deliverables_reviewed';
  }
  if (inputKinds[0] === 'customer') {
    return 'partial_customer_authorized_deliverables_reviewed';
  }
  return 'partial_deliverables_reviewed';
}

function nextActionsForDeliverablesInputs(args) {
  const actions = [];
  if (!args.syntheticDeliverables) {
    actions.push('run synthetic PPT playback extraction and attach generated_artifacts when available');
  }
  if (!args.publicCourseDeliverables) {
    actions.push('run public course video review only with an anonymous direct video URL or upload fixture');
  }
  if (!args.customerDeliverables) {
    actions.push('run customer sample review only after explicit customer/operator authorization');
  }
  if (actions.length === 0) {
    actions.push('review all three quality conclusions and record deliverable, needs_manual_review, or not_deliverable for each category');
  }
  return actions;
}

function buildQualityMatrixReport({
  status,
  selfTest,
  inputMode,
  matrixComplete = false,
  cases: inputCases,
  gates = {},
  redaction = {},
  nextActions = [],
}) {
  const cases = inputCases.map((testCase) => {
    const evaluation = evaluateCase(testCase);
    return {
      ...testCase,
      evaluation,
      expectation_matched: testCase.expected_verdict
        ? evaluation.verdict === testCase.expected_verdict
        : null,
    };
  });
  const summary = summarizeCases(cases);
  const allRequiredCategoriesPresent = REQUIRED_CATEGORIES.every((category) =>
    cases.some((testCase) => testCase.category === category),
  );
  const report = {
    schema: 'v3.video_ppt_quality_matrix_smoke.v1',
    status,
    self_test: selfTest,
    input_mode: inputMode,
    matrix_complete: matrixComplete,
    required_categories: REQUIRED_CATEGORIES,
    all_required_categories_present: allRequiredCategoriesPresent,
    summary,
    cases,
    gates: {
      live_smoke_run: false,
      production_write_allowed: false,
      generated_artifacts_committable: false,
      ...gates,
    },
    redaction: {
      status: 'applied',
      source_urls_included: false,
      object_paths_included: false,
      credentials_included: false,
      provider_payloads_included: false,
      ...redaction,
    },
    next_actions: nextActions,
  };
  validateQualityMatrixReport(report);
  return report;
}

function summarizeCases(cases) {
  const summary = {
    case_count: cases.length,
    deliverable_count: 0,
    needs_manual_review_count: 0,
    not_deliverable_count: 0,
    pending_accessible_sample_count: 0,
    pending_authorization_count: 0,
    expectation_mismatch_count: 0,
  };
  for (const testCase of cases) {
    const verdict = testCase.evaluation.verdict;
    if (verdict === 'deliverable') {
      summary.deliverable_count += 1;
    } else if (verdict === 'needs_manual_review' || verdict === 'quality_report_missing') {
      summary.needs_manual_review_count += 1;
    } else if (verdict === 'pending_accessible_sample') {
      summary.pending_accessible_sample_count += 1;
    } else if (verdict === 'pending_authorization') {
      summary.pending_authorization_count += 1;
    } else {
      summary.not_deliverable_count += 1;
    }
    if (testCase.expectation_matched === false) {
      summary.expectation_mismatch_count += 1;
    }
  }
  summary.pending_count = summary.pending_accessible_sample_count + summary.pending_authorization_count;
  return summary;
}

function validateSelfTestReport(report) {
  if (report.matrix_complete !== false || report.status !== 'partial_local_self_test_ready') {
    throw new Error('self-test must not claim the full P2-2E matrix is complete');
  }
  if (report.summary.case_count !== REQUIRED_CATEGORIES.length) {
    throw new Error('self-test report case count mismatch');
  }
  if (report.summary.deliverable_count !== 1 || report.summary.pending_count !== 2) {
    throw new Error('self-test report must keep only the synthetic case deliverable');
  }
  if (report.summary.expectation_mismatch_count !== 0) {
    throw new Error('self-test report has expectation mismatches');
  }
}

function validateQualityMatrixReport(report) {
  if (report.schema !== 'v3.video_ppt_quality_matrix_smoke.v1') {
    throw new Error('invalid quality matrix report schema');
  }
  if (typeof report.matrix_complete !== 'boolean') {
    throw new Error('quality matrix report matrix_complete must be boolean');
  }
  if (
    report.matrix_complete
    && (report.summary.pending_count !== 0 || report.gates.all_deliverables_inputs_reviewed !== true)
  ) {
    throw new Error('quality matrix report must only be complete after all deliverables inputs are reviewed');
  }
  if (!report.all_required_categories_present) {
    throw new Error('quality matrix report must cover all required P2-2E categories');
  }
  if (!Array.isArray(report.cases) || report.cases.length !== REQUIRED_CATEGORIES.length) {
    throw new Error('quality matrix report case count mismatch');
  }
  for (const category of REQUIRED_CATEGORIES) {
    if (!report.cases.some((testCase) => testCase.category === category)) {
      throw new Error(`quality matrix report missing category ${category}`);
    }
  }
  if (
    report.gates.live_smoke_run
    || report.gates.production_write_allowed
    || report.redaction.source_urls_included
    || report.redaction.object_paths_included
    || report.redaction.credentials_included
    || report.redaction.provider_payloads_included
  ) {
    throw new Error('quality matrix report safety gates are invalid');
  }
  if (JSON.stringify(report).match(/[A-Za-z]:[\\/]|[\\/]Users[\\/]|[\\/]home[\\/]|https?:\/\/|token=/i)) {
    throw new Error('quality matrix report contains an unredacted local path or token-like string');
  }
}

function makeRunId() {
  const now = new Date();
  const timestamp = now.toISOString().replace(/[-:]/g, '').replace(/\..+/, '');
  return `${timestamp}-${process.pid}`;
}

function writeReport(outputDir, report, pretty) {
  fs.mkdirSync(outputDir, { recursive: true });
  const suffix = report.input_mode.replaceAll('_', '-');
  const reportPath = path.join(outputDir, `${makeRunId()}-${suffix}.json`);
  fs.writeFileSync(reportPath, JSON.stringify(report, null, pretty ? 2 : 0));
  return reportPath;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(usage());
    return;
  }
  const deliverableInputs = [
    args.syntheticDeliverables,
    args.publicCourseDeliverables,
    args.customerDeliverables,
  ].filter(Boolean);
  if (args.selfTest && deliverableInputs.length > 0) {
    throw new Error('use either --self-test or deliverables input flags, not both');
  }
  if (args.customerDeliverables && !args.customerApprovalId) {
    throw new Error('--customer-approval-id is required with --customer-deliverables');
  }
  if (args.customerApprovalId && !args.customerDeliverables) {
    throw new Error('--customer-approval-id requires --customer-deliverables');
  }
  if (!args.selfTest && deliverableInputs.length === 0) {
    throw new Error('--self-test, --synthetic-deliverables, --public-course-deliverables, or --customer-deliverables is required');
  }
  let report;
  if (deliverableInputs.length > 0) {
    report = buildDeliverablesReportFromArgs(args);
  } else {
    report = buildSelfTestReport();
  }
  const reportPath = writeReport(args.outputDir, report, args.pretty);
  const label = report.input_mode === 'self_test'
    ? 'self-test'
    : `${report.input_mode.replaceAll('_', ' ')} review`;
  console.log([
    `OK video PPT quality matrix ${label}:`,
    `cases=${report.summary.case_count}`,
    `deliverable=${report.summary.deliverable_count}`,
    `needs_manual_review=${report.summary.needs_manual_review_count}`,
    `not_deliverable=${report.summary.not_deliverable_count}`,
    `pending=${report.summary.pending_count}`,
    `report=${reportPath}`,
  ].join(' '));
}

try {
  main();
} catch (error) {
  console.error(`video PPT quality matrix smoke failed: ${error.message}`);
  process.exitCode = 1;
}
