import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { describe, it } from 'node:test';

import { fingerprintLocalSecret } from './local-secret-fingerprint.js';

describe('local secret fingerprint helpers', () => {
  it('returns a sha256 hex fingerprint for trimmed local secret values', async () => {
    const expected = createHash('sha256').update('local-key').digest('hex');

    assert.equal(await fingerprintLocalSecret('  local-key  '), expected);
  });

  it('returns an empty fingerprint for empty secret values', async () => {
    assert.equal(await fingerprintLocalSecret(''), '');
    assert.equal(await fingerprintLocalSecret('   '), '');
    assert.equal(await fingerprintLocalSecret(null), '');
  });

  it('throws the existing browser capability message when subtle crypto is unavailable', async () => {
    await assert.rejects(
      () => fingerprintLocalSecret('local-key', { crypto: {} }),
      /当前浏览器不支持本地密钥指纹计算。/,
    );
  });
});
