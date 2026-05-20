import crypto from 'node:crypto';

export const EXTERNAL_OBSERVABILITY_COOKIE = 'v3_external_observability_access';

const ACCESS_COOKIE_MAX_AGE_SECONDS = 60 * 60 * 12;
const ACCESS_COOKIE_SALT = 'ai-data-platform-v3:external-observability';

function configuredAccessKey() {
  return String(
    process.env.EXTERNAL_OBSERVABILITY_ACCESS_KEY
      || process.env.EXTERNAL_INTEGRATIONS_OBSERVATION_KEY
      || '',
  ).trim();
}

function digestAccessKey(key) {
  return crypto
    .createHash('sha256')
    .update(`${ACCESS_COOKIE_SALT}:${key}`)
    .digest('hex');
}

function cookieValueFromHeader(cookieHeader, name) {
  return String(cookieHeader || '')
    .split(';')
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const separator = part.indexOf('=');
      if (separator < 0) {
        return [part, ''];
      }
      return [part.slice(0, separator), part.slice(separator + 1)];
    })
    .find(([cookieName]) => cookieName === name)?.[1] || '';
}

export function externalObservabilityAccessRequired() {
  return Boolean(configuredAccessKey());
}

export function externalObservabilityCookieOptions({ secure = true } = {}) {
  return {
    httpOnly: true,
    sameSite: 'lax',
    secure,
    path: '/',
    maxAge: ACCESS_COOKIE_MAX_AGE_SECONDS,
  };
}

export function externalObservabilityCookieValue() {
  const key = configuredAccessKey();
  return key ? digestAccessKey(key) : '';
}

export function externalObservabilityProxyHeaderValue() {
  return configuredAccessKey();
}

export function verifyExternalObservabilityKey(input) {
  const expectedKey = configuredAccessKey();
  if (!expectedKey) {
    return true;
  }
  const candidate = String(input || '').trim();
  if (!candidate) {
    return false;
  }
  const expected = Buffer.from(expectedKey);
  const actual = Buffer.from(candidate);
  return expected.length === actual.length && crypto.timingSafeEqual(expected, actual);
}

export function hasExternalObservabilityAccessCookie(cookieHeader) {
  if (!externalObservabilityAccessRequired()) {
    return true;
  }
  const expected = externalObservabilityCookieValue();
  const actual = cookieValueFromHeader(cookieHeader, EXTERNAL_OBSERVABILITY_COOKIE);
  return Boolean(actual) && actual === expected;
}
