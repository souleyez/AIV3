import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage } from './build-external-handoff-package.mjs';
import { validatePackage } from './validate-external-handoff-package.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

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
  assert.equal(result.checks.find((check) => check.key === 'included_file_integrity').passed, true);
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
