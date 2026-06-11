import crypto from 'node:crypto';

export const ADMIN_CONSOLE_COOKIE = 'v3_admin_console_access';

const ADMIN_COOKIE_MAX_AGE_SECONDS = 60 * 60 * 24 * 30;
const ADMIN_COOKIE_SALT = 'ai-data-platform-v3:admin-console';

export function configuredAdminAccessKey() {
  return String(
    process.env.ADMIN_CONSOLE_ACCESS_KEY
      || process.env.V3_ADMIN_CONSOLE_ACCESS_KEY
      || process.env.EXTERNAL_OBSERVABILITY_ACCESS_KEY
      || process.env.EXTERNAL_INTEGRATIONS_OBSERVATION_KEY
      || '',
  ).trim();
}

export function adminConsoleAccessRequired() {
  return Boolean(configuredAdminAccessKey());
}

function digestAdminAccessKey(key) {
  return crypto
    .createHash('sha256')
    .update(`${ADMIN_COOKIE_SALT}:${key}`)
    .digest('hex');
}

export function adminConsoleCookieValue() {
  const key = configuredAdminAccessKey();
  return key ? digestAdminAccessKey(key) : '';
}

export function adminConsoleCookieOptions({ secure = true } = {}) {
  return {
    httpOnly: true,
    sameSite: 'lax',
    secure,
    path: '/',
    maxAge: ADMIN_COOKIE_MAX_AGE_SECONDS,
  };
}

export function hasAdminConsoleAccessCookieValue(value) {
  if (!adminConsoleAccessRequired()) {
    return true;
  }
  const expected = adminConsoleCookieValue();
  const actual = String(value || '').trim();
  if (!expected || !actual) {
    return false;
  }
  const expectedBuffer = Buffer.from(expected);
  const actualBuffer = Buffer.from(actual);
  return expectedBuffer.length === actualBuffer.length && crypto.timingSafeEqual(expectedBuffer, actualBuffer);
}

export function verifyAdminConsoleAccessKey(input) {
  const expectedKey = configuredAdminAccessKey();
  const candidate = String(input || '').trim();
  if (!expectedKey || !candidate) {
    return false;
  }
  const expected = Buffer.from(expectedKey);
  const actual = Buffer.from(candidate);
  return expected.length === actual.length && crypto.timingSafeEqual(expected, actual);
}
