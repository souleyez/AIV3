#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const EVIDENCE_MANIFEST_TYPE = 'v3.external_third_party_handoff_evidence_manifest.v1';
const EVIDENCE_ARTIFACT_ROLES = [
  'package_directory',
  'package_manifest',
  'archive',
  'archive_sha256_sidecar',
  'release_json',
  'release_markdown',
  'delivery_manifest',
  'aggregate_json',
  'aggregate_markdown',
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

function parseJsonFile(filePath, errors, codePrefix) {
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch (error) {
    addError(errors, `${codePrefix}_invalid_json`, `failed to parse JSON: ${error.message}`, filePath);
    return null;
  }
}

function validateEvidenceFileArtifact({
  artifactsByRole,
  role,
  expectedPath,
  expectedFilePath,
  errors,
}) {
  const artifact = artifactsByRole.get(role);
  if (!artifact) {
    addError(errors, `evidence_${role}_missing`, `evidence manifest must include ${role}`, role);
    return;
  }
  if (artifact.path !== expectedPath) {
    addError(
      errors,
      `evidence_${role}_path_mismatch`,
      `evidence manifest ${role} path must be ${expectedPath}`,
      artifact.path || role,
    );
  }
  if (!fs.existsSync(expectedFilePath)) {
    addError(errors, `evidence_${role}_file_missing`, 'evidence artifact file is missing', expectedPath);
    return;
  }
  const digest = fileDigest(expectedFilePath);
  if (artifact.bytes !== digest.bytes) {
    addError(errors, `evidence_${role}_bytes_mismatch`, `evidence manifest ${role} byte size does not match file`, expectedPath);
  }
  if (artifact.sha256 !== digest.sha256) {
    addError(errors, `evidence_${role}_sha256_mismatch`, `evidence manifest ${role} SHA256 does not match file`, expectedPath);
  }
}

function validateEvidence({
  packageRootInput = '.',
  archivePathInput = '',
  sidecarPathInput = '',
  releaseReportPathInput = '',
  releaseMarkdownPathInput = '',
  deliveryManifestInput = '',
  allReportPathInput = '',
  allMarkdownPathInput = '',
  evidenceManifestInput = '',
} = {}) {
  const errors = [];
  const packageRoot = path.resolve(packageRootInput || '.');
  const packageName = path.basename(packageRoot);
  const deliveryRoot = path.dirname(packageRoot);
  const archivePath = path.resolve(archivePathInput || path.join(deliveryRoot, `${packageName}.tar.gz`));
  const sidecarPath = path.resolve(sidecarPathInput || `${archivePath}.sha256`);
  const releaseReportPath = path.resolve(releaseReportPathInput || path.join(deliveryRoot, `${packageName}.release.json`));
  const releaseMarkdownPath = path.resolve(releaseMarkdownPathInput || path.join(deliveryRoot, `${packageName}.release.md`));
  const deliveryManifestPath = path.resolve(deliveryManifestInput || path.join(deliveryRoot, `${packageName}.delivery-manifest.json`));
  const allReportPath = path.resolve(allReportPathInput || path.join(deliveryRoot, `${packageName}.all.json`));
  const allMarkdownPath = path.resolve(allMarkdownPathInput || path.join(deliveryRoot, `${packageName}.all.md`));
  const evidenceManifestPath = path.resolve(evidenceManifestInput || path.join(deliveryRoot, `${packageName}.evidence-manifest.json`));

  if (!fs.existsSync(evidenceManifestPath)) {
    addError(errors, 'evidence_manifest_missing', 'evidence manifest is required for final handoff validation', evidenceManifestPath);
    return {
      report_type: 'external_third_party_handoff_evidence_validation',
      evidence_manifest_ready: false,
      evidence_manifest_path: evidenceManifestPath,
      evidence_manifest_sha256: null,
      manifest_type: null,
      package_name: packageName,
      artifact_count: 0,
      error_codes: errors.map((error) => error.code),
      errors,
    };
  }

  const manifest = parseJsonFile(evidenceManifestPath, errors, 'evidence_manifest');
  if (!manifest) {
    return {
      report_type: 'external_third_party_handoff_evidence_validation',
      evidence_manifest_ready: false,
      evidence_manifest_path: evidenceManifestPath,
      evidence_manifest_sha256: null,
      manifest_type: null,
      package_name: packageName,
      artifact_count: 0,
      error_codes: errors.map((error) => error.code),
      errors,
    };
  }

  if (manifest.manifest_type !== EVIDENCE_MANIFEST_TYPE) {
    addError(errors, 'evidence_manifest_type_invalid', `evidence manifest type must be ${EVIDENCE_MANIFEST_TYPE}`, evidenceManifestPath);
  }
  if (manifest.package_name !== packageName) {
    addError(errors, 'evidence_package_name_mismatch', 'evidence manifest package_name must match package directory', manifest.package_name || '');
  }
  if (manifest.package_root !== packageName) {
    addError(errors, 'evidence_package_root_mismatch', 'evidence manifest package_root must match package directory', manifest.package_root || '');
  }
  if (manifest.delivery_root !== '.') {
    addError(errors, 'evidence_root_invalid', 'evidence manifest delivery_root must be "."', manifest.delivery_root || '');
  }
  if (manifest.release_ready !== true) {
    addError(errors, 'evidence_release_not_ready', 'evidence manifest must mark release_ready=true', evidenceManifestPath);
  }
  if (manifest.all_ready !== true) {
    addError(errors, 'evidence_aggregate_not_ready', 'evidence manifest must mark all_ready=true', evidenceManifestPath);
  }

  const artifacts = Array.isArray(manifest.artifacts) ? manifest.artifacts : [];
  if (!Array.isArray(manifest.artifacts)) {
    addError(errors, 'evidence_artifacts_invalid', 'evidence manifest artifacts must be an array', evidenceManifestPath);
  }
  const artifactsByRole = new Map(artifacts.map((artifact) => [artifact.role, artifact]));
  for (const role of EVIDENCE_ARTIFACT_ROLES) {
    if (!artifactsByRole.has(role)) {
      addError(errors, `evidence_${role}_missing`, `evidence manifest must include ${role}`, role);
    }
  }

  const packageDirectory = artifactsByRole.get('package_directory');
  if (packageDirectory) {
    if (packageDirectory.type !== 'directory') {
      addError(errors, 'evidence_package_directory_type_invalid', 'package_directory artifact must be a directory', 'package_directory');
    }
    if (packageDirectory.path !== packageName) {
      addError(errors, 'evidence_package_directory_path_mismatch', `package_directory path must be ${packageName}`, packageDirectory.path || '');
    }
  }

  validateEvidenceFileArtifact({
    artifactsByRole,
    role: 'package_manifest',
    expectedPath: `${packageName}/handoff-package-manifest.json`,
    expectedFilePath: path.join(packageRoot, 'handoff-package-manifest.json'),
    errors,
  });
  validateEvidenceFileArtifact({
    artifactsByRole,
    role: 'archive',
    expectedPath: path.relative(deliveryRoot, archivePath).replaceAll('\\', '/'),
    expectedFilePath: archivePath,
    errors,
  });
  validateEvidenceFileArtifact({
    artifactsByRole,
    role: 'archive_sha256_sidecar',
    expectedPath: path.relative(deliveryRoot, sidecarPath).replaceAll('\\', '/'),
    expectedFilePath: sidecarPath,
    errors,
  });
  validateEvidenceFileArtifact({
    artifactsByRole,
    role: 'release_json',
    expectedPath: path.relative(deliveryRoot, releaseReportPath).replaceAll('\\', '/'),
    expectedFilePath: releaseReportPath,
    errors,
  });
  validateEvidenceFileArtifact({
    artifactsByRole,
    role: 'release_markdown',
    expectedPath: path.relative(deliveryRoot, releaseMarkdownPath).replaceAll('\\', '/'),
    expectedFilePath: releaseMarkdownPath,
    errors,
  });
  validateEvidenceFileArtifact({
    artifactsByRole,
    role: 'delivery_manifest',
    expectedPath: path.relative(deliveryRoot, deliveryManifestPath).replaceAll('\\', '/'),
    expectedFilePath: deliveryManifestPath,
    errors,
  });
  validateEvidenceFileArtifact({
    artifactsByRole,
    role: 'aggregate_json',
    expectedPath: path.relative(deliveryRoot, allReportPath).replaceAll('\\', '/'),
    expectedFilePath: allReportPath,
    errors,
  });
  validateEvidenceFileArtifact({
    artifactsByRole,
    role: 'aggregate_markdown',
    expectedPath: path.relative(deliveryRoot, allMarkdownPath).replaceAll('\\', '/'),
    expectedFilePath: allMarkdownPath,
    errors,
  });

  const releaseReport = fs.existsSync(releaseReportPath) ? parseJsonFile(releaseReportPath, errors, 'evidence_release_json') : null;
  if (releaseReport && releaseReport.release_ready !== true) {
    addError(errors, 'evidence_release_json_not_ready', 'release JSON must report release_ready=true', releaseReportPath);
  }

  const allReport = fs.existsSync(allReportPath) ? parseJsonFile(allReportPath, errors, 'evidence_aggregate_json') : null;
  if (allReport && allReport.all_ready !== true) {
    addError(errors, 'evidence_aggregate_json_not_ready', 'aggregate JSON must report all_ready=true', allReportPath);
  }
  if (allReport && allReport.summaries?.delivery?.delivery_manifest_ready !== true) {
    addError(errors, 'evidence_aggregate_delivery_not_ready', 'aggregate JSON must report delivery manifest ready', allReportPath);
  }
  if (allReport && allReport.summaries?.release?.release_ready !== true) {
    addError(errors, 'evidence_aggregate_release_not_ready', 'aggregate JSON must report release ready', allReportPath);
  }

  return {
    report_type: 'external_third_party_handoff_evidence_validation',
    evidence_manifest_ready: errors.length === 0,
    evidence_manifest_path: evidenceManifestPath,
    evidence_manifest_sha256: fileDigest(evidenceManifestPath).sha256,
    manifest_type: manifest.manifest_type || null,
    package_name: manifest.package_name || packageName,
    artifact_count: artifacts.length,
    error_codes: errors.map((error) => error.code),
    errors,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = validateEvidence({
    packageRootInput: args.package || '.',
    archivePathInput: args.archive || '',
    sidecarPathInput: args.sha256 || '',
    releaseReportPathInput: args.release || '',
    releaseMarkdownPathInput: args.releaseMarkdown || '',
    deliveryManifestInput: args.deliveryManifest || '',
    allReportPathInput: args.all || '',
    allMarkdownPathInput: args.allMarkdown || '',
    evidenceManifestInput: args.manifest || '',
  });
  console.log(JSON.stringify(result, null, 2));
  if (!result.evidence_manifest_ready) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export { EVIDENCE_MANIFEST_TYPE, validateEvidence };
