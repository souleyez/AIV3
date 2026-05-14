import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const MANIFEST_VERSION = 'v3.external_handoff.v1';
const RAW_SECRET_KEYS = new Set([
  'api_key',
  'authorization',
  'bearer_token',
  'client_secret',
  'password',
  'private_key',
  'secret',
  'signing_secret',
  'token',
]);

function isObject(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function nonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function addError(errors, code, message, pathName = '') {
  errors.push({ code, message, path: pathName });
}

function addWarning(warnings, code, message, pathName = '') {
  warnings.push({ code, message, path: pathName });
}

function isLoopbackUrl(urlString) {
  try {
    const parsed = new URL(urlString);
    return parsed.protocol === 'http:' && ['127.0.0.1', 'localhost', '::1'].includes(parsed.hostname);
  } catch {
    return false;
  }
}

function validateHttpsOrLoopback(urlString, allowHttpLoopback) {
  try {
    const parsed = new URL(urlString);
    if (parsed.protocol === 'https:') {
      return true;
    }
    return allowHttpLoopback === true && isLoopbackUrl(urlString);
  } catch {
    return false;
  }
}

function collectRawSecretFindings(value, currentPath = '') {
  const findings = [];
  if (Array.isArray(value)) {
    value.forEach((item, index) => {
      findings.push(...collectRawSecretFindings(item, `${currentPath}[${index}]`));
    });
    return findings;
  }
  if (!isObject(value)) {
    if (typeof value === 'string' && /Bearer\s+|-----BEGIN|\bsk-[a-zA-Z0-9_-]{12,}|AKIA[0-9A-Z]{16}/.test(value)) {
      findings.push({ path: currentPath, reason: 'secret_like_value' });
    }
    return findings;
  }
  Object.entries(value).forEach(([key, nested]) => {
    const nextPath = currentPath ? `${currentPath}.${key}` : key;
    if (RAW_SECRET_KEYS.has(key.toLowerCase())) {
      findings.push({ path: nextPath, reason: 'raw_secret_key' });
      return;
    }
    findings.push(...collectRawSecretFindings(nested, nextPath));
  });
  return findings;
}

function validateRequiredObjects(manifest, errors) {
  for (const key of ['customer', 'environment', 'channel', 'dispatch', 'callbacks', 'documents', 'operations']) {
    if (!isObject(manifest[key])) {
      addError(errors, 'required_object_missing', `${key} must be an object`, key);
    }
  }
}

function validateExternalHandoffManifest(manifest) {
  const errors = [];
  const warnings = [];

  if (!isObject(manifest)) {
    addError(errors, 'manifest_not_object', 'handoff manifest must be a JSON object');
    return buildResult({ manifest, errors, warnings });
  }

  if (manifest.manifest_version !== MANIFEST_VERSION) {
    addError(
      errors,
      'manifest_version_invalid',
      `manifest_version must be ${MANIFEST_VERSION}`,
      'manifest_version',
    );
  }
  validateRequiredObjects(manifest, errors);

  const customer = isObject(manifest.customer) ? manifest.customer : {};
  if (!nonEmptyString(customer.name)) {
    addError(errors, 'customer_name_missing', 'customer.name is required', 'customer.name');
  }
  if (!nonEmptyString(customer.technical_contact)) {
    addError(errors, 'technical_contact_missing', 'customer.technical_contact is required', 'customer.technical_contact');
  }

  const environment = isObject(manifest.environment) ? manifest.environment : {};
  if (!nonEmptyString(environment.name)) {
    addError(errors, 'environment_name_missing', 'environment.name is required', 'environment.name');
  }
  if (!validateHttpsOrLoopback(environment.base_url, environment.allow_http_loopback)) {
    addError(
      errors,
      'environment_base_url_invalid',
      'environment.base_url must be HTTPS, except explicit HTTP loopback for local mock tests',
      'environment.base_url',
    );
  }

  const channel = isObject(manifest.channel) ? manifest.channel : {};
  for (const key of ['mode', 'sample_conversation_id', 'sample_user_id', 'sample_message_id']) {
    if (!nonEmptyString(channel[key])) {
      addError(errors, 'channel_stable_id_missing', `channel.${key} is required`, `channel.${key}`);
    }
  }

  const dispatch = isObject(manifest.dispatch) ? manifest.dispatch : {};
  if (!validateHttpsOrLoopback(dispatch.endpoint_url, environment.allow_http_loopback)) {
    addError(
      errors,
      'dispatch_endpoint_url_invalid',
      'dispatch.endpoint_url must be HTTPS, except explicit HTTP loopback for local mock tests',
      'dispatch.endpoint_url',
    );
  }
  const authModes = Array.isArray(dispatch.auth_modes) ? dispatch.auth_modes.map(String) : [];
  if (!authModes.some((mode) => ['bearer', 'hmac'].includes(mode))) {
    addError(errors, 'dispatch_auth_mode_missing', 'dispatch.auth_modes must include bearer or hmac', 'dispatch.auth_modes');
  }
  if (authModes.includes('bearer') && !nonEmptyString(dispatch.bearer_token_delivery)) {
    addError(
      errors,
      'bearer_delivery_missing',
      'dispatch.bearer_token_delivery must describe out-of-band token exchange',
      'dispatch.bearer_token_delivery',
    );
  }
  if (authModes.includes('hmac') && !nonEmptyString(dispatch.signing_secret_delivery)) {
    addError(
      errors,
      'signing_secret_delivery_missing',
      'dispatch.signing_secret_delivery must describe out-of-band secret exchange',
      'dispatch.signing_secret_delivery',
    );
  }

  const callbacks = isObject(manifest.callbacks) ? manifest.callbacks : {};
  if (callbacks.v3_result_callback_allowlisted !== true) {
    addError(
      errors,
      'callback_allowlist_missing',
      'callbacks.v3_result_callback_allowlisted must be true before customer sandbox validation',
      'callbacks.v3_result_callback_allowlisted',
    );
  }
  if (!validateHttpsOrLoopback(callbacks.allowed_v3_base_url, environment.allow_http_loopback)) {
    addError(
      errors,
      'callback_v3_base_url_invalid',
      'callbacks.allowed_v3_base_url must be HTTPS, except explicit HTTP loopback for local mock tests',
      'callbacks.allowed_v3_base_url',
    );
  }
  if (!nonEmptyString(callbacks.network_rule_reference)) {
    addWarning(
      warnings,
      'callback_network_rule_reference_missing',
      'callbacks.network_rule_reference is recommended for deployment sign-off',
      'callbacks.network_rule_reference',
    );
  }

  const documents = isObject(manifest.documents) ? manifest.documents : {};
  if (Number(documents.fixture_count || 0) < 1) {
    addError(errors, 'document_fixture_missing', 'documents.fixture_count must be at least 1', 'documents.fixture_count');
  }
  if (Number(documents.acl_fixture_count || 0) < 1) {
    addError(errors, 'acl_fixture_missing', 'documents.acl_fixture_count must be at least 1', 'documents.acl_fixture_count');
  }
  if (!Array.isArray(documents.known_access_cases) || documents.known_access_cases.length < 1) {
    addError(
      errors,
      'access_case_missing',
      'documents.known_access_cases must include at least one allow/deny case',
      'documents.known_access_cases',
    );
  }

  const operations = isObject(manifest.operations) ? manifest.operations : {};
  for (const key of ['retry_contact', 'escalation_contact']) {
    if (!nonEmptyString(operations[key])) {
      addError(errors, 'operations_contact_missing', `operations.${key} is required`, `operations.${key}`);
    }
  }

  for (const finding of collectRawSecretFindings(manifest)) {
    addError(
      errors,
      'raw_secret_material_present',
      `handoff manifest must not include raw secret material (${finding.reason})`,
      finding.path,
    );
  }

  return buildResult({ manifest, errors, warnings });
}

function buildResult({ manifest, errors, warnings }) {
  const checks = [
    { key: 'manifest_version', passed: errors.every((error) => error.code !== 'manifest_version_invalid') },
    { key: 'required_objects', passed: errors.every((error) => error.code !== 'required_object_missing') },
    { key: 'customer_contacts', passed: errors.every((error) => !['customer_name_missing', 'technical_contact_missing'].includes(error.code)) },
    { key: 'https_or_loopback_urls', passed: errors.every((error) => !error.code.endsWith('_url_invalid')) },
    { key: 'dispatch_auth', passed: errors.every((error) => !['dispatch_auth_mode_missing', 'bearer_delivery_missing', 'signing_secret_delivery_missing'].includes(error.code)) },
    { key: 'callback_allowlist', passed: errors.every((error) => error.code !== 'callback_allowlist_missing') },
    { key: 'document_acl_fixtures', passed: errors.every((error) => !['document_fixture_missing', 'acl_fixture_missing', 'access_case_missing'].includes(error.code)) },
    { key: 'operations_contacts', passed: errors.every((error) => error.code !== 'operations_contact_missing') },
    { key: 'no_raw_secret_material', passed: errors.every((error) => error.code !== 'raw_secret_material_present') },
  ];
  return {
    report_type: 'external_third_party_handoff_validation',
    manifest_version: isObject(manifest) ? manifest.manifest_version || null : null,
    ready_for_customer_sandbox: errors.length === 0,
    checks,
    errors,
    warnings,
  };
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith('--')) {
      continue;
    }
    parsed[arg.slice(2)] = argv[index + 1];
    index += 1;
  }
  return parsed;
}

function readManifest(inputPath) {
  if (!inputPath) {
    throw new Error('Usage: node tools/validate-external-handoff.mjs --manifest <path>');
  }
  return JSON.parse(fs.readFileSync(inputPath, 'utf8'));
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = validateExternalHandoffManifest(readManifest(args.manifest));
  console.log(JSON.stringify(result, null, 2));
  if (!result.ready_for_customer_sandbox) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export { MANIFEST_VERSION, validateExternalHandoffManifest };
