import crypto from 'node:crypto';
import { configuredAdminAccessKey } from './admin-console-access.js';

export const ADMIN_MICROSOFT_STATE_COOKIE = 'datamax_admin_microsoft_state';

const STATE_MAX_AGE_SECONDS = 10 * 60;
const STATE_COOKIE_VERSION = 1;
const CLOCK_SKEW_SECONDS = 120;

function env(name) {
  return String(process.env[name] || '').trim();
}

function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value || '').trim());
}

function splitList(value) {
  return String(value || '')
    .split(/[\s,;]+/)
    .map((item) => item.trim().toLowerCase())
    .filter(Boolean);
}

function base64UrlEncode(value) {
  return Buffer.from(value)
    .toString('base64')
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');
}

function base64UrlDecode(value) {
  const normalized = String(value || '').replace(/-/g, '+').replace(/_/g, '/');
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=');
  return Buffer.from(padded, 'base64');
}

function sessionSecret() {
  return env('ADMIN_CONSOLE_SESSION_SECRET')
    || configuredAdminAccessKey()
    || env('NEXTAUTH_SECRET')
    || '';
}

export function adminMicrosoftAuthEnabled() {
  const config = adminMicrosoftConfig();
  return truthy(env('ADMIN_MICROSOFT_AUTH_ENABLED') || env('V3_ADMIN_MICROSOFT_AUTH_ENABLED'))
    && Boolean(config)
    && Boolean(config.allowedEmails.length || config.allowedDomains.length);
}

export function adminMicrosoftConfig(requestUrl = null) {
  const tenantId = env('ADMIN_MICROSOFT_TENANT_ID') || env('AZURE_AD_TENANT_ID');
  const clientId = env('ADMIN_MICROSOFT_CLIENT_ID') || env('AZURE_AD_CLIENT_ID');
  const clientSecret = env('ADMIN_MICROSOFT_CLIENT_SECRET') || env('AZURE_AD_CLIENT_SECRET');
  const secret = sessionSecret();
  if (!tenantId || !clientId || !clientSecret || !secret) {
    return null;
  }
  const origin = requestUrl ? new URL(requestUrl).origin : '';
  const redirectUri = env('ADMIN_MICROSOFT_REDIRECT_URI')
    || env('AZURE_AD_REDIRECT_URI')
    || (origin ? `${origin}/admin/microsoft/callback` : '');
  if (!redirectUri) {
    return null;
  }
  return {
    tenantId,
    clientId,
    clientSecret,
    redirectUri,
    issuer: env('ADMIN_MICROSOFT_ISSUER'),
    allowedEmails: splitList(env('ADMIN_MICROSOFT_ALLOWED_EMAILS') || env('ADMIN_CONSOLE_ALLOWED_EMAILS')),
    allowedDomains: splitList(env('ADMIN_MICROSOFT_ALLOWED_DOMAINS')),
    secret,
  };
}

export function adminMicrosoftCookieOptions({ secure = true } = {}) {
  return {
    httpOnly: true,
    sameSite: 'lax',
    secure,
    path: '/admin',
    maxAge: STATE_MAX_AGE_SECONDS,
  };
}

export function safeAdminNextPath(value) {
  const next = String(value || '').trim();
  return next.startsWith('/admin') && !next.startsWith('/admin/microsoft') ? next : '/admin';
}

export function createAdminMicrosoftState({ next = '/admin' } = {}) {
  return {
    v: STATE_COOKIE_VERSION,
    state: crypto.randomBytes(24).toString('hex'),
    nonce: crypto.randomBytes(24).toString('hex'),
    next: safeAdminNextPath(next),
    issuedAt: Math.floor(Date.now() / 1000),
  };
}

function signStatePayload(payload, secret) {
  return crypto.createHmac('sha256', secret).update(payload).digest('base64url');
}

export function encodeAdminMicrosoftStateCookie(state, config = adminMicrosoftConfig()) {
  if (!config?.secret) {
    throw new Error('admin microsoft auth is not configured');
  }
  const payload = base64UrlEncode(JSON.stringify(state));
  const signature = signStatePayload(payload, config.secret);
  return `${payload}.${signature}`;
}

export function decodeAdminMicrosoftStateCookie(value, config = adminMicrosoftConfig()) {
  if (!config?.secret) {
    return null;
  }
  const [payload, signature] = String(value || '').split('.');
  if (!payload || !signature) {
    return null;
  }
  const expected = signStatePayload(payload, config.secret);
  const expectedBuffer = Buffer.from(expected);
  const actualBuffer = Buffer.from(signature);
  if (expectedBuffer.length !== actualBuffer.length || !crypto.timingSafeEqual(expectedBuffer, actualBuffer)) {
    return null;
  }
  let parsed;
  try {
    parsed = JSON.parse(base64UrlDecode(payload).toString('utf8'));
  } catch {
    return null;
  }
  const issuedAt = Number(parsed?.issuedAt || 0);
  const now = Math.floor(Date.now() / 1000);
  if (parsed?.v !== STATE_COOKIE_VERSION || !parsed?.state || !parsed?.nonce || now - issuedAt > STATE_MAX_AGE_SECONDS) {
    return null;
  }
  return {
    state: String(parsed.state),
    nonce: String(parsed.nonce),
    next: safeAdminNextPath(parsed.next),
    issuedAt,
  };
}

export function buildAdminMicrosoftAuthorizeUrl({ requestUrl, next = '/admin' }) {
  const config = adminMicrosoftConfig(requestUrl);
  if (!config) {
    return null;
  }
  const state = createAdminMicrosoftState({ next });
  const url = new URL(`https://login.microsoftonline.com/${encodeURIComponent(config.tenantId)}/oauth2/v2.0/authorize`);
  url.searchParams.set('client_id', config.clientId);
  url.searchParams.set('response_type', 'code');
  url.searchParams.set('redirect_uri', config.redirectUri);
  url.searchParams.set('response_mode', 'query');
  url.searchParams.set('scope', 'openid profile email');
  url.searchParams.set('state', state.state);
  url.searchParams.set('nonce', state.nonce);
  return {
    url,
    state,
    cookieValue: encodeAdminMicrosoftStateCookie(state, config),
    config,
  };
}

export function adminMicrosoftEmailAllowed(email, config = adminMicrosoftConfig()) {
  const normalized = String(email || '').trim().toLowerCase();
  if (!normalized || !normalized.includes('@')) {
    return false;
  }
  const allowedEmails = config?.allowedEmails || [];
  const allowedDomains = config?.allowedDomains || [];
  if (!allowedEmails.length && !allowedDomains.length) {
    return false;
  }
  const domain = normalized.split('@').pop();
  return allowedEmails.includes(normalized) || allowedDomains.includes(domain);
}

function decodeJwtSegment(token, index) {
  const part = String(token || '').split('.')[index];
  if (!part) {
    throw new Error('invalid_id_token');
  }
  return JSON.parse(base64UrlDecode(part).toString('utf8'));
}

function microsoftIssuerAllowed(issuer, config, tenantIdClaim) {
  if (config.issuer) {
    return issuer === config.issuer;
  }
  const tenantId = String(config.tenantId || '');
  if (!['common', 'organizations', 'consumers'].includes(tenantId.toLowerCase())) {
    return issuer === `https://login.microsoftonline.com/${tenantId}/v2.0`;
  }
  return Boolean(tenantIdClaim)
    && issuer === `https://login.microsoftonline.com/${tenantIdClaim}/v2.0`;
}

export async function fetchMicrosoftJwks(config) {
  const response = await fetch(`https://login.microsoftonline.com/${encodeURIComponent(config.tenantId)}/discovery/v2.0/keys`, {
    cache: 'no-store',
  });
  if (!response.ok) {
    throw new Error(`microsoft_jwks_failed:${response.status}`);
  }
  return response.json();
}

export async function exchangeMicrosoftCode({ code, config }) {
  const form = new URLSearchParams();
  form.set('client_id', config.clientId);
  form.set('client_secret', config.clientSecret);
  form.set('code', code);
  form.set('redirect_uri', config.redirectUri);
  form.set('grant_type', 'authorization_code');

  const response = await fetch(`https://login.microsoftonline.com/${encodeURIComponent(config.tenantId)}/oauth2/v2.0/token`, {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: form.toString(),
    cache: 'no-store',
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(`microsoft_token_failed:${response.status}:${payload.error || 'unknown'}`);
  }
  return payload;
}

export async function verifyMicrosoftIdToken(idToken, config, expectedNonce, jwks = null) {
  const parts = String(idToken || '').split('.');
  if (parts.length !== 3) {
    throw new Error('invalid_id_token');
  }
  const header = decodeJwtSegment(idToken, 0);
  const payload = decodeJwtSegment(idToken, 1);
  if (header.alg !== 'RS256' || !header.kid) {
    throw new Error('unsupported_id_token_alg');
  }
  const keySet = jwks || await fetchMicrosoftJwks(config);
  const jwk = (keySet.keys || []).find((key) => key.kid === header.kid && key.kty === 'RSA');
  if (!jwk) {
    throw new Error('microsoft_jwk_not_found');
  }
  const publicKey = crypto.createPublicKey({ key: jwk, format: 'jwk' });
  const verifier = crypto.createVerify('RSA-SHA256');
  verifier.update(`${parts[0]}.${parts[1]}`);
  verifier.end();
  if (!verifier.verify(publicKey, base64UrlDecode(parts[2]))) {
    throw new Error('id_token_signature_invalid');
  }
  const now = Math.floor(Date.now() / 1000);
  if (Number(payload.exp || 0) + CLOCK_SKEW_SECONDS < now) {
    throw new Error('id_token_expired');
  }
  if (payload.nbf && Number(payload.nbf) - CLOCK_SKEW_SECONDS > now) {
    throw new Error('id_token_not_yet_valid');
  }
  const audience = Array.isArray(payload.aud) ? payload.aud : [payload.aud];
  if (!audience.includes(config.clientId)) {
    throw new Error('id_token_audience_invalid');
  }
  if (String(payload.nonce || '') !== expectedNonce) {
    throw new Error('id_token_nonce_invalid');
  }
  if (!microsoftIssuerAllowed(String(payload.iss || ''), config, payload.tid)) {
    throw new Error('id_token_issuer_invalid');
  }
  const email = String(payload.preferred_username || payload.email || payload.upn || '').trim().toLowerCase();
  if (!adminMicrosoftEmailAllowed(email, config)) {
    throw new Error('admin_email_not_allowed');
  }
  return {
    email,
    name: String(payload.name || '').trim(),
    tenantId: String(payload.tid || '').trim(),
    subject: String(payload.sub || '').trim(),
  };
}
