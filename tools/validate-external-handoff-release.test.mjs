import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage } from './build-external-handoff-package.mjs';
import { renderReleaseMarkdown, validateRelease } from './validate-external-handoff-release.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('validateRelease accepts a generated package, archive, and sidecar', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-release-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'release-valid-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });

  const result = validateRelease({ packageRootInput: built.packageRoot });

  assert.equal(result.release_ready, true);
  assert.equal(result.generated_at, '2026-05-14T00:00:00.000Z');
  assert.match(result.repository_head || '', /^[a-f0-9]+$/);
  assert.equal(result.package_summary.generated_at, '2026-05-14T00:00:00.000Z');
  assert.equal(result.package_summary.package_ready, true);
  assert.equal(result.package_summary.html_artifact_ready, true);
  assert.equal(result.package_summary.html_artifact_template_id, 'third_party_handoff_document');
  assert.equal(result.package_summary.html_artifact_command_contract_ready, true);
  assert.equal(result.package_summary.html_artifact_validation_command_count, 6);
  assert.equal(result.archive_summary.archive_ready, true);
  assert.equal(result.archive_summary.html_artifact_ready, true);
  assert.equal(result.archive_summary.html_artifact_template_id, 'third_party_handoff_document');
  assert.equal(result.archive_summary.html_artifact_command_contract_ready, true);
  assert.equal(result.archive_summary.html_artifact_validation_command_count, 6);
  assert.equal(result.archive_summary.root_name, 'release-valid-package');
  assert.equal(result.archive_sha256, built.archiveSha256);
  assert.equal(result.delivery_manifest_summary.delivery_manifest_ready, true);
  assert.equal(result.delivery_manifest_summary.package_type, 'v3.external_third_party_handoff_package.v1');
  assert.equal(result.delivery_manifest_summary.generated_at, '2026-05-14T00:00:00.000Z');
  assert.equal(result.delivery_manifest_summary.repository_head, result.repository_head);
  assert.equal(result.delivery_manifest_summary.artifact_count, 6);
  assert.equal(result.delivery_manifest_sha256, built.deliveryManifestSha256);
  assert.equal(result.checks.find((check) => check.key === 'delivery_manifest_ready').passed, true);
  assert.equal(result.errors.length, 0);
  assert.equal(result.checks.find((check) => check.key === 'archive_root_matches_package').passed, true);
});

test('renderReleaseMarkdown summarizes a ready release for human review', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-release-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'release-markdown-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });

  const markdown = renderReleaseMarkdown(validateRelease({ packageRootInput: built.packageRoot }));

  assert.match(markdown, /Status: \*\*READY\*\*/);
  assert.match(markdown, /Generated at: 2026-05-14T00:00:00.000Z/);
  assert.match(markdown, /Repository head:/);
  assert.match(markdown, /Archive SHA256:/);
  assert.match(markdown, /Delivery generated at: 2026-05-14T00:00:00.000Z/);
  assert.match(markdown, /Delivery repository head:/);
  assert.match(markdown, /HTML artifact ready: yes/);
  assert.match(markdown, /HTML command contract ready: yes/);
  assert.match(markdown, /Archive HTML artifact ready: yes/);
  assert.match(markdown, /Archive HTML command contract ready: yes/);
  assert.match(markdown, /Delivery manifest ready: yes/);
  assert.match(markdown, /PASS `archive_root_matches_package`/);
  assert.match(markdown, /PASS `delivery_manifest_ready`/);
  assert.match(markdown, /## Errors/);
});

test('validateRelease rejects an archive that belongs to a different package directory', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-release-'));
  const packageUnderTest = buildPackage({
    repoRoot,
    outDir,
    basename: 'release-package-under-test',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });
  const otherPackage = buildPackage({
    repoRoot,
    outDir,
    basename: 'release-other-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });

  const result = validateRelease({
    packageRootInput: packageUnderTest.packageRoot,
    archivePathInput: otherPackage.archivePath,
    sidecarPathInput: otherPackage.archiveSha256Path,
  });

  assert.equal(result.release_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'archive_root_package_mismatch'));
});

test('validateRelease rejects tampered delivery manifest artifact checksums', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-release-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'release-delivery-tamper-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });
  const deliveryManifest = JSON.parse(fs.readFileSync(built.deliveryManifestPath, 'utf8'));
  const archiveArtifact = deliveryManifest.artifacts.find((artifact) => artifact.role === 'archive');
  archiveArtifact.sha256 = '0'.repeat(64);
  fs.writeFileSync(built.deliveryManifestPath, `${JSON.stringify(deliveryManifest, null, 2)}\n`);

  const result = validateRelease({ packageRootInput: built.packageRoot });

  assert.equal(result.release_ready, false);
  assert.equal(result.delivery_manifest_summary.delivery_manifest_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'delivery_archive_sha256_mismatch'));
});
