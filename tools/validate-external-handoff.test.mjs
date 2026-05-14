import test from 'node:test';
import assert from 'node:assert/strict';
import { validateExternalHandoffManifest } from './validate-external-handoff.mjs';

function validManifest(overrides = {}) {
  return {
    manifest_version: 'v3.external_handoff.v1',
    customer: {
      name: 'Example Customer',
      technical_contact: 'integration@example.com',
    },
    environment: {
      name: 'sandbox',
      base_url: 'https://customer-sandbox.example.com',
      allow_http_loopback: false,
    },
    channel: {
      mode: 'generic_chat',
      sample_conversation_id: 'chat-risk-room',
      sample_user_id: 'user-ext-001',
      sample_message_id: 'msg-001',
    },
    dispatch: {
      endpoint_url: 'https://customer-sandbox.example.com/v3/actions',
      auth_modes: ['bearer', 'hmac'],
      bearer_token_delivery: 'out_of_band',
      signing_secret_delivery: 'out_of_band',
    },
    callbacks: {
      v3_result_callback_allowlisted: true,
      allowed_v3_base_url: 'https://v3.elepcloud.com',
      network_rule_reference: 'FW-12345',
    },
    documents: {
      fixture_count: 3,
      acl_fixture_count: 3,
      known_access_cases: [
        {
          user_id: 'user-ext-001',
          can_access: ['doc-public'],
          cannot_access: ['doc-finance'],
        },
      ],
    },
    artifacts: {
      supports_publish: true,
      supports_status: true,
      supports_revoke: true,
    },
    operations: {
      retry_contact: 'ops@example.com',
      escalation_contact: 'support@example.com',
      maintenance_window: 'UTC+8 02:00-04:00',
    },
    ...overrides,
  };
}

test('validateExternalHandoffManifest accepts complete sandbox manifest', () => {
  const result = validateExternalHandoffManifest(validManifest());

  assert.equal(result.ready_for_customer_sandbox, true);
  assert.equal(result.errors.length, 0);
  assert.ok(result.checks.every((check) => check.passed));
});

test('validateExternalHandoffManifest allows explicit HTTP loopback for local mock tests', () => {
  const result = validateExternalHandoffManifest(validManifest({
    environment: {
      name: 'local-mock',
      base_url: 'http://127.0.0.1:43180',
      allow_http_loopback: true,
    },
    dispatch: {
      endpoint_url: 'http://127.0.0.1:43180/third-party/actions',
      auth_modes: ['hmac'],
      signing_secret_delivery: 'out_of_band',
    },
    callbacks: {
      v3_result_callback_allowlisted: true,
      allowed_v3_base_url: 'http://localhost:3100',
      network_rule_reference: 'local-only',
    },
  }));

  assert.equal(result.ready_for_customer_sandbox, true);
});

test('validateExternalHandoffManifest rejects public HTTP and missing auth', () => {
  const result = validateExternalHandoffManifest(validManifest({
    environment: {
      name: 'bad-sandbox',
      base_url: 'http://customer-sandbox.example.com',
      allow_http_loopback: false,
    },
    dispatch: {
      endpoint_url: 'http://customer-sandbox.example.com/v3/actions',
      auth_modes: [],
    },
  }));

  assert.equal(result.ready_for_customer_sandbox, false);
  assert.ok(result.errors.some((error) => error.code === 'environment_base_url_invalid'));
  assert.ok(result.errors.some((error) => error.code === 'dispatch_endpoint_url_invalid'));
  assert.ok(result.errors.some((error) => error.code === 'dispatch_auth_mode_missing'));
});

test('validateExternalHandoffManifest rejects raw secret material', () => {
  const result = validateExternalHandoffManifest(validManifest({
    dispatch: {
      endpoint_url: 'https://customer-sandbox.example.com/v3/actions',
      auth_modes: ['bearer'],
      bearer_token_delivery: 'out_of_band',
      bearer_token: 'Bearer customer-secret-token',
    },
  }));

  assert.equal(result.ready_for_customer_sandbox, false);
  assert.ok(result.errors.some((error) => error.code === 'raw_secret_material_present'));
});

test('validateExternalHandoffManifest requires document ACL fixtures and contacts', () => {
  const result = validateExternalHandoffManifest(validManifest({
    documents: {
      fixture_count: 0,
      acl_fixture_count: 0,
      known_access_cases: [],
    },
    operations: {
      retry_contact: '',
      escalation_contact: '',
    },
  }));

  assert.equal(result.ready_for_customer_sandbox, false);
  assert.ok(result.errors.some((error) => error.code === 'document_fixture_missing'));
  assert.ok(result.errors.some((error) => error.code === 'acl_fixture_missing'));
  assert.ok(result.errors.some((error) => error.code === 'access_case_missing'));
  assert.ok(result.errors.some((error) => error.code === 'operations_contact_missing'));
});
