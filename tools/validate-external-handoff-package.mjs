#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validateExternalHandoffManifest } from './validate-external-handoff.mjs';

const PACKAGE_TYPE = 'v3.external_third_party_handoff_package.v1';
const THIRD_PARTY_HTML_ARTIFACT_PATH = 'html-artifacts/third-party-handoff-document.json';
const REQUIRED_THIRD_PARTY_EVENT_ENDPOINT = '/v1/external/channels/{connection_id}/events';
const UNSAFE_HTML_ARTIFACT_STRING_PATTERNS = [
  /<\s*script\b/i,
  /<\s*iframe\b/i,
  /<\s*object\b/i,
  /<\s*embed\b/i,
  /<\s*link\b/i,
  /<\s*meta\b/i,
  /<\s*form\b/i,
  /\son[a-z]+\s*=/i,
  /\bjavascript\s*:/i,
  /\bdata\s*:\s*text\/html/i,
  /\bsrcdoc\s*=/i,
  /\bsrc\s*=/i,
  /\bhref\s*=/i,
  /https?:\/\//i,
  /\/\/[a-z0-9.-]+\.[a-z]{2,}/i,
  /@import\b/i,
  /\burl\s*\(/i,
  /\b(api[_-]?key|access[_-]?token|authorization|bearer\s+|cookie|secret)\b/i,
];

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

function sha256Hex(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

function addError(errors, code, message, filePath = '') {
  errors.push({ code, message, path: filePath });
}

function safeRelativePath(filePath) {
  if (typeof filePath !== 'string' || filePath.trim().length === 0) {
    return false;
  }
  if (path.isAbsolute(filePath)) {
    return false;
  }
  const normalized = path.normalize(filePath).replaceAll('\\', '/');
  return normalized !== '..' && !normalized.startsWith('../') && !normalized.includes('/../');
}

function readJsonFile(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function isPlainObject(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function unsafeHtmlArtifactStringReason(value) {
  if (typeof value !== 'string') {
    return '';
  }
  const hit = UNSAFE_HTML_ARTIFACT_STRING_PATTERNS.find((pattern) => pattern.test(value));
  return hit ? `unsafe string matched ${hit.source}` : '';
}

function findUnsafeHtmlArtifactPayloadPath(value, payloadPath = '$') {
  if (typeof value === 'function' || typeof value === 'symbol' || typeof value === 'undefined') {
    return `${payloadPath}: unsupported value type`;
  }
  if (typeof value === 'string') {
    const reason = unsafeHtmlArtifactStringReason(value);
    return reason ? `${payloadPath}: ${reason}` : '';
  }
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index += 1) {
      const reason = findUnsafeHtmlArtifactPayloadPath(value[index], `${payloadPath}[${index}]`);
      if (reason) {
        return reason;
      }
    }
    return '';
  }
  if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) {
      const reason = unsafeHtmlArtifactStringReason(key) || findUnsafeHtmlArtifactPayloadPath(child, `${payloadPath}.${key}`);
      if (reason) {
        return reason.startsWith('$') ? reason : `${payloadPath}.${key}: ${reason}`;
      }
    }
  }
  return '';
}

function addHtmlArtifactError(errors, summary, code, message, payloadPath = '') {
  summary.error_codes.push(code);
  addError(errors, code, message, payloadPath ? `${THIRD_PARTY_HTML_ARTIFACT_PATH}:${payloadPath}` : THIRD_PARTY_HTML_ARTIFACT_PATH);
}

function validateThirdPartyHtmlArtifact(packageRoot, errors) {
  const summary = {
    artifact_ready: false,
    path: THIRD_PARTY_HTML_ARTIFACT_PATH,
    template_id: null,
    source_type: null,
    interaction_mode: null,
    endpoint_count: 0,
    validation_command_count: 0,
    error_codes: [],
  };
  const artifactPath = path.join(packageRoot, THIRD_PARTY_HTML_ARTIFACT_PATH);
  let artifact = {};

  if (!fs.existsSync(artifactPath)) {
    addHtmlArtifactError(errors, summary, 'html_artifact_manifest_missing', 'third-party handoff HTML artifact manifest is required');
    return summary;
  }

  try {
    artifact = readJsonFile(artifactPath);
  } catch (error) {
    addHtmlArtifactError(errors, summary, 'html_artifact_manifest_invalid_json', String(error?.message || error));
    return summary;
  }

  summary.template_id = artifact.template_id || artifact.templateId || null;
  summary.source_type = artifact.source_type || artifact.sourceType || null;
  summary.interaction_mode = artifact.interaction_mode || artifact.interactionMode || null;
  const ownerScope = artifact.owner_scope || artifact.ownerScope || {};
  const dataRefs = Array.isArray(artifact.data_refs || artifact.dataRefs) ? artifact.data_refs || artifact.dataRefs : [];
  const payload = isPlainObject(artifact.payload) ? artifact.payload : {};
  const handoff = isPlainObject(payload.handoff) ? payload.handoff : {};
  const endpoints = Array.isArray(payload.endpoints) ? payload.endpoints : [];
  const validationCommands = Array.isArray(payload.validationCommands || payload.validation_commands)
    ? payload.validationCommands || payload.validation_commands
    : [];
  summary.endpoint_count = endpoints.length;
  summary.validation_command_count = validationCommands.length;

  if (artifact.kind !== 'html_artifact') {
    addHtmlArtifactError(errors, summary, 'html_artifact_kind_invalid', 'HTML artifact kind must be html_artifact', 'kind');
  }
  if (artifact.version !== 1) {
    addHtmlArtifactError(errors, summary, 'html_artifact_version_invalid', 'HTML artifact version must be 1', 'version');
  }
  if (summary.source_type !== 'external_integration') {
    addHtmlArtifactError(errors, summary, 'html_artifact_source_type_invalid', 'HTML artifact source_type must be external_integration', 'source_type');
  }
  if (summary.template_id !== 'third_party_handoff_document') {
    addHtmlArtifactError(errors, summary, 'html_artifact_template_invalid', 'HTML artifact template_id must be third_party_handoff_document', 'template_id');
  }
  if (summary.interaction_mode !== 'read_only') {
    addHtmlArtifactError(errors, summary, 'html_artifact_interaction_mode_invalid', 'third-party handoff HTML artifact must be read_only', 'interaction_mode');
  }
  if ((ownerScope.type || ownerScope.scope_type) !== 'external_integration_handoff') {
    addHtmlArtifactError(errors, summary, 'html_artifact_owner_scope_invalid', 'owner scope must describe an external integration handoff', 'owner_scope.type');
  }
  if (!dataRefs.some((ref) => ref.kind === 'source_document' && ref.id === 'docs/pure-third-party-integration-guide.zh-CN.md')) {
    addHtmlArtifactError(errors, summary, 'html_artifact_source_document_ref_missing', 'source document data_ref is required', 'data_refs');
  }
  if (handoff.defaultDomain !== 'v3.elepcloud.com') {
    addHtmlArtifactError(errors, summary, 'html_artifact_default_domain_invalid', 'default domain must be v3.elepcloud.com', 'payload.handoff.defaultDomain');
  }
  if (!endpoints.some((endpoint) => endpoint.method === 'POST' && endpoint.path === REQUIRED_THIRD_PARTY_EVENT_ENDPOINT)) {
    addHtmlArtifactError(errors, summary, 'html_artifact_event_endpoint_missing', 'standard third-party event endpoint is required', 'payload.endpoints');
  }
  if (!validationCommands.some((command) => command.command === 'npm run validate:all')) {
    addHtmlArtifactError(errors, summary, 'html_artifact_all_validation_missing', 'validate:all command must be listed', 'payload.validationCommands');
  }
  const unsafePayloadPath = findUnsafeHtmlArtifactPayloadPath(payload);
  if (unsafePayloadPath) {
    addHtmlArtifactError(errors, summary, 'html_artifact_payload_unsafe', unsafePayloadPath, 'payload');
  }

  summary.artifact_ready = summary.error_codes.length === 0;
  return summary;
}

function validateIncludedFiles(packageRoot, manifest, errors) {
  if (!Array.isArray(manifest.included_files)) {
    addError(errors, 'included_files_invalid', 'included_files must be an array', 'included_files');
    return [];
  }

  return manifest.included_files.map((file) => {
    const filePath = String(file.path || '');
    const item = {
      path: filePath,
      exists: false,
      size_matches: false,
      sha256_matches: false,
    };
    if (!safeRelativePath(filePath)) {
      addError(errors, 'included_file_path_invalid', 'included file path must stay inside package', filePath);
      return item;
    }
    const absolutePath = path.join(packageRoot, filePath);
    if (!fs.existsSync(absolutePath)) {
      addError(errors, 'included_file_missing', 'included file is missing', filePath);
      return item;
    }
    item.exists = true;
    const bytes = fs.readFileSync(absolutePath);
    item.size_matches = Number(file.bytes) === bytes.length;
    item.sha256_matches = String(file.sha256 || '') === sha256Hex(bytes);
    if (!item.size_matches) {
      addError(errors, 'included_file_size_mismatch', 'included file byte size does not match manifest', filePath);
    }
    if (!item.sha256_matches) {
      addError(errors, 'included_file_sha256_mismatch', 'included file sha256 does not match manifest', filePath);
    }
    return item;
  });
}

function validatePackage(packageRootInput = '.') {
  const errors = [];
  const packageRoot = path.resolve(packageRootInput);
  const manifestPath = path.join(packageRoot, 'handoff-package-manifest.json');
  let manifest = {};

  if (!fs.existsSync(manifestPath)) {
    addError(errors, 'package_manifest_missing', 'handoff-package-manifest.json is required', 'handoff-package-manifest.json');
  } else {
    try {
      manifest = readJsonFile(manifestPath);
    } catch (error) {
      addError(errors, 'package_manifest_invalid_json', String(error?.message || error), 'handoff-package-manifest.json');
    }
  }

  if (manifest.package_type !== PACKAGE_TYPE) {
    addError(errors, 'package_type_invalid', `package_type must be ${PACKAGE_TYPE}`, 'package_type');
  }
  if (manifest.package_root && manifest.package_root !== '.') {
    addError(errors, 'package_root_not_relative', 'package_root must be "." for sendable packages', 'package_root');
  }

  const files = validateIncludedFiles(packageRoot, manifest, errors);
  const htmlArtifactValidation = validateThirdPartyHtmlArtifact(packageRoot, errors);
  const handoffPath = path.join(packageRoot, 'handoff', 'third-party-handoff.sample.json');
  let handoffValidation = null;
  if (fs.existsSync(handoffPath)) {
    handoffValidation = validateExternalHandoffManifest(readJsonFile(handoffPath));
    for (const error of handoffValidation.errors) {
      addError(errors, `handoff_${error.code}`, error.message, `handoff/third-party-handoff.sample.json:${error.path}`);
    }
  } else {
    addError(errors, 'handoff_manifest_missing', 'handoff/third-party-handoff.sample.json is required', 'handoff/third-party-handoff.sample.json');
  }

  const checks = [
    { key: 'package_manifest_present', passed: errors.every((error) => error.code !== 'package_manifest_missing') },
    { key: 'package_type', passed: errors.every((error) => error.code !== 'package_type_invalid') },
    { key: 'package_root_relative', passed: errors.every((error) => error.code !== 'package_root_not_relative') },
    {
      key: 'included_file_paths',
      passed: errors.every((error) => error.code !== 'included_file_path_invalid'),
    },
    {
      key: 'included_file_integrity',
      passed: errors.every((error) => !['included_file_missing', 'included_file_size_mismatch', 'included_file_sha256_mismatch'].includes(error.code)),
    },
    {
      key: 'handoff_manifest',
      passed: handoffValidation?.ready_for_customer_sandbox === true,
    },
    {
      key: 'third_party_html_artifact_manifest',
      passed: htmlArtifactValidation.artifact_ready === true,
    },
  ];

  return {
    report_type: 'external_third_party_handoff_package_validation',
    package_type: manifest.package_type || null,
    generated_at: manifest.generated_at || null,
    repository_head: manifest.repository_head || null,
    package_ready: errors.length === 0,
    package_root: packageRoot,
    checks,
    included_file_count: files.length,
    html_artifact_validation: htmlArtifactValidation,
    handoff_validation: handoffValidation
      ? {
          ready_for_customer_sandbox: handoffValidation.ready_for_customer_sandbox === true,
          error_codes: handoffValidation.errors.map((error) => error.code),
          warning_codes: handoffValidation.warnings.map((warning) => warning.code),
        }
      : null,
    errors,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = validatePackage(args.package || '.');
  console.log(JSON.stringify(result, null, 2));
  if (!result.package_ready) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export { validatePackage };
