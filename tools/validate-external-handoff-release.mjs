#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validatePackage } from './validate-external-handoff-package.mjs';
import { validateArchive } from './validate-external-handoff-archive.mjs';

const DELIVERY_MANIFEST_TYPE = 'v3.external_third_party_handoff_delivery_manifest.v1';
const DELIVERY_ARTIFACT_ROLES = [
  'package_directory',
  'package_manifest',
  'archive',
  'archive_sha256_sidecar',
  'release_json',
  'release_markdown',
];

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

function sha256Hex(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function fileDigest(filePath) {
  const bytes = fs.readFileSync(filePath);
  return {
    bytes: bytes.length,
    sha256: sha256Hex(bytes),
  };
}

function summarizeChecks(prefix, checks = []) {
  return checks.map((check) => ({
    key: check.key.startsWith(`${prefix}_`) ? check.key : `${prefix}_${check.key}`,
    passed: check.passed === true,
  }));
}

function parseJsonFile(filePath, errors, codePrefix) {
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch (error) {
    addError(errors, `${codePrefix}_invalid_json`, `failed to parse JSON: ${error.message}`, filePath);
    return null;
  }
}

function validateDeliveryFileArtifact({
  artifactsByRole,
  role,
  expectedPath,
  expectedFilePath,
  errors,
}) {
  const artifact = artifactsByRole.get(role);
  if (!artifact) {
    addError(errors, `delivery_${role}_missing`, `delivery manifest must include ${role}`, role);
    return;
  }
  if (artifact.path !== expectedPath) {
    addError(
      errors,
      `delivery_${role}_path_mismatch`,
      `delivery manifest ${role} path must be ${expectedPath}`,
      artifact.path || role,
    );
  }
  if (!fs.existsSync(expectedFilePath)) {
    addError(errors, `delivery_${role}_file_missing`, `delivery artifact file is missing`, expectedPath);
    return;
  }
  const digest = fileDigest(expectedFilePath);
  if (artifact.bytes !== digest.bytes) {
    addError(errors, `delivery_${role}_bytes_mismatch`, `delivery manifest ${role} byte size does not match file`, expectedPath);
  }
  if (artifact.sha256 !== digest.sha256) {
    addError(errors, `delivery_${role}_sha256_mismatch`, `delivery manifest ${role} SHA256 does not match file`, expectedPath);
  }
}

function validateDeliveryManifest({
  packageRoot,
  archivePath,
  sidecarPath,
  releaseReportPath,
  releaseMarkdownPath,
  deliveryManifestPath,
  required = true,
} = {}) {
  const errors = [];
  const packageName = path.basename(packageRoot);
  const deliveryRoot = path.dirname(packageRoot);
  const inferredDeliveryManifestPath = path.join(deliveryRoot, `${packageName}.delivery-manifest.json`);
  const manifestPath = path.resolve(deliveryManifestPath || inferredDeliveryManifestPath);

  if (!fs.existsSync(manifestPath)) {
    if (required) {
      addError(errors, 'delivery_manifest_missing', 'delivery manifest is required for release validation', manifestPath);
    }
    return {
      delivery_manifest_ready: required ? errors.length === 0 : null,
      delivery_manifest_path: manifestPath,
      delivery_manifest_sha256: null,
      manifest_type: null,
      artifact_count: 0,
      required: required === true,
      error_codes: errors.map((error) => error.code),
      errors,
    };
  }

  const manifest = parseJsonFile(manifestPath, errors, 'delivery_manifest');
  if (!manifest) {
    return {
      delivery_manifest_ready: false,
      delivery_manifest_path: manifestPath,
      delivery_manifest_sha256: null,
      manifest_type: null,
      artifact_count: 0,
      required: required === true,
      error_codes: errors.map((error) => error.code),
      errors,
    };
  }

  if (manifest.manifest_type !== DELIVERY_MANIFEST_TYPE) {
    addError(errors, 'delivery_manifest_type_invalid', `delivery manifest type must be ${DELIVERY_MANIFEST_TYPE}`, manifestPath);
  }
  if (manifest.package_name !== packageName) {
    addError(errors, 'delivery_package_name_mismatch', 'delivery manifest package_name must match package directory', manifest.package_name || '');
  }
  if (manifest.package_root !== packageName) {
    addError(errors, 'delivery_package_root_mismatch', 'delivery manifest package_root must match package directory', manifest.package_root || '');
  }
  if (manifest.delivery_root !== '.') {
    addError(errors, 'delivery_root_invalid', 'delivery manifest delivery_root must be "."', manifest.delivery_root || '');
  }
  if (manifest.release_ready !== true) {
    addError(errors, 'delivery_release_not_ready', 'delivery manifest must mark release_ready=true', manifestPath);
  }

  const artifacts = Array.isArray(manifest.artifacts) ? manifest.artifacts : [];
  if (!Array.isArray(manifest.artifacts)) {
    addError(errors, 'delivery_artifacts_invalid', 'delivery manifest artifacts must be an array', manifestPath);
  }
  const artifactsByRole = new Map(artifacts.map((artifact) => [artifact.role, artifact]));
  for (const role of DELIVERY_ARTIFACT_ROLES) {
    if (!artifactsByRole.has(role)) {
      addError(errors, `delivery_${role}_missing`, `delivery manifest must include ${role}`, role);
    }
  }

  const packageDirectory = artifactsByRole.get('package_directory');
  if (packageDirectory) {
    if (packageDirectory.type !== 'directory') {
      addError(errors, 'delivery_package_directory_type_invalid', 'package_directory artifact must be a directory', 'package_directory');
    }
    if (packageDirectory.path !== packageName) {
      addError(errors, 'delivery_package_directory_path_mismatch', `package_directory path must be ${packageName}`, packageDirectory.path || '');
    }
  }

  validateDeliveryFileArtifact({
    artifactsByRole,
    role: 'package_manifest',
    expectedPath: `${packageName}/handoff-package-manifest.json`,
    expectedFilePath: path.join(packageRoot, 'handoff-package-manifest.json'),
    errors,
  });
  validateDeliveryFileArtifact({
    artifactsByRole,
    role: 'archive',
    expectedPath: path.relative(deliveryRoot, archivePath).replaceAll('\\', '/'),
    expectedFilePath: archivePath,
    errors,
  });
  validateDeliveryFileArtifact({
    artifactsByRole,
    role: 'archive_sha256_sidecar',
    expectedPath: path.relative(deliveryRoot, sidecarPath).replaceAll('\\', '/'),
    expectedFilePath: sidecarPath,
    errors,
  });
  validateDeliveryFileArtifact({
    artifactsByRole,
    role: 'release_json',
    expectedPath: path.relative(deliveryRoot, releaseReportPath).replaceAll('\\', '/'),
    expectedFilePath: releaseReportPath,
    errors,
  });
  validateDeliveryFileArtifact({
    artifactsByRole,
    role: 'release_markdown',
    expectedPath: path.relative(deliveryRoot, releaseMarkdownPath).replaceAll('\\', '/'),
    expectedFilePath: releaseMarkdownPath,
    errors,
  });

  return {
    delivery_manifest_ready: errors.length === 0,
    delivery_manifest_path: manifestPath,
    delivery_manifest_sha256: fileDigest(manifestPath).sha256,
    manifest_type: manifest.manifest_type || null,
    package_name: manifest.package_name || null,
    artifact_count: artifacts.length,
    required: required === true,
    error_codes: errors.map((error) => error.code),
    errors,
  };
}

function renderReleaseMarkdown(report) {
  const status = report.release_ready ? 'READY' : 'NOT READY';
  const deliveryReady =
    report.delivery_manifest_summary?.delivery_manifest_ready === null
      ? 'not checked'
      : report.delivery_manifest_summary?.delivery_manifest_ready
        ? 'yes'
        : 'no';
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

## Delivery Manifest

- Delivery manifest path: \`${report.delivery_manifest_path || 'unknown'}\`
- Delivery manifest SHA256: \`${report.delivery_manifest_sha256 || 'unknown'}\`
- Delivery manifest ready: ${deliveryReady}
- Delivery artifacts: ${report.delivery_manifest_summary?.artifact_count ?? 0}

## Checks

${checks}

## Errors

${errors}
`;
}

function validateRelease({
  packageRootInput = '.',
  archivePathInput = '',
  sidecarPathInput = '',
  deliveryManifestInput = '',
  deliveryManifestRequired = true,
} = {}) {
  const errors = [];
  const packageRoot = path.resolve(packageRootInput || '.');
  const inferredArchivePath = path.resolve(path.dirname(packageRoot), `${path.basename(packageRoot)}.tar.gz`);
  const archivePath = path.resolve(archivePathInput || inferredArchivePath);
  const sidecarPath = path.resolve(sidecarPathInput || `${archivePath}.sha256`);
  const releaseReportPath = path.resolve(path.dirname(packageRoot), `${path.basename(packageRoot)}.release.json`);
  const releaseMarkdownPath = path.resolve(path.dirname(packageRoot), `${path.basename(packageRoot)}.release.md`);
  const packageValidation = validatePackage(packageRoot);
  const archiveValidation = validateArchive(archivePath, sidecarPath);
  const deliveryValidation = validateDeliveryManifest({
    packageRoot,
    archivePath,
    sidecarPath,
    releaseReportPath,
    releaseMarkdownPath,
    deliveryManifestPath: deliveryManifestInput,
    required: deliveryManifestRequired,
  });

  for (const error of packageValidation.errors || []) {
    addError(errors, `package_${error.code}`, error.message, error.path);
  }
  for (const error of archiveValidation.errors || []) {
    addError(errors, `archive_${error.code}`, error.message, error.path);
  }
  for (const error of deliveryValidation.errors || []) {
    addError(errors, error.code, error.message, error.path);
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
  if (deliveryManifestRequired || deliveryValidation.delivery_manifest_sha256) {
    checks.splice(3, 0, {
      key: 'delivery_manifest_ready',
      passed: deliveryValidation.delivery_manifest_ready === true,
    });
  }

  return {
    report_type: 'external_third_party_handoff_release_validation',
    release_ready: errors.length === 0,
    package_type: packageValidation.package_type || null,
    generated_at: packageValidation.generated_at || null,
    repository_head: packageValidation.repository_head || null,
    package_root: packageRoot,
    archive_path: archivePath,
    archive_sha256: archiveValidation.archive_sha256,
    delivery_manifest_path: deliveryValidation.delivery_manifest_path,
    delivery_manifest_sha256: deliveryValidation.delivery_manifest_sha256,
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
    delivery_manifest_summary: {
      delivery_manifest_ready: deliveryValidation.delivery_manifest_ready,
      manifest_type: deliveryValidation.manifest_type,
      package_name: deliveryValidation.package_name,
      artifact_count: deliveryValidation.artifact_count,
      required: deliveryValidation.required,
      error_codes: deliveryValidation.error_codes,
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
    deliveryManifestInput: args.deliveryManifest || '',
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

export { renderReleaseMarkdown, validateDeliveryManifest, validateRelease };
