import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  EXTERNAL_OBSERVABILITY_COOKIE,
  consumeExternalObservabilityAccessKey,
  externalObservabilityAccessRequired,
  externalObservabilityCookieValue,
  externalObservabilityProxyHeaderValue,
  hasExternalObservabilityAccessCookie,
  verifyExternalObservabilityKey,
} from './external-observability-access.js';

function oneTimeDigest(key) {
  return crypto
    .createHash('sha256')
    .update(`ai-data-platform-v3:external-observability:one-time:${key}`)
    .digest('hex');
}

function withExternalObservabilityEnv(updates, callback) {
  const names = [
    'EXTERNAL_OBSERVABILITY_ACCESS_KEY',
    'EXTERNAL_INTEGRATIONS_OBSERVATION_KEY',
    'EXTERNAL_OBSERVABILITY_PROXY_ACCESS_KEY',
    'EXTERNAL_OBSERVABILITY_BACKEND_ACCESS_KEY',
    'EXTERNAL_OBSERVABILITY_DISABLE_STATIC_ACCESS_KEY',
    'EXTERNAL_OBSERVABILITY_ONE_TIME_KEYS_FILE',
  ];
  const previous = Object.fromEntries(names.map((name) => [name, process.env[name]]));
  try {
    names.forEach((name) => {
      delete process.env[name];
    });
    Object.entries(updates).forEach(([name, value]) => {
      process.env[name] = value;
    });
    callback();
  } finally {
    names.forEach((name) => {
      if (previous[name] === undefined) {
        delete process.env[name];
      } else {
        process.env[name] = previous[name];
      }
    });
  }
}

test('external observability access only locks protected data when key is configured', () => {
  withExternalObservabilityEnv({}, () => {
    assert.equal(externalObservabilityAccessRequired(), false);
    assert.equal(hasExternalObservabilityAccessCookie(''), true);

    process.env.EXTERNAL_OBSERVABILITY_ACCESS_KEY = 'obs-secret';
    assert.equal(externalObservabilityAccessRequired(), true);
    assert.equal(verifyExternalObservabilityKey('obs-secret'), true);
    assert.equal(verifyExternalObservabilityKey('wrong'), false);

    const cookieValue = externalObservabilityCookieValue();
    assert.notEqual(cookieValue, 'obs-secret');
    assert.equal(externalObservabilityProxyHeaderValue(), 'obs-secret');
    assert.equal(hasExternalObservabilityAccessCookie(''), false);
    assert.equal(
      hasExternalObservabilityAccessCookie(`${EXTERNAL_OBSERVABILITY_COOKIE}=${cookieValue}`),
      true,
    );
  });
});

test('external observability supports one-time access keys without accepting static login', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'v3-one-time-key-'));
  const keyFile = path.join(tempDir, 'keys.json');
  fs.writeFileSync(
    keyFile,
    `${JSON.stringify({
      keys: [
        {
          id: 'test-key',
          sha256: oneTimeDigest('one-shot-secret'),
          created_at: new Date().toISOString(),
          consumed_at: null,
        },
      ],
    })}\n`,
  );

  withExternalObservabilityEnv({
    EXTERNAL_OBSERVABILITY_ACCESS_KEY: 'backend-secret',
    EXTERNAL_OBSERVABILITY_DISABLE_STATIC_ACCESS_KEY: '1',
    EXTERNAL_OBSERVABILITY_ONE_TIME_KEYS_FILE: keyFile,
  }, () => {
    assert.equal(externalObservabilityAccessRequired(), true);
    assert.equal(externalObservabilityProxyHeaderValue(), 'backend-secret');
    assert.equal(verifyExternalObservabilityKey('backend-secret'), false);
    assert.equal(consumeExternalObservabilityAccessKey('one-shot-secret'), true);
    assert.equal(consumeExternalObservabilityAccessKey('one-shot-secret'), false);
    const stored = JSON.parse(fs.readFileSync(keyFile, 'utf8'));
    assert.match(stored.keys[0].consumed_at, /^\d{4}-\d{2}-\d{2}T/);
  });
});
