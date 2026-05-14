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
  assert.equal(result.package_summary.package_ready, true);
  assert.equal(result.archive_summary.archive_ready, true);
  assert.equal(result.archive_summary.root_name, 'release-valid-package');
  assert.equal(result.archive_sha256, built.archiveSha256);
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
  assert.match(markdown, /Archive SHA256:/);
  assert.match(markdown, /PASS `archive_root_matches_package`/);
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
