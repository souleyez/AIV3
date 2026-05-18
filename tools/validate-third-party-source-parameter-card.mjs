import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const PARAMETER_CARD_VERSION = 'v3.third_party_source_parameter_card.v1';

const REQUIRED_ENDPOINTS = [
  'users',
  'documents',
  'document_content',
  'document_acl',
];

const LOOPBACK_HOSTS = new Set(['127.0.0.1', 'localhost', '::1']);

export function validateThirdPartySourceParameterCard(card, { manifestPath = '' } = {}) {
  const errors = [];
  const warnings = [];
  const checks = [];

  const check = (key, label, passed) => {
    checks.push({ key, label, passed: Boolean(passed) });
    return Boolean(passed);
  };
  const error = (code, message, field = '') => errors.push({ code, message, field });
  const warning = (code, message, field = '') => warnings.push({ code, message, field });

  if (!card || typeof card !== 'object' || Array.isArray(card)) {
    return {
      report_type: 'third_party_source_parameter_card_validation',
      manifest_path: manifestPath || null,
      manifest_version: null,
      parameter_card_ready: false,
      checks: [{ key: 'json_object', label: 'Parameter card is a JSON object', passed: false }],
      errors: [{ code: 'card_not_object', message: 'parameter card must be a JSON object', field: '' }],
      warnings,
    };
  }

  if (!check('manifest_version', 'Manifest version is supported', card.manifest_version === PARAMETER_CARD_VERSION)) {
    error('manifest_version_invalid', `manifest_version must be ${PARAMETER_CARD_VERSION}`, 'manifest_version');
  }

  const customerReady = nonEmptyString(card.customer?.name) && nonEmptyString(card.customer?.technical_contact);
  if (!check('customer_contact', 'Customer name and technical contact are present', customerReady)) {
    error('customer_contact_missing', 'customer.name and customer.technical_contact are required', 'customer');
  }

  const environmentReady = validateEnvironment(card.environment, error);
  check('environment_base_url', 'Environment base_url is HTTPS or approved loopback', environmentReady);

  const sourceReady =
    nonEmptyString(card.source?.source_id) &&
    card.source?.connector_kind === 'document' &&
    nonEmptyString(card.source?.dataset_name) &&
    ['pull', 'push', 'hybrid'].includes(card.source?.sync_mode) &&
    card.source?.permission_mode === 'source_acl_snapshot';
  if (!check('source_identity', 'Source identity and permission mode are explicit', sourceReady)) {
    error(
      'source_identity_invalid',
      'source.source_id, source.dataset_name, connector_kind=document, sync_mode and permission_mode=source_acl_snapshot are required',
      'source',
    );
  }

  const endpointsReady = validateEndpoints(card.endpoints, error);
  check('required_endpoints', 'Required source endpoints are declared', endpointsReady);

  const authReady = validateAuth(card.auth, error);
  check('auth_delivery', 'Source API auth is explicit and delivered out of band', authReady);

  const fixtures = card.fixtures || {};
  const users = Array.isArray(fixtures.users) ? fixtures.users : [];
  if (!check('minimum_users', 'At least three test users are provided', users.length >= 3)) {
    error('fixture_users_insufficient', 'fixtures.users must include at least 3 test users', 'fixtures.users');
  }
  const requiredUserIds = new Set();
  for (const user of users) {
    if (nonEmptyString(user?.user_external_id)) {
      requiredUserIds.add(user.user_external_id);
    }
  }

  const documents = Array.isArray(fixtures.documents) ? fixtures.documents : [];
  if (!check('minimum_documents', 'At least five test documents are provided', documents.length >= 5)) {
    error('fixture_documents_insufficient', 'fixtures.documents must include at least 5 test documents', 'fixtures.documents');
  }
  const requiredDocumentIds = new Set();
  for (const document of documents) {
    if (nonEmptyString(document?.document_external_id)) {
      requiredDocumentIds.add(document.document_external_id);
    }
  }

  const accessCases = Array.isArray(fixtures.access_cases) ? fixtures.access_cases : [];
  const accessReady = accessCases.length >= 3 && accessCases.every((accessCase) => {
    return nonEmptyString(accessCase?.case_id) &&
      requiredUserIds.has(accessCase.user_external_id) &&
      requiredDocumentIds.has(accessCase.document_external_id) &&
      ['allow', 'deny'].includes(accessCase.expected);
  });
  if (!check('access_cases', 'Known access cases cover allow and deny expectations', accessReady)) {
    error(
      'access_cases_invalid',
      'fixtures.access_cases must include at least 3 cases referencing declared users and documents with expected allow/deny',
      'fixtures.access_cases',
    );
  } else {
    const expectedKinds = new Set(accessCases.map((accessCase) => accessCase.expected));
    if (!expectedKinds.has('allow') || !expectedKinds.has('deny')) {
      error('access_cases_missing_boundary', 'access cases must include both allow and deny expectations', 'fixtures.access_cases');
      checks.find((item) => item.key === 'access_cases').passed = false;
    }
  }

  const operationsReady = nonEmptyString(card.operations?.retry_contact) && nonEmptyString(card.operations?.escalation_contact);
  if (!check('operations_contacts', 'Retry and escalation contacts are present', operationsReady)) {
    error('operations_contacts_missing', 'operations.retry_contact and operations.escalation_contact are required', 'operations');
  }

  const secretFindings = findRawSecretMaterial(card);
  if (!check('no_raw_secret_material', 'Parameter card does not contain raw secret material', secretFindings.length === 0)) {
    for (const finding of secretFindings) {
      error('raw_secret_material', finding.message, finding.path);
    }
  }

  if (card.environment?.allow_http_loopback === true) {
    warning('loopback_only', 'loopback base_url is suitable for local smoke only, not customer sandbox delivery', 'environment.base_url');
  }

  const parameterCardReady = checks.every((item) => item.passed) && errors.length === 0;
  return {
    report_type: 'third_party_source_parameter_card_validation',
    manifest_path: manifestPath || null,
    manifest_version: card.manifest_version || null,
    parameter_card_ready: parameterCardReady,
    checks,
    errors,
    warnings,
    summary: {
      source_id: card.source?.source_id || null,
      dataset_name: card.source?.dataset_name || null,
      base_url_redacted: redactBaseUrl(card.environment?.base_url || ''),
      user_fixture_count: users.length,
      document_fixture_count: documents.length,
      access_case_count: accessCases.length,
      auth_modes: Array.isArray(card.auth?.request_auth_modes) ? card.auth.request_auth_modes : [],
    },
  };
}

function validateEnvironment(environment, error) {
  if (!environment || typeof environment !== 'object') {
    error('environment_missing', 'environment is required', 'environment');
    return false;
  }
  if (!nonEmptyString(environment.name) || !nonEmptyString(environment.base_url)) {
    error('environment_fields_missing', 'environment.name and environment.base_url are required', 'environment');
    return false;
  }
  let url;
  try {
    url = new URL(environment.base_url);
  } catch {
    error('environment_base_url_invalid', 'environment.base_url must be a valid URL', 'environment.base_url');
    return false;
  }
  const isLoopback = LOOPBACK_HOSTS.has(url.hostname);
  if (url.protocol === 'https:') return true;
  if (url.protocol === 'http:' && isLoopback && environment.allow_http_loopback === true) return true;
  error('environment_base_url_insecure', 'environment.base_url must be HTTPS unless loopback is explicitly allowed', 'environment.base_url');
  return false;
}

function validateEndpoints(endpoints, error) {
  if (!endpoints || typeof endpoints !== 'object') {
    error('endpoints_missing', 'endpoints object is required', 'endpoints');
    return false;
  }
  let ready = true;
  for (const key of REQUIRED_ENDPOINTS) {
    if (!nonEmptyString(endpoints[key])) {
      error('endpoint_missing', `endpoints.${key} is required`, `endpoints.${key}`);
      ready = false;
      continue;
    }
    if (!/^GET\s+\S+/i.test(endpoints[key])) {
      error('endpoint_method_invalid', `endpoints.${key} must be declared as GET <path>`, `endpoints.${key}`);
      ready = false;
    }
  }
  if (nonEmptyString(endpoints.document_content) && !endpoints.document_content.includes('{document_external_id}')) {
    error('endpoint_content_template_invalid', 'document_content endpoint must include {document_external_id}', 'endpoints.document_content');
    ready = false;
  }
  if (nonEmptyString(endpoints.document_acl) && !endpoints.document_acl.includes('{document_external_id}')) {
    error('endpoint_acl_template_invalid', 'document_acl endpoint must include {document_external_id}', 'endpoints.document_acl');
    ready = false;
  }
  return ready;
}

function validateAuth(auth, error) {
  if (!auth || typeof auth !== 'object') {
    error('auth_missing', 'auth object is required', 'auth');
    return false;
  }
  const modes = Array.isArray(auth.request_auth_modes) ? auth.request_auth_modes : [];
  const validModes = modes.length > 0 && modes.every((mode) => ['bearer', 'hmac'].includes(mode));
  const deliveryReady = auth.credential_delivery === 'out_of_band';
  if (!validModes) {
    error('auth_modes_invalid', 'auth.request_auth_modes must include bearer and/or hmac', 'auth.request_auth_modes');
  }
  if (!deliveryReady) {
    error('credential_delivery_invalid', 'auth.credential_delivery must be out_of_band', 'auth.credential_delivery');
  }
  return validModes && deliveryReady;
}

function findRawSecretMaterial(value, currentPath = '') {
  const findings = [];
  if (Array.isArray(value)) {
    value.forEach((item, index) => findings.push(...findRawSecretMaterial(item, `${currentPath}[${index}]`)));
    return findings;
  }
  if (!value || typeof value !== 'object') {
    return findings;
  }
  for (const [key, child] of Object.entries(value)) {
    const childPath = currentPath ? `${currentPath}.${key}` : key;
    if (typeof child === 'string') {
      if (looksLikeRawSecret(key, child)) {
        findings.push({ path: childPath, message: `${childPath} appears to contain raw secret material` });
      }
    } else {
      findings.push(...findRawSecretMaterial(child, childPath));
    }
  }
  return findings;
}

function looksLikeRawSecret(key, rawValue) {
  const value = rawValue.trim();
  if (!value) return false;
  if (isAllowedPlaceholder(value)) return false;
  if (/v3in_live_|Bearer\s+[A-Za-z0-9_-]{12,}|sk-[A-Za-z0-9_-]{12,}|-----BEGIN|AKIA[0-9A-Z]{16}/.test(value)) {
    return true;
  }
  if (/(token|secret|password|credential|private_key|authorization)/i.test(key)) {
    return !/^(out_of_band|customer-managed|v3-managed|none|n\/a)$/i.test(value);
  }
  return false;
}

function isAllowedPlaceholder(value) {
  return /^\[[a-z0-9 _-]+\]$/i.test(value) ||
    /^<[^>]+>$/.test(value) ||
    value.includes('<') ||
    value.includes('[redacted]') ||
    value === 'out_of_band';
}

function redactBaseUrl(rawUrl) {
  if (!rawUrl) return null;
  try {
    const url = new URL(rawUrl);
    return `${url.protocol}//${url.hostname}${url.port ? `:${url.port}` : ''}`;
  } catch {
    return '[invalid-url]';
  }
}

function nonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

export function renderParameterCardMarkdown(report) {
  const lines = [
    '# Third-party Source Parameter Card Validation',
    '',
    `Ready: \`${report.parameter_card_ready ? 'yes' : 'no'}\``,
    `Manifest: \`${report.manifest_version || 'unknown'}\``,
    `Source: \`${report.summary?.source_id || 'unknown'}\``,
    `Dataset: \`${report.summary?.dataset_name || 'unknown'}\``,
    '',
    '## Checks',
    '',
    '| Check | Passed |',
    '| --- | --- |',
    ...report.checks.map((check) => `| ${check.label} | ${check.passed ? 'yes' : 'no'} |`),
  ];
  if (report.errors.length > 0) {
    lines.push('', '## Errors', '');
    for (const item of report.errors) {
      lines.push(`- \`${item.code}\` ${item.field ? `(${item.field}) ` : ''}${item.message}`);
    }
  }
  if (report.warnings.length > 0) {
    lines.push('', '## Warnings', '');
    for (const item of report.warnings) {
      lines.push(`- \`${item.code}\` ${item.field ? `(${item.field}) ` : ''}${item.message}`);
    }
  }
  return `${lines.join('\n')}\n`;
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--manifest') args.manifest = argv[++index];
    else if (arg === '--out') args.out = argv[++index];
    else if (arg === '--markdown') args.markdown = argv[++index];
    else if (arg === '--help' || arg === '-h') args.help = true;
  }
  return args;
}

function printHelp() {
  console.log('Usage: node tools/validate-third-party-source-parameter-card.mjs --manifest <path> [--out report.json] [--markdown report.md]');
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help || !args.manifest) {
    printHelp();
    process.exit(args.help ? 0 : 2);
  }
  const manifestPath = path.resolve(args.manifest);
  const card = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const report = validateThirdPartySourceParameterCard(card, { manifestPath });
  const json = `${JSON.stringify(report, null, 2)}\n`;
  if (args.out) fs.writeFileSync(path.resolve(args.out), json);
  if (args.markdown) fs.writeFileSync(path.resolve(args.markdown), renderParameterCardMarkdown(report));
  if (!args.out) process.stdout.write(json);
  process.exit(report.parameter_card_ready ? 0 : 1);
}

const isCli = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  main().catch((error) => {
    console.error(error?.stack || error);
    process.exit(1);
  });
}
