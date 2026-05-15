import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';
import { buildPackage } from './build-external-handoff-package.mjs';
import { validateArchive } from './validate-external-handoff-archive.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function sha256Hex(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

function writeTarOctal(header, value, offset, length) {
  const raw = Math.trunc(Number(value) || 0).toString(8);
  const text = raw.padStart(length - 1, '0').slice(-(length - 1));
  header.write(`${text}\0`, offset, length, 'ascii');
}

function writeTarChecksum(header, checksum) {
  const text = checksum.toString(8).padStart(6, '0').slice(-6);
  header.write(`${text}\0 `, 148, 8, 'ascii');
}

function tarHeader(name, size) {
  const header = Buffer.alloc(512, 0);
  header.write(name, 0, 100, 'utf8');
  writeTarOctal(header, 0o644, 100, 8);
  writeTarOctal(header, 0, 108, 8);
  writeTarOctal(header, 0, 116, 8);
  writeTarOctal(header, size, 124, 12);
  writeTarOctal(header, 0, 136, 12);
  header.fill(0x20, 148, 156);
  header.write('0', 156, 1, 'ascii');
  header.write('ustar\0', 257, 6, 'ascii');
  header.write('00', 263, 2, 'ascii');
  let checksum = 0;
  for (const byte of header) {
    checksum += byte;
  }
  writeTarChecksum(header, checksum);
  return header;
}

function tarPadding(size) {
  const remainder = size % 512;
  return remainder === 0 ? Buffer.alloc(0) : Buffer.alloc(512 - remainder, 0);
}

function tarString(buffer, start, end) {
  return buffer.toString('utf8', start, end).replace(/\0.*$/, '');
}

function tarNumber(buffer, start, end) {
  const text = buffer.toString('ascii', start, end).replace(/\0.*$/, '').trim();
  return Number.parseInt(text || '0', 8);
}

function parseTarEntries(tarBytes) {
  const entries = [];
  let offset = 0;
  while (offset + 512 <= tarBytes.length) {
    const header = tarBytes.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) {
      break;
    }
    const name = tarString(header, 0, 100);
    const prefix = tarString(header, 345, 500);
    const size = tarNumber(header, 124, 136);
    const bodyStart = offset + 512;
    const bodyEnd = bodyStart + size;
    entries.push({
      name: prefix ? `${prefix}/${name}` : name,
      body: tarBytes.subarray(bodyStart, bodyEnd),
    });
    offset = bodyStart + Math.ceil(size / 512) * 512;
  }
  return entries;
}

function writeTarEntries(entries) {
  return Buffer.concat([
    ...entries.flatMap((entry) => [
      tarHeader(entry.name, entry.body.length),
      entry.body,
      tarPadding(entry.body.length),
    ]),
    Buffer.alloc(1024, 0),
  ]);
}

function rewriteArchiveEntries(archivePath, rewrite) {
  const entries = parseTarEntries(zlib.gunzipSync(fs.readFileSync(archivePath)));
  const rewritten = rewrite(entries);
  const archiveBytes = zlib.gzipSync(writeTarEntries(rewritten), { level: 9, mtime: 0 });
  fs.writeFileSync(archivePath, archiveBytes);
  fs.writeFileSync(`${archivePath}.sha256`, `${sha256Hex(archiveBytes)}  ${path.basename(archivePath)}\n`);
}

function writeTinyArchive({ outDir, name, entryName, body = 'x\n' }) {
  const bodyBytes = Buffer.from(body);
  const tarBytes = Buffer.concat([
    tarHeader(entryName, bodyBytes.length),
    bodyBytes,
    tarPadding(bodyBytes.length),
    Buffer.alloc(1024, 0),
  ]);
  const archivePath = path.join(outDir, name);
  const archiveBytes = zlib.gzipSync(tarBytes, { level: 9, mtime: 0 });
  fs.writeFileSync(archivePath, archiveBytes);
  fs.writeFileSync(`${archivePath}.sha256`, `${sha256Hex(archiveBytes)}  ${name}\n`);
  return archivePath;
}

test('validateArchive accepts a generated archive and sidecar', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-archive-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'archive-valid-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });

  const result = validateArchive(built.archivePath);

  assert.equal(result.archive_ready, true);
  assert.equal(result.archive_sha256, built.archiveSha256);
  assert.equal(result.root_name, 'archive-valid-package');
  assert.equal(result.checks.find((check) => check.key === 'included_file_integrity').passed, true);
  assert.equal(result.checks.find((check) => check.key === 'third_party_html_artifact_manifest').passed, true);
  assert.equal(result.html_artifact_validation.artifact_ready, true);
  assert.equal(result.html_artifact_validation.template_id, 'third_party_handoff_document');
  assert.equal(result.handoff_validation.ready_for_customer_sandbox, true);
  assert.equal(result.errors.length, 0);
});

test('validateArchive accepts ustar-prefixed long document paths', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-archive-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'external-third-party-handoff-package-20260515T000000Z',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });

  const result = validateArchive(built.archivePath);

  assert.equal(result.archive_ready, true);
  assert.equal(result.root_name, 'external-third-party-handoff-package-20260515T000000Z');
  assert.equal(result.errors.length, 0);
});

test('validateArchive rejects a sidecar digest mismatch', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-archive-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'archive-mismatch-package',
    generatedAt: '2026-05-14T00:00:00.000Z',
  });
  fs.writeFileSync(built.archiveSha256Path, `${'0'.repeat(64)}  ${path.basename(built.archivePath)}\n`);

  const result = validateArchive(built.archivePath);

  assert.equal(result.archive_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'archive_sha256_mismatch'));
});

test('validateArchive rejects unsafe third-party handoff HTML artifact manifests', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-archive-'));
  const built = buildPackage({
    repoRoot,
    outDir,
    basename: 'archive-html-artifact-tamper',
    generatedAt: '2026-05-15T00:00:00.000Z',
  });
  rewriteArchiveEntries(built.archivePath, (entries) => {
    const artifactName = 'archive-html-artifact-tamper/html-artifacts/third-party-handoff-document.json';
    const manifestName = 'archive-html-artifact-tamper/handoff-package-manifest.json';
    const artifactEntry = entries.find((entry) => entry.name === artifactName);
    const manifestEntry = entries.find((entry) => entry.name === manifestName);
    assert.ok(artifactEntry, 'HTML artifact entry should exist in archive');
    assert.ok(manifestEntry, 'package manifest entry should exist in archive');
    const artifact = JSON.parse(artifactEntry.body.toString('utf8'));
    artifact.template_id = 'codex_execution_report';
    artifact.payload.summary = 'unsafe source https://example.com/internal';
    artifactEntry.body = Buffer.from(`${JSON.stringify(artifact, null, 2)}\n`);
    const manifest = JSON.parse(manifestEntry.body.toString('utf8'));
    const included = manifest.included_files.find((file) => file.path === 'html-artifacts/third-party-handoff-document.json');
    assert.ok(included, 'HTML artifact should be listed in package manifest');
    included.bytes = artifactEntry.body.length;
    included.sha256 = sha256Hex(artifactEntry.body);
    manifestEntry.body = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`);
    return entries;
  });

  const result = validateArchive(built.archivePath);

  assert.equal(result.archive_ready, false);
  assert.equal(result.checks.find((check) => check.key === 'included_file_integrity').passed, true);
  assert.equal(result.checks.find((check) => check.key === 'third_party_html_artifact_manifest').passed, false);
  assert.equal(result.html_artifact_validation.artifact_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'html_artifact_template_invalid'));
  assert.ok(result.errors.some((error) => error.code === 'html_artifact_payload_unsafe'));
});

test('validateArchive rejects path traversal entries', () => {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-external-handoff-archive-'));
  const archivePath = writeTinyArchive({
    outDir,
    name: 'archive-path-escape.tar.gz',
    entryName: 'package/../outside.txt',
  });

  const result = validateArchive(archivePath);

  assert.equal(result.archive_ready, false);
  assert.ok(result.errors.some((error) => error.code === 'archive_entry_path_invalid'));
});
