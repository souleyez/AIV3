#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validateExternalHandoffManifest } from './validate-external-handoff.mjs';

const PACKAGE_TYPE = 'v3.external_third_party_handoff_package.v1';

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
  ];

  return {
    report_type: 'external_third_party_handoff_package_validation',
    package_type: manifest.package_type || null,
    package_ready: errors.length === 0,
    package_root: packageRoot,
    checks,
    included_file_count: files.length,
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
