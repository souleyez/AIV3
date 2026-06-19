const DEFAULT_CODEX_CONTROL_BASE_URL = 'https://ad.goods-editor.com';

function trimValue(value) {
  return String(value || '').trim();
}

export function codexControlBaseUrl() {
  return trimValue(process.env.V3_CODEX_CONTROL_BASE_URL)
    .replace(/\/+$/, '')
    || DEFAULT_CODEX_CONTROL_BASE_URL;
}

export function codexControlBillingUrl() {
  return `${codexControlBaseUrl()}/codex/billing`;
}

export function codexControlAdminUrl() {
  return 'https://souleye.cc/codex/quota-admin';
}

export function codexQuotaServiceToken() {
  return trimValue(process.env.V3_CODEX_QUOTA_SERVICE_TOKEN);
}

export function codexControlUrl(pathname, search = '') {
  const normalizedPath = pathname.startsWith('/') ? pathname : `/${pathname}`;
  const normalizedSearch = search
    ? search.startsWith('?') ? search : `?${search}`
    : '';
  return `${codexControlBaseUrl()}${normalizedPath}${normalizedSearch}`;
}

export async function fetchCodexControlJson(pathname, { search = '', serviceToken = false } = {}) {
  const headers = new Headers({ accept: 'application/json' });
  if (serviceToken) {
    const token = codexQuotaServiceToken();
    if (token) {
      headers.set('authorization', `Bearer ${token}`);
    }
  }

  const response = await fetch(codexControlUrl(pathname, search), {
    headers,
    cache: 'no-store',
  });
  const payload = await response.json().catch(() => ({}));
  return { ok: response.ok, status: response.status, payload };
}
