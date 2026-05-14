import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage, PACKAGE_TYPE, SOURCE_FILES } from './build-external-handoff-package.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('buildPackage creates a third-party handoff directory with manifest and tooling', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-package-'));
  const result = buildPackage({
    repoRoot,
    outDir,
    basename: 'package-under-test',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });

  assert.equal(result.ready, true);
  assert.equal(result.fileCount, SOURCE_FILES.length + 3);
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'README.zh-CN.md')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'docs/third-party-integration-api.zh-CN.md')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'handoff/third-party-handoff.sample.json')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'tools/validate-external-handoff.mjs')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'sandbox/external-third-party-mock-gateway.mjs')));

  const manifest = JSON.parse(fs.readFileSync(result.manifestPath, 'utf8'));
  assert.equal(manifest.package_type, PACKAGE_TYPE);
  assert.equal(manifest.handoff_validation.ready_for_customer_sandbox, true);
  assert.equal(manifest.handoff_validation.error_codes.length, 0);
  assert.equal(manifest.included_files.length, result.fileCount);
  assert.ok(manifest.included_files.every((file) => /^[a-f0-9]{64}$/.test(file.sha256)));
});

test('generated package exposes simple npm scripts for third parties', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-package-'));
  const result = buildPackage({
    repoRoot,
    outDir,
    basename: 'package-scripts-under-test',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });
  const packageJson = JSON.parse(fs.readFileSync(path.join(result.packageRoot, 'package.json'), 'utf8'));

  assert.equal(packageJson.scripts['validate:handoff'], 'node tools/validate-external-handoff.mjs --manifest handoff/third-party-handoff.sample.json');
  assert.equal(packageJson.scripts['start:mock-gateway'], 'node sandbox/external-third-party-mock-gateway.mjs');
});
