import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage } from './build-external-handoff-package.mjs';
import { validateAll } from './validate-external-handoff-all.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('validateAll accepts a generated handoff package and siblings', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-all-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'all-valid-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });

  const result = validateAll({ packageRootInput: built.packageRoot });

  assert.equal(result.all_ready, true);
  assert.equal(result.checks.every((check) => check.passed), true);
  assert.equal(result.summaries.handoff.ready_for_customer_sandbox, true);
  assert.equal(result.summaries.package.package_ready, true);
  assert.equal(result.summaries.archive.archive_ready, true);
  assert.equal(result.summaries.delivery.delivery_manifest_ready, true);
  assert.equal(result.summaries.release.release_ready, true);
  assert.deepEqual(result.errors, []);
});

test('validateAll reports scoped failures when a sibling artifact changes', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-all-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'all-tampered-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  fs.appendFileSync(built.releaseMarkdownPath, '\nchanged after delivery manifest\n');

  const result = validateAll({ packageRootInput: built.packageRoot });

  assert.equal(result.all_ready, false);
  assert.equal(result.checks.find((check) => check.key === 'delivery_manifest_ready').passed, false);
  assert.equal(result.checks.find((check) => check.key === 'release_ready').passed, false);
  assert.ok(result.errors.some((error) => error.scope === 'delivery' && error.code === 'delivery_release_markdown_sha256_mismatch'));
  assert.ok(result.errors.some((error) => error.scope === 'release' && error.code === 'delivery_release_markdown_sha256_mismatch'));
});
