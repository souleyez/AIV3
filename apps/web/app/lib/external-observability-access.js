import crypto from 'node:crypto';
import fs from 'node:fs';

export const EXTERNAL_OBSERVABILITY_COOKIE = 'v3_external_observability_access';

const ACCESS_COOKIE_MAX_AGE_SECONDS = 60 * 60 * 24 * 400;
const ACCESS_COOKIE_SALT = 'ai-data-platform-v3:external-observability';
const ONE_TIME_KEY_SALT = 'ai-data-platform-v3:external-observability:one-time';

function configuredRawAccessKey() {
  return String(
    process.env.EXTERNAL_OBSERVABILITY_ACCESS_KEY
      || process.env.EXTERNAL_INTEGRATIONS_OBSERVATION_KEY
      || '',
  ).trim();
}

function staticAccessKeyDisabled() {
  return String(process.env.EXTERNAL_OBSERVABILITY_DISABLE_STATIC_ACCESS_KEY || '').trim() === '1';
}

function configuredUserAccessKey() {
  if (staticAccessKeyDisabled()) {
    return '';
  }
  return configuredRawAccessKey();
}

function configuredProxyAccessKey() {
  return String(
    process.env.EXTERNAL_OBSERVABILITY_PROXY_ACCESS_KEY
      || process.env.EXTERNAL_OBSERVABILITY_BACKEND_ACCESS_KEY
      || configuredRawAccessKey(),
  ).trim();
}

function oneTimeKeysFile() {
  return String(process.env.EXTERNAL_OBSERVABILITY_ONE_TIME_KEYS_FILE || '').trim();
}

function digestAccessKey(key) {
  return crypto
    .createHash('sha256')
    .update(`${ACCESS_COOKIE_SALT}:${key}`)
    .digest('hex');
}

function digestOneTimeAccessKey(key) {
  return crypto
    .createHash('sha256')
    .update(`${ONE_TIME_KEY_SALT}:${key}`)
    .digest('hex');
}

function readOneTimeKeyStore(filePath) {
  if (!filePath) {
    return { keys: [] };
  }
  try {
    const parsed = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    return {
      ...parsed,
      keys: Array.isArray(parsed?.keys) ? parsed.keys : [],
    };
  } catch {
    return { keys: [] };
  }
}

function writeOneTimeKeyStore(filePath, store) {
  const payload = `${JSON.stringify(store, null, 2)}\n`;
  fs.writeFileSync(filePath, payload, { encoding: 'utf8', mode: 0o600 });
}

function oneTimeKeyStoreHasActiveKey() {
  const filePath = oneTimeKeysFile();
  if (!filePath) {
    return false;
  }
  const now = Date.now();
  const store = readOneTimeKeyStore(filePath);
  return store.keys.some((key) => {
    if (!key || key.consumed_at) {
      return false;
    }
    const expiresAt = key.expires_at ? Date.parse(key.expires_at) : NaN;
    return Number.isNaN(expiresAt) || expiresAt > now;
  });
}

function consumeOneTimeAccessKey(input) {
  const filePath = oneTimeKeysFile();
  if (!filePath) {
    return false;
  }
  const candidate = String(input || '').trim();
  if (!candidate) {
    return false;
  }
  const candidateDigest = digestOneTimeAccessKey(candidate);
  const store = readOneTimeKeyStore(filePath);
  const now = new Date();
  const key = store.keys.find((item) => item?.sha256 === candidateDigest);
  if (!key || key.consumed_at) {
    return false;
  }
  const expiresAt = key.expires_at ? Date.parse(key.expires_at) : NaN;
  if (!Number.isNaN(expiresAt) && expiresAt <= now.getTime()) {
    return false;
  }
  key.consumed_at = now.toISOString();
  writeOneTimeKeyStore(filePath, store);
  return true;
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
  return Boolean(configuredProxyAccessKey() || configuredUserAccessKey() || oneTimeKeyStoreHasActiveKey());
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
  const key = configuredProxyAccessKey() || configuredUserAccessKey();
  return key ? digestAccessKey(key) : '';
}

export function externalObservabilityProxyHeaderValue() {
  return configuredProxyAccessKey();
}

export function verifyExternalObservabilityKey(input) {
  const expectedKey = configuredUserAccessKey();
  if (!expectedKey) {
    return false;
  }
  const candidate = String(input || '').trim();
  if (!candidate) {
    return false;
  }
  const expected = Buffer.from(expectedKey);
  const actual = Buffer.from(candidate);
  return expected.length === actual.length && crypto.timingSafeEqual(expected, actual);
}

export function consumeExternalObservabilityAccessKey(input) {
  return verifyExternalObservabilityKey(input) || consumeOneTimeAccessKey(input);
}

export function hasExternalObservabilityAccessCookie(cookieHeader) {
  if (!externalObservabilityAccessRequired()) {
    return true;
  }
  const expected = externalObservabilityCookieValue();
  const actual = cookieValueFromHeader(cookieHeader, EXTERNAL_OBSERVABILITY_COOKIE);
  return Boolean(actual) && actual === expected;
}
