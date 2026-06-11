import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {
  adminMicrosoftAuthEnabled,
  adminMicrosoftConfig,
  adminMicrosoftEmailAllowed,
  decodeAdminMicrosoftStateCookie,
  encodeAdminMicrosoftStateCookie,
  verifyMicrosoftIdToken,
} from './admin-microsoft-auth.js';

function withAdminMicrosoftEnv(updates, callback) {
  const names = [
    'ADMIN_CONSOLE_ACCESS_KEY',
    'ADMIN_CONSOLE_SESSION_SECRET',
    'ADMIN_MICROSOFT_AUTH_ENABLED',
    'ADMIN_MICROSOFT_TENANT_ID',
    'ADMIN_MICROSOFT_CLIENT_ID',
    'ADMIN_MICROSOFT_CLIENT_SECRET',
    'ADMIN_MICROSOFT_REDIRECT_URI',
    'ADMIN_MICROSOFT_ALLOWED_EMAILS',
    'ADMIN_MICROSOFT_ALLOWED_DOMAINS',
    'ADMIN_MICROSOFT_ISSUER',
    'AZURE_AD_TENANT_ID',
    'AZURE_AD_CLIENT_ID',
    'AZURE_AD_CLIENT_SECRET',
    'AZURE_AD_REDIRECT_URI',
  ];
  const previous = Object.fromEntries(names.map((name) => [name, process.env[name]]));
  try {
    names.forEach((name) => {
      delete process.env[name];
    });
    Object.entries(updates).forEach(([name, value]) => {
      process.env[name] = value;
    });
    return callback();
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

function b64urlJson(value) {
  return Buffer.from(JSON.stringify(value)).toString('base64url');
}

function signJwt(header, payload, privateKey) {
  const signingInput = `${b64urlJson(header)}.${b64urlJson(payload)}`;
  const signature = crypto.sign('RSA-SHA256', Buffer.from(signingInput), privateKey).toString('base64url');
  return `${signingInput}.${signature}`;
}

test('admin microsoft auth requires explicit enablement and complete config', () => {
  withAdminMicrosoftEnv({}, () => {
    assert.equal(adminMicrosoftAuthEnabled(), false);
  });
  withAdminMicrosoftEnv({
    ADMIN_MICROSOFT_AUTH_ENABLED: 'true',
    ADMIN_MICROSOFT_TENANT_ID: 'tenant-1',
    ADMIN_MICROSOFT_CLIENT_ID: 'client-1',
    ADMIN_MICROSOFT_CLIENT_SECRET: 'secret-1',
    ADMIN_MICROSOFT_REDIRECT_URI: 'https://v3.elepcloud.com/admin/microsoft/callback',
    ADMIN_CONSOLE_SESSION_SECRET: 'session-secret',
    ADMIN_MICROSOFT_ALLOWED_EMAILS: 'admin@example.com',
  }, () => {
    assert.equal(adminMicrosoftAuthEnabled(), true);
    assert.equal(adminMicrosoftConfig().clientId, 'client-1');
  });
});

test('admin microsoft state cookie is signed and tamper resistant', () => {
  withAdminMicrosoftEnv({
    ADMIN_MICROSOFT_TENANT_ID: 'tenant-1',
    ADMIN_MICROSOFT_CLIENT_ID: 'client-1',
    ADMIN_MICROSOFT_CLIENT_SECRET: 'secret-1',
    ADMIN_MICROSOFT_REDIRECT_URI: 'https://v3.elepcloud.com/admin/microsoft/callback',
    ADMIN_CONSOLE_SESSION_SECRET: 'session-secret',
  }, () => {
    const config = adminMicrosoftConfig();
    const value = encodeAdminMicrosoftStateCookie({
      v: 1,
      state: 'state-1',
      nonce: 'nonce-1',
      next: '/admin/external-integrations',
      issuedAt: Math.floor(Date.now() / 1000),
    }, config);

    assert.equal(decodeAdminMicrosoftStateCookie(value, config).next, '/admin/external-integrations');
    assert.equal(decodeAdminMicrosoftStateCookie(`${value}x`, config), null);
  });
});

test('admin microsoft email allowlist supports exact emails and domains', () => {
  withAdminMicrosoftEnv({
    ADMIN_MICROSOFT_TENANT_ID: 'tenant-1',
    ADMIN_MICROSOFT_CLIENT_ID: 'client-1',
    ADMIN_MICROSOFT_CLIENT_SECRET: 'secret-1',
    ADMIN_MICROSOFT_REDIRECT_URI: 'https://v3.elepcloud.com/admin/microsoft/callback',
    ADMIN_CONSOLE_SESSION_SECRET: 'session-secret',
    ADMIN_MICROSOFT_ALLOWED_EMAILS: 'Admin@Example.com',
    ADMIN_MICROSOFT_ALLOWED_DOMAINS: 'contoso.com',
  }, () => {
    const config = adminMicrosoftConfig();
    assert.equal(adminMicrosoftEmailAllowed('admin@example.com', config), true);
    assert.equal(adminMicrosoftEmailAllowed('user@contoso.com', config), true);
    assert.equal(adminMicrosoftEmailAllowed('user@other.com', config), false);
  });
});

test('admin microsoft id token verifier checks signature claims and allowlist', async () => {
  await withAdminMicrosoftEnv({
    ADMIN_MICROSOFT_TENANT_ID: 'tenant-1',
    ADMIN_MICROSOFT_CLIENT_ID: 'client-1',
    ADMIN_MICROSOFT_CLIENT_SECRET: 'secret-1',
    ADMIN_MICROSOFT_REDIRECT_URI: 'https://v3.elepcloud.com/admin/microsoft/callback',
    ADMIN_CONSOLE_SESSION_SECRET: 'session-secret',
    ADMIN_MICROSOFT_ALLOWED_EMAILS: 'admin@example.com',
    ADMIN_MICROSOFT_ISSUER: 'https://login.microsoftonline.com/tenant-1/v2.0',
  }, async () => {
    const { publicKey, privateKey } = crypto.generateKeyPairSync('rsa', { modulusLength: 2048 });
    const jwk = publicKey.export({ format: 'jwk' });
    jwk.kid = 'kid-1';
    jwk.use = 'sig';
    jwk.alg = 'RS256';
    const now = Math.floor(Date.now() / 1000);
    const token = signJwt(
      { alg: 'RS256', typ: 'JWT', kid: 'kid-1' },
      {
        aud: 'client-1',
        iss: 'https://login.microsoftonline.com/tenant-1/v2.0',
        exp: now + 300,
        nbf: now - 10,
        nonce: 'nonce-1',
        preferred_username: 'Admin@Example.com',
        tid: 'tenant-1',
        sub: 'subject-1',
        name: 'Admin User',
      },
      privateKey,
    );

    const identity = await verifyMicrosoftIdToken(token, adminMicrosoftConfig(), 'nonce-1', { keys: [jwk] });
    assert.equal(identity.email, 'admin@example.com');
    assert.equal(identity.tenantId, 'tenant-1');
  });
});
