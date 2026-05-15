import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage } from './build-external-handoff-package.mjs';
import { validatePackage } from './validate-external-handoff-package.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function sha256Hex(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

function refreshPackageManifestDigest(packageRoot, relativePath) {
  const manifestPath = path.join(packageRoot, 'handoff-package-manifest.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const target = manifest.included_files.find((file) => file.path === relativePath);
  assert.ok(target, `${relativePath} should be listed in package manifest`);
  const bytes = fs.readFileSync(path.join(packageRoot, relativePath));
  target.bytes = bytes.length;
  target.sha256 = sha256Hex(bytes);
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}

test('validatePackage accepts a generated package', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-package-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'valid-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });

  const result = validatePackage(built.packageRoot);

  assert.equal(result.package_ready, true);
  assert.equal(result.generated_at, '2026-05-14T00:00:00.000Z');
  assert.match(result.repository_head || '', /^[a-f0-9]+$/);
  assert.equal(result.checks.find((check) => check.key === 'included_file_integrity').passed, true);
  assert.equal(result.checks.find((check) => check.key === 'third_party_html_artifact_manifest').passed, true);
  assert.equal(result.html_artifact_validation.artifact_ready, true);
  assert.equal(result.html_artifact_validation.template_id, 'third_party_handoff_document');
  assert.equal(result.html_artifact_validation.source_type, 'external_integration');
  assert.equal(result.html_artifact_validation.endpoint_count > 0, true);
  assert.equal(result.handoff_validation.ready_for_customer_sandbox, true);
  assert.equal(result.errors.length, 0);
});

test('validatePackage rejects changed files', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-package-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'tampered-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });
  fs.appendFileSync(path.join(built.packageRoot, 'README.md'), '\nchanged after manifest\n');

  const result = validatePackage(built.packageRoot);

  assert.equal(result.package_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'included_file_size_mismatch'));
  assert.ok(result.errors.some((error) => error.code === 'included_file_sha256_mismatch'));
});

test('validatePackage rejects unsafe third-party handoff HTML artifact manifests', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-package-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'unsafe-html-artifact-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });
  const relativePath = 'html-artifacts/third-party-handoff-document.json';
  const artifactPath = path.join(built.packageRoot, relativePath);
  const artifact = JSON.parse(fs.readFileSync(artifactPath, 'utf8'));
  artifact.template_id = 'codex_execution_report';
  artifact.payload.summary = 'Read the latest source at https://example.com/internal';
  fs.writeFileSync(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`);
  refreshPackageManifestDigest(built.packageRoot, relativePath);

  const result = validatePackage(built.packageRoot);

  assert.equal(result.package_ready, false);
  assert.equal(result.checks.find((check) => check.key === 'included_file_integrity').passed, true);
  assert.equal(result.checks.find((check) => check.key === 'third_party_html_artifact_manifest').passed, false);
  assert.equal(result.html_artifact_validation.artifact_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'html_artifact_template_invalid'));
  assert.ok(result.errors.some((error) => error.code === 'html_artifact_payload_unsafe'));
});

test('validatePackage requires final evidence validation in the HTML artifact manifest', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-package-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'missing-evidence-command-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });
  const relativePath = 'html-artifacts/third-party-handoff-document.json';
  const artifactPath = path.join(built.packageRoot, relativePath);
  const artifact = JSON.parse(fs.readFileSync(artifactPath, 'utf8'));
  artifact.payload.validationCommands = artifact.payload.validationCommands.filter(
    (command) => command.command !== 'npm run validate:evidence',
  );
  fs.writeFileSync(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`);
  refreshPackageManifestDigest(built.packageRoot, relativePath);

  const result = validatePackage(built.packageRoot);

  assert.equal(result.package_ready, false);
  assert.equal(result.checks.find((check) => check.key === 'included_file_integrity').passed, true);
  assert.equal(result.checks.find((check) => check.key === 'third_party_html_artifact_manifest').passed, false);
  assert.equal(result.html_artifact_validation.artifact_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'html_artifact_evidence_validation_missing'));
});

test('validatePackage rejects path escapes in package manifest', () => {
  const packageRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-package-'));
  fs.mkdirSync(path.join(packageRoot, 'handoff'), { recursive: true });
  fs.writeFileSync(path.join(packageRoot, 'handoff', 'third-party-handoff.sample.json'), '{}\n');
  fs.writeFileSync(path.join(packageRoot, 'handoff-package-manifest.json'), JSON.stringify({
    package_type: 'v3.external_third_party_handoff_package.v1',
    package_root: '.',
    included_files: [
      {
        path: '../outside.txt',
        bytes: 0,
        sha256: '0'.repeat(64),
      },
    ],
  }, null, 2));

  const result = validatePackage(packageRoot);

  assert.equal(result.package_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'included_file_path_invalid'));
});
