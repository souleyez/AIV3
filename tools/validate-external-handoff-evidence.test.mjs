import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage } from './build-external-handoff-package.mjs';
import {
  EVIDENCE_MANIFEST_TYPE,
  renderEvidenceMarkdown,
  validateEvidence,
  validateEvidenceMarkdownReceipt,
  writeEvidenceReportFiles,
} from './validate-external-handoff-evidence.mjs';

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
  assert.equal(result.package_type, 'v3.external_third_party_handoff_package.v1');
  assert.equal(result.generated_at, '2026-05-15T00:00:00.000Z');
  assert.match(result.repository_head || '', /^[a-f0-9]+$/);
  assert.equal(result.package_name, 'evidence-valid-package');
  assert.equal(result.artifact_count, 9);
  assert.equal(result.evidence_manifest_sha256, built.evidenceManifestSha256);
  assert.equal(result.html_artifact_summary.package_ready, true);
  assert.equal(result.html_artifact_summary.package_template_id, 'third_party_handoff_document');
  assert.equal(result.html_artifact_summary.archive_ready, true);
  assert.equal(result.html_artifact_summary.archive_template_id, 'third_party_handoff_document');
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

test('validateEvidence rejects final evidence manifests for the wrong package type', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-wrong-package-type',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const manifest = JSON.parse(fs.readFileSync(built.evidenceManifestPath, 'utf8'));
  manifest.package_type = 'v3.other_package.v1';
  fs.writeFileSync(built.evidenceManifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

  const result = validateEvidence({ packageRootInput: built.packageRoot });

  assert.equal(result.evidence_manifest_ready, false);
  assert.equal(result.package_type, 'v3.other_package.v1');
  assert.ok(result.errors.some((error) => error.code === 'evidence_package_type_invalid'));
});

test('validateEvidence rejects final evidence manifests without provenance fields', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-missing-provenance',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const manifest = JSON.parse(fs.readFileSync(built.evidenceManifestPath, 'utf8'));
  delete manifest.generated_at;
  delete manifest.repository_head;
  fs.writeFileSync(built.evidenceManifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

  const result = validateEvidence({ packageRootInput: built.packageRoot });

  assert.equal(result.evidence_manifest_ready, false);
  assert.equal(result.generated_at, null);
  assert.equal(result.repository_head, null);
  assert.ok(result.errors.some((error) => error.code === 'evidence_generated_at_missing'));
  assert.ok(result.errors.some((error) => error.code === 'evidence_repository_head_missing'));
});

test('validateEvidence rejects final evidence manifests with invalid provenance fields', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-invalid-provenance',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const manifest = JSON.parse(fs.readFileSync(built.evidenceManifestPath, 'utf8'));
  manifest.generated_at = '2026-05-15 00:00:00';
  manifest.repository_head = 'not-a-git-head';
  fs.writeFileSync(built.evidenceManifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

  const result = validateEvidence({ packageRootInput: built.packageRoot });

  assert.equal(result.evidence_manifest_ready, false);
  assert.equal(result.generated_at, '2026-05-15 00:00:00');
  assert.equal(result.repository_head, 'not-a-git-head');
  assert.ok(result.errors.some((error) => error.code === 'evidence_generated_at_invalid'));
  assert.ok(result.errors.some((error) => error.code === 'evidence_repository_head_invalid'));
});

test('validateEvidence rejects aggregate JSON receipts without HTML artifact readiness', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-html-artifact-not-ready-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const allReport = JSON.parse(fs.readFileSync(built.allReportPath, 'utf8'));
  allReport.summaries.package.html_artifact_ready = false;
  delete allReport.summaries.archive.html_artifact_ready;
  fs.writeFileSync(built.allReportPath, `${JSON.stringify(allReport, null, 2)}\n`);

  const result = validateEvidence({ packageRootInput: built.packageRoot });

  assert.equal(result.evidence_manifest_ready, false);
  assert.equal(result.html_artifact_summary.package_ready, false);
  assert.equal(result.html_artifact_summary.archive_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'evidence_aggregate_json_sha256_mismatch'));
  assert.ok(result.errors.some((error) => error.code === 'evidence_aggregate_package_html_artifact_not_ready'));
  assert.ok(result.errors.some((error) => error.code === 'evidence_aggregate_archive_html_artifact_not_ready'));
});

test('renderEvidenceMarkdown summarizes final evidence without raw payloads', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-markdown-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const result = validateEvidence({ packageRootInput: built.packageRoot });
  const markdown = renderEvidenceMarkdown(result);

  assert.match(markdown, /Status: \*\*READY\*\*/);
  assert.match(markdown, /Manifest SHA256:/);
  assert.match(markdown, /Package type: `v3\.external_third_party_handoff_package\.v1`/);
  assert.match(markdown, /Generated at: 2026-05-15T00:00:00\.000Z/);
  assert.match(markdown, /Repository head: `[a-f0-9]+`/);
  assert.match(markdown, /Package HTML artifact ready: yes/);
  assert.match(markdown, /Archive HTML artifact ready: yes/);
  assert.doesNotMatch(markdown, /third-party-secret|raw prompt secret|callback-token-should-not-leak/);
});

test('validateEvidenceMarkdownReceipt rejects stale final evidence receipts', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-stale-receipt-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const result = validateEvidence({ packageRootInput: built.packageRoot });
  fs.appendFileSync(built.evidenceMarkdownPath, '\nchanged after evidence validation\n');

  const receipt = validateEvidenceMarkdownReceipt({
    validation: result,
    markdownPathInput: built.evidenceMarkdownPath,
  });

  assert.equal(receipt.receipt_ready, false);
  assert.ok(receipt.errors.some((error) => error.code === 'evidence_markdown_receipt_bytes_mismatch'));
  assert.ok(receipt.errors.some((error) => error.code === 'evidence_markdown_receipt_sha256_mismatch'));
});

test('CLI --verifyMarkdown auto verifies the sibling final evidence receipt', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-cli-auto-receipt-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });

  const stdout = execFileSync(
    process.execPath,
    [
      path.join(repoRoot, 'tools', 'validate-external-handoff-evidence.mjs'),
      '--package',
      built.packageRoot,
      '--verifyMarkdown',
      'auto',
    ],
    { encoding: 'utf8' },
  );
  const result = JSON.parse(stdout);

  assert.equal(result.evidence_manifest_ready, true);
  assert.equal(result.evidence_markdown_receipt.receipt_ready, true);
  assert.equal(path.resolve(result.evidence_markdown_receipt.receipt_path), built.evidenceMarkdownPath);
  assert.deepEqual(result.error_codes, []);
});

test('writeEvidenceReportFiles writes JSON and Markdown validation receipts', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-evidence-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'evidence-write-files-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const result = validateEvidence({ packageRootInput: built.packageRoot });
  const jsonPath = path.join(outDir, 'evidence-validation.json');
  const markdownPath = path.join(outDir, 'evidence-validation.md');
  const written = writeEvidenceReportFiles(result, {
    out: jsonPath,
    markdown: markdownPath,
  });

  assert.equal(written.jsonPath, jsonPath);
  assert.equal(written.markdownPath, markdownPath);
  assert.equal(JSON.parse(fs.readFileSync(jsonPath, 'utf8')).evidence_manifest_ready, true);
  assert.match(fs.readFileSync(markdownPath, 'utf8'), /External Third-Party Handoff Final Evidence/);
});
