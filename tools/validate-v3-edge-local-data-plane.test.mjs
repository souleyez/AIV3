import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { execFileSync } from 'node:child_process';
import { validateV3EdgeLocalDataPlaneManifest } from './validate-v3-edge-local-data-plane.mjs';

const samplePath = path.resolve('docs/integrations/v3-edge-local-data-plane.sample.json');

function readSample() {
  return JSON.parse(fs.readFileSync(samplePath, 'utf8'));
}

test('sample V3 Edge local data plane manifest is valid', () => {
  const result = validateV3EdgeLocalDataPlaneManifest(readSample());
  assert.equal(result.ok, true);
  assert.deepEqual(result.errors, []);
});

test('validator rejects direct browser V3 calls and token exposure', () => {
  const manifest = readSample();
  manifest.edge_agent.browser_calls_v3_directly = true;
  manifest.edge_agent.exposes_browser_token = true;

  const result = validateV3EdgeLocalDataPlaneManifest(manifest);
  assert.equal(result.ok, false);
  assert(result.errors.some((error) => error.code === 'browser_direct_v3_forbidden'));
  assert(result.errors.some((error) => error.code === 'browser_token_exposure_forbidden'));
});

test('validator rejects V3 storage of local data plane source material', () => {
  const manifest = readSample();
  manifest.v3_control_plane.stores_source_documents = true;
  manifest.v3_control_plane.allowed_data_classes.push('raw_document');
  manifest.page_generation.v3_receives_html_body = true;

  const result = validateV3EdgeLocalDataPlaneManifest(manifest);
  assert.equal(result.ok, false);
  assert(result.errors.some((error) => error.code === 'v3_storage_boundary_invalid'));
  assert(result.errors.some((error) => error.code === 'v3_allowed_data_class_forbidden'));
  assert(result.errors.some((error) => error.code === 'html_body_to_v3_forbidden'));
});

test('validator rejects raw secret material in parameter card', () => {
  const manifest = readSample();
  manifest.security.token = 'Bearer should-not-be-here';

  const result = validateV3EdgeLocalDataPlaneManifest(manifest);
  assert.equal(result.ok, false);
  assert(result.errors.some((error) => error.code === 'raw_secret_material_present'));
});

test('validator supports local retrieval with V3 generation only when boundary is explicit', () => {
  const manifest = readSample();
  manifest.environment.execution_profile = 'local_retrieval_v3_generation';
  manifest.capability_contracts.model_routing.execution_owner = 'v3';
  manifest.capability_contracts.model_routing.approved_generation_boundary = 'permission_filtered_evidence_to_v3';
  manifest.capability_contracts.retrieval.evidence_snippets_sent_to_v3 = true;
  manifest.edge_agent.capabilities.local_answer_generation = false;

  const result = validateV3EdgeLocalDataPlaneManifest(manifest);
  assert.equal(result.ok, true);
});

test('CLI exits successfully for the checked-in sample', () => {
  const output = execFileSync(
    process.execPath,
    ['tools/validate-v3-edge-local-data-plane.mjs', '--manifest', samplePath],
    { encoding: 'utf8' },
  );
  const result = JSON.parse(output);
  assert.equal(result.ok, true);
});

test('CLI exits non-zero for an invalid manifest', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-edge-local-data-plane-'));
  const invalidPath = path.join(tempDir, 'invalid.json');
  const manifest = readSample();
  manifest.security.password = 'secret';
  fs.writeFileSync(invalidPath, JSON.stringify(manifest, null, 2));

  assert.throws(
    () =>
      execFileSync(process.execPath, ['tools/validate-v3-edge-local-data-plane.mjs', '--manifest', invalidPath], {
        encoding: 'utf8',
      }),
    /Command failed/,
  );
});
