#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validateArchive } from './validate-external-handoff-archive.mjs';
import { validateDelivery } from './validate-external-handoff-delivery.mjs';
import { validateExternalHandoffManifest } from './validate-external-handoff.mjs';
import { validatePackage } from './validate-external-handoff-package.mjs';
import { validateRelease } from './validate-external-handoff-release.mjs';

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

function readHandoffManifest(manifestPath) {
  try {
    return validateExternalHandoffManifest(JSON.parse(fs.readFileSync(manifestPath, 'utf8')));
  } catch (error) {
    return {
      report_type: 'external_third_party_handoff_validation',
      ready_for_customer_sandbox: false,
      checks: [{ key: 'handoff_manifest_readable', passed: false }],
      errors: [{
        code: 'handoff_manifest_read_failed',
        message: `failed to read handoff manifest: ${error.message}`,
        path: manifestPath,
      }],
      warnings: [],
    };
  }
}

function scopedErrors(scope, errors = []) {
  return errors.map((error) => ({
    scope,
    code: error.code,
    message: error.message,
    path: error.path || '',
  }));
}

function summarizeErrorCodes(report) {
  return (report.errors || []).map((error) => error.code);
}

const HTML_ARTIFACT_COMMAND_CONTRACT_ERROR_CODES = new Set([
  'html_artifact_all_validation_missing',
  'html_artifact_all_validation_receipt_check_missing',
  'html_artifact_all_validation_verify_markdown_missing',
  'html_artifact_evidence_validation_missing',
  'html_artifact_evidence_validation_receipt_check_missing',
  'html_artifact_evidence_validation_verify_markdown_missing',
]);

function summarizeHtmlArtifactCommandContract(validation) {
  const errorCodes = Array.isArray(validation?.error_codes) ? validation.error_codes : [];
  const commandErrorCodes = errorCodes.filter((code) => HTML_ARTIFACT_COMMAND_CONTRACT_ERROR_CODES.has(code));
  const validationCommandCount = Number(validation?.validation_command_count || 0);
  return {
    validation_command_count: validationCommandCount,
    command_contract_ready: validationCommandCount > 0 && commandErrorCodes.length === 0,
    command_contract_error_codes: commandErrorCodes,
  };
}

function addError(errors, code, message, location = '') {
  errors.push({ code, message, path: location });
}

function sha256Hex(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function renderAllMarkdown(report) {
  const status = report.all_ready ? 'READY' : 'NOT READY';
  const checks = (report.checks || [])
    .map((check) => `- ${check.passed ? 'PASS' : 'FAIL'} \`${check.key}\``)
    .join('\n');
  const errors = (report.errors || []).length
    ? report.errors
        .map((error) => `- \`${error.scope}:${error.code}\`: ${error.message}${error.path ? ` (${error.path})` : ''}`)
        .join('\n')
    : '- None';

  return `# External Third-Party Handoff Aggregate Validation

Status: **${status}**

## Package

- Package type: \`${report.package_type || 'unknown'}\`
- Generated at: ${report.generated_at || 'unknown'}
- Repository head: \`${report.repository_head || 'unknown'}\`
- Package root: \`${report.package_root || 'unknown'}\`
- Handoff manifest: \`${report.manifest_path || 'unknown'}\`
- Archive: \`${report.archive_path || 'unknown'}\`
- SHA256 sidecar: \`${report.sidecar_path || 'unknown'}\`

## Checks

${checks}

## Summaries

- Handoff ready: ${report.summaries?.handoff?.ready_for_customer_sandbox ? 'yes' : 'no'}
- Package ready: ${report.summaries?.package?.package_ready ? 'yes' : 'no'}
- Included files: ${report.summaries?.package?.included_file_count ?? 0}
- HTML artifact ready: ${report.summaries?.package?.html_artifact_ready ? 'yes' : 'no'}
- HTML command contract ready: ${report.summaries?.package?.html_artifact_command_contract_ready ? 'yes' : 'no'}
- HTML validation commands: ${report.summaries?.package?.html_artifact_validation_command_count ?? 0}
- Archive ready: ${report.summaries?.archive?.archive_ready ? 'yes' : 'no'}
- Archive HTML artifact ready: ${report.summaries?.archive?.html_artifact_ready ? 'yes' : 'no'}
- Archive HTML command contract ready: ${report.summaries?.archive?.html_artifact_command_contract_ready ? 'yes' : 'no'}
- Archive HTML validation commands: ${report.summaries?.archive?.html_artifact_validation_command_count ?? 0}
- Archive root: \`${report.summaries?.archive?.root_name || 'unknown'}\`
- Archive entries: ${report.summaries?.archive?.entry_count ?? 0}
- Delivery manifest ready: ${report.summaries?.delivery?.delivery_manifest_ready ? 'yes' : 'no'}
- Delivery artifacts: ${report.summaries?.delivery?.artifact_count ?? 0}
- Release ready: ${report.summaries?.release?.release_ready ? 'yes' : 'no'}
- Archive SHA256: \`${report.summaries?.release?.archive_sha256 || 'unknown'}\`
- Delivery manifest SHA256: \`${report.summaries?.release?.delivery_manifest_sha256 || 'unknown'}\`

## Errors

${errors}
`;
}

function writeOutputFile(filePath, contents) {
  if (!filePath) {
    return '';
  }
  const resolved = path.resolve(filePath);
  fs.mkdirSync(path.dirname(resolved), { recursive: true });
  fs.writeFileSync(resolved, contents);
  return resolved;
}

function writeAllReportFiles(report, { out = '', markdown = '' } = {}) {
  return {
    jsonPath: writeOutputFile(out, `${JSON.stringify(report, null, 2)}\n`),
    markdownPath: writeOutputFile(markdown, renderAllMarkdown(report)),
  };
}

function validateAllMarkdownReceipt({ validation, markdownPathInput = '' } = {}) {
  const errors = [];
  const packageRoot = validation?.package_root || '';
  const markdownPath = path.resolve(
    markdownPathInput
      || (packageRoot
        ? path.join(path.dirname(packageRoot), `${path.basename(packageRoot)}.all.md`)
        : 'handoff-all.md'),
  );
  const expected = renderAllMarkdown(validation || {});
  const expectedDigest = {
    bytes: Buffer.byteLength(expected),
    sha256: sha256Hex(Buffer.from(expected)),
  };

  if (!fs.existsSync(markdownPath)) {
    addError(errors, 'aggregate_markdown_receipt_missing', 'aggregate markdown receipt is missing', markdownPath);
    return {
      receipt_ready: false,
      receipt_path: markdownPath,
      receipt_sha256: null,
      expected_sha256: expectedDigest.sha256,
      error_codes: errors.map((error) => error.code),
      errors,
    };
  }

  const actual = fs.readFileSync(markdownPath);
  const actualDigest = {
    bytes: actual.length,
    sha256: sha256Hex(actual),
  };
  if (actualDigest.bytes !== expectedDigest.bytes) {
    addError(
      errors,
      'aggregate_markdown_receipt_bytes_mismatch',
      'aggregate markdown receipt byte size does not match validation output',
      markdownPath,
    );
  }
  if (actualDigest.sha256 !== expectedDigest.sha256) {
    addError(
      errors,
      'aggregate_markdown_receipt_sha256_mismatch',
      'aggregate markdown receipt SHA256 does not match validation output',
      markdownPath,
    );
  }

  return {
    receipt_ready: errors.length === 0,
    receipt_path: markdownPath,
    receipt_sha256: actualDigest.sha256,
    expected_sha256: expectedDigest.sha256,
    error_codes: errors.map((error) => error.code),
    errors,
  };
}

function validateAll({
  packageRootInput = '.',
  manifestPathInput = '',
  archivePathInput = '',
  sidecarPathInput = '',
} = {}) {
  const packageRoot = path.resolve(packageRootInput || '.');
  const packageName = path.basename(packageRoot);
  const deliveryRoot = path.dirname(packageRoot);
  const manifestPath = path.resolve(manifestPathInput || path.join(packageRoot, 'handoff', 'third-party-handoff.sample.json'));
  const archivePath = path.resolve(archivePathInput || path.join(deliveryRoot, `${packageName}.tar.gz`));
  const sidecarPath = path.resolve(sidecarPathInput || `${archivePath}.sha256`);

  const handoff = readHandoffManifest(manifestPath);
  const packageReport = validatePackage(packageRoot);
  const archive = validateArchive(archivePath, sidecarPath);
  const delivery = validateDelivery({
    packageRootInput: packageRoot,
    archivePathInput: archivePath,
    sidecarPathInput: sidecarPath,
  });
  const release = validateRelease({
    packageRootInput: packageRoot,
    archivePathInput: archivePath,
    sidecarPathInput: sidecarPath,
  });
  const packageHtmlCommandContract = summarizeHtmlArtifactCommandContract(packageReport.html_artifact_validation);
  const archiveHtmlCommandContract = summarizeHtmlArtifactCommandContract(archive.html_artifact_validation);

  const checks = [
    { key: 'handoff_manifest_ready', passed: handoff.ready_for_customer_sandbox === true },
    { key: 'package_ready', passed: packageReport.package_ready === true },
    { key: 'archive_ready', passed: archive.archive_ready === true },
    { key: 'delivery_manifest_ready', passed: delivery.delivery_manifest_ready === true },
    { key: 'release_ready', passed: release.release_ready === true },
  ];
  const errors = [
    ...scopedErrors('handoff', handoff.errors),
    ...scopedErrors('package', packageReport.errors),
    ...scopedErrors('archive', archive.errors),
    ...scopedErrors('delivery', delivery.errors),
    ...scopedErrors('release', release.errors),
  ];

  return {
    report_type: 'external_third_party_handoff_all_validation',
    all_ready: checks.every((check) => check.passed),
    package_type: packageReport.package_type || null,
    generated_at: packageReport.generated_at || null,
    repository_head: packageReport.repository_head || null,
    package_root: packageRoot,
    manifest_path: manifestPath,
    archive_path: archivePath,
    sidecar_path: sidecarPath,
    checks,
    summaries: {
      handoff: {
        ready_for_customer_sandbox: handoff.ready_for_customer_sandbox === true,
        error_codes: summarizeErrorCodes(handoff),
        warning_codes: (handoff.warnings || []).map((warning) => warning.code),
      },
      package: {
        package_type: packageReport.package_type || null,
        generated_at: packageReport.generated_at || null,
        repository_head: packageReport.repository_head || null,
        package_ready: packageReport.package_ready === true,
        included_file_count: packageReport.included_file_count,
        html_artifact_ready: packageReport.html_artifact_validation?.artifact_ready === true,
        html_artifact_template_id: packageReport.html_artifact_validation?.template_id || null,
        html_artifact_validation_command_count: packageHtmlCommandContract.validation_command_count,
        html_artifact_command_contract_ready: packageHtmlCommandContract.command_contract_ready,
        html_artifact_command_contract_error_codes: packageHtmlCommandContract.command_contract_error_codes,
        error_codes: summarizeErrorCodes(packageReport),
      },
      archive: {
        archive_ready: archive.archive_ready === true,
        root_name: archive.root_name,
        entry_count: archive.entry_count,
        html_artifact_ready: archive.html_artifact_validation?.artifact_ready === true,
        html_artifact_template_id: archive.html_artifact_validation?.template_id || null,
        html_artifact_validation_command_count: archiveHtmlCommandContract.validation_command_count,
        html_artifact_command_contract_ready: archiveHtmlCommandContract.command_contract_ready,
        html_artifact_command_contract_error_codes: archiveHtmlCommandContract.command_contract_error_codes,
        error_codes: summarizeErrorCodes(archive),
      },
      delivery: {
        delivery_manifest_ready: delivery.delivery_manifest_ready === true,
        artifact_count: delivery.artifact_count,
        error_codes: summarizeErrorCodes(delivery),
      },
      release: {
        release_ready: release.release_ready === true,
        archive_sha256: release.archive_sha256 || null,
        delivery_manifest_sha256: release.delivery_manifest_sha256 || null,
        error_codes: summarizeErrorCodes(release),
      },
    },
    errors,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  let result = validateAll({
    packageRootInput: args.package || '.',
    manifestPathInput: args.manifest || '',
    archivePathInput: args.archive || '',
    sidecarPathInput: args.sha256 || '',
  });
  if (args.verifyMarkdown) {
    const verifyMarkdownPath = args.verifyMarkdown === 'auto' ? '' : args.verifyMarkdown;
    const receipt = validateAllMarkdownReceipt({
      validation: result,
      markdownPathInput: verifyMarkdownPath,
    });
    result = {
      ...result,
      all_ready: result.all_ready === true && receipt.receipt_ready === true,
      checks: [
        ...result.checks,
        { key: 'aggregate_markdown_receipt_ready', passed: receipt.receipt_ready === true },
      ],
      aggregate_markdown_receipt: receipt,
      errors: [...result.errors, ...scopedErrors('aggregate_markdown', receipt.errors)],
    };
  }
  writeAllReportFiles(result, {
    out: args.out || '',
    markdown: args.markdown || '',
  });
  console.log(JSON.stringify(result, null, 2));
  if (!result.all_ready) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export { renderAllMarkdown, validateAll, validateAllMarkdownReceipt, writeAllReportFiles };
