import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validateExternalHandoffManifest } from './validate-external-handoff.mjs';
import { validateAll } from './validate-external-handoff-all.mjs';
import { validateRelease } from './validate-external-handoff-release.mjs';

export const DEFAULT_READINESS_CHECKS = [
  {
    key: 'dispatch_request_received',
    label: 'Signed dispatch reached third-party gateway',
    pass: ({ requests }) => Array.isArray(requests) && requests.length === 1,
  },
  {
    key: 'dispatch_bearer_valid',
    label: 'Dispatch Bearer token accepted',
    pass: ({ request }) => request?.bearer_valid === true,
  },
  {
    key: 'dispatch_signature_valid',
    label: 'Dispatch HMAC signature accepted',
    pass: ({ request }) => request?.signature_valid === true,
  },
  {
    key: 'dispatch_body_hash_valid',
    label: 'Dispatch body hash accepted',
    pass: ({ request }) => request?.body_hash_valid === true,
  },
  {
    key: 'dispatch_requester_present',
    label: 'Requester summary is present',
    pass: ({ request }) => request?.requester_sender_present === true,
  },
  {
    key: 'dispatch_payload_redacted',
    label: 'Dispatch payload is redacted',
    pass: ({ request }) => request?.raw_arguments_included !== true && request?.contains_forbidden_text !== true,
  },
  {
    key: 'result_callback_received',
    label: 'Result callback reached V3',
    pass: ({ callbacks }) => Array.isArray(callbacks) && callbacks.length === 1,
  },
  {
    key: 'result_callback_accepted',
    label: 'Result callback was accepted',
    pass: ({ callback }) => callback?.callback_status === 200 && callback?.response_accepted === true,
  },
  {
    key: 'result_callback_redacted',
    label: 'Result callback response is redacted',
    pass: ({ callback }) => callback?.contains_forbidden_text !== true,
  },
];

export const THIRD_PARTY_HANDOFF_ITEMS = [
  'Public HTTPS dispatch endpoint for artifact/action calls',
  'Dispatch-specific Bearer token or HMAC signing secret',
  'Stable external user, conversation, message, document, and action identifiers',
  'V3 result callback allowlist or outbound network rule',
  'Document metadata/body/ACL test fixtures with known access boundaries',
  'Operational contact and retry/error escalation path',
];

export function buildReadinessReport({
  generatedAt = new Date().toISOString(),
  gatewayUrl = '',
  repository = '',
  head = '',
  requests = [],
  callbacks = [],
  handoffManifestPath = '',
  handoffManifestValidation = null,
  releasePackagePath = '',
  releaseValidation = null,
  handoffAllValidation = null,
} = {}) {
  const request = Array.isArray(requests) ? requests[0] : null;
  const callback = Array.isArray(callbacks) ? callbacks[0] : null;
  const context = { requests, callbacks, request, callback };
  const checks = DEFAULT_READINESS_CHECKS.map((check) => ({
    key: check.key,
    label: check.label,
    passed: Boolean(check.pass(context)),
  }));
  const handoffManifestSummary = summarizeHandoffManifestValidation(
    handoffManifestValidation,
    handoffManifestPath,
  );
  if (handoffManifestSummary) {
    checks.push({
      key: 'handoff_manifest_ready',
      label: 'Handoff manifest is ready for customer sandbox',
      passed: handoffManifestSummary.ready_for_customer_sandbox,
    });
  }
  const releaseSummary = summarizeReleaseValidation(releaseValidation, releasePackagePath);
  if (releaseSummary) {
    checks.push({
      key: 'handoff_release_ready',
      label: 'Handoff release package is ready for delivery',
      passed: releaseSummary.release_ready,
    });
  }
  const handoffAllSummary = summarizeHandoffAllValidation(handoffAllValidation, releasePackagePath);
  if (handoffAllSummary) {
    checks.push({
      key: 'handoff_all_ready',
      label: 'All handoff package validation gates passed',
      passed: handoffAllSummary.all_ready,
    });
  }
  return {
    report_type: 'external_third_party_readiness',
    generated_at: generatedAt,
    repository,
    head,
    gateway_url: gatewayUrl,
    ready_for_customer_sandbox: checks.every((check) => check.passed),
    checks,
    dispatch_summary: request
      ? {
          action_id: request.action_id || null,
          action_type: request.action_type || null,
          path: request.path || null,
          bearer_valid: request.bearer_valid === true,
          signature_valid: request.signature_valid === true,
          body_hash_valid: request.body_hash_valid === true,
          requester_sender_present: request.requester_sender_present === true,
        }
      : null,
    callback_summary: callback
      ? {
          callback_status: callback.callback_status ?? null,
          response_accepted: callback.response_accepted === true,
          action_id: callback.response_summary?.action_id || null,
          status: callback.response_summary?.status || null,
          external_request_id: callback.response_summary?.external_request_id || null,
        }
      : null,
    handoff_manifest_summary: handoffManifestSummary,
    handoff_release_summary: releaseSummary,
    handoff_all_summary: handoffAllSummary,
    third_party_handoff_items: THIRD_PARTY_HANDOFF_ITEMS,
  };
}

function summarizeHandoffManifestValidation(validation, manifestPath) {
  if (!validation) {
    return null;
  }
  return {
    manifest_path: manifestPath || null,
    manifest_version: validation.manifest_version || null,
    ready_for_customer_sandbox: validation.ready_for_customer_sandbox === true,
    checks: Array.isArray(validation.checks)
      ? validation.checks.map((check) => ({
          key: check.key,
          passed: check.passed === true,
        }))
      : [],
    error_codes: Array.isArray(validation.errors)
      ? [...new Set(validation.errors.map((error) => error.code).filter(Boolean))]
      : [],
    warning_codes: Array.isArray(validation.warnings)
      ? [...new Set(validation.warnings.map((warning) => warning.code).filter(Boolean))]
      : [],
  };
}

function summarizeReleaseValidation(validation, packagePath) {
  if (!validation) {
    return null;
  }
  return {
    package_root: packagePath || validation.package_root || null,
    release_ready: validation.release_ready === true,
    package_type: validation.package_type || null,
    generated_at: validation.generated_at || null,
    repository_head: validation.repository_head || null,
    archive_sha256: validation.archive_sha256 || null,
    delivery_manifest_sha256: validation.delivery_manifest_sha256 || null,
    checks: Array.isArray(validation.checks)
      ? validation.checks.map((check) => ({
          key: check.key,
          passed: check.passed === true,
        }))
      : [],
    error_codes: Array.isArray(validation.errors)
      ? [...new Set(validation.errors.map((error) => error.code).filter(Boolean))]
      : [],
    package_error_codes: validation.package_summary?.error_codes || [],
    archive_error_codes: validation.archive_summary?.error_codes || [],
    delivery_error_codes: validation.delivery_manifest_summary?.error_codes || [],
  };
}

function summarizeHandoffAllValidation(validation, packagePath) {
  if (!validation) {
    return null;
  }
  return {
    package_root: packagePath || validation.package_root || null,
    all_ready: validation.all_ready === true,
    checks: Array.isArray(validation.checks)
      ? validation.checks.map((check) => ({
          key: check.key,
          passed: check.passed === true,
        }))
      : [],
    error_codes: Array.isArray(validation.errors)
      ? [...new Set(validation.errors.map((error) => error.code).filter(Boolean))]
      : [],
    scoped_error_codes: Array.isArray(validation.errors)
      ? validation.errors
          .filter((error) => error?.scope && error?.code)
          .map((error) => `${error.scope}:${error.code}`)
      : [],
    package_error_codes: validation.summaries?.package?.error_codes || [],
    archive_error_codes: validation.summaries?.archive?.error_codes || [],
    delivery_error_codes: validation.summaries?.delivery?.error_codes || [],
    release_error_codes: validation.summaries?.release?.error_codes || [],
  };
}

export function renderReadinessMarkdown(report) {
  const status = report.ready_for_customer_sandbox ? 'passed' : 'failed';
  const checks = report.checks
    .map((check) => `- [${check.passed ? 'x' : ' '}] ${check.label} (${check.key})`)
    .join('\n');
  const handoff = report.third_party_handoff_items.map((item) => `- ${item}`).join('\n');
  const dispatch = report.dispatch_summary
    ? [
        `- Action: \`${report.dispatch_summary.action_id || 'unknown'}\``,
        `- Type: \`${report.dispatch_summary.action_type || 'unknown'}\``,
        `- Bearer/signature/body hash: ${[
          report.dispatch_summary.bearer_valid ? 'bearer' : '',
          report.dispatch_summary.signature_valid ? 'signature' : '',
          report.dispatch_summary.body_hash_valid ? 'body_hash' : '',
        ]
          .filter(Boolean)
          .join(', ') || 'not verified'}`,
      ].join('\n')
    : '- No dispatch summary.';
  const callback = report.callback_summary
    ? [
        `- HTTP status: ${report.callback_summary.callback_status ?? 'unknown'}`,
        `- Accepted: ${report.callback_summary.response_accepted ? 'yes' : 'no'}`,
        `- Action: \`${report.callback_summary.action_id || 'unknown'}\``,
        `- Result status: \`${report.callback_summary.status || 'unknown'}\``,
      ].join('\n')
    : '- No callback summary.';
  const handoffManifest = report.handoff_manifest_summary
    ? [
        `- Manifest: \`${report.handoff_manifest_summary.manifest_path || 'unknown'}\``,
        `- Ready: ${report.handoff_manifest_summary.ready_for_customer_sandbox ? 'yes' : 'no'}`,
        `- Errors: ${report.handoff_manifest_summary.error_codes.length || 0}`,
        `- Warnings: ${report.handoff_manifest_summary.warning_codes.length || 0}`,
      ].join('\n')
    : '- No handoff manifest validation attached.';
  const handoffRelease = report.handoff_release_summary
    ? [
        `- Package: \`${report.handoff_release_summary.package_root || 'unknown'}\``,
        `- Ready: ${report.handoff_release_summary.release_ready ? 'yes' : 'no'}`,
        `- Archive SHA256: \`${report.handoff_release_summary.archive_sha256 || 'unknown'}\``,
        `- Delivery manifest SHA256: \`${report.handoff_release_summary.delivery_manifest_sha256 || 'unknown'}\``,
        `- Errors: ${report.handoff_release_summary.error_codes.length || 0}`,
      ].join('\n')
    : '- No handoff release validation attached.';
  const handoffAll = report.handoff_all_summary
    ? [
        `- Package: \`${report.handoff_all_summary.package_root || 'unknown'}\``,
        `- Ready: ${report.handoff_all_summary.all_ready ? 'yes' : 'no'}`,
        `- Checks: ${report.handoff_all_summary.checks.length || 0}`,
        `- Errors: ${report.handoff_all_summary.error_codes.length || 0}`,
      ].join('\n')
    : '- No aggregate handoff validation attached.';

  return `# External Third-Party Readiness Report

- Status: ${status}
- Generated at: ${report.generated_at}
- Repository: ${report.repository || 'unknown'}
- HEAD: ${report.head || 'unknown'}
- Gateway: ${report.gateway_url || 'unknown'}

## Checks

${checks}

## Dispatch Summary

${dispatch}

## Result Callback Summary

${callback}

## Handoff Manifest Summary

${handoffManifest}

## Handoff Release Summary

${handoffRelease}

## Aggregate Handoff Summary

${handoffAll}

## Third-Party Handoff Items

${handoff}
`;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith('--')) {
      continue;
    }
    parsed[arg.slice(2)] = argv[index + 1];
    index += 1;
  }
  return parsed;
}

function readJsonFile(filename, fallback) {
  if (!filename) {
    return fallback;
  }
  return JSON.parse(fs.readFileSync(filename, 'utf8'));
}

function readJsonText(text, fallback) {
  if (!text) {
    return fallback;
  }
  return JSON.parse(text);
}

function writeReportFiles(report, outDir, basename = 'external-third-party-readiness-report') {
  fs.mkdirSync(outDir, { recursive: true });
  const jsonPath = path.join(outDir, `${basename}.json`);
  const markdownPath = path.join(outDir, `${basename}.md`);
  fs.writeFileSync(jsonPath, `${JSON.stringify(report, null, 2)}\n`);
  fs.writeFileSync(markdownPath, renderReadinessMarkdown(report));
  return { jsonPath, markdownPath };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const handoffManifestPath = args.handoffManifest || process.env.EXTERNAL_THIRD_PARTY_HANDOFF_MANIFEST || '';
  const handoffManifestValidation = handoffManifestPath
    ? validateExternalHandoffManifest(readJsonFile(handoffManifestPath, null))
    : null;
  const releasePackagePath = args.releasePackage || process.env.EXTERNAL_THIRD_PARTY_HANDOFF_RELEASE_PACKAGE || '';
  const releaseValidation = releasePackagePath
    ? validateRelease({ packageRootInput: releasePackagePath })
    : null;
  const handoffAllValidation = releasePackagePath
    ? validateAll({ packageRootInput: releasePackagePath })
    : null;
  const requestsPayload = args.requests
    ? readJsonFile(args.requests, { requests: [] })
    : readJsonText(process.env.EXTERNAL_THIRD_PARTY_REQUESTS_JSON, { requests: [] });
  const callbacksPayload = args.callbacks
    ? readJsonFile(args.callbacks, { callbacks: [] })
    : readJsonText(process.env.EXTERNAL_THIRD_PARTY_CALLBACKS_JSON, { callbacks: [] });
  const report = buildReadinessReport({
    generatedAt: args.generatedAt || new Date().toISOString(),
    gatewayUrl: args.gateway || '',
    repository: args.repository || '',
    head: args.head || '',
    requests: Array.isArray(requestsPayload.requests) ? requestsPayload.requests : [],
    callbacks: Array.isArray(callbacksPayload.callbacks) ? callbacksPayload.callbacks : [],
    handoffManifestPath,
    handoffManifestValidation,
    releasePackagePath,
    releaseValidation,
    handoffAllValidation,
  });
  const outDir = args.outDir || path.join('target', 'external-third-party-readiness');
  const basename = args.basename || 'external-third-party-readiness-report';
  const files = writeReportFiles(report, outDir, basename);
  console.log(JSON.stringify({ ...files, ready: report.ready_for_customer_sandbox }, null, 2));
  if (!report.ready_for_customer_sandbox) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}
