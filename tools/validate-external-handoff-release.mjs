#!/usr/bin/env node
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validatePackage } from './validate-external-handoff-package.mjs';
import { validateArchive } from './validate-external-handoff-archive.mjs';

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

function addError(errors, code, message, location = '') {
  errors.push({ code, message, path: location });
}

function summarizeChecks(prefix, checks = []) {
  return checks.map((check) => ({
    key: check.key.startsWith(`${prefix}_`) ? check.key : `${prefix}_${check.key}`,
    passed: check.passed === true,
  }));
}

function renderReleaseMarkdown(report) {
  const status = report.release_ready ? 'READY' : 'NOT READY';
  const checks = (report.checks || [])
    .map((check) => `- ${check.passed ? 'PASS' : 'FAIL'} \`${check.key}\``)
    .join('\n');
  const errors = (report.errors || []).length
    ? report.errors.map((error) => `- \`${error.code}\`: ${error.message}${error.path ? ` (${error.path})` : ''}`).join('\n')
    : '- None';

  return `# External Third-Party Handoff Release Report

Status: **${status}**

## Package

- Package type: \`${report.package_summary?.package_type || 'unknown'}\`
- Generated at: ${report.package_summary?.generated_at || 'unknown'}
- Repository head: \`${report.package_summary?.repository_head || 'unknown'}\`
- Package root: \`${report.package_root}\`
- Included files: ${report.package_summary?.included_file_count ?? 0}
- Package ready: ${report.package_summary?.package_ready ? 'yes' : 'no'}
- Handoff ready: ${report.package_summary?.handoff_ready ? 'yes' : 'no'}

## Archive

- Archive path: \`${report.archive_path}\`
- Archive SHA256: \`${report.archive_sha256 || 'unknown'}\`
- Archive root: \`${report.archive_summary?.root_name || 'unknown'}\`
- Archive entries: ${report.archive_summary?.entry_count ?? 0}
- Archive ready: ${report.archive_summary?.archive_ready ? 'yes' : 'no'}

## Checks

${checks}

## Errors

${errors}
`;
}

function validateRelease({ packageRootInput = '.', archivePathInput = '', sidecarPathInput = '' } = {}) {
  const errors = [];
  const packageRoot = path.resolve(packageRootInput || '.');
  const inferredArchivePath = path.resolve(path.dirname(packageRoot), `${path.basename(packageRoot)}.tar.gz`);
  const archivePath = path.resolve(archivePathInput || inferredArchivePath);
  const packageValidation = validatePackage(packageRoot);
  const archiveValidation = validateArchive(archivePath, sidecarPathInput);

  for (const error of packageValidation.errors || []) {
    addError(errors, `package_${error.code}`, error.message, error.path);
  }
  for (const error of archiveValidation.errors || []) {
    addError(errors, `archive_${error.code}`, error.message, error.path);
  }

  const packageName = path.basename(packageRoot);
  if (archiveValidation.root_name && archiveValidation.root_name !== packageName) {
    addError(
      errors,
      'archive_root_package_mismatch',
      'archive root directory must match package directory name',
      archiveValidation.root_name,
    );
  }

  const checks = [
    { key: 'package_ready', passed: packageValidation.package_ready === true },
    { key: 'archive_ready', passed: archiveValidation.archive_ready === true },
    {
      key: 'archive_root_matches_package',
      passed: archiveValidation.root_name === packageName,
    },
    ...summarizeChecks('package', packageValidation.checks),
    ...summarizeChecks('archive', archiveValidation.checks),
  ];

  return {
    report_type: 'external_third_party_handoff_release_validation',
    release_ready: errors.length === 0,
    package_type: packageValidation.package_type || null,
    generated_at: packageValidation.generated_at || null,
    repository_head: packageValidation.repository_head || null,
    package_root: packageRoot,
    archive_path: archivePath,
    archive_sha256: archiveValidation.archive_sha256,
    checks,
    package_summary: {
      package_type: packageValidation.package_type || null,
      generated_at: packageValidation.generated_at || null,
      repository_head: packageValidation.repository_head || null,
      package_ready: packageValidation.package_ready === true,
      included_file_count: packageValidation.included_file_count,
      handoff_ready: packageValidation.handoff_validation?.ready_for_customer_sandbox === true,
      error_codes: (packageValidation.errors || []).map((error) => error.code),
    },
    archive_summary: {
      archive_ready: archiveValidation.archive_ready === true,
      root_name: archiveValidation.root_name,
      entry_count: archiveValidation.entry_count,
      handoff_ready: archiveValidation.handoff_validation?.ready_for_customer_sandbox === true,
      error_codes: (archiveValidation.errors || []).map((error) => error.code),
    },
    errors,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = validateRelease({
    packageRootInput: args.package || '.',
    archivePathInput: args.archive || '',
    sidecarPathInput: args.sha256 || '',
  });
  if (args.markdown) {
    await import('node:fs').then((fs) => {
      fs.writeFileSync(path.resolve(args.markdown), renderReleaseMarkdown(result));
    });
  }
  console.log(JSON.stringify(result, null, 2));
  if (!result.release_ready) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export { renderReleaseMarkdown, validateRelease };
