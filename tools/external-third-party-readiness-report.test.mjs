import test from 'node:test';
import assert from 'node:assert/strict';
import { buildReadinessReport, renderReadinessMarkdown } from './external-third-party-readiness-report.mjs';

test('buildReadinessReport marks a signed gateway roundtrip ready', () => {
  const report = buildReadinessReport({
    generatedAt: '2026-05-14T00:00:00.000Z',
    gatewayUrl: 'http://127.0.0.1:43180',
    repository: '/srv/aiv3/repo',
    head: 'abc1234',
    requests: [
      {
        path: '/third-party/actions?tenant=tenant-ext-001',
        action_id: 'act-external-gateway-001',
        action_type: 'external_business_action.invoke',
        bearer_valid: true,
        signature_valid: true,
        body_hash_valid: true,
        requester_sender_present: true,
        raw_arguments_included: false,
        contains_forbidden_text: false,
      },
    ],
    callbacks: [
      {
        callback_status: 200,
        response_accepted: true,
        contains_forbidden_text: false,
        response_summary: {
          action_id: 'act-gateway-callback-001',
          status: 'succeeded',
          external_request_id: 'gateway-req-001',
        },
      },
    ],
  });

  assert.equal(report.ready_for_customer_sandbox, true);
  assert.equal(report.checks.length, 9);
  assert.ok(report.checks.every((check) => check.passed));
  assert.equal(report.dispatch_summary.signature_valid, true);
  assert.equal(report.callback_summary.status, 'succeeded');
  assert.ok(report.third_party_handoff_items.some((item) => item.includes('HTTPS dispatch endpoint')));
});

test('buildReadinessReport includes handoff manifest validation when provided', () => {
  const report = buildReadinessReport({
    requests: [
      {
        bearer_valid: true,
        signature_valid: true,
        body_hash_valid: true,
        requester_sender_present: true,
        raw_arguments_included: false,
        contains_forbidden_text: false,
      },
    ],
    callbacks: [
      {
        callback_status: 200,
        response_accepted: true,
        contains_forbidden_text: false,
      },
    ],
    handoffManifestPath: 'docs/integrations/third-party-handoff.sample.json',
    handoffManifestValidation: {
      manifest_version: 'v3.external_handoff.v1',
      ready_for_customer_sandbox: true,
      checks: [{ key: 'manifest_version', passed: true }],
      errors: [],
      warnings: [],
    },
  });

  assert.equal(report.ready_for_customer_sandbox, true);
  assert.equal(report.checks.find((check) => check.key === 'handoff_manifest_ready').passed, true);
  assert.equal(report.handoff_manifest_summary.manifest_version, 'v3.external_handoff.v1');
  assert.deepEqual(report.handoff_manifest_summary.error_codes, []);
});

test('buildReadinessReport fails closed when handoff manifest validation fails', () => {
  const report = buildReadinessReport({
    requests: [
      {
        bearer_valid: true,
        signature_valid: true,
        body_hash_valid: true,
        requester_sender_present: true,
        raw_arguments_included: false,
        contains_forbidden_text: false,
      },
    ],
    callbacks: [
      {
        callback_status: 200,
        response_accepted: true,
        contains_forbidden_text: false,
      },
    ],
    handoffManifestValidation: {
      manifest_version: 'v3.external_handoff.v1',
      ready_for_customer_sandbox: false,
      checks: [{ key: 'dispatch_auth', passed: false }],
      errors: [{ code: 'dispatch_auth_mode_missing' }],
      warnings: [],
    },
  });

  assert.equal(report.ready_for_customer_sandbox, false);
  assert.equal(report.checks.find((check) => check.key === 'handoff_manifest_ready').passed, false);
  assert.deepEqual(report.handoff_manifest_summary.error_codes, ['dispatch_auth_mode_missing']);
});

test('buildReadinessReport fails closed when signatures or redaction fail', () => {
  const report = buildReadinessReport({
    requests: [
      {
        bearer_valid: true,
        signature_valid: false,
        body_hash_valid: true,
        requester_sender_present: true,
        raw_arguments_included: false,
        contains_forbidden_text: true,
      },
    ],
    callbacks: [],
  });

  assert.equal(report.ready_for_customer_sandbox, false);
  assert.equal(report.checks.find((check) => check.key === 'dispatch_signature_valid').passed, false);
  assert.equal(report.checks.find((check) => check.key === 'dispatch_payload_redacted').passed, false);
  assert.equal(report.checks.find((check) => check.key === 'result_callback_received').passed, false);
});

test('renderReadinessMarkdown renders operator-facing checklist without raw payloads', () => {
  const report = buildReadinessReport({
    generatedAt: '2026-05-14T00:00:00.000Z',
    gatewayUrl: 'http://127.0.0.1:43180',
    repository: '/srv/aiv3/repo',
    head: 'abc1234',
    requests: [
      {
        action_id: 'act-001',
        action_type: 'external_artifact.publish',
        bearer_valid: true,
        signature_valid: true,
        body_hash_valid: true,
        requester_sender_present: true,
        raw_arguments_included: false,
        contains_forbidden_text: false,
      },
    ],
    callbacks: [
      {
        callback_status: 200,
        response_accepted: true,
        contains_forbidden_text: false,
        response_summary: { action_id: 'act-001', status: 'succeeded' },
      },
    ],
  });

  const markdown = renderReadinessMarkdown(report);

  assert.match(markdown, /Status: passed/);
  assert.match(markdown, /Handoff Manifest Summary/);
  assert.match(markdown, /Third-Party Handoff Items/);
  assert.doesNotMatch(markdown, /third-party-secret|raw prompt secret|callback-token-should-not-leak/);
});
