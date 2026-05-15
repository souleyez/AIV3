import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  normalizeHtmlArtifactManifest,
  renderHtmlArtifactDocument,
} from '../apps/web/app/lib/html-artifact-manifest.js';
import {
  thirdPartyHandoffHtmlArtifactManifest,
  writeThirdPartyHandoffHtmlArtifact,
} from './render-third-party-handoff-html-artifact.mjs';

test('third-party handoff HTML artifact manifest uses the safe V3 template contract', () => {
  const manifest = thirdPartyHandoffHtmlArtifactManifest({
    generatedAt: '2026-05-15T00:00:00.000Z',
    head: 'abc1234',
  });

  assert.equal(manifest.kind, 'html_artifact');
  assert.equal(manifest.source_type, 'external_integration');
  assert.equal(manifest.template_id, 'third_party_handoff_document');
  assert.equal(manifest.interaction_mode, 'read_only');
  assert.equal(manifest.owner_scope.type, 'external_integration_handoff');
  assert.equal(manifest.payload.handoff.defaultDomain, 'v3.elepcloud.com');
  assert.ok(manifest.payload.endpoints.some((endpoint) => endpoint.path === '/v1/external/channels/{connection_id}/events'));
  assert.ok(manifest.payload.deliveryArtifacts.some((artifact) => artifact.path === 'html-artifacts/third-party-handoff-document.json'));

  const normalized = normalizeHtmlArtifactManifest(manifest);
  assert.equal(normalized.rejected, false, normalized.reason);
  assert.equal(normalized.manifest.templateId, 'third_party_handoff_document');
  assert.equal(normalized.manifest.sourceType, 'external_integration');

  const rendered = renderHtmlArtifactDocument(manifest);
  assert.equal(rendered.rejected, false, rendered.reason);
  assert.match(rendered.html, /第三方交接文档/);
  assert.match(rendered.html, /v3\.elepcloud\.com/);
  assert.match(rendered.html, /POST \/v1\/external\/channels\/\{connection_id\}\/events/);
  assert.match(rendered.html, /安全 HTML artifact manifest/);
  assert.match(rendered.html, /npm run validate:all/);
  assert.match(rendered.html, /不搬迁整库/);
});

test('writeThirdPartyHandoffHtmlArtifact writes a stable JSON manifest file', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-third-party-html-artifact-'));
  const outputPath = path.join(outDir, 'html-artifacts/third-party-handoff-document.json');

  writeThirdPartyHandoffHtmlArtifact({
    outputPath,
    generatedAt: '2026-05-15T00:00:00.000Z',
    head: 'abc1234',
  });

  const manifest = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
  assert.equal(manifest.created_at, '2026-05-15T00:00:00.000Z');
  assert.equal(manifest.provenance.source_run_id, 'abc1234');
  assert.equal(manifest.payload.handoff.sourceDocument, 'docs/pure-third-party-integration-guide.zh-CN.md');
});
