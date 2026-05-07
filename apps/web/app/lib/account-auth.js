const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const DEFAULT_AUTH_PURPOSE = 'login';

export function normalizeAccountEmail(value) {
  return String(value || '').trim().toLowerCase();
}

export function validateAccountEmail(value) {
  const email = normalizeAccountEmail(value);
  return {
    email,
    valid: EMAIL_PATTERN.test(email),
  };
}

export function normalizeVerificationCode(value) {
  return String(value || '').replace(/\D/g, '').slice(0, 8);
}

export function buildDeviceFingerprint() {
  if (typeof window === 'undefined') {
    return 'server-render-device';
  }
  const userAgent = window.navigator?.userAgent || 'unknown-browser';
  const language = window.navigator?.language || 'unknown-language';
  const screenSize = window.screen ? `${window.screen.width}x${window.screen.height}` : 'unknown-screen';
  return `${userAgent}|${language}|${screenSize}`.slice(0, 240);
}

export function buildStartEmailAuthPayload(email, purpose = DEFAULT_AUTH_PURPOSE, deviceFingerprint = '') {
  return {
    email: normalizeAccountEmail(email),
    purpose,
    ...(deviceFingerprint ? { device_fingerprint: deviceFingerprint } : {}),
  };
}

export function buildVerifyEmailAuthPayload(email, code, purpose = DEFAULT_AUTH_PURPOSE, deviceFingerprint = '') {
  return {
    email: normalizeAccountEmail(email),
    code: normalizeVerificationCode(code),
    purpose,
    ...(deviceFingerprint ? { device_fingerprint: deviceFingerprint } : {}),
  };
}

export function buildKeyLoginPayload(email, localKey, deviceFingerprint = '') {
  return {
    email: normalizeAccountEmail(email),
    local_key: String(localKey || '').trim(),
    ...(deviceFingerprint ? { device_fingerprint: deviceFingerprint } : {}),
  };
}

export function summarizeAccountState({ user, session, activeSecretCount = 0 } = {}) {
  if (user?.email && session?.auth_method === 'email_key') {
    return {
      label: user.email,
      detail: activeSecretCount
        ? `邮箱密钥已登录 · 已启用 ${activeSecretCount} 个本地绑定`
        : '邮箱密钥已登录 · 当前未解锁私密绑定',
      signedIn: true,
    };
  }
  if (user?.email) {
    return {
      label: user.email,
      detail: '邮箱验证码已登录 · 数据集和产物会归属当前账号',
      signedIn: true,
    };
  }
  if (activeSecretCount > 0) {
    return {
      label: '本地密钥模式',
      detail: `未绑定邮箱 · 已启用 ${activeSecretCount} 个本地绑定`,
      signedIn: false,
    };
  }
  return {
    label: '未登录',
    detail: '可普通聊天；私密数据集需要邮箱或本地密钥',
    signedIn: false,
  };
}
