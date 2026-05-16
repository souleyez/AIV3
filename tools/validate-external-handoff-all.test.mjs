import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage } from './build-external-handoff-package.mjs';
import {
  renderAllMarkdown,
  validateAll,
  validateAllMarkdownReceipt,
  writeAllReportFiles,
} from './validate-external-handoff-all.mjs';

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
  assert.equal(result.package_type, 'v3.external_third_party_handoff_package.v1');
  assert.equal(result.generated_at, '2026-05-15T00:00:00.000Z');
  assert.match(result.repository_head || '', /^[a-f0-9]+$/);
  assert.equal(result.checks.every((check) => check.passed), true);
  assert.equal(result.summaries.handoff.ready_for_customer_sandbox, true);
  assert.equal(result.summaries.package.package_ready, true);
  assert.equal(result.summaries.package.package_type, result.package_type);
  assert.equal(result.summaries.package.generated_at, result.generated_at);
  assert.equal(result.summaries.package.repository_head, result.repository_head);
  assert.equal(result.summaries.package.html_artifact_ready, true);
  assert.equal(result.summaries.package.html_artifact_template_id, 'third_party_handoff_document');
  assert.equal(result.summaries.package.html_artifact_command_contract_ready, true);
  assert.equal(result.summaries.package.html_artifact_validation_command_count, 6);
  assert.equal(result.summaries.archive.archive_ready, true);
  assert.equal(result.summaries.archive.html_artifact_ready, true);
  assert.equal(result.summaries.archive.html_artifact_template_id, 'third_party_handoff_document');
  assert.equal(result.summaries.archive.html_artifact_command_contract_ready, true);
  assert.equal(result.summaries.archive.html_artifact_validation_command_count, 6);
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

test('renderAllMarkdown summarizes aggregate validation without raw payloads', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-all-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'all-markdown-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });

  const markdown = renderAllMarkdown(validateAll({ packageRootInput: built.packageRoot }));

  assert.match(markdown, /Status: \*\*READY\*\*/);
  assert.match(markdown, /Package type: `v3\.external_third_party_handoff_package\.v1`/);
  assert.match(markdown, /Generated at: 2026-05-15T00:00:00\.000Z/);
  assert.match(markdown, /Repository head: `[a-f0-9]+`/);
  assert.match(markdown, /PASS `handoff_manifest_ready`/);
  assert.match(markdown, /HTML artifact ready: yes/);
  assert.match(markdown, /HTML command contract ready: yes/);
  assert.match(markdown, /Delivery manifest ready: yes/);
  assert.match(markdown, /Archive HTML artifact ready: yes/);
  assert.match(markdown, /Archive HTML command contract ready: yes/);
  assert.match(markdown, /Release ready: yes/);
  assert.doesNotMatch(markdown, /dispatch-token|dispatch-secret|Bearer /);
});

test('writeAllReportFiles writes JSON and Markdown evidence files', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-all-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'all-output-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const report = validateAll({ packageRootInput: built.packageRoot });
  const jsonPath = path.join(outDir, 'aggregate.json');
  const markdownPath = path.join(outDir, 'aggregate.md');

  const written = writeAllReportFiles(report, { out: jsonPath, markdown: markdownPath });

  assert.equal(written.jsonPath, jsonPath);
  assert.equal(written.markdownPath, markdownPath);
  assert.equal(JSON.parse(fs.readFileSync(jsonPath, 'utf8')).all_ready, true);
  assert.match(fs.readFileSync(markdownPath, 'utf8'), /External Third-Party Handoff Aggregate Validation/);
});

test('validateAllMarkdownReceipt rejects stale aggregate receipts', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-all-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'all-stale-receipt-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const report = validateAll({ packageRootInput: built.packageRoot });
  fs.appendFileSync(built.allMarkdownPath, '\nchanged after aggregate validation\n');

  const receipt = validateAllMarkdownReceipt({
    validation: report,
    markdownPathInput: built.allMarkdownPath,
  });

  assert.equal(receipt.receipt_ready, false);
  assert.ok(receipt.errors.some((error) => error.code === 'aggregate_markdown_receipt_bytes_mismatch'));
  assert.ok(receipt.errors.some((error) => error.code === 'aggregate_markdown_receipt_sha256_mismatch'));
});

test('CLI --verifyMarkdown auto verifies the sibling aggregate receipt', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-all-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'all-cli-auto-receipt-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });

  const stdout = execFileSync(
    process.execPath,
    [
      path.join(repoRoot, 'tools', 'validate-external-handoff-all.mjs'),
      '--package',
      built.packageRoot,
      '--verifyMarkdown',
      'auto',
    ],
    { encoding: 'utf8' },
  );
  const result = JSON.parse(stdout);

  assert.equal(result.all_ready, true);
  assert.equal(result.aggregate_markdown_receipt.receipt_ready, true);
  assert.equal(path.resolve(result.aggregate_markdown_receipt.receipt_path), built.allMarkdownPath);
  assert.ok(result.checks.some((check) => check.key === 'aggregate_markdown_receipt_ready' && check.passed));
  assert.deepEqual(result.errors, []);
});
