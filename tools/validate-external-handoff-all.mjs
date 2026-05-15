#!/usr/bin/env node
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
- Archive ready: ${report.summaries?.archive?.archive_ready ? 'yes' : 'no'}
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
        package_ready: packageReport.package_ready === true,
        included_file_count: packageReport.included_file_count,
        error_codes: summarizeErrorCodes(packageReport),
      },
      archive: {
        archive_ready: archive.archive_ready === true,
        root_name: archive.root_name,
        entry_count: archive.entry_count,
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
  const result = validateAll({
    packageRootInput: args.package || '.',
    manifestPathInput: args.manifest || '',
    archivePathInput: args.archive || '',
    sidecarPathInput: args.sha256 || '',
  });
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

export { renderAllMarkdown, validateAll, writeAllReportFiles };
