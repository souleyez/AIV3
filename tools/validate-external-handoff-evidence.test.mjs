import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage } from './build-external-handoff-package.mjs';
import { EVIDENCE_MANIFEST_TYPE, validateEvidence } from './validate-external-handoff-evidence.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('validateEvidence accepts generated final evidence manifests', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-valid-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });

  const result = validateEvidence({ packageRootInput: built.packageRoot });

  assert.equal(result.evidence_manifest_ready, true);
  assert.equal(result.manifest_type, EVIDENCE_MANIFEST_TYPE);
  assert.equal(result.package_name, 'evidence-valid-package');
  assert.equal(result.artifact_count, 9);
  assert.equal(result.evidence_manifest_sha256, built.evidenceManifestSha256);
  assert.deepEqual(result.error_codes, []);
});

test('validateEvidence rejects changed aggregate evidence artifacts', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-tampered-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  fs.appendFileSync(built.allMarkdownPath, '\nchanged after evidence manifest\n');

  const result = validateEvidence({ packageRootInput: built.packageRoot });

  assert.equal(result.evidence_manifest_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'evidence_aggregate_markdown_bytes_mismatch'));
  assert.ok(result.errors.some((error) => error.code === 'evidence_aggregate_markdown_sha256_mismatch'));
});

test('validateEvidence rejects not-ready aggregate JSON receipts', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-not-ready-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const allReport = JSON.parse(fs.readFileSync(built.allReportPath, 'utf8'));
  allReport.all_ready = false;
  fs.writeFileSync(built.allReportPath, `${JSON.stringify(allReport, null, 2)}\n`);

  const result = validateEvidence({ packageRootInput: built.packageRoot });

  assert.equal(result.evidence_manifest_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'evidence_aggregate_json_sha256_mismatch'));
  assert.ok(result.errors.some((error) => error.code === 'evidence_aggregate_json_not_ready'));
});
