import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPackage } from './build-external-handoff-package.mjs';
import { validateDelivery } from './validate-external-handoff-delivery.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('validateDelivery accepts a generated delivery manifest and sibling artifacts', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-delivery-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'delivery-valid-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });

  const result = validateDelivery({ packageRootInput: built.packageRoot });

  assert.equal(result.delivery_manifest_ready, true);
  assert.equal(result.manifest_type, 'v3.external_third_party_handoff_delivery_manifest.v1');
  assert.equal(result.package_type, 'v3.external_third_party_handoff_package.v1');
  assert.equal(result.generated_at, '2026-05-15T00:00:00.000Z');
  assert.match(result.repository_head || '', /^[a-f0-9]+$/);
  assert.equal(result.package_name, 'delivery-valid-package');
  assert.equal(result.artifact_count, 6);
  assert.equal(result.delivery_manifest_sha256, built.deliveryManifestSha256);
  assert.deepEqual(result.error_codes, []);
});

test('validateDelivery rejects changed sibling release artifacts', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-delivery-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'delivery-tampered-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  fs.appendFileSync(built.releaseMarkdownPath, '\nchanged after delivery manifest\n');

  const result = validateDelivery({ packageRootInput: built.packageRoot });

  assert.equal(result.delivery_manifest_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'delivery_release_markdown_bytes_mismatch'));
  assert.ok(result.errors.some((error) => error.code === 'delivery_release_markdown_sha256_mismatch'));
});

test('validateDelivery rejects delivery provenance that does not match package and release manifests', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-delivery-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'delivery-provenance-mismatch-package',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  const deliveryManifest = JSON.parse(fs.readFileSync(built.deliveryManifestPath, 'utf8'));
  deliveryManifest.generated_at = '2026-05-16T00:00:00.000Z';
  deliveryManifest.repository_head = deliveryManifest.repository_head === '0000000' ? '1111111' : '0000000';
  fs.writeFileSync(built.deliveryManifestPath, `${JSON.stringify(deliveryManifest, null, 2)}\n`);

  const result = validateDelivery({ packageRootInput: built.packageRoot });

  assert.equal(result.delivery_manifest_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'delivery_package_manifest_generated_at_mismatch'));
  assert.ok(result.errors.some((error) => error.code === 'delivery_package_manifest_repository_head_mismatch'));
  assert.ok(result.errors.some((error) => error.code === 'delivery_release_json_generated_at_mismatch'));
  assert.ok(result.errors.some((error) => error.code === 'delivery_release_json_repository_head_mismatch'));
});
