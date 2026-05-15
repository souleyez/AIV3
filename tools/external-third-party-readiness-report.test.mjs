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

test('buildReadinessReport includes handoff release validation when provided', () => {
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
    releasePackagePath: 'target/external-third-party-handoff/package',
    releaseValidation: {
      release_ready: true,
      package_type: 'v3.external_third_party_handoff_package.v1',
      generated_at: '2026-05-15T00:00:00.000Z',
      repository_head: 'abc1234',
      archive_sha256: 'a'.repeat(64),
      delivery_manifest_sha256: 'b'.repeat(64),
      checks: [{ key: 'delivery_manifest_ready', passed: true }],
      errors: [],
      package_summary: { error_codes: [] },
      archive_summary: { error_codes: [] },
      delivery_manifest_summary: { error_codes: [] },
    },
  });

  assert.equal(report.ready_for_customer_sandbox, true);
  assert.equal(report.checks.find((check) => check.key === 'handoff_release_ready').passed, true);
  assert.equal(report.handoff_release_summary.release_ready, true);
  assert.equal(report.handoff_release_summary.delivery_manifest_sha256, 'b'.repeat(64));
});

test('buildReadinessReport includes aggregate handoff validation when provided', () => {
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
    releasePackagePath: 'target/external-third-party-handoff/package',
    handoffAllValidation: {
      all_ready: true,
      checks: [
        { key: 'handoff_manifest_ready', passed: true },
        { key: 'release_ready', passed: true },
      ],
      summaries: {
        package: {
          html_artifact_ready: true,
          html_artifact_template_id: 'third_party_handoff_document',
          error_codes: [],
        },
        archive: {
          html_artifact_ready: true,
          html_artifact_template_id: 'third_party_handoff_document',
          error_codes: [],
        },
        delivery: { error_codes: [] },
        release: { error_codes: [] },
      },
      errors: [],
    },
  });

  assert.equal(report.ready_for_customer_sandbox, true);
  assert.equal(report.checks.find((check) => check.key === 'handoff_all_ready').passed, true);
  assert.equal(report.handoff_all_summary.all_ready, true);
  assert.equal(report.handoff_all_summary.package_html_artifact_ready, true);
  assert.equal(report.handoff_all_summary.package_html_artifact_template_id, 'third_party_handoff_document');
  assert.equal(report.handoff_all_summary.archive_html_artifact_ready, true);
  assert.equal(report.handoff_all_summary.archive_html_artifact_template_id, 'third_party_handoff_document');
  assert.deepEqual(report.handoff_all_summary.error_codes, []);
});

test('buildReadinessReport includes final evidence manifest validation when provided', () => {
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
    releasePackagePath: 'target/external-third-party-handoff/package',
    handoffEvidenceValidation: {
      evidence_manifest_ready: true,
      evidence_manifest_path: 'target/external-third-party-handoff/package.evidence-manifest.json',
      evidence_manifest_sha256: 'c'.repeat(64),
      manifest_type: 'v3.external_third_party_handoff_evidence_manifest.v1',
      package_name: 'package',
      artifact_count: 9,
      html_artifact_summary: {
        package_ready: true,
        package_template_id: 'third_party_handoff_document',
        archive_ready: true,
        archive_template_id: 'third_party_handoff_document',
      },
      errors: [],
    },
  });

  assert.equal(report.ready_for_customer_sandbox, true);
  assert.equal(report.checks.find((check) => check.key === 'handoff_evidence_ready').passed, true);
  assert.equal(report.handoff_evidence_summary.evidence_manifest_ready, true);
  assert.equal(report.handoff_evidence_summary.evidence_manifest_sha256, 'c'.repeat(64));
  assert.equal(report.handoff_evidence_summary.artifact_count, 9);
  assert.equal(report.handoff_evidence_summary.html_artifact_summary.package_ready, true);
  assert.equal(report.handoff_evidence_summary.html_artifact_summary.package_template_id, 'third_party_handoff_document');
  assert.equal(report.handoff_evidence_summary.html_artifact_summary.archive_ready, true);
  assert.equal(report.handoff_evidence_summary.html_artifact_summary.archive_template_id, 'third_party_handoff_document');
});

test('buildReadinessReport fails closed when handoff release validation fails', () => {
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
    releaseValidation: {
      release_ready: false,
      checks: [{ key: 'delivery_manifest_ready', passed: false }],
      errors: [{ code: 'delivery_archive_sha256_mismatch' }],
      package_summary: { error_codes: [] },
      archive_summary: { error_codes: [] },
      delivery_manifest_summary: { error_codes: ['delivery_archive_sha256_mismatch'] },
    },
  });

  assert.equal(report.ready_for_customer_sandbox, false);
  assert.equal(report.checks.find((check) => check.key === 'handoff_release_ready').passed, false);
  assert.deepEqual(report.handoff_release_summary.error_codes, ['delivery_archive_sha256_mismatch']);
});

test('buildReadinessReport fails closed when aggregate handoff validation fails', () => {
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
    handoffAllValidation: {
      all_ready: false,
      checks: [{ key: 'delivery_manifest_ready', passed: false }],
      summaries: {
        package: { error_codes: [] },
        archive: { error_codes: [] },
        delivery: { error_codes: ['delivery_release_markdown_sha256_mismatch'] },
        release: { error_codes: ['delivery_release_markdown_sha256_mismatch'] },
      },
      errors: [
        {
          scope: 'delivery',
          code: 'delivery_release_markdown_sha256_mismatch',
          message: 'release markdown SHA256 mismatch',
          path: 'package.release.md',
        },
      ],
    },
  });

  assert.equal(report.ready_for_customer_sandbox, false);
  assert.equal(report.checks.find((check) => check.key === 'handoff_all_ready').passed, false);
  assert.equal(report.handoff_all_summary.package_html_artifact_ready, false);
  assert.equal(report.handoff_all_summary.archive_html_artifact_ready, false);
  assert.deepEqual(report.handoff_all_summary.error_codes, ['delivery_release_markdown_sha256_mismatch']);
  assert.deepEqual(report.handoff_all_summary.scoped_error_codes, ['delivery:delivery_release_markdown_sha256_mismatch']);
});

test('buildReadinessReport fails closed when final evidence manifest validation fails', () => {
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
    handoffEvidenceValidation: {
      evidence_manifest_ready: false,
      evidence_manifest_path: 'target/external-third-party-handoff/package.evidence-manifest.json',
      evidence_manifest_sha256: null,
      manifest_type: 'v3.external_third_party_handoff_evidence_manifest.v1',
      package_name: 'package',
      artifact_count: 9,
      errors: [
        {
          code: 'evidence_aggregate_markdown_sha256_mismatch',
          message: 'aggregate markdown SHA256 mismatch',
          path: 'package.all.md',
        },
      ],
    },
  });

  assert.equal(report.ready_for_customer_sandbox, false);
  assert.equal(report.checks.find((check) => check.key === 'handoff_evidence_ready').passed, false);
  assert.equal(report.handoff_evidence_summary.html_artifact_summary.package_ready, false);
  assert.equal(report.handoff_evidence_summary.html_artifact_summary.archive_ready, false);
  assert.deepEqual(report.handoff_evidence_summary.error_codes, ['evidence_aggregate_markdown_sha256_mismatch']);
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
    handoffAllValidation: {
      all_ready: true,
      checks: [{ key: 'release_ready', passed: true }],
      summaries: {
        package: {
          html_artifact_ready: true,
          html_artifact_template_id: 'third_party_handoff_document',
          error_codes: [],
        },
        archive: {
          html_artifact_ready: true,
          html_artifact_template_id: 'third_party_handoff_document',
          error_codes: [],
        },
        delivery: { error_codes: [] },
        release: { error_codes: [] },
      },
      errors: [],
    },
    handoffEvidenceValidation: {
      evidence_manifest_ready: true,
      evidence_manifest_sha256: 'c'.repeat(64),
      artifact_count: 9,
      html_artifact_summary: {
        package_ready: true,
        package_template_id: 'third_party_handoff_document',
        archive_ready: true,
        archive_template_id: 'third_party_handoff_document',
      },
      errors: [],
    },
  });

  const markdown = renderReadinessMarkdown(report);

  assert.match(markdown, /Status: passed/);
  assert.match(markdown, /Handoff Manifest Summary/);
  assert.match(markdown, /Handoff Release Summary/);
  assert.match(markdown, /Aggregate Handoff Summary/);
  assert.match(markdown, /Final Evidence Manifest Summary/);
  assert.match(markdown, /Package HTML artifact/);
  assert.match(markdown, /Archive HTML artifact/);
  assert.match(markdown, /Third-Party Handoff Items/);
  assert.doesNotMatch(markdown, /third-party-secret|raw prompt secret|callback-token-should-not-leak/);
});
