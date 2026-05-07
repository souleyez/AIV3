import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildKeyLoginPayload,
  buildStartEmailAuthPayload,
  buildVerifyEmailAuthPayload,
  normalizeAccountEmail,
  normalizeVerificationCode,
  summarizeAccountState,
  validateAccountEmail,
} from './account-auth.js';

test('account auth normalizes and validates email input', () => {
  assert.equal(normalizeAccountEmail('  User@Example.COM '), 'user@example.com');
  assert.deepEqual(validateAccountEmail('user@example.com'), {
    email: 'user@example.com',
    valid: true,
  });
  assert.equal(validateAccountEmail('bad-email').valid, false);
});

test('account auth builds backend payloads with snake case fields', () => {
  assert.deepEqual(buildStartEmailAuthPayload('USER@example.COM', 'login', 'device-1'), {
    email: 'user@example.com',
    purpose: 'login',
    device_fingerprint: 'device-1',
  });
  assert.deepEqual(buildVerifyEmailAuthPayload('USER@example.COM', '12 3a456', 'recover_key'), {
    email: 'user@example.com',
    code: '123456',
    purpose: 'recover_key',
  });
  assert.deepEqual(buildKeyLoginPayload('USER@example.COM', '  secret-key  ', 'device-2'), {
    email: 'user@example.com',
    local_key: 'secret-key',
    device_fingerprint: 'device-2',
  });
});

test('account auth summarizes local key and account states', () => {
  assert.match(summarizeAccountState().detail, /普通聊天/);
  assert.match(summarizeAccountState({ activeSecretCount: 2 }).detail, /2 个本地绑定/);
  assert.equal(
    summarizeAccountState({
      user: { email: 'user@example.com' },
      session: { auth_method: 'email_key' },
      activeSecretCount: 1,
    }).signedIn,
    true,
  );
});

test('verification code keeps only digits and caps length', () => {
  assert.equal(normalizeVerificationCode(' 12-34 abc 567890 '), '12345678');
});
