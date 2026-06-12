export async function fingerprintLocalSecret(secretValue, options = {}) {
  const normalized = String(secretValue || '').trim();
  if (!normalized) {
    return '';
  }
  const cryptoImpl = Object.prototype.hasOwnProperty.call(options, 'crypto')
    ? options.crypto
    : globalThis.crypto;
  if (!cryptoImpl?.subtle) {
    throw new Error('当前浏览器不支持本地密钥指纹计算。');
  }
  const bytes = new TextEncoder().encode(normalized);
  const digest = await cryptoImpl.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
}
