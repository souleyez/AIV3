import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import { validateThirdPartySourceParameterCard } from './validate-third-party-source-parameter-card.mjs';
import { startMockThirdPartySourceGateway } from './mock-third-party-source-gateway.mjs';
import { runThirdPartySourceGatewaySmoke } from './smoke-third-party-source-gateway.mjs';

const sampleCard = JSON.parse(fs.readFileSync(new URL('../docs/integrations/third-party-source-parameter-card.sample.json', import.meta.url), 'utf8'));

test('sample third-party source parameter card is ready', () => {
  const report = validateThirdPartySourceParameterCard(sampleCard, {
    manifestPath: 'docs/integrations/third-party-source-parameter-card.sample.json',
  });

  assert.equal(report.parameter_card_ready, true);
  assert.equal(report.summary.user_fixture_count, 3);
  assert.equal(report.summary.document_fixture_count, 5);
  assert.equal(report.summary.access_case_count, 3);
  assert.deepEqual(report.errors, []);
});

test('parameter card rejects raw secrets and insecure URLs', () => {
  const unsafe = structuredClone(sampleCard);
  unsafe.environment.base_url = 'http://customer.example.com';
  unsafe.auth.raw_token = 'Bearer abcdefghijklmnopqrstuvwxyz';

  const report = validateThirdPartySourceParameterCard(unsafe);

  assert.equal(report.parameter_card_ready, false);
  assert.ok(report.errors.some((error) => error.code === 'environment_base_url_insecure'));
  assert.ok(report.errors.some((error) => error.code === 'raw_secret_material'));
});

test('parameter card rejects insufficient fixtures and broken access cases', () => {
  const invalid = structuredClone(sampleCard);
  invalid.fixtures.users = invalid.fixtures.users.slice(0, 2);
  invalid.fixtures.documents = invalid.fixtures.documents.slice(0, 4);
  invalid.fixtures.access_cases = [{
    case_id: 'missing-document',
    user_external_id: 'user-10001',
    document_external_id: 'doc-missing',
    expected: 'allow',
  }];

  const report = validateThirdPartySourceParameterCard(invalid);

  assert.equal(report.parameter_card_ready, false);
  assert.ok(report.errors.some((error) => error.code === 'fixture_users_insufficient'));
  assert.ok(report.errors.some((error) => error.code === 'fixture_documents_insufficient'));
  assert.ok(report.errors.some((error) => error.code === 'access_cases_invalid'));
});

test('mock third-party source gateway serves protected documents and ACL smoke', async () => {
  const { server, baseUrl, token } = await startMockThirdPartySourceGateway({ port: 0, token: 'mock-source-token' });
  try {
    const report = await runThirdPartySourceGatewaySmoke({ baseUrl, token });

    assert.equal(report.ready, true);
    assert.equal(report.summary.user_count, 3);
    assert.equal(report.summary.document_count, 5);
    assert.ok(report.checks.every((check) => check.passed));
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test('validator CLI exits successfully for checked-in sample', () => {
  const result = spawnSync(process.execPath, [
    'tools/validate-third-party-source-parameter-card.mjs',
    '--manifest',
    'docs/integrations/third-party-source-parameter-card.sample.json',
  ], {
    cwd: new URL('..', import.meta.url),
    encoding: 'utf8',
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /"parameter_card_ready": true/);
});
