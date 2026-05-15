import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';
import { buildPackage, PACKAGE_TYPE, SOURCE_FILES } from './build-external-handoff-package.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function listTarEntries(tarGzPath) {
  const tar = zlib.gunzipSync(fs.readFileSync(tarGzPath));
  const entries = [];
  let offset = 0;
  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) {
      break;
    }
    const name = header.toString('utf8', 0, 100).replace(/\0.*$/, '');
    const sizeText = header.toString('ascii', 124, 136).replace(/\0.*$/, '').trim();
    const size = Number.parseInt(sizeText || '0', 8);
    entries.push(name);
    offset += 512 + Math.ceil(size / 512) * 512;
  }
  return entries;
}

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
  assert.ok(fs.existsSync(result.archivePath));
  assert.ok(fs.existsSync(result.archiveSha256Path));
  assert.ok(fs.existsSync(result.releaseReportPath));
  assert.ok(fs.existsSync(result.releaseMarkdownPath));
  assert.ok(fs.existsSync(result.deliveryManifestPath));
  assert.ok(fs.existsSync(result.allReportPath));
  assert.ok(fs.existsSync(result.allMarkdownPath));
  assert.ok(fs.existsSync(result.evidenceManifestPath));
  assert.match(result.archiveSha256, /^[a-f0-9]{64}$/);
  assert.match(result.releaseReportSha256, /^[a-f0-9]{64}$/);
  assert.match(result.releaseMarkdownSha256, /^[a-f0-9]{64}$/);
  assert.match(result.deliveryManifestSha256, /^[a-f0-9]{64}$/);
  assert.match(result.allReportSha256, /^[a-f0-9]{64}$/);
  assert.match(result.allMarkdownSha256, /^[a-f0-9]{64}$/);
  assert.match(result.evidenceManifestSha256, /^[a-f0-9]{64}$/);
  assert.equal(result.releaseReady, true);
  assert.equal(result.allReady, true);
  assert.equal(result.evidenceReady, true);
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'README.zh-CN.md')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'docs/third-party-integration-api.zh-CN.md')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'docs/pure-third-party-integration-guide.zh-CN.md')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'handoff/third-party-handoff.sample.json')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'tools/validate-external-handoff.mjs')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'tools/validate-external-handoff-package.mjs')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'tools/validate-external-handoff-archive.mjs')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'tools/validate-external-handoff-delivery.mjs')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'tools/validate-external-handoff-release.mjs')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'tools/validate-external-handoff-all.mjs')));
  assert.ok(fs.existsSync(path.join(result.packageRoot, 'sandbox/external-third-party-mock-gateway.mjs')));

  const cnGuide = fs.readFileSync(path.join(result.packageRoot, 'docs/third-party-integration-api.zh-CN.md'), 'utf8');
  const pureGuide = fs.readFileSync(path.join(result.packageRoot, 'docs/pure-third-party-integration-guide.zh-CN.md'), 'utf8');
  const enGuide = fs.readFileSync(path.join(result.packageRoot, 'docs/third-party-integration-api.md'), 'utf8');
  assert.match(cnGuide, /AI Data Platform V3/);
  assert.match(cnGuide, /当前不可见\/未供料/);
  assert.match(cnGuide, /不能声称已经联网搜索/);
  assert.match(pureGuide, /V3 纯第三方模式对接文档/);
  assert.match(pureGuide, /不需要把第三方服务器上的所有文档一次性拷贝到 V3/);
  assert.match(pureGuide, /POST \/v1\/external\/channels\/\{connection_id\}\/events/);
  assert.match(enGuide, /AI Data Platform V3/);
  assert.match(enGuide, /currently not visible or not supplied by V3/);
  assert.match(enGuide, /must not claim live search/);

  const manifest = JSON.parse(fs.readFileSync(result.manifestPath, 'utf8'));
  assert.equal(manifest.package_type, PACKAGE_TYPE);
  assert.equal(manifest.handoff_validation.ready_for_customer_sandbox, true);
  assert.equal(manifest.handoff_validation.error_codes.length, 0);
  assert.equal(manifest.included_files.length, result.fileCount);
  assert.ok(manifest.included_files.every((file) => /^[a-f0-9]{64}$/.test(file.sha256)));
  const releaseReport = JSON.parse(fs.readFileSync(result.releaseReportPath, 'utf8'));
  assert.equal(releaseReport.release_ready, true);
  assert.equal(releaseReport.generated_at, '2026-05-14T00:00:00.000Z');
  assert.equal(releaseReport.repository_head, manifest.repository_head);
  assert.equal(releaseReport.archive_sha256, result.archiveSha256);
  assert.equal(releaseReport.package_summary.included_file_count, result.fileCount);
  const releaseMarkdown = fs.readFileSync(result.releaseMarkdownPath, 'utf8');
  assert.match(releaseMarkdown, /Status: \*\*READY\*\*/);
  assert.match(releaseMarkdown, /Archive SHA256:/);
  const deliveryManifest = JSON.parse(fs.readFileSync(result.deliveryManifestPath, 'utf8'));
  assert.equal(deliveryManifest.manifest_type, 'v3.external_third_party_handoff_delivery_manifest.v1');
  assert.equal(deliveryManifest.package_type, PACKAGE_TYPE);
  assert.equal(deliveryManifest.generated_at, '2026-05-14T00:00:00.000Z');
  assert.equal(deliveryManifest.repository_head, manifest.repository_head);
  assert.equal(deliveryManifest.release_ready, true);
  assert.equal(deliveryManifest.package_name, 'package-under-test');
  const artifactsByRole = new Map(deliveryManifest.artifacts.map((artifact) => [artifact.role, artifact]));
  assert.equal(artifactsByRole.get('package_directory').path, 'package-under-test');
  assert.equal(artifactsByRole.get('archive').sha256, result.archiveSha256);
  assert.equal(artifactsByRole.get('archive').path, 'package-under-test.tar.gz');
  assert.equal(artifactsByRole.get('archive_sha256_sidecar').path, 'package-under-test.tar.gz.sha256');
  assert.equal(artifactsByRole.get('release_json').sha256, result.releaseReportSha256);
  assert.equal(artifactsByRole.get('release_markdown').sha256, result.releaseMarkdownSha256);
  assert.match(artifactsByRole.get('package_manifest').sha256, /^[a-f0-9]{64}$/);
  const allReport = JSON.parse(fs.readFileSync(result.allReportPath, 'utf8'));
  assert.equal(allReport.all_ready, true);
  assert.equal(allReport.summaries.delivery.delivery_manifest_ready, true);
  assert.equal(allReport.summaries.release.release_ready, true);
  const allMarkdown = fs.readFileSync(result.allMarkdownPath, 'utf8');
  assert.match(allMarkdown, /Status: \*\*READY\*\*/);
  assert.match(allMarkdown, /Delivery manifest ready: yes/);
  const evidenceManifest = JSON.parse(fs.readFileSync(result.evidenceManifestPath, 'utf8'));
  assert.equal(evidenceManifest.manifest_type, 'v3.external_third_party_handoff_evidence_manifest.v1');
  assert.equal(evidenceManifest.release_ready, true);
  assert.equal(evidenceManifest.all_ready, true);
  assert.equal(evidenceManifest.artifacts.length, 9);
  const evidenceArtifactsByRole = new Map(evidenceManifest.artifacts.map((artifact) => [artifact.role, artifact]));
  assert.equal(evidenceArtifactsByRole.get('delivery_manifest').sha256, result.deliveryManifestSha256);
  assert.equal(evidenceArtifactsByRole.get('aggregate_json').sha256, result.allReportSha256);
  assert.equal(evidenceArtifactsByRole.get('aggregate_markdown').sha256, result.allMarkdownSha256);

  const archiveEntries = listTarEntries(result.archivePath);
  assert.ok(archiveEntries.includes('package-under-test/README.zh-CN.md'));
  assert.ok(archiveEntries.includes('package-under-test/docs/pure-third-party-integration-guide.zh-CN.md'));
  assert.ok(archiveEntries.includes('package-under-test/handoff-package-manifest.json'));
  assert.ok(archiveEntries.includes('package-under-test/tools/validate-external-handoff-package.mjs'));
  assert.ok(archiveEntries.includes('package-under-test/tools/validate-external-handoff-archive.mjs'));
  assert.ok(archiveEntries.includes('package-under-test/tools/validate-external-handoff-delivery.mjs'));
  assert.ok(archiveEntries.includes('package-under-test/tools/validate-external-handoff-release.mjs'));
  assert.ok(archiveEntries.includes('package-under-test/tools/validate-external-handoff-all.mjs'));
  assert.equal(fs.readFileSync(result.archiveSha256Path, 'utf8').startsWith(result.archiveSha256), true);
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
  assert.equal(packageJson.scripts['validate:package'], 'node tools/validate-external-handoff-package.mjs --package .');
  assert.equal(packageJson.scripts['validate:archive'], 'node tools/validate-external-handoff-archive.mjs');
  assert.equal(packageJson.scripts['validate:delivery'], 'node tools/validate-external-handoff-delivery.mjs --package .');
  assert.equal(packageJson.scripts['validate:release'], 'node tools/validate-external-handoff-release.mjs --package .');
  assert.equal(packageJson.scripts['validate:all'], 'node tools/validate-external-handoff-all.mjs --package .');
  assert.equal(packageJson.scripts['validate:evidence'], 'node tools/validate-external-handoff-evidence.mjs --package .');
  assert.equal(packageJson.scripts['start:mock-gateway'], 'node sandbox/external-third-party-mock-gateway.mjs');
});
