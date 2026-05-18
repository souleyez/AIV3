import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const MANIFEST_VERSION = 'v3.edge_local_data_plane.v1';
const EXECUTION_PROFILES = new Set(['local_strict', 'local_retrieval_v3_generation']);
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

function addIssue(issues, code, message, pathName = '') {
  issues.push({ code, message, path: pathName });
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
    return parsed.protocol === 'https:' || (allowHttpLoopback === true && isLoopbackUrl(urlString));
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
  for (const key of [
    'customer',
    'environment',
    'v3_control_plane',
    'edge_agent',
    'data_plane',
    'capability_contracts',
    'page_generation',
    'observability',
    'security',
  ]) {
    if (!isObject(manifest[key])) {
      addIssue(errors, 'required_object_missing', `${key} must be an object`, key);
    }
  }
}

function validateBoolean(value, expected, errors, code, message, pathName) {
  if (value !== expected) {
    addIssue(errors, code, message, pathName);
  }
}

function validateV3EdgeLocalDataPlaneManifest(manifest) {
  const errors = [];
  const warnings = [];

  if (!isObject(manifest)) {
    addIssue(errors, 'manifest_not_object', 'manifest must be a JSON object');
    return buildResult({ manifest, errors, warnings });
  }

  if (manifest.manifest_version !== MANIFEST_VERSION) {
    addIssue(errors, 'manifest_version_invalid', `manifest_version must be ${MANIFEST_VERSION}`, 'manifest_version');
  }
  validateRequiredObjects(manifest, errors);

  const customer = isObject(manifest.customer) ? manifest.customer : {};
  if (!nonEmptyString(customer.name)) {
    addIssue(errors, 'customer_name_missing', 'customer.name is required', 'customer.name');
  }
  if (!nonEmptyString(customer.technical_contact)) {
    addIssue(errors, 'technical_contact_missing', 'customer.technical_contact is required', 'customer.technical_contact');
  }

  const environment = isObject(manifest.environment) ? manifest.environment : {};
  if (!nonEmptyString(environment.name)) {
    addIssue(errors, 'environment_name_missing', 'environment.name is required', 'environment.name');
  }
  if (!EXECUTION_PROFILES.has(environment.execution_profile)) {
    addIssue(
      errors,
      'execution_profile_invalid',
      `environment.execution_profile must be one of ${Array.from(EXECUTION_PROFILES).join(', ')}`,
      'environment.execution_profile',
    );
  }

  const controlPlane = isObject(manifest.v3_control_plane) ? manifest.v3_control_plane : {};
  if (!validateHttpsOrLoopback(controlPlane.base_url, false)) {
    addIssue(errors, 'v3_base_url_invalid', 'v3_control_plane.base_url must be HTTPS', 'v3_control_plane.base_url');
  }
  for (const key of ['stores_source_documents', 'stores_parsed_text', 'stores_vector_index', 'stores_generated_html']) {
    validateBoolean(
      controlPlane[key],
      false,
      errors,
      'v3_storage_boundary_invalid',
      `v3_control_plane.${key} must be false in local data plane mode`,
      `v3_control_plane.${key}`,
    );
  }
  const allowedDataClasses = Array.isArray(controlPlane.allowed_data_classes)
    ? controlPlane.allowed_data_classes.map(String)
    : [];
  for (const forbidden of ['raw_document', 'parsed_text', 'vector_index', 'html_body', 'download_url']) {
    if (allowedDataClasses.includes(forbidden)) {
      addIssue(
        errors,
        'v3_allowed_data_class_forbidden',
        `v3_control_plane.allowed_data_classes must not include ${forbidden}`,
        'v3_control_plane.allowed_data_classes',
      );
    }
  }

  const edgeAgent = isObject(manifest.edge_agent) ? manifest.edge_agent : {};
  if (edgeAgent.hosted_by !== 'third_party') {
    addIssue(errors, 'edge_agent_host_invalid', 'edge_agent.hosted_by must be third_party', 'edge_agent.hosted_by');
  }
  if (!validateHttpsOrLoopback(edgeAgent.public_base_url, environment.allow_http_loopback)) {
    addIssue(
      errors,
      'edge_agent_url_invalid',
      'edge_agent.public_base_url must be HTTPS, except explicit HTTP loopback for local tests',
      'edge_agent.public_base_url',
    );
  }
  validateBoolean(
    edgeAgent.browser_calls_v3_directly,
    false,
    errors,
    'browser_direct_v3_forbidden',
    'edge_agent.browser_calls_v3_directly must be false',
    'edge_agent.browser_calls_v3_directly',
  );
  validateBoolean(
    edgeAgent.exposes_browser_token,
    false,
    errors,
    'browser_token_exposure_forbidden',
    'edge_agent.exposes_browser_token must be false',
    'edge_agent.exposes_browser_token',
  );
  const capabilities = isObject(edgeAgent.capabilities) ? edgeAgent.capabilities : {};
  for (const key of ['local_acl_filter', 'local_retrieval', 'local_page_generation', 'local_artifact_publish']) {
    if (capabilities[key] !== true) {
      addIssue(errors, 'edge_capability_missing', `edge_agent.capabilities.${key} must be true`, `edge_agent.capabilities.${key}`);
    }
  }

  const dataPlane = isObject(manifest.data_plane) ? manifest.data_plane : {};
  const documentStore = isObject(dataPlane.document_store) ? dataPlane.document_store : {};
  const indexStore = isObject(dataPlane.index_store) ? dataPlane.index_store : {};
  const aclSource = isObject(dataPlane.acl_source) ? dataPlane.acl_source : {};
  if (documentStore.owner !== 'third_party') {
    addIssue(errors, 'document_store_owner_invalid', 'data_plane.document_store.owner must be third_party', 'data_plane.document_store.owner');
  }
  validateBoolean(
    documentStore.source_documents_leave_customer_network,
    false,
    errors,
    'source_document_egress_forbidden',
    'source documents must not leave the customer network in this mode',
    'data_plane.document_store.source_documents_leave_customer_network',
  );
  if (indexStore.owner !== 'third_party') {
    addIssue(errors, 'index_store_owner_invalid', 'data_plane.index_store.owner must be third_party', 'data_plane.index_store.owner');
  }
  validateBoolean(
    indexStore.index_leaves_customer_network,
    false,
    errors,
    'index_egress_forbidden',
    'vector indexes must not leave the customer network in this mode',
    'data_plane.index_store.index_leaves_customer_network',
  );
  if (aclSource.principal_key !== 'sender_external_id') {
    addIssue(errors, 'acl_principal_key_invalid', 'data_plane.acl_source.principal_key must be sender_external_id', 'data_plane.acl_source.principal_key');
  }

  const contracts = isObject(manifest.capability_contracts) ? manifest.capability_contracts : {};
  const retrieval = isObject(contracts.retrieval) ? contracts.retrieval : {};
  const permissions = isObject(contracts.permissions) ? contracts.permissions : {};
  const modelRouting = isObject(contracts.model_routing) ? contracts.model_routing : {};
  if (modelRouting.policy_owner !== 'v3') {
    addIssue(errors, 'model_policy_owner_invalid', 'capability_contracts.model_routing.policy_owner must be v3', 'capability_contracts.model_routing.policy_owner');
  }
  if (retrieval.execution_owner !== 'third_party') {
    addIssue(errors, 'retrieval_owner_invalid', 'capability_contracts.retrieval.execution_owner must be third_party', 'capability_contracts.retrieval.execution_owner');
  }
  if (environment.execution_profile === 'local_strict') {
    validateBoolean(
      retrieval.evidence_snippets_sent_to_v3,
      false,
      errors,
      'strict_mode_evidence_egress_forbidden',
      'local_strict must not send evidence snippets to V3',
      'capability_contracts.retrieval.evidence_snippets_sent_to_v3',
    );
    if (modelRouting.approved_generation_boundary !== 'local_only') {
      addIssue(
        errors,
        'strict_mode_generation_boundary_invalid',
        'local_strict requires approved_generation_boundary=local_only',
        'capability_contracts.model_routing.approved_generation_boundary',
      );
    }
  }
  if (environment.execution_profile === 'local_retrieval_v3_generation') {
    if (retrieval.evidence_snippets_sent_to_v3 !== true) {
      addIssue(
        errors,
        'federated_generation_evidence_boundary_missing',
        'local_retrieval_v3_generation must explicitly allow permission-filtered evidence snippets to V3',
        'capability_contracts.retrieval.evidence_snippets_sent_to_v3',
      );
    }
    if (modelRouting.approved_generation_boundary !== 'permission_filtered_evidence_to_v3') {
      addIssue(
        errors,
        'federated_generation_boundary_invalid',
        'local_retrieval_v3_generation requires approved_generation_boundary=permission_filtered_evidence_to_v3',
        'capability_contracts.model_routing.approved_generation_boundary',
      );
    }
  }
  if (permissions.enforcement_owner !== 'third_party' || permissions.deny_unknown_principal !== true) {
    addIssue(
      errors,
      'permission_enforcement_invalid',
      'permissions must be enforced by third_party and deny unknown principals',
      'capability_contracts.permissions',
    );
  }

  const pageGeneration = isObject(manifest.page_generation) ? manifest.page_generation : {};
  if (pageGeneration.request_entrypoint !== 'third_party_gateway') {
    addIssue(errors, 'page_request_entrypoint_invalid', 'page_generation.request_entrypoint must be third_party_gateway', 'page_generation.request_entrypoint');
  }
  if (pageGeneration.preview_hosted_by !== 'third_party' || pageGeneration.publish_hosted_by !== 'third_party') {
    addIssue(errors, 'page_hosting_owner_invalid', 'page preview and publish must be hosted by third_party', 'page_generation');
  }
  validateBoolean(
    pageGeneration.v3_receives_html_body,
    false,
    errors,
    'html_body_to_v3_forbidden',
    'page_generation.v3_receives_html_body must be false',
    'page_generation.v3_receives_html_body',
  );
  const outputModes = Array.isArray(pageGeneration.output_modes) ? pageGeneration.output_modes.map(String) : [];
  if (!outputModes.includes('page_blueprint') && !outputModes.includes('html_package')) {
    addIssue(errors, 'page_output_mode_missing', 'page_generation.output_modes must include page_blueprint or html_package', 'page_generation.output_modes');
  }

  const observability = isObject(manifest.observability) ? manifest.observability : {};
  validateBoolean(
    observability.v3_receives_raw_logs,
    false,
    errors,
    'raw_logs_to_v3_forbidden',
    'observability.v3_receives_raw_logs must be false',
    'observability.v3_receives_raw_logs',
  );
  if (observability.v3_receives_redacted_status !== true) {
    addIssue(errors, 'redacted_status_missing', 'observability.v3_receives_redacted_status must be true', 'observability.v3_receives_redacted_status');
  }

  const security = isObject(manifest.security) ? manifest.security : {};
  if (security.credential_delivery !== 'out_of_band') {
    addIssue(errors, 'credential_delivery_invalid', 'security.credential_delivery must be out_of_band', 'security.credential_delivery');
  }
  if (security.browser_token_policy !== 'no_v3_tokens_in_browser') {
    addIssue(errors, 'browser_token_policy_invalid', 'security.browser_token_policy must be no_v3_tokens_in_browser', 'security.browser_token_policy');
  }
  validateBoolean(
    security.secret_material_included,
    false,
    errors,
    'secret_material_included',
    'security.secret_material_included must be false',
    'security.secret_material_included',
  );

  for (const finding of collectRawSecretFindings(manifest)) {
    addIssue(errors, 'raw_secret_material_present', `manifest contains raw secret material: ${finding.reason}`, finding.path);
  }

  return buildResult({ manifest, errors, warnings });
}

function buildResult({ manifest, errors, warnings }) {
  return {
    ok: errors.length === 0,
    manifest_version: isObject(manifest) ? manifest.manifest_version : undefined,
    errors,
    warnings,
  };
}

function parseArgs(argv) {
  const args = { manifestPath: '' };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--manifest') {
      args.manifestPath = argv[index + 1] || '';
      index += 1;
    }
  }
  return args;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function main() {
  const { manifestPath } = parseArgs(process.argv.slice(2));
  if (!manifestPath) {
    console.error('Usage: node tools/validate-v3-edge-local-data-plane.mjs --manifest <path>');
    process.exitCode = 2;
    return;
  }
  const absolutePath = path.resolve(manifestPath);
  const result = validateV3EdgeLocalDataPlaneManifest(readJson(absolutePath));
  console.log(JSON.stringify(result, null, 2));
  if (!result.ok) {
    process.exitCode = 1;
  }
}

const isCli = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  main();
}

export { validateV3EdgeLocalDataPlaneManifest };
