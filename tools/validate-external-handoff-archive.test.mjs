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
