import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT_DIR = path.resolve(fileURLToPath(new URL('..', import.meta.url)));

const PUBLIC_CONTRACT_DIRS = [
  'docs/integrations',
  'apps/web/public/external-integrations',
];

const FORBIDDEN_UNREVIEWED_ASSET_CONTRACTS = [
  /\basset-imports\b/i,
  /\bfashion-design-images\b/i,
  /\basset_library_external_id\b/i,
  /\basset_library_external_ids\b/i,
  /\basset_collection_external_id\b/i,
  /\basset_collection_external_ids\b/i,
  /\basset_external_id\b/i,
  /\basset_external_ids\b/i,
];

function listPublicContractFiles() {
  const files = [];
  for (const relativeDir of PUBLIC_CONTRACT_DIRS) {
    const absoluteDir = path.join(ROOT_DIR, relativeDir);
    if (!fs.existsSync(absoluteDir)) {
      continue;
    }
    for (const name of fs.readdirSync(absoluteDir)) {
      const absolutePath = path.join(absoluteDir, name);
      const stat = fs.statSync(absolutePath);
      if (!stat.isFile()) {
        continue;
      }
      if (/\.(md|html)$/i.test(name)) {
        files.push(absolutePath);
      }
    }
  }
  return files.sort();
}

test('public third-party docs do not expose unreviewed asset import contracts', () => {
  const files = listPublicContractFiles();
  assert.ok(files.length > 0, 'expected public third-party contract files to exist');

  const violations = [];
  for (const file of files) {
    const text = fs.readFileSync(file, 'utf8');
    for (const pattern of FORBIDDEN_UNREVIEWED_ASSET_CONTRACTS) {
      if (pattern.test(text)) {
        violations.push({
          file: path.relative(ROOT_DIR, file),
          pattern: String(pattern),
        });
      }
    }
  }

  assert.deepEqual(violations, []);
});
