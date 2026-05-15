#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';
import { validateExternalHandoffManifest } from './validate-external-handoff.mjs';

const PACKAGE_TYPE = 'v3.external_third_party_handoff_package.v1';
const REQUIRED_ENTRIES = [
  'README.zh-CN.md',
  'README.md',
  'package.json',
  'handoff-package-manifest.json',
  'docs/pure-third-party-integration-guide.zh-CN.md',
  'docs/pure-third-party-integration-guide.zh-CN.html',
  'html-artifacts/third-party-handoff-document.json',
  'handoff/third-party-handoff.sample.json',
  'tools/validate-external-handoff.mjs',
  'tools/validate-external-handoff-package.mjs',
  'tools/validate-external-handoff-archive.mjs',
  'sandbox/external-third-party-mock-gateway.mjs',
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

function addError(errors, code, message, entry = '') {
  errors.push({ code, message, path: entry });
}

function tarString(buffer, start, end) {
  return buffer.toString('utf8', start, end).replace(/\0.*$/, '');
}

function tarNumber(buffer, start, end) {
  const text = buffer.toString('ascii', start, end).replace(/\0.*$/, '').trim();
  return Number.parseInt(text || '0', 8);
}

function tarEntryName(header) {
  const name = tarString(header, 0, 100);
  const prefix = tarString(header, 345, 500);
  return prefix ? `${prefix}/${name}` : name;
}

function safeArchiveEntry(name) {
  if (!name || path.isAbsolute(name)) {
    return false;
  }
  const unixName = name.replaceAll('\\', '/');
  if (unixName.split('/').includes('..')) {
    return false;
  }
  const normalized = path.posix.normalize(unixName);
  return normalized !== '..' && !normalized.startsWith('../') && !normalized.includes('/../');
}

function parseTarEntries(tarBytes, errors) {
  const entries = [];
  let offset = 0;
  while (offset + 512 <= tarBytes.length) {
    const header = tarBytes.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) {
      break;
    }
    const name = tarEntryName(header);
    const size = tarNumber(header, 124, 136);
    const typeflag = tarString(header, 156, 157) || '0';
    if (!safeArchiveEntry(name)) {
      addError(errors, 'archive_entry_path_invalid', 'archive entry path must be relative and stay inside package root', name);
    }
    const bodyStart = offset + 512;
    const bodyEnd = bodyStart + size;
    if (bodyEnd > tarBytes.length) {
      addError(errors, 'archive_entry_truncated', 'archive entry body is truncated', name);
      break;
    }
    if (typeflag === '0' || typeflag === '') {
      entries.push({ name, bytes: tarBytes.subarray(bodyStart, bodyEnd) });
    }
    offset = bodyStart + Math.ceil(size / 512) * 512;
  }
  return entries;
}

function readSidecarSha256(sidecarPath, archivePath, errors) {
  if (!fs.existsSync(sidecarPath)) {
    addError(errors, 'archive_sha256_sidecar_missing', 'archive .sha256 sidecar is required', sidecarPath);
    return null;
  }
  const text = fs.readFileSync(sidecarPath, 'utf8').trim();
  const [sha, filename] = text.split(/\s+/);
  if (!/^[a-f0-9]{64}$/i.test(sha || '')) {
    addError(errors, 'archive_sha256_sidecar_invalid', 'archive .sha256 sidecar must start with a sha256 hex digest', sidecarPath);
    return null;
  }
  if (filename && filename !== path.basename(archivePath)) {
    addError(errors, 'archive_sha256_filename_mismatch', 'archive .sha256 filename does not match archive basename', sidecarPath);
  }
  return sha.toLowerCase();
}

function stripRoot(entries, errors) {
  const roots = new Set();
  const stripped = new Map();
  for (const entry of entries) {
    const normalized = path.posix.normalize(entry.name.replaceAll('\\', '/'));
    const parts = normalized.split('/');
    if (parts.length < 2) {
      addError(errors, 'archive_entry_root_missing', 'archive entries must be under a single package root', entry.name);
      continue;
    }
    roots.add(parts[0]);
    stripped.set(parts.slice(1).join('/'), entry.bytes);
  }
  if (roots.size !== 1) {
    addError(errors, 'archive_root_invalid', 'archive must contain exactly one package root directory', [...roots].join(','));
  }
  return {
    rootName: [...roots][0] || null,
    files: stripped,
  };
}

function validateManifestFiles(files, manifest, errors) {
  if (!Array.isArray(manifest.included_files)) {
    addError(errors, 'included_files_invalid', 'included_files must be an array', 'handoff-package-manifest.json');
    return;
  }
  for (const file of manifest.included_files) {
    const relativePath = String(file.path || '');
    if (!safeArchiveEntry(relativePath)) {
      addError(errors, 'included_file_path_invalid', 'included file path must stay inside package', relativePath);
      continue;
    }
    const bytes = files.get(relativePath);
    if (!bytes) {
      addError(errors, 'included_file_missing', 'included file is missing from archive', relativePath);
      continue;
    }
    if (Number(file.bytes) !== bytes.length) {
      addError(errors, 'included_file_size_mismatch', 'included file byte size does not match manifest', relativePath);
    }
    if (String(file.sha256 || '') !== sha256Hex(bytes)) {
      addError(errors, 'included_file_sha256_mismatch', 'included file sha256 does not match manifest', relativePath);
    }
  }
}

function validateArchive(archivePathInput, sidecarPathInput = '') {
  const errors = [];
  const archivePath = path.resolve(archivePathInput || '');
  const sidecarPath = path.resolve(sidecarPathInput || `${archivePath}.sha256`);
  let archiveBytes = Buffer.alloc(0);
  let sidecarSha256 = null;
  let actualSha256 = null;
  let entries = [];
  let files = new Map();
  let rootName = null;
  let manifest = {};
  let handoffValidation = null;

  if (!archivePathInput || !fs.existsSync(archivePath)) {
    addError(errors, 'archive_missing', 'archive file is required', archivePathInput || '');
  } else {
    archiveBytes = fs.readFileSync(archivePath);
    actualSha256 = sha256Hex(archiveBytes);
    sidecarSha256 = readSidecarSha256(sidecarPath, archivePath, errors);
    if (sidecarSha256 && sidecarSha256 !== actualSha256) {
      addError(errors, 'archive_sha256_mismatch', 'archive sha256 does not match sidecar', archivePath);
    }
    try {
      entries = parseTarEntries(zlib.gunzipSync(archiveBytes), errors);
      const stripped = stripRoot(entries, errors);
      files = stripped.files;
      rootName = stripped.rootName;
    } catch (error) {
      addError(errors, 'archive_gzip_or_tar_invalid', String(error?.message || error), archivePath);
    }
  }

  for (const required of REQUIRED_ENTRIES) {
    if (!files.has(required)) {
      addError(errors, 'required_archive_entry_missing', 'required archive entry is missing', required);
    }
  }

  if (files.has('handoff-package-manifest.json')) {
    try {
      manifest = JSON.parse(files.get('handoff-package-manifest.json').toString('utf8'));
    } catch (error) {
      addError(errors, 'package_manifest_invalid_json', String(error?.message || error), 'handoff-package-manifest.json');
    }
  }
  if (manifest.package_type !== PACKAGE_TYPE) {
    addError(errors, 'package_type_invalid', `package_type must be ${PACKAGE_TYPE}`, 'handoff-package-manifest.json');
  }
  if (manifest.package_root && manifest.package_root !== '.') {
    addError(errors, 'package_root_not_relative', 'package_root must be "." for sendable packages', 'handoff-package-manifest.json');
  }
  validateManifestFiles(files, manifest, errors);

  if (files.has('handoff/third-party-handoff.sample.json')) {
    try {
      handoffValidation = validateExternalHandoffManifest(
        JSON.parse(files.get('handoff/third-party-handoff.sample.json').toString('utf8')),
      );
      for (const error of handoffValidation.errors) {
        addError(errors, `handoff_${error.code}`, error.message, `handoff/third-party-handoff.sample.json:${error.path}`);
      }
    } catch (error) {
      addError(errors, 'handoff_manifest_invalid_json', String(error?.message || error), 'handoff/third-party-handoff.sample.json');
    }
  }

  const checks = [
    { key: 'archive_present', passed: errors.every((error) => error.code !== 'archive_missing') },
    { key: 'archive_sha256_sidecar', passed: errors.every((error) => !error.code.startsWith('archive_sha256')) },
    { key: 'archive_gzip_tar', passed: errors.every((error) => error.code !== 'archive_gzip_or_tar_invalid') },
    { key: 'archive_entry_paths', passed: errors.every((error) => error.code !== 'archive_entry_path_invalid') },
    { key: 'archive_single_root', passed: errors.every((error) => !['archive_root_invalid', 'archive_entry_root_missing'].includes(error.code)) },
    { key: 'required_entries', passed: errors.every((error) => error.code !== 'required_archive_entry_missing') },
    {
      key: 'included_file_integrity',
      passed: errors.every((error) => !['included_file_missing', 'included_file_size_mismatch', 'included_file_sha256_mismatch'].includes(error.code)),
    },
    { key: 'handoff_manifest', passed: handoffValidation?.ready_for_customer_sandbox === true },
  ];

  return {
    report_type: 'external_third_party_handoff_archive_validation',
    archive_ready: errors.length === 0,
    archive_path: archivePath,
    archive_sha256: actualSha256,
    sidecar_path: sidecarPath,
    root_name: rootName,
    entry_count: entries.length,
    checks,
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
  const inferredArchivePath = path.resolve(process.cwd(), '..', `${path.basename(process.cwd())}.tar.gz`);
  const result = validateArchive(args.archive || inferredArchivePath, args.sha256);
  console.log(JSON.stringify(result, null, 2));
  if (!result.archive_ready) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export { REQUIRED_ENTRIES, validateArchive };
