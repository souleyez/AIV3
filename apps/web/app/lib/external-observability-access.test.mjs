import test from 'node:test';
import assert from 'node:assert/strict';
import {
  EXTERNAL_OBSERVABILITY_COOKIE,
  externalObservabilityAccessRequired,
  externalObservabilityCookieValue,
  externalObservabilityProxyHeaderValue,
  hasExternalObservabilityAccessCookie,
  verifyExternalObservabilityKey,
} from './external-observability-access.js';

test('external observability access only locks protected data when key is configured', () => {
  const previous = process.env.EXTERNAL_OBSERVABILITY_ACCESS_KEY;
  const previousAlias = process.env.EXTERNAL_INTEGRATIONS_OBSERVATION_KEY;
  try {
    delete process.env.EXTERNAL_OBSERVABILITY_ACCESS_KEY;
    delete process.env.EXTERNAL_INTEGRATIONS_OBSERVATION_KEY;
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
  } finally {
    if (previous === undefined) {
      delete process.env.EXTERNAL_OBSERVABILITY_ACCESS_KEY;
    } else {
      process.env.EXTERNAL_OBSERVABILITY_ACCESS_KEY = previous;
    }
    if (previousAlias === undefined) {
      delete process.env.EXTERNAL_INTEGRATIONS_OBSERVATION_KEY;
    } else {
      process.env.EXTERNAL_INTEGRATIONS_OBSERVATION_KEY = previousAlias;
    }
  }
});
