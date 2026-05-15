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
  if (allReport && allReport.summaries?.package?.html_artifact_ready !== true) {
    addError(errors, 'evidence_aggregate_package_html_artifact_not_ready', 'aggregate JSON must report package HTML artifact ready', allReportPath);
  }
  if (allReport && allReport.summaries?.archive?.html_artifact_ready !== true) {
    addError(errors, 'evidence_aggregate_archive_html_artifact_not_ready', 'aggregate JSON must report archived HTML artifact ready', allReportPath);
  }

  return {
    report_type: 'external_third_party_handoff_evidence_validation',
    evidence_manifest_ready: errors.length === 0,
    evidence_manifest_path: evidenceManifestPath,
    evidence_manifest_sha256: fileDigest(evidenceManifestPath).sha256,
    manifest_type: manifest.manifest_type || null,
    package_name: manifest.package_name || packageName,
    artifact_count: artifacts.length,
    html_artifact_summary: {
      package_ready: allReport?.summaries?.package?.html_artifact_ready === true,
      package_template_id: allReport?.summaries?.package?.html_artifact_template_id || null,
      archive_ready: allReport?.summaries?.archive?.html_artifact_ready === true,
      archive_template_id: allReport?.summaries?.archive?.html_artifact_template_id || null,
    },
    error_codes: errors.map((error) => error.code),
    errors,
  };
}

function renderEvidenceMarkdown(report) {
  const status = report.evidence_manifest_ready ? 'READY' : 'NOT READY';
  const htmlArtifactSummary = report.html_artifact_summary || {};
  const errors = (report.errors || []).length
    ? report.errors.map((error) => `- \`${error.code}\`: ${error.message}${error.path ? ` (${error.path})` : ''}`).join('\n')
    : '- None';

  return `# External Third-Party Handoff Final Evidence

Status: **${status}**

## Evidence Manifest

- Manifest path: \`${report.evidence_manifest_path || 'unknown'}\`
- Manifest SHA256: \`${report.evidence_manifest_sha256 || 'unknown'}\`
- Manifest type: \`${report.manifest_type || 'unknown'}\`
- Package name: \`${report.package_name || 'unknown'}\`
- Final artifacts: ${report.artifact_count || 0}

## HTML Artifact Readiness

- Package HTML artifact ready: ${htmlArtifactSummary.package_ready ? 'yes' : 'no'}
- Package HTML artifact template: \`${htmlArtifactSummary.package_template_id || 'unknown'}\`
- Archive HTML artifact ready: ${htmlArtifactSummary.archive_ready ? 'yes' : 'no'}
- Archive HTML artifact template: \`${htmlArtifactSummary.archive_template_id || 'unknown'}\`

## Errors

${errors}
`;
}

function validateEvidenceMarkdownReceipt({ validation, markdownPathInput = '' } = {}) {
  const errors = [];
  const evidenceManifestPath = validation?.evidence_manifest_path || '';
  const markdownPath = path.resolve(
    markdownPathInput || (evidenceManifestPath ? evidenceManifestPath.replace(/\.json$/u, '.md') : 'evidence-manifest.md'),
  );
  const expected = renderEvidenceMarkdown(validation || {});
  const expectedDigest = {
    bytes: Buffer.byteLength(expected),
    sha256: sha256Hex(Buffer.from(expected)),
  };

  if (!fs.existsSync(markdownPath)) {
    addError(errors, 'evidence_markdown_receipt_missing', 'evidence markdown receipt is missing', markdownPath);
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
    addError(errors, 'evidence_markdown_receipt_bytes_mismatch', 'evidence markdown receipt byte size does not match validation output', markdownPath);
  }
  if (actualDigest.sha256 !== expectedDigest.sha256) {
    addError(errors, 'evidence_markdown_receipt_sha256_mismatch', 'evidence markdown receipt SHA256 does not match validation output', markdownPath);
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

function writeOutputFile(filePath, contents) {
  if (!filePath) {
    return '';
  }
  const resolved = path.resolve(filePath);
  fs.mkdirSync(path.dirname(resolved), { recursive: true });
  fs.writeFileSync(resolved, contents);
  return resolved;
}

function writeEvidenceReportFiles(report, { out = '', markdown = '' } = {}) {
  return {
    jsonPath: writeOutputFile(out, `${JSON.stringify(report, null, 2)}\n`),
    markdownPath: writeOutputFile(markdown, renderEvidenceMarkdown(report)),
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  let result = validateEvidence({
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
  if (args.verifyMarkdown) {
    const verifyMarkdownPath = args.verifyMarkdown === 'auto' ? '' : args.verifyMarkdown;
    const receipt = validateEvidenceMarkdownReceipt({
      validation: result,
      markdownPathInput: verifyMarkdownPath,
    });
    result = {
      ...result,
      evidence_manifest_ready: result.evidence_manifest_ready === true && receipt.receipt_ready === true,
      evidence_markdown_receipt: receipt,
      error_codes: [...result.error_codes, ...receipt.error_codes],
      errors: [...result.errors, ...receipt.errors],
    };
  }
  writeEvidenceReportFiles(result, {
    out: args.out || '',
    markdown: args.markdown || '',
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

export {
  EVIDENCE_MANIFEST_TYPE,
  renderEvidenceMarkdown,
  validateEvidence,
  validateEvidenceMarkdownReceipt,
  writeEvidenceReportFiles,
};
